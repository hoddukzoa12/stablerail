//! Sphere Value Object
//!
//! The Sphere defines the n-dimensional geometric space for the Orbital AMM.
//! Invariant: ||r⃗ - x⃗||² = r²
//!
//! Immutable after creation — a true Value Object in DDD terms.

use anchor_lang::prelude::*;

use super::FixedPoint;

/// Maximum number of assets in a pool
pub const MAX_ASSETS: usize = 8;

/// Sphere: defines the geometric space for the AMM invariant
#[derive(Clone, Copy, AnchorSerialize, AnchorDeserialize)]
pub struct Sphere {
    /// Radius of the sphere
    pub radius: FixedPoint,
    /// Number of assets (dimensions)
    pub n: u8,
}

impl Sphere {
    /// Create a new Sphere from total liquidity and asset count
    pub fn new(total_liquidity: FixedPoint, n: u8) -> Result<Self> {
        require!(
            n >= 2 && n as usize <= MAX_ASSETS,
            crate::errors::OrbitalError::InvalidAssetCount
        );

        // r = Total Liquidity / n
        // Each asset contributes equally at the equal price point
        let n_fp = FixedPoint::from_int(n as i64);
        let radius = total_liquidity.checked_div(n_fp)?;

        Ok(Self { radius, n })
    }

    /// r² — radius squared
    pub fn radius_squared(&self) -> Result<FixedPoint> {
        self.radius.squared()
    }

    /// Equal price point: q = r(1 - 1/√n) for each dimension
    pub fn equal_price_point(&self) -> Result<FixedPoint> {
        let n_fp = FixedPoint::from_int(self.n as i64);
        let sqrt_n = n_fp.sqrt()?;
        let one = FixedPoint::one();
        let ratio = one.checked_div(sqrt_n)?;
        let factor = one.checked_sub(ratio)?;
        self.radius.checked_mul(factor)
    }

    /// Compute Σ(r - xᵢ)² from reserve vector
    /// This is ||r⃗ - x⃗||² where r⃗ = (r, r, ..., r)
    pub fn distance_squared(&self, reserves: &[FixedPoint]) -> Result<FixedPoint> {
        let mut sum = FixedPoint::zero();
        for &x_i in reserves.iter().take(self.n as usize) {
            let diff = self.radius.checked_sub(x_i)?;
            let sq = diff.squared()?;
            sum = sum.checked_add(sq)?;
        }
        Ok(sum)
    }

    /// Verify the sphere invariant: ||r⃗ - x⃗||² = r²
    pub fn verify_invariant(&self, reserves: &[FixedPoint], epsilon: FixedPoint) -> Result<bool> {
        let lhs = self.distance_squared(reserves)?;
        let rhs = self.radius_squared()?;
        Ok(lhs.approx_eq(rhs, epsilon))
    }
}
