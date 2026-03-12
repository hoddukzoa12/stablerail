/**
 * Sphere Invariant Functions (TypeScript)
 *
 * Mirrors the on-chain Rust `Sphere` value object.
 * Invariant: ||r - x||^2 = r^2
 *
 * Where r = (r, r, ..., r) is the radius vector and x = (x_0, x_1, ..., x_{n-1})
 * is the reserve vector. The sphere surface defines the set of valid reserve
 * states for the Orbital AMM.
 */

import { Q6464 } from './q64-64';

/**
 * Check whether a reserve vector satisfies the sphere invariant
 * within a given tolerance.
 *
 * Computes lhs = sum((radius - reserves[i])^2) and checks if
 * |lhs - radius^2| <= tolerance.
 *
 * @param radius - Sphere radius (Q64.64)
 * @param reserves - Reserve vector (Q64.64[]), length >= nAssets
 * @param tolerance - Maximum acceptable deviation (Q64.64)
 * @returns true if the invariant holds within tolerance
 */
export function checkInvariant(
  radius: Q6464,
  reserves: Q6464[],
  tolerance: Q6464,
): boolean {
  const rSq = radius.squared();
  const distSq = distanceSquared(radius, reserves);
  return distSq.approxEq(rSq, tolerance);
}

/**
 * Compute alpha = sqrt(sum((r - x_i)^2)) / r
 *
 * Alpha represents the normalized distance of the reserve vector
 * from the sphere center. At the invariant surface, alpha = 1.0.
 *
 * @param radius - Sphere radius (Q64.64)
 * @param reserves - Reserve vector (Q64.64[])
 * @returns alpha value (Q64.64)
 */
export function computeAlpha(
  radius: Q6464,
  reserves: Q6464[],
): Q6464 {
  const distSq = distanceSquared(radius, reserves);
  const dist = distSq.sqrt();
  return dist.div(radius);
}

/**
 * Compute the distance squared from the sphere center to the reserve point.
 *
 * ||r - x||^2 = sum((radius - reserves[i])^2) for i in 0..n
 *
 * Mirrors Rust `Sphere::distance_squared`.
 *
 * @param radius - Sphere radius (Q64.64)
 * @param reserves - Reserve vector (Q64.64[])
 * @returns sum of squared differences (Q64.64)
 */
export function distanceSquared(
  radius: Q6464,
  reserves: Q6464[],
): Q6464 {
  let sum = Q6464.zero();
  for (const xi of reserves) {
    const diff = radius.sub(xi);
    const sq = diff.squared();
    sum = sum.add(sq);
  }
  return sum;
}

/**
 * Compute the invariant tolerance matching the on-chain calculation.
 *
 * tolerance = r^2 >> 24  (approximately r^2 * 6e-8)
 *
 * Provides ample headroom for Q64.64 rounding in n <= 8 dimensions
 * while rejecting economically significant deviations.
 *
 * @param radius - Sphere radius (Q64.64)
 * @returns tolerance value (Q64.64)
 */
export function invariantTolerance(radius: Q6464): Q6464 {
  const rSq = radius.squared();
  return new Q6464(rSq.raw >> 24n);
}

/**
 * Verify the sphere invariant using the default on-chain tolerance.
 *
 * Convenience wrapper that computes `invariantTolerance(radius)` and
 * delegates to `checkInvariant`.
 *
 * @param radius - Sphere radius (Q64.64)
 * @param reserves - Reserve vector (Q64.64[])
 * @returns true if the invariant holds
 */
export function verifyInvariant(
  radius: Q6464,
  reserves: Q6464[],
): boolean {
  const tolerance = invariantTolerance(radius);
  return checkInvariant(radius, reserves, tolerance);
}

/**
 * Compute the equal price point for each dimension.
 *
 * q = r * (1 - 1/sqrt(n))
 *
 * At this reserve level, all pairwise prices are 1.0.
 *
 * @param radius - Sphere radius (Q64.64)
 * @param nAssets - Number of assets in the pool
 * @returns The equal-price reserve level (Q64.64)
 */
export function equalPricePoint(radius: Q6464, nAssets: number): Q6464 {
  const nFp = Q6464.fromInt(BigInt(nAssets));
  const sqrtN = nFp.sqrt();
  const one = Q6464.one();
  const ratio = one.div(sqrtN);
  const factor = one.sub(ratio);
  return radius.mul(factor);
}

/**
 * Compute the marginal price of token i in terms of token j.
 *
 * price(i, j) = (r - x_j) / (r - x_i)
 *
 * @param radius - Sphere radius (Q64.64)
 * @param reserves - Reserve vector (Q64.64[])
 * @param i - Token index to price
 * @param j - Token index as numeraire
 * @returns Marginal price (Q64.64)
 * @throws If i == j or indices out of bounds
 */
export function marginalPrice(
  radius: Q6464,
  reserves: Q6464[],
  i: number,
  j: number,
): Q6464 {
  if (i === j) {
    throw new Error('marginalPrice: same token (i == j)');
  }
  if (i >= reserves.length || j >= reserves.length) {
    throw new Error('marginalPrice: index out of bounds');
  }
  const numerator = radius.sub(reserves[j]);
  const denominator = radius.sub(reserves[i]);
  return numerator.div(denominator);
}
