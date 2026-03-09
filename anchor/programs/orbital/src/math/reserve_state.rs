//! ReserveState Value Object
//!
//! Transient computation helper that wraps a reserve vector with cached
//! aggregate statistics (Σxᵢ, Σxᵢ²) for O(1) sphere geometry queries.
//!
//! NOT stored on-chain — constructed from PoolState fields at instruction start.
//! Caches enable O(1) alpha, w_norm_squared, and distance_squared computations
//! that would otherwise require O(n) loops on every call.

use anchor_lang::prelude::*;

use super::sphere::MAX_ASSETS;
use super::FixedPoint;
use super::Sphere;

/// Reserve vector with cached running sums for O(1) sphere geometry queries.
///
/// Invariants maintained:
/// - `running_sum == Σ amounts[i] for i in 0..n`
/// - `running_sq_sum == Σ amounts[i]² for i in 0..n`
#[derive(Clone, Copy)]
pub struct ReserveState {
    /// Current reserve amounts [x₁, x₂, ..., xₙ, 0, ..., 0]
    pub amounts: [FixedPoint; MAX_ASSETS],
    /// Number of active assets
    pub n: u8,
    /// Σxᵢ — cached for O(1) alpha and distance computations
    pub running_sum: FixedPoint,
    /// Σxᵢ² — cached for O(1) w_norm_squared and distance computations
    pub running_sq_sum: FixedPoint,
}

impl ReserveState {
    /// Construct from a fixed-size reserve array and asset count.
    /// Computes running_sum and running_sq_sum in O(n).
    pub fn new(amounts: &[FixedPoint; MAX_ASSETS], n: u8) -> Result<Self> {
        require!(
            n >= 2 && n as usize <= MAX_ASSETS,
            crate::errors::OrbitalError::InvalidAssetCount
        );

        let mut running_sum = FixedPoint::zero();
        let mut running_sq_sum = FixedPoint::zero();

        for &x_i in amounts.iter().take(n as usize) {
            running_sum = running_sum.checked_add(x_i)?;
            running_sq_sum = running_sq_sum.checked_add(x_i.squared()?)?;
        }

        Ok(Self {
            amounts: *amounts,
            n,
            running_sum,
            running_sq_sum,
        })
    }

    /// Slice of active reserve amounts (indices 0..n)
    pub fn active_amounts(&self) -> &[FixedPoint] {
        &self.amounts[..self.n as usize]
    }

    /// Bounds-checked access to a single reserve amount
    pub fn get(&self, index: usize) -> Result<FixedPoint> {
        require!(
            index < self.n as usize,
            crate::errors::OrbitalError::InvalidTokenIndex
        );
        Ok(self.amounts[index])
    }

    /// Parallel projection: α = Σxᵢ / √n
    ///
    /// The component of the reserve vector along the (1,1,...,1)/√n direction.
    /// Used for tick crossing detection (alpha changes monotonically during swaps).
    pub fn alpha(&self) -> Result<FixedPoint> {
        let n_fp = FixedPoint::from_int(self.n as i64);
        let sqrt_n = n_fp.sqrt()?;
        self.running_sum.checked_div(sqrt_n)
    }

    /// Orthogonal component norm squared: ||w||² = Σxᵢ² - (Σxᵢ)² / n
    ///
    /// Measures how far the reserve vector deviates from the equal-price diagonal.
    /// Zero when all reserves are equal; increases with imbalance.
    pub fn w_norm_squared(&self) -> Result<FixedPoint> {
        let n_fp = FixedPoint::from_int(self.n as i64);
        let sum_sq = self.running_sum.squared()?;
        let term = sum_sq.checked_div(n_fp)?;
        self.running_sq_sum.checked_sub(term)
    }

