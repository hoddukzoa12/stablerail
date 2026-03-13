use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token};

use crate::domain::core::{recompute_sphere, swap, update_caches, verify_invariant};
use crate::errors::OrbitalError;
use crate::events::{SwapExecuted, TickCrossed};
use crate::math::newton::compute_amount_out_analytical;
use crate::math::torus::{
    compute_delta_to_boundary, compute_new_alpha, find_nearest_tick_boundaries,
};
use crate::math::FixedPoint;
use crate::state::{PoolState, TickState, TickStatus};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SwapParams {
    pub token_in_index: u8,
    pub token_out_index: u8,
    /// Amount of token_in to deposit, in SPL base units (e.g., 1_000_000 = 1 USDC).
    /// Off-chain SDK computes via Q64.64 math then truncates to u64.
    pub amount_in: u64,
    /// SDK-computed expected output, in SPL base units (informational).
    /// The on-chain handler recomputes the exact Q64.64 amount_out via the
    /// analytical solver to avoid invariant violations from u64 truncation.
    pub expected_amount_out: u64,
    /// Minimum acceptable output in SPL base units (slippage floor).
    pub min_amount_out: u64,
}

/// Accounts for `execute_swap`.
///
/// `remaining_accounts` layout:
///   [0] = vault_in     (writable, receives token_in deposit)
///   [1] = vault_out    (writable, sends token_out to user)
///   [2] = user_ata_in  (writable, user's source for token_in)
///   [3] = user_ata_out (writable, user's destination for token_out)
///   [4..4+T) = tick accounts (writable, optional — enables trade segmentation)
///
/// When no tick accounts are provided (len == 4), the swap runs the
/// original single-segment path (backward compatible).
#[derive(Accounts)]
pub struct ExecuteSwap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [b"pool", pool.authority.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, PoolState>,

    pub token_program: Program<'info, Token>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, ExecuteSwap<'info>>,
    params: SwapParams,
) -> Result<()> {
    let pool = &ctx.accounts.pool;
    let token_in = params.token_in_index as usize;
    let token_out = params.token_out_index as usize;

    // ── Early validation (save CU on bad inputs) ──
    require!(pool.is_active, OrbitalError::PoolNotActive);
    require!(
        token_in < pool.n_assets as usize && token_out < pool.n_assets as usize,
        OrbitalError::InvalidTokenIndex
    );
    require!(token_in != token_out, OrbitalError::SameTokenSwap);
    require!(params.amount_in > 0, OrbitalError::NegativeTradeAmount);

    let remaining = &ctx.remaining_accounts;
    require!(remaining.len() >= 4, OrbitalError::InvalidRemainingAccounts);

    // Validate vault addresses match pool state
    require!(
        *remaining[0].key == pool.token_vaults[token_in],
        OrbitalError::InvalidVaultAddress
    );
    require!(
        *remaining[1].key == pool.token_vaults[token_out],
        OrbitalError::InvalidVaultAddress
    );

    // ── SPL transfer IN: user_ata_in → vault_in (user signs) ──
    let vault_in_info = &remaining[0];
    let user_ata_in_info = &remaining[2];

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: user_ata_in_info.clone(),
                to: vault_in_info.clone(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        params.amount_in,
    )?;

    // ── Convert u64 → FixedPoint for domain logic (decimal-normalized) ──
    let amount_in = FixedPoint::from_token_amount(params.amount_in, pool.token_decimals[token_in])?;
    let min_amount_out =
        FixedPoint::from_token_amount(params.min_amount_out, pool.token_decimals[token_out])?;

    // ── Fee computation (applied once to entire trade) ──
    let fee = swap::compute_fee(amount_in, pool.fee_rate_bps)?;
    let net_in = amount_in.checked_sub(fee)?;

    let tick_accounts = &remaining[4..];
    let pool = &mut ctx.accounts.pool;
    let n = pool.n_assets;

    // ══════════════════════════════════════════════════════
    // Swap execution: single-segment or trade segmentation
    // ══════════════════════════════════════════════════════

    // Track slippage info (only meaningful for single-segment path)
    let mut swap_slippage_bps: u16 = 0;
    let mut swap_execution_price = FixedPoint::zero();

    let (total_out, total_fee) = if tick_accounts.is_empty() {
        // ── No ticks: original single-segment path (backward compatible) ──
        let precise_amount_out = compute_amount_out_analytical(
            &pool.sphere,
            pool.active_reserves(),
            token_in,
            token_out,
            net_in,
        )?;

        let result = swap::execute_swap(
            pool, token_in, token_out, amount_in, precise_amount_out, min_amount_out,
        )?;

        swap_slippage_bps = result.slippage_bps;
        swap_execution_price = result.execution_price;

        (result.amount_out, result.fee)
    } else {
        // ── Tick-aware trade segmentation while loop ──
        let mut remaining_in = net_in;
        let mut total_out = FixedPoint::zero();

        // Safety: limit iterations to prevent infinite loop (max = number of ticks + 1)
        let max_iterations = tick_accounts.len() + 1;
        let mut iteration = 0;

        while remaining_in.raw > 0 && iteration < max_iterations {
            iteration += 1;

            // 1. Load tick states and find nearest boundary
            let tick_data_pairs = load_tick_data(tick_accounts, &pool.key())?;
            let tick_boundaries =
                find_nearest_tick_boundaries(&tick_data_pairs, pool.alpha_cache);

            // 2. Compute tentative full swap output
            let tentative_out = compute_amount_out_analytical(
                &pool.sphere,
                pool.active_reserves(),
                token_in,
                token_out,
                remaining_in,
            )?;

            // 3. Compute running_sum for alpha prediction
            let mut running_sum = FixedPoint::zero();
            for i in 0..n as usize {
                running_sum = running_sum.checked_add(pool.reserves[i])?;
            }

            // 4. Predict post-trade alpha
            let tentative_alpha =
                compute_new_alpha(running_sum, remaining_in, tentative_out, n)?;

            // 5. Determine if a tick crossing occurs
            let crossing_k = determine_crossing_k(
                pool.alpha_cache,
                tentative_alpha,
                &tick_boundaries,
            );

            match crossing_k {
                None => {
                    // No crossing → apply full remaining swap
                    apply_partial_swap(pool, token_in, token_out, remaining_in, tentative_out)?;
                    total_out = total_out.checked_add(tentative_out)?;
                    remaining_in = FixedPoint::zero();
                }
                Some(k_cross) => {
                    // Compute delta to reach the tick boundary
                    let delta = compute_delta_to_boundary(
                        &pool.sphere,
                        pool.active_reserves(),
                        token_in,
                        token_out,
                        k_cross,
                        n,
                    )?;

                    if delta.raw <= 0 || delta.raw >= remaining_in.raw {
                        // Can't reach boundary or delta exceeds remaining → full swap
                        apply_partial_swap(
                            pool, token_in, token_out, remaining_in, tentative_out,
                        )?;
                        total_out = total_out.checked_add(tentative_out)?;
                        remaining_in = FixedPoint::zero();
                    } else {
                        // Partial swap up to the tick boundary
                        let partial_out = compute_amount_out_analytical(
                            &pool.sphere,
                            pool.active_reserves(),
                            token_in,
                            token_out,
                            delta,
                        )?;
                        apply_partial_swap(pool, token_in, token_out, delta, partial_out)?;
                        total_out = total_out.checked_add(partial_out)?;

                        // Flip the crossed tick's status and redistribute reserves
                        flip_tick(tick_accounts, k_cross, pool)?;

                        // Recompute sphere with updated reserves
                        recompute_sphere(pool)?;
                        update_caches(pool)?;

                        // Emit TickCrossed event
                        emit_tick_crossed_event(tick_accounts, k_cross, pool.key(), pool.alpha_cache)?;

                        remaining_in = remaining_in.checked_sub(delta)?;
                    }
                }
            }
        }

        // Final invariant verification after segmented trade
        recompute_sphere(pool)?;
        verify_invariant(pool)?;
        update_caches(pool)?;

        // Slippage check on total output
        require!(
            total_out.raw >= min_amount_out.raw,
            OrbitalError::SlippageExceeded
        );

        (total_out, fee)
    };

    // ── SPL transfer OUT: vault_out → user_ata_out (pool PDA signs) ──
    let amount_out_u64 =
        total_out.to_token_amount_floor(pool.token_decimals[token_out])?;
    require!(amount_out_u64 > 0, OrbitalError::SwapOutputTooSmall);
    let authority_key = pool.authority;
    let pool_bump = pool.bump;
    let pool_seeds: &[&[u8]] = &[b"pool", authority_key.as_ref(), &[pool_bump]];

    let vault_out_info = &remaining[1];
    let user_ata_out_info = &remaining[3];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: vault_out_info.clone(),
                to: user_ata_out_info.clone(),
                authority: pool.to_account_info(),
            },
            &[pool_seeds],
        ),
        amount_out_u64,
    )?;

    // ── Correct reserve for Q64.64 → u64 floor-rounding drift ──
    let transferred_fp =
        FixedPoint::from_token_amount(amount_out_u64, pool.token_decimals[token_out])?;
    if total_out.raw > transferred_fp.raw {
        let dust = total_out.checked_sub(transferred_fp)?;
        pool.reserves[token_out] = pool.reserves[token_out].checked_add(dust)?;
        recompute_sphere(pool)?;
        update_caches(pool)?;
    }

    // ── Emit event ──
    // For single-segment path, reuse the result's execution_price & slippage.
    // For tick-aware path, compute execution_price from totals (slippage is 0 — MVP).
    let (execution_price, slippage_bps) = if tick_accounts.is_empty() {
        (swap_execution_price, swap_slippage_bps)
    } else {
        let ep = if total_out.raw > 0 {
            amount_in.checked_div(total_out)?
        } else {
            FixedPoint::zero()
        };
        (ep, 0u16)
    };

    let pool_key = pool.key();
    emit!(SwapExecuted {
        pool: pool_key,
        token_in: pool.token_mints[token_in],
        token_out: pool.token_mints[token_out],
        amount_in: amount_in.raw,
        amount_out: total_out.raw,
        price: execution_price.raw,
        slippage_bps,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "Swap: {} -> {}, in={}, out={}, fee={}, slippage={}bps",
        params.token_in_index,
        params.token_out_index,
        params.amount_in,
        amount_out_u64,
        total_fee,
        slippage_bps
    );

    Ok(())
}

