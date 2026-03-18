use anchor_lang::prelude::*;

use crate::errors::OrbitalError;
use crate::events::KycEntryUpdated;
use crate::state::{KycEntryState, KycStatus, PolicyState};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ManageKycEntryParams {
    /// Executor wallet to create/update KYC for
    pub member: Pubkey,
    /// KYC verification status
    pub kyc_status: KycStatus,
    /// Expiry timestamp (unix seconds)
    pub kyc_expiry: i64,
    /// KYT risk score (0-100)
    pub risk_score: u8,
    /// ISO 3166-1 alpha-2 jurisdiction code (e.g., b"US")
    pub jurisdiction: [u8; 2],
    /// AML screening cleared
    pub aml_cleared: bool,
}

/// Accounts for `manage_kyc_entry`.
///
/// Creates or updates a KYC compliance entry for an institutional executor.
/// Only the policy authority can manage KYC entries.
///
/// PDA: `["kyc_entry", policy, member]`
#[derive(Accounts)]
#[instruction(params: ManageKycEntryParams)]
pub struct ManageKycEntry<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        has_one = authority @ OrbitalError::Unauthorized,
    )]
    pub policy: Account<'info, PolicyState>,

    #[account(
        init_if_needed,
        payer = authority,
        space = KycEntryState::SIZE,
        seeds = [
            b"kyc_entry",
            policy.key().as_ref(),
            params.member.as_ref(),
        ],
        bump,
    )]
    pub kyc_entry: Box<Account<'info, KycEntryState>>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ManageKycEntry>, params: ManageKycEntryParams) -> Result<()> {
    // Validate risk score range
    require!(
        params.risk_score <= 100,
        OrbitalError::InvalidRiskScore
    );

    let kyc_entry = &mut ctx.accounts.kyc_entry;
    let clock = Clock::get()?;

    // Initialize or update fields
    kyc_entry.bump = ctx.bumps.kyc_entry;
    kyc_entry.policy = ctx.accounts.policy.key();
    kyc_entry.address = params.member;
    kyc_entry.kyc_status = params.kyc_status;
    kyc_entry.kyc_expiry = params.kyc_expiry;
    kyc_entry.risk_score = params.risk_score;
    kyc_entry.jurisdiction = params.jurisdiction;
    kyc_entry.aml_cleared = params.aml_cleared;
    kyc_entry.updated_at = clock.unix_timestamp;

    // Emit event
    let status_u8 = match params.kyc_status {
        KycStatus::Pending => 0u8,
        KycStatus::Verified => 1,
        KycStatus::Expired => 2,
        KycStatus::Revoked => 3,
    };

    emit!(KycEntryUpdated {
        policy: ctx.accounts.policy.key(),
        authority: ctx.accounts.authority.key(),
        member: params.member,
        kyc_status: status_u8,
        risk_score: params.risk_score,
        jurisdiction: params.jurisdiction,
        aml_cleared: params.aml_cleared,
        kyc_expiry: params.kyc_expiry,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "KYC entry updated: member={}, status={}, risk={}",
        params.member,
        status_u8,
        params.risk_score
    );

    Ok(())
}
