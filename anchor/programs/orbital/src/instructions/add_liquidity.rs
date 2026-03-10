use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token};

use crate::domain::liquidity::add_liquidity_to_pool;
use crate::errors::OrbitalError;
use crate::events::LiquidityAdded;
use crate::math::{sphere::MAX_ASSETS, FixedPoint};
use crate::state::{PoolState, PositionState};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddLiquidityParams {
    /// Per-token deposit amounts in base units (e.g., 1_000_000 for 1 USDC).
    /// Only first `pool.n_assets` entries are used.
    pub amounts: [u64; MAX_ASSETS],
}

/// Accounts for `add_liquidity`.
///
/// `remaining_accounts` layout (2 × n_assets):
///   [0..n)  = vault token accounts  (writable, receive deposits)
///   [n..2n) = provider ATAs         (writable, deposit source)
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
    require!(
        remaining.len() == 2 * n,
        OrbitalError::InvalidRemainingAccounts
    );

    // Validate all deposit amounts are positive for active assets
    for i in 0..n {
        require!(
            params.amounts[i] > 0,
            OrbitalError::InvalidLiquidityAmount
        );
    }

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

    // ── SPL token transfers: provider ATAs → pool vaults ──
    for i in 0..n {
        let ata_info = &remaining[ata_offset + i];
        let vault_info = &remaining[vault_offset + i];

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

    // ── Convert amounts to FixedPoint ──
    let mut deposits_fp = [FixedPoint::zero(); MAX_ASSETS];
    for i in 0..n {
        deposits_fp[i] = FixedPoint::checked_from_u64(params.amounts[i])?;
    }

    // ── Domain logic: update reserves, recompute sphere, verify invariant ──
    let pool = &mut ctx.accounts.pool;
    let result = add_liquidity_to_pool(pool, &deposits_fp[..n])?;

    // ── Set position fields ──
    let position = &mut ctx.accounts.position;
    position.bump = ctx.bumps.position;
    position.pool = pool.key();
    position.tick = Pubkey::default(); // no tick for full-range MVP
    position.owner = ctx.accounts.provider.key();
    position.liquidity = result.liquidity;
    position.tick_lower = FixedPoint::zero(); // full range
    position.tick_upper = FixedPoint::from_raw(i128::MAX); // full range
    position.fees_earned = FixedPoint::zero();
    position._reserved = [0u8; 64];

    let clock = Clock::get()?;
    position.created_at = clock.unix_timestamp;
    position.updated_at = clock.unix_timestamp;

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
        new_radius: result.new_radius.raw,
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
