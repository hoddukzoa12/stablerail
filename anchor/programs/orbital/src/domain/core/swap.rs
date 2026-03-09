//! Swap Execution — Domain Logic
//!
//! Implements the on-chain portion of the swap flow:
//!   1. SDK computes expected_amount_out off-chain (torus invariant + Newton)
//!   2. This module verifies and applies the trade on-chain
//!   3. Post-swap: sphere invariant check, slippage enforcement, cache update
//!
//! The on-chain program trusts the off-chain amount_out only if the
//! sphere invariant holds after the trade is applied.

use anchor_lang::prelude::*;

use crate::errors::OrbitalError;
use crate::math::FixedPoint;
use crate::state::PoolState;

use super::pool::{update_caches, verify_invariant};

// ══════════════════════════════════════════════════════════════
// SwapResult
// ══════════════════════════════════════════════════════════════

/// Result of an on-chain swap execution.
pub struct SwapResult {
    /// Gross amount deposited by user (before fee)
    pub amount_in: FixedPoint,
    /// Fee deducted from amount_in
    pub fee: FixedPoint,
    /// Net amount added to pool reserves (amount_in - fee)
    pub net_amount_in: FixedPoint,
    /// Amount withdrawn from pool to user
    pub amount_out: FixedPoint,
    /// Execution price: amount_in / amount_out (Q64.64 raw)
    pub execution_price: FixedPoint,
    /// Slippage in basis points vs pre-swap mid-market price
    pub slippage_bps: u16,
}

// ══════════════════════════════════════════════════════════════
// Fee computation
// ══════════════════════════════════════════════════════════════

/// Compute swap fee from gross amount and fee rate.
///
/// fee = amount_in * fee_rate_bps / 10_000
pub fn compute_fee(amount_in: FixedPoint, fee_rate_bps: u16) -> Result<FixedPoint> {
    if fee_rate_bps == 0 {
        return Ok(FixedPoint::zero());
    }
    let bps = FixedPoint::from_int(fee_rate_bps as i64);
    let ten_k = FixedPoint::from_int(10_000);
    amount_in.checked_mul(bps)?.checked_div(ten_k)
}

// ══════════════════════════════════════════════════════════════
// Slippage computation
// ══════════════════════════════════════════════════════════════

/// Compute slippage in basis points.
///
/// slippage = ((exec_price - mid_price) / mid_price) * 10_000
/// Returns 0 if execution is at or better than mid price.
pub fn compute_slippage_bps(
    mid_price: FixedPoint,
    execution_price: FixedPoint,
) -> Result<u16> {
    if execution_price.raw <= mid_price.raw {
        return Ok(0);
    }
    let diff = execution_price.checked_sub(mid_price)?;
    let ratio = diff.checked_div(mid_price)?;
    let bps_fp = ratio.checked_mul(FixedPoint::from_int(10_000))?;
    let bps = bps_fp.to_u64()?;
    Ok(if bps > u16::MAX as u64 { u16::MAX } else { bps as u16 })
}

// ══════════════════════════════════════════════════════════════
// Core swap execution
// ══════════════════════════════════════════════════════════════

/// Execute a swap on the pool.
///
/// The off-chain SDK computes `expected_amount_out` via the torus invariant
/// and Newton solver. This function:
///   1. Validates inputs
///   2. Deducts fee from amount_in
///   3. Enforces slippage (expected_amount_out >= min_amount_out)
///   4. Applies the trade to reserves
///   5. Verifies the sphere invariant post-swap
///   6. Updates caches (alpha_cache, w_norm_sq_cache)
///   7. Updates pool statistics (total_volume, total_fees)
///   8. Computes execution price and slippage
///
/// Preconditions: pool.is_active, token_in != token_out,
/// both indices < n_assets, amounts > 0.
pub fn execute_swap(
    pool: &mut PoolState,
    token_in: usize,
    token_out: usize,
    amount_in: FixedPoint,
    expected_amount_out: FixedPoint,
    min_amount_out: FixedPoint,
) -> Result<SwapResult> {
    let n = pool.n_assets as usize;

    // 1. Validate inputs
    require!(pool.is_active, OrbitalError::PoolNotActive);
    require!(token_in != token_out, OrbitalError::SameTokenSwap);
    require!(token_in < n && token_out < n, OrbitalError::InvalidTokenIndex);
    require!(amount_in.is_positive(), OrbitalError::NegativeTradeAmount);
    require!(
        expected_amount_out.is_positive(),
        OrbitalError::NegativeTradeAmount
    );

    // 2. Fee computation
    let fee = compute_fee(amount_in, pool.fee_rate_bps)?;
    let net_amount_in = amount_in.checked_sub(fee)?;

    // 3. Slippage check
    require!(
        expected_amount_out.raw >= min_amount_out.raw,
        OrbitalError::SlippageExceeded
    );

    // 4. Snapshot pre-swap prices for slippage calculation
    let r = pool.sphere.radius;
    let old_in_reserve = pool.reserves[token_in];
    let old_out_reserve = pool.reserves[token_out];
    let mid_price_num = r.checked_sub(old_out_reserve)?;
    let mid_price_den = r.checked_sub(old_in_reserve)?;
    let mid_price = mid_price_num.checked_div(mid_price_den)?;

    // 5. Apply trade to reserves
    pool.reserves[token_in] = old_in_reserve.checked_add(net_amount_in)?;
    let new_out = old_out_reserve.checked_sub(expected_amount_out)?;
    require!(new_out.raw >= 0, OrbitalError::InsufficientLiquidity);
    pool.reserves[token_out] = new_out;

    // 6. Verify sphere invariant post-swap (key safety check)
    verify_invariant(pool)?;

    // 7. Update caches
    update_caches(pool)?;

    // 8. Update pool statistics
    pool.total_volume = pool.total_volume.checked_add(amount_in)?;
    pool.total_fees = pool.total_fees.checked_add(fee)?;

    // 9. Compute execution price and slippage
    let execution_price = amount_in.checked_div(expected_amount_out)?;
    let slippage_bps = compute_slippage_bps(mid_price, execution_price)?;

    Ok(SwapResult {
        amount_in,
        fee,
        net_amount_in,
        amount_out: expected_amount_out,
        execution_price,
        slippage_bps,
    })
}

