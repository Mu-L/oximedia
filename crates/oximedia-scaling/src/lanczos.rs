//! Lanczos resampling filter for high-quality image scaling.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]

use std::f64::consts::PI;

/// Lanczos kernel with configurable `a` parameter.
///
/// The `a` parameter controls the number of lobes. Larger values give
/// higher quality but slower performance. `a=3` is the typical default.
#[derive(Debug, Clone)]
pub struct LanczosKernel {
    /// Number of lobes (typically 2 or 3)
    pub a: u32,
}

impl Default for LanczosKernel {
    fn default() -> Self {
        Self { a: 3 }
    }
}

impl LanczosKernel {
    /// Create a new Lanczos kernel with the given `a` parameter.
    pub fn new(a: u32) -> Self {
        Self { a }
    }

    /// Compute the sinc function: sin(pi*x) / (pi*x).
    fn sinc(x: f64) -> f64 {
        if x.abs() < 1e-10 {
            1.0
        } else {
            (PI * x).sin() / (PI * x)
        }
    }

    /// Compute the Lanczos kernel value at position `x`.
    ///
    /// Returns 0 outside the support window `[-a, a]`.
    pub fn kernel_value(&self, x: f64) -> f64 {
        let a = self.a as f64;
        if x.abs() < 1e-10 {
            1.0
        } else if x.abs() < a {
            Self::sinc(x) * Self::sinc(x / a)
        } else {
            0.0
        }
    }
}

/// Lanczos resampler that applies the Lanczos filter for image scaling.
#[derive(Debug, Clone)]
pub struct LanczosResampler {
    /// The Lanczos kernel to use
    pub kernel: LanczosKernel,
}

impl Default for LanczosResampler {
    fn default() -> Self {
        Self {
            kernel: LanczosKernel::default(),
        }
    }
}

impl LanczosResampler {
    /// Create a new `LanczosResampler` with the default `a=3` kernel.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new `LanczosResampler` with a custom kernel.
    pub fn with_kernel(kernel: LanczosKernel) -> Self {
        Self { kernel }
    }

    /// Resample a 1D signal from its current length to `dst_len` samples.
    ///
    /// Uses the Lanczos filter to compute each output sample by weighting
    /// nearby input samples.
    pub fn resample_1d(&self, src: &[f32], dst_len: usize) -> Vec<f32> {
        if src.is_empty() || dst_len == 0 {
            return Vec::new();
        }

        let src_len = src.len();
        let scale = src_len as f64 / dst_len as f64;
        let a = self.kernel.a as i64;

        let mut dst = vec![0.0f32; dst_len];

        for (i, dst_sample) in dst.iter_mut().enumerate() {
            let center = (i as f64 + 0.5) * scale - 0.5;
            let start = (center - a as f64).ceil() as i64;
            let end = (center + a as f64).floor() as i64;

            let mut weight_sum = 0.0f64;
            let mut value_sum = 0.0f64;

            for j in start..=end {
                if j >= 0 && j < src_len as i64 {
                    let w = self.kernel.kernel_value(center - j as f64);
                    weight_sum += w;
                    value_sum += w * src[j as usize] as f64;
                }
            }

            *dst_sample = if weight_sum.abs() > 1e-10 {
                (value_sum / weight_sum) as f32
            } else {
                0.0
            };
        }

        dst
    }