    /// O(1) distance squared from sphere center using cached sums.
    ///
    /// ||r⃗ - x⃗||² = Σ(r - xᵢ)² = n·r² - 2r·Σxᵢ + Σxᵢ²
    ///
    /// Algebraically equivalent to `Sphere::distance_squared()` but O(1) instead of O(n).
    /// Requires `self.n == sphere.n` to produce meaningful results.
    pub fn distance_squared_from_center(&self, sphere: &Sphere) -> Result<FixedPoint> {
        require!(
            self.n == sphere.n,
            crate::errors::OrbitalError::InvalidAssetCount
        );

        let n_fp = FixedPoint::from_int(self.n as i64);
        let r_sq = sphere.radius_squared()?;

        // n * r²
        let term1 = n_fp.checked_mul(r_sq)?;
        // 2r·Σxᵢ — computed as r·Σxᵢ + r·Σxᵢ to reduce intermediate magnitude
        let r_times_sum = sphere.radius.checked_mul(self.running_sum)?;
        let term2 = r_times_sum.checked_add(r_times_sum)?;
        // n·r² - 2r·Σxᵢ + Σxᵢ²
        term1.checked_sub(term2)?.checked_add(self.running_sq_sum)
    }

    /// Marginal price of token i in terms of token j.
    ///
    /// price(i, j) = dx_i/dx_j = (r - x_j) / (r - x_i)
    ///
    /// At equal reserves, price = 1.0 for all pairs.
    /// When x_i < x_j (token i is scarcer), price < 1.0 (less output per unit input).
    pub fn price(&self, i: usize, j: usize, sphere: &Sphere) -> Result<FixedPoint> {
        require!(
            self.n == sphere.n,
            crate::errors::OrbitalError::InvalidAssetCount
        );
        require!(
            i != j,
            crate::errors::OrbitalError::SameTokenSwap
        );
        require!(
            i < self.n as usize && j < self.n as usize,
            crate::errors::OrbitalError::InvalidTokenIndex
        );

        let x_i = self.amounts[i];
        let x_j = self.amounts[j];
        let numerator = sphere.radius.checked_sub(x_j)?;
        let denominator = sphere.radius.checked_sub(x_i)?;

        numerator.checked_div(denominator)
            .map_err(|_| error!(crate::errors::OrbitalError::DivisionByZero))
    }

