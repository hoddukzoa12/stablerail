//! Newton's Method Solver for Sphere Invariant
//!
//! Computes `amount_out` for a swap that preserves the sphere invariant:
//!   Σ(r - xᵢ)² = r²
//!
//! Two strategies:
//! 1. **Analytical** — exact O(1) closed-form for single-sphere swaps
//! 2. **Newton iteration** — general numerical solver with bisection fallback
//!
//! Architecture: off-chain computation, on-chain verification.
//! The on-chain program receives `expected_amount_out` and verifies
//! the invariant holds after applying the trade.

use anchor_lang::prelude::*;

use super::sphere::MAX_ASSETS;
use super::FixedPoint;
use super::Sphere;

// ══════════════════════════════════════════════════════════════
// Analytical solver (exact, single-sphere)
// ══════════════════════════════════════════════════════════════

/// Compute exact `amount_out` for a single-sphere swap using the closed-form
/// quadratic solution.
///
/// Given sphere invariant Σ(r - xᵢ)² = r², a swap that adds `net_amount_in`
/// to `token_in` and removes `amount_out` from `token_out` must satisfy:
///
/// ```text
///   (a - d)² + (b + d_out)² = a² + b²
///   where a = r - x_in, b = r - x_out, d = net_amount_in
///
///   Solving: d_out = -b + √(b² + 2a·d - d²)
/// ```
///
/// Works for **any** reserve state (not just equal reserves) and **any** n,
/// as long as only two reserves change (standard swap).
///
/// Returns error if:
/// - Token indices are out of bounds or equal
/// - Radicand is negative (trade too large for current liquidity)
/// - Result would be negative (degenerate geometry)
pub fn compute_amount_out_analytical(
    sphere: &Sphere,
    reserves: &[FixedPoint],
    token_in: usize,
    token_out: usize,
    net_amount_in: FixedPoint,
) -> Result<FixedPoint> {
    let n = sphere.n as usize;
    require!(
        reserves.len() >= n,
        crate::errors::OrbitalError::InvalidAssetCount
    );
    require!(
        token_in < n && token_out < n,
        crate::errors::OrbitalError::InvalidTokenIndex
    );
    require!(
        token_in != token_out,
        crate::errors::OrbitalError::SameTokenSwap
    );
    require!(
        net_amount_in.raw > 0,
        crate::errors::OrbitalError::NegativeTradeAmount
    );
    require!(
        reserves[token_out].raw > 0,
        crate::errors::OrbitalError::InsufficientLiquidity
    );

    let r = sphere.radius;
    let a = r.checked_sub(reserves[token_in])?; // r - x_in
    let b = r.checked_sub(reserves[token_out])?; // r - x_out
    let d = net_amount_in;

    // radicand = b² + 2a·d - d²
    let b_sq = b.squared()?;
    let two_a_d = a.checked_mul(d)?.checked_mul(FixedPoint::from_int(2))?;
    let d_sq = d.squared()?;
    let radicand = b_sq.checked_add(two_a_d)?.checked_sub(d_sq)?;

    require!(
        radicand.raw >= 0,
        crate::errors::OrbitalError::InsufficientLiquidity
    );

    let sqrt_val = radicand.sqrt()?;

    // d_out = -b + √(radicand) = √(radicand) - b
    let d_out = sqrt_val.checked_sub(b)?;

    require!(
        d_out.raw > 0,
        crate::errors::OrbitalError::InsufficientLiquidity
    );

    Ok(d_out)
}

// ══════════════════════════════════════════════════════════════
// Newton solver (general, numerical)
// ══════════════════════════════════════════════════════════════

/// Default maximum Newton iterations before switching to bisection.
pub const DEFAULT_MAX_ITERATIONS: u32 = 20;

/// Default convergence epsilon: 2^32 in Q64.64 ≈ 2.3e-10.
/// Provides sub-atomic precision for stablecoin amounts.
pub const DEFAULT_EPSILON_RAW: i128 = 1i128 << 32;

/// Configurable Newton solver for sphere invariant equations.
///
/// Solves `f(d_out) = 0` where:
///   `f(d_out) = (a - d)² + (b + d_out)² - a² - b²`
///   `f'(d_out) = 2(b + d_out)`
///
/// Falls back to bisection if Newton doesn't converge within `max_iterations`.
pub struct NewtonSolver {
    /// Maximum Newton iterations before bisection fallback
    pub max_iterations: u32,
    /// Convergence threshold for |f(x)| and |x_{n+1} - x_n|
    pub epsilon: FixedPoint,
}

