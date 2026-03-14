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
// Consolidated tick data for trade segmentation
// ══════════════════════════════════════════════════════════════

use crate::state::TickStatus;

/// Consolidated view of active tick boundaries near the current alpha.
///
/// Used by the trade segmentation loop in `execute_swap` to determine
/// if a swap will cross a tick boundary and which tick's k to target.
#[derive(Clone, Copy, Debug)]
pub struct ConsolidatedTickData {
    /// Whether any boundary tick exists in the pool
    pub has_boundary: bool,
    /// Nearest Interior tick k below current alpha (alpha decreasing direction)
    /// When alpha decreases past this k, that tick transitions Interior → Boundary
    pub nearest_k_lower: Option<FixedPoint>,
    /// Nearest Boundary tick k above current alpha (alpha increasing direction)
    /// When alpha increases past this k, that Boundary tick transitions
    /// Boundary → Interior
    pub nearest_k_upper: Option<FixedPoint>,
}

/// Find the nearest tick boundaries relative to the current alpha.
///
/// Scans all ticks and finds:
///   - nearest_k_lower: largest Interior tick k that is < current alpha (strict)
///   - nearest_k_upper: smallest Boundary tick k that is > current alpha (strict)
///
/// This maps to the reference implementation's `_getConsolidatedTickData()`.
pub fn find_nearest_tick_boundaries(
    ticks: &[(FixedPoint, TickStatus)],
    current_alpha: FixedPoint,
) -> ConsolidatedTickData {
    let mut has_boundary = false;
    let mut nearest_k_lower: Option<FixedPoint> = None;
    let mut nearest_k_upper: Option<FixedPoint> = None;

    for &(k, status) in ticks {
        match status {
            TickStatus::Interior => {
                // Interior ticks strictly below alpha: potential crossing on alpha decrease.
                // Strict inequality prevents spurious crossings when alpha == k,
                // and mirrors the strict `>` used for Boundary ticks above.
                if k.raw < current_alpha.raw {
                    match nearest_k_lower {
                        None => nearest_k_lower = Some(k),
                        Some(prev) if k.raw > prev.raw => nearest_k_lower = Some(k),
                        _ => {}
                    }
                }
            }
            TickStatus::Boundary => {
                has_boundary = true;
                // Boundary ticks above alpha: potential crossing on alpha increase
                if k.raw > current_alpha.raw {
                    match nearest_k_upper {
                        None => nearest_k_upper = Some(k),
                        Some(prev) if k.raw < prev.raw => nearest_k_upper = Some(k),
                        _ => {}
                    }
                }
            }
        }
    }

    ConsolidatedTickData {
        has_boundary,
        nearest_k_lower,
        nearest_k_upper,
    }
}

// ══════════════════════════════════════════════════════════════
// Delta-to-boundary solver (quadratic closed-form)
// ══════════════════════════════════════════════════════════════

