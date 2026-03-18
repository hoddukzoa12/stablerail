use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token};

use crate::domain::core::{recompute_sphere, update_caches};
use crate::domain::liquidity::add_liquidity_to_pool;
use crate::errors::OrbitalError;
use crate::events::LiquidityAdded;
use crate::math::{sphere::MAX_ASSETS, FixedPoint};
use crate::instructions::tick_helpers::{load_tick_state_mut, save_tick_state};
use crate::state::{PoolState, PositionState, TickStatus};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddLiquidityParams {
    /// Per-token deposit amounts in base units (e.g., 1_000_000 for 1 USDC).
    /// Only first `pool.n_assets` entries are used.
    pub amounts: [u64; MAX_ASSETS],
}

/// Accounts for `add_liquidity`.
///
/// `remaining_accounts` layout:
///   [0..n)  = vault token accounts  (writable, receive deposits)
///   [n..2n) = provider ATAs         (writable, deposit source)
///   [2n]    = optional tick account (writable, for concentrated liquidity)
///
/// When no tick account is provided (len == 2*n), position is full-range.
/// When tick account is provided (len == 2*n + 1), liquidity is concentrated
/// within the tick's spherical cap bounds.
#[derive(Accounts)]
#[instruction(params: AddLiquidityParams)]
pub struct AddLiquidity<'info> {
    #[account(mut)]
    pub provider: Signer<'info>,

    #[account(
        mut,
        seeds = [b"pool", pool.authority.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, PoolState>,

    #[account(
        init,
        payer = provider,
        space = PositionState::SIZE,
        seeds = [
            b"position",
            pool.key().as_ref(),
            provider.key().as_ref(),
            &pool.position_count.to_le_bytes(),
        ],
        bump,
    )]
    pub position: Account<'info, PositionState>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, AddLiquidity<'info>>,
    params: AddLiquidityParams,
) -> Result<()> {
    let pool = &ctx.accounts.pool;
    let n = pool.n_assets as usize;

    // ── Input validation ──
    require!(pool.is_active, OrbitalError::PoolNotActive);

    let remaining = &ctx.remaining_accounts;
    let has_tick = remaining.len() == 2 * n + 1;
    require!(
        remaining.len() == 2 * n || has_tick,
        OrbitalError::InvalidRemainingAccounts
    );

    // Validate all deposit amounts are positive for active assets.
    for i in 0..n {
        require!(
            params.amounts[i] > 0,
            OrbitalError::InvalidLiquidityAmount
        );
    }

    // remaining_accounts layout: [0..n) vaults, [n..2n) provider ATAs, [2n]? tick
    let ata_offset = n;

    // Validate vault addresses match pool state
    for i in 0..n {
        require!(
            *remaining[i].key == pool.token_vaults[i],
            OrbitalError::InvalidVaultAddress
        );
    }

    // ── SPL token transfers: provider ATAs → pool vaults ──
    for i in 0..n {
        let ata_info = &remaining[ata_offset + i];
        let vault_info = &remaining[i];

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ata_info.clone(),
                    to: vault_info.clone(),
                    authority: ctx.accounts.provider.to_account_info(),
                },
            ),
            params.amounts[i],
        )?;
    }

    // ── Convert amounts to FixedPoint (decimal-normalized) ──
    let mut deposits_fp = [FixedPoint::zero(); MAX_ASSETS];
    for i in 0..n {
        deposits_fp[i] = FixedPoint::from_token_amount(params.amounts[i], pool.token_decimals[i])?;
    }

    // ── Domain logic: update reserves, recompute sphere, verify invariant ──
    let pool = &mut ctx.accounts.pool;
    let result = add_liquidity_to_pool(pool, &deposits_fp[..n])?;

    // ── Set position fields ──
    let position = &mut ctx.accounts.position;
    position.bump = ctx.bumps.position;
    position.pool = pool.key();
    position.owner = ctx.accounts.provider.key();
    position.liquidity = result.liquidity;
    position.fees_earned = FixedPoint::zero();
    position._reserved = [0u8; 64];

    let clock = Clock::get()?;
    position.created_at = clock.unix_timestamp;
    position.updated_at = clock.unix_timestamp;

    // ── Tick-specific logic (concentrated liquidity) ──
    if has_tick {
        let tick_acc = &remaining[2 * n];
        let mut tick = load_tick_state_mut(tick_acc)?;

        require!(tick.pool == pool.key(), OrbitalError::TickPoolMismatch);

        // Add deposits to tick's per-tick reserves
        for i in 0..n {
            tick.reserves[i] = tick.reserves[i].checked_add(deposits_fp[i])?;
        }
        tick.liquidity = tick.liquidity.checked_add(result.liquidity)?;

        // Handle accounting based on tick status:
        //
        // Interior ticks: deposits are already correctly counted in
        //   pool.reserves and total_interior_liquidity by add_liquidity_to_pool.
        //
        // Boundary ticks: deposits must NOT inflate pool.reserves (boundary
        //   reserves are frozen until the tick transitions to Interior via a
        //   swap-driven tick crossing). Undo the pool.reserves addition and
        //   move liquidity from interior to boundary accounting.
        //
        //   This is essential for balanced pools where alpha == k_min:
        //   every valid tick (k > k_min) starts as Boundary, and without
        //   this path the create-tick → add-liquidity flow would be blocked.
        if tick.status == TickStatus::Boundary {
            for i in 0..n {
                pool.reserves[i] = pool.reserves[i].checked_sub(deposits_fp[i])?;
            }
            pool.total_interior_liquidity = pool
                .total_interior_liquidity
                .checked_sub(result.liquidity)?;
            pool.total_boundary_liquidity = pool
                .total_boundary_liquidity
                .checked_add(result.liquidity)?;
            // Recompute sphere and caches since pool.reserves changed
            recompute_sphere(pool)?;
            update_caches(pool)?;
        }

        // Set position tick reference and bounds
        position.tick = *tick_acc.key;
        position.tick_lower = tick.x_min;
        position.tick_upper = tick.x_max;

        // Serialize tick back to account
        save_tick_state(tick_acc, &tick)?;
    } else {
        // Full-range position (no tick)
        position.tick = Pubkey::default();
        position.tick_lower = FixedPoint::zero();
        position.tick_upper = FixedPoint::from_raw(i128::MAX);
    }

    // Increment position counter for next PDA derivation
    pool.position_count = pool
        .position_count
        .checked_add(1)
        .ok_or(OrbitalError::MathOverflow)?;

    // ── Emit event ──
    emit!(LiquidityAdded {
        pool: pool.key(),
        provider: ctx.accounts.provider.key(),
        position: ctx.accounts.position.key(),
        amounts: params.amounts,
        liquidity: result.liquidity.raw,
        // Use pool.sphere.radius (not result.new_radius) because the Boundary
        // tick path recomputes the sphere after undoing provisional reserves.
        new_radius: pool.sphere.radius.raw,
        n_assets: pool.n_assets,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Liquidity added: {} assets, liquidity={}",
        n,
        result.liquidity
    );
    Ok(())
}

// Tick helpers (load_tick_state_mut, save_tick_state) imported from tick_helpers module.
