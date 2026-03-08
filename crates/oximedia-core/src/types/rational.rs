//! Rational number type for precise time calculations.
//!
//! This module provides a [`Rational`] type that represents fractions with
//! 64-bit numerator and denominator, useful for exact time calculations
//! in multimedia applications.

use std::fmt;
use std::ops::{Add, Div, Mul, Sub};

/// Rational number for precise time calculations.
///
/// Represents a fraction as numerator/denominator with 64-bit precision.
/// This is essential for multimedia timing where floating-point rounding
/// errors are unacceptable.
///
/// # Examples
///
/// ```
/// use oximedia_core::types::Rational;
///
/// // Create a rational representing 30000/1001 (NTSC frame rate)
/// let ntsc = Rational::new(30000, 1001);
/// assert!((ntsc.to_f64() - 29.97).abs() < 0.01);
///
/// // Arithmetic operations maintain precision
/// let doubled = ntsc * Rational::new(2, 1);
/// assert_eq!(doubled.num, 60000);
/// assert_eq!(doubled.den, 1001);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Rational {
    /// Numerator of the fraction.
    pub num: i64,
    /// Denominator of the fraction.
    pub den: i64,
}

impl Rational {
    /// Creates a new `Rational` from numerator and denominator.
    ///
    /// # Arguments
    ///
    /// * `num` - The numerator
    /// * `den` - The denominator (must not be zero)
    ///
    /// # Panics
    ///
    /// Panics if `den` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::Rational;
    ///
    /// let half = Rational::new(1, 2);
    /// assert_eq!(half.num, 1);
    /// assert_eq!(half.den, 2);
    /// ```
    #[must_use]
    pub fn new(num: i64, den: i64) -> Self {
        assert!(den != 0, "Denominator cannot be zero");
        Self { num, den }
    }

    /// Reduces the rational to its lowest terms.
    ///
    /// Returns a new `Rational` with the numerator and denominator
    /// divided by their greatest common divisor.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::Rational;
    ///
    /// let r = Rational::new(4, 8);
    /// let reduced = r.reduce();
    /// assert_eq!(reduced.num, 1);
    /// assert_eq!(reduced.den, 2);
    /// ```
    #[must_use]
    #[allow(clippy::cast_possible_wrap)]
    pub fn reduce(self) -> Self {
        let g = gcd(self.num.unsigned_abs(), self.den.unsigned_abs());
        if g == 0 {
            return self;
        }
        let g_signed = g as i64;

        // Normalize sign: denominator should be positive
        let sign = if self.den < 0 { -1 } else { 1 };

        Self {
            num: sign * self.num / g_signed,
            den: sign * self.den / g_signed,
        }
    }

    /// Converts the rational to a 64-bit floating point number.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::Rational;
    ///
    /// let half = Rational::new(1, 2);
    /// assert!((half.to_f64() - 0.5).abs() < f64::EPSILON);
    /// ```
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    /// Returns the reciprocal of this rational.
    ///
    /// # Panics
    ///
    /// Panics if the numerator is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::Rational;
    ///
    /// let two = Rational::new(2, 1);
    /// let half = two.reciprocal();
    /// assert_eq!(half.num, 1);
    /// assert_eq!(half.den, 2);
    /// ```
    #[must_use]
    pub fn reciprocal(self) -> Self {
        assert!(self.num != 0, "Cannot take reciprocal of zero");
        Self {
            num: self.den,
            den: self.num,
        }
    }
}

/// Computes the greatest common divisor using Euclidean algorithm.
fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

impl Default for Rational {
    fn default() -> Self {
        Self { num: 0, den: 1 }
    }
}

impl Add for Rational {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        // a/b + c/d = (a*d + c*b) / (b*d)
        Self {
            num: self.num * rhs.den + rhs.num * self.den,
            den: self.den * rhs.den,
        }
        .reduce()
    }
}

impl Sub for Rational {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        // a/b - c/d = (a*d - c*b) / (b*d)
        Self {
            num: self.num * rhs.den - rhs.num * self.den,
            den: self.den * rhs.den,
        }
        .reduce()
    }
}

impl Mul for Rational {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            num: self.num * rhs.num,
            den: self.den * rhs.den,
        }
        .reduce()
    }
}

impl Div for Rational {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        assert!(rhs.num != 0, "Cannot divide by zero");
        Self {
            num: self.num * rhs.den,
            den: self.den * rhs.num,
        }
        .reduce()
    }
}

impl From<i64> for Rational {
    fn from(value: i64) -> Self {
        Self { num: value, den: 1 }
    }
}

impl From<i32> for Rational {
    fn from(value: i32) -> Self {
        Self {
            num: i64::from(value),
            den: 1,
        }
    }
}

impl From<(i64, i64)> for Rational {
    fn from(value: (i64, i64)) -> Self {
        Self::new(value.0, value.1)
    }
}

impl From<(i32, i32)> for Rational {
    fn from(value: (i32, i32)) -> Self {
        Self::new(i64::from(value.0), i64::from(value.1))
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.num, self.den)
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Compare a/b with c/d by comparing a*d with c*b
        let lhs = self.num * other.den;
        let rhs = other.num * self.den;
        lhs.cmp(&rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let r = Rational::new(3, 4);
        assert_eq!(r.num, 3);
        assert_eq!(r.den, 4);
    }

    #[test]
    #[should_panic(expected = "Denominator cannot be zero")]
    fn test_new_zero_denominator() {
        let _ = Rational::new(1, 0);
    }

