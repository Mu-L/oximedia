//! Feature Similarity Index (FSIM) calculation.
//!
//! FSIM is based on the fact that human visual system (HVS) understands
//! an image mainly according to its low-level features. It uses phase
//! congruency (PC) and gradient magnitude (GM) as features.
//!
//! # Reference
//!
//! L. Zhang et al., "FSIM: A Feature Similarity Index for Image Quality
//! Assessment," IEEE Transactions on Image Processing, 2011.

use crate::{Frame, MetricType, QualityScore};
use oximedia_core::OxiResult;

/// FSIM calculator for video quality assessment.
pub struct FsimCalculator {
    /// Constant T1 for phase congruency
    t1: f64,
    /// Constant T2 for gradient magnitude
    t2: f64,
}

impl FsimCalculator {
    /// Creates a new FSIM calculator with default parameters.
    #[must_use]
    pub fn new() -> Self {
        Self {
            t1: 0.85,  // Threshold for phase congruency
            t2: 160.0, // Threshold for gradient magnitude
        }
    }

    /// Calculates FSIM between reference and distorted frames.
    ///
    /// # Errors
    ///
    /// Returns an error if frame dimensions don't match.
    pub fn calculate(&self, reference: &Frame, distorted: &Frame) -> OxiResult<QualityScore> {
        if reference.width != distorted.width || reference.height != distorted.height {
            return Err(oximedia_core::OxiError::InvalidData(
                "Frame dimensions must match".to_string(),
            ));
        }

        let mut score = QualityScore::new(MetricType::Fsim, 0.0);

        // Calculate FSIM for Y plane
        let y_fsim = self.calculate_plane(
            &reference.planes[0],
            &distorted.planes[0],
            reference.width,
            reference.height,
        )?;

        score.add_component("Y", y_fsim);
        score.score = y_fsim;

        Ok(score)
    }

    /// Calculates FSIM for a single plane.
    #[allow(clippy::unnecessary_wraps)]
    fn calculate_plane(
        &self,
        ref_plane: &[u8],
        dist_plane: &[u8],
        width: usize,
        height: usize,
    ) -> OxiResult<f64> {
        // Convert to f64
        let ref_f64 = self.plane_to_f64(ref_plane);
        let dist_f64 = self.plane_to_f64(dist_plane);

        // Compute phase congruency maps
        let pc_ref = self.compute_phase_congruency(&ref_f64, width, height);
        let pc_dist = self.compute_phase_congruency(&dist_f64, width, height);

        // Compute gradient magnitude maps
        let gm_ref = self.compute_gradient_magnitude(&ref_f64, width, height);
        let gm_dist = self.compute_gradient_magnitude(&dist_f64, width, height);

        // Compute similarity maps
        let mut sum_sim = 0.0;
        let mut sum_weight = 0.0;

        for i in 0..ref_f64.len() {
            // Phase congruency similarity
            let pc_sim = (2.0 * pc_ref[i] * pc_dist[i] + self.t1)
                / (pc_ref[i] * pc_ref[i] + pc_dist[i] * pc_dist[i] + self.t1);

            // Gradient magnitude similarity
            let gm_sim = (2.0 * gm_ref[i] * gm_dist[i] + self.t2)
                / (gm_ref[i] * gm_ref[i] + gm_dist[i] * gm_dist[i] + self.t2);

            // Feature similarity
            let sim = pc_sim * gm_sim;

            // Weight by maximum PC
            let weight = pc_ref[i].max(pc_dist[i]);

            sum_sim += sim * weight;
            sum_weight += weight;
        }

        let fsim = if sum_weight > 1e-10 {
            sum_sim / sum_weight
        } else {
            1.0
        };

        Ok(fsim.clamp(0.0, 1.0))
    }