/// Compute the amount of token_in needed to reach a tick boundary at k_cross.
///
/// Solves the quadratic equation arising from two constraints:
///   1. Sphere invariant: Σ(r - xᵢ)² = r²
///   2. Alpha target: new_alpha = (Σxᵢ + delta_in - delta_out) / √n = k_cross
///
/// Derivation:
///   Let a = r - x_in, b = r - x_out, C = running_sum - k_cross·√n
///   Then delta_out = delta_in + C  (from alpha constraint)
///   Substituting into sphere invariant:
///     (a - d)² + (b + d + C)² = a² + b²
///   Expanding: 2d² + 2(b+C-a)d + (2bC + C²) = 0
///
/// Returns the positive root (delta_in to reach boundary).
/// Returns zero if the boundary is already reached or unreachable.
pub fn compute_delta_to_boundary(
    sphere: &Sphere,
    reserves: &[FixedPoint],
    token_in: usize,
    token_out: usize,
    k_cross: FixedPoint,
    n: u8,
) -> Result<FixedPoint> {
    let r = sphere.radius;
    let n_fp = FixedPoint::from_int(n as i64);
    let sqrt_n = n_fp.sqrt()?;

    // a = r - x_in, b = r - x_out
    let a = r.checked_sub(reserves[token_in])?;
    let b = r.checked_sub(reserves[token_out])?;

    // running_sum = Σ x_i
    let mut running_sum = FixedPoint::zero();
    for i in 0..n as usize {
        running_sum = running_sum.checked_add(reserves[i])?;
    }

    // C = running_sum - k_cross · √n
    let target_sum = k_cross.checked_mul(sqrt_n)?;
    let c = running_sum.checked_sub(target_sum)?;

    // Quadratic: 2d² + 2(b+C-a)d + (2bC + C²) = 0
    // Coefficients (divided by 2):
    //   A_coeff = 1  (after dividing by 2)
    //   B_coeff = b + C - a
    //   C_coeff = (2bC + C²) / 2 = C(2b + C) / 2
    let b_coeff = b.checked_add(c)?.checked_sub(a)?;
    let two = FixedPoint::from_int(2);
    let c_coeff_numer = c.checked_mul(two.checked_mul(b)?.checked_add(c)?)?;
    let c_coeff = c_coeff_numer.checked_div(two)?;

    // Discriminant: B² - 4AC = b_coeff² - 4·1·c_coeff = b_coeff² - 4·c_coeff
    let discriminant = b_coeff.squared()?.checked_sub(
        FixedPoint::from_int(4).checked_mul(c_coeff)?,
    )?;

    // If discriminant < 0, boundary is geometrically unreachable from the
    // current sphere state. This can occur if the sphere radius changed
    // (e.g., after liquidity additions) since the tick was created.
    if discriminant.raw < 0 {
        msg!("compute_delta_to_boundary: negative discriminant ({}) for k_cross={}, boundary unreachable", discriminant.raw, k_cross.raw);
        return Ok(FixedPoint::zero());
    }

    let sqrt_disc = discriminant.sqrt()?;

    // Two roots: d = (-b_coeff ± sqrt_disc) / 2
    let neg_b = FixedPoint::zero().checked_sub(b_coeff)?;
    let root1 = neg_b.checked_add(sqrt_disc)?.checked_div(two)?;
    let root2 = neg_b.checked_sub(sqrt_disc)?.checked_div(two)?;

    // Select the smallest positive root
    let result = match (root1.raw > 0, root2.raw > 0) {
        (true, true) => {
            if root1.raw <= root2.raw {
                root1
            } else {
                root2
            }
        }
        (true, false) => root1,
        (false, true) => root2,
        (false, false) => {
            // No positive root — already past boundary or boundary unreachable
            msg!("compute_delta_to_boundary: no positive root for k_cross={}, roots=({}, {})", k_cross.raw, root1.raw, root2.raw);
            FixedPoint::zero()
        }
    };

    Ok(result)
}