// ══════════════════════════════════════════════════════════════
// Helper functions for trade segmentation
// ══════════════════════════════════════════════════════════════

/// Load tick (k, status) pairs from remaining_accounts for boundary detection.
/// Validates each tick account belongs to this pool.
fn load_tick_data(
    tick_accounts: &[AccountInfo],
    pool_key: &Pubkey,
) -> Result<Vec<(FixedPoint, TickStatus)>> {
    let mut data = Vec::with_capacity(tick_accounts.len());
    for acc in tick_accounts {
        let tick = load_tick_state(acc)?;
        require!(tick.pool == *pool_key, OrbitalError::InvalidVaultAddress);
        data.push((tick.k, tick.status));
    }
    Ok(data)
}

/// Deserialize a TickState from an AccountInfo.
fn load_tick_state(acc: &AccountInfo) -> Result<TickState> {
    let data = acc.try_borrow_data()?;
    // Skip 8-byte Anchor discriminator
    let mut slice = &data[8..];
    TickState::deserialize(&mut slice).map_err(|_| OrbitalError::InvalidVaultAddress.into())
}

/// Determine which tick k would be crossed by the alpha movement.
///
/// If alpha decreases: look for nearest_k_lower (Interior tick becoming Boundary)
/// If alpha increases: look for nearest_k_upper (Boundary tick becoming Interior)
fn determine_crossing_k(
    old_alpha: FixedPoint,
    new_alpha: FixedPoint,
    tick_data: &crate::math::torus::ConsolidatedTickData,
) -> Option<FixedPoint> {
    if new_alpha.raw < old_alpha.raw {
        // Alpha decreasing → check lower boundary
        if let Some(k_lower) = tick_data.nearest_k_lower {
            if new_alpha.raw <= k_lower.raw {
                return Some(k_lower);
            }
        }
    } else if new_alpha.raw > old_alpha.raw {
        // Alpha increasing → check upper boundary
        if let Some(k_upper) = tick_data.nearest_k_upper {
            if new_alpha.raw >= k_upper.raw {
                return Some(k_upper);
            }
        }
    }
    None
}

