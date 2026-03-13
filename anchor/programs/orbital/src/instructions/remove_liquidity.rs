use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token};

use crate::domain::liquidity::remove_liquidity_from_pool;
use crate::errors::OrbitalError;
use crate::events::LiquidityRemoved;
use crate::math::FixedPoint;
use crate::state::{PoolState, PositionState, TickState};

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

    // ── Domain logic: compute returns, update reserves, verify invariant ──
    let pool = &mut ctx.accounts.pool;
    let result = remove_liquidity_from_pool(pool, remove_amount)?;

    // ── Tick-specific logic: subtract from per-tick reserves ──
    if has_tick {
        let tick_acc = &remaining[2 * n];
        let mut tick = load_tick_state(tick_acc)?;

        // Validate tick matches position and pool
        require!(
            *tick_acc.key == ctx.accounts.position.tick,
            OrbitalError::InvalidVaultAddress
        );
        require!(tick.pool == pool.key(), OrbitalError::InvalidVaultAddress);

        // Subtract proportional returns from tick reserves
        for i in 0..n {
            tick.reserves[i] = tick.reserves[i].checked_sub(result.return_amounts[i])?;
        }
        tick.liquidity = tick.liquidity.checked_sub(remove_amount)?;

        // Serialize tick back to account
        save_tick_state(tick_acc, &tick)?;
    }

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
    let data = acc.try_borrow_data()?;
    let mut slice = &data[8..];
    TickState::deserialize(&mut slice).map_err(|_| OrbitalError::InvalidVaultAddress.into())
}

fn save_tick_state(acc: &AccountInfo, tick: &TickState) -> Result<()> {
    let mut data = acc.try_borrow_mut_data()?;
    let mut writer = &mut data[8..];
    tick.serialize(&mut writer)
        .map_err(|_| OrbitalError::MathOverflow)?;
    Ok(())
}
