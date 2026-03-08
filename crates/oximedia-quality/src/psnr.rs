//! Peak Signal-to-Noise Ratio (PSNR) calculation.
//!
//! PSNR is a widely used objective metric for measuring reconstruction quality
//! in lossy compression. It's based on the mean squared error (MSE) between
//! the reference and distorted signals.
//!
//! # Formula
//!
//! PSNR = 10 × log₁₀(MAX²/MSE)
//!
//! where MAX is the maximum possible pixel value (255 for 8-bit)
//! and MSE is the mean squared error.

use crate::{Frame, MetricType, QualityScore};
use oximedia_core::OxiResult;
use rayon::prelude::*;

/// PSNR calculator for video quality assessment.
pub struct PsnrCalculator {
    /// Weight for luma (Y) component
    luma_weight: f64,
    /// Weight for chroma (Cb, Cr) components
    chroma_weight: f64,
}

impl PsnrCalculator {
    /// Creates a new PSNR calculator with default weights.
    ///
    /// Uses standard weights: Y=4, Cb=1, Cr=1 (normalized to sum to 1).
    #[must_use]
    pub fn new() -> Self {
        Self {
            luma_weight: 4.0 / 6.0,
            chroma_weight: 1.0 / 6.0,
        }
    }

    /// Creates a PSNR calculator with custom component weights.
    #[must_use]
    pub fn with_weights(luma_weight: f64, chroma_weight: f64) -> Self {
        let total = luma_weight + 2.0 * chroma_weight;
        Self {
            luma_weight: luma_weight / total,
            chroma_weight: chroma_weight / total,
        }
    }

    /// Calculates PSNR between reference and distorted frames.
    ///
    /// # Errors
    ///
    /// Returns an error if frame dimensions don't match or if frames are invalid.
    pub fn calculate(&self, reference: &Frame, distorted: &Frame) -> OxiResult<QualityScore> {
        let mut score = QualityScore::new(MetricType::Psnr, 0.0);

        // Calculate Y PSNR
        let y_psnr = self.calculate_plane(&reference.planes[0], &distorted.planes[0])?;
        score.add_component("Y", y_psnr);

        let mut weighted_psnr = self.luma_weight * y_psnr;

        // Calculate chroma PSNR if available
        if reference.planes.len() >= 3 && distorted.planes.len() >= 3 {
            let cb_psnr = self.calculate_plane(&reference.planes[1], &distorted.planes[1])?;
            let cr_psnr = self.calculate_plane(&reference.planes[2], &distorted.planes[2])?;

            score.add_component("Cb", cb_psnr);
            score.add_component("Cr", cr_psnr);

            weighted_psnr += self.chroma_weight * (cb_psnr + cr_psnr);
        }

        score.score = weighted_psnr;
        Ok(score)
    }

    /// Calculates PSNR for a single plane.
    fn calculate_plane(&self, reference: &[u8], distorted: &[u8]) -> OxiResult<f64> {
        if reference.len() != distorted.len() {
            return Err(oximedia_core::OxiError::InvalidData(
                "Plane sizes must match".to_string(),
            ));
        }

        if reference.is_empty() {
            return Ok(f64::INFINITY);
        }

        // Calculate MSE using parallel processing for large planes
        let mse = if reference.len() > 10000 {
            self.calculate_mse_parallel(reference, distorted)
        } else {
            self.calculate_mse(reference, distorted)
        };

        if mse < 1e-10 {
            // Practically identical
            return Ok(100.0); // Return a very high PSNR
        }

        // PSNR = 10 * log10(MAX^2 / MSE)
        // For 8-bit: MAX = 255
        let max_value = 255.0;
        let psnr = 10.0 * (max_value * max_value / mse).log10();

        Ok(psnr)
    }

    /// Calculates mean squared error sequentially.
    fn calculate_mse(&self, reference: &[u8], distorted: &[u8]) -> f64 {
        let sum_squared_diff: u64 = reference
            .iter()
            .zip(distorted.iter())
            .map(|(r, d)| {
                let diff = i32::from(*r) - i32::from(*d);
                (diff * diff) as u64
            })
            .sum();

        sum_squared_diff as f64 / reference.len() as f64
    }

