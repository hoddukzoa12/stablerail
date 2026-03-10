use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token};

use crate::domain::liquidity::remove_liquidity_from_pool;
use crate::errors::OrbitalError;
use crate::events::LiquidityRemoved;
use crate::math::{sphere::MAX_ASSETS, FixedPoint};
use crate::state::{PoolState, PositionState};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct RemoveLiquidityParams {
    /// Liquidity units to remove (in token base units).
    /// Use the full `position.liquidity` value for complete withdrawal.
    pub liquidity_amount: u64,
}

/// Accounts for `remove_liquidity`.
///
/// `remaining_accounts` layout (2 × n_assets):
///   [0..n)  = vault token accounts  (writable, send tokens from)
///   [n..2n) = provider ATAs         (writable, receive tokens)
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
    pub system_program: Program<'info, System>,
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
    require!(
        remaining.len() == 2 * n,
        OrbitalError::InvalidRemainingAccounts
    );

    // Validate removal amount against position balance
    let remove_amount = FixedPoint::checked_from_u64(params.liquidity_amount)?;
    let position = &ctx.accounts.position;
    require!(
        remove_amount.raw <= position.liquidity.raw,
        OrbitalError::InsufficientPositionBalance
    );
    require!(
        remove_amount.is_positive(),
        OrbitalError::InvalidLiquidityAmount
    );

    // remaining_accounts named offsets
    let vault_offset = 0; // [0..n)
    let ata_offset = n; // [n..2n)

    // Validate vault addresses match pool state
    for i in 0..n {
        require!(
            *remaining[vault_offset + i].key == pool.token_vaults[i],
            OrbitalError::InvalidVaultAddress
        );
    }

    // ── Domain logic: compute returns, update reserves, verify invariant ──
    let pool = &mut ctx.accounts.pool;
    let result = remove_liquidity_from_pool(pool, remove_amount)?;

    // ── SPL token transfers: pool vaults → provider ATAs ──
    // Pool PDA signs as vault authority
    let authority_key = pool.authority;
    let pool_bump = pool.bump;
    let pool_seeds: &[&[u8]] = &[b"pool", authority_key.as_ref(), &[pool_bump]];

    for i in 0..n {
        if result.return_amounts_u64[i] == 0 {
            continue; // skip zero transfers
        }
        let vault_info = &remaining[vault_offset + i];
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
    // Capture keys before mutable borrow
    let pool_key = pool.key();
    let provider_key = ctx.accounts.provider.key();
    let position_key = ctx.accounts.position.key();
    let n_assets = pool.n_assets;

    let position = &mut ctx.accounts.position;
    position.liquidity = position.liquidity.checked_sub(remove_amount)?;
    let clock = Clock::get()?;
    position.updated_at = clock.unix_timestamp;

    // ── Emit event ──
    let mut amounts_u64 = [0u64; MAX_ASSETS];
    for i in 0..n {
        amounts_u64[i] = result.return_amounts_u64[i];
    }

    emit!(LiquidityRemoved {
        pool: pool_key,
        provider: provider_key,
        position: position_key,
        amounts: amounts_u64,
        liquidity_removed: remove_amount.raw,
        remaining_liquidity: position.liquidity.raw,
        new_radius: result.new_radius.raw,
        n_assets,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Liquidity removed: {}, remaining: {}",
        params.liquidity_amount,
        position.liquidity
    );
    Ok(())
}
