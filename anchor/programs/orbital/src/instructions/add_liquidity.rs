use anchor_lang::prelude::*;

use crate::state::{PoolState, PositionState};
use crate::errors::OrbitalError;
use crate::math::{FixedPoint, sphere::MAX_ASSETS};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddLiquidityParams {
    pub amounts: [u64; MAX_ASSETS],
    pub tick_lower: i64,
    pub tick_upper: i64,
}

#[derive(Accounts)]
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
}

pub fn handler(ctx: Context<AddLiquidity>, params: AddLiquidityParams) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    let position = &mut ctx.accounts.position;

    require!(pool.is_active, OrbitalError::PoolNotActive);
    require!(
        params.tick_lower < params.tick_upper,
        OrbitalError::InvalidTickBound
    );
    require!(params.amounts[0] > 0, OrbitalError::InvalidLiquidityAmount);

    position.bump = ctx.bumps.position;
    position.pool = pool.key();
    position.tick = Pubkey::default();
    position.owner = ctx.accounts.provider.key();
    position.tick_lower = FixedPoint::from_int(params.tick_lower);
    position.tick_upper = FixedPoint::from_int(params.tick_upper);
    position.fees_earned = FixedPoint::zero();
    // STUB: Uses amounts[0] only as placeholder liquidity value.
    // Full implementation (Issue #11) will accept per-token asymmetric deposits,
    // validate via sphere invariant checkInvariants(k, r, amounts),
    // and compute actual liquidity from the deposit geometry.
    position.liquidity = FixedPoint::checked_from_u64(params.amounts[0])?;
    position._reserved = [0u8; 64];

    let clock = Clock::get()?;
    position.created_at = clock.unix_timestamp;
    position.updated_at = clock.unix_timestamp;

    // Increment pool position counter for next PDA derivation
    pool.position_count = pool.position_count.checked_add(1)
        .ok_or(OrbitalError::MathOverflow)?;

    // TODO: Full liquidity addition logic via domain::liquidity (Issue #11)

    msg!("Liquidity added by {}", ctx.accounts.provider.key());
    Ok(())
}
