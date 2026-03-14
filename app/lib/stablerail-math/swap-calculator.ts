/**
 * Off-chain Swap Quote Calculator
 *
 * Computes swap quotes that mirror the on-chain execution exactly.
 * Uses Q64.64 fixed-point BigInt arithmetic -- no floating point in
 * the computation path.
 *
 * The swap logic follows `execute_swap` from domain/core/swap.rs and
 * `compute_amount_out_analytical` from math/newton.rs:
 *
 *   1. Apply fee: net_in = amount_in * (10000 - fee_rate_bps) / 10000
 *   2. Update reserve_in: new_reserve_in = reserve_in + net_in
 *   3. Solve sphere invariant for new reserve_out analytically:
 *      d_out = -b + sqrt(b^2 + 2*a*d - d^2)
 *      where a = r - x_in, b = r - x_out, d = net_in
 *   4. amount_out = reserve_out - new_reserve_out
 *   5. Floor round to token base units for output
 */

import { Q6464 } from './q64-64';

/**
 * On-chain pool state as read from the PoolState account.
 *
 * All fixed-point fields use Q6464. The frontend reads these from
 * the deserialized Anchor account and constructs this interface.
 */
export interface PoolState {
  /** Sphere radius (Q64.64) */
  radius: Q6464;
  /** Reserve vector -- one per asset (Q64.64) */
  reserves: Q6464[];
  /** Number of active assets in the pool */
  nAssets: number;
  /** Fee rate in basis points (e.g. 30 = 0.30%) */
  feeRateBps: number;
  /** Decimal places for each token mint (e.g. [6, 6, 6] for USDC/USDT/PYUSD) */
  tokenDecimals: number[];
  /** Cumulative trade volume (Q64.64) */
  totalVolume: Q6464;
  /** Cumulative fees collected (Q64.64) */
  totalFees: Q6464;
  /** Number of LP positions created */
  positionCount: number;
  /** Whether the pool accepts new swaps/deposits */
  isActive: boolean;
  /** Total interior liquidity (Q64.64) — denominator for proportional withdrawals */
  totalInteriorLiquidity: Q6464;
  /** Number of ticks created for this pool */
  tickCount: number;
}

/**
 * Result of an off-chain swap quote computation.
 */
export interface SwapQuote {
  /** Gross amount deposited by user (before fee, Q64.64) */
  amountIn: Q6464;
  /** Computed amount the user receives (Q64.64, full precision) */
  amountOut: Q6464;
  /** Amount out floor-rounded to SPL token base units (u64) */
  amountOutU64: bigint;
  /** Fee deducted from amountIn (Q64.64) */
  feeAmount: Q6464;
  /** Execution exchange rate: amountIn / amountOut (display-friendly float) */
  exchangeRate: number;
  /** Price impact in basis points vs pre-swap mid-market price */
  priceImpactBps: number;
}

/**
 * Compute an off-chain swap quote that mirrors the on-chain execution.
 *
 * The returned `amountOutU64` is the floor-rounded SPL token amount that
 * should be passed as `min_amount_out` (or used for UI display). The full
 * Q64.64 `amountOut` can be used for further off-chain calculations.
 *
 * @param poolState - Current on-chain pool state
 * @param tokenInIndex - Index of the input token in the pool
 * @param tokenOutIndex - Index of the output token in the pool
 * @param amountIn - Gross input amount (Q64.64, before fee)
 * @returns SwapQuote with computed amounts, fee, exchange rate, and price impact
 * @throws If inputs are invalid or the trade exceeds available liquidity
 */
export function computeSwapQuote(
  poolState: PoolState,
  tokenInIndex: number,
  tokenOutIndex: number,
  amountIn: Q6464,
): SwapQuote {
  // ── 1. Input validation ──
  const n = poolState.nAssets;
  if (tokenInIndex === tokenOutIndex) {
    throw new Error('computeSwapQuote: tokenIn == tokenOut');
  }
  if (tokenInIndex >= n || tokenOutIndex >= n) {
    throw new Error('computeSwapQuote: token index out of bounds');
  }
  if (!amountIn.isPositive()) {
    throw new Error('computeSwapQuote: amountIn must be positive');
  }

  // ── 2. Snapshot pre-swap mid-market price ──
  // mid_price = (r - x_out) / (r - x_in)
  const r = poolState.radius;
  const oldReserveIn = poolState.reserves[tokenInIndex];
  const oldReserveOut = poolState.reserves[tokenOutIndex];

  const midPriceDen = r.sub(oldReserveIn);
  const midPriceNum = r.sub(oldReserveOut);

  let midPrice: Q6464 | null = null;
  if (!midPriceDen.isZero() && !midPriceNum.isZero()) {
    const mp = midPriceNum.div(midPriceDen);
    if (!mp.isZero()) {
      midPrice = mp;
    }
  }

  // ── 3. Fee computation ──
  // fee = amount_in * fee_rate_bps / 10_000
  const feeAmount = computeFee(amountIn, poolState.feeRateBps);
  const netAmountIn = amountIn.sub(feeAmount);

  // ── 4. Solve sphere invariant analytically ──
  // Mirrors compute_amount_out_analytical from newton.rs
  const amountOut = computeAmountOutAnalytical(
    r,
    poolState.reserves,
    tokenInIndex,
    tokenOutIndex,
    netAmountIn,
  );

  // ── 5. Floor round to token base units ──
  const outDecimals = poolState.tokenDecimals[tokenOutIndex];
  const amountOutU64 = amountOut.toTokenAmountFloor(outDecimals);

  // ── 6. Compute execution price and price impact ──
  // execution_price = amount_in / amount_out (higher = worse for buyer)
  const executionPrice = amountIn.div(amountOut);

  let priceImpactBps = 0;
  if (midPrice !== null) {
    priceImpactBps = computeSlippageBps(midPrice, executionPrice);
  }

  // Display-friendly exchange rate (lossy float)
  const exchangeRate = amountIn.toNumber() / amountOut.toNumber();

  return {
    amountIn,
    amountOut,
    amountOutU64,
    feeAmount,
    exchangeRate,
    priceImpactBps,
  };
}

