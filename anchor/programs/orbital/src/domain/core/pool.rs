//! Pool Aggregate Root — Domain Logic
//!
//! Pure business logic operating on `PoolState`. Handles invariant
//! verification, cache management, radius computation from deposits,
//! vault PDA derivation, and pool initialization.
//!
//! The on-chain account struct `PoolState` lives in `state/pool.rs`;
//! this module provides the domain operations that mutate it.

use anchor_lang::prelude::*;

use crate::errors::OrbitalError;
use crate::math::sphere::MAX_ASSETS;
use crate::math::{FixedPoint, ReserveState, Sphere};
use crate::state::PoolState;

// ── Private Helper ──

/// Construct a transient ReserveState from PoolState fields.
///
/// ReserveState is not stored on-chain; it is created on-demand for
/// O(1) math (alpha, w_norm_squared, distance_squared_from_center)
/// via its cached running_sum and running_sq_sum.
fn build_reserve_state(pool: &PoolState) -> Result<ReserveState> {
    ReserveState::new(&pool.reserves, pool.n_assets)
}

// ── Public Domain Functions ──

/// Compute sphere radius from per-asset deposit amount.
///
/// Given equal deposits D for each of n assets, the radius r that
/// satisfies the sphere invariant n·(r - D)² = r² is:
///
///   r = D · √n / (√n - 1)
///
/// Cross-check: `Sphere { radius: r, n }.equal_price_point()` should
/// return D, since all reserves start at the equal price point.
pub fn compute_radius_from_deposit(deposit: FixedPoint, n: u8) -> Result<FixedPoint> {
    require!(deposit.is_positive(), OrbitalError::InvalidLiquidityAmount);
    require!(
        n >= 2 && (n as usize) <= MAX_ASSETS,
        OrbitalError::InvalidAssetCount
    );

    let n_fp = FixedPoint::from_int(n as i64);
    let sqrt_n = n_fp.sqrt()?;
    let one = FixedPoint::one();

    // r = D · √n / (√n - 1)
    let numerator = deposit.checked_mul(sqrt_n)?;
    let denominator = sqrt_n.checked_sub(one)?;
    numerator.checked_div(denominator)
}

/// Compute sphere radius from arbitrary reserve vector.
///
/// Solves the sphere invariant `Σ(r - xᵢ)² = r²` for r, which yields
/// the quadratic `(n-1)·r² - 2r·Σxᵢ + Σxᵢ² = 0`. By the quadratic
/// formula (taking the larger root):
///
///   r = (Σxᵢ + √((Σxᵢ)² - (n-1)·Σxᵢ²)) / (n-1)
///
/// This is the general form of [`compute_radius_from_deposit`].
/// At equal reserves xᵢ = D, it produces the same result: r = D·√n/(√n-1).
///
/// Returns error if the discriminant is negative (reserves too imbalanced
/// for a valid sphere to exist).
pub fn compute_radius_from_reserves(
    reserves: &[FixedPoint; MAX_ASSETS],
    n: u8,
) -> Result<FixedPoint> {
    require!(
        n >= 2 && (n as usize) <= MAX_ASSETS,
        OrbitalError::InvalidAssetCount
    );

    let n_usize = n as usize;
    let n_minus_1 = FixedPoint::from_int((n as i64) - 1);

    let mut sum_x = FixedPoint::zero();
    let mut sum_x_sq = FixedPoint::zero();
    for i in 0..n_usize {
        require!(reserves[i].raw >= 0, OrbitalError::InvalidLiquidityAmount);
        sum_x = sum_x.checked_add(reserves[i])?;
        sum_x_sq = sum_x_sq.checked_add(reserves[i].squared()?)?;
    }

    // discriminant = (Σxᵢ)² - (n-1)·Σxᵢ²
    let sum_x_squared = sum_x.squared()?;
    let scaled_sum_sq = n_minus_1.checked_mul(sum_x_sq)?;
    let discriminant = sum_x_squared.checked_sub(scaled_sum_sq)?;

    require!(discriminant.raw >= 0, OrbitalError::InvariantViolation);

    let sqrt_disc = discriminant.sqrt()?;
    let numerator = sum_x.checked_add(sqrt_disc)?;
    numerator.checked_div(n_minus_1)
}

