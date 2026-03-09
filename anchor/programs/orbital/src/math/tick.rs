//! Tick Value Object
//!
//! A tick defines a spherical cap on the Orbital AMM sphere, bounded by
//! the hyperplane x⃗ · v⃗ = k, where v⃗ = (1,1,...,1)/√n.
//!
//! Each tick represents a concentrated liquidity region with computed
//! geometric bounds (x_min, x_max), depeg price, capital efficiency,
//! and boundary sphere radius.
//!
//! Immutable after creation — the on-chain storage counterpart is
//! `TickState` in `state/tick.rs`.

use anchor_lang::prelude::*;

use super::{FixedPoint, Sphere};
use crate::state::TickStatus;

/// Tick: spherical cap defined by plane constant k.
///
/// The hyperplane x⃗ · v⃗ = k (where v⃗ = (1,...,1)/√n) intersects the
/// sphere to form a spherical cap. This struct holds all derived
/// geometric properties computed from (k, Sphere).
#[derive(Clone, Copy)]
pub struct Tick {
    /// Plane constant defining the tick boundary: x⃗ · v⃗ = k
    pub k: FixedPoint,
    /// Number of assets (cached from Sphere)
    pub n: u8,
    /// Tick status: Interior or Boundary
    pub status: TickStatus,
    /// Minimum single-asset reserve within this tick
    pub x_min: FixedPoint,
    /// Maximum single-asset reserve within this tick (≤ r)
    pub x_max: FixedPoint,
    /// Depeg price at maximum reserve imbalance
    pub depeg_price: FixedPoint,
    /// Capital efficiency: x_base / (x_base - x_min)
    pub capital_efficiency: FixedPoint,
    /// Boundary sphere radius: s = √(r² - (k - r√n)²)
    pub boundary_sphere_radius: FixedPoint,
}

impl Tick {
    /// Create a new Tick from plane constant k and parent Sphere.
    ///
    /// Validates: k_min < k < k_max (strict inequality)
    /// where:
    ///   k_min = r · (√n - 1)
    ///   k_max = r · (n - 1) / √n
    ///
    /// Computes all derived values: x_min, x_max, depeg_price,
    /// capital_efficiency, boundary_sphere_radius.
    pub fn new(k: FixedPoint, sphere: &Sphere) -> Result<Self> {
        let r = sphere.radius;
        let n = sphere.n;
        let n_fp = FixedPoint::from_int(n as i64);
        let sqrt_n = n_fp.sqrt()?;
        let one = FixedPoint::one();

        // Step 1: Compute and validate k bounds
        let k_min_val = r.checked_mul(sqrt_n.checked_sub(one)?)?;
        let n_minus_1 = n_fp.checked_sub(one)?;
        let k_max_val = r.checked_mul(n_minus_1)?.checked_div(sqrt_n)?;

        require!(
            k.raw > k_min_val.raw && k.raw < k_max_val.raw,
            crate::errors::OrbitalError::InvalidTickBound
        );

        // Step 2: Compute shared discriminant D (used by x_min, x_max, depeg_price)
        let d = Self::compute_discriminant(k, r, n_fp, sqrt_n)?;

        // Step 3: Compute derived values
        let x_min = Self::compute_x_min_from_parts(k, n_fp, sqrt_n, d)?;
        let x_max = Self::compute_x_max_from_parts(k, r, n_fp, sqrt_n, d)?;
        let depeg_price = Self::compute_depeg_price_from_parts(k, r, n_fp, sqrt_n, d)?;
        let capital_efficiency = Self::compute_capital_efficiency(sphere, x_min)?;
        let boundary_sphere_radius = Self::compute_boundary_radius(r, k, sqrt_n)?;

        Ok(Self {
            k,
            n,
            status: TickStatus::Interior,
            x_min,
            x_max,
            depeg_price,
            capital_efficiency,
            boundary_sphere_radius,
        })
    }