    /// O(1) incremental update after a trade.
    ///
    /// Updates amounts, running_sum, and running_sq_sum without re-looping.
    /// Called during tick-segment trade execution for constant-time updates.
    ///
    /// All new values are computed into temporaries first and assigned atomically
    /// to prevent partial-update corruption if intermediate arithmetic fails.
    pub fn apply_trade(
        &mut self,
        token_in: usize,
        amount_in: FixedPoint,
        token_out: usize,
        amount_out: FixedPoint,
    ) -> Result<()> {
        require!(
            token_in < self.n as usize && token_out < self.n as usize,
            crate::errors::OrbitalError::InvalidTokenIndex
        );
        require!(
            token_in != token_out,
            crate::errors::OrbitalError::SameTokenSwap
        );
        require!(
            amount_in.raw >= 0 && amount_out.raw >= 0,
            crate::errors::OrbitalError::NegativeTradeAmount
        );

        let old_in = self.amounts[token_in];
        let old_out = self.amounts[token_out];

        let new_in = old_in.checked_add(amount_in)?;
        let new_out = old_out.checked_sub(amount_out)?;

        // Guard: reserves cannot go negative (physically impossible)
        require!(
            new_out.raw >= 0,
            crate::errors::OrbitalError::InsufficientLiquidity
        );

        // Compute all new values into temporaries before any mutation
        let new_running_sum = self.running_sum
            .checked_add(amount_in)?
            .checked_sub(amount_out)?;

        let new_running_sq_sum = self.running_sq_sum
            .checked_add(new_in.squared()?)?
            .checked_sub(old_in.squared()?)?
            .checked_add(new_out.squared()?)?
            .checked_sub(old_out.squared()?)?;

        // Atomic assignment: all-or-nothing update
        self.amounts[token_in] = new_in;
        self.amounts[token_out] = new_out;
        self.running_sum = new_running_sum;
        self.running_sq_sum = new_running_sq_sum;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a MAX_ASSETS array from a slice of i64 values
    fn make_amounts(vals: &[i64]) -> [FixedPoint; MAX_ASSETS] {
        let mut arr = [FixedPoint::zero(); MAX_ASSETS];
        for (i, &v) in vals.iter().enumerate() {
            arr[i] = FixedPoint::from_int(v);
        }
        arr
    }

    /// Standard test epsilon: ~2^-32 ≈ 2.3e-10
    fn epsilon() -> FixedPoint {
        FixedPoint::from_raw(1 << 32)
    }

    // ── Construction ──

    #[test]
    fn test_construction_cached_sums() {
        let amounts = make_amounts(&[100, 200, 300]);
        let rs = ReserveState::new(&amounts, 3).unwrap();

        // running_sum = 100 + 200 + 300 = 600
        assert_eq!(rs.running_sum, FixedPoint::from_int(600));

        // running_sq_sum = 100² + 200² + 300² = 10000 + 40000 + 90000 = 140000
        assert_eq!(rs.running_sq_sum, FixedPoint::from_int(140_000));
    }

    #[test]
    fn test_active_amounts_and_get() {
        let amounts = make_amounts(&[10, 20, 30]);
        let rs = ReserveState::new(&amounts, 3).unwrap();

        assert_eq!(rs.active_amounts().len(), 3);
        assert_eq!(rs.get(0).unwrap(), FixedPoint::from_int(10));
        assert_eq!(rs.get(2).unwrap(), FixedPoint::from_int(30));
        assert!(rs.get(3).is_err()); // out of bounds
    }

    // ── Alpha ──

    #[test]
    fn test_alpha_equal_reserves() {
        // n=3, all reserves = 100
        // alpha = 300 / sqrt(3) = 100*sqrt(3) ≈ 173.205
        let amounts = make_amounts(&[100, 100, 100]);
        let rs = ReserveState::new(&amounts, 3).unwrap();
        let alpha = rs.alpha().unwrap();

        // 100 * sqrt(3) ≈ 173.205
        let sqrt3 = FixedPoint::from_int(3).sqrt().unwrap();
        let expected = FixedPoint::from_int(100).checked_mul(sqrt3).unwrap();

        let eps = FixedPoint::from_raw(1i128 << 42); // larger epsilon for sqrt imprecision
        assert!(
            alpha.approx_eq(expected, eps),
            "alpha={:?}, expected={:?}",
            alpha,
            expected
        );
    }

    #[test]
    fn test_alpha_known_values() {
        // n=2, reserves = [150, 250]
        // alpha = 400 / sqrt(2) = 200*sqrt(2) ≈ 282.843
        let amounts = make_amounts(&[150, 250]);
        let rs = ReserveState::new(&amounts, 2).unwrap();
        let alpha = rs.alpha().unwrap();

        let sqrt2 = FixedPoint::from_int(2).sqrt().unwrap();
        let expected = FixedPoint::from_int(200).checked_mul(sqrt2).unwrap();

        let eps = FixedPoint::from_raw(1i128 << 42);
        assert!(
            alpha.approx_eq(expected, eps),
            "alpha={:?}, expected={:?}",
            alpha,
            expected
        );
    }

    // ── W Norm Squared ──

    #[test]
    fn test_w_norm_squared_equal_reserves() {
        // All reserves equal → w_norm_sq = 0 (perfectly on the diagonal)
        let amounts = make_amounts(&[100, 100, 100]);
        let rs = ReserveState::new(&amounts, 3).unwrap();
        let w_sq = rs.w_norm_squared().unwrap();

        // Should be zero (or very close due to fixed-point rounding)
        assert!(
            w_sq.approx_eq(FixedPoint::zero(), epsilon()),
            "w_norm_sq should be ~0, got {:?}",
            w_sq
        );
    }

    #[test]
    fn test_w_norm_squared_unequal() {
        // n=2, reserves = [100, 200]
        // sum = 300, sq_sum = 10000 + 40000 = 50000
        // w_norm_sq = 50000 - 300²/2 = 50000 - 45000 = 5000
        let amounts = make_amounts(&[100, 200]);
        let rs = ReserveState::new(&amounts, 2).unwrap();
        let w_sq = rs.w_norm_squared().unwrap();

        assert!(
            w_sq.approx_eq(FixedPoint::from_int(5000), epsilon()),
            "w_norm_sq should be 5000, got {:?}",
            w_sq
        );
    }

    // ── Price ──

    #[test]
    fn test_price_equal_reserves() {
        // All reserves at equal price point → price = 1.0
        let sphere = Sphere::new(FixedPoint::from_int(600), 3).unwrap(); // r = 200
        let q = sphere.equal_price_point().unwrap();
        let amounts = {
            let mut arr = [FixedPoint::zero(); MAX_ASSETS];
            arr[0] = q;
            arr[1] = q;
            arr[2] = q;
            arr
        };
        let rs = ReserveState::new(&amounts, 3).unwrap();

        let price = rs.price(0, 1, &sphere).unwrap();
        let eps = FixedPoint::from_raw(1i128 << 42);
        assert!(
            price.approx_eq(FixedPoint::one(), eps),
            "price at equal reserves should be 1.0, got {:?}",
            price
        );
    }

    #[test]
    fn test_price_unequal_reserves() {
        // 2-asset pool, r = 200, reserves = [80, 120]
        // price(0, 1) = (r - x1) / (r - x0) = (200-120) / (200-80) = 80/120 = 2/3
        // price(1, 0) = (r - x0) / (r - x1) = (200-80) / (200-120) = 120/80 = 3/2
        let sphere = Sphere { radius: FixedPoint::from_int(200), n: 2 };
        let amounts = make_amounts(&[80, 120]);
        let rs = ReserveState::new(&amounts, 2).unwrap();

        let p01 = rs.price(0, 1, &sphere).unwrap();
        let expected_01 = FixedPoint::from_fraction(2, 3).unwrap();
        let eps = FixedPoint::from_raw(1i128 << 33);
        assert!(
            p01.approx_eq(expected_01, eps),
            "price(0,1) should be 2/3, got {:?}",
            p01
        );

        let p10 = rs.price(1, 0, &sphere).unwrap();
        let expected_10 = FixedPoint::from_fraction(3, 2).unwrap();
        assert!(
            p10.approx_eq(expected_10, eps),
            "price(1,0) should be 3/2, got {:?}",
            p10
        );
    }

    #[test]
    fn test_price_same_token_error() {
        let sphere = Sphere { radius: FixedPoint::from_int(200), n: 2 };
        let amounts = make_amounts(&[100, 100]);
        let rs = ReserveState::new(&amounts, 2).unwrap();

        assert!(rs.price(0, 0, &sphere).is_err());
    }

    // ── Distance Squared O(1) vs O(n) ──

    #[test]
    fn test_distance_sq_o1_vs_on() {
        // Both methods should give the same result
        let sphere = Sphere::new(FixedPoint::from_int(900), 3).unwrap(); // r = 300
        let amounts = make_amounts(&[250, 300, 350]);
        let rs = ReserveState::new(&amounts, 3).unwrap();

        let d_sq_o1 = rs.distance_squared_from_center(&sphere).unwrap();
        let d_sq_on = sphere.distance_squared(&amounts[..3]).unwrap();

        let eps = FixedPoint::from_raw(1i128 << 42);
        assert!(
            d_sq_o1.approx_eq(d_sq_on, eps),
            "O(1)={:?} vs O(n)={:?} should match",
            d_sq_o1,
            d_sq_on
        );
    }

    // ── Apply Trade ──

    #[test]
    fn test_apply_trade_consistency() {
        let amounts = make_amounts(&[100, 200, 300]);
        let mut rs = ReserveState::new(&amounts, 3).unwrap();

        // Trade: token 0 receives 10, token 1 gives 10
        let trade_amt = FixedPoint::from_int(10);
        rs.apply_trade(0, trade_amt, 1, trade_amt).unwrap();

        // Reconstruct from updated amounts for verification
        let rs2 = ReserveState::new(&rs.amounts, 3).unwrap();

        assert!(
            rs.running_sum.approx_eq(rs2.running_sum, epsilon()),
            "running_sum mismatch: {:?} vs {:?}",
            rs.running_sum,
            rs2.running_sum
        );
        assert!(
            rs.running_sq_sum.approx_eq(rs2.running_sq_sum, epsilon()),
            "running_sq_sum mismatch: {:?} vs {:?}",
            rs.running_sq_sum,
            rs2.running_sq_sum
        );

        // Verify updated amounts
        assert_eq!(rs.amounts[0], FixedPoint::from_int(110));
        assert_eq!(rs.amounts[1], FixedPoint::from_int(190));
        assert_eq!(rs.amounts[2], FixedPoint::from_int(300));
    }

    // ── Invariant Checks (via Sphere) ──

    #[test]
    fn test_check_invariant_pass() {
        // n=3, total_liquidity=600 → r=200
        let sphere = Sphere::new(FixedPoint::from_int(600), 3).unwrap();
        let q = sphere.equal_price_point().unwrap();

        let amounts = {
            let mut arr = [FixedPoint::zero(); MAX_ASSETS];
            arr[0] = q;
            arr[1] = q;
            arr[2] = q;
            arr
        };
        let rs = ReserveState::new(&amounts, 3).unwrap();
        let d_sq = rs.distance_squared_from_center(&sphere).unwrap();

        // Should be approximately r²
        let r_sq = sphere.radius_squared().unwrap();
        let tolerance = FixedPoint::from_raw(r_sq.raw / 100); // 1% generous tolerance for sqrt rounding
        assert!(
            d_sq.approx_eq(r_sq, tolerance),
            "d_sq={:?} should ≈ r_sq={:?}",
            d_sq,
            r_sq
        );
    }

    #[test]
    fn test_check_invariant_fail() {
        // Reserves far from the sphere → invariant violation
        let sphere = Sphere::new(FixedPoint::from_int(600), 3).unwrap(); // r=200
        let amounts = make_amounts(&[10, 10, 10]); // way too small
        let rs = ReserveState::new(&amounts, 3).unwrap();
        let d_sq = rs.distance_squared_from_center(&sphere).unwrap();

        let r_sq = sphere.radius_squared().unwrap();
        let tolerance = FixedPoint::from_raw(r_sq.raw / 1000); // 0.1%

        // d_sq should NOT be approximately r_sq
        assert!(
            !d_sq.approx_eq(r_sq, tolerance),
            "d_sq={:?} should NOT ≈ r_sq={:?} with 0.1% tolerance",
            d_sq,
            r_sq
        );
    }

    // ── Negative Trade Amount Guard ──

    #[test]
    fn test_apply_trade_rejects_negative_amount_in() {
        let amounts = make_amounts(&[100, 200, 300]);
        let mut rs = ReserveState::new(&amounts, 3).unwrap();

        let neg = FixedPoint::from_int(-10);
        let pos = FixedPoint::from_int(10);
        assert!(rs.apply_trade(0, neg, 1, pos).is_err());
    }

    #[test]
    fn test_apply_trade_rejects_negative_amount_out() {
        let amounts = make_amounts(&[100, 200, 300]);
        let mut rs = ReserveState::new(&amounts, 3).unwrap();

        let pos = FixedPoint::from_int(10);
        let neg = FixedPoint::from_int(-10);
        assert!(rs.apply_trade(0, pos, 1, neg).is_err());
    }

    // ── Reserve Drain Guard ──

    #[test]
    fn test_apply_trade_rejects_reserve_drain() {
        // reserves = [100, 200, 300], try to withdraw 250 from token 1 (reserve = 200)
        let amounts = make_amounts(&[100, 200, 300]);
        let mut rs = ReserveState::new(&amounts, 3).unwrap();

        let amount_in = FixedPoint::from_int(50);
        let amount_out = FixedPoint::from_int(250); // > 200, should fail
        assert!(rs.apply_trade(0, amount_in, 1, amount_out).is_err());
    }

    #[test]
    fn test_apply_trade_allows_exact_drain() {
        // Withdrawing exactly the full reserve should succeed (new_out = 0)
        let amounts = make_amounts(&[100, 200, 300]);
        let mut rs = ReserveState::new(&amounts, 3).unwrap();

        let amount_in = FixedPoint::from_int(50);
        let amount_out = FixedPoint::from_int(200); // == reserve, new_out = 0
        assert!(rs.apply_trade(0, amount_in, 1, amount_out).is_ok());
        assert_eq!(rs.amounts[1].raw, 0);
    }
}
