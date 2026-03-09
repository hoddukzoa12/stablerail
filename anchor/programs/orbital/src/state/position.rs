use anchor_lang::prelude::*;
use crate::math::FixedPoint;

#[account]
pub struct PositionState {
    pub bump: u8,
    pub pool: Pubkey,
    pub tick: Pubkey,
    pub owner: Pubkey,
    pub liquidity: FixedPoint,
    pub tick_lower: FixedPoint,
    pub tick_upper: FixedPoint,
    pub fees_earned: FixedPoint,
    pub created_at: i64,
    pub updated_at: i64,
    pub _reserved: [u8; 64],
}

impl PositionState {
    pub const SIZE: usize = 8 + 1 + 32 + 32 + 32 + 16 + 16 + 16 + 16 + 8 + 8 + 64;
}
