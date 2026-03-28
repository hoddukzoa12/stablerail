use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;
use anchor_spl::token::{self, Token};

use crate::domain::core::{recompute_sphere, swap, update_caches};
use crate::errors::OrbitalError;
use crate::events::SettlementExecuted;
use crate::math::newton::compute_amount_out_analytical;
use crate::math::FixedPoint;
use crate::state::{
    AllowlistState, AuditEntryState, KycEntryState, KycStatus, PolicyState, PoolState,
    SettlementState, SettlementStatus,
};

/// Travel Rule payload — per-transfer originator/beneficiary identification
/// required for settlements above the policy's configured threshold.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct TravelRuleData {
    /// Originator name (e.g., institution name), max 64 bytes
    pub originator_name: [u8; 64],
    /// Beneficiary name, max 64 bytes
    pub beneficiary_name: [u8; 64],
    /// Originator VASP identifier (e.g., LEI or DID), max 32 bytes
    pub originator_vasp: [u8; 32],
    /// Transfer purpose code (e.g., b"TRADE", b"SETTL")
    pub purpose: [u8; 8],
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ExecuteSettlementParams {
    pub token_in_index: u8,
    pub token_out_index: u8,
    pub amount: u64,
    pub min_amount_out: u64,
    pub nonce: u64,
    /// Travel Rule data, required when policy.require_travel_rule is true
    /// and amount >= policy.travel_rule_threshold
    pub travel_rule_data: Option<TravelRuleData>,
}

