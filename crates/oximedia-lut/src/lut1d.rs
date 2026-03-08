//! 1D LUT (Look-Up Table) implementation.
//!
//! 1D LUTs are per-channel curves used for tone mapping, gamma correction,
//! and color grading. They are faster than 3D LUTs but can only affect
//! each channel independently.
//!
//! # Example
//!
//! ```rust
//! use oximedia_lut::{Lut1d, LutInterpolation};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a simple gamma 2.2 curve
//! let mut lut = Lut1d::new(256);
//! for i in 0..256 {
//!     let t = i as f64 / 255.0;
//!     lut.set_r(i, t.powf(2.2));
//!     lut.set_g(i, t.powf(2.2));
//!     lut.set_b(i, t.powf(2.2));
//! }
//!
//! // Apply to a color
//! let input = [0.5, 0.3, 0.7];
//! let output = lut.apply(&input, LutInterpolation::Linear);
//! # Ok(())
//! # }
//! ```

use crate::error::{LutError, LutResult};
use crate::interpolation::{self, LutInterpolation};
use crate::Rgb;
use std::path::Path;

/// 1D LUT for per-channel color correction.
#[derive(Clone, Debug)]
pub struct Lut1d {
    /// Red channel LUT.
    pub r: Vec<f64>,
    /// Green channel LUT.
    pub g: Vec<f64>,
    /// Blue channel LUT.
    pub b: Vec<f64>,
    /// Size of each LUT (all three have the same size).
    size: usize,
    /// Input range minimum (usually 0.0).
    pub input_min: f64,
    /// Input range maximum (usually 1.0).
    pub input_max: f64,
}

impl Lut1d {
    /// Create a new 1D LUT with the specified size.
    ///
    /// All channels are initialized to identity (linear mapping).
    #[must_use]
    pub fn new(size: usize) -> Self {
        let mut lut = Self {
            r: vec![0.0; size],
            g: vec![0.0; size],
            b: vec![0.0; size],
            size,
            input_min: 0.0,
            input_max: 1.0,
        };
        lut.set_identity();
        lut
    }

    /// Create an identity 1D LUT.
    #[must_use]
    pub fn identity(size: usize) -> Self {
        Self::new(size)
    }

    /// Get the size of the LUT.
    #[must_use]
    pub const fn size(&self) -> usize {
        self.size
    }

    /// Set all channels to identity mapping.
    pub fn set_identity(&mut self) {
        for i in 0..self.size {
            let t = i as f64 / (self.size - 1) as f64;
            self.r[i] = t;
            self.g[i] = t;
            self.b[i] = t;
        }
    }

    /// Set a value in the red channel.
    pub fn set_r(&mut self, index: usize, value: f64) {
        self.r[index] = value;
    }

    /// Set a value in the green channel.
    pub fn set_g(&mut self, index: usize, value: f64) {
        self.g[index] = value;
    }

    /// Set a value in the blue channel.
    pub fn set_b(&mut self, index: usize, value: f64) {
        self.b[index] = value;
    }

    /// Apply the 1D LUT to an RGB color.
    #[must_use]
    pub fn apply(&self, rgb: &Rgb, interpolation: LutInterpolation) -> Rgb {
        [
            self.apply_channel(&self.r, rgb[0], interpolation),
            self.apply_channel(&self.g, rgb[1], interpolation),
            self.apply_channel(&self.b, rgb[2], interpolation),
        ]
    }

    /// Apply LUT to a single channel value.
    #[must_use]
    fn apply_channel(&self, channel: &[f64], value: f64, interpolation: LutInterpolation) -> f64 {
        // Normalize input to 0-1 range
        let normalized = (value - self.input_min) / (self.input_max - self.input_min);
        let clamped = normalized.clamp(0.0, 1.0);

        // Map to LUT index space
        let index_f = clamped * (self.size - 1) as f64;

        match interpolation {
            LutInterpolation::Nearest => {
                let index = index_f.round() as usize;
                channel[index.min(self.size - 1)]
            }
            LutInterpolation::Linear => {
                let index = index_f.floor() as usize;
                if index >= self.size - 1 {
                    channel[self.size - 1]
                } else {
                    let frac = index_f - index as f64;
                    interpolation::lerp(channel[index], channel[index + 1], frac)
                }
            }
            LutInterpolation::Cubic => {
                let index = index_f.floor() as usize;
                if index >= self.size - 1 {
                    channel[self.size - 1]
                } else {
                    let frac = index_f - index as f64;
                    let p0 = if index > 0 {
                        channel[index - 1]
                    } else {
                        channel[0]
                    };
                    let p1 = channel[index];
                    let p2 = channel[index + 1];
                    let p3 = if index + 2 < self.size {
                        channel[index + 2]
                    } else {
                        channel[self.size - 1]
                    };
                    interpolation::cubic_interp(p0, p1, p2, p3, frac)
                }
            }
            _ => {
                // Fallback to linear for unsupported interpolation modes
                let index = index_f.floor() as usize;
                if index >= self.size - 1 {
                    channel[self.size - 1]
                } else {
                    let frac = index_f - index as f64;
                    interpolation::lerp(channel[index], channel[index + 1], frac)
                }
            }
        }
    }

