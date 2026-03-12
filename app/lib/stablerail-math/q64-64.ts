/**
 * Q64.64 Fixed-Point Arithmetic Library (TypeScript / BigInt)
 *
 * Mirrors the on-chain Rust `FixedPoint` struct backed by i128.
 * Upper 64 bits = integer part, lower 64 bits = fractional part.
 * All arithmetic uses BigInt exclusively -- no floating point in math paths.
 *
 * Design decisions (matching Rust):
 * - Signed: handles negative intermediate values without separate sign tracking
 * - Q64.64: 64 fractional bits for sub-atomic stablecoin precision
 * - Checked operations: throw on overflow instead of silent wrap
 */

/** Number of fractional bits in Q64.64 representation */
const FRAC_BITS = 64n;

/** The scaling factor: 2^64 */
const SCALE = 1n << FRAC_BITS;

/** Mask for extracting the fractional part */
const FRAC_MASK = SCALE - 1n;

/**
 * i128 range limits.
 * JavaScript BigInt has arbitrary precision, so we enforce i128 bounds explicitly
 * to match the Rust implementation.
 */
const I128_MAX = (1n << 127n) - 1n;
const I128_MIN = -(1n << 127n);

/** u128 max for intermediate unsigned computations */
const U128_MAX = (1n << 128n) - 1n;

/** u64 max */
const U64_MAX = (1n << 64n) - 1n;

/** i64 max */
const I64_MAX = (1n << 63n) - 1n;

/**
 * Q64.64 fixed-point number.
 *
 * The `raw` field stores the i128-equivalent value where:
 *   actual_value = raw / 2^64
 *
 * All factory methods and arithmetic enforce i128 range [-2^127, 2^127-1].
 */
export class Q6464 {
  /** Raw BigInt value in Q64.64 format. Actual value = raw / 2^64. */
  readonly raw: bigint;

  // ── Constructors ──

  /**
   * Create from a raw BigInt value already in Q64.64 format.
   * @param raw - The raw Q64.64 value (integer part << 64 | fractional part)
   */
  constructor(raw: bigint) {
    this.raw = raw;
  }

  /** Zero (0.0 in Q64.64) */
  static zero(): Q6464 {
    return new Q6464(0n);
  }

  /** One (1.0 in Q64.64) */
  static one(): Q6464 {
    return new Q6464(SCALE);
  }

  /**
   * Create from a JavaScript number (lossy for non-integer values).
   *
   * Splits the number into integer and fractional parts and encodes
   * each into Q64.64. Suitable for constants and test setup.
   * For precise token amounts, use `fromTokenAmount` instead.
   *
   * @param n - A finite JavaScript number
   * @returns Q6464 representation
   * @throws If n is not finite
   */
  static fromNumber(n: number): Q6464 {
    if (!Number.isFinite(n)) {
      throw new Error('Q6464.fromNumber: input must be finite');
    }
    const negative = n < 0;
    const abs = Math.abs(n);
    const intPart = BigInt(Math.floor(abs));
    const fracPart = abs - Number(intPart);
    // Encode fractional part: frac * 2^64, truncated to integer
    const fracScaled = BigInt(Math.round(fracPart * Number(SCALE)));
    let raw = (intPart << FRAC_BITS) + fracScaled;
    if (negative) raw = -raw;
    return new Q6464(raw);
  }

  /**
   * Create from an integer (exact).
   *
   * @param n - Integer value (BigInt)
   * @returns Q6464 representation of the integer
   */
  static fromInt(n: bigint): Q6464 {
    return new Q6464(n << FRAC_BITS);
  }

  /**
   * Create from an SPL token base-unit amount with decimal normalization.
   *
   * Transforms base-unit amounts (e.g. 1_500_000n for 1.5 USDC at 6 decimals)
   * into whole-token Q6464 values (Q6464(1.5)).
   *
   * Mirrors the Rust `FixedPoint::from_token_amount` exactly:
   *   whole = raw / 10^decimals
   *   frac  = raw % 10^decimals
   *   result = (whole << 64) + (frac << 64) / 10^decimals
   *
   * @param amount - Raw SPL token amount (u64-range)
   * @param decimals - Token mint decimal places (0..18)
   * @returns Q6464 representation of the token amount
   * @throws On overflow or invalid decimals
   */
  static fromTokenAmount(amount: bigint, decimals: number): Q6464 {
    if (decimals === 0) {
      return Q6464.checkedFromU64(amount);
    }
    if (decimals < 0 || decimals > 18) {
      throw new Error('Q6464.fromTokenAmount: decimals must be 0..18');
    }
    if (amount < 0n) {
      throw new Error('Q6464.fromTokenAmount: amount must be non-negative');
    }
    const scale = 10n ** BigInt(decimals);
    const whole = amount / scale;
    const frac = amount % scale;
    const wholeShifted = whole << FRAC_BITS;
    // frac < scale <= 10^18, frac << 64 fits comfortably in BigInt
    const fracShifted = (frac << FRAC_BITS) / scale;
    const result = wholeShifted + fracShifted;
    if (result > I128_MAX) {
      throw new Error('Q6464.fromTokenAmount: overflow');
    }
    return new Q6464(result);
  }