/// Accounts for `execute_settlement`.
///
/// `remaining_accounts` layout (4 accounts):
///   [0] = vault_in         (writable, receives token_in deposit)
///   [1] = vault_out        (writable, sends token_out to executor)
///   [2] = executor_ata_in  (writable, executor's source for token_in)
///   [3] = executor_ata_out (writable, executor's destination for token_out)
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
        mut,
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

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, ExecuteSettlement<'info>>,
    params: ExecuteSettlementParams,
) -> Result<()> {
    let pool = &ctx.accounts.pool;
    let allowlist = &ctx.accounts.allowlist;
    let executor = &ctx.accounts.executor;

    let token_in = params.token_in_index as usize;
    let token_out = params.token_out_index as usize;

    // ── Early validation (save CU on bad inputs) ──
    require!(pool.is_active, OrbitalError::PoolNotActive);
    require!(token_in != token_out, OrbitalError::SameTokenSwap);
    require!(
        token_in < pool.n_assets as usize && token_out < pool.n_assets as usize,
        OrbitalError::InvalidTokenIndex
    );
    require!(params.amount > 0, OrbitalError::NegativeTradeAmount);

    // ── Policy checks + daily volume tracking ──
    require!(
        allowlist.contains(&executor.key()),
        OrbitalError::Unauthorized
    );

    let amount = FixedPoint::from_token_amount(params.amount, pool.token_decimals[token_in])?;
    let clock = Clock::get()?;
    let policy = &mut ctx.accounts.policy;

    require!(
        amount.raw <= policy.max_trade_amount.raw,
        OrbitalError::PolicyLimitExceeded
    );

    const SECONDS_PER_DAY: i64 = 86_400;
    if clock.unix_timestamp - policy.last_reset_timestamp >= SECONDS_PER_DAY {
        policy.current_daily_volume = FixedPoint::zero();
        policy.last_reset_timestamp = clock.unix_timestamp;
    }

    let new_daily_volume = policy.current_daily_volume.checked_add(amount)?;
    require!(
        new_daily_volume.raw <= policy.max_daily_volume.raw,
        OrbitalError::DailyVolumeLimitExceeded
    );
    policy.current_daily_volume = new_daily_volume;

    // ── KYC/KYT/AML compliance checks (when policy.kyc_required is true) ──
    //
    // remaining_accounts layout:
    //   [0..4) = vaults + ATAs (always required)
    //   [4]    = kyc_entry PDA (required when policy.kyc_required == true)
    let remaining = &ctx.remaining_accounts;
    let expected_remaining = if policy.kyc_required { 5 } else { 4 };
    require!(
        remaining.len() >= expected_remaining,
        OrbitalError::InvalidRemainingAccounts
    );

    if policy.kyc_required {
        // Cache policy compliance fields to avoid borrow conflicts
        let policy_key = policy.key();
        let max_risk = policy.max_risk_score;
        let jur_count = policy.jurisdiction_count as usize;
        let jur_list = policy.allowed_jurisdictions;
        let require_travel_rule = policy.require_travel_rule;
        let travel_rule_threshold = policy.travel_rule_threshold;

        let kyc_acc = &remaining[4];
        // Validate program ownership (prevents forged accounts)
        require!(kyc_acc.owner == &crate::ID, OrbitalError::KycNotVerified);
        let data = kyc_acc.try_borrow_data()?;
        let mut slice: &[u8] = &data;
        let kyc_entry = KycEntryState::try_deserialize(&mut slice)
            .map_err(|_| OrbitalError::KycNotVerified)?;

        // Validate the KYC entry belongs to this policy and executor
        require!(kyc_entry.policy == policy_key, OrbitalError::KycNotVerified);
        require!(kyc_entry.address == executor.key(), OrbitalError::KycNotVerified);

        // KYC status must be Verified
        require!(kyc_entry.kyc_status == KycStatus::Verified, OrbitalError::KycNotVerified);

        // KYC must not be expired
        require!(kyc_entry.kyc_expiry > clock.unix_timestamp, OrbitalError::KycExpired);

        // Risk score must be within policy threshold
        require!(kyc_entry.risk_score <= max_risk, OrbitalError::RiskScoreExceeded);

        // AML screening must be cleared
        require!(kyc_entry.aml_cleared, OrbitalError::AmlNotCleared);

        // Jurisdiction check (only when policy has allowed jurisdictions)
        if jur_count > 0 {
            let allowed = jur_list[..jur_count]
                .iter()
                .any(|j| *j == kyc_entry.jurisdiction);
            require!(allowed, OrbitalError::JurisdictionNotAllowed);
        }

        // Travel Rule enforcement: when enabled, settlements at or above
        // the threshold require a TravelRuleData payload with non-empty
        // originator/beneficiary identification per FATF guidelines.
        // When threshold is 0, ALL settlements require Travel Rule data.
        if require_travel_rule {
            if travel_rule_threshold == 0 || params.amount >= travel_rule_threshold {
                let tr = params
                    .travel_rule_data
                    .as_ref()
                    .ok_or(OrbitalError::TravelRuleRequired)?;
                // Originator name must not be all zeros
                require!(
                    tr.originator_name.iter().any(|&b| b != 0),
                    OrbitalError::TravelRuleRequired
                );
                // Beneficiary name must not be all zeros
                require!(
                    tr.beneficiary_name.iter().any(|&b| b != 0),
                    OrbitalError::TravelRuleRequired
                );
            }
        }
    }

    // ── Validate remaining_accounts (vaults + ATAs) ──

    require!(
        *remaining[0].key == pool.token_vaults[token_in],
        OrbitalError::InvalidVaultAddress
    );
    require!(
        *remaining[1].key == pool.token_vaults[token_out],
        OrbitalError::InvalidVaultAddress
    );

    // ── Validate executor_ata_out is owned by executor ──
    // SPL Token Transfer does not enforce destination owner. Without this
    // check an allowlisted executor could route settlement output to an
    // arbitrary account of the same mint, producing a misleading audit trail.
    {
        let ata_out_data = remaining[3].try_borrow_data()?;
        require!(
            ata_out_data.len() >= 64,
            OrbitalError::InvalidRemainingAccounts
        );
        // SPL Token Account layout: [mint 32B][owner 32B][amount 8B]...
        require!(
            ata_out_data[32..64] == executor.key().to_bytes(),
            OrbitalError::Unauthorized
        );
    }

    // ── SPL transfer IN: executor_ata_in → vault_in (executor signs) ──
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: remaining[2].clone(),
                to: remaining[0].clone(),
                authority: ctx.accounts.executor.to_account_info(),
            },
        ),
        params.amount,
    )?;

    // ── Compute precise amount_out on-chain ──
    let min_amount_out = FixedPoint::from_token_amount(params.min_amount_out, pool.token_decimals[token_out])?;
    let fee = swap::compute_fee(amount, pool.fee_rate_bps)?;
    let net_in = amount.checked_sub(fee)?;
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
        amount,
        precise_amount_out,
        min_amount_out,
    )?;

    // ── SPL transfer OUT: vault_out → executor_ata_out (pool PDA signs) ──
    // Floor rounding: vault always has enough tokens. Executor receives ≤ computed amount.
    let amount_out_u64 = result.amount_out.to_token_amount_floor(pool.token_decimals[token_out])?;
    require!(amount_out_u64 > 0, OrbitalError::SwapOutputTooSmall);

    let authority_key = pool.authority;
    let pool_bump = pool.bump;
    let pool_seeds: &[&[u8]] = &[b"pool", authority_key.as_ref(), &[pool_bump]];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: remaining[1].clone(),
                to: remaining[3].clone(),
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

    // ── Compute action_hash (on-chain SHA256 syscall) ──
    let settlement_key = ctx.accounts.settlement.key();
    let pool_key = pool.key();
    let policy_key = ctx.accounts.policy.key();
    let executor_key = executor.key();
    let action_hash: [u8; 32] = hashv(&[
        settlement_key.as_ref(),
        pool_key.as_ref(),
        policy_key.as_ref(),
        executor_key.as_ref(),
        &params.token_in_index.to_le_bytes(),
        &params.token_out_index.to_le_bytes(),
        &result.amount_in.raw.to_le_bytes(),
        &result.amount_out.raw.to_le_bytes(),
        &clock.unix_timestamp.to_le_bytes(),
    ])
    .to_bytes();

    // ── Record settlement (Executed) ──
    let settlement = &mut ctx.accounts.settlement;
    settlement.bump = ctx.bumps.settlement;
    settlement.pool = pool_key;
    settlement.policy = policy_key;
    settlement.executor = executor_key;
    settlement.token_in_index = params.token_in_index;
    settlement.token_out_index = params.token_out_index;
    settlement.amount_in = result.amount_in;
    settlement.amount_out = result.amount_out;
    settlement.execution_price = result.execution_price;
    settlement.status = SettlementStatus::Executed;
    settlement.executed_at = clock.unix_timestamp;
    settlement.nonce = params.nonce;
    settlement._reserved = [0u8; 64];

    // ── Create audit entry (immutable) ──
    let audit_entry = &mut ctx.accounts.audit_entry;
    audit_entry.bump = ctx.bumps.audit_entry;
    audit_entry.settlement = settlement.key();
    audit_entry.executor = executor_key;
    audit_entry.pool = pool_key;
    audit_entry.policy = policy_key;
    audit_entry.action_hash = action_hash;
    audit_entry.amount = result.amount_in;
    audit_entry.timestamp = clock.unix_timestamp;
    audit_entry.sequence_number = params.nonce;
    audit_entry._reserved = [0u8; 64];

    // ── Emit event ──
    emit!(SettlementExecuted {
        settlement: settlement.key(),
        pool: pool_key,
        policy: policy_key,
        executor: executor_key,
        token_in: pool.token_mints[token_in],
        token_out: pool.token_mints[token_out],
        amount_in: result.amount_in.raw,
        amount_out: result.amount_out.raw,
        price: result.execution_price.raw,
        action_hash,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Settlement executed: {} -> {}, in={}, out={}",
        params.token_in_index,
        params.token_out_index,
        params.amount,
        amount_out_u64
    );
    Ok(())
}
