use anchor_lang::prelude::*;
use crate::math::FixedPoint;

#[derive(Clone, Copy, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub enum SettlementStatus {
    Pending,
    Executed,
    Failed,
}

#[account]
pub struct SettlementState {
    pub bump: u8,
    pub pool: Pubkey,
    pub policy: Pubkey,
    pub executor: Pubkey,
    pub token_in_index: u8,
    pub token_out_index: u8,
    pub amount_in: FixedPoint,
    pub amount_out: FixedPoint,
    pub execution_price: FixedPoint,
    pub status: SettlementStatus,
    pub executed_at: i64,
    pub nonce: u64,
    pub _reserved: [u8; 64],
}

impl SettlementState {
    pub const SIZE: usize = 8 + 1 + 32 + 32 + 32 + 1 + 1 + 16 + 16 + 16 + 1 + 8 + 8 + 64;
}
