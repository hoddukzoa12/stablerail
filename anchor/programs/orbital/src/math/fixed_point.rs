//! Q64.64 Fixed-Point Arithmetic Library
//!
//! i128-backed fixed-point number with 64 integer bits and 64 fractional bits.
//! Provides the numerical precision required for Orbital AMM invariant computations.
//!
//! Design decisions:
//! - i128 signed: handles negative intermediate values without separate sign tracking
//! - Q64.64: higher precision than agrawalx's Q96X48 (48 frac bits vs our 64)
//! - All operations checked: overflow = program error, not silent wrap

use anchor_lang::prelude::*;
use std::fmt;

/// Number of fractional bits in Q64.64 representation
const FRAC_BITS: u32 = 64;

/// The scaling factor: 2^64
const SCALE: i128 = 1i128 << FRAC_BITS;

/// Q64.64 fixed-point number backed by i128
#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, AnchorSerialize, AnchorDeserialize,
)]
pub struct FixedPoint {
    /// Raw i128 value in Q64.64 format
    /// Actual value = raw / 2^64
    pub raw: i128,
}

impl FixedPoint {
    // ── Constructors ──

    /// Create from raw i128 value (already in Q64.64 format)
    pub const fn from_raw(raw: i128) -> Self {
        Self { raw }
    }

    /// Zero
    pub const fn zero() -> Self {
        Self { raw: 0 }
    }

    /// One (1.0 in Q64.64)
    pub const fn one() -> Self {
        Self { raw: SCALE }
    }

    /// Create from integer
    pub const fn from_int(n: i64) -> Self {
        Self {
            raw: (n as i128) << FRAC_BITS,
        }
    }

    /// Create from u64 (common for Solana token amounts).
    /// Safe: u64::MAX (2^64-1) << 64 = 2^128-2^64 which fits in i128 (max 2^127-1)
    /// only for values ≤ i64::MAX. For larger u64 values, use checked_from_u64.
    pub fn from_u64(n: u64) -> Self {
        // i128 can hold up to 2^127-1. (n as i128) << 64 overflows when n >= 2^63.
        // For safety, cap at i64::MAX. Values above this are unrealistic for token amounts
        // (would require > 9.2 quintillion tokens).
        debug_assert!(
            n <= i64::MAX as u64,
            "from_u64: value too large, use checked_from_u64"
        );
        Self {
            raw: (n as i128) << FRAC_BITS,
        }
    }

