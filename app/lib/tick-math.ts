/**
 * Frontend tick math — lightweight number-based preview calculations.
 *
 * These mirror the on-chain Tick value object (math/tick.rs) but use
 * JavaScript number types for real-time UI previews. NOT suitable for
 * transaction construction (use Q6464 for that).
 *
 * Formulas (from Paradigm Orbital whitepaper):
 *   k_min = r · (√n - 1)
 *   k_max = r · (n - 1) / √n
 *   D = √(k²·n - n·((n-1)·r - k·√n)²)
 *   x_min = (k·√n - D) / n
 *   x_max = min(r, (k·√n + D) / n)
 *   depeg_price = (r - x_max) / (r - x_min) × (n-1)/(n-1)  [simplified]
 *   capital_efficiency = x_base / (x_base - x_min)
 *   boundary_sphere_radius = √(r² - (k - r·√n)²)
 */

/** Compute k_min for a given sphere (lower bound for valid k). */
export function computeKMin(radius: number, n: number): number {
  return radius * (Math.sqrt(n) - 1);
}

/** Compute k_max for a given sphere (upper bound for valid k). */
export function computeKMax(radius: number, n: number): number {
  return (radius * (n - 1)) / Math.sqrt(n);
}

/** Check if a k value is within valid bounds (strict inequality). */
export function isKValid(k: number, radius: number, n: number): boolean {
  const kMin = computeKMin(radius, n);
  const kMax = computeKMax(radius, n);
  return k > kMin && k < kMax;
}

/**
 * Compute the shared discriminant D used by x_min, x_max, depeg_price.
 * D = √(k²·n - n·((n-1)·r - k·√n)²)
 */
function computeDiscriminant(
  k: number,
  radius: number,
  n: number,
): number {
  const sqrtN = Math.sqrt(n);
  const inner = (n - 1) * radius - k * sqrtN;
  const radicand = k * k * n - n * inner * inner;
  if (radicand < 0) return 0; // clamp negative radicand (rounding)
  return Math.sqrt(radicand);
}

/** Compute x_min: minimum reserve within this tick. */
export function computeXMin(
  k: number,
  radius: number,
  n: number,
): number {
  const sqrtN = Math.sqrt(n);
  const d = computeDiscriminant(k, radius, n);
  return (k * sqrtN - d) / n;
}

/** Compute x_max: maximum reserve within this tick (clamped to r). */
export function computeXMax(
  k: number,
  radius: number,
  n: number,
): number {
  const sqrtN = Math.sqrt(n);
  const d = computeDiscriminant(k, radius, n);
  return Math.min(radius, (k * sqrtN + d) / n);
}

/**
 * Compute depeg price at maximum reserve imbalance.
 *
 * At x_max, one asset is at its maximum. The marginal price of
 * that asset vs others: price = (r - x_other) / (r - x_max).
 * Since all other (n-1) assets are symmetric at the boundary:
 *   x_other = (k·√n - x_max) / (n-1)    (from alpha constraint)
 *   depeg_price = (r - x_other) / (r - x_max)
 */
export function computeDepegPrice(
  k: number,
  radius: number,
  n: number,
): number {
  const xMax = computeXMax(k, radius, n);
  const sqrtN = Math.sqrt(n);
  // Other assets: from alpha = k, sum of all reserves = k * sqrtN
  // x_other = (k * sqrtN - x_max) / (n - 1)
  const xOther = (k * sqrtN - xMax) / (n - 1);
  const denominator = radius - xMax;
  if (denominator <= 0) return Infinity;
  return (radius - xOther) / denominator;
}

/**
 * Compute capital efficiency: how much more concentrated vs full range.
 *
 * capital_efficiency = x_base / (x_base - x_min)
 * where x_base = equal price point = r · (1 - 1/√n)
 */
export function computeCapitalEfficiency(
  k: number,
  radius: number,
  n: number,
): number {
  const xBase = radius * (1 - 1 / Math.sqrt(n));
  const xMin = computeXMin(k, radius, n);
  const denominator = xBase - xMin;
  if (denominator <= 0) return Infinity;
  return xBase / denominator;
}

/**
 * Compute boundary sphere radius: s = √(r² - (k - r·√n)²)
 */
export function computeBoundarySphereRadius(
  k: number,
  radius: number,
  n: number,
): number {
  const sqrtN = Math.sqrt(n);
  const diff = k - radius * sqrtN;
  const radicand = radius * radius - diff * diff;
  if (radicand < 0) return 0;
  return Math.sqrt(radicand);
}

/**
 * Compute all tick properties at once for UI preview.
 * Returns null if k is out of valid bounds.
 */
export interface TickPreview {
  k: number;
  kMin: number;
  kMax: number;
  xMin: number;
  xMax: number;
  depegPrice: number;
  capitalEfficiency: number;
  boundarySphereRadius: number;
  /** k position as percentage between kMin and kMax (0-100) */
  kPercent: number;
}

export function computeTickPreview(
  k: number,
  radius: number,
  n: number,
): TickPreview | null {
  const kMin = computeKMin(radius, n);
  const kMax = computeKMax(radius, n);

  if (k <= kMin || k >= kMax) return null;

  const kPercent = ((k - kMin) / (kMax - kMin)) * 100;

  return {
    k,
    kMin,
    kMax,
    xMin: computeXMin(k, radius, n),
    xMax: computeXMax(k, radius, n),
    depegPrice: computeDepegPrice(k, radius, n),
    capitalEfficiency: computeCapitalEfficiency(k, radius, n),
    boundarySphereRadius: computeBoundarySphereRadius(k, radius, n),
    kPercent,
  };
}
