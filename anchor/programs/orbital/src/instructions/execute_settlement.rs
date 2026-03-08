use anchor_lang::prelude::*;

use crate::state::{
    AllowlistState, AuditEntryState, PolicyState, PoolState, SettlementState, SettlementStatus,
};
use crate::errors::OrbitalError;
use crate::math::FixedPoint;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ExecuteSettlementParams {
    pub token_in_index: u8,
    pub token_out_index: u8,
    pub amount: u64,
    pub min_amount_out: u64,
    pub nonce: u64,
}

#[derive(Accounts)]
#[instruction(params: ExecuteSettlementParams)]
pub struct ExecuteSettlement<'info> {
    #[account(mut)]
    pub executor: Signer<'info>,

    #[account(
        mut,
        seeds = [b"pool", pool.authority.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Box<Account<'info, PoolState>>,

    #[account(
        constraint = policy.pool == pool.key() @ OrbitalError::PolicyNotFound,
        constraint = policy.is_active @ OrbitalError::SettlementPolicyViolation,
    )]
    pub policy: Box<Account<'info, PolicyState>>,

    #[account(
        seeds = [b"allowlist", policy.key().as_ref()],
        bump = allowlist.bump,
    )]
    pub allowlist: Box<Account<'info, AllowlistState>>,

    #[account(
        init,
        payer = executor,
        space = SettlementState::SIZE,
        seeds = [
            b"settlement",
            pool.key().as_ref(),
            executor.key().as_ref(),
            &params.nonce.to_le_bytes(),
        ],
        bump,
    )]
    pub settlement: Box<Account<'info, SettlementState>>,

    #[account(
        init,
        payer = executor,
        space = AuditEntryState::SIZE,
        seeds = [b"audit", settlement.key().as_ref()],
        bump,
    )]
    pub audit_entry: Box<Account<'info, AuditEntryState>>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ExecuteSettlement>, params: ExecuteSettlementParams) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    let policy = &ctx.accounts.policy;
    let allowlist = &ctx.accounts.allowlist;
    let settlement = &mut ctx.accounts.settlement;
    let audit_entry = &mut ctx.accounts.audit_entry;
    let executor = &ctx.accounts.executor;

    // Policy checks
    require!(
        allowlist.contains(&executor.key()),
        OrbitalError::Unauthorized
    );

    let amount = FixedPoint::checked_from_u64(params.amount)?;
    require!(
        amount.raw <= policy.max_trade_amount.raw,
        OrbitalError::PolicyLimitExceeded
    );

    // TODO: Execute swap via domain::core, check daily volume
    // Once swap logic is implemented, amount_out will be computed from the swap
    // and the slippage check below should be re-enabled:
    //   require!(amount_out.raw >= min_out.raw, OrbitalError::SlippageExceeded);
    // For now, settlement is recorded as Pending until swap execution is wired.

    let amount_out = FixedPoint::zero();
    let _min_out = FixedPoint::checked_from_u64(params.min_amount_out)?;
    let clock = Clock::get()?;

    // Record settlement
    settlement.bump = ctx.bumps.settlement;
    settlement.pool = pool.key();
    settlement.policy = policy.key();
    settlement.executor = executor.key();
    settlement.token_in_index = params.token_in_index;
    settlement.token_out_index = params.token_out_index;
    settlement.amount_in = amount;
    settlement.amount_out = amount_out;
    settlement.execution_price = FixedPoint::zero();
    settlement.status = SettlementStatus::Pending;
    settlement.executed_at = clock.unix_timestamp;
    settlement.nonce = params.nonce;
    settlement._reserved = [0u8; 64];

    // Create audit entry
    audit_entry.bump = ctx.bumps.audit_entry;
    audit_entry.settlement = settlement.key();
    audit_entry.executor = executor.key();
    audit_entry.pool = pool.key();
    audit_entry.policy = policy.key();
    audit_entry.action_hash = [0u8; 32];
    audit_entry.amount = amount;
    audit_entry.timestamp = clock.unix_timestamp;
    audit_entry.sequence_number = params.nonce;
    audit_entry._reserved = [0u8; 64];

    msg!(
        "Settlement executed: {} -> {}, amount: {}",
        params.token_in_index,
        params.token_out_index,
        params.amount
    );
    Ok(())
}