    /// Create a 1D LUT from a function.
    ///
    /// The function takes a normalized input value (0.0-1.0) and returns an RGB output.
    #[must_use]
    pub fn from_fn<F>(size: usize, f: F) -> Self
    where
        F: Fn(f64) -> Rgb,
    {
        let mut lut = Self::new(size);
        for i in 0..size {
            let t = i as f64 / (size - 1) as f64;
            let rgb = f(t);
            lut.r[i] = rgb[0];
            lut.g[i] = rgb[1];
            lut.b[i] = rgb[2];
        }
        lut
    }

    /// Create a gamma curve.
    #[must_use]
    pub fn gamma(size: usize, gamma: f64) -> Self {
        Self::from_fn(size, |t| {
            let v = t.powf(gamma);
            [v, v, v]
        })
    }

    /// Create an inverse gamma curve.
    #[must_use]
    pub fn inverse_gamma(size: usize, gamma: f64) -> Self {
        Self::gamma(size, 1.0 / gamma)
    }

    /// Invert the 1D LUT.
    ///
    /// This creates a LUT that reverses the effect of this LUT.
    /// Only works well for monotonic LUTs.
    #[must_use]
    pub fn invert(&self) -> Self {
        let mut inverted = Self::new(self.size);

        for channel_in in 0..3 {
            let (input, output) = match channel_in {
                0 => (&self.r, &mut inverted.r),
                1 => (&self.g, &mut inverted.g),
                _ => (&self.b, &mut inverted.b),
            };

            for (i, out) in output.iter_mut().enumerate() {
                let target = f64::from(i as u32) / (self.size - 1) as f64;

                // Binary search for the input value that produces this output
                let mut low = 0;
                let mut high = self.size - 1;

                while high - low > 1 {
                    let mid = (low + high) / 2;
                    if input[mid] < target {
                        low = mid;
                    } else {
                        high = mid;
                    }
                }

                // Linear interpolation between low and high
                if high == low {
                    *out = low as f64 / (self.size - 1) as f64;
                } else {
                    let t = (target - input[low]) / (input[high] - input[low]);
                    let low_f = low as f64 / (self.size - 1) as f64;
                    let high_f = high as f64 / (self.size - 1) as f64;
                    *out = interpolation::lerp(low_f, high_f, t);
                }
            }
        }

        inverted
    }

    /// Compose this LUT with another LUT.
    ///
    /// Returns a new LUT that is equivalent to applying `self` followed by `other`.
    #[must_use]
    pub fn compose(&self, other: &Self) -> Self {
        let mut composed = Self::new(self.size);

        for i in 0..self.size {
            let intermediate = [self.r[i], self.g[i], self.b[i]];
            let output = other.apply(&intermediate, LutInterpolation::Linear);
            composed.r[i] = output[0];
            composed.g[i] = output[1];
            composed.b[i] = output[2];
        }

        composed
    }

    /// Create a 1D LUT mapping `LogC` to linear.
    ///
    /// Arri `LogC` (EI 800) to scene-linear conversion.
    #[must_use]
    pub fn from_log_to_linear() -> Self {
        // LogC EI 800 parameters
        const A: f64 = 5.555_556;
        const B: f64 = 0.052_272;
        const C: f64 = 0.247_190;
        const D: f64 = 0.385_537;
        const E: f64 = 5.367_655;
        const F: f64 = 0.092_809;
        // Linear cut: encoded values below (E*LIN_CUT+F) use the linear segment
        const LIN_ENCODED_CUT: f64 = E * 0.005_526 + F;

        let size = 4096;
        Self::from_fn(size, |t| {
            let linear = if t > LIN_ENCODED_CUT {
                (10_f64.powf((t - D) / C) - B) / A
            } else {
                (t - F) / E
            };
            let clamped = linear.max(0.0);
            [clamped, clamped, clamped]
        })
    }

    /// Apply the 1D LUT to a single channel value (using linear interpolation).
    ///
    /// Uses the red channel of the LUT.
    #[must_use]
    pub fn apply_single(&self, value: f32) -> f32 {
        self.apply_channel(
            &self.r,
            f64::from(value),
            crate::interpolation::LutInterpolation::Linear,
        ) as f32
    }

