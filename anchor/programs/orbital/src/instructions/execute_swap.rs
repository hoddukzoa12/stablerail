use anchor_lang::prelude::*;

use crate::state::PoolState;
use crate::errors::OrbitalError;
use crate::math::FixedPoint;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SwapParams {
    pub token_in_index: u8,
    pub token_out_index: u8,
    pub amount_in: u64,
    pub min_amount_out: u64,
}

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
}

pub fn handler(ctx: Context<ExecuteSwap>, params: SwapParams) -> Result<()> {
    let pool = &mut ctx.accounts.pool;

    require!(pool.is_active, OrbitalError::PoolNotActive);
    require!(
        params.token_in_index != params.token_out_index,
        OrbitalError::SameTokenSwap
    );
    require!(
        (params.token_in_index as usize) < pool.n_assets as usize
            && (params.token_out_index as usize) < pool.n_assets as usize,
        OrbitalError::InvalidTokenIndex
    );

    let _amount_in = FixedPoint::checked_from_u64(params.amount_in)?;
    let _min_out = FixedPoint::checked_from_u64(params.min_amount_out)?;

    // TODO: Implement Torus invariant swap via domain::core::SwapCalculator

    msg!(
        "Swap: {} -> {}, amount: {}",
        params.token_in_index,
        params.token_out_index,
        params.amount_in
    );
    Ok(())
}
