//! Torus Geometry Value Object
//!
//! The torus invariant arises from consolidating all interior ticks into
//! one n-dimensional sphere and all boundary ticks into one (n-1)-dimensional
//! sphere, then rotating the interior sphere around the boundary circle.
//!
//! Heavy torus computation (Newton solver) runs off-chain (Issue #9, #27).
//! This module provides on-chain helpers for:
//!   - Torus consolidation parameters
//!   - Tick crossing detection via alpha comparison
//!   - Orthogonal subspace radius computation

use anchor_lang::prelude::*;

use super::fixed_point::FixedPoint;
use super::sphere::Sphere;
use crate::errors::OrbitalError;

// ══════════════════════════════════════════════════════════════
// TorusParams — consolidated torus parameters
// ══════════════════════════════════════════════════════════════

/// Consolidated torus parameters derived from pool state.
///
/// Interior ticks consolidate: r_interior = Σ liquidity (interior ticks)
/// Boundary ticks consolidate: s_boundary = Σ s (boundary ticks)
///
/// For MVP with no ticks, both are zero and the pool operates as
/// a single-sphere AMM (equivalent to one interior tick spanning
/// the entire sphere).
#[derive(Clone, Copy, Debug)]
pub struct TorusParams {
    pub r_interior: FixedPoint,
    pub s_boundary: FixedPoint,
}

impl TorusParams {
    /// Construct from pool's cached liquidity totals.
    pub fn from_pool_liquidity(
        total_interior: FixedPoint,
        total_boundary: FixedPoint,
    ) -> Self {
        Self {
            r_interior: total_interior,
            s_boundary: total_boundary,
        }
    }

    /// Whether any boundary liquidity exists (torus vs pure sphere).
    pub fn has_boundary_liquidity(&self) -> bool {
        self.s_boundary.raw > 0
    }

    /// Whether no ticks exist (pure sphere mode, MVP default).
    pub fn is_single_sphere(&self) -> bool {
        self.r_interior.is_zero() && self.s_boundary.is_zero()
    }
}

// ══════════════════════════════════════════════════════════════
// Tick crossing detection
// ══════════════════════════════════════════════════════════════

/// Direction of a tick boundary crossing during a swap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrossingDirection {
    /// Alpha decreased past tick k: tick transitions Interior → Boundary
    InteriorToBoundary,
    /// Alpha increased past tick k: tick transitions Boundary → Interior
    BoundaryToInterior,
}

/// Detect whether alpha crossed a tick boundary k during a swap.
///
/// Alpha = Σxᵢ / √n is the parallel projection of the reserve vector
/// onto the (1,...,1)/√n diagonal. Each tick's plane constant k defines
/// a hyperplane x·v = k. A crossing occurs when alpha moves from one
/// side of k to the other.
///
/// Returns `None` if no crossing occurred.
pub fn detect_tick_crossing(
    old_alpha: FixedPoint,
    new_alpha: FixedPoint,
    tick_k: FixedPoint,
) -> Option<CrossingDirection> {
    if old_alpha.raw > tick_k.raw && new_alpha.raw <= tick_k.raw {
        Some(CrossingDirection::InteriorToBoundary)
    } else if old_alpha.raw <= tick_k.raw && new_alpha.raw > tick_k.raw {
        Some(CrossingDirection::BoundaryToInterior)
    } else {
        None
    }
}

// ══════════════════════════════════════════════════════════════
// Orthogonal subspace radius
// ══════════════════════════════════════════════════════════════

/// Compute the orthogonal subspace radius s at a given alpha.
///
/// s(α) = √(r² - (α - r·√n)²)
///
/// Measures the "room" available in the orthogonal direction at the
/// current parallel projection alpha. Used for torus geometry validation
/// and boundary tick consolidation.
pub fn orthogonal_radius(sphere: &Sphere, alpha: FixedPoint) -> Result<FixedPoint> {
    let n_fp = FixedPoint::from_int(sphere.n as i64);
    let sqrt_n = n_fp.sqrt()?;
    let r = sphere.radius;

    let r_sq = r.squared()?;
    let offset = alpha.checked_sub(r.checked_mul(sqrt_n)?)?;
    let offset_sq = offset.squared()?;
    let radicand = r_sq.checked_sub(offset_sq)?;

    // Clamp tiny negatives from fixed-point rounding.
    // Tolerance ≈ 6e-8 (2^-40): covers squared-operation rounding
    // while rejecting genuine geometric constraint violations.
    if radicand.raw < 0 {
        const RADICAND_EPSILON_RAW: i128 = -(1i128 << 40);
        require!(
            radicand.raw >= RADICAND_EPSILON_RAW,
            OrbitalError::TorusInvariantError
        );
        return Ok(FixedPoint::zero());
    }
    radicand.sqrt()
}

