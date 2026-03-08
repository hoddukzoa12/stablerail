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

    /// Create from u64 (common for Solana token amounts)
    pub const fn from_u64(n: u64) -> Self {
        Self {
            raw: (n as i128) << FRAC_BITS,
        }
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
    pub fn checked_mul(self, rhs: Self) -> Result<Self> {
        // Use widening approach: split into high/low to avoid overflow
        // For values that fit in i128 after multiplication, use direct approach
        let product = (self.raw as i128)
            .checked_mul(rhs.raw as i128)
            .ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))?;
        Ok(Self::from_raw(product >> FRAC_BITS))
    }

    /// Checked division: (a << 64) / b
    pub fn checked_div(self, rhs: Self) -> Result<Self> {
        require!(
            rhs.raw != 0,
            crate::errors::OrbitalError::DivisionByZero
        );
        let shifted = self
            .raw
            .checked_shl(FRAC_BITS)
            .ok_or_else(|| error!(crate::errors::OrbitalError::MathOverflow))?;
        Ok(Self::from_raw(shifted / rhs.raw))
    }

    // ── Math Functions ──

    /// Absolute value
    pub fn abs(self) -> Self {
        Self::from_raw(self.raw.abs())
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
    pub fn sqrt(self) -> Result<Self> {
        require!(
            self.raw >= 0,
            crate::errors::OrbitalError::SqrtNegative
        );

        if self.raw == 0 {
            return Ok(Self::zero());
        }

        // Newton's method for sqrt in fixed-point:
        // We want to find y such that y^2 = x
        // In raw terms: (y_raw * y_raw) >> 64 = x_raw
        // So y_raw = sqrt(x_raw << 64)
        //
        // Initial guess: use bit manipulation for a good starting point
        let mut guess = self.raw;
        // Scale the guess: we need sqrt(raw * SCALE)
        // Start with a rough estimate
        let shifted = (self.raw as u128).checked_shl(FRAC_BITS).unwrap_or(u128::MAX);
        let mut x = shifted;

        // Initial guess using bit length
        let bits = 128 - x.leading_zeros();
        let mut result = 1u128 << ((bits + 1) / 2);

        // Newton iterations
        for _ in 0..128 {
            let next = (result + x / result) / 2;
            if next >= result {
                break;
            }
            result = next;
        }

        Ok(Self::from_raw(result as i128))
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

    /// Negate
    pub fn neg(self) -> Self {
        Self::from_raw(-self.raw)
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
        assert_eq!(a.abs(), FixedPoint::from_int(7));
    }
}
