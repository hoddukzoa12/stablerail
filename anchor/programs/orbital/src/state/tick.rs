use anchor_lang::prelude::*;
use crate::math::FixedPoint;

#[derive(Clone, Copy, Debug, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub enum TickStatus {
    Interior,
    Boundary,
}

#[account]
pub struct TickState {
    pub bump: u8,
    pub pool: Pubkey,
    pub k: FixedPoint,
    pub status: TickStatus,
    pub liquidity: FixedPoint,
    pub sphere_radius: FixedPoint,
    pub depeg_price: FixedPoint,
    pub x_min: FixedPoint,
    pub x_max: FixedPoint,
    pub capital_efficiency: FixedPoint,
    pub owner: Pubkey,
    pub created_at: i64,
    pub _reserved: [u8; 64],
}

impl TickState {
    pub const SIZE: usize = 8 + 1 + 32 + 16 + 1 + 16 + 16 + 16 + 16 + 16 + 16 + 32 + 8 + 64;
}