/// Verify sphere invariant: ||r⃗ - x⃗||² ≈ r² (O(1) path).
///
/// Constructs a transient ReserveState for O(1) distance computation,
/// then delegates to `Sphere::check_invariant_with_distance_sq`.
/// Tolerance: r² / 1000 (0.1%).
pub fn verify_invariant(pool: &PoolState) -> Result<()> {
    let rs = build_reserve_state(pool)?;
    let d_sq = rs.distance_squared_from_center(&pool.sphere)?;
    pool.sphere.check_invariant_with_distance_sq(d_sq)
}

/// Recompute and store alpha_cache and w_norm_sq_cache on PoolState.
///
/// alpha = Σxᵢ / √n   (parallel projection onto diagonal)
/// w_norm_sq = Σxᵢ² - (Σxᵢ)²/n   (orthogonal deviation)
///
/// Must be called after every reserve mutation (swap, add/remove liquidity).
pub fn update_caches(pool: &mut PoolState) -> Result<()> {
    let rs = build_reserve_state(pool)?;
    pool.alpha_cache = rs.alpha()?;
    pool.w_norm_sq_cache = rs.w_norm_squared()?;
    Ok(())
}

/// Derive token vault PDA address for a given pool and mint.
///
/// Seeds: `["vault", pool_pubkey, mint_pubkey]`.
/// Returns `(derived_address, bump_seed)`.
pub fn derive_vault_pda(
    pool_key: &Pubkey,
    mint_key: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"vault", pool_key.as_ref(), mint_key.as_ref()],
        program_id,
    )
}