// ══════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::core::pool::initialize_pool_reserves;
    use crate::math::sphere::{Sphere, MAX_ASSETS};

    use anchor_lang::prelude::Pubkey;

    // ── Test helpers ──

    fn unique_pubkeys(n: usize) -> Vec<Pubkey> {
        (0..n).map(|_| Pubkey::new_unique()).collect()
    }

    fn make_pool(n: u8) -> PoolState {
        PoolState {
            bump: 0,
            authority: Pubkey::new_unique(),
            sphere: Sphere {
                radius: FixedPoint::zero(),
                n,
            },
            reserves: [FixedPoint::zero(); MAX_ASSETS],
            n_assets: n,
            token_mints: [Pubkey::default(); MAX_ASSETS],
            token_vaults: [Pubkey::default(); MAX_ASSETS],
            fee_rate_bps: 30,
            total_interior_liquidity: FixedPoint::zero(),
            total_boundary_liquidity: FixedPoint::zero(),
            alpha_cache: FixedPoint::zero(),
            w_norm_sq_cache: FixedPoint::zero(),
            tick_count: 0,
            is_active: true,
            total_volume: FixedPoint::zero(),
            total_fees: FixedPoint::zero(),
            created_at: 0,
            position_count: 0,
            _reserved: [0u8; 120],
        }
    }

    fn init_pool(n: u8, deposit: i64) -> PoolState {
        let mut pool = make_pool(n);
        let mints = unique_pubkeys(n as usize);
        let vaults = unique_pubkeys(n as usize);
        let deposit_fp = FixedPoint::from_int(deposit);
        initialize_pool_reserves(&mut pool, deposit_fp, &mints, &vaults).unwrap();
        pool
    }

    /// Compute valid amount_out for n=3 equal reserves using analytical formula.
    ///
    /// At equal price point q = r(1 - 1/√n), c = r - q = r/√n.
    /// After adding d to token_in (net, after fee):
    ///   d_out = -c + √(c² + 2cd - d²)
    ///
    /// This satisfies the sphere invariant exactly.
    fn compute_valid_amount_out_n3(pool: &PoolState, net_amount_in: FixedPoint) -> FixedPoint {
        let r = pool.sphere.radius;
        let n_fp = FixedPoint::from_int(pool.n_assets as i64);
        let sqrt_n = n_fp.sqrt().unwrap();
        let c = r.checked_div(sqrt_n).unwrap(); // c = r/√n

        let d = net_amount_in;
        let c_sq = c.squared().unwrap();
        let two_cd = c.checked_mul(d).unwrap().checked_mul(FixedPoint::from_int(2)).unwrap();
        let d_sq = d.squared().unwrap();

        // radicand = c² + 2cd - d²
        let radicand = c_sq.checked_add(two_cd).unwrap().checked_sub(d_sq).unwrap();
        let sqrt_val = radicand.sqrt().unwrap();

        // d_out = -c + √(radicand)
        sqrt_val.checked_sub(c).unwrap()
    }

    // ── Fee tests ──

    #[test]
    fn test_compute_fee_30bps() {
        let amount = FixedPoint::from_int(10_000);
        let fee = compute_fee(amount, 30).unwrap();
        // 10000 * 30 / 10000 = 30
        assert!(fee.approx_eq(FixedPoint::from_int(30), FixedPoint::from_int(1)));
    }

    #[test]
    fn test_compute_fee_zero_bps() {
        let amount = FixedPoint::from_int(10_000);
        let fee = compute_fee(amount, 0).unwrap();
        assert!(fee.is_zero());
    }

    #[test]
    fn test_compute_fee_max_bps() {
        let amount = FixedPoint::from_int(10_000);
        let fee = compute_fee(amount, 10_000).unwrap();
        // 10000 * 10000 / 10000 = 10000
        assert!(fee.approx_eq(amount, FixedPoint::from_int(1)));
    }

    // ── Slippage tests ──

    #[test]
    fn test_slippage_zero_when_same_price() {
        let price = FixedPoint::from_int(1);
        assert_eq!(compute_slippage_bps(price, price).unwrap(), 0);
    }

    #[test]
    fn test_slippage_positive_when_worse() {
        let mid = FixedPoint::from_int(100);
        let exec = FixedPoint::from_int(101); // 1% worse
        let bps = compute_slippage_bps(mid, exec).unwrap();
        assert!(bps > 0 && bps <= 200); // ~100 bps ± rounding
    }

    #[test]
    fn test_slippage_zero_when_better() {
        let mid = FixedPoint::from_int(100);
        let exec = FixedPoint::from_int(99); // Better price
        assert_eq!(compute_slippage_bps(mid, exec).unwrap(), 0);
    }

    // ── execute_swap integration tests ──

    #[test]
    fn test_swap_happy_path() {
        let mut pool = init_pool(3, 1_000);
        pool.fee_rate_bps = 0; // No fee for simpler test

        let amount_in = FixedPoint::from_int(10);
        let amount_out = compute_valid_amount_out_n3(&pool, amount_in);
        let min_out = FixedPoint::from_int(1);

        let result = execute_swap(&mut pool, 0, 1, amount_in, amount_out, min_out).unwrap();

        assert!(result.amount_in.approx_eq(amount_in, FixedPoint::from_int(1)));
        assert!(result.fee.is_zero());
        assert!(result.amount_out.is_positive());
    }

    #[test]
    fn test_swap_updates_reserves() {
        let mut pool = init_pool(3, 1_000);
        pool.fee_rate_bps = 0;

        let old_in = pool.reserves[0];
        let old_out = pool.reserves[1];
        let amount_in = FixedPoint::from_int(10);
        let amount_out = compute_valid_amount_out_n3(&pool, amount_in);

        execute_swap(&mut pool, 0, 1, amount_in, amount_out, FixedPoint::from_int(1)).unwrap();

        // token_in reserve increased
        assert!(pool.reserves[0].raw > old_in.raw);
        // token_out reserve decreased
        assert!(pool.reserves[1].raw < old_out.raw);
        // uninvolved token unchanged
        assert_eq!(pool.reserves[2].raw, FixedPoint::from_int(1_000).raw);
    }

    #[test]
    fn test_swap_updates_statistics() {
        let mut pool = init_pool(3, 1_000);
        pool.fee_rate_bps = 30;

        let amount_in = FixedPoint::from_int(10);
        let fee = compute_fee(amount_in, 30).unwrap();
        let net = amount_in.checked_sub(fee).unwrap();
        let amount_out = compute_valid_amount_out_n3(&pool, net);

        execute_swap(&mut pool, 0, 1, amount_in, amount_out, FixedPoint::from_int(1)).unwrap();

        assert!(pool.total_volume.is_positive());
        assert!(pool.total_fees.is_positive());
    }

    #[test]
    fn test_swap_rejects_invariant_violation() {
        let mut pool = init_pool(3, 1_000);
        pool.fee_rate_bps = 0;

        let amount_in = FixedPoint::from_int(10);
        // Wrong amount_out (too large) → invariant violation
        let bad_out = FixedPoint::from_int(20);

        let result = execute_swap(&mut pool, 0, 1, amount_in, bad_out, FixedPoint::from_int(1));
        assert!(result.is_err());
    }

    #[test]
    fn test_swap_rejects_slippage_exceeded() {
        let mut pool = init_pool(3, 1_000);
        pool.fee_rate_bps = 0;

        let amount_in = FixedPoint::from_int(10);
        let amount_out = compute_valid_amount_out_n3(&pool, amount_in);
        // min_amount_out higher than actual → slippage exceeded
        let high_min = FixedPoint::from_int(999);

        let result = execute_swap(&mut pool, 0, 1, amount_in, amount_out, high_min);
        assert!(result.is_err());
    }

    #[test]
    fn test_swap_rejects_same_token() {
        let mut pool = init_pool(3, 1_000);
        let result = execute_swap(
            &mut pool,
            0,
            0,
            FixedPoint::from_int(10),
            FixedPoint::from_int(9),
            FixedPoint::from_int(1),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_swap_rejects_inactive_pool() {
        let mut pool = init_pool(3, 1_000);
        pool.is_active = false;
        let result = execute_swap(
            &mut pool,
            0,
            1,
            FixedPoint::from_int(10),
            FixedPoint::from_int(9),
            FixedPoint::from_int(1),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_swap_rejects_insufficient_liquidity() {
        let mut pool = init_pool(3, 1_000);
        pool.fee_rate_bps = 0;
        // Try to withdraw more than reserve
        let huge_out = FixedPoint::from_int(2_000);
        let result = execute_swap(
            &mut pool,
            0,
            1,
            FixedPoint::from_int(10),
            huge_out,
            FixedPoint::from_int(1),
        );
        assert!(result.is_err());
    }
}