    #[test]
    fn test_reduce() {
        let r = Rational::new(6, 8);
        let reduced = r.reduce();
        assert_eq!(reduced.num, 3);
        assert_eq!(reduced.den, 4);

        let r = Rational::new(-6, 8);
        let reduced = r.reduce();
        assert_eq!(reduced.num, -3);
        assert_eq!(reduced.den, 4);

        let r = Rational::new(6, -8);
        let reduced = r.reduce();
        assert_eq!(reduced.num, -3);
        assert_eq!(reduced.den, 4);
    }

    #[test]
    fn test_to_f64() {
        let r = Rational::new(1, 2);
        assert!((r.to_f64() - 0.5).abs() < f64::EPSILON);

        let r = Rational::new(1, 3);
        assert!((r.to_f64() - 0.333_333_333_333_333_3).abs() < 1e-10);
    }

    #[test]
    fn test_add() {
        let a = Rational::new(1, 2);
        let b = Rational::new(1, 3);
        let sum = a + b;
        assert_eq!(sum, Rational::new(5, 6));
    }

    #[test]
    fn test_sub() {
        let a = Rational::new(1, 2);
        let b = Rational::new(1, 3);
        let diff = a - b;
        assert_eq!(diff, Rational::new(1, 6));
    }

    #[test]
    fn test_mul() {
        let a = Rational::new(2, 3);
        let b = Rational::new(3, 4);
        let product = a * b;
        assert_eq!(product, Rational::new(1, 2));
    }

    #[test]
    fn test_div() {
        let a = Rational::new(2, 3);
        let b = Rational::new(4, 5);
        let quotient = a / b;
        assert_eq!(quotient, Rational::new(5, 6));
    }

    #[test]
    fn test_from_i64() {
        let r: Rational = 5_i64.into();
        assert_eq!(r.num, 5);
        assert_eq!(r.den, 1);
    }

    #[test]
    fn test_from_tuple() {
        let r: Rational = (3_i64, 4_i64).into();
        assert_eq!(r.num, 3);
        assert_eq!(r.den, 4);
    }

    #[test]
    fn test_display() {
        let r = Rational::new(3, 4);
        assert_eq!(format!("{r}"), "3/4");
    }

    #[test]
    fn test_ord() {
        let a = Rational::new(1, 3);
        let b = Rational::new(1, 2);
        assert!(a < b);
        assert!(b > a);

        let c = Rational::new(2, 6);
        assert_eq!(a, c.reduce());
    }

    #[test]
    fn test_reciprocal() {
        let r = Rational::new(3, 4);
        let recip = r.reciprocal();
        assert_eq!(recip.num, 4);
        assert_eq!(recip.den, 3);
    }

    #[test]
    fn test_default() {
        let r = Rational::default();
        assert_eq!(r.num, 0);
        assert_eq!(r.den, 1);
    }

    // Additional edge case tests

    #[test]
    fn test_negative_denominator_normalization() {
        let r = Rational::new(5, -7).reduce();
        assert_eq!(r.num, -5);
        assert_eq!(r.den, 7);
    }

    #[test]
    fn test_both_negative() {
        let r = Rational::new(-3, -4).reduce();
        assert_eq!(r.num, 3);
        assert_eq!(r.den, 4);
    }

    #[test]
    fn test_zero_numerator() {
        let r = Rational::new(0, 5);
        assert_eq!(r.to_f64(), 0.0);
    }

    #[test]
    #[should_panic(expected = "Cannot take reciprocal of zero")]
    fn test_reciprocal_of_zero() {
        let r = Rational::new(0, 1);
        let _ = r.reciprocal();
    }

    #[test]
    #[should_panic(expected = "Cannot divide by zero")]
    fn test_divide_by_zero() {
        let a = Rational::new(1, 2);
        let b = Rational::new(0, 1);
        let _ = a / b;
    }

    #[test]
    fn test_large_numbers() {
        let a = Rational::new(1_000_000, 1);
        let b = Rational::new(1, 1_000_000);
        let product = a * b;
        assert_eq!(product, Rational::new(1, 1));
    }

    #[test]
    fn test_ntsc_frame_rate() {
        let ntsc = Rational::new(30000, 1001);
        let fps = ntsc.to_f64();
        assert!((fps - 29.970_029_970_029_97).abs() < 1e-10);
    }

    #[test]
    fn test_pal_frame_rate() {
        let pal = Rational::new(25, 1);
        assert_eq!(pal.to_f64(), 25.0);
    }

    #[test]
    fn test_arithmetic_preserves_precision() {
        let a = Rational::new(1, 3);
        let b = Rational::new(2, 3);
        let sum = a + b;
        assert_eq!(sum, Rational::new(1, 1));
    }

    #[test]
    fn test_reduce_already_reduced() {
        let r = Rational::new(3, 5);
        let reduced = r.reduce();
        assert_eq!(reduced.num, 3);
        assert_eq!(reduced.den, 5);
    }

    #[test]
    fn test_comparison_different_denominators() {
        let a = Rational::new(1, 4);
        let b = Rational::new(2, 7);
        // 1/4 = 7/28, 2/7 = 8/28, so 1/4 < 2/7
        assert!(a < b);
    }

    #[test]
    fn test_gcd_function() {
        assert_eq!(gcd(12, 8), 4);
        assert_eq!(gcd(7, 5), 1);
        assert_eq!(gcd(0, 5), 5);
        assert_eq!(gcd(5, 0), 5);
    }
}
