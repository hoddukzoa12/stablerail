use anchor_lang::prelude::*;
use crate::math::FixedPoint;

#[account]
pub struct PolicyState {
    pub bump: u8,
    pub authority: Pubkey,
    pub pool: Pubkey,
    pub max_trade_amount: FixedPoint,
    pub max_daily_volume: FixedPoint,
    pub current_daily_volume: FixedPoint,
    pub last_reset_timestamp: i64,
    pub is_active: bool,
    pub created_at: i64,
    pub updated_at: i64,

    // ── KYC/KYT/AML Compliance Fields (carved from _reserved) ──

    /// Maximum acceptable KYT risk score (0-100). Executors with
    /// risk_score > this threshold are rejected. Only enforced
    /// when kyc_required == true.
    pub max_risk_score: u8,
    /// Whether Travel Rule data is required for settlements
    pub require_travel_rule: bool,
    /// Settlement amount threshold above which Travel Rule applies (raw u64)
    pub travel_rule_threshold: u64,
    /// Allowed jurisdictions (ISO 3166-1 alpha-2). Only enforced when
    /// jurisdiction_count > 0 and kyc_required == true.
    pub allowed_jurisdictions: [[u8; 2]; 16],
    /// Number of active jurisdiction entries in allowed_jurisdictions
    pub jurisdiction_count: u8,
    /// Master switch: when true, execute_settlement enforces KYC/KYT/AML
    /// checks via KycEntryState. When false, falls back to legacy
    /// allowlist-only mode. Defaults to false for backward compatibility
    /// (existing on-chain accounts have _reserved bytes = 0).
    pub kyc_required: bool,

    pub _reserved: [u8; 20],
}

impl PolicyState {
    // Total size unchanged at 210:
    // 8 + 1 + 32 + 32 + 16 + 16 + 16 + 8 + 1 + 8 + 8
    // + 1 + 1 + 8 + 32 + 1 + 1 + 20 = 210
    pub const SIZE: usize = 8 + 1 + 32 + 32 + 16 + 16 + 16 + 8 + 1 + 8 + 8
        + 1 + 1 + 8 + 32 + 1 + 1 + 20;
}
