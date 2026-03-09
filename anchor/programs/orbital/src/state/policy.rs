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
    pub _reserved: [u8; 64],
}

impl PolicyState {
    pub const SIZE: usize = 8 + 1 + 32 + 32 + 16 + 16 + 16 + 8 + 1 + 8 + 8 + 64;
}