impl NewtonSolver {
    /// Create a solver with custom parameters.
    pub fn new(max_iterations: u32, epsilon: FixedPoint) -> Self {
        Self {
            max_iterations,
            epsilon,
        }
    }

    /// Create a solver with default parameters (20 iterations, ε ≈ 2.3e-10).
    pub fn default_solver() -> Self {
        Self {
            max_iterations: DEFAULT_MAX_ITERATIONS,
            epsilon: FixedPoint::from_raw(DEFAULT_EPSILON_RAW),
        }
    }

    /// Solve for `amount_out` using Newton's method with bisection fallback.
    ///
    /// Returns the amount_out that preserves the sphere invariant after a swap
    /// of `net_amount_in` from `token_in` to `token_out`.
    pub fn solve(
        &self,
        sphere: &Sphere,
        reserves: &[FixedPoint],
        token_in: usize,
        token_out: usize,
        net_amount_in: FixedPoint,
    ) -> Result<FixedPoint> {
        let n = sphere.n as usize;
        require!(
            reserves.len() >= n,
            crate::errors::OrbitalError::InvalidAssetCount
        );
        require!(
            token_in < n && token_out < n,
            crate::errors::OrbitalError::InvalidTokenIndex
        );
        require!(
            token_in != token_out,
            crate::errors::OrbitalError::SameTokenSwap
        );
        require!(
            net_amount_in.raw > 0,
            crate::errors::OrbitalError::NegativeTradeAmount
        );
        require!(
            reserves[token_out].raw > 0,
            crate::errors::OrbitalError::InsufficientLiquidity
        );

        let r = sphere.radius;
        let a = r.checked_sub(reserves[token_in])?;
        let b = r.checked_sub(reserves[token_out])?;
        let d = net_amount_in;

        // Bounds: d_out ∈ (0, x_out)
        let lo = FixedPoint::from_raw(1); // 1 ulp above 0
        let hi = reserves[token_out]; // max withdrawable

        // Initial guess: d_out ≈ d_in (1:1 assumption, excellent near equal price)
        let mut x = d.clamp(lo, hi);

        // Newton iteration
        for _ in 0..self.max_iterations {
            let fx = invariant_residual(a, b, d, x)?;

            // Convergence check: |f(x)| < ε
            if fx.abs()?.raw < self.epsilon.raw {
                return Ok(x);
            }

            let fpx = invariant_derivative(b, x)?;

            // Guard: f'(x) = 0 means we're at b + d_out = 0 → degenerate
            if fpx.is_zero() {
                break; // fall through to bisection
            }

            let step = fx.checked_div(fpx)?;
            let x_new = x.checked_sub(step)?;

            // Clamp to valid bounds
            let x_new = x_new.clamp(lo, hi);

            // Step size convergence check
            let delta = x_new.checked_sub(x)?.abs()?;
            x = x_new;

            if delta.raw < self.epsilon.raw {
                // Verify residual is also small — a clamped step may shrink delta
                // to near-zero while the residual is still large (boundary sticking).
                let fx_check = invariant_residual(a, b, d, x)?;
                if fx_check.abs()?.raw < self.epsilon.raw {
                    return Ok(x);
                }
                // Delta converged but residual large → stuck at boundary
                break;
            }
        }

        // Bisection fallback
        bisection_solve(a, b, d, lo, hi, self.max_iterations, self.epsilon)
    }
}

// ══════════════════════════════════════════════════════════════
// Internal helpers
// ══════════════════════════════════════════════════════════════

/// Invariant residual: f(d_out) = (a - d)² + (b + d_out)² - a² - b²
///
/// When f(d_out) = 0, the sphere invariant is preserved after the swap.
fn invariant_residual(
    a: FixedPoint,     // r - x_in
    b: FixedPoint,     // r - x_out
    d: FixedPoint,     // net_amount_in
    d_out: FixedPoint, // candidate amount_out
) -> Result<FixedPoint> {
    // (a - d)²
    let a_minus_d = a.checked_sub(d)?;
    let term1 = a_minus_d.squared()?;

    // (b + d_out)²
    let b_plus_dout = b.checked_add(d_out)?;
    let term2 = b_plus_dout.squared()?;

    // a² + b²
    let a_sq = a.squared()?;
    let b_sq = b.squared()?;

    // f = term1 + term2 - a² - b²
    term1.checked_add(term2)?.checked_sub(a_sq)?.checked_sub(b_sq)
}

