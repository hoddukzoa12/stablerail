use anchor_lang::prelude::*;
use crate::math::FixedPoint;

#[account]
pub struct AuditEntryState {
    pub bump: u8,
    pub settlement: Pubkey,
    pub executor: Pubkey,
    pub pool: Pubkey,
    pub policy: Pubkey,
    pub action_hash: [u8; 32],
    pub amount: FixedPoint,
    pub timestamp: i64,
    pub sequence_number: u64,
    pub _reserved: [u8; 64],
}

impl AuditEntryState {
    pub const SIZE: usize = 8 + 1 + 32 + 32 + 32 + 32 + 32 + 16 + 8 + 8 + 64;
}