  /**
   * Checked conversion from u64-range value.
   * Mirrors Rust `FixedPoint::checked_from_u64`.
   *
   * @param n - Non-negative BigInt in u64 range
   * @returns Q6464 representation
   * @throws If value would overflow signed Q64.64 range
   */
  static checkedFromU64(n: bigint): Q6464 {
    if (n < 0n || n > U64_MAX) {
      throw new Error('Q6464.checkedFromU64: value out of u64 range');
    }
    const raw = n << FRAC_BITS;
    if (raw < 0n || raw > I128_MAX) {
      throw new Error('Q6464.checkedFromU64: overflow');
    }
    return new Q6464(raw);
  }

  /**
   * Create from a fraction (numerator / denominator).
   *
   * @param num - Numerator
   * @param den - Denominator (must be non-zero)
   * @returns Q6464 representation of num/den
   * @throws On division by zero
   */
  static fromFraction(num: bigint, den: bigint): Q6464 {
    if (den === 0n) {
      throw new Error('Q6464.fromFraction: division by zero');
    }
    const raw = (num << FRAC_BITS) / den;
    return new Q6464(raw);
  }

  // ── Arithmetic Operations ──

  /**
   * Addition: this + rhs
   * @throws On i128 overflow
   */
  add(rhs: Q6464): Q6464 {
    const result = this.raw + rhs.raw;
    checkI128(result, 'add');
    return new Q6464(result);
  }

  /**
   * Subtraction: this - rhs
   * @throws On i128 overflow
   */
  sub(rhs: Q6464): Q6464 {
    const result = this.raw - rhs.raw;
    checkI128(result, 'sub');
    return new Q6464(result);
  }

  /**
   * Multiplication: (this * rhs) >> 64
   *
   * Uses hi/lo splitting to avoid needing 256-bit intermediates.
   * Mirrors the Rust `checked_mul` exactly:
   *   (a_hi*2^64 + a_lo) * (b_hi*2^64 + b_lo) >> 64
   *   = a_hi*b_hi*2^64 + a_hi*b_lo + a_lo*b_hi + (a_lo*b_lo >> 64)
   *
   * @throws On overflow
   */
  mul(rhs: Q6464): Q6464 {
    const a = this.raw;
    const b = rhs.raw;

    // Handle sign
    const sign = (a < 0n) !== (b < 0n) ? -1n : 1n;
    const aAbs = bigintAbs(a);
    const bAbs = bigintAbs(b);

    const mask = (1n << 64n) - 1n;
    const aHi = aAbs >> 64n;
    const aLo = aAbs & mask;
    const bHi = bAbs >> 64n;
    const bLo = bAbs & mask;

    // (a * b) >> 64 = a_hi*b_hi*2^64 + a_hi*b_lo + a_lo*b_hi + (a_lo*b_lo >> 64)
    const hiHi = aHi * bHi;
    // Match Rust: hi_hi >= 2^64 means overflow
    if (hiHi >= (1n << 64n)) {
      throw new Error('Q6464.mul: overflow (hi_hi)');
    }
    const term1 = hiHi << 64n;
    const hiLo = aHi * bLo;
    const loHi = aLo * bHi;
    const loLoShifted = (aLo * bLo) >> 64n;

    const result = term1 + hiLo + loHi + loLoShifted;

    // Check fits in i128 positive range
    if (result > I128_MAX) {
      throw new Error('Q6464.mul: overflow');
    }

    const signed = (result * sign);
    return new Q6464(signed);
  }

  /**
   * Division: (this << 64) / rhs
   *
   * Uses split technique to avoid 256-bit overflow, matching the Rust
   * `checked_div` with its bit-by-bit long division for the remainder term.
   *
   * @throws On division by zero or overflow
   */
  div(rhs: Q6464): Q6464 {
    if (rhs.raw === 0n) {
      throw new Error('Q6464.div: division by zero');
    }

    const a = this.raw;
    const b = rhs.raw;

    // Handle sign
    const sign = (a < 0n) !== (b < 0n) ? -1n : 1n;
    const aAbs = bigintAbs(a);
    const bAbs = bigintAbs(b);

    // Split: quotient * SCALE + ((remainder * SCALE) / b_abs)
    const quotient = aAbs / bAbs;
    const remainder = aAbs % bAbs;

    // Match Rust: quotient >= 2^63 means overflow
    if (quotient >= (1n << 63n)) {
      throw new Error('Q6464.div: overflow (quotient)');
    }
    const hi = quotient << FRAC_BITS;

    // Compute (remainder << 64) / b_abs using bit-by-bit long division
    // to match the Rust implementation exactly.
    // Invariant: r < d after each subtraction step.
    const d = bAbs;
    let r = remainder;
    let q = 0n;
    for (let i = 0; i < 64; i++) {
      r <<= 1n;
      q <<= 1n;
      if (r >= d) {
        r -= d;
        q |= 1n;
      }
    }
    const lo = q;

    const result = hi + lo;
    return new Q6464(result * sign);
  }