// ══════════════════════════════════════════════════════════════
// Post-trade alpha prediction
// ══════════════════════════════════════════════════════════════

/// Compute the post-trade alpha without modifying reserves.
///
/// new_sum = old_sum + amount_in - amount_out
/// new_alpha = new_sum / √n
///
/// Used for tick crossing detection before applying the trade.
pub fn compute_new_alpha(
    current_running_sum: FixedPoint,
    amount_in: FixedPoint,
    amount_out: FixedPoint,
    n: u8,
) -> Result<FixedPoint> {
    let new_sum = current_running_sum
        .checked_add(amount_in)?
        .checked_sub(amount_out)?;
    let n_fp = FixedPoint::from_int(n as i64);
    let sqrt_n = n_fp.sqrt()?;
    new_sum.checked_div(sqrt_n)
}

// ══════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── TorusParams ──

    #[test]
    fn test_torus_params_single_sphere_when_both_zero() {
        let tp = TorusParams::from_pool_liquidity(FixedPoint::zero(), FixedPoint::zero());
        assert!(tp.is_single_sphere());
        assert!(!tp.has_boundary_liquidity());
    }

    #[test]
    fn test_torus_params_has_boundary_liquidity() {
        let tp = TorusParams::from_pool_liquidity(
            FixedPoint::from_int(100),
            FixedPoint::from_int(50),
        );
        assert!(!tp.is_single_sphere());
        assert!(tp.has_boundary_liquidity());
    }

    #[test]
    fn test_torus_params_interior_only() {
        let tp = TorusParams::from_pool_liquidity(
            FixedPoint::from_int(100),
            FixedPoint::zero(),
        );
        assert!(!tp.is_single_sphere());
        assert!(!tp.has_boundary_liquidity());
    }

    // ── detect_tick_crossing ──

    #[test]
    fn test_crossing_interior_to_boundary() {
        let old = FixedPoint::from_int(10);
        let new = FixedPoint::from_int(5);
        let k = FixedPoint::from_int(7);
        assert_eq!(
            detect_tick_crossing(old, new, k),
            Some(CrossingDirection::InteriorToBoundary)
        );
    }

    #[test]
    fn test_crossing_boundary_to_interior() {
        let old = FixedPoint::from_int(5);
        let new = FixedPoint::from_int(10);
        let k = FixedPoint::from_int(7);
        assert_eq!(
            detect_tick_crossing(old, new, k),
            Some(CrossingDirection::BoundaryToInterior)
        );
    }

    #[test]
    fn test_no_crossing_same_side() {
        let old = FixedPoint::from_int(10);
        let new = FixedPoint::from_int(9);
        let k = FixedPoint::from_int(7);
        assert_eq!(detect_tick_crossing(old, new, k), None);
    }

    #[test]
    fn test_crossing_exact_at_k() {
        // old > k, new == k → InteriorToBoundary
        let old = FixedPoint::from_int(8);
        let new = FixedPoint::from_int(7);
        let k = FixedPoint::from_int(7);
        assert_eq!(
            detect_tick_crossing(old, new, k),
            Some(CrossingDirection::InteriorToBoundary)
        );
    }

    // ── orthogonal_radius ──

    #[test]
    fn test_orthogonal_radius_at_equal_price_point() {
        // At α = r·√n - r (equal price point), s should be > 0
        let sphere = Sphere { radius: FixedPoint::from_int(100), n: 3 };
        let eq_point = sphere.equal_price_point().unwrap();
        // α at equal price = n * eq_point / √n = √n * eq_point
        let n_fp = FixedPoint::from_int(3);
        let sqrt_n = n_fp.sqrt().unwrap();
        let alpha_eq = eq_point.checked_mul(sqrt_n).unwrap();
        let s = orthogonal_radius(&sphere, alpha_eq).unwrap();
        assert!(s.is_positive());
    }

    #[test]
    fn test_orthogonal_radius_at_diagonal() {
        // At α = r·√n (offset = 0), s = √(r²) = r (max orthogonal room)
        let sphere = Sphere { radius: FixedPoint::from_int(100), n: 3 };
        let n_fp = FixedPoint::from_int(3);
        let sqrt_n = n_fp.sqrt().unwrap();
        let alpha_diag = sphere.radius.checked_mul(sqrt_n).unwrap();
        let s = orthogonal_radius(&sphere, alpha_diag).unwrap();
        assert!(s.approx_eq(sphere.radius, FixedPoint::from_int(1)));
    }

    #[test]
    fn test_orthogonal_radius_at_equal_price_zero() {
        // At equal price point α = r(√n - 1), offset = -r, s = √(r² - r²) = 0
        let sphere = Sphere { radius: FixedPoint::from_int(100), n: 3 };
        let n_fp = FixedPoint::from_int(3);
        let sqrt_n = n_fp.sqrt().unwrap();
        let one = FixedPoint::one();
        let alpha_eq = sphere.radius.checked_mul(sqrt_n.checked_sub(one).unwrap()).unwrap();
        let s = orthogonal_radius(&sphere, alpha_eq).unwrap();
        // Should be approximately zero (tiny fp rounding)
        assert!(s.approx_eq(FixedPoint::zero(), FixedPoint::from_int(2)));
    }

    #[test]
    fn test_orthogonal_radius_positive_for_valid_alpha() {
        let sphere = Sphere { radius: FixedPoint::from_int(100), n: 3 };
        // Alpha somewhere in the valid range
        let alpha = FixedPoint::from_int(150);
        let s = orthogonal_radius(&sphere, alpha).unwrap();
        assert!(s.is_positive());
    }

    #[test]
    fn test_orthogonal_radius_rejects_large_negative_radicand() {
        // Alpha far outside valid range → radicand is very negative → should error
        let sphere = Sphere { radius: FixedPoint::from_int(100), n: 3 };
        // Alpha = 0 → offset = 0 - r√3 ≈ -173 → offset² ≈ 29929 >> r²=10000
        // radicand = 10000 - 29929 ≈ -19929 (way beyond epsilon)
        let alpha = FixedPoint::zero();
        let result = orthogonal_radius(&sphere, alpha);
        assert!(result.is_err(), "Large negative radicand should be rejected");
    }

    #[test]
    fn test_orthogonal_radius_clamps_tiny_negative() {
        // Radicand just barely negative (within epsilon) → should clamp to zero
        let sphere = Sphere { radius: FixedPoint::from_int(100), n: 3 };
        let n_fp = FixedPoint::from_int(3);
        let sqrt_n = n_fp.sqrt().unwrap();

        // At α = r(√n - 1), radicand should be ≈ 0 but may be slightly negative
        // due to fixed-point rounding → should clamp to 0, not error
        let one = FixedPoint::one();
        let alpha_boundary = sphere.radius.checked_mul(sqrt_n.checked_sub(one).unwrap()).unwrap();
        let s = orthogonal_radius(&sphere, alpha_boundary).unwrap();
        // Should be zero or very close to zero (clamped)
        assert!(s.approx_eq(FixedPoint::zero(), FixedPoint::from_int(2)));
    }

    // ── compute_new_alpha ──

    #[test]
    fn test_compute_new_alpha_symmetric_trade() {
        // If amount_in == amount_out, alpha shouldn't change
        let sum = FixedPoint::from_int(300); // 3 assets at 100 each
        let amount = FixedPoint::from_int(10);
        let new_alpha = compute_new_alpha(sum, amount, amount, 3).unwrap();
        let n_fp = FixedPoint::from_int(3);
        let sqrt_n = n_fp.sqrt().unwrap();
        let expected = sum.checked_div(sqrt_n).unwrap();
        assert!(new_alpha.approx_eq(expected, FixedPoint::from_int(1)));
    }

    #[test]
    fn test_compute_new_alpha_net_inflow() {
        // amount_in > amount_out → alpha increases
        let sum = FixedPoint::from_int(300);
        let n_fp = FixedPoint::from_int(3);
        let sqrt_n = n_fp.sqrt().unwrap();
        let old_alpha = sum.checked_div(sqrt_n).unwrap();

        let new_alpha = compute_new_alpha(
            sum,
            FixedPoint::from_int(20),
            FixedPoint::from_int(10),
            3,
        )
        .unwrap();
        assert!(new_alpha.raw > old_alpha.raw);
    }
}
