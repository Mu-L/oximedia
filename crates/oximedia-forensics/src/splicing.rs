//! Image splicing detection via noise-level inconsistency.
//!
//! When a region is copied from a different image (splicing), its local noise
//! characteristics typically differ from the surrounding content.  This module
//! estimates per-block noise using the Median Absolute Deviation (MAD) and
//! then flags statistical outliers (> 2.5 σ from the mean) as likely spliced.

#![allow(dead_code)]

/// A region suspected to have been spliced in.
#[derive(Debug, Clone)]
pub struct SplicingIndicator {
    /// Bounding box of the suspected region: (x, y, width, height) in pixels.
    pub region: (u32, u32, u32, u32),
    /// Measured noise level in this block (MAD-based).
    pub noise_level: f32,
    /// Expected noise level (global median of all blocks).
    pub expected_noise: f32,
}

/// Detector for image splicing via noise inconsistency analysis.
pub struct SplicingDetector {
    /// Block size used for per-region noise estimation (default 32).
    pub block_size: u32,
    /// Outlier threshold in units of standard deviation (default 2.5).
    pub sigma_threshold: f32,
}

impl Default for SplicingDetector {
    fn default() -> Self {
        Self {
            block_size: 32,
            sigma_threshold: 2.5,
        }
    }
}

impl SplicingDetector {
    /// Creates a [`SplicingDetector`] with the default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a [`SplicingDetector`] with custom settings.
    #[must_use]
    pub fn with_params(block_size: u32, sigma_threshold: f32) -> Self {
        Self {
            block_size,
            sigma_threshold,
        }
    }

    /// Estimates a per-block noise map for a luma plane using MAD.
    ///
    /// # Arguments
    ///
    /// * `luma`       - Luma values in [0, 1], row-major, `width * height` elements.
    /// * `width`      - Image width in pixels.
    /// * `height`     - Image height in pixels.
    /// * `block_size` - Block size in pixels.
    ///
    /// # Returns
    ///
    /// A flat `Vec<f32>` of per-block MAD values, row-major block order.
    #[must_use]
    pub fn estimate_noise_by_region(
        &self,
        luma: &[f32],
        width: u32,
        height: u32,
        block_size: u32,
    ) -> Vec<f32> {
        estimate_noise_by_region(luma, width, height, block_size)
    }

    /// Detects spliced regions from a pre-computed noise map.
    ///
    /// # Arguments
    ///
    /// * `noise_map`  - Per-block noise values (from [`estimate_noise_by_region`]).
    /// * `width`      - Image width in pixels.
    /// * `height`     - Image height in pixels.
    ///
    /// # Returns
    ///
    /// A `Vec<SplicingIndicator>` for blocks whose noise level deviates by more
    /// than `self.sigma_threshold` σ from the global mean.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn detect_splicing(
        &self,
        noise_map: &[f32],
        width: u32,
        height: u32,
    ) -> Vec<SplicingIndicator> {
        detect_splicing_impl(
            noise_map,
            width,
            height,
            self.block_size,
            self.sigma_threshold,
        )
    }

    /// Convenience method: estimate noise and detect splicing in one call.
    #[must_use]
    pub fn analyze(&self, luma: &[f32], width: u32, height: u32) -> Vec<SplicingIndicator> {
        let noise_map = self.estimate_noise_by_region(luma, width, height, self.block_size);
        self.detect_splicing(&noise_map, width, height)
    }
}

// ---------------------------------------------------------------------------
// Core functions (also exposed as free functions for flexibility)
// ---------------------------------------------------------------------------

/// Estimates a per-block noise level using the Median Absolute Deviation.
///
/// For each `block_size × block_size` block the MAD of residuals (difference
/// from local block mean) is computed as the noise estimate.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_precision_loss)]
pub fn estimate_noise_by_region(
    luma: &[f32],
    width: u32,
    height: u32,
    block_size: u32,
) -> Vec<f32> {
    let w = width as usize;
    let h = height as usize;
    let bs = block_size as usize;

    if luma.len() < w * h || bs == 0 {
        return Vec::new();
    }

    let blocks_x = (w + bs - 1) / bs;
    let blocks_y = (h + bs - 1) / bs;
    let mut noise_map = Vec::with_capacity(blocks_x * blocks_y);

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let x0 = bx * bs;
            let y0 = by * bs;
            let x1 = (x0 + bs).min(w);
            let y1 = (y0 + bs).min(h);

            // Collect pixel values in this block
            let mut values: Vec<f32> = Vec::with_capacity(bs * bs);
            for y in y0..y1 {
                for x in x0..x1 {
                    values.push(luma[y * w + x]);
                }
            }

            noise_map.push(mad_noise(&values));
        }
    }

    noise_map
}

/// Detect spliced regions from a noise map using z-score outlier detection.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_precision_loss)]
pub fn detect_splicing_impl(
    noise_map: &[f32],
    width: u32,
    _height: u32,
    block_size: u32,
    sigma_threshold: f32,
) -> Vec<SplicingIndicator> {
    if noise_map.is_empty() {
        return Vec::new();
    }

    let w = width as usize;
    let bs = block_size as usize;
    let blocks_x = if bs > 0 {
        (w + bs - 1) / bs
    } else {
        return Vec::new();
    };

    // Global mean and std dev of noise levels
    let n = noise_map.len() as f32;
    let mean: f32 = noise_map.iter().sum::<f32>() / n;
    let variance: f32 = noise_map
        .iter()
        .map(|&v| (v - mean) * (v - mean))
        .sum::<f32>()
        / n;
    let std_dev = variance.sqrt();

    // Expected noise = global median
    let mut sorted = noise_map.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let expected_noise = if sorted.is_empty() {
        mean
    } else {
        sorted[sorted.len() / 2]
    };

    let mut indicators = Vec::new();

    for (idx, &noise_level) in noise_map.iter().enumerate() {
        let z_score = if std_dev > 1e-9 {
            (noise_level - mean).abs() / std_dev
        } else {
            0.0
        };

        if z_score > sigma_threshold {
            let bx = (idx % blocks_x) as u32;
            let by = (idx / blocks_x) as u32;
            let x0 = bx * block_size;
            let y0 = by * block_size;

            indicators.push(SplicingIndicator {
                region: (x0, y0, block_size, block_size),
                noise_level,
                expected_noise,
            });
        }
    }

    indicators
}

