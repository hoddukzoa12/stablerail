use anchor_lang::prelude::*;

use crate::state::{PoolState, PositionState};
use crate::errors::OrbitalError;
use crate::math::FixedPoint;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RemoveLiquidityParams {
    pub liquidity_amount: u64,
}

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
}

pub fn handler(ctx: Context<RemoveLiquidity>, params: RemoveLiquidityParams) -> Result<()> {
    let _pool = &mut ctx.accounts.pool;
    let position = &mut ctx.accounts.position;

    let remove_amount = FixedPoint::from_u64(params.liquidity_amount);
    require!(
        remove_amount.raw <= position.liquidity.raw,
        OrbitalError::InsufficientPositionBalance
    );

    position.liquidity = position.liquidity.checked_sub(remove_amount)?;
    let clock = Clock::get()?;
    position.updated_at = clock.unix_timestamp;

    // TODO: Full removal logic via domain::liquidity

    msg!("Liquidity removed: {}", params.liquidity_amount);
    Ok(())
}
