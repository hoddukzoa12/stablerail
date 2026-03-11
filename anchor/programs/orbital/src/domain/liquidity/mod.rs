//! Liquidity Context Domain Logic
//!
//! Pure business rules for LP position management. Handles reserve
//! mutation, sphere recomputation, and proportional withdrawal.
//!
//! No Solana CPI here — operates directly on `PoolState` references.
//! SPL token transfers are handled by the instruction handlers.

use anchor_lang::prelude::*;

use crate::domain::core::{recompute_sphere, update_caches, verify_invariant};
use crate::errors::OrbitalError;
use crate::math::sphere::MAX_ASSETS;
use crate::math::FixedPoint;
use crate::state::PoolState;

// ── Result Types ──

/// Outcome of adding liquidity to a pool.
pub struct AddLiquidityResult {
    /// Liquidity units assigned to the position (sum of deposits)
    pub liquidity: FixedPoint,
    /// New sphere radius after deposit
    pub new_radius: FixedPoint,
}

/// Outcome of removing liquidity from a pool.
pub struct RemoveLiquidityResult {
    /// Per-token amounts returned to the LP (FixedPoint for domain use)
    pub return_amounts: [FixedPoint; MAX_ASSETS],
    /// Per-token amounts as u64 (for SPL transfers)
    pub return_amounts_u64: [u64; MAX_ASSETS],
    /// New sphere radius after withdrawal
    pub new_radius: FixedPoint,
}

// ── Domain Functions ──

/// Add liquidity to a pool: update reserves, recompute sphere, verify invariant.
///
/// Workflow:
///   1. Validate all deposits are positive for active assets
///   2. Add deposits to reserves and accumulate liquidity sum
///   3. Recompute sphere radius from new reserves
///   4. Update total_interior_liquidity
///   5. Update caches (alpha, w_norm_sq)
///   6. Verify sphere invariant (post-condition)
///
/// Precondition: SPL token transfers already completed by instruction handler.
pub fn add_liquidity_to_pool(
    pool: &mut PoolState,
    deposits: &[FixedPoint],
) -> Result<AddLiquidityResult> {
    let n = pool.n_assets as usize;

    // 1. Validate deposits
    require!(deposits.len() >= n, OrbitalError::InvalidAssetCount);
    for i in 0..n {
        require!(
            deposits[i].is_positive(),
            OrbitalError::InvalidLiquidityAmount
        );
    }

    // 2. Add deposits to reserves and accumulate liquidity sum
    //
    // WARNING: MVP uses sum-of-deposits model (liquidity = Σ deposits).
    // This allows fee-free rebalance via asymmetric deposit + proportional
    // withdrawal during depeg events. Safe for stablecoin pools where all
    // tokens ≈ $1, but must be replaced for production.
    // Post-MVP fix: reserve-ratio based shares or radius-delta model.
    // See: https://github.com/hoddukzoa12/stablerail/issues/36
    let mut liquidity = FixedPoint::zero();
    for i in 0..n {
        pool.reserves[i] = pool.reserves[i].checked_add(deposits[i])?;
        liquidity = liquidity.checked_add(deposits[i])?;
    }

    // 3. Recompute sphere radius
    let new_radius = recompute_sphere(pool)?;

    // 4. Update total liquidity tracking
    pool.total_interior_liquidity = pool.total_interior_liquidity.checked_add(liquidity)?;

    // 5. Update caches
    update_caches(pool)?;

    // 6. Post-condition: invariant must hold
    verify_invariant(pool)?;

    Ok(AddLiquidityResult {
        liquidity,
        new_radius,
    })
}

