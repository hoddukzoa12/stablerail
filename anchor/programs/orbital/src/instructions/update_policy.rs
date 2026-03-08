use anchor_lang::prelude::*;

use crate::state::PolicyState;
use crate::errors::OrbitalError;
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
}

pub fn handler(ctx: Context<UpdatePolicy>, params: UpdatePolicyParams) -> Result<()> {
    let policy = &mut ctx.accounts.policy;

    if let Some(max_trade) = params.max_trade_amount {
        policy.max_trade_amount = FixedPoint::from_u64(max_trade);
    }
    if let Some(max_daily) = params.max_daily_volume {
        policy.max_daily_volume = FixedPoint::from_u64(max_daily);
    }
    if let Some(active) = params.is_active {
        policy.is_active = active;
    }

    let clock = Clock::get()?;
    policy.updated_at = clock.unix_timestamp;

    msg!("Policy updated");
    Ok(())
}
