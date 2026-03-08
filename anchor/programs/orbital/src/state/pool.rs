use anchor_lang::prelude::*;
use crate::math::{sphere::MAX_ASSETS, FixedPoint, Sphere};

#[account]
pub struct PoolState {
    pub bump: u8,
    pub authority: Pubkey,
    pub sphere: Sphere,
    pub reserves: [FixedPoint; MAX_ASSETS],
    pub n_assets: u8,
    pub token_mints: [Pubkey; MAX_ASSETS],
    pub token_vaults: [Pubkey; MAX_ASSETS],
    pub fee_rate_bps: u16,
    pub total_interior_liquidity: FixedPoint,
    pub total_boundary_liquidity: FixedPoint,
    pub alpha_cache: FixedPoint,
    pub w_norm_sq_cache: FixedPoint,
    pub tick_count: u16,
    pub is_active: bool,
    pub total_volume: FixedPoint,
    pub total_fees: FixedPoint,
    pub created_at: i64,
    pub _reserved: [u8; 128],
}

impl PoolState {
    pub const SIZE: usize = 8 + 1 + 32 + 17 + (16 * MAX_ASSETS) + 1
        + (32 * MAX_ASSETS) + (32 * MAX_ASSETS) + 2 + 16 + 16 + 16 + 16
        + 2 + 1 + 16 + 16 + 8 + 128;

    pub fn active_reserves(&self) -> &[FixedPoint] {
        &self.reserves[..self.n_assets as usize]
    }
}
