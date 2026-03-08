//! Rational number arithmetic for frame rates and time bases.
//!
//! This module provides a lightweight [`Rational`] type with `i32` numerator
//! and denominator, suitable for frame-rate and time-base calculations.
//! For high-precision time arithmetic, see [`crate::types::Rational`] which
//! uses `i64`.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

/// A rational number with `i32` numerator and denominator.
///
/// Intended for frame-rate and time-base representation where `i32` range
/// is sufficient (e.g. 30000/1001, 24000/1001).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rational {
    /// Numerator.
    pub num: i32,
    /// Denominator. Must not be zero.
    pub den: i32,
}

/// Computes the greatest common divisor of two non-negative integers.
fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

impl Rational {
    /// Creates a new `Rational`.
    ///
    /// # Panics
    ///
    /// Panics if `d` is zero.
    #[must_use]
    pub fn new(n: i32, d: i32) -> Self {
        assert!(d != 0, "Rational denominator must not be zero");
        Self { num: n, den: d }
    }

    /// Creates a `Rational` from a floating-point value using continued-fraction
    /// approximation with a maximum denominator of 1001.
    #[must_use]
    pub fn from_f64(f: f64) -> Self {
        const MAX_DEN: i32 = 1001;
        if f == 0.0 {
            return Self::new(0, 1);
        }
        let negative = f < 0.0;
        let f = f.abs();
        let mut best_num = 0i32;
        let mut best_den = 1i32;
        let mut best_err = f64::MAX;
        for den in 1..=MAX_DEN {
            #[allow(clippy::cast_possible_truncation)]
            let num = (f * f64::from(den)).round() as i32;
            let err = (f - f64::from(num) / f64::from(den)).abs();
            if err < best_err {
                best_err = err;
                best_num = num;
                best_den = den;
            }
            if err < 1e-9 {
                break;
            }
        }
        Self::new(if negative { -best_num } else { best_num }, best_den)
    }

    /// Converts this rational to a 64-bit float.
    #[must_use]
    pub fn to_f64(self) -> f64 {
        f64::from(self.num) / f64::from(self.den)
    }

    /// Returns a new `Rational` reduced to lowest terms.
    #[must_use]
    #[allow(clippy::cast_possible_wrap)]
    pub fn reduce(self) -> Self {
        let g = gcd(self.num.unsigned_abs(), self.den.unsigned_abs());
        if g == 0 {
            return self;
        }
        let g = g as i32;
        let sign = if self.den < 0 { -1 } else { 1 };
        Self {
            num: sign * self.num / g,
            den: sign * self.den / g,
        }
    }

    /// Adds two rationals and returns the (unreduced) result.
    #[must_use]
    pub fn add(&self, other: &Self) -> Self {
        Self {
            num: self.num * other.den + other.num * self.den,
            den: self.den * other.den,
        }
        .reduce()
    }

    /// Multiplies two rationals and returns the reduced result.
    #[must_use]
    pub fn multiply(&self, other: &Self) -> Self {
        Self {
            num: self.num * other.num,
            den: self.den * other.den,
        }
        .reduce()
    }

    /// Returns `true` if this rational equals zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.num == 0
    }

    /// Returns `true` if this rational equals one (after reduction).
    #[must_use]
    pub fn is_one(&self) -> bool {
        let r = self.reduce();
        r.num == r.den
    }

    // --- Common frame rate constructors ---

    /// 24 fps (cinema).
    #[must_use]
    pub fn fps_24() -> Self {
        Self::new(24, 1)
    }

    /// 25 fps (PAL).
    #[must_use]
    pub fn fps_25() -> Self {
        Self::new(25, 1)
    }

    /// 30 fps.
    #[must_use]
    pub fn fps_30() -> Self {
        Self::new(30, 1)
    }

    /// ~29.97 fps (NTSC, 30000/1001).
    #[must_use]
    pub fn fps_2997() -> Self {
        Self::new(30000, 1001)
    }

    /// 60 fps.
    #[must_use]
    pub fn fps_60() -> Self {
        Self::new(60, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_basic() {
        let r = Rational::new(3, 4);
        assert_eq!(r.num, 3);
        assert_eq!(r.den, 4);
    }

    #[test]
    #[should_panic(expected = "denominator must not be zero")]
    fn test_new_zero_den_panics() {
        let _ = Rational::new(1, 0);
    }

    #[test]
    fn test_to_f64_half() {
        let r = Rational::new(1, 2);
        assert!((r.to_f64() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_reduce_fraction() {
        let r = Rational::new(6, 9).reduce();
        assert_eq!(r.num, 2);
        assert_eq!(r.den, 3);
    }

    #[test]
    fn test_reduce_already_reduced() {
        let r = Rational::new(3, 7).reduce();
        assert_eq!(r.num, 3);
        assert_eq!(r.den, 7);
    }

    #[test]
    fn test_add_fractions() {
        let a = Rational::new(1, 3);
        let b = Rational::new(1, 6);
        let sum = a.add(&b);
        assert_eq!(sum, Rational::new(1, 2));
    }

    #[test]
    fn test_multiply_fractions() {
        let a = Rational::new(2, 3);
        let b = Rational::new(3, 4);
        let prod = a.multiply(&b);
        assert_eq!(prod, Rational::new(1, 2));
    }

    #[test]
    fn test_is_zero_true() {
        assert!(Rational::new(0, 5).is_zero());
    }

    #[test]
    fn test_is_zero_false() {
        assert!(!Rational::new(1, 5).is_zero());
    }

    #[test]
    fn test_is_one_true() {
        assert!(Rational::new(4, 4).is_one());
    }

    #[test]
    fn test_is_one_false() {
        assert!(!Rational::new(1, 2).is_one());
    }

    #[test]
    fn test_fps_24() {
        let r = Rational::fps_24();
        assert_eq!(r.to_f64(), 24.0);
    }

    #[test]
    fn test_fps_25() {
        let r = Rational::fps_25();
        assert_eq!(r.to_f64(), 25.0);
    }

    #[test]
    fn test_fps_30() {
        let r = Rational::fps_30();
        assert_eq!(r.to_f64(), 30.0);
    }

    #[test]
    fn test_fps_2997() {
        let r = Rational::fps_2997();
        assert_eq!(r.num, 30000);
        assert_eq!(r.den, 1001);
        assert!((r.to_f64() - 29.970_029_97).abs() < 1e-6);
    }

    #[test]
    fn test_fps_60() {
        let r = Rational::fps_60();
        assert_eq!(r.to_f64(), 60.0);
    }

    #[test]
    fn test_from_f64_half() {
        let r = Rational::from_f64(0.5);
        assert_eq!(r.num, 1);
        assert_eq!(r.den, 2);
    }

    #[test]
    fn test_from_f64_zero() {
        let r = Rational::from_f64(0.0);
        assert!(r.is_zero());
    }
}