/// Initialize pool reserves and sphere from equal initial deposits.
///
/// Workflow:
///   1. Validate inputs (counts, deposit > 0, no zero/duplicate mints)
///   2. Compute sphere radius: r = D·√n/(√n-1)
///   3. Set all active reserves to deposit amount
///   4. Store token mints and vault addresses
///   5. Recompute alpha_cache and w_norm_sq_cache
///   6. Verify sphere invariant (post-condition)
///
/// Precondition: `pool.n_assets`, `pool.fee_rate_bps`, `pool.bump`,
/// `pool.authority` already set by the instruction handler.
pub fn initialize_pool_reserves(
    pool: &mut PoolState,
    per_asset_deposit: FixedPoint,
    token_mints: &[Pubkey],
    token_vaults: &[Pubkey],
) -> Result<()> {
    let n = pool.n_assets;
    let n_usize = n as usize;

    // 1. Validate inputs
    require!(
        token_mints.len() == n_usize && token_vaults.len() == n_usize,
        OrbitalError::InvalidAssetCount
    );
    require!(
        per_asset_deposit.is_positive(),
        OrbitalError::InvalidLiquidityAmount
    );
    // Reject default (zero) and duplicate mints (O(n²) is fine for n ≤ MAX_ASSETS = 8).
    // Note: duplicate-mint check also runs early in the instruction handler (before CPI)
    // to return OrbitalError::DuplicateTokenMint instead of opaque system program errors.
    // This check remains as a domain-level invariant guard for any future callers.
    for i in 0..n_usize {
        require!(
            token_mints[i] != Pubkey::default(),
            OrbitalError::InvalidTokenIndex
        );
        for j in (i + 1)..n_usize {
            require!(
                token_mints[i] != token_mints[j],
                OrbitalError::DuplicateTokenMint
            );
        }
    }

    // 2. Compute sphere from deposits
    let radius = compute_radius_from_deposit(per_asset_deposit, n)?;
    pool.sphere = Sphere { radius, n };

    // 3. Set reserves (all equal at initialization)
    for i in 0..n_usize {
        pool.reserves[i] = per_asset_deposit;
    }
    for i in n_usize..MAX_ASSETS {
        pool.reserves[i] = FixedPoint::zero();
    }

    // 4. Store token references
    for i in 0..n_usize {
        pool.token_mints[i] = token_mints[i];
        pool.token_vaults[i] = token_vaults[i];
    }

    // 5. Seed total_interior_liquidity with initial deposit sum
    //    This serves as the denominator for proportional withdrawals.
    //    The authority's initial deposit is "implicit liquidity" (no Position PDA).
    let n_fp = FixedPoint::from_int(n as i64);
    pool.total_interior_liquidity = per_asset_deposit.checked_mul(n_fp)?;

    // 6. Update caches
    update_caches(pool)?;

    // 7. Post-condition: invariant must hold
    verify_invariant(pool)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::core::test_helpers::{make_pool, unique_pubkeys};

    /// Generous epsilon for sqrt-derived comparisons (~2^-22)
    fn sqrt_epsilon() -> FixedPoint {
        FixedPoint::from_raw(1i128 << 42)
    }

    /// Initialize a pool with equal deposits and return it.
    fn init_pool(n: u8, deposit: i64) -> PoolState {
        let mut pool = make_pool(n);
        let deposit_fp = FixedPoint::from_int(deposit);
        let mints = unique_pubkeys(n as usize);
        let vaults = unique_pubkeys(n as usize);
        initialize_pool_reserves(&mut pool, deposit_fp, &mints, &vaults).unwrap();
        pool
    }

    // ══════════════════════════════════════════════
    // compute_radius_from_deposit tests
    // ══════════════════════════════════════════════

    #[test]
    fn test_compute_radius_n2_equal_price_point_roundtrip() {
        let deposit = FixedPoint::from_int(100);
        let r = compute_radius_from_deposit(deposit, 2).unwrap();
        let sphere = Sphere { radius: r, n: 2 };
        let epp = sphere.equal_price_point().unwrap();
        assert!(
            epp.approx_eq(deposit, sqrt_epsilon()),
            "equal_price_point ({:?}) should ≈ deposit ({:?})",
            epp,
            deposit
        );
    }

    #[test]
    fn test_compute_radius_n3_equal_price_point_roundtrip() {
        let deposit = FixedPoint::from_int(100);
        let r = compute_radius_from_deposit(deposit, 3).unwrap();
        let sphere = Sphere { radius: r, n: 3 };
        let epp = sphere.equal_price_point().unwrap();
        assert!(
            epp.approx_eq(deposit, sqrt_epsilon()),
            "equal_price_point ({:?}) should ≈ deposit ({:?})",
            epp,
            deposit
        );
    }

    #[test]
    fn test_compute_radius_n8_equal_price_point_roundtrip() {
        let deposit = FixedPoint::from_int(50);
        let r = compute_radius_from_deposit(deposit, 8).unwrap();
        let sphere = Sphere { radius: r, n: 8 };
        let epp = sphere.equal_price_point().unwrap();
        assert!(
            epp.approx_eq(deposit, sqrt_epsilon()),
            "equal_price_point ({:?}) should ≈ deposit ({:?})",
            epp,
            deposit
        );
    }

    #[test]
    fn test_compute_radius_rejects_zero_deposit() {
        assert!(compute_radius_from_deposit(FixedPoint::zero(), 3).is_err());
    }

    // ══════════════════════════════════════════════
    // verify_invariant tests
    // ══════════════════════════════════════════════

    #[test]
    fn test_verify_invariant_at_equal_price_point() {
        let pool = init_pool(3, 100);
        verify_invariant(&pool).unwrap();
    }

    #[test]
    fn test_verify_invariant_fails_broken_state() {
        let mut pool = init_pool(3, 100);
        // Corrupt reserves to break invariant
        pool.reserves[0] = FixedPoint::from_int(999);
        assert!(verify_invariant(&pool).is_err());
    }

    #[test]
    fn test_verify_invariant_o1_matches_on() {
        let pool = init_pool(3, 100);
        // O(1) path via domain function
        let o1_result = verify_invariant(&pool);
        // O(n) path via Sphere directly
        let on_result =
            pool.sphere
                .check_invariant(&pool.reserves[..pool.n_assets as usize]);
        assert_eq!(o1_result.is_ok(), on_result.is_ok());
    }

    // ══════════════════════════════════════════════
    // update_caches tests
    // ══════════════════════════════════════════════

    #[test]
    fn test_update_caches_equal_reserves() {
        let pool = init_pool(3, 100);
        // alpha = 3*100/√3 = 100√3 ≈ 173.21
        let expected_alpha = FixedPoint::from_int(173);
        assert!(
            pool.alpha_cache.approx_eq(expected_alpha, FixedPoint::from_int(1)),
            "alpha ({:?}) should ≈ 173",
            pool.alpha_cache
        );
        // w_norm_sq at equal reserves should be ≈ 0
        let zero = FixedPoint::zero();
        assert!(
            pool.w_norm_sq_cache.approx_eq(zero, sqrt_epsilon()),
            "w_norm_sq ({:?}) should ≈ 0",
            pool.w_norm_sq_cache
        );
    }

    #[test]
    fn test_update_caches_unequal_reserves() {
        let mut pool = init_pool(2, 100);
        // Manually set unequal reserves that still lie on the sphere
        // For n=2, r ≈ 341.42: try x1=80, x2 = ?
        // (r-x1)^2 + (r-x2)^2 = r^2 → x2 = r - sqrt(r^2 - (r-x1)^2)
        // Instead, just set and recompute caches — we test that caches
        // match ReserveState output, not invariant compliance here.
        pool.reserves[0] = FixedPoint::from_int(80);
        pool.reserves[1] = FixedPoint::from_int(120);
        update_caches(&mut pool).unwrap();

        let rs = ReserveState::new(&pool.reserves, pool.n_assets).unwrap();
        assert_eq!(pool.alpha_cache.raw, rs.alpha().unwrap().raw);
        assert_eq!(pool.w_norm_sq_cache.raw, rs.w_norm_squared().unwrap().raw);
    }

    #[test]
    fn test_update_caches_idempotent() {
        let mut pool = init_pool(3, 100);
        let alpha_first = pool.alpha_cache;
        let wnorm_first = pool.w_norm_sq_cache;
        update_caches(&mut pool).unwrap();
        assert_eq!(pool.alpha_cache.raw, alpha_first.raw);
        assert_eq!(pool.w_norm_sq_cache.raw, wnorm_first.raw);
    }

    // ══════════════════════════════════════════════
    // initialize_pool_reserves tests
    // ══════════════════════════════════════════════

    #[test]
    fn test_initialize_pool_reserves_n3_equal() {
        let pool = init_pool(3, 100);
        let deposit = FixedPoint::from_int(100);

        // All active reserves should equal deposit
        for i in 0..3 {
            assert_eq!(pool.reserves[i].raw, deposit.raw);
        }
        // Inactive reserves should be zero
        for i in 3..MAX_ASSETS {
            assert_eq!(pool.reserves[i].raw, 0);
        }
        // Sphere radius should produce correct equal_price_point
        let epp = pool.sphere.equal_price_point().unwrap();
        assert!(epp.approx_eq(deposit, sqrt_epsilon()));
        // Caches should be populated (non-zero alpha)
        assert!(pool.alpha_cache.raw > 0);
        // Invariant should hold
        verify_invariant(&pool).unwrap();
    }

    #[test]
    fn test_initialize_pool_reserves_stores_mints_and_vaults() {
        let mut pool = make_pool(3);
        let deposit = FixedPoint::from_int(100);
        let mints = unique_pubkeys(3);
        let vaults = unique_pubkeys(3);
        initialize_pool_reserves(&mut pool, deposit, &mints, &vaults).unwrap();

        for i in 0..3 {
            assert_eq!(pool.token_mints[i], mints[i]);
            assert_eq!(pool.token_vaults[i], vaults[i]);
        }
    }

    #[test]
    fn test_initialize_pool_reserves_rejects_mismatched_mint_count() {
        let mut pool = make_pool(3);
        let deposit = FixedPoint::from_int(100);
        let mints = unique_pubkeys(2); // Wrong count
        let vaults = unique_pubkeys(3);
        assert!(initialize_pool_reserves(&mut pool, deposit, &mints, &vaults).is_err());
    }

    #[test]
    fn test_initialize_pool_reserves_rejects_duplicate_mints() {
        let mut pool = make_pool(3);
        let deposit = FixedPoint::from_int(100);
        let mint_a = Pubkey::new_unique();
        let mint_b = Pubkey::new_unique();
        let mints = vec![mint_a, mint_b, mint_a]; // Duplicate mint_a
        let vaults = unique_pubkeys(3);
        assert!(initialize_pool_reserves(&mut pool, deposit, &mints, &vaults).is_err());
    }

    #[test]
    fn test_initialize_pool_reserves_rejects_zero_mint() {
        let mut pool = make_pool(3);
        let deposit = FixedPoint::from_int(100);
        let mint_a = Pubkey::new_unique();
        let mint_b = Pubkey::new_unique();
        let mints = vec![mint_a, Pubkey::default(), mint_b]; // Zero key
        let vaults = unique_pubkeys(3);
        assert!(initialize_pool_reserves(&mut pool, deposit, &mints, &vaults).is_err());
    }

    #[test]
    fn test_initialize_pool_reserves_accepts_unique_mints() {
        let pool = init_pool(3, 100);
        // init_pool generates unique mints via unique_pubkeys;
        // reaching here without error confirms acceptance.
        assert!(pool.sphere.radius.is_positive());
    }

    // ══════════════════════════════════════════════
    // derive_vault_pda test
    // ══════════════════════════════════════════════

    // ══════════════════════════════════════════════
    // compute_radius_from_reserves tests
    // ══════════════════════════════════════════════

    #[test]
    fn test_compute_radius_from_reserves_matches_equal_deposit() {
        // For equal reserves, compute_radius_from_reserves should match compute_radius_from_deposit
        let deposit = FixedPoint::from_int(100);
        for n in [2u8, 3, 5, 8] {
            let r_deposit = compute_radius_from_deposit(deposit, n).unwrap();

            let mut reserves = [FixedPoint::zero(); MAX_ASSETS];
            for i in 0..(n as usize) {
                reserves[i] = deposit;
            }
            let r_reserves = compute_radius_from_reserves(&reserves, n).unwrap();

            assert!(
                r_deposit.approx_eq(r_reserves, sqrt_epsilon()),
                "n={}: from_deposit ({:?}) should ≈ from_reserves ({:?})",
                n,
                r_deposit,
                r_reserves
            );
        }
    }

    #[test]
    fn test_compute_radius_from_reserves_asymmetric() {
        // Slightly asymmetric reserves should still produce a valid radius
        let mut reserves = [FixedPoint::zero(); MAX_ASSETS];
        reserves[0] = FixedPoint::from_int(90);
        reserves[1] = FixedPoint::from_int(100);
        reserves[2] = FixedPoint::from_int(110);

        let r = compute_radius_from_reserves(&reserves, 3).unwrap();
        assert!(r.is_positive(), "radius should be positive");

        // Verify invariant: Σ(r - xᵢ)² = r²
        let sphere = Sphere { radius: r, n: 3 };
        let active = &reserves[..3];
        // Use generous tolerance for asymmetric case
        assert!(
            sphere.check_invariant(active).is_ok(),
            "invariant should hold for asymmetric reserves"
        );
    }

    #[test]
    fn test_compute_radius_from_reserves_rejects_invalid_n() {
        let reserves = [FixedPoint::from_int(100); MAX_ASSETS];
        assert!(compute_radius_from_reserves(&reserves, 1).is_err());
        assert!(compute_radius_from_reserves(&reserves, 9).is_err());
    }

    // ══════════════════════════════════════════════
    // derive_vault_pda test
    // ══════════════════════════════════════════════

    #[test]
    fn test_derive_vault_pda_deterministic_and_distinct() {
        let pool = Pubkey::new_unique();
        let mint_a = Pubkey::new_unique();
        let mint_b = Pubkey::new_unique();
        let program = Pubkey::new_unique();

        let (pda_a1, bump_a1) = derive_vault_pda(&pool, &mint_a, &program);
        let (pda_a2, bump_a2) = derive_vault_pda(&pool, &mint_a, &program);
        let (pda_b, _) = derive_vault_pda(&pool, &mint_b, &program);

        // Deterministic: same inputs → same output
        assert_eq!(pda_a1, pda_a2);
        assert_eq!(bump_a1, bump_a2);
        // Distinct: different mints → different PDAs
        assert_ne!(pda_a1, pda_b);
    }
}
