use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token};

use crate::domain::core::{recompute_sphere, swap, update_caches};
use crate::errors::OrbitalError;
use crate::events::SwapExecuted;
use crate::math::newton::compute_amount_out_analytical;
use crate::math::FixedPoint;
use crate::state::PoolState;

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
/// `remaining_accounts` layout (4 accounts):
///   [0] = vault_in     (writable, receives token_in deposit)
///   [1] = vault_out    (writable, sends token_out to user)
///   [2] = user_ata_in  (writable, user's source for token_in)
///   [3] = user_ata_out (writable, user's destination for token_out)
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
    // (Domain layer also validates; these early checks avoid wasting CU on
    //  SPL transfers that would ultimately be reverted.)
    require!(pool.is_active, OrbitalError::PoolNotActive);
    require!(
        token_in < pool.n_assets as usize && token_out < pool.n_assets as usize,
        OrbitalError::InvalidTokenIndex
    );
    require!(token_in != token_out, OrbitalError::SameTokenSwap);
    require!(params.amount_in > 0, OrbitalError::NegativeTradeAmount);

    let remaining = &ctx.remaining_accounts;
    require!(remaining.len() == 4, OrbitalError::InvalidRemainingAccounts);

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
    let min_amount_out = FixedPoint::from_token_amount(params.min_amount_out, pool.token_decimals[token_out])?;

    // ── Compute precise amount_out on-chain ──
    // The SDK provides expected_amount_out as a u64 reference, but integer
    // truncation from Q64.64 → u64 → Q64.64 loses fractional precision that
    // violates the tight sphere invariant tolerance (r² >> 24).
    // We recompute the exact Q64.64 amount_out using the analytical solver,
    // guaranteeing invariant compliance. ~500 CU for the sqrt is negligible.
    let fee = swap::compute_fee(amount_in, pool.fee_rate_bps)?;
    let net_in = amount_in.checked_sub(fee)?;
    let precise_amount_out = compute_amount_out_analytical(
        &pool.sphere,
        pool.active_reserves(),
        token_in,
        token_out,
        net_in,
    )?;

    // ── Domain logic: validate, mutate reserves, verify invariant ──
    let pool = &mut ctx.accounts.pool;
    let result = swap::execute_swap(
        pool,
        token_in,
        token_out,
        amount_in,
        precise_amount_out,
        min_amount_out,
    )?;

    // ── SPL transfer OUT: vault_out → user_ata_out (pool PDA signs) ──
    // Floor rounding: vault always has enough tokens. User receives ≤ computed amount.
    let amount_out_u64 = result.amount_out.to_token_amount_floor(pool.token_decimals[token_out])?;
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
    // Floor always rounds down, so transferred_fp ≤ amount_out.
    // Add the dust back to reserves so they match the actual vault balance.
    let transferred_fp = FixedPoint::from_token_amount(amount_out_u64, pool.token_decimals[token_out])?;
    if result.amount_out.raw > transferred_fp.raw {
        let dust = result.amount_out.checked_sub(transferred_fp)?;
        pool.reserves[token_out] = pool.reserves[token_out].checked_add(dust)?;
        recompute_sphere(pool)?;
        update_caches(pool)?;
    }

    // ── Emit event ──
    let pool_key = pool.key();
    emit!(SwapExecuted {
        pool: pool_key,
        token_in: pool.token_mints[token_in],
        token_out: pool.token_mints[token_out],
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
        amount_out_u64,
        result.fee,
        result.slippage_bps
    );

    Ok(())
}