    // ── Public Static Methods ──

    /// Minimum valid k for a given sphere: k_min = r · (√n - 1)
    pub fn k_min(sphere: &Sphere) -> Result<FixedPoint> {
        let n_fp = FixedPoint::from_int(sphere.n as i64);
        let sqrt_n = n_fp.sqrt()?;
        sphere.radius.checked_mul(sqrt_n.checked_sub(FixedPoint::one())?)
    }

    /// Maximum valid k for a given sphere: k_max = r · (n - 1) / √n
    pub fn k_max(sphere: &Sphere) -> Result<FixedPoint> {
        let n_fp = FixedPoint::from_int(sphere.n as i64);
        let sqrt_n = n_fp.sqrt()?;
        let n_minus_1 = n_fp.checked_sub(FixedPoint::one())?;
        sphere.radius.checked_mul(n_minus_1)?.checked_div(sqrt_n)
    }

    /// Compute x_min for a given (k, sphere) without constructing a full Tick.
    ///
    /// x_min = (k·√n - D) / n
    pub fn compute_x_min(k: FixedPoint, sphere: &Sphere) -> Result<FixedPoint> {
        let n_fp = FixedPoint::from_int(sphere.n as i64);
        let sqrt_n = n_fp.sqrt()?;
        let d = Self::compute_discriminant(k, sphere.radius, n_fp, sqrt_n)?;
        Self::compute_x_min_from_parts(k, n_fp, sqrt_n, d)
    }

    /// Compute x_max for a given (k, sphere) without constructing a full Tick.
    ///
    /// x_max = min(r, (k·√n + D) / n)
    pub fn compute_x_max(k: FixedPoint, sphere: &Sphere) -> Result<FixedPoint> {
        let n_fp = FixedPoint::from_int(sphere.n as i64);
        let sqrt_n = n_fp.sqrt()?;
        let d = Self::compute_discriminant(k, sphere.radius, n_fp, sqrt_n)?;
        Self::compute_x_max_from_parts(k, sphere.radius, n_fp, sqrt_n, d)
    }

    // ── Private Computation Methods ──

    /// Shared discriminant: D = √(k²·n - n·((n-1)·r - k·√n)²)
    ///
    /// This intermediate value appears in x_min, x_max, and depeg_price.
    /// Pre-computed once in the constructor to avoid redundant sqrt calls.
    fn compute_discriminant(
        k: FixedPoint,
        r: FixedPoint,
        n_fp: FixedPoint,
        sqrt_n: FixedPoint,
    ) -> Result<FixedPoint> {
        let one = FixedPoint::one();
        let n_minus_1 = n_fp.checked_sub(one)?;

        // inner = (n-1)·r - k·√n
        let inner = n_minus_1.checked_mul(r)?
            .checked_sub(k.checked_mul(sqrt_n)?)?;

        // k²·n
        let k_sq_n = k.squared()?.checked_mul(n_fp)?;

        // n·inner²
        let n_inner_sq = n_fp.checked_mul(inner.squared()?)?;

        // radicand = k²·n - n·inner²
        let radicand = k_sq_n.checked_sub(n_inner_sq)?;

        // Clamp to zero for fixed-point edge cases near tick bounds
        let radicand_safe = radicand.max(FixedPoint::zero());
        radicand_safe.sqrt()
    }

    /// x_min = (k·√n - D) / n
    ///
    /// Minimum reserve any single asset can reach within this tick.
    fn compute_x_min_from_parts(
        k: FixedPoint,
        n_fp: FixedPoint,
        sqrt_n: FixedPoint,
        d: FixedPoint,
    ) -> Result<FixedPoint> {
        let k_sqrt_n = k.checked_mul(sqrt_n)?;
        k_sqrt_n.checked_sub(d)?.checked_div(n_fp)
    }