// ---------------------------------------------------------------------------
// Statistical helpers
// ---------------------------------------------------------------------------

/// Compute the Median Absolute Deviation as a robust noise estimator.
///
/// `mad = median(|x_i - median(x)|)`
///
/// Scaled by 1.4826 to be a consistent estimator of σ for Gaussian noise.
#[allow(clippy::cast_precision_loss)]
fn mad_noise(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }

    // Compute median
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    let median = if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    };

    // Compute MAD
    let mut deviations: Vec<f32> = values.iter().map(|&v| (v - median).abs()).collect();
    deviations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mad_mid = deviations.len() / 2;
    let mad = if deviations.len() % 2 == 0 {
        (deviations[mad_mid - 1] + deviations[mad_mid]) / 2.0
    } else {
        deviations[mad_mid]
    };

    // Scale factor for Gaussian consistency
    mad * 1.4826
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn uniform_luma(w: usize, h: usize, val: f32) -> Vec<f32> {
        vec![val; w * h]
    }

    /// Luma with a "high-noise" region pasted in.
    fn luma_with_noise_patch(w: usize, h: usize) -> Vec<f32> {
        let mut v = uniform_luma(w, h, 0.5);
        // Add sinusoidal noise to a 32×32 patch at (64, 64)
        for y in 64..(64 + 32).min(h) {
            for x in 64..(64 + 32).min(w) {
                v[y * w + x] = 0.5 + 0.4 * ((x + y) as f32 * 0.3).sin();
            }
        }
        v
    }

    #[test]
    fn test_mad_noise_uniform() {
        let values = vec![0.5f32; 64];
        let noise = mad_noise(&values);
        assert!(noise < 1e-5, "Uniform values should have ~zero noise");
    }

    #[test]
    fn test_mad_noise_noisy() {
        let values: Vec<f32> = (0..64)
            .map(|i| (i as f32 * 0.3).sin() * 0.4 + 0.5)
            .collect();
        let noise = mad_noise(&values);
        assert!(noise > 0.0, "Varying values should have nonzero MAD noise");
    }

    #[test]
    fn test_estimate_noise_by_region_size() {
        let luma = uniform_luma(128, 128, 0.5);
        let det = SplicingDetector::default();
        let map = det.estimate_noise_by_region(&luma, 128, 128, 32);
        let expected_blocks = 4 * 4; // 128/32 = 4 in each dim
        assert_eq!(map.len(), expected_blocks);
    }

    #[test]
    fn test_uniform_image_no_splicing() {
        let luma = uniform_luma(128, 128, 0.5);
        let det = SplicingDetector::default();
        let indicators = det.analyze(&luma, 128, 128);
        // All blocks have identical (zero) noise → no outliers
        assert!(
            indicators.is_empty(),
            "Uniform image should have no splicing indicators"
        );
    }

    #[test]
    fn test_noisy_patch_detected() {
        let luma = luma_with_noise_patch(128, 128);
        let det = SplicingDetector::with_params(32, 2.0); // lower threshold for test
        let indicators = det.analyze(&luma, 128, 128);
        assert!(
            !indicators.is_empty(),
            "High-noise patch should be detected as a splicing indicator"
        );
    }

    #[test]
    fn test_splicing_indicator_fields() {
        let luma = luma_with_noise_patch(128, 128);
        let det = SplicingDetector::with_params(32, 1.5);
        let indicators = det.analyze(&luma, 128, 128);
        for ind in &indicators {
            assert!(ind.noise_level >= 0.0);
            assert!(ind.expected_noise >= 0.0);
            assert!(ind.region.2 > 0 && ind.region.3 > 0);
        }
    }

    #[test]
    fn test_detect_splicing_empty_noise_map() {
        let det = SplicingDetector::default();
        let indicators = det.detect_splicing(&[], 64, 64);
        assert!(indicators.is_empty());
    }

    #[test]
    fn test_sigma_threshold_sensitivity() {
        let luma = luma_with_noise_patch(128, 128);
        let strict = SplicingDetector::with_params(32, 5.0);
        let lenient = SplicingDetector::with_params(32, 1.0);
        let strict_result = strict.analyze(&luma, 128, 128);
        let lenient_result = lenient.analyze(&luma, 128, 128);
        // Lenient threshold should find at least as many regions
        assert!(lenient_result.len() >= strict_result.len());
    }

    #[test]
    fn test_partial_block_at_edge() {
        // Image size not a multiple of block_size
        let luma = uniform_luma(100, 100, 0.5);
        let det = SplicingDetector::with_params(32, 2.5);
        let map = det.estimate_noise_by_region(&luma, 100, 100, 32);
        // Should not panic; blocks_x = ceil(100/32) = 4, blocks_y = 4
        assert_eq!(map.len(), 4 * 4);
    }
}
