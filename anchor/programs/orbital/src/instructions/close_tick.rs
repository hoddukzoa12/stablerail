use anchor_lang::prelude::*;

use crate::errors::OrbitalError;
use crate::state::{PoolState, TickState};

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

    // Boundary tick reserves after full LP withdrawal are Q64.64 dust
    // (sub-token fractional bits from floor rounding in remove_liquidity).
    // These dust amounts do NOT exist in the vault — adding them to
    // pool.reserves would create pool.reserves > vault balance, causing
    // the last LP's withdraw to fail on SPL transfer.
    //
    // Interior tick reserves are already included in pool.reserves
    // (interior swaps update pool.reserves directly, not tick.reserves),
    // so they must NOT be added back either (would double-count).
    //
    // In both cases: discard tick.reserves silently. The dust is at most
    // n × 1 sub-token (≈ $0.000003 for 3-asset USDC pool).
    let pool = &mut ctx.accounts.pool;

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