// ══════════════════════════════════════════════════════════════
// Internal helpers
// ══════════════════════════════════════════════════════════════

/**
 * Compute swap fee from gross amount and fee rate.
 *
 * fee = amount_in * fee_rate_bps / 10_000
 *
 * Mirrors Rust `compute_fee` from domain/core/swap.rs.
 *
 * @internal
 */
function computeFee(amountIn: Q6464, feeRateBps: number): Q6464 {
  if (feeRateBps === 0) {
    return Q6464.zero();
  }
  const bps = Q6464.fromInt(BigInt(feeRateBps));
  const tenK = Q6464.fromInt(10_000n);
  return amountIn.mul(bps).div(tenK);
}

/**
 * Compute exact amount_out for a single-sphere swap using the closed-form
 * quadratic solution.
 *
 * Given sphere invariant sum((r - x_i)^2) = r^2, a swap that adds `netAmountIn`
 * to `tokenIn` and removes `amountOut` from `tokenOut` must satisfy:
 *
 *   (a - d)^2 + (b + d_out)^2 = a^2 + b^2
 *   where a = r - x_in, b = r - x_out, d = netAmountIn
 *
 *   Solving: d_out = -b + sqrt(b^2 + 2*a*d - d^2)
 *
 * Mirrors Rust `compute_amount_out_analytical` from math/newton.rs.
 *
 * @internal
 */
function computeAmountOutAnalytical(
  radius: Q6464,
  reserves: Q6464[],
  tokenIn: number,
  tokenOut: number,
  netAmountIn: Q6464,
): Q6464 {
  if (!netAmountIn.isPositive()) {
    throw new Error('computeAmountOutAnalytical: netAmountIn must be positive');
  }
  if (reserves[tokenOut].raw <= 0n) {
    throw new Error('computeAmountOutAnalytical: insufficient liquidity (zero reserve)');
  }

  const r = radius;
  const a = r.sub(reserves[tokenIn]);   // r - x_in
  const b = r.sub(reserves[tokenOut]);  // r - x_out
  const d = netAmountIn;

  // radicand = b^2 + 2*a*d - d^2
  const bSq = b.squared();
  const twoAD = a.mul(d).mul(Q6464.fromInt(2n));
  const dSq = d.squared();
  const radicand = bSq.add(twoAD).sub(dSq);

  if (radicand.raw < 0n) {
    throw new Error('computeAmountOutAnalytical: insufficient liquidity (negative radicand)');
  }

  const sqrtVal = radicand.sqrt();

  // d_out = sqrt(radicand) - b
  const dOut = sqrtVal.sub(b);

  if (dOut.raw <= 0n) {
    throw new Error('computeAmountOutAnalytical: insufficient liquidity (non-positive output)');
  }

  return dOut;
}

/**
 * Compute slippage in basis points.
 *
 * slippage = ((exec_price - mid_price) / mid_price) * 10_000
 * Returns 0 if execution is at or better than mid price.
 *
 * Mirrors Rust `compute_slippage_bps` from domain/core/swap.rs.
 * Saturates to 65535 (u16::MAX) on overflow, matching on-chain behavior.
 *
 * @internal
 */
function computeSlippageBps(
  midPrice: Q6464,
  executionPrice: Q6464,
): number {
  if (executionPrice.raw <= midPrice.raw) {
    return 0;
  }
  const diff = executionPrice.sub(midPrice);

  let ratio: Q6464;
  try {
    ratio = diff.div(midPrice);
  } catch {
    return 65535; // saturate on overflow
  }

  let bpsFp: Q6464;
  try {
    bpsFp = ratio.mul(Q6464.fromInt(10_000n));
  } catch {
    return 65535; // saturate on overflow
  }

  // Use toNumber() to preserve sub-bps fractional precision.
  // Stablecoin AMMs routinely produce < 1 bps price impact;
  // truncating to integer bps would hide Orbital's advantage over Curve.
  const bps = bpsFp.toNumber();
  return Math.min(bps, 65535);
}