    /// Computes phase congruency map (simplified version).
    ///
    /// This is a simplified approximation using gradient-based approach.
    fn compute_phase_congruency(&self, plane: &[f64], width: usize, height: usize) -> Vec<f64> {
        let mut pc = vec![0.0; plane.len()];

        // Sobel kernels for gradient
        let sobel_x = [-1.0, 0.0, 1.0, -2.0, 0.0, 2.0, -1.0, 0.0, 1.0];
        let sobel_y = [-1.0, -2.0, -1.0, 0.0, 0.0, 0.0, 1.0, 2.0, 1.0];

        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let mut gx = 0.0;
                let mut gy = 0.0;

                for dy in 0..3 {
                    for dx in 0..3 {
                        let idx = (y + dy - 1) * width + (x + dx - 1);
                        let kernel_idx = dy * 3 + dx;
                        gx += plane[idx] * sobel_x[kernel_idx];
                        gy += plane[idx] * sobel_y[kernel_idx];
                    }
                }

                let magnitude = (gx * gx + gy * gy).sqrt();
                let phase = gy.atan2(gx);

                // Simplified phase congruency based on local phase consistency
                // In a full implementation, this would use Log-Gabor filters
                pc[y * width + x] = (magnitude / 255.0) * (phase.cos() + 1.0) / 2.0;
            }
        }

        pc
    }

    /// Computes gradient magnitude map.
    fn compute_gradient_magnitude(&self, plane: &[f64], width: usize, height: usize) -> Vec<f64> {
        let mut gm = vec![0.0; plane.len()];

        // Scharr kernels (better rotation invariance than Sobel)
        let scharr_x = [-3.0, 0.0, 3.0, -10.0, 0.0, 10.0, -3.0, 0.0, 3.0];
        let scharr_y = [-3.0, -10.0, -3.0, 0.0, 0.0, 0.0, 3.0, 10.0, 3.0];

        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let mut gx = 0.0;
                let mut gy = 0.0;

                for dy in 0..3 {
                    for dx in 0..3 {
                        let idx = (y + dy - 1) * width + (x + dx - 1);
                        let kernel_idx = dy * 3 + dx;
                        gx += plane[idx] * scharr_x[kernel_idx];
                        gy += plane[idx] * scharr_y[kernel_idx];
                    }
                }

                gm[y * width + x] = (gx * gx + gy * gy).sqrt();
            }
        }

        gm
    }

    /// Converts u8 plane to f64 plane.
    fn plane_to_f64(&self, plane: &[u8]) -> Vec<f64> {
        plane.iter().map(|&v| f64::from(v)).collect()
    }
}

impl Default for FsimCalculator {
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
        frame
    }

    fn create_gradient_frame(width: usize, height: usize) -> Frame {
        let mut frame =
            Frame::new(width, height, PixelFormat::Yuv420p).expect("should succeed in test");
        for y in 0..height {
            for x in 0..width {
                frame.planes[0][y * width + x] = (x * 255 / width) as u8;
            }
        }
        frame
    }

    #[test]
    fn test_fsim_identical_frames() {
        let calc = FsimCalculator::new();
        let frame1 = create_test_frame(64, 64, 128);
        let frame2 = create_test_frame(64, 64, 128);

        let result = calc
            .calculate(&frame1, &frame2)
            .expect("should succeed in test");
        assert!(result.score >= 0.0 && result.score <= 1.0);
    }

    #[test]
    fn test_fsim_different_frames() {
        let calc = FsimCalculator::new();
        let frame1 = create_gradient_frame(64, 64);
        let frame2 = create_test_frame(64, 64, 128);

        let result = calc
            .calculate(&frame1, &frame2)
            .expect("should succeed in test");
        assert!(result.score >= 0.0 && result.score <= 1.0);
        assert!(result.components.contains_key("Y"));
    }

    #[test]
    fn test_gradient_magnitude() {
        let calc = FsimCalculator::new();
        let plane = vec![
            0.0, 0.0, 0.0, 0.0, 0.0, 100.0, 100.0, 100.0, 100.0, 100.0, 200.0, 200.0, 200.0, 200.0,
            200.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ];

        let gm = calc.compute_gradient_magnitude(&plane, 5, 4);

        // Check that gradient is computed
        assert!(gm.iter().any(|&v| v > 0.0));
    }

    #[test]
    fn test_phase_congruency() {
        let calc = FsimCalculator::new();
        let plane = vec![
            0.0, 0.0, 0.0, 0.0, 0.0, 100.0, 100.0, 100.0, 100.0, 100.0, 200.0, 200.0, 200.0, 200.0,
            200.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ];

        let pc = calc.compute_phase_congruency(&plane, 5, 4);

        // PC should be computed (may be zero for uniform regions)
        assert_eq!(pc.len(), 20);
    }
}
