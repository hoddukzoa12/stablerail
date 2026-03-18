use anchor_lang::prelude::*;

/// KYC verification status for institutional executor identity.
#[derive(Clone, Copy, PartialEq, Eq, AnchorSerialize, AnchorDeserialize, Debug)]
pub enum KycStatus {
    Pending,  // 0 — identity submitted, not yet verified
    Verified, // 1 — identity verified by provider
    Expired,  // 2 — verification expired, needs renewal
    Revoked,  // 3 — verification revoked due to compliance issue
}

/// Per-member KYC/KYT/AML compliance entry.
///
/// One PDA per (policy, member) pair. Stores identity verification
/// status, risk assessment, and jurisdictional data required by
/// regulated institutional operators.
///
/// PDA seeds: `["kyc_entry", policy.key(), member.key()]`
#[account]
pub struct KycEntryState {
    pub bump: u8,
    /// Parent policy this KYC entry belongs to
    pub policy: Pubkey,
    /// Executor wallet address
    pub address: Pubkey,
    /// KYC verification status
    pub kyc_status: KycStatus,
    /// KYC verification expiry (unix timestamp)
    pub kyc_expiry: i64,
    /// KYT risk score (0-100, lower = safer)
    pub risk_score: u8,
    /// ISO 3166-1 alpha-2 jurisdiction code (e.g., b"US", b"CH", b"KR")
    pub jurisdiction: [u8; 2],
    /// AML screening cleared
    pub aml_cleared: bool,
    /// Last update timestamp
    pub updated_at: i64,
    pub _reserved: [u8; 32],
}

impl KycEntryState {
    // 8 (disc) + 1 + 32 + 32 + 1 + 8 + 1 + 2 + 1 + 8 + 32 = 126
    pub const SIZE: usize = 8 + 1 + 32 + 32 + 1 + 8 + 1 + 2 + 1 + 8 + 32;
}