    /// x_max = min(r, (k·√n + D) / n)
    ///
    /// Maximum reserve any single asset can reach within this tick.
    /// Clamped to r because reserves cannot exceed the sphere radius.
    fn compute_x_max_from_parts(
        k: FixedPoint,
        r: FixedPoint,
        n_fp: FixedPoint,
        sqrt_n: FixedPoint,
        d: FixedPoint,
    ) -> Result<FixedPoint> {
        let k_sqrt_n = k.checked_mul(sqrt_n)?;
        let unclamped = k_sqrt_n.checked_add(d)?.checked_div(n_fp)?;
        Ok(unclamped.min(r))
    }

    /// Depeg price at maximum reserve imbalance.
    ///
    /// x_depeg = (k·√n + D) / n  (unclamped x_max)
    /// x_other = (k·√n - x_depeg) / (n - 1)
    /// p_depeg = (r - x_depeg) / (r - x_other)
    ///
    /// Represents the worst-case price ratio when one asset reaches
    /// its maximum reserve and all others are at minimum.
    fn compute_depeg_price_from_parts(
        k: FixedPoint,
        r: FixedPoint,
        n_fp: FixedPoint,
        sqrt_n: FixedPoint,
        d: FixedPoint,
    ) -> Result<FixedPoint> {
        let one = FixedPoint::one();
        let n_minus_1 = n_fp.checked_sub(one)?;
        let k_sqrt_n = k.checked_mul(sqrt_n)?;

        // x_depeg = min(r, (k·√n + D) / n)
        // Clamped to r: reserves cannot exceed sphere radius.
        // When x_depeg == r, depeg_price == 0 (complete depeg).
        let x_depeg = k_sqrt_n.checked_add(d)?.checked_div(n_fp)?.min(r);

        // x_other = (k·√n - x_depeg) / (n - 1)
        let x_other = k_sqrt_n.checked_sub(x_depeg)?.checked_div(n_minus_1)?;

        // p_depeg = (r - x_depeg) / (r - x_other)
        let numerator = r.checked_sub(x_depeg)?;
        let denominator = r.checked_sub(x_other)?;

        numerator.checked_div(denominator)
    }

    /// Capital efficiency: x_base / (x_base - x_min)
    ///
    /// Measures how efficiently LP capital is deployed for a given depeg tolerance.
    /// Higher values mean less capital needed for the same liquidity depth.
    /// x_base = r(1 - 1/√n) = Sphere::equal_price_point()
    fn compute_capital_efficiency(
        sphere: &Sphere,
        x_min: FixedPoint,
    ) -> Result<FixedPoint> {
        let x_base = sphere.equal_price_point()?;
        let denominator = x_base.checked_sub(x_min)?;

        require!(
            denominator.raw > 0,
            crate::errors::OrbitalError::InvalidTickBound
        );

        x_base.checked_div(denominator)
    }

