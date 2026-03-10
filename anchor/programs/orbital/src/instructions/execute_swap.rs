use anchor_lang::prelude::*;

use crate::domain::core::swap;
use crate::events::SwapExecuted;
use crate::math::FixedPoint;
use crate::state::PoolState;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SwapParams {
    pub token_in_index: u8,
    pub token_out_index: u8,
    /// Q64.64 raw value — off-chain SDK passes full fixed-point precision.
    /// SPL token u64 conversion is handled separately at the transfer layer.
    pub amount_in: i128,
    /// Computed off-chain via torus invariant + Newton solver (Q64.64 raw)
    pub expected_amount_out: i128,
    /// Minimum acceptable output (Q64.64 raw)
    pub min_amount_out: i128,
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

    // Wrap Q64.64 raw values — no precision loss from u64 quantization
    let amount_in = FixedPoint::from_raw(params.amount_in);
    let expected_amount_out = FixedPoint::from_raw(params.expected_amount_out);
    let min_amount_out = FixedPoint::from_raw(params.min_amount_out);

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
        result.amount_in.raw,
        result.amount_out.raw,
        result.fee.raw,
        result.slippage_bps
    );

    Ok(())
}