/// Apply a partial swap to pool reserves without full domain validation.
/// Used within the segmentation loop — final invariant check happens after.
fn apply_partial_swap(
    pool: &mut PoolState,
    token_in: usize,
    token_out: usize,
    amount_in: FixedPoint,
    amount_out: FixedPoint,
) -> Result<()> {
    pool.reserves[token_in] = pool.reserves[token_in].checked_add(amount_in)?;
    pool.reserves[token_out] = pool.reserves[token_out].checked_sub(amount_out)?;

    // Update volume tracking
    pool.total_volume = pool.total_volume.checked_add(amount_in)?;

    Ok(())
}

/// Flip a tick's status (Interior ↔ Boundary) and redistribute its reserves.
///
/// Interior → Boundary: subtract tick reserves from pool (tick is "deactivated")
/// Boundary → Interior: add tick reserves back to pool (tick is "reactivated")
fn flip_tick(
    tick_accounts: &[AccountInfo],
    k_cross: FixedPoint,
    pool: &mut PoolState,
) -> Result<()> {
    let n = pool.n_assets as usize;
    // Epsilon for k matching: 1.0 in Q64.64 integer units (generous for fixed-point)
    let epsilon_raw = 1i128 << 60; // ≈ 0.0625 — tight enough for distinct ticks

    for acc in tick_accounts {
        let mut tick = load_tick_state(acc)?;

        if (tick.k.raw - k_cross.raw).abs() < epsilon_raw {
            match tick.status {
                TickStatus::Interior => {
                    // Interior → Boundary: remove tick reserves from pool
                    tick.status = TickStatus::Boundary;
                    for i in 0..n {
                        pool.reserves[i] = pool.reserves[i].checked_sub(tick.reserves[i])?;
                    }
                    pool.total_interior_liquidity = pool
                        .total_interior_liquidity
                        .checked_sub(tick.liquidity)?;
                    pool.total_boundary_liquidity = pool
                        .total_boundary_liquidity
                        .checked_add(tick.liquidity)?;
                }
                TickStatus::Boundary => {
                    // Boundary → Interior: add tick reserves back to pool
                    tick.status = TickStatus::Interior;
                    for i in 0..n {
                        pool.reserves[i] = pool.reserves[i].checked_add(tick.reserves[i])?;
                    }
                    pool.total_boundary_liquidity = pool
                        .total_boundary_liquidity
                        .checked_sub(tick.liquidity)?;
                    pool.total_interior_liquidity = pool
                        .total_interior_liquidity
                        .checked_add(tick.liquidity)?;
                }
            }

            // Serialize updated tick back to account
            let mut data = acc.try_borrow_mut_data()?;
            let mut writer = &mut data[8..]; // skip 8-byte discriminator
            tick.serialize(&mut writer)
                .map_err(|_| OrbitalError::MathOverflow)?;

            break;
        }
    }

    Ok(())
}

/// Emit a TickCrossed event for the first matching tick at k_cross.
fn emit_tick_crossed_event(
    tick_accounts: &[AccountInfo],
    k_cross: FixedPoint,
    pool_key: Pubkey,
    alpha_at_crossing: FixedPoint,
) -> Result<()> {
    let epsilon_raw = 1i128 << 60;

    for acc in tick_accounts {
        let tick = load_tick_state(acc)?;
        if (tick.k.raw - k_cross.raw).abs() < epsilon_raw {
            let from_status = match tick.status {
                // After flip, current status is the new one
                TickStatus::Interior => TickStatus::Boundary,
                TickStatus::Boundary => TickStatus::Interior,
            };

            emit!(TickCrossed {
                pool: pool_key,
                tick: *acc.key,
                from_status,
                to_status: tick.status,
                alpha_at_crossing: alpha_at_crossing.raw,
                timestamp: Clock::get()?.unix_timestamp,
            });
            break;
        }
    }
    Ok(())
}
