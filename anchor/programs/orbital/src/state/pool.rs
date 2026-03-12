use anchor_lang::prelude::*;
use crate::math::{sphere::MAX_ASSETS, FixedPoint, Sphere};

#[account]
pub struct PoolState {
    pub bump: u8,
    pub authority: Pubkey,
    pub sphere: Sphere,
    pub reserves: [FixedPoint; MAX_ASSETS],
    pub n_assets: u8,
    /// Decimal places for each token mint (e.g., 6 for USDC).
    /// Used for boundary normalization: raw SPL amounts ÷ 10^decimals → FixedPoint.
    pub token_decimals: [u8; MAX_ASSETS],
    pub token_mints: [Pubkey; MAX_ASSETS],
    pub token_vaults: [Pubkey; MAX_ASSETS],
    /// Bump seeds for vault PDAs (needed for CPI signing)
    pub vault_bumps: [u8; MAX_ASSETS],
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
    /// Monotonically incrementing counter for position PDA derivation
    pub position_count: u64,
    pub _reserved: [u8; 104],
}

impl PoolState {
    pub const SIZE: usize = 8               // anchor discriminator
        + 1                                  // bump
        + 32                                 // authority
        + 17                                 // sphere (FixedPoint=16 + u8=1)
        + (16 * MAX_ASSETS)                  // reserves
        + 1                                  // n_assets
        + MAX_ASSETS                         // token_decimals
        + (32 * MAX_ASSETS)                  // token_mints
        + (32 * MAX_ASSETS)                  // token_vaults
        + MAX_ASSETS                         // vault_bumps
        + 2                                  // fee_rate_bps
        + 16                                 // total_interior_liquidity
        + 16                                 // total_boundary_liquidity
        + 16                                 // alpha_cache
        + 16                                 // w_norm_sq_cache
        + 2                                  // tick_count
        + 1                                  // is_active
        + 16                                 // total_volume
        + 16                                 // total_fees
        + 8                                  // created_at
        + 8                                  // position_count
        + 104;                               // _reserved

    pub fn active_reserves(&self) -> &[FixedPoint] {
        &self.reserves[..self.n_assets as usize]
    }
}
