/**
 * StableRail Math SDK — Off-chain TypeScript mirror of on-chain Q64.64 math.
 *
 * This SDK provides BigInt-based fixed-point arithmetic that produces
 * results identical to the Rust on-chain program, enabling the frontend
 * to compute swap quotes, verify invariants, and display prices without
 * any floating-point drift.
 *
 * @example
 * ```ts
 * import { Q6464, computeSwapQuote, type PoolState } from '@/lib/stablerail-math';
 *
 * const pool: PoolState = { ... }; // from deserialized Anchor account
 * const amountIn = Q6464.fromTokenAmount(1_000_000n, 6); // 1 USDC
 * const quote = computeSwapQuote(pool, 0, 1, amountIn);
 * console.log('You receive:', quote.amountOutU64, 'base units');
 * ```
 */

export { Q6464 } from './q64-64';
export {
  checkInvariant,
  computeAlpha,
  distanceSquared,
  invariantTolerance,
  verifyInvariant,
  equalPricePoint,
  marginalPrice,
} from './sphere';
export {
  computeSwapQuote,
  type PoolState,
  type SwapQuote,
} from './swap-calculator';