  /**
   * Checked addition (alias for `add` -- throws on overflow).
   */
  checked_add(rhs: Q6464): Q6464 {
    return this.add(rhs);
  }

  /**
   * Checked subtraction (alias for `sub` -- throws on overflow).
   */
  checked_sub(rhs: Q6464): Q6464 {
    return this.sub(rhs);
  }

  /**
   * Checked multiplication (alias for `mul` -- throws on overflow).
   */
  checked_mul(rhs: Q6464): Q6464 {
    return this.mul(rhs);
  }

  /**
   * Checked division (alias for `div` -- throws on overflow/zero).
   */
  checked_div(rhs: Q6464): Q6464 {
    return this.div(rhs);
  }

  // ── Math Functions ──

  /**
   * Integer square root using Newton's method.
   *
   * Mirrors the Rust `FixedPoint::sqrt` exactly:
   *   sqrt(x_raw * 2^64) = isqrt(x_raw) * 2^32
   *
   * @returns sqrt(this) in Q64.64
   * @throws If this is negative
   */
  sqrt(): Q6464 {
    if (this.raw < 0n) {
      throw new Error('Q6464.sqrt: negative input');
    }
    if (this.raw === 0n) {
      return Q6464.zero();
    }

    const x = this.raw; // treated as unsigned

    // Compute bit-length
    const bits = bigintBitLength(x);
    let result = 1n << BigInt((bits + 1) >> 1);

    // Newton iterations for isqrt(x)
    for (let i = 0; i < 128; i++) {
      if (result === 0n) break;
      const next = (result + x / result) / 2n;
      if (next >= result) break;
      result = next;
    }

    // Scale by 2^32 to get Q64.64 result
    const resultRaw = result << 32n;
    return new Q6464(resultRaw);
  }

  /**
   * Square: this * this
   */
  squared(): Q6464 {
    return this.mul(this);
  }

  /**
   * Absolute value.
   * @throws On i128::MIN (cannot be negated)
   */
  abs(): Q6464 {
    if (this.raw === I128_MIN) {
      throw new Error('Q6464.abs: overflow on I128_MIN');
    }
    return new Q6464(this.raw < 0n ? -this.raw : this.raw);
  }

  /**
   * Negation.
   * @throws On i128::MIN
   */
  neg(): Q6464 {
    if (this.raw === I128_MIN) {
      throw new Error('Q6464.neg: overflow on I128_MIN');
    }
    return new Q6464(-this.raw);
  }

  // ── Comparison ──

  /** True if this > rhs */
  gt(rhs: Q6464): boolean {
    return this.raw > rhs.raw;
  }

  /** True if this < rhs */
  lt(rhs: Q6464): boolean {
    return this.raw < rhs.raw;
  }

  /** True if this == rhs (exact) */
  eq(rhs: Q6464): boolean {
    return this.raw === rhs.raw;
  }

  /** True if this >= rhs */
  gte(rhs: Q6464): boolean {
    return this.raw >= rhs.raw;
  }

  /** True if this <= rhs */
  lte(rhs: Q6464): boolean {
    return this.raw <= rhs.raw;
  }

  /** True if value is zero */
  isZero(): boolean {
    return this.raw === 0n;
  }

  /** True if value is negative */
  isNegative(): boolean {
    return this.raw < 0n;
  }

  /** True if value is positive */
  isPositive(): boolean {
    return this.raw > 0n;
  }

  /**
   * Check if approximately equal within epsilon.
   * Mirrors Rust `FixedPoint::approx_eq`.
   */
  approxEq(other: Q6464, epsilon: Q6464): boolean {
    const diff = this.raw > other.raw
      ? this.raw - other.raw
      : other.raw - this.raw;
    return diff <= epsilon.raw;
  }

  /**
   * Clamp value between min and max.
   */
  clamp(min: Q6464, max: Q6464): Q6464 {
    if (this.raw < min.raw) return min;
    if (this.raw > max.raw) return max;
    return this;
  }

  /** Minimum of this and other */
  min(other: Q6464): Q6464 {
    return this.raw <= other.raw ? this : other;
  }

