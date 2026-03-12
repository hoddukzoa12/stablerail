use anchor_lang::prelude::*;

use crate::state::{PolicyState, PoolState};
use crate::errors::OrbitalError;
use crate::events::PolicyUpdated;
use crate::math::FixedPoint;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct UpdatePolicyParams {
    pub max_trade_amount: Option<u64>,
    pub max_daily_volume: Option<u64>,
    pub is_active: Option<bool>,
}

#[derive(Accounts)]
pub struct UpdatePolicy<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = authority @ OrbitalError::Unauthorized,
    )]
    pub policy: Account<'info, PolicyState>,

    /// Pool account for reading token_decimals (decimal normalization).
    #[account(
        seeds = [b"pool", pool.authority.as_ref()],
        bump = pool.bump,
        constraint = policy.pool == pool.key() @ OrbitalError::PolicyNotFound,
    )]
    pub pool: Account<'info, PoolState>,
}

pub fn handler(ctx: Context<UpdatePolicy>, params: UpdatePolicyParams) -> Result<()> {
    require!(
        params.max_trade_amount.is_some()
            || params.max_daily_volume.is_some()
            || params.is_active.is_some(),
        OrbitalError::NoFieldsToUpdate
    );

    let pool_decimals = ctx.accounts.pool.token_decimals[0];
    let policy = &mut ctx.accounts.policy;

    if let Some(max_trade) = params.max_trade_amount {
        policy.max_trade_amount = FixedPoint::from_token_amount(max_trade, pool_decimals)?;
    }
    if let Some(max_daily) = params.max_daily_volume {
        policy.max_daily_volume = FixedPoint::from_token_amount(max_daily, pool_decimals)?;
    }
    if let Some(active) = params.is_active {
        policy.is_active = active;
    }

    let clock = Clock::get()?;
    policy.updated_at = clock.unix_timestamp;

    emit!(PolicyUpdated {
        policy: policy.key(),
        authority: ctx.accounts.authority.key(),
        max_trade_amount: params.max_trade_amount.map(|_| policy.max_trade_amount.raw),
        max_daily_volume: params.max_daily_volume.map(|_| policy.max_daily_volume.raw),
        is_active: params.is_active,
        timestamp: clock.unix_timestamp,
    });

    msg!("Policy updated");
    Ok(())
}