    /// Load a 1D LUT from a file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn from_file<P: AsRef<Path>>(_path: P) -> LutResult<Self> {
        Err(LutError::UnsupportedFormat(
            "1D LUT file loading not yet implemented".to_string(),
        ))
    }

    /// Save the 1D LUT to a file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub fn to_file<P: AsRef<Path>>(&self, _path: P) -> LutResult<()> {
        Err(LutError::UnsupportedFormat(
            "1D LUT file saving not yet implemented".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_lut() {
        let lut = Lut1d::identity(256);
        let input = [0.5, 0.3, 0.7];
        let output = lut.apply(&input, LutInterpolation::Linear);
        assert!((output[0] - input[0]).abs() < 1e-6);
        assert!((output[1] - input[1]).abs() < 1e-6);
        assert!((output[2] - input[2]).abs() < 1e-6);
    }

    #[test]
    fn test_gamma_lut() {
        let lut = Lut1d::gamma(256, 2.2);
        let input = [0.5, 0.5, 0.5];
        let output = lut.apply(&input, LutInterpolation::Linear);
        let expected = 0.5_f64.powf(2.2);
        assert!((output[0] - expected).abs() < 0.01);
    }

    #[test]
    fn test_invert_lut() {
        let lut = Lut1d::gamma(256, 2.2);
        let inverted = lut.invert();
        let input = [0.5, 0.3, 0.7];
        let encoded = lut.apply(&input, LutInterpolation::Linear);
        let decoded = inverted.apply(&encoded, LutInterpolation::Linear);
        assert!((decoded[0] - input[0]).abs() < 0.01);
        assert!((decoded[1] - input[1]).abs() < 0.01);
        assert!((decoded[2] - input[2]).abs() < 0.01);
    }

    #[test]
    fn test_compose_lut() {
        let lut1 = Lut1d::gamma(256, 2.2);
        let lut2 = Lut1d::inverse_gamma(256, 2.2);
        let composed = lut1.compose(&lut2);

        let input = [0.5, 0.3, 0.7];
        let output = composed.apply(&input, LutInterpolation::Linear);

        // Composed should be close to identity
        assert!((output[0] - input[0]).abs() < 0.01);
        assert!((output[1] - input[1]).abs() < 0.01);
        assert!((output[2] - input[2]).abs() < 0.01);
    }

    #[test]
    fn test_from_fn() {
        let lut = Lut1d::from_fn(256, |t| [t * 2.0, t * 0.5, t]);
        let input = [0.5, 0.5, 0.5];
        let output = lut.apply(&input, LutInterpolation::Linear);
        assert!((output[0] - 1.0).abs() < 0.01);
        assert!((output[1] - 0.25).abs() < 0.01);
        assert!((output[2] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_interpolation_modes() {
        let lut = Lut1d::gamma(256, 2.2);
        let input = [0.5, 0.5, 0.5];

        let nearest = lut.apply(&input, LutInterpolation::Nearest);
        let linear = lut.apply(&input, LutInterpolation::Linear);
        let cubic = lut.apply(&input, LutInterpolation::Cubic);

        // All should be similar
        assert!((nearest[0] - linear[0]).abs() < 0.02);
        assert!((cubic[0] - linear[0]).abs() < 0.02);
    }

    #[test]
    fn test_from_log_to_linear_midpoint() {
        let lut = Lut1d::from_log_to_linear();
        // LogC middle grey ~0.391 should map to ~0.18 scene-linear
        let result = lut.apply_single(0.391_f32);
        assert!(result > 0.1 && result < 0.3, "Expected ~0.18, got {result}");
    }

    #[test]
    fn test_from_log_to_linear_black() {
        let lut = Lut1d::from_log_to_linear();
        // Very low values should map near zero
        let result = lut.apply_single(0.0_f32);
        assert!(result < 0.01, "Expected near 0.0, got {result}");
    }

    #[test]
    fn test_apply_single_identity() {
        let lut = Lut1d::identity(256);
        // For identity LUT, apply_single should return nearly same value
        let result = lut.apply_single(0.5_f32);
        assert!((result - 0.5).abs() < 0.002, "Expected ~0.5, got {result}");
    }

    #[test]
    fn test_apply_single_gamma() {
        let lut = Lut1d::gamma(256, 2.2);
        let expected = 0.5_f32.powf(2.2);
        let result = lut.apply_single(0.5_f32);
        assert!(
            (result - expected).abs() < 0.01,
            "Expected {expected}, got {result}"
        );
    }

    #[test]
    fn test_from_log_to_linear_size() {
        let lut = Lut1d::from_log_to_linear();
        assert_eq!(lut.size(), 4096);
    }

    #[test]
    fn test_apply_single_clamp_high() {
        let lut = Lut1d::identity(256);
        // Values at or above max should return max
        let result = lut.apply_single(1.0_f32);
        assert!((result - 1.0).abs() < 0.002, "Expected 1.0, got {result}");
    }
}