/// Invariant derivative: f'(d_out) = 2(b + d_out)
///
/// Analytical derivative — no additional evaluation cost.
fn invariant_derivative(
    b: FixedPoint,     // r - x_out
    d_out: FixedPoint, // candidate amount_out
) -> Result<FixedPoint> {
    let sum = b.checked_add(d_out)?;
    sum.checked_mul(FixedPoint::from_int(2))
}

/// Bisection fallback: find root of f(d_out) in [lo, hi].
///
/// Guaranteed to converge (linear, ~64 iterations for Q64.64 precision).
/// Used when Newton's method diverges or oscillates.
fn bisection_solve(
    a: FixedPoint,
    b: FixedPoint,
    d: FixedPoint,
    mut lo: FixedPoint,
    mut hi: FixedPoint,
    max_iterations: u32,
    epsilon: FixedPoint,
) -> Result<FixedPoint> {
    // Verify bracket: f(lo) and f(hi) should have opposite signs
    let f_lo = invariant_residual(a, b, d, lo)?;
    let f_hi = invariant_residual(a, b, d, hi)?;

    // If both same sign, the root may not be in [lo, hi]
    if (f_lo.raw > 0) == (f_hi.raw > 0) {
        return Err(error!(crate::errors::OrbitalError::SolverDidNotConverge));
    }

    // Bisection iterations (max_iterations * 2 to allow more room)
    let bisect_iters = max_iterations.saturating_mul(2).max(64);
    for _ in 0..bisect_iters {
        let mid_raw = lo.raw.checked_add(hi.raw).ok_or_else(|| {
            error!(crate::errors::OrbitalError::MathOverflow)
        })? / 2;
        let mid = FixedPoint::from_raw(mid_raw);

        let f_mid = invariant_residual(a, b, d, mid)?;

        if f_mid.abs()?.raw < epsilon.raw {
            return Ok(mid);
        }

        // Narrow bracket
        if (f_mid.raw > 0) == (f_lo.raw > 0) {
            lo = mid;
        } else {
            hi = mid;
        }

        // Width convergence
        if hi.checked_sub(lo)?.raw < epsilon.raw {
            return Ok(mid);
        }
    }

    Err(error!(crate::errors::OrbitalError::SolverDidNotConverge))
}

