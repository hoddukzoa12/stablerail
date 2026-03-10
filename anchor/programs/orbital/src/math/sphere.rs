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
        require!(
            reserves.len() >= self.n as usize,
            crate::errors::OrbitalError::InvalidAssetCount
        );
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

    /// Marginal price of token i in terms of token j.
    ///
    /// price(i, j) = dx_i/dx_j = (r - x_j) / (r - x_i)
    ///
    /// Convenience method that operates on a reserve slice directly.
    /// For repeated price queries, prefer `ReserveState::price()`.
    pub fn price(&self, i: usize, j: usize, reserves: &[FixedPoint]) -> Result<FixedPoint> {
        require!(
            i != j,
            crate::errors::OrbitalError::SameTokenSwap
        );
        require!(
            i < self.n as usize && j < self.n as usize,
            crate::errors::OrbitalError::InvalidTokenIndex
        );
        require!(
            reserves.len() >= self.n as usize,
            crate::errors::OrbitalError::InvalidAssetCount
        );

        let numerator = self.radius.checked_sub(reserves[j])?;
        let denominator = self.radius.checked_sub(reserves[i])?;

        numerator.checked_div(denominator)
    }

    /// Invariant tolerance: r² >> 24 ≈ r² × 6e-8
    ///
    /// Derived from Q64.64 squared-operation rounding bounds: each
    /// `checked_mul` introduces up to 2^-64 relative error; summing n
    /// terms and squaring yields worst-case O(n · 2^-63). Shifting by
    /// 24 bits (≈ 6e-8) provides ample headroom for n ≤ 8 while
    /// rejecting economically significant deviations.
    fn invariant_tolerance(&self) -> Result<FixedPoint> {
        let r_sq = self.radius_squared()?;
        Ok(FixedPoint::from_raw(r_sq.raw >> 24))
    }

    /// Check sphere invariant with fixed-point-aware tolerance.
    ///
    /// Returns `Ok(())` if satisfied, `InvariantViolation` error otherwise.
    /// Uses O(n) loop; for O(1) path, use `check_invariant_with_distance_sq`.
    pub fn check_invariant(&self, reserves: &[FixedPoint]) -> Result<()> {
        let tolerance = self.invariant_tolerance()?;
        let valid = self.verify_invariant(reserves, tolerance)?;
        require!(valid, crate::errors::OrbitalError::InvariantViolation);
        Ok(())
    }

    /// Check sphere invariant using pre-computed distance squared (O(1) path).
    ///
    /// Accepts the result of `ReserveState::distance_squared_from_center()`.
    /// Avoids O(n) re-computation during swap execution.
    pub fn check_invariant_with_distance_sq(&self, distance_sq: FixedPoint) -> Result<()> {
        let r_sq = self.radius_squared()?;
        let tolerance = self.invariant_tolerance()?;
        require!(
            distance_sq.approx_eq(r_sq, tolerance),
            crate::errors::OrbitalError::InvariantViolation
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sphere_price_equal_reserves() {
        // At equal price point, price(i, j) = 1.0 for all i, j
        let sphere = Sphere::new(FixedPoint::from_int(600), 3).unwrap(); // r = 200
        let q = sphere.equal_price_point().unwrap();
        let reserves = [q, q, q];

        let price = sphere.price(0, 1, &reserves).unwrap();
        let eps = FixedPoint::from_raw(1i128 << 40);
        assert!(
            price.approx_eq(FixedPoint::one(), eps),
            "price at equal reserves should be 1.0, got {:?}",
            price
        );
    }

    #[test]
    fn test_sphere_check_invariant_pass() {
        let sphere = Sphere::new(FixedPoint::from_int(600), 3).unwrap(); // r = 200
        let q = sphere.equal_price_point().unwrap();
        let reserves = [q, q, q];

        // Should not return an error
        sphere.check_invariant(&reserves).unwrap();
    }

    #[test]
    fn test_sphere_check_invariant_fail() {
        let sphere = Sphere::new(FixedPoint::from_int(600), 3).unwrap(); // r = 200
        // Reserves far from the sphere surface
        let reserves = [
            FixedPoint::from_int(10),
            FixedPoint::from_int(10),
            FixedPoint::from_int(10),
        ];

        // Should return InvariantViolation
        assert!(sphere.check_invariant(&reserves).is_err());
    }
}
