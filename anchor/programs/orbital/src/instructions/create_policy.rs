use anchor_lang::prelude::*;

use crate::state::{PoolState, PolicyState};
use crate::errors::OrbitalError;
use crate::events::PolicyCreated;
use crate::math::FixedPoint;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreatePolicyParams {
    pub max_trade_amount: u64,
    pub max_daily_volume: u64,
}

#[derive(Accounts)]
pub struct CreatePolicy<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"pool", pool.authority.as_ref()],
        bump = pool.bump,
        constraint = pool.authority == authority.key() @ OrbitalError::Unauthorized,
    )]
    pub pool: Account<'info, PoolState>,

    #[account(
        init,
        payer = authority,
        space = PolicyState::SIZE,
        seeds = [b"policy", pool.key().as_ref(), authority.key().as_ref()],
        bump,
    )]
    pub policy: Account<'info, PolicyState>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CreatePolicy>, params: CreatePolicyParams) -> Result<()> {
    let policy_key = ctx.accounts.policy.key();
    let policy = &mut ctx.accounts.policy;

    policy.bump = ctx.bumps.policy;
    policy.authority = ctx.accounts.authority.key();
    policy.pool = ctx.accounts.pool.key();
    policy.max_trade_amount = FixedPoint::checked_from_u64(params.max_trade_amount)?;
    policy.max_daily_volume = FixedPoint::checked_from_u64(params.max_daily_volume)?;
    policy.current_daily_volume = FixedPoint::zero();
    policy.is_active = true;
    policy._reserved = [0u8; 64];

    let clock = Clock::get()?;
    policy.last_reset_timestamp = clock.unix_timestamp;
    policy.created_at = clock.unix_timestamp;
    policy.updated_at = clock.unix_timestamp;

    emit!(PolicyCreated {
        policy: policy_key,
        pool: ctx.accounts.pool.key(),
        authority: ctx.accounts.authority.key(),
        max_trade_amount: policy.max_trade_amount.raw,
        max_daily_volume: policy.max_daily_volume.raw,
        timestamp: clock.unix_timestamp,
    });

    msg!("Policy created for pool {}", ctx.accounts.pool.key());
    Ok(())
}