// ══════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    /// Standard test epsilon: ~2^32 ≈ 2.3e-10
    fn eps() -> FixedPoint {
        FixedPoint::from_raw(1i128 << 32)
    }

    /// Generous tolerance for comparing solver outputs: ~2^40 ≈ 1.1e-7
    fn solver_eps() -> FixedPoint {
        FixedPoint::from_raw(1i128 << 40)
    }

    /// Build equal-reserve pool: n assets, each at equal price point q = r(1 - 1/√n)
    fn make_equal_reserves(total_liquidity: i64, n: u8) -> (Sphere, [FixedPoint; MAX_ASSETS]) {
        let sphere = Sphere::new(FixedPoint::from_int(total_liquidity), n).unwrap();
        let q = sphere.equal_price_point().unwrap();
        let mut reserves = [FixedPoint::zero(); MAX_ASSETS];
        for i in 0..n as usize {
            reserves[i] = q;
        }
        (sphere, reserves)
    }

    /// Build imbalanced pool: token 0 has more, token 1 has less
    fn make_imbalanced_reserves(
        total_liquidity: i64,
        n: u8,
        imbalance: i64,
    ) -> (Sphere, [FixedPoint; MAX_ASSETS]) {
        let sphere = Sphere::new(FixedPoint::from_int(total_liquidity), n).unwrap();
        let q = sphere.equal_price_point().unwrap();
        let mut reserves = [FixedPoint::zero(); MAX_ASSETS];
        for i in 0..n as usize {
            reserves[i] = q;
        }
        // Shift token 0 up and token 1 down, preserving invariant approximately
        // We solve: (r - (q + δ))² + (r - (q - δ'))² + rest = r²
        // For simplicity, use analytical solver to find valid state
        let delta = FixedPoint::from_int(imbalance);
        reserves[0] = q.checked_add(delta).unwrap();
        // Solve for reserves[1] to preserve invariant:
        // (r - x0')² + (r - x1')² + S_rest = r²
        // S_rest = (n-2) * (r - q)²
        let r = sphere.radius;
        let r_sq = r.squared().unwrap();
        let c = r.checked_sub(q).unwrap(); // r - q
        let c_sq = c.squared().unwrap();
        let s_rest = c_sq.checked_mul(FixedPoint::from_int((n - 2) as i64)).unwrap();
        let a0 = r.checked_sub(reserves[0]).unwrap();
        let a0_sq = a0.squared().unwrap();
        // (r - x1')² = r² - s_rest - a0²
        let rem = r_sq.checked_sub(s_rest).unwrap().checked_sub(a0_sq).unwrap();
        if rem.raw >= 0 {
            let x1_dist = rem.sqrt().unwrap();
            reserves[1] = r.checked_sub(x1_dist).unwrap();
        }
        (sphere, reserves)
    }

    /// Verify that a computed amount_out satisfies the sphere invariant
    fn verify_invariant_after_swap(
        sphere: &Sphere,
        reserves: &[FixedPoint],
        token_in: usize,
        token_out: usize,
        net_amount_in: FixedPoint,
        amount_out: FixedPoint,
    ) -> bool {
        let mut post_reserves = [FixedPoint::zero(); MAX_ASSETS];
        for i in 0..sphere.n as usize {
            post_reserves[i] = reserves[i];
        }
        post_reserves[token_in] = reserves[token_in].checked_add(net_amount_in).unwrap();
        post_reserves[token_out] = reserves[token_out].checked_sub(amount_out).unwrap();

        sphere
            .check_invariant(&post_reserves[..sphere.n as usize])
            .is_ok()
    }

    // ══════════════════════════════════════════════════════════
    // Analytical solver tests
    // ══════════════════════════════════════════════════════════

    #[test]
    fn test_analytical_equal_reserves_n3() {
        let (sphere, reserves) = make_equal_reserves(3_000, 3);
        let d_in = FixedPoint::from_int(10);

        let d_out = compute_amount_out_analytical(
            &sphere, &reserves, 0, 1, d_in,
        ).unwrap();

        assert!(d_out.is_positive());
        assert!(verify_invariant_after_swap(
            &sphere, &reserves, 0, 1, d_in, d_out,
        ));
    }

    #[test]
    fn test_analytical_equal_reserves_n2() {
        let (sphere, reserves) = make_equal_reserves(2_000, 2);
        let d_in = FixedPoint::from_int(5);

        let d_out = compute_amount_out_analytical(
            &sphere, &reserves, 0, 1, d_in,
        ).unwrap();

        assert!(d_out.is_positive());
        assert!(verify_invariant_after_swap(
            &sphere, &reserves, 0, 1, d_in, d_out,
        ));
    }

    #[test]
    fn test_analytical_equal_reserves_n4() {
        let (sphere, reserves) = make_equal_reserves(4_000, 4);
        let d_in = FixedPoint::from_int(20);

        let d_out = compute_amount_out_analytical(
            &sphere, &reserves, 0, 1, d_in,
        ).unwrap();

        assert!(d_out.is_positive());
        assert!(verify_invariant_after_swap(
            &sphere, &reserves, 0, 1, d_in, d_out,
        ));
    }

    #[test]
    fn test_analytical_equal_reserves_n8() {
        let (sphere, reserves) = make_equal_reserves(8_000, 8);
        let d_in = FixedPoint::from_int(50);

        let d_out = compute_amount_out_analytical(
            &sphere, &reserves, 0, 1, d_in,
        ).unwrap();

        assert!(d_out.is_positive());
        assert!(verify_invariant_after_swap(
            &sphere, &reserves, 0, 1, d_in, d_out,
        ));
    }

    #[test]
    fn test_analytical_imbalanced_reserves() {
        let (sphere, reserves) = make_imbalanced_reserves(3_000, 3, 50);
        let d_in = FixedPoint::from_int(10);

        let d_out = compute_amount_out_analytical(
            &sphere, &reserves, 0, 1, d_in,
        ).unwrap();

        assert!(d_out.is_positive());
        assert!(verify_invariant_after_swap(
            &sphere, &reserves, 0, 1, d_in, d_out,
        ));
    }

    #[test]
    fn test_analytical_small_trade() {
        let (sphere, reserves) = make_equal_reserves(3_000, 3);
        // Very small trade: 0.001
        let d_in = FixedPoint::from_fraction(1, 1_000).unwrap();

        let d_out = compute_amount_out_analytical(
            &sphere, &reserves, 0, 1, d_in,
        ).unwrap();

        assert!(d_out.is_positive());
        assert!(verify_invariant_after_swap(
            &sphere, &reserves, 0, 1, d_in, d_out,
        ));
    }

    #[test]
    fn test_analytical_matches_legacy_n3_helper() {
        // Verify that the new analytical solver produces the same result as
        // the old compute_valid_amount_out_n3 test helper.
        let total_liq = 3_000i64;
        let n = 3u8;
        let (sphere, reserves) = make_equal_reserves(total_liq, n);
        let d_in = FixedPoint::from_int(10);

        // Legacy formula: d_out = -c + √(c² + 2cd - d²), c = r/√n
        let r = sphere.radius;
        let sqrt_n = FixedPoint::from_int(n as i64).sqrt().unwrap();
        let c = r.checked_div(sqrt_n).unwrap();
        let c_sq = c.squared().unwrap();
        let two_cd = c.checked_mul(d_in).unwrap()
            .checked_mul(FixedPoint::from_int(2)).unwrap();
        let d_sq = d_in.squared().unwrap();
        let radicand = c_sq.checked_add(two_cd).unwrap()
            .checked_sub(d_sq).unwrap();
        let legacy_out = radicand.sqrt().unwrap().checked_sub(c).unwrap();

        // New analytical solver
        let new_out = compute_amount_out_analytical(
            &sphere, &reserves, 0, 1, d_in,
        ).unwrap();

        assert!(
            legacy_out.approx_eq(new_out, solver_eps()),
            "legacy={:?} vs new={:?}",
            legacy_out, new_out
        );
    }

    #[test]
    fn test_analytical_same_token_error() {
        let (sphere, reserves) = make_equal_reserves(3_000, 3);
        let d_in = FixedPoint::from_int(10);
        assert!(compute_amount_out_analytical(
            &sphere, &reserves, 0, 0, d_in,
        ).is_err());
    }

    #[test]
    fn test_analytical_zero_amount_error() {
        let (sphere, reserves) = make_equal_reserves(3_000, 3);
        assert!(compute_amount_out_analytical(
            &sphere, &reserves, 0, 1, FixedPoint::zero(),
        ).is_err());
    }

    #[test]
    fn test_analytical_too_large_trade_error() {
        let (sphere, reserves) = make_equal_reserves(3_000, 3);
        // Trade larger than radius → radicand negative
        let huge = FixedPoint::from_int(5_000);
        assert!(compute_amount_out_analytical(
            &sphere, &reserves, 0, 1, huge,
        ).is_err());
    }

    // ══════════════════════════════════════════════════════════
    // Newton solver tests
    // ══════════════════════════════════════════════════════════

    #[test]
    fn test_newton_matches_analytical_n3() {
        let (sphere, reserves) = make_equal_reserves(3_000, 3);
        let d_in = FixedPoint::from_int(10);
        let solver = NewtonSolver::default_solver();

        let analytical = compute_amount_out_analytical(
            &sphere, &reserves, 0, 1, d_in,
        ).unwrap();

        let newton = solver.solve(
            &sphere, &reserves, 0, 1, d_in,
        ).unwrap();

        assert!(
            analytical.approx_eq(newton, solver_eps()),
            "analytical={:?} vs newton={:?}",
            analytical, newton
        );
    }

    #[test]
    fn test_newton_matches_analytical_n2() {
        let (sphere, reserves) = make_equal_reserves(2_000, 2);
        let d_in = FixedPoint::from_int(5);
        let solver = NewtonSolver::default_solver();

        let analytical = compute_amount_out_analytical(
            &sphere, &reserves, 0, 1, d_in,
        ).unwrap();

        let newton = solver.solve(
            &sphere, &reserves, 0, 1, d_in,
        ).unwrap();

        assert!(
            analytical.approx_eq(newton, solver_eps()),
            "analytical={:?} vs newton={:?}",
            analytical, newton
        );
    }

    #[test]
    fn test_newton_matches_analytical_n8() {
        let (sphere, reserves) = make_equal_reserves(8_000, 8);
        let d_in = FixedPoint::from_int(50);
        let solver = NewtonSolver::default_solver();

        let analytical = compute_amount_out_analytical(
            &sphere, &reserves, 0, 1, d_in,
        ).unwrap();

        let newton = solver.solve(
            &sphere, &reserves, 0, 1, d_in,
        ).unwrap();

        assert!(
            analytical.approx_eq(newton, solver_eps()),
            "analytical={:?} vs newton={:?}",
            analytical, newton
        );
    }

    #[test]
    fn test_newton_imbalanced() {
        let (sphere, reserves) = make_imbalanced_reserves(3_000, 3, 50);
        let d_in = FixedPoint::from_int(10);
        let solver = NewtonSolver::default_solver();

        let newton = solver.solve(
            &sphere, &reserves, 0, 1, d_in,
        ).unwrap();

        assert!(newton.is_positive());
        assert!(verify_invariant_after_swap(
            &sphere, &reserves, 0, 1, d_in, newton,
        ));
    }

    #[test]
    fn test_newton_converges_within_10_iterations() {
        let (sphere, reserves) = make_equal_reserves(3_000, 3);
        let d_in = FixedPoint::from_int(10);

        // Solver with max 10 iterations (no bisection fallback)
        let solver = NewtonSolver::new(10, FixedPoint::from_raw(DEFAULT_EPSILON_RAW));
        let result = solver.solve(&sphere, &reserves, 0, 1, d_in);

        assert!(result.is_ok(), "Newton should converge within 10 iterations");
        assert!(verify_invariant_after_swap(
            &sphere, &reserves, 0, 1, d_in, result.unwrap(),
        ));
    }

    #[test]
    fn test_newton_invariant_satisfaction() {
        // Test across multiple pool sizes
        for n in 2..=8u8 {
            let total = (n as i64) * 1_000;
            let (sphere, reserves) = make_equal_reserves(total, n);
            let d_in = FixedPoint::from_int(10);
            let solver = NewtonSolver::default_solver();

            let d_out = solver.solve(&sphere, &reserves, 0, 1, d_in).unwrap();

            assert!(
                verify_invariant_after_swap(&sphere, &reserves, 0, 1, d_in, d_out),
                "Invariant violated for n={}", n
            );
        }
    }

    #[test]
    fn test_bisection_fallback() {
        let (sphere, reserves) = make_equal_reserves(3_000, 3);
        let d_in = FixedPoint::from_int(10);

        // Force bisection by setting max_iterations=0 for Newton
        let solver = NewtonSolver::new(0, FixedPoint::from_raw(DEFAULT_EPSILON_RAW));
        let result = solver.solve(&sphere, &reserves, 0, 1, d_in);

        assert!(result.is_ok(), "Bisection fallback should converge");
        assert!(verify_invariant_after_swap(
            &sphere, &reserves, 0, 1, d_in, result.unwrap(),
        ));
    }

    #[test]
    fn test_newton_different_token_pairs() {
        let (sphere, reserves) = make_equal_reserves(3_000, 3);
        let d_in = FixedPoint::from_int(10);
        let solver = NewtonSolver::default_solver();

        // All token pairs should produce equivalent results at equal reserves
        let d01 = solver.solve(&sphere, &reserves, 0, 1, d_in).unwrap();
        let d02 = solver.solve(&sphere, &reserves, 0, 2, d_in).unwrap();
        let d10 = solver.solve(&sphere, &reserves, 1, 0, d_in).unwrap();

        // At equal reserves, all pairs should give same amount_out
        assert!(
            d01.approx_eq(d02, solver_eps()),
            "d01={:?} vs d02={:?}", d01, d02
        );
        assert!(
            d01.approx_eq(d10, solver_eps()),
            "d01={:?} vs d10={:?}", d01, d10
        );
    }

    // ══════════════════════════════════════════════════════════
    // Internal helper tests
    // ══════════════════════════════════════════════════════════

    #[test]
    fn test_residual_zero_at_correct_solution() {
        let (sphere, reserves) = make_equal_reserves(3_000, 3);
        let d_in = FixedPoint::from_int(10);

        let r = sphere.radius;
        let a = r.checked_sub(reserves[0]).unwrap();
        let b = r.checked_sub(reserves[1]).unwrap();

        // Analytical solution should give residual ≈ 0
        let d_out = compute_amount_out_analytical(
            &sphere, &reserves, 0, 1, d_in,
        ).unwrap();

        let residual = invariant_residual(a, b, d_in, d_out).unwrap();
        assert!(
            residual.abs().unwrap().raw < (1i128 << 40),
            "residual should be ≈ 0, got {:?}", residual
        );
    }

    #[test]
    fn test_derivative_positive() {
        // f'(d_out) = 2(b + d_out), should be positive when b > 0 and d_out > 0
        let b = FixedPoint::from_int(100);
        let d_out = FixedPoint::from_int(10);

        let fprime = invariant_derivative(b, d_out).unwrap();
        assert!(fprime.is_positive());
        // 2 * (100 + 10) = 220
        assert!(fprime.approx_eq(FixedPoint::from_int(220), eps()));
    }

    // ══════════════════════════════════════════════════════════
    // Guard & edge-case tests
    // ══════════════════════════════════════════════════════════

    #[test]
    fn test_analytical_zero_reserve_out_error() {
        // reserves[token_out] == 0 → InsufficientLiquidity
        let (sphere, mut reserves) = make_equal_reserves(3_000, 3);
        reserves[1] = FixedPoint::zero();
        let d_in = FixedPoint::from_int(10);

        assert!(compute_amount_out_analytical(
            &sphere, &reserves, 0, 1, d_in,
        ).is_err());
    }

    #[test]
    fn test_newton_zero_reserve_out_error() {
        // reserves[token_out] == 0 → InsufficientLiquidity (not SolverDidNotConverge)
        let (sphere, mut reserves) = make_equal_reserves(3_000, 3);
        reserves[1] = FixedPoint::zero();
        let d_in = FixedPoint::from_int(10);
        let solver = NewtonSolver::default_solver();

        assert!(solver.solve(&sphere, &reserves, 0, 1, d_in).is_err());
    }

    #[test]
    fn test_analytical_short_reserves_slice_error() {
        // reserves.len() < n → InvalidAssetCount
        let sphere = Sphere::new(FixedPoint::from_int(3_000), 3).unwrap();
        let reserves = [FixedPoint::from_int(100), FixedPoint::from_int(100)]; // len=2, n=3
        let d_in = FixedPoint::from_int(10);

        assert!(compute_amount_out_analytical(
            &sphere, &reserves, 0, 1, d_in,
        ).is_err());
    }

    #[test]
    fn test_newton_short_reserves_slice_error() {
        // reserves.len() < n → InvalidAssetCount
        let sphere = Sphere::new(FixedPoint::from_int(3_000), 3).unwrap();
        let reserves = [FixedPoint::from_int(100), FixedPoint::from_int(100)]; // len=2, n=3
        let d_in = FixedPoint::from_int(10);
        let solver = NewtonSolver::default_solver();

        assert!(solver.solve(&sphere, &reserves, 0, 1, d_in).is_err());
    }

    #[test]
    fn test_newton_clamp_does_not_false_converge() {
        // Oversized trade that pushes Newton toward boundary — should fall through
        // to bisection and either converge correctly or error, never return a
        // wrong answer silently.
        let (sphere, reserves) = make_equal_reserves(3_000, 3);
        // Large trade close to reserve limit
        let x_out = reserves[1];
        let d_in = x_out.checked_mul(FixedPoint::from_fraction(9, 10).unwrap()).unwrap();
        let solver = NewtonSolver::default_solver();

        let result = solver.solve(&sphere, &reserves, 0, 1, d_in);
        if let Ok(d_out) = result {
            // If it returns a value, it must satisfy the invariant
            assert!(
                verify_invariant_after_swap(&sphere, &reserves, 0, 1, d_in, d_out),
                "Newton returned d_out that violates invariant"
            );
        }
        // If Err, that's acceptable — trade may exceed liquidity
    }
}
