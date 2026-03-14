use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token};

use crate::domain::liquidity::remove_liquidity_from_pool;
use crate::errors::OrbitalError;
use crate::events::LiquidityRemoved;
use crate::math::FixedPoint;
use crate::math::sphere::MAX_ASSETS;
use crate::state::{PoolState, PositionState, TickState, TickStatus};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RemoveLiquidityParams {
    /// Raw Q64.64 liquidity to remove.
    /// For full withdrawal, pass `position.liquidity.raw` exactly.
    /// Partial withdrawal: compute the desired fraction of `position.liquidity.raw`.
    pub liquidity_raw: i128,
}

/// Accounts for `remove_liquidity`.
///
/// `remaining_accounts` layout:
///   [0..n)  = vault token accounts  (writable, send tokens from)
///   [n..2n) = provider ATAs         (writable, receive tokens)
///   [2n]    = optional tick account (writable, required if position has tick)
///
/// NOTE: No `pool.is_active` guard — LPs must always be able to withdraw
/// (DeFi emergency exit pattern: Curve/Aave/Compound convention).
#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(mut)]
    pub provider: Signer<'info>,

    #[account(
        mut,
        seeds = [b"pool", pool.authority.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, PoolState>,

    #[account(
        mut,
        constraint = position.owner == provider.key() @ OrbitalError::Unauthorized,
        constraint = position.pool == pool.key() @ OrbitalError::PositionNotFound,
    )]
    pub position: Account<'info, PositionState>,

    pub token_program: Program<'info, Token>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, RemoveLiquidity<'info>>,
    params: RemoveLiquidityParams,
) -> Result<()> {
    let pool = &ctx.accounts.pool;
    let n = pool.n_assets as usize;

    // NOTE: Intentionally no pool.is_active guard.
    // Emergency exit pattern — LPs must always be able to withdraw.

    let remaining = &ctx.remaining_accounts;
    let position = &ctx.accounts.position;
    let has_tick = position.tick != Pubkey::default();

    // Validate remaining_accounts count: 2*n (full-range) or 2*n+1 (tick)
    if has_tick {
        require!(
            remaining.len() == 2 * n + 1,
            OrbitalError::InvalidRemainingAccounts
        );
    } else {
        require!(
            remaining.len() == 2 * n,
            OrbitalError::InvalidRemainingAccounts
        );
    }

    // Validate removal amount against position balance
    let remove_amount = FixedPoint::from_raw(params.liquidity_raw);
    require!(
        remove_amount.is_positive(),
        OrbitalError::InvalidLiquidityAmount
    );
    require!(
        remove_amount.raw <= position.liquidity.raw,
        OrbitalError::InsufficientPositionBalance
    );

    // remaining_accounts layout: [0..n) vaults, [n..2n) provider ATAs, [2n]? tick
    let ata_offset = n;

    // Validate vault addresses match pool state
    for i in 0..n {
        require!(
            *remaining[i].key == pool.token_vaults[i],
            OrbitalError::InvalidVaultAddress
        );
    }

    // ── Tick-specific logic: handle boundary vs interior withdrawal ──
    let pool = &mut ctx.accounts.pool;

    if has_tick {
        let tick_acc = &remaining[2 * n];
        let mut tick = load_tick_state(tick_acc)?;

        // Validate tick matches position and pool
        require!(
            *tick_acc.key == ctx.accounts.position.tick,
            OrbitalError::InvalidVaultAddress
        );
        require!(tick.pool == pool.key(), OrbitalError::InvalidVaultAddress);

        if tick.status == TickStatus::Boundary {
            // Boundary tick: liquidity is NOT part of pool.total_interior_liquidity.
            // flip_tick already subtracted this tick's reserves from pool.reserves
            // and moved liquidity to total_boundary_liquidity.
            // We must subtract from total_boundary_liquidity instead.
            let liq_sub = if remove_amount.raw > tick.liquidity.raw {
                tick.liquidity
            } else {
                remove_amount
            };
            pool.total_boundary_liquidity = pool
                .total_boundary_liquidity
                .checked_sub(liq_sub)?;

            // For boundary ticks, return amounts are computed from tick's own
            // reserves (which were snapshotted at crossing time), not pool reserves.
            let tick_fraction = if tick.liquidity.is_positive() {
                remove_amount.checked_div(tick.liquidity)?
            } else {
                FixedPoint::zero()
            };

            tick.liquidity = tick.liquidity.checked_sub(liq_sub)?;

            // Compute return amounts from tick reserves.
            // Reserve accounting uses the floor-rounded u64 amount (what actually
            // leaves the vault), not the higher-precision Q64.64 value. This
            // prevents dust accumulation where tick.reserves is decremented by
            // more than the vault actually transferred.
            let mut return_amounts_u64 = [0u64; MAX_ASSETS];
            for i in 0..n {
                let return_fp = tick.reserves[i].checked_mul(tick_fraction)?;
                return_amounts_u64[i] = return_fp
                    .to_token_amount_floor(pool.token_decimals[i])?;
                // Subtract the denormalized transferred amount from tick reserves
                let transferred_fp =
                    FixedPoint::from_token_amount(return_amounts_u64[i], pool.token_decimals[i])?;
                let sub = if transferred_fp.raw > tick.reserves[i].raw {
                    tick.reserves[i]
                } else {
                    transferred_fp
                };
                tick.reserves[i] = tick.reserves[i].checked_sub(sub)?;
            }

            // Reject if all returns round to zero
            let has_nonzero = return_amounts_u64[..n].iter().any(|&a| a > 0);
            require!(has_nonzero, OrbitalError::WithdrawalTooSmall);

            save_tick_state(tick_acc, &tick)?;

            // Transfer tokens from vaults to provider
            let authority_key = pool.authority;
            let pool_bump = pool.bump;
            let pool_seeds: &[&[u8]] = &[b"pool", authority_key.as_ref(), &[pool_bump]];

            for i in 0..n {
                if return_amounts_u64[i] == 0 {
                    continue;
                }
                let vault_info = &remaining[i];
                let ata_info = &remaining[ata_offset + i];

                token::transfer(
                    CpiContext::new_with_signer(
                        ctx.accounts.token_program.to_account_info(),
                        token::Transfer {
                            from: vault_info.clone(),
                            to: ata_info.clone(),
                            authority: pool.to_account_info(),
                        },
                        &[pool_seeds],
                    ),
                    return_amounts_u64[i],
                )?;
            }

            // Update position
            let pool_key = pool.key();
            let provider_key = ctx.accounts.provider.key();
            let position_key = ctx.accounts.position.key();
            let n_assets = pool.n_assets;

            let position = &mut ctx.accounts.position;
            position.liquidity = position.liquidity.checked_sub(remove_amount)?;
            let clock = Clock::get()?;
            position.updated_at = clock.unix_timestamp;

            emit!(LiquidityRemoved {
                pool: pool_key,
                provider: provider_key,
                position: position_key,
                amounts: return_amounts_u64,
                liquidity_removed: remove_amount.raw,
                remaining_liquidity: position.liquidity.raw,
                new_radius: pool.sphere.radius.raw,
                n_assets,
                timestamp: clock.unix_timestamp,
            });

            msg!(
                "Boundary liquidity removed: {}, remaining: {}",
                remove_amount,
                position.liquidity
            );
            return Ok(());
        }

        // Interior tick: use standard pool-level withdrawal, then adjust tick reserves
        let result = remove_liquidity_from_pool(pool, remove_amount)?;

        // Subtract proportional share from tick reserves.
        // Per-tick reserves may be stale (interior swaps don't update them),
        // so use min(return_amount, tick_reserve) to prevent underflow.
        // This is safe because tick.reserves is accounting-only — actual token
        // transfers use pool-level return_amounts computed from pool reserves.
        for i in 0..n {
            let sub = if result.return_amounts[i].raw > tick.reserves[i].raw {
                tick.reserves[i]
            } else {
                result.return_amounts[i]
            };
            tick.reserves[i] = tick.reserves[i].checked_sub(sub)?;
        }
        let liq_sub = if remove_amount.raw > tick.liquidity.raw {
            tick.liquidity
        } else {
            remove_amount
        };
        tick.liquidity = tick.liquidity.checked_sub(liq_sub)?;

        save_tick_state(tick_acc, &tick)?;

        // Continue to standard transfer path below with `result`
        let authority_key = pool.authority;
        let pool_bump = pool.bump;
        let pool_seeds: &[&[u8]] = &[b"pool", authority_key.as_ref(), &[pool_bump]];

        for i in 0..n {
            if result.return_amounts_u64[i] == 0 {
                continue;
            }
            let vault_info = &remaining[i];
            let ata_info = &remaining[ata_offset + i];

            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    token::Transfer {
                        from: vault_info.clone(),
                        to: ata_info.clone(),
                        authority: pool.to_account_info(),
                    },
                    &[pool_seeds],
                ),
                result.return_amounts_u64[i],
            )?;
        }

        let pool_key = pool.key();
        let provider_key = ctx.accounts.provider.key();
        let position_key = ctx.accounts.position.key();
        let n_assets = pool.n_assets;

        let position = &mut ctx.accounts.position;
        position.liquidity = position.liquidity.checked_sub(remove_amount)?;
        let clock = Clock::get()?;
        position.updated_at = clock.unix_timestamp;

        emit!(LiquidityRemoved {
            pool: pool_key,
            provider: provider_key,
            position: position_key,
            amounts: result.return_amounts_u64,
            liquidity_removed: remove_amount.raw,
            remaining_liquidity: position.liquidity.raw,
            new_radius: result.new_radius.raw,
            n_assets,
            timestamp: clock.unix_timestamp,
        });

        msg!(
            "Liquidity removed: {}, remaining: {}",
            remove_amount,
            position.liquidity
        );
        return Ok(());
    }

    // ── Full-range (no tick): standard pool-level withdrawal ──
    let result = remove_liquidity_from_pool(pool, remove_amount)?;

    // ── SPL token transfers: pool vaults → provider ATAs ──
    let authority_key = pool.authority;
    let pool_bump = pool.bump;
    let pool_seeds: &[&[u8]] = &[b"pool", authority_key.as_ref(), &[pool_bump]];

    for i in 0..n {
        if result.return_amounts_u64[i] == 0 {
            continue; // skip zero transfers
        }
        let vault_info = &remaining[i];
        let ata_info = &remaining[ata_offset + i];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: vault_info.clone(),
                    to: ata_info.clone(),
                    authority: pool.to_account_info(),
                },
                &[pool_seeds],
            ),
            result.return_amounts_u64[i],
        )?;
    }

    // ── Update position ──
    let pool_key = pool.key();
    let provider_key = ctx.accounts.provider.key();
    let position_key = ctx.accounts.position.key();
    let n_assets = pool.n_assets;

    let position = &mut ctx.accounts.position;
    position.liquidity = position.liquidity.checked_sub(remove_amount)?;
    let clock = Clock::get()?;
    position.updated_at = clock.unix_timestamp;

    // ── Emit event ──
    emit!(LiquidityRemoved {
        pool: pool_key,
        provider: provider_key,
        position: position_key,
        amounts: result.return_amounts_u64,
        liquidity_removed: remove_amount.raw,
        remaining_liquidity: position.liquidity.raw,
        new_radius: result.new_radius.raw,
        n_assets,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Liquidity removed: {}, remaining: {}",
        remove_amount,
        position.liquidity
    );
    Ok(())
}

// ── Tick account helpers ──

fn load_tick_state(acc: &AccountInfo) -> Result<TickState> {
    // Validate account is owned by this program (prevents forged tick accounts)
    require!(acc.owner == &crate::ID, OrbitalError::InvalidTickAccount);

    let data = acc.try_borrow_data()?;
    let mut slice: &[u8] = &data;
    TickState::try_deserialize(&mut slice)
        .map_err(|_| OrbitalError::InvalidTickAccount.into())
}

fn save_tick_state(acc: &AccountInfo, tick: &TickState) -> Result<()> {
    let mut data = acc.try_borrow_mut_data()?;
    let mut writer = &mut data[8..];
    tick.serialize(&mut writer)
        .map_err(|_| OrbitalError::TickSerializationFailed)?;
    Ok(())
}