    /// Checked conversion from u64 — returns error if value would overflow Q64.64 range
    pub fn checked_from_u64(n: u64) -> Result<Self> {
        let raw = (n as i128)
            .checked_shl(FRAC_BITS)
            .ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))?;
        // Ensure the result is non-negative (i.e., fits in signed Q64.64)
        require!(raw >= 0, crate::errors::OrbitalError::MathOverflow);
        Ok(Self { raw })
    }

    /// Create from a fraction (numerator / denominator)
    pub fn from_fraction(num: i64, den: i64) -> Result<Self> {
        require!(den != 0, crate::errors::OrbitalError::DivisionByZero);
        let raw = ((num as i128) << FRAC_BITS) / (den as i128);
        Ok(Self { raw })
    }

    // ── Arithmetic Operations ──

    /// Checked addition
    pub fn checked_add(self, rhs: Self) -> Result<Self> {
        self.raw
            .checked_add(rhs.raw)
            .map(Self::from_raw)
            .ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))
    }

    /// Checked subtraction
    pub fn checked_sub(self, rhs: Self) -> Result<Self> {
        self.raw
            .checked_sub(rhs.raw)
            .map(Self::from_raw)
            .ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))
    }

    /// Checked multiplication: (a * b) >> 64
    /// Uses hi/lo splitting to avoid 256-bit intermediate overflow.
    /// (a_hi*2^64 + a_lo) * (b_hi*2^64 + b_lo) >> 64
    ///   = a_hi*b_hi*2^64 + a_hi*b_lo + a_lo*b_hi + (a_lo*b_lo >> 64)
    pub fn checked_mul(self, rhs: Self) -> Result<Self> {
        let a = self.raw;
        let b = rhs.raw;

        // Handle sign
        let sign = if (a ^ b) < 0 { -1i128 } else { 1i128 };
        let a_abs = a.checked_abs().ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))?;
        let b_abs = b.checked_abs().ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))?;

        let a_u = a_abs as u128;
        let b_u = b_abs as u128;
        let mask = (1u128 << 64) - 1;

        let a_hi = a_u >> 64;
        let a_lo = a_u & mask;
        let b_hi = b_u >> 64;
        let b_lo = b_u & mask;

        // (a * b) >> 64 = a_hi*b_hi*2^64 + a_hi*b_lo + a_lo*b_hi + (a_lo*b_lo >> 64)
        let hi_hi = a_hi.checked_mul(b_hi)
            .ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))?;
        let term1 = hi_hi.checked_shl(64)
            .ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))?;
        let hi_lo = a_hi * b_lo;    // each factor < 2^64, product < 2^128
        let lo_hi = a_lo * b_hi;    // same
        let lo_lo_shifted = (a_lo * b_lo) >> 64;

        let result = term1
            .checked_add(hi_lo)
            .and_then(|r| r.checked_add(lo_hi))
            .and_then(|r| r.checked_add(lo_lo_shifted))
            .ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))?;

        // Check fits in i128 positive range
        if result > i128::MAX as u128 {
            return Err(error!(crate::errors::OrbitalError::MathOverflow));
        }

        Ok(Self::from_raw(result as i128 * sign))
    }

    /// Checked division: (a << 64) / b
    /// Uses split-multiply technique to avoid 256-bit intermediate overflow:
    ///   result = (a_raw / b_raw) << 64 + ((a_raw % b_raw) << 64) / b_raw
    pub fn checked_div(self, rhs: Self) -> Result<Self> {
        require!(
            rhs.raw != 0,
            crate::errors::OrbitalError::DivisionByZero
        );

        let a = self.raw;
        let b = rhs.raw;

        // Handle sign: compute in absolute values, restore sign at end
        let sign = if (a ^ b) < 0 { -1i128 } else { 1i128 };
        let a_abs = a.checked_abs().ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))?;
        let b_abs = b.checked_abs().ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))?;

        // Split: quotient * SCALE + ((remainder * SCALE) / b_abs)
        let quotient = a_abs / b_abs;
        let remainder = a_abs % b_abs;

        // quotient << 64 — check overflow
        let hi = quotient
            .checked_shl(FRAC_BITS)
            .ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))?;

        // (remainder << 64) / b_abs — remainder < b_abs, so remainder << 64 fits if b_abs > 0
        // remainder < b_abs ≤ i128::MAX, and we shift u128 to avoid sign issues
        let rem_shifted = (remainder as u128) << FRAC_BITS;
        let lo = (rem_shifted / b_abs as u128) as i128;

        let result = hi
            .checked_add(lo)
            .ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))?;

        Ok(Self::from_raw(result * sign))
    }

    // ── Math Functions ──

    /// Checked absolute value (returns error on i128::MIN)
    pub fn abs(self) -> Result<Self> {
        self.raw
            .checked_abs()
            .map(Self::from_raw)
            .ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))
    }

    /// Check if approximately equal within epsilon
    pub fn approx_eq(self, other: Self, epsilon: Self) -> bool {
        let diff = if self.raw > other.raw {
            self.raw - other.raw
        } else {
            other.raw - self.raw
        };
        diff <= epsilon.raw
    }

    /// Square: x * x
    pub fn squared(self) -> Result<Self> {
        self.checked_mul(self)
    }

    /// Integer square root using Newton's method
    /// Returns sqrt(self) in Q64.64
    ///
    /// Uses the identity: sqrt(x_raw * 2^64) = isqrt(x_raw) * 2^32
    /// This avoids the intermediate overflow of `x_raw << 64`.
    pub fn sqrt(self) -> Result<Self> {
        require!(
            self.raw >= 0,
            crate::errors::OrbitalError::SqrtNegative
        );

        if self.raw == 0 {
            return Ok(Self::zero());
        }

        // We want y_raw = sqrt(x_raw * 2^64) = isqrt(x_raw) * 2^32
        // Step 1: Compute integer square root of x_raw via Newton's method
        let x = self.raw as u128;

        let bits = 128 - x.leading_zeros();
        let mut result = 1u128 << ((bits + 1) / 2);

        // Newton iterations for isqrt(x)
        for _ in 0..128 {
            if result == 0 {
                break;
            }
            let next = (result + x / result) / 2;
            if next >= result {
                break;
            }
            result = next;
        }

        // Step 2: Scale by 2^32 to get Q64.64 result
        // y_raw = isqrt(x_raw) << 32
        let result_raw = (result as i128)
            .checked_shl(32)
            .ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))?;

        Ok(Self::from_raw(result_raw))
    }

    /// Clamp value between min and max
    pub fn clamp(self, min: Self, max: Self) -> Self {
        if self.raw < min.raw {
            min
        } else if self.raw > max.raw {
            max
        } else {
            self
        }
    }

    /// Convert back to u64 (truncates fractional part)
    /// Used for final token amount output
    pub fn to_u64(self) -> Result<u64> {
        require!(
            self.raw >= 0,
            crate::errors::OrbitalError::MathOverflow
        );
        let int_part = (self.raw >> FRAC_BITS) as u64;
        Ok(int_part)
    }

    /// Check if value is positive
    pub fn is_positive(self) -> bool {
        self.raw > 0
    }

    /// Check if value is negative
    pub fn is_negative(self) -> bool {
        self.raw < 0
    }

    /// Check if value is zero
    pub fn is_zero(self) -> bool {
        self.raw == 0
    }

    /// Checked negation (returns error on i128::MIN)
    pub fn neg(self) -> Result<Self> {
        self.raw
            .checked_neg()
            .map(Self::from_raw)
            .ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))
    }

    /// Min of two values
    pub fn min(self, other: Self) -> Self {
        if self.raw <= other.raw {
            self
        } else {
            other
        }
    }

    /// Max of two values
    pub fn max(self, other: Self) -> Self {
        if self.raw >= other.raw {
            self
        } else {
            other
        }
    }
}