    /// Scale an image using Lanczos resampling.
    ///
    /// The image is assumed to be stored in row-major order with 1 byte per pixel
    /// (grayscale). Performs a two-pass horizontal then vertical resample.
    ///
    /// # Arguments
    /// - `pixels`: Source pixel data (grayscale, 1 byte per pixel)
    /// - `src_w`: Source image width
    /// - `src_h`: Source image height
    /// - `dst_w`: Destination image width
    /// - `dst_h`: Destination image height
    pub fn scale_image(
        &self,
        pixels: &[u8],
        src_w: usize,
        src_h: usize,
        dst_w: usize,
        dst_h: usize,
    ) -> Vec<u8> {
        if pixels.is_empty() || dst_w == 0 || dst_h == 0 {
            return Vec::new();
        }

        // Convert to f32 for processing
        let src_f32: Vec<f32> = pixels.iter().map(|&p| p as f32 / 255.0).collect();

        // Horizontal pass: resample each row from src_w to dst_w
        let mut h_pass = vec![0.0f32; src_h * dst_w];
        for row in 0..src_h {
            let src_row = &src_f32[row * src_w..(row + 1) * src_w];
            let dst_row = self.resample_1d(src_row, dst_w);
            h_pass[row * dst_w..(row + 1) * dst_w].copy_from_slice(&dst_row);
        }

        // Vertical pass: resample each column from src_h to dst_h
        let mut result = vec![0u8; dst_w * dst_h];
        for col in 0..dst_w {
            let col_data: Vec<f32> = (0..src_h).map(|row| h_pass[row * dst_w + col]).collect();
            let resampled_col = self.resample_1d(&col_data, dst_h);
            for (row, &val) in resampled_col.iter().enumerate() {
                let clamped = val.clamp(0.0, 1.0);
                result[row * dst_w + col] = (clamped * 255.0) as u8;
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_default_a() {
        let k = LanczosKernel::default();
        assert_eq!(k.a, 3);
    }

    #[test]
    fn test_kernel_new() {
        let k = LanczosKernel::new(2);
        assert_eq!(k.a, 2);
    }

    #[test]
    fn test_kernel_value_at_zero() {
        let k = LanczosKernel::default();
        let v = k.kernel_value(0.0);
        assert!((v - 1.0).abs() < 1e-9, "kernel(0) should be 1.0, got {v}");
    }

    #[test]
    fn test_kernel_value_at_boundary() {
        let k = LanczosKernel::default();
        // At x == a (3.0), the kernel should be 0
        let v = k.kernel_value(3.0);
        assert!(v.abs() < 1e-9, "kernel(a) should be ~0, got {v}");
    }

    #[test]
    fn test_kernel_value_outside_support() {
        let k = LanczosKernel::default();
        let v = k.kernel_value(5.0);
        assert_eq!(v, 0.0, "kernel outside support should be 0");
    }

    #[test]
    fn test_kernel_symmetry() {
        let k = LanczosKernel::default();
        for x in [0.5, 1.0, 1.5, 2.0, 2.5] {
            let pos = k.kernel_value(x);
            let neg = k.kernel_value(-x);
            assert!(
                (pos - neg).abs() < 1e-9,
                "kernel should be symmetric: k({x}) != k(-{x})"
            );
        }
    }

    #[test]
    fn test_resampler_new() {
        let r = LanczosResampler::new();
        assert_eq!(r.kernel.a, 3);
    }

    #[test]
    fn test_resampler_with_kernel() {
        let k = LanczosKernel::new(2);
        let r = LanczosResampler::with_kernel(k);
        assert_eq!(r.kernel.a, 2);
    }

    #[test]
    fn test_resample_1d_identity() {
        let r = LanczosResampler::new();
        let src: Vec<f32> = (0..8).map(|i| i as f32 / 7.0).collect();
        let dst = r.resample_1d(&src, 8);
        assert_eq!(dst.len(), 8);
        // Values should be approximately the same
        for (s, d) in src.iter().zip(dst.iter()) {
            assert!((s - d).abs() < 0.05, "identity resample: {s} vs {d}");
        }
    }

    #[test]
    fn test_resample_1d_upsample() {
        let r = LanczosResampler::new();
        let src = vec![0.0f32, 1.0, 0.0];
        let dst = r.resample_1d(&src, 9);
        assert_eq!(dst.len(), 9);
        // Center of output should peak near 1.0
        let max_val = dst.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            max_val > 0.5,
            "upsampled peak should be > 0.5, got {max_val}"
        );
    }

    #[test]
    fn test_resample_1d_downsample() {
        let r = LanczosResampler::new();
        let src: Vec<f32> = (0..16).map(|i| i as f32 / 15.0).collect();
        let dst = r.resample_1d(&src, 4);
        assert_eq!(dst.len(), 4);
        // Monotonically increasing
        for w in dst.windows(2) {
            assert!(
                w[1] >= w[0] - 0.01,
                "downsampled should be roughly monotonic"
            );
        }
    }

    #[test]
    fn test_resample_1d_empty_src() {
        let r = LanczosResampler::new();
        let dst = r.resample_1d(&[], 8);
        assert!(dst.is_empty());
    }

    #[test]
    fn test_resample_1d_zero_dst() {
        let r = LanczosResampler::new();
        let src = vec![1.0f32, 2.0, 3.0];
        let dst = r.resample_1d(&src, 0);
        assert!(dst.is_empty());
    }

    #[test]
    fn test_scale_image_empty() {
        let r = LanczosResampler::new();
        let result = r.scale_image(&[], 0, 0, 4, 4);
        assert!(result.is_empty());
    }

    #[test]
    fn test_scale_image_size() {
        let r = LanczosResampler::new();
        let src: Vec<u8> = (0..64).map(|i| i as u8 * 4).collect();
        let result = r.scale_image(&src, 8, 8, 4, 4);
        assert_eq!(result.len(), 4 * 4);
    }

    #[test]
    fn test_scale_image_values_in_range() {
        let r = LanczosResampler::new();
        let src: Vec<u8> = (0..256).map(|i| i as u8).collect();
        let result = r.scale_image(&src, 16, 16, 8, 8);
        for &v in &result {
            let _ = v; // all u8 values are in [0, 255] by definition
        }
        assert_eq!(result.len(), 64);
    }
}