// ══════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Test helpers ──

    fn make_sphere(r: i64, n: u8) -> Sphere {
        Sphere { radius: FixedPoint::from_int(r), n }
    }

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
        let sphere = make_sphere(100, 3);
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
        let sphere = make_sphere(100, 3);
        let n_fp = FixedPoint::from_int(3);
        let sqrt_n = n_fp.sqrt().unwrap();
        let alpha_diag = sphere.radius.checked_mul(sqrt_n).unwrap();
        let s = orthogonal_radius(&sphere, alpha_diag).unwrap();
        assert!(s.approx_eq(sphere.radius, FixedPoint::from_int(1)));
    }

    #[test]
    fn test_orthogonal_radius_at_equal_price_zero() {
        // At equal price point α = r(√n - 1), offset = -r, s = √(r² - r²) = 0
        let sphere = make_sphere(100, 3);
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
        let sphere = make_sphere(100, 3);
        // Alpha somewhere in the valid range
        let alpha = FixedPoint::from_int(150);
        let s = orthogonal_radius(&sphere, alpha).unwrap();
        assert!(s.is_positive());
    }

    #[test]
    fn test_orthogonal_radius_rejects_large_negative_radicand() {
        // Alpha far outside valid range → large negative radicand → error
        let sphere = make_sphere(100, 3);
        let result = orthogonal_radius(&sphere, FixedPoint::zero());
        assert!(result.is_err(), "Large negative radicand should be rejected");
    }

    #[test]
    fn test_orthogonal_radius_clamps_tiny_negative() {
        // At the boundary α = r(√n - 1), radicand is ~0 but may be slightly
        // negative from fixed-point rounding → should clamp to 0, not error.
        let sphere = make_sphere(100, 3);
        let sqrt_n = FixedPoint::from_int(3).sqrt().unwrap();
        let alpha_boundary = sphere.radius
            .checked_mul(sqrt_n.checked_sub(FixedPoint::one()).unwrap())
            .unwrap();
        let s = orthogonal_radius(&sphere, alpha_boundary).unwrap();
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

    // ── find_nearest_tick_boundaries ──

    #[test]
    fn test_find_nearest_empty_ticks() {
        let data = find_nearest_tick_boundaries(&[], FixedPoint::from_int(100));
        assert!(!data.has_boundary);
        assert!(data.nearest_k_lower.is_none());
        assert!(data.nearest_k_upper.is_none());
    }

    #[test]
    fn test_find_nearest_interior_below() {
        let ticks = vec![
            (FixedPoint::from_int(90), TickStatus::Interior),
            (FixedPoint::from_int(80), TickStatus::Interior),
        ];
        let data = find_nearest_tick_boundaries(&ticks, FixedPoint::from_int(100));
        assert_eq!(data.nearest_k_lower.unwrap().raw, FixedPoint::from_int(90).raw);
        assert!(data.nearest_k_upper.is_none());
    }

    #[test]
    fn test_find_nearest_boundary_above() {
        let ticks = vec![
            (FixedPoint::from_int(110), TickStatus::Boundary),
            (FixedPoint::from_int(120), TickStatus::Boundary),
        ];
        let data = find_nearest_tick_boundaries(&ticks, FixedPoint::from_int(100));
        assert!(data.has_boundary);
        assert!(data.nearest_k_lower.is_none());
        assert_eq!(data.nearest_k_upper.unwrap().raw, FixedPoint::from_int(110).raw);
    }

    #[test]
    fn test_find_nearest_mixed_ticks() {
        let ticks = vec![
            (FixedPoint::from_int(85), TickStatus::Interior),
            (FixedPoint::from_int(95), TickStatus::Interior),
            (FixedPoint::from_int(105), TickStatus::Boundary),
            (FixedPoint::from_int(115), TickStatus::Boundary),
        ];
        let data = find_nearest_tick_boundaries(&ticks, FixedPoint::from_int(100));
        assert!(data.has_boundary);
        assert_eq!(data.nearest_k_lower.unwrap().raw, FixedPoint::from_int(95).raw);
        assert_eq!(data.nearest_k_upper.unwrap().raw, FixedPoint::from_int(105).raw);
    }

    // ── compute_delta_to_boundary ──

    #[test]
    fn test_delta_to_boundary_returns_positive() {
        // 3-asset pool with asymmetric reserves: [10, 56, 42]
        // r = 100, a = r-x_in = 90, b = r-x_out = 44 (asymmetric → positive discriminant)
        // sum = 108, alpha = 108/√3 ≈ 62.35, k_cross = 60 (below alpha → reachable)
        //
        // Positive discriminant requires (a-b)² > C·(2(a+b) + C).
        // With a=90, b=44: (a-b)²=2116 > C·(2·134 + C) ≈ 1093 ✓
        let sphere = make_sphere(100, 3);
        let reserves = [
            FixedPoint::from_int(10),
            FixedPoint::from_int(56),
            FixedPoint::from_int(42),
        ];
        let k_cross = FixedPoint::from_int(60);
        let delta = compute_delta_to_boundary(&sphere, &reserves, 0, 1, k_cross, 3).unwrap();
        // Should be positive (some amount needed to reach boundary)
        assert!(delta.raw > 0, "delta should be positive, got {}", delta);
    }

    #[test]
    fn test_delta_to_boundary_zero_when_at_boundary() {
        // When current alpha is already at k_cross, delta should be ~0
        let sphere = make_sphere(100, 3);
        let reserves = [
            FixedPoint::from_int(100),
            FixedPoint::from_int(100),
            FixedPoint::from_int(100),
        ];
        let n_fp = FixedPoint::from_int(3);
        let sqrt_n = n_fp.sqrt().unwrap();
        // current alpha = 300 / √3
        let current_alpha = FixedPoint::from_int(300).checked_div(sqrt_n).unwrap();
        let delta = compute_delta_to_boundary(&sphere, &reserves, 0, 1, current_alpha, 3).unwrap();
        // Should be approximately zero
        assert!(
            delta.raw.abs() < FixedPoint::from_int(1).raw,
            "delta should be ≈0 when at boundary, got {}",
            delta
        );
    }
}