  /** Maximum of this and other */
  max(other: Q6464): Q6464 {
    return this.raw >= other.raw ? this : other;
  }

  // ── Conversion ──

  /**
   * Convert to a JavaScript number (lossy, for display only).
   *
   * @returns Approximate floating-point representation
   */
  toNumber(): number {
    const intPart = this.raw >> FRAC_BITS;
    const fracPart = this.raw & FRAC_MASK;
    // For negative values, the fractional part needs special handling
    if (this.raw < 0n) {
      const absRaw = -this.raw;
      const absInt = absRaw >> FRAC_BITS;
      const absFrac = absRaw & FRAC_MASK;
      return -(Number(absInt) + Number(absFrac) / Number(SCALE));
    }
    return Number(intPart) + Number(fracPart) / Number(SCALE);
  }

  /**
   * Convert to SPL token base-unit amount using **round-half-up**.
   *
   * Mirrors Rust `FixedPoint::to_token_amount` (round_half_up = true).
   * Suitable for **deposit** paths where accuracy is priority.
   *
   * @param decimals - Token mint decimal places (0..18)
   * @returns Token base-unit amount as bigint
   * @throws If value is negative or overflows u64
   */
  toTokenAmount(decimals: number): bigint {
    return this.toTokenAmountInner(decimals, true);
  }

  /**
   * Convert to SPL token base-unit amount using **floor** rounding.
   *
   * Mirrors Rust `FixedPoint::to_token_amount_floor` (round_half_up = false).
   * Always truncates toward zero -- LP receives at most their proportional
   * share, never more. Suitable for **withdrawal** and **swap output** paths.
   *
   * @param decimals - Token mint decimal places (0..18)
   * @returns Token base-unit amount as bigint
   * @throws If value is negative or overflows u64
   */
  toTokenAmountFloor(decimals: number): bigint {
    return this.toTokenAmountInner(decimals, false);
  }

  /**
   * Convert to u64 integer (truncates fractional part).
   * Mirrors Rust `FixedPoint::to_u64`.
   */
  toU64(): bigint {
    if (this.raw < 0n) {
      throw new Error('Q6464.toU64: negative value');
    }
    return this.raw >> FRAC_BITS;
  }

  // ── Internal ──

  /**
   * Shared conversion logic for Q6464 -> SPL token base-units.
   * Mirrors Rust `to_token_amount_inner` exactly.
   */
  private toTokenAmountInner(decimals: number, roundHalfUp: boolean): bigint {
    if (decimals === 0) {
      return this.toU64();
    }
    if (decimals < 0 || decimals > 18) {
      throw new Error('Q6464.toTokenAmount: decimals must be 0..18');
    }
    if (this.raw < 0n) {
      throw new Error('Q6464.toTokenAmount: negative value');
    }
    const scale = 10n ** BigInt(decimals);
    const rawU128 = this.raw; // non-negative, treat as u128
    const whole = rawU128 >> FRAC_BITS;
    const frac = rawU128 & FRAC_MASK;
    const fracScaled = frac * scale;
    const fracRounded = roundHalfUp
      ? (fracScaled + (1n << (FRAC_BITS - 1n))) >> FRAC_BITS
      : fracScaled >> FRAC_BITS;
    const result = whole * scale + fracRounded;
    if (result > U64_MAX) {
      throw new Error('Q6464.toTokenAmount: overflow u64');
    }
    return result;
  }
}

// ── Internal helpers ──

/** Enforce i128 range on a BigInt result */
function checkI128(value: bigint, op: string): void {
  if (value > I128_MAX || value < I128_MIN) {
    throw new Error(`Q6464.${op}: i128 overflow`);
  }
}

/** BigInt absolute value (no built-in in all runtimes) */
function bigintAbs(n: bigint): bigint {
  return n < 0n ? -n : n;
}

/**
 * Compute the bit-length of a non-negative BigInt.
 * Equivalent to `128 - x.leading_zeros()` in Rust u128.
 */
function bigintBitLength(n: bigint): number {
  if (n === 0n) return 0;
  let bits = 0;
  let v = n;
  // Binary search for leading bit position
  if (v >= (1n << 64n)) { bits += 64; v >>= 64n; }
  if (v >= (1n << 32n)) { bits += 32; v >>= 32n; }
  if (v >= (1n << 16n)) { bits += 16; v >>= 16n; }
  if (v >= (1n << 8n))  { bits += 8;  v >>= 8n;  }
  if (v >= (1n << 4n))  { bits += 4;  v >>= 4n;  }
  if (v >= (1n << 2n))  { bits += 2;  v >>= 2n;  }
  if (v >= (1n << 1n))  { bits += 1;  v >>= 1n;  }
  if (v >= 1n)          { bits += 1; }
  return bits;
}