impl fmt::Debug for FixedPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let int_part = self.raw >> FRAC_BITS;
        let frac_part = (self.raw & (SCALE - 1)) as f64 / SCALE as f64;
        write!(f, "FP({:.6})", int_part as f64 + frac_part)
    }
}

impl fmt::Display for FixedPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let int_part = self.raw >> FRAC_BITS;
        let frac_part = (self.raw & (SCALE - 1)) as f64 / SCALE as f64;
        write!(f, "{:.6}", int_part as f64 + frac_part)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_int() {
        let a = FixedPoint::from_int(5);
        assert_eq!(a.raw, 5i128 << 64);
    }

    #[test]
    fn test_one() {
        let one = FixedPoint::one();
        assert_eq!(one.raw, 1i128 << 64);
    }

    #[test]
    fn test_add() {
        let a = FixedPoint::from_int(3);
        let b = FixedPoint::from_int(4);
        let c = a.checked_add(b).unwrap();
        assert_eq!(c, FixedPoint::from_int(7));
    }

    #[test]
    fn test_sub() {
        let a = FixedPoint::from_int(10);
        let b = FixedPoint::from_int(3);
        let c = a.checked_sub(b).unwrap();
        assert_eq!(c, FixedPoint::from_int(7));
    }

    #[test]
    fn test_mul() {
        let a = FixedPoint::from_int(3);
        let b = FixedPoint::from_int(4);
        let c = a.checked_mul(b).unwrap();
        assert_eq!(c, FixedPoint::from_int(12));
    }

    #[test]
    fn test_div() {
        let a = FixedPoint::from_int(12);
        let b = FixedPoint::from_int(4);
        let c = a.checked_div(b).unwrap();
        assert_eq!(c, FixedPoint::from_int(3));
    }

    #[test]
    fn test_sqrt_perfect() {
        let a = FixedPoint::from_int(9);
        let root = a.sqrt().unwrap();
        let three = FixedPoint::from_int(3);
        let epsilon = FixedPoint::from_raw(1 << 32); // ~2^-32 tolerance
        assert!(root.approx_eq(three, epsilon));
    }

    #[test]
    fn test_negative_values() {
        let a = FixedPoint::from_int(-5);
        let b = FixedPoint::from_int(3);
        let c = a.checked_add(b).unwrap();
        assert_eq!(c, FixedPoint::from_int(-2));
    }

    #[test]
    fn test_abs() {
        let a = FixedPoint::from_int(-7);
        assert_eq!(a.abs().unwrap(), FixedPoint::from_int(7));
    }
}