    /// Boundary sphere radius: s = √(r² - (k - r·√n)²)
    ///
    /// When this tick transitions to Boundary status, it operates as an
    /// (n-1)-dimensional sphere with this radius in the orthogonal subspace.
    fn compute_boundary_radius(
        r: FixedPoint,
        k: FixedPoint,
        sqrt_n: FixedPoint,
    ) -> Result<FixedPoint> {
        let r_sq = r.squared()?;
        let offset = k.checked_sub(r.checked_mul(sqrt_n)?)?;
        let offset_sq = offset.squared()?;
        let radicand = r_sq.checked_sub(offset_sq)?;

        // Clamp for fixed-point edge cases
        let radicand_safe = radicand.max(FixedPoint::zero());
        radicand_safe.sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a Sphere with total_liquidity = n * r
    fn make_sphere(r: i64, n: u8) -> Sphere {
        Sphere {
            radius: FixedPoint::from_int(r),
            n,
        }
    }

    /// Tolerance for approx_eq comparisons (0.1%)
    fn tolerance(value: FixedPoint) -> FixedPoint {
        FixedPoint::from_raw(value.raw.abs() / 1000)
    }

    /// Compute a valid k at the midpoint between k_min and k_max
    fn midpoint_k(sphere: &Sphere) -> FixedPoint {
        let k_min = Tick::k_min(sphere).unwrap();
        let k_max = Tick::k_max(sphere).unwrap();
        let sum = k_min.checked_add(k_max).unwrap();
        FixedPoint::from_raw(sum.raw / 2)
    }

    // ══════════════════════════════════════════════
    // k-bounds validation tests
    // ══════════════════════════════════════════════

    #[test]
    fn test_valid_k_midpoint() {
        let sphere = make_sphere(200, 3);
        let k = midpoint_k(&sphere);
        let tick = Tick::new(k, &sphere);
        assert!(tick.is_ok(), "midpoint k should be valid");
    }

    #[test]
    fn test_k_below_k_min_rejected() {
        let sphere = make_sphere(200, 3);
        let k_min = Tick::k_min(&sphere).unwrap();
        let k = FixedPoint::from_raw(k_min.raw - FixedPoint::one().raw);
        assert!(Tick::new(k, &sphere).is_err());
    }

    #[test]
    fn test_k_above_k_max_rejected() {
        let sphere = make_sphere(200, 3);
        let k_max = Tick::k_max(&sphere).unwrap();
        let k = FixedPoint::from_raw(k_max.raw + FixedPoint::one().raw);
        assert!(Tick::new(k, &sphere).is_err());
    }

    #[test]
    fn test_k_at_k_min_rejected() {
        let sphere = make_sphere(200, 3);
        let k_min = Tick::k_min(&sphere).unwrap();
        assert!(Tick::new(k_min, &sphere).is_err(), "k == k_min should be rejected (strict inequality)");
    }

    #[test]
    fn test_k_at_k_max_rejected() {
        let sphere = make_sphere(200, 3);
        let k_max = Tick::k_max(&sphere).unwrap();
        assert!(Tick::new(k_max, &sphere).is_err(), "k == k_max should be rejected (strict inequality)");
    }

    // ══════════════════════════════════════════════
    // Derived value correctness (n=3, r=200, k=190)
    // ══════════════════════════════════════════════

    fn reference_tick() -> (Tick, Sphere) {
        let sphere = make_sphere(200, 3);
        let k = FixedPoint::from_int(190);
        let tick = Tick::new(k, &sphere).unwrap();
        (tick, sphere)
    }

    #[test]
    fn test_x_min_positive() {
        let (tick, _) = reference_tick();
        assert!(tick.x_min.raw > 0, "x_min should be positive, got {:?}", tick.x_min);
    }

    #[test]
    fn test_x_max_le_r() {
        let (tick, sphere) = reference_tick();
        assert!(
            tick.x_max.raw <= sphere.radius.raw,
            "x_max ({:?}) should be <= r ({:?})",
            tick.x_max,
            sphere.radius
        );
    }

    #[test]
    fn test_depeg_price_in_valid_range() {
        let (tick, _) = reference_tick();
        assert!(
            tick.depeg_price.raw >= 0,
            "depeg_price ({:?}) should be >= 0",
            tick.depeg_price
        );
        assert!(
            tick.depeg_price.raw < FixedPoint::one().raw,
            "depeg_price ({:?}) should be < 1.0",
            tick.depeg_price
        );
    }

    #[test]
    fn test_capital_efficiency_greater_than_one() {
        let (tick, _) = reference_tick();
        assert!(
            tick.capital_efficiency.raw > FixedPoint::one().raw,
            "capital_efficiency ({:?}) should be > 1.0",
            tick.capital_efficiency
        );
    }

    #[test]
    fn test_boundary_sphere_radius_positive() {
        let (tick, _) = reference_tick();
        assert!(
            tick.boundary_sphere_radius.raw > 0,
            "boundary_sphere_radius should be positive"
        );
    }

    #[test]
    fn test_pythagorean_identity() {
        // s² + (k - r·√n)² ≈ r²
        let (tick, sphere) = reference_tick();
        let r = sphere.radius;
        let n_fp = FixedPoint::from_int(sphere.n as i64);
        let sqrt_n = n_fp.sqrt().unwrap();

        let s_sq = tick.boundary_sphere_radius.squared().unwrap();
        let offset = tick.k.checked_sub(r.checked_mul(sqrt_n).unwrap()).unwrap();
        let offset_sq = offset.squared().unwrap();
        let sum = s_sq.checked_add(offset_sq).unwrap();
        let r_sq = r.squared().unwrap();

        let eps = tolerance(r_sq);
        assert!(
            sum.approx_eq(r_sq, eps),
            "s² + (k - r√n)² = {:?} should ≈ r² = {:?}",
            sum, r_sq
        );
    }

    // ══════════════════════════════════════════════
    // Mathematical invariant tests
    // ══════════════════════════════════════════════

    #[test]
    fn test_x_min_less_than_x_max() {
        let (tick, _) = reference_tick();
        assert!(
            tick.x_min.raw < tick.x_max.raw,
            "x_min ({:?}) should be < x_max ({:?})",
            tick.x_min, tick.x_max
        );
    }

    #[test]
    fn test_tick_spans_equal_price_point() {
        let (tick, sphere) = reference_tick();
        let q = sphere.equal_price_point().unwrap();
        assert!(
            tick.x_min.raw < q.raw && q.raw < tick.x_max.raw,
            "x_min ({:?}) < q ({:?}) < x_max ({:?}) should hold",
            tick.x_min, q, tick.x_max
        );
    }

    #[test]
    fn test_higher_k_lower_efficiency() {
        // Higher k → wider spherical cap → smaller x_min → LOWER efficiency.
        // Lower k → narrower cap → larger x_min (closer to x_base) → HIGHER efficiency.
        let sphere = make_sphere(200, 3);
        let k_min = Tick::k_min(&sphere).unwrap();
        let k_max = Tick::k_max(&sphere).unwrap();

        let range = k_max.raw - k_min.raw;
        let k_low = FixedPoint::from_raw(k_min.raw + range / 4);
        let k_high = FixedPoint::from_raw(k_min.raw + range * 3 / 4);

        let tick_low = Tick::new(k_low, &sphere).unwrap();
        let tick_high = Tick::new(k_high, &sphere).unwrap();

        assert!(
            tick_low.capital_efficiency.raw > tick_high.capital_efficiency.raw,
            "lower k (k={:?}, eff={:?}) should have higher efficiency than higher k (k={:?}, eff={:?})",
            k_low, tick_low.capital_efficiency,
            k_high, tick_high.capital_efficiency
        );
    }

    #[test]
    fn test_x_min_x_max_midpoint_symmetry() {
        // (x_min + x_max) / 2 ≈ k·√n / n  (only when x_max is NOT clamped to r)
        // Use n=2, r=100 where clamping doesn't occur at midpoint k.
        let sphere = make_sphere(100, 2);
        let k = FixedPoint::from_int(56);
        let tick = Tick::new(k, &sphere).unwrap();

        // Verify no clamping: x_max < r
        assert!(tick.x_max.raw < sphere.radius.raw, "test requires unclamped x_max");

        let n_fp = FixedPoint::from_int(sphere.n as i64);
        let sqrt_n = n_fp.sqrt().unwrap();

        let midpoint = tick.x_min.checked_add(tick.x_max).unwrap();
        let midpoint_half = FixedPoint::from_raw(midpoint.raw / 2);

        let expected = tick.k.checked_mul(sqrt_n).unwrap().checked_div(n_fp).unwrap();

        let eps = tolerance(expected);
        assert!(
            midpoint_half.approx_eq(expected, eps),
            "midpoint ({:?}) should ≈ k·√n/n ({:?})",
            midpoint_half, expected
        );
    }

    // ══════════════════════════════════════════════
    // Edge case tests
    // ══════════════════════════════════════════════

    #[test]
    fn test_n2_minimum_assets() {
        let sphere = make_sphere(100, 2);
        let k = midpoint_k(&sphere);
        let tick = Tick::new(k, &sphere).unwrap();
        assert!(tick.x_min.raw < tick.x_max.raw);
        assert!(tick.capital_efficiency.raw > FixedPoint::one().raw);
    }

    #[test]
    fn test_n8_maximum_assets() {
        let sphere = make_sphere(100, 8);
        let k = midpoint_k(&sphere);
        let tick = Tick::new(k, &sphere).unwrap();
        assert!(tick.x_min.raw < tick.x_max.raw);
        assert!(tick.capital_efficiency.raw > FixedPoint::one().raw);
    }

    #[test]
    fn test_k_near_k_min_narrow_tick() {
        let sphere = make_sphere(200, 3);
        let k_min = Tick::k_min(&sphere).unwrap();
        let k_max = Tick::k_max(&sphere).unwrap();
        let range = k_max.raw - k_min.raw;

        // k just 5% above k_min
        let k = FixedPoint::from_raw(k_min.raw + range / 20);
        let tick = Tick::new(k, &sphere).unwrap();

        // Narrow tick: x_min close to x_max, high efficiency
        assert!(tick.x_min.raw < tick.x_max.raw);
        assert!(tick.capital_efficiency.raw > FixedPoint::one().raw);
    }

    // ══════════════════════════════════════════════
    // Public static method tests
    // ══════════════════════════════════════════════

    #[test]
    fn test_k_min_k_max_manual() {
        // n=3, r=200: k_min = 200*(√3-1) ≈ 146.41, k_max = 200*2/√3 ≈ 230.94
        let sphere = make_sphere(200, 3);
        let k_min = Tick::k_min(&sphere).unwrap();
        let k_max = Tick::k_max(&sphere).unwrap();

        // Verify k_min ≈ 146.41 (within 1%)
        let expected_k_min = FixedPoint::from_int(146);
        let eps = FixedPoint::from_int(2);
        assert!(
            k_min.approx_eq(expected_k_min, eps),
            "k_min ({:?}) should ≈ 146",
            k_min
        );

        // Verify k_max ≈ 230.94 (within 1%)
        let expected_k_max = FixedPoint::from_int(231);
        assert!(
            k_max.approx_eq(expected_k_max, eps),
            "k_max ({:?}) should ≈ 231",
            k_max
        );

        // k_min < k_max
        assert!(k_min.raw < k_max.raw);
    }

    #[test]
    fn test_compute_x_min_standalone_matches_constructor() {
        let sphere = make_sphere(200, 3);
        let k = FixedPoint::from_int(190);
        let tick = Tick::new(k, &sphere).unwrap();
        let standalone = Tick::compute_x_min(k, &sphere).unwrap();
        assert_eq!(tick.x_min.raw, standalone.raw);
    }

    #[test]
    fn test_compute_x_max_standalone_matches_constructor() {
        let sphere = make_sphere(200, 3);
        let k = FixedPoint::from_int(190);
        let tick = Tick::new(k, &sphere).unwrap();
        let standalone = Tick::compute_x_max(k, &sphere).unwrap();
        assert_eq!(tick.x_max.raw, standalone.raw);
    }

    // ══════════════════════════════════════════════
    // Status test
    // ══════════════════════════════════════════════

    #[test]
    fn test_new_tick_is_interior() {
        let sphere = make_sphere(200, 3);
        let k = midpoint_k(&sphere);
        let tick = Tick::new(k, &sphere).unwrap();
        assert_eq!(tick.status, TickStatus::Interior);
    }
}