/// Remove liquidity from a pool: compute proportional returns, update reserves.
///
/// Workflow:
///   1. Validate removal amount
///   2. Compute LP's fraction: remove_amount / total_interior_liquidity
///   3. Calculate proportional per-token returns (truncated to u64)
///   4. Reject if all returns round to zero (prevents zero-payout burns)
///   5. Subtract truncated returns from reserves (aligned with SPL transfers)
///   6. Subtract from total_interior_liquidity
///   7. Recompute sphere radius
///   8. Update caches
///   9. Verify sphere invariant (post-condition)
///
/// Precondition: position ownership validated by instruction handler.
/// SPL token transfers executed by instruction handler after this returns.
pub fn remove_liquidity_from_pool(
    pool: &mut PoolState,
    remove_amount: FixedPoint,
) -> Result<RemoveLiquidityResult> {
    let n = pool.n_assets as usize;

    // 1. Validate
    require!(
        remove_amount.is_positive(),
        OrbitalError::InvalidLiquidityAmount
    );
    require!(
        pool.total_interior_liquidity.is_positive(),
        OrbitalError::InsufficientLiquidity
    );
    require!(
        remove_amount.raw <= pool.total_interior_liquidity.raw,
        OrbitalError::InsufficientLiquidity
    );

    // 2. Compute fraction
    let fraction = remove_amount.checked_div(pool.total_interior_liquidity)?;

    // 3. Calculate proportional returns (truncated to integer token amounts)
    let mut return_amounts = [FixedPoint::zero(); MAX_ASSETS];
    let mut return_amounts_u64 = [0u64; MAX_ASSETS];
    for i in 0..n {
        return_amounts[i] = pool.reserves[i].checked_mul(fraction)?;
        return_amounts_u64[i] = return_amounts[i].to_u64()?;
    }

    // 4. Reject if all returns round to zero (prevents zero-payout burns)
    let has_nonzero = return_amounts_u64[..n].iter().any(|&a| a > 0);
    require!(has_nonzero, OrbitalError::WithdrawalTooSmall);

    // 5. Subtract truncated returns from reserves (aligned with SPL transfers
    //    to prevent reserve/vault drift from fractional rounding).
    for i in 0..n {
        let transferred = FixedPoint::checked_from_u64(return_amounts_u64[i])?;
        pool.reserves[i] = pool.reserves[i].checked_sub(transferred)?;
    }

    // 6. Subtract from total liquidity
    pool.total_interior_liquidity = pool.total_interior_liquidity.checked_sub(remove_amount)?;

    // 7. Recompute sphere radius
    let new_radius = recompute_sphere(pool)?;

    // 8. Update caches
    update_caches(pool)?;

    // 9. Post-condition: invariant must hold
    verify_invariant(pool)?;

    Ok(RemoveLiquidityResult {
        return_amounts,
        return_amounts_u64,
        new_radius,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::core::test_helpers::{init_pool, sqrt_epsilon};
    use crate::domain::core::verify_invariant;

    // ══════════════════════════════════════════════
    // add_liquidity_to_pool tests
    // ══════════════════════════════════════════════

    #[test]
    fn test_add_liquidity_equal_deposits() {
        let mut pool = init_pool(3, 100);
        let initial_radius = pool.sphere.radius;

        let deposits = vec![
            FixedPoint::from_int(50),
            FixedPoint::from_int(50),
            FixedPoint::from_int(50),
        ];
        let result = add_liquidity_to_pool(&mut pool, &deposits).unwrap();

        // Liquidity should be sum of deposits
        let expected_liq = FixedPoint::from_int(150); // 50+50+50
        assert_eq!(result.liquidity.raw, expected_liq.raw);

        // Radius should increase
        assert!(result.new_radius.raw > initial_radius.raw);

        // Reserves should be 150 each
        for i in 0..3 {
            assert_eq!(pool.reserves[i].raw, FixedPoint::from_int(150).raw);
        }

        // Invariant should hold
        verify_invariant(&pool).unwrap();
    }

    #[test]
    fn test_add_liquidity_asymmetric_deposits() {
        let mut pool = init_pool(3, 100);

        let deposits = vec![
            FixedPoint::from_int(30),
            FixedPoint::from_int(50),
            FixedPoint::from_int(40),
        ];
        let result = add_liquidity_to_pool(&mut pool, &deposits).unwrap();

        // Liquidity = 30+50+40 = 120
        assert_eq!(result.liquidity.raw, FixedPoint::from_int(120).raw);

        // Reserves should be 130, 150, 140
        assert_eq!(pool.reserves[0].raw, FixedPoint::from_int(130).raw);
        assert_eq!(pool.reserves[1].raw, FixedPoint::from_int(150).raw);
        assert_eq!(pool.reserves[2].raw, FixedPoint::from_int(140).raw);

        // Invariant should hold
        verify_invariant(&pool).unwrap();
    }

    #[test]
    fn test_add_liquidity_updates_total() {
        let mut pool = init_pool(3, 100);
        // total_interior_liquidity seeded to 3*100 = 300
        let initial_total = pool.total_interior_liquidity;

        let deposits = vec![
            FixedPoint::from_int(50),
            FixedPoint::from_int(50),
            FixedPoint::from_int(50),
        ];
        add_liquidity_to_pool(&mut pool, &deposits).unwrap();

        let expected_total = initial_total
            .checked_add(FixedPoint::from_int(150))
            .unwrap();
        assert_eq!(pool.total_interior_liquidity.raw, expected_total.raw);
    }

    #[test]
    fn test_add_liquidity_rejects_zero_deposit() {
        let mut pool = init_pool(3, 100);
        let deposits = vec![
            FixedPoint::from_int(50),
            FixedPoint::zero(), // invalid
            FixedPoint::from_int(50),
        ];
        assert!(add_liquidity_to_pool(&mut pool, &deposits).is_err());
    }

    #[test]
    fn test_add_liquidity_rejects_insufficient_deposits() {
        let mut pool = init_pool(3, 100);
        // Only 2 deposits for 3-asset pool
        let deposits = vec![FixedPoint::from_int(50), FixedPoint::from_int(50)];
        assert!(add_liquidity_to_pool(&mut pool, &deposits).is_err());
    }

    // ══════════════════════════════════════════════
    // remove_liquidity_from_pool tests
    // ══════════════════════════════════════════════

    #[test]
    fn test_remove_liquidity_proportional() {
        let mut pool = init_pool(3, 100);
        // total_interior_liquidity = 300

        // Remove 150 of 300 = 50% fraction
        let remove = FixedPoint::from_int(150);
        let result = remove_liquidity_from_pool(&mut pool, remove).unwrap();

        // Each reserve should be halved: 100 * 0.5 = 50
        for i in 0..3 {
            assert!(
                pool.reserves[i].approx_eq(FixedPoint::from_int(50), sqrt_epsilon()),
                "reserve[{}] should ≈ 50, got {:?}",
                i,
                pool.reserves[i]
            );
        }

        // Return amounts should each be ≈ 50
        for i in 0..3 {
            assert!(
                result.return_amounts[i].approx_eq(FixedPoint::from_int(50), sqrt_epsilon()),
                "return[{}] should ≈ 50, got {:?}",
                i,
                result.return_amounts[i]
            );
        }

        // Invariant should hold
        verify_invariant(&pool).unwrap();
    }

    #[test]
    fn test_remove_liquidity_rejects_more_than_total() {
        let mut pool = init_pool(3, 100);
        let remove = FixedPoint::from_int(400); // > 300 total
        assert!(remove_liquidity_from_pool(&mut pool, remove).is_err());
    }

    #[test]
    fn test_remove_liquidity_rejects_zero() {
        let mut pool = init_pool(3, 100);
        assert!(remove_liquidity_from_pool(&mut pool, FixedPoint::zero()).is_err());
    }

    #[test]
    fn test_remove_liquidity_rejects_dust_withdrawal() {
        // P1: tiny withdrawal where all per-token returns truncate to 0
        let mut pool = init_pool(3, 1_000_000); // large pool
        // total_interior_liquidity = 3,000,000
        // fraction = 1 / 3,000,000 → per-asset return = 1,000,000 * (1/3,000,000) ≈ 0.333 → truncates to 0
        let dust = FixedPoint::from_int(1);
        assert!(remove_liquidity_from_pool(&mut pool, dust).is_err());
    }

    #[test]
    fn test_roundtrip_add_then_remove() {
        let mut pool = init_pool(3, 100);
        let initial_reserves: Vec<_> = pool.reserves[..3].to_vec();

        // Add equal liquidity
        let deposits = vec![
            FixedPoint::from_int(50),
            FixedPoint::from_int(50),
            FixedPoint::from_int(50),
        ];
        let add_result = add_liquidity_to_pool(&mut pool, &deposits).unwrap();

        // Remove exactly what was added
        let result = remove_liquidity_from_pool(&mut pool, add_result.liquidity).unwrap();

        // Reserves should approximately return to initial values
        // (small rounding errors due to fixed-point division)
        let generous_eps = FixedPoint::from_int(1); // within 1 token unit
        for i in 0..3 {
            assert!(
                pool.reserves[i].approx_eq(initial_reserves[i], generous_eps),
                "reserve[{}] should ≈ {:?}, got {:?}",
                i,
                initial_reserves[i],
                pool.reserves[i]
            );
        }

        // Return amounts should be approximately the deposits
        for i in 0..3 {
            assert!(
                result.return_amounts[i].approx_eq(deposits[i], generous_eps),
                "return[{}] should ≈ {:?}, got {:?}",
                i,
                deposits[i],
                result.return_amounts[i]
            );
        }
    }
}
