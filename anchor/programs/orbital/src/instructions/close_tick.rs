use anchor_lang::prelude::*;

use crate::domain::core::{recompute_sphere, update_caches};
use crate::errors::OrbitalError;
use crate::state::{PoolState, TickState, TickStatus};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CloseTickParams {
    /// k_raw of the tick to close (used in PDA derivation)
    pub k_raw: i128,
}

/// Accounts for `close_tick`.
///
/// Closes a tick PDA and returns its lamports to the authority.
/// The tick must have zero liquidity (LP must withdraw first).
#[derive(Accounts)]
#[instruction(params: CloseTickParams)]
pub struct CloseTick<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"pool", pool.authority.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Box<Account<'info, PoolState>>,

    #[account(
        mut,
        seeds = [
            b"tick",
            pool.key().as_ref(),
            &params.k_raw.to_le_bytes(),
        ],
        bump = tick.bump,
        close = authority,
    )]
    pub tick: Box<Account<'info, TickState>>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CloseTick>, _params: CloseTickParams) -> Result<()> {
    let pool = &ctx.accounts.pool;
    let tick = &ctx.accounts.tick;

    // Only pool authority can close ticks
    require!(
        ctx.accounts.authority.key() == pool.authority,
        OrbitalError::Unauthorized
    );

    // Tick must belong to this pool
    require!(
        tick.pool == pool.key(),
        OrbitalError::TickPoolMismatch
    );

    // Tick must have zero liquidity — LP must withdraw first
    require!(
        tick.liquidity.is_zero(),
        OrbitalError::TickHasLiquidity
    );

    // Only boundary ticks hold reserves separate from pool.reserves.
    // Interior tick reserves are already included in pool.reserves
    // (interior swaps update pool.reserves directly, not tick.reserves),
    // so adding them back would double-count and corrupt pool state.
    let n = pool.n_assets as usize;
    let pool = &mut ctx.accounts.pool;
    let mut reserves_changed = false;
    if tick.status == TickStatus::Boundary {
        for i in 0..n {
            if !tick.reserves[i].is_zero() {
                pool.reserves[i] = pool.reserves[i].checked_add(tick.reserves[i])
                    .unwrap_or(pool.reserves[i]);
                reserves_changed = true;
            }
        }
    }

    // Refresh sphere geometry and caches so subsequent swaps use
    // correct pricing after the reserve mutation.
    if reserves_changed {
        recompute_sphere(pool)?;
        update_caches(pool)?;
    }

    pool.tick_count = pool
        .tick_count
        .checked_sub(1)
        .ok_or(OrbitalError::MathOverflow)?;

    msg!(
        "Tick closed: k={}, remaining ticks={}",
        tick.k,
        pool.tick_count,
    );

    Ok(())
}