    /// Calculates mean squared error using parallel processing.
    fn calculate_mse_parallel(&self, reference: &[u8], distorted: &[u8]) -> f64 {
        let sum_squared_diff: u64 = reference
            .par_iter()
            .zip(distorted.par_iter())
            .map(|(r, d)| {
                let diff = i32::from(*r) - i32::from(*d);
                (diff * diff) as u64
            })
            .sum();

        sum_squared_diff as f64 / reference.len() as f64
    }

    /// Calculates PSNR for 10-bit or 12-bit content.
    #[allow(dead_code)]
    fn calculate_plane_high_bit_depth(
        &self,
        reference: &[u16],
        distorted: &[u16],
        bit_depth: u32,
    ) -> OxiResult<f64> {
        if reference.len() != distorted.len() {
            return Err(oximedia_core::OxiError::InvalidData(
                "Plane sizes must match".to_string(),
            ));
        }

        if reference.is_empty() {
            return Ok(f64::INFINITY);
        }

        let sum_squared_diff: u64 = reference
            .iter()
            .zip(distorted.iter())
            .map(|(r, d)| {
                let diff = i64::from(*r) - i64::from(*d);
                (diff * diff) as u64
            })
            .sum();

        let mse = sum_squared_diff as f64 / reference.len() as f64;

        if mse < 1e-10 {
            return Ok(100.0);
        }

        let max_value = f64::from((1u32 << bit_depth) - 1);
        let psnr = 10.0 * (max_value * max_value / mse).log10();

        Ok(psnr)
    }
}

impl Default for PsnrCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oximedia_core::PixelFormat;

    fn create_test_frame(width: usize, height: usize, value: u8) -> Frame {
        let mut frame =
            Frame::new(width, height, PixelFormat::Yuv420p).expect("should succeed in test");
        frame.planes[0].fill(value);
        frame.planes[1].fill(128);
        frame.planes[2].fill(128);
        frame
    }

    #[test]
    fn test_psnr_identical_frames() {
        let calc = PsnrCalculator::new();
        let frame1 = create_test_frame(64, 64, 128);
        let frame2 = create_test_frame(64, 64, 128);

        let result = calc
            .calculate(&frame1, &frame2)
            .expect("should succeed in test");
        assert!(result.score > 99.0); // Very high PSNR for identical frames
    }

    #[test]
    fn test_psnr_different_frames() {
        let calc = PsnrCalculator::new();
        let frame1 = create_test_frame(64, 64, 100);
        let frame2 = create_test_frame(64, 64, 110);

        let result = calc
            .calculate(&frame1, &frame2)
            .expect("should succeed in test");
        assert!(result.score > 0.0 && result.score < 100.0);
        assert!(result.components.contains_key("Y"));
    }

    #[test]
    fn test_psnr_custom_weights() {
        let calc = PsnrCalculator::with_weights(1.0, 0.5);
        let frame1 = create_test_frame(64, 64, 100);
        let frame2 = create_test_frame(64, 64, 110);

        let result = calc
            .calculate(&frame1, &frame2)
            .expect("should succeed in test");
        assert!(result.score > 0.0);
    }

    #[test]
    fn test_mse_calculation() {
        let calc = PsnrCalculator::new();
        let ref_plane = vec![100u8; 100];
        let dist_plane = vec![110u8; 100];

        let mse = calc.calculate_mse(&ref_plane, &dist_plane);
        assert!((mse - 100.0).abs() < 0.01); // (110-100)^2 = 100
    }

    #[test]
    fn test_psnr_formula() {
        let calc = PsnrCalculator::new();

        // Create frames with known MSE
        let mut frame1 = create_test_frame(10, 10, 0);
        let mut frame2 = create_test_frame(10, 10, 0);

        // Set specific values to get MSE = 100 for Y plane
        frame1.planes[0].fill(100);
        frame2.planes[0].fill(110);

        let result = calc
            .calculate(&frame1, &frame2)
            .expect("should succeed in test");

        // Y PSNR ≈ 28.13, Cb/Cr PSNR ≈ 100 (identical chroma)
        // Weighted: (4/6)*28 + (2/6)*100 ≈ 52
        assert!(result.score > 45.0 && result.score < 60.0);
    }
}
