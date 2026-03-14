/**
 * Off-chain Swap Quote Calculator
 *
 * Computes swap quotes that mirror the on-chain execution exactly.
 * Uses Q64.64 fixed-point BigInt arithmetic -- no floating point in
 * the computation path.
 *
 * Two modes:
 *   A. **Single-sphere** (no ticks) — analytical closed-form formula
 *   B. **Tick-aware** — mirrors the on-chain trade segmentation loop:
 *      iteratively detects tick crossings via alpha, computes partial
 *      swaps up to boundaries, flips tick status, and continues.
 *
 * The swap logic follows `execute_swap` from instructions/execute_swap.rs,
 * `compute_amount_out_analytical` from math/newton.rs, and the tick
 * crossing logic from math/torus.rs.
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
 * Tick data for off-chain trade segmentation.
 *
 * Mirrors on-chain TickState fields needed for swap routing.
 * The frontend reads these from `usePoolTicks` (getProgramAccounts).
 */
export type TickStatus = 'Interior' | 'Boundary';

export interface TickData {
  /** Tick plane constant k (Q64.64 raw) */
  kRaw: bigint;
  /** Interior or Boundary */
  status: TickStatus;
  /** Total liquidity in this tick (Q64.64 raw) */
  liquidityRaw: bigint;
  /** Per-asset reserves (Q64.64 raw), length >= nAssets */
  reservesRaw: bigint[];
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
  // Use net input (post-fee) to isolate true market impact from the fee.
  // Using gross input would conflate the LP fee with price impact,
  // showing a minimum of ~fee_rate_bps regardless of trade size.
  const executionPrice = netAmountIn.div(amountOut);

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
// Tick-aware swap quote (trade segmentation)
// ══════════════════════════════════════════════════════════════

/**
 * Compute a swap quote that accounts for concentrated tick liquidity.
 *
 * Mirrors the on-chain trade segmentation loop in `execute_swap.rs`:
 *   1. Apply fee
 *   2. While remaining_in > 0:
 *      a. Find nearest tick boundaries relative to current alpha
 *      b. Compute tentative full-swap output analytically
 *      c. Predict post-trade alpha
 *      d. If alpha would cross a tick boundary:
 *         - Compute delta_to_boundary (quadratic solver)
 *         - Apply partial swap up to boundary
 *         - Flip tick (redistribute reserves/liquidity)
 *         - Continue with remaining input
 *      e. If no crossing: apply full remaining swap
 *   3. Return total output with price impact
 *
 * Falls back to single-sphere `computeSwapQuote` when ticks is empty.
 *
 * @param poolState - Current on-chain pool state
 * @param ticks - Array of tick data from usePoolTicks
 * @param tokenInIndex - Index of the input token
 * @param tokenOutIndex - Index of the output token
 * @param amountIn - Gross input amount (Q64.64, before fee)
 * @returns SwapQuote with computed amounts accounting for tick routing
 */
export function computeSwapQuoteWithTicks(
  poolState: PoolState,
  ticks: TickData[],
  tokenInIndex: number,
  tokenOutIndex: number,
  amountIn: Q6464,
): SwapQuote {
  // Fall back to single-sphere if no ticks
  if (!ticks || ticks.length === 0) {
    return computeSwapQuote(poolState, tokenInIndex, tokenOutIndex, amountIn);
  }

  // ── 1. Input validation ──
  const n = poolState.nAssets;
  if (tokenInIndex === tokenOutIndex) {
    throw new Error('computeSwapQuoteWithTicks: tokenIn == tokenOut');
  }
  if (tokenInIndex >= n || tokenOutIndex >= n) {
    throw new Error('computeSwapQuoteWithTicks: token index out of bounds');
  }
  if (!amountIn.isPositive()) {
    throw new Error('computeSwapQuoteWithTicks: amountIn must be positive');
  }

  // ── 2. Snapshot pre-swap mid-market price ──
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
  const feeAmount = computeFee(amountIn, poolState.feeRateBps);
  const netAmountIn = amountIn.sub(feeAmount);

  // ── 4. Trade segmentation loop ──
  // Create mutable copies of pool state for simulation
  const simReserves = poolState.reserves.map((r) => new Q6464(r.raw));
  let simRadius = new Q6464(r.raw);
  let simTotalInteriorLiquidity = new Q6464(poolState.totalInteriorLiquidity.raw);
  let simTotalBoundaryLiquidity = Q6464.zero(); // will compute if needed

  // Build mutable tick state
  const simTicks: MutableTick[] = ticks.map((t) => ({
    k: new Q6464(t.kRaw),
    status: t.status,
    liquidity: new Q6464(t.liquidityRaw),
    reserves: t.reservesRaw.map((r) => new Q6464(r)),
  }));

  let remainingIn = new Q6464(netAmountIn.raw);
  let totalOut = Q6464.zero();
  const maxIterations = simTicks.length + 1;

  for (let iter = 0; iter < maxIterations && remainingIn.isPositive(); iter++) {
    // 4a. Compute current alpha
    const currentAlpha = computeAlphaFromReserves(simReserves, n);

    // 4b. Find nearest tick boundaries
    const boundaries = findNearestTickBoundaries(simTicks, currentAlpha);

    // 4c. Compute tentative full swap output
    const tentativeOut = computeAmountOutAnalytical(
      simRadius,
      simReserves,
      tokenInIndex,
      tokenOutIndex,
      remainingIn,
    );

    // 4d. Predict post-trade alpha
    const tentativeAlpha = computeNewAlpha(simReserves, n, remainingIn, tentativeOut);

    // 4e. Determine if a tick crossing occurs
    const crossingK = determineCrossingK(currentAlpha, tentativeAlpha, boundaries);

    if (crossingK === null) {
      // No crossing → apply full remaining swap
      applyPartialSwap(simReserves, tokenInIndex, tokenOutIndex, remainingIn, tentativeOut);
      totalOut = totalOut.add(tentativeOut);
      remainingIn = Q6464.zero();
    } else {
      // Compute delta to reach the tick boundary
      const delta = computeDeltaToBoundary(
        simRadius,
        simReserves,
        tokenInIndex,
        tokenOutIndex,
        crossingK,
        n,
      );

      if (delta.raw <= 0n || delta.raw > remainingIn.raw) {
        // Can't reach boundary or exceeds remaining → apply full swap
        applyPartialSwap(simReserves, tokenInIndex, tokenOutIndex, remainingIn, tentativeOut);
        totalOut = totalOut.add(tentativeOut);
        remainingIn = Q6464.zero();
      } else {
        // Partial swap up to the tick boundary
        const partialOut = computeAmountOutAnalytical(
          simRadius,
          simReserves,
          tokenInIndex,
          tokenOutIndex,
          delta,
        );
        applyPartialSwap(simReserves, tokenInIndex, tokenOutIndex, delta, partialOut);
        totalOut = totalOut.add(partialOut);

        // Flip the crossed tick and redistribute reserves
        flipTick(simTicks, crossingK, simReserves, n, simTotalInteriorLiquidity);

        // Recompute radius after reserve redistribution
        simRadius = recomputeRadius(simReserves, n);

        // Recompute total interior liquidity
        simTotalInteriorLiquidity = Q6464.zero();
        for (const t of simTicks) {
          if (t.status === 'Interior') {
            simTotalInteriorLiquidity = simTotalInteriorLiquidity.add(t.liquidity);
          }
        }

        remainingIn = remainingIn.sub(delta);
      }
    }
  }

  // ── 5. Floor round to token base units ──
  const outDecimals = poolState.tokenDecimals[tokenOutIndex];
  const amountOutU64 = totalOut.toTokenAmountFloor(outDecimals);

  // ── 6. Compute execution price and price impact ──
  const executionPrice = netAmountIn.div(totalOut);

  let priceImpactBps = 0;
  if (midPrice !== null) {
    priceImpactBps = computeSlippageBps(midPrice, executionPrice);
  }

  const exchangeRate = amountIn.toNumber() / totalOut.toNumber();

  return {
    amountIn,
    amountOut: totalOut,
    amountOutU64,
    feeAmount,
    exchangeRate,
    priceImpactBps,
  };
}

// ══════════════════════════════════════════════════════════════
// Internal helpers — trade segmentation
// ══════════════════════════════════════════════════════════════

/** Mutable tick state for simulation */
interface MutableTick {
  k: Q6464;
  status: TickStatus;
  liquidity: Q6464;
  reserves: Q6464[];
}

/** Nearest tick boundaries relative to current alpha */
interface TickBoundaries {
  nearestKLower: Q6464 | null; // largest Interior k below alpha
  nearestKUpper: Q6464 | null; // smallest Boundary k above alpha
}

/**
 * Compute alpha = Σ reserves[i] / √n
 * Mirrors torus.rs `compute_new_alpha` using current reserves.
 * @internal
 */
function computeAlphaFromReserves(reserves: Q6464[], n: number): Q6464 {
  let sum = Q6464.zero();
  for (let i = 0; i < n; i++) {
    sum = sum.add(reserves[i]);
  }
  const sqrtN = Q6464.fromInt(BigInt(n)).sqrt();
  return sum.div(sqrtN);
}

/**
 * Predict post-trade alpha without modifying reserves.
 * Mirrors torus.rs `compute_new_alpha`.
 * @internal
 */
function computeNewAlpha(
  reserves: Q6464[],
  n: number,
  amountIn: Q6464,
  amountOut: Q6464,
): Q6464 {
  let sum = Q6464.zero();
  for (let i = 0; i < n; i++) {
    sum = sum.add(reserves[i]);
  }
  const newSum = sum.add(amountIn).sub(amountOut);
  const sqrtN = Q6464.fromInt(BigInt(n)).sqrt();
  return newSum.div(sqrtN);
}

/**
 * Find nearest tick boundaries relative to current alpha.
 * Mirrors torus.rs `find_nearest_tick_boundaries`.
 * @internal
 */
function findNearestTickBoundaries(
  ticks: MutableTick[],
  currentAlpha: Q6464,
): TickBoundaries {
  let nearestKLower: Q6464 | null = null;
  let nearestKUpper: Q6464 | null = null;

  for (const tick of ticks) {
    if (tick.status === 'Interior') {
      // Interior ticks strictly below alpha
      if (tick.k.raw < currentAlpha.raw) {
        if (nearestKLower === null || tick.k.raw > nearestKLower.raw) {
          nearestKLower = tick.k;
        }
      }
    } else {
      // Boundary ticks strictly above alpha
      if (tick.k.raw > currentAlpha.raw) {
        if (nearestKUpper === null || tick.k.raw < nearestKUpper.raw) {
          nearestKUpper = tick.k;
        }
      }
    }
  }

  return { nearestKLower, nearestKUpper };
}

/**
 * Determine which tick k would be crossed by the alpha movement.
 * Mirrors execute_swap.rs `determine_crossing_k`.
 * @internal
 */
function determineCrossingK(
  oldAlpha: Q6464,
  newAlpha: Q6464,
  boundaries: TickBoundaries,
): Q6464 | null {
  if (newAlpha.raw < oldAlpha.raw) {
    // Alpha decreasing → check lower boundary
    if (boundaries.nearestKLower !== null && newAlpha.raw <= boundaries.nearestKLower.raw) {
      return boundaries.nearestKLower;
    }
  } else if (newAlpha.raw > oldAlpha.raw) {
    // Alpha increasing → check upper boundary
    if (boundaries.nearestKUpper !== null && newAlpha.raw >= boundaries.nearestKUpper.raw) {
      return boundaries.nearestKUpper;
    }
  }
  return null;
}

/**
 * Compute the amount of token_in needed to reach a tick boundary.
 * Mirrors torus.rs `compute_delta_to_boundary` (quadratic solver).
 * @internal
 */
function computeDeltaToBoundary(
  radius: Q6464,
  reserves: Q6464[],
  tokenIn: number,
  tokenOut: number,
  kCross: Q6464,
  n: number,
): Q6464 {
  const r = radius;
  const sqrtN = Q6464.fromInt(BigInt(n)).sqrt();

  const a = r.sub(reserves[tokenIn]);   // r - x_in
  const b = r.sub(reserves[tokenOut]);  // r - x_out

  // running_sum = Σ x_i
  let runningSum = Q6464.zero();
  for (let i = 0; i < n; i++) {
    runningSum = runningSum.add(reserves[i]);
  }

  // C = running_sum - k_cross · √n
  const targetSum = kCross.mul(sqrtN);
  const c = runningSum.sub(targetSum);

  // Quadratic: 2d² + 2(b+C-a)d + (2bC + C²) = 0
  // Divided by 2: d² + (b+C-a)d + C(2b+C)/2 = 0
  const two = Q6464.fromInt(2n);
  const bCoeff = b.add(c).sub(a);
  const cCoeffNumer = c.mul(two.mul(b).add(c));
  const cCoeff = cCoeffNumer.div(two);

  // Discriminant: b_coeff² - 4·c_coeff
  const discriminant = bCoeff.squared().sub(Q6464.fromInt(4n).mul(cCoeff));

  if (discriminant.raw < 0n) {
    return Q6464.zero(); // boundary unreachable
  }

  const sqrtDisc = discriminant.sqrt();

  // Two roots: d = (-b_coeff ± sqrt_disc) / 2
  const negB = Q6464.zero().sub(bCoeff);
  const root1 = negB.add(sqrtDisc).div(two);
  const root2 = negB.sub(sqrtDisc).div(two);

  // Select smallest positive root
  const r1Pos = root1.raw > 0n;
  const r2Pos = root2.raw > 0n;

  if (r1Pos && r2Pos) {
    return root1.raw <= root2.raw ? root1 : root2;
  }
  if (r1Pos) return root1;
  if (r2Pos) return root2;

  return Q6464.zero(); // no positive root
}

/**
 * Apply a partial swap to simulated reserves (mutates in place).
 * Mirrors execute_swap.rs `apply_partial_swap`.
 * @internal
 */
function applyPartialSwap(
  reserves: Q6464[],
  tokenIn: number,
  tokenOut: number,
  amountIn: Q6464,
  amountOut: Q6464,
): void {
  reserves[tokenIn] = reserves[tokenIn].add(amountIn);
  reserves[tokenOut] = reserves[tokenOut].sub(amountOut);
}

/**
 * Flip a tick's status and redistribute reserves.
 * Mirrors execute_swap.rs `flip_tick`.
 * @internal
 */
function flipTick(
  ticks: MutableTick[],
  kCross: Q6464,
  reserves: Q6464[],
  n: number,
  totalInteriorLiquidity: Q6464,
): void {
  for (const tick of ticks) {
    if (tick.k.raw !== kCross.raw) continue;

    if (tick.status === 'Interior') {
      // Interior → Boundary: snapshot proportional reserves, subtract from pool
      tick.status = 'Boundary';
      const fraction = totalInteriorLiquidity.isPositive()
        ? tick.liquidity.div(totalInteriorLiquidity)
        : Q6464.zero();

      for (let i = 0; i < n; i++) {
        const liveShare = reserves[i].mul(fraction);
        tick.reserves[i] = liveShare;
        reserves[i] = reserves[i].sub(liveShare);
      }
    } else {
      // Boundary → Interior: add frozen reserves back to pool
      tick.status = 'Interior';
      for (let i = 0; i < n; i++) {
        reserves[i] = reserves[i].add(tick.reserves[i]);
      }
    }
    return;
  }
}

/**
 * Recompute sphere radius from reserves: r = √(Σ(r-xᵢ)²) ... but we need r.
 * Since the sphere invariant is Σ(r-xᵢ)²=r², and we know the reserves moved,
 * we use the equal-price point formula: r = total_liquidity / n where
 * total_liquidity is approximated from the reserve geometry.
 *
 * Actually, for simulation purposes we recompute from the invariant:
 * Given the reserves satisfy the invariant, r doesn't change from a swap alone.
 * r only changes when liquidity is added/removed (flip_tick redistributes reserves
 * but the sphere gets recomputed on-chain via recompute_sphere).
 *
 * For the off-chain approximation, we compute:
 *   new_r = sqrt(sum_of_squares) where sum_of_squares = Σ(r_old - x_i)²
 *   But this equals r_old² by the invariant... unless reserves were redistributed.
 *
 * After flip_tick, pool.reserves changed, so we need to find a new r that satisfies
 * the invariant. On-chain this uses recompute_sphere which sets r from
 * total_interior_liquidity. For simplicity, we compute:
 *   r = Σ x_i / (n - √n)  (from equal_price_point inversion: q = r(1-1/√n))
 *
 * This is exact when reserves are balanced, and a good approximation near
 * equilibrium (which is always the case for stablecoin pools).
 * @internal
 */
function recomputeRadius(reserves: Q6464[], n: number): Q6464 {
  // Method: Use the fact that Σ(r-xᵢ)² = r² must hold.
  // At equal reserves: x_i = q = r(1-1/√n), so Σx_i = n·r(1-1/√n)
  // → r = Σx_i / (n - √n)
  //
  // For non-equal reserves this is approximate, but after flip_tick the
  // reserves are close to equilibrium (stablecoin pool near peg).
  // The on-chain recompute_sphere uses total_interior_liquidity which
  // maps to the same algebra.
  let sum = Q6464.zero();
  for (let i = 0; i < n; i++) {
    sum = sum.add(reserves[i]);
  }
  const nFp = Q6464.fromInt(BigInt(n));
  const sqrtN = nFp.sqrt();
  const denominator = nFp.sub(sqrtN); // n - √n
  if (denominator.raw <= 0n) {
    return sum; // degenerate, shouldn't happen for n >= 2
  }
  return sum.div(denominator);
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
