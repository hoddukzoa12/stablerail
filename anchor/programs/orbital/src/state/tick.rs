use anchor_lang::prelude::*;
use crate::math::{sphere::MAX_ASSETS, FixedPoint};

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
    /// Per-tick reserves: tracks each asset's share of liquidity within this tick.
    /// Only first `pool.n_assets` entries are used (rest are zero).
    /// When status == Interior, these reserves contribute to pool.reserves.
    /// On Interior→Boundary crossing, subtracted from pool; on Boundary→Interior, added back.
    pub reserves: [FixedPoint; MAX_ASSETS],
    pub _reserved: [u8; 32],
}

impl TickState {
    // SIZE breakdown:
    //   8   = anchor discriminator
    //   1   = bump
    //   32  = pool (Pubkey)
    //   16  = k (FixedPoint)
    //   1   = status (enum u8)
    //   16  = liquidity
    //   16  = sphere_radius
    //   16  = depeg_price
    //   16  = x_min
    //   16  = x_max
    //   16  = capital_efficiency
    //   32  = owner (Pubkey)
    //   8   = created_at (i64)
    //   128 = reserves (16 * MAX_ASSETS=8)
    //   32  = _reserved
    // total = 374
    pub const SIZE: usize = 8 + 1 + 32 + 16 + 1 + 16 + 16 + 16 + 16 + 16 + 16 + 32 + 8
        + (16 * MAX_ASSETS) // reserves
        + 32;               // _reserved
}
