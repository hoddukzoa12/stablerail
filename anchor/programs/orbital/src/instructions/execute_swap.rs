use anchor_lang::prelude::*;

use crate::domain::core::swap;
use crate::events::SwapExecuted;
use crate::math::FixedPoint;
use crate::state::PoolState;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SwapParams {
    pub token_in_index: u8,
    pub token_out_index: u8,
    pub amount_in: u64,
    /// Computed off-chain via torus invariant + Newton solver
    pub expected_amount_out: u64,
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
    // TODO (Issue #12): Add user token accounts, vault accounts for SPL transfers
}

pub fn handler(ctx: Context<ExecuteSwap>, params: SwapParams) -> Result<()> {
    let pool_key = ctx.accounts.pool.key();
    let pool = &mut ctx.accounts.pool;

    // Convert u64 params to FixedPoint
    let amount_in = FixedPoint::checked_from_u64(params.amount_in)?;
    let expected_amount_out = FixedPoint::checked_from_u64(params.expected_amount_out)?;
    let min_amount_out = FixedPoint::checked_from_u64(params.min_amount_out)?;

    // Execute swap via domain logic
    let result = swap::execute_swap(
        pool,
        params.token_in_index as usize,
        params.token_out_index as usize,
        amount_in,
        expected_amount_out,
        min_amount_out,
    )?;

    // TODO (Issue #12): SPL token transfers
    // transfer amount_in from user token account → vault_in
    // transfer amount_out from vault_out → user token account

    // Emit domain event
    emit!(SwapExecuted {
        pool: pool_key,
        token_in: pool.token_mints[params.token_in_index as usize],
        token_out: pool.token_mints[params.token_out_index as usize],
        amount_in: result.amount_in.raw,
        amount_out: result.amount_out.raw,
        price: result.execution_price.raw,
        slippage_bps: result.slippage_bps,
        timestamp: Clock::get()?.unix_timestamp,
    });

    msg!(
        "Swap: {} -> {}, in={}, out={}, fee={}, slippage={}bps",
        params.token_in_index,
        params.token_out_index,
        params.amount_in,
        params.expected_amount_out,
        result.fee.raw,
        result.slippage_bps
    );

    Ok(())
}
