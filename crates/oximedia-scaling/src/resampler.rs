//! High-quality image/video resampler with separable filter passes.
//!
//! Supports a variety of filter kernels from nearest-neighbor to Lanczos5,
//! suitable for both upscaling and downscaling operations.

use std::f32::consts::PI;

/// Resampling filter kernel.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterKernel {
    /// Nearest-neighbor (box) filter - fastest, lowest quality.
    Nearest,
    /// Bilinear (linear tent) filter - fast, decent quality.
    Bilinear,
    /// Bicubic filter with B=0, C=0.5 (Mitchell-Netravali).
    Bicubic,
    /// Lanczos with support radius 3 - high quality.
    Lanczos3,
    /// Lanczos with support radius 5 - very high quality.
    Lanczos5,
    /// Mitchell-Netravali filter with configurable B and C.
    MitchellNetravali,
    /// Spline16 filter.
    Spline16,
}

impl FilterKernel {
    /// Return the support radius of this kernel.
    #[must_use]
    #[allow(dead_code)]
    pub fn support(&self) -> f32 {
        match self {
            Self::Nearest => 0.5,
            Self::Bilinear => 1.0,
            Self::Bicubic => 2.0,
            Self::Lanczos3 => 3.0,
            Self::Lanczos5 => 5.0,
            Self::MitchellNetravali => 2.0,
            Self::Spline16 => 2.0,
        }
    }

    /// Evaluate the kernel at position `x`.
    #[must_use]
    #[allow(dead_code)]
    pub fn evaluate(&self, x: f32) -> f32 {
        match self {
            Self::Nearest => {
                if x.abs() <= 0.5 {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Bilinear => (1.0 - x.abs()).max(0.0),
            Self::Bicubic => mitchell_netravali(x, 0.0, 0.5),
            Self::Lanczos3 => lanczos(x, 3.0),
            Self::Lanczos5 => lanczos(x, 5.0),
            Self::MitchellNetravali => mitchell_netravali(x, 1.0 / 3.0, 1.0 / 3.0),
            Self::Spline16 => spline16(x),
        }
    }
}

/// Sinc function: sin(PI*x) / (PI*x), returning 1.0 for x=0.
#[inline]
fn sinc(x: f32) -> f32 {
    if x.abs() < 1e-8 {
        1.0
    } else {
        (PI * x).sin() / (PI * x)
    }
}

/// Lanczos filter of given radius.
#[inline]
fn lanczos(x: f32, radius: f32) -> f32 {
    let ax = x.abs();
    if ax >= radius {
        return 0.0;
    }
    if ax < 1e-8 {
        return 1.0;
    }
    sinc(ax) * sinc(ax / radius)
}

/// Mitchell-Netravali filter.
#[inline]
fn mitchell_netravali(x: f32, b: f32, c: f32) -> f32 {
    let ax = x.abs();
    if ax < 1.0 {
        ((12.0 - 9.0 * b - 6.0 * c) * ax.powi(3)
            + (-18.0 + 12.0 * b + 6.0 * c) * ax.powi(2)
            + (6.0 - 2.0 * b))
            / 6.0
    } else if ax < 2.0 {
        ((-b - 6.0 * c) * ax.powi(3)
            + (6.0 * b + 30.0 * c) * ax.powi(2)
            + (-12.0 * b - 48.0 * c) * ax
            + (8.0 * b + 24.0 * c))
            / 6.0
    } else {
        0.0
    }
}

/// Spline16 filter (local polynomial spline).
#[inline]
fn spline16(x: f32) -> f32 {
    let ax = x.abs();
    if ax < 1.0 {
        ((ax - 9.0 / 5.0) * ax - 1.0 / 5.0) * ax + 1.0
    } else if ax < 2.0 {
        ((-1.0 / 3.0 * (ax - 1.0) + 4.0 / 5.0) * (ax - 1.0) - 7.0 / 15.0) * (ax - 1.0)
    } else {
        0.0
    }
}

/// Configuration for the resampler.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ResamplerConfig {
    /// Filter kernel to use.
    pub filter: FilterKernel,
    /// Pre-blur sigma (0.0 = no blur).
    pub pre_blur: f32,
    /// Sharpening amount (0.0 = no sharpening).
    pub sharpening: f32,
}

impl Default for ResamplerConfig {
    fn default() -> Self {
        Self {
            filter: FilterKernel::Lanczos3,
            pre_blur: 0.0,
            sharpening: 0.0,
        }
    }
}

/// High-quality image/video resampler.
pub struct Resampler;

impl Resampler {
    /// Resize a single-channel float image from `(src_w, src_h)` to `(dst_w, dst_h)`.
    ///
    /// The image is stored in row-major order with one `f32` per pixel.
    /// Uses separable horizontal-then-vertical passes for efficiency.
    #[must_use]
    #[allow(dead_code)]
    pub fn resize(
        src: &[f32],
        src_w: u32,
        src_h: u32,
        dst_w: u32,
        dst_h: u32,
        config: &ResamplerConfig,
    ) -> Vec<f32> {
        // Horizontal pass: src_w -> dst_w, height stays src_h
        let h_pass = Self::resize_horizontal(src, src_w, dst_w, src_h, &config.filter);
        // Vertical pass: src_h -> dst_h, width is now dst_w
        Self::resize_vertical(&h_pass, dst_w, src_h, dst_h, &config.filter)
    }

    /// Horizontal resampling pass: changes width from `src_w` to `dst_w`.
    #[must_use]
    #[allow(dead_code)]
    pub fn resize_horizontal(
        src: &[f32],
        src_w: u32,
        dst_w: u32,
        height: u32,
        filter: &FilterKernel,
    ) -> Vec<f32> {
        if src_w == 0 || dst_w == 0 || height == 0 {
            return Vec::new();
        }

        let sw = src_w as usize;
        let dw = dst_w as usize;
        let h = height as usize;
        let scale = sw as f32 / dw as f32;
        let support = filter.support();
        let filter_scale = if scale > 1.0 { scale } else { 1.0 };
        let effective_support = support * filter_scale;

        let mut dst = vec![0.0f32; dw * h];

        for y in 0..h {
            for dx in 0..dw {
                let center = (dx as f32 + 0.5) * scale - 0.5;
                let start = ((center - effective_support).ceil() as i64).max(0) as usize;
                let end = ((center + effective_support).floor() as i64 + 1).min(sw as i64) as usize;

                let mut weight_sum = 0.0f32;
                let mut value = 0.0f32;

                for sx in start..end {
                    let w = filter.evaluate((sx as f32 - center) / filter_scale) / filter_scale;
                    let w = w.max(0.0); // Some filters can return small negatives near edge
                    value += src[y * sw + sx] * w;
                    weight_sum += w;
                }

                dst[y * dw + dx] = if weight_sum > 1e-8 {
                    value / weight_sum
                } else {
                    0.0
                };
            }
        }

        dst
    }

    /// Vertical resampling pass: changes height from `src_h` to `dst_h`.
    #[must_use]
    #[allow(dead_code)]
    pub fn resize_vertical(
        src: &[f32],
        width: u32,
        src_h: u32,
        dst_h: u32,
        filter: &FilterKernel,
    ) -> Vec<f32> {
        if width == 0 || src_h == 0 || dst_h == 0 {
            return Vec::new();
        }

        let w = width as usize;
        let sh = src_h as usize;
        let dh = dst_h as usize;
        let scale = sh as f32 / dh as f32;
        let support = filter.support();
        let filter_scale = if scale > 1.0 { scale } else { 1.0 };
        let effective_support = support * filter_scale;

        let mut dst = vec![0.0f32; w * dh];

        for dy in 0..dh {
            let center = (dy as f32 + 0.5) * scale - 0.5;
            let start = ((center - effective_support).ceil() as i64).max(0) as usize;
            let end = ((center + effective_support).floor() as i64 + 1).min(sh as i64) as usize;

            for x in 0..w {
                let mut weight_sum = 0.0f32;
                let mut value = 0.0f32;

                for sy in start..end {
                    let w_val = filter.evaluate((sy as f32 - center) / filter_scale) / filter_scale;
                    let w_val = w_val.max(0.0);
                    value += src[sy * w + x] * w_val;
                    weight_sum += w_val;
                }

                dst[dy * w + x] = if weight_sum > 1e-8 {
                    value / weight_sum
                } else {
                    0.0
                };
            }
        }

        dst
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_support_nearest() {
        assert!((FilterKernel::Nearest.support() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_filter_support_bilinear() {
        assert!((FilterKernel::Bilinear.support() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_filter_support_lanczos3() {
        assert!((FilterKernel::Lanczos3.support() - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_filter_evaluate_bilinear_center() {
        assert!((FilterKernel::Bilinear.evaluate(0.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_filter_evaluate_bilinear_edge() {
        assert!((FilterKernel::Bilinear.evaluate(1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_filter_evaluate_lanczos3_center() {
        assert!((FilterKernel::Lanczos3.evaluate(0.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_filter_evaluate_lanczos3_outside() {
        assert!((FilterKernel::Lanczos3.evaluate(3.5)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_resize_same_size() {
        let src: Vec<f32> = (0..16).map(|i| i as f32).collect();
        let config = ResamplerConfig::default();
        let dst = Resampler::resize(&src, 4, 4, 4, 4, &config);
        assert_eq!(dst.len(), 16);
        // Values should remain approximately the same
        for (a, b) in src.iter().zip(dst.iter()) {
            assert!((a - b).abs() < 0.5, "src={a} dst={b}");
        }
    }

    #[test]
    fn test_resize_upscale_2x() {
        let src = vec![0.0f32, 1.0, 0.0, 1.0];
        let config = ResamplerConfig {
            filter: FilterKernel::Bilinear,
            ..Default::default()
        };
        let dst = Resampler::resize(&src, 2, 2, 4, 4, &config);
        assert_eq!(dst.len(), 16);
    }

    #[test]
    fn test_resize_downscale_2x() {
        // 4x4 checkerboard
        let mut src = vec![0.0f32; 16];
        for y in 0..4 {
            for x in 0..4 {
                src[y * 4 + x] = if (x + y) % 2 == 0 { 1.0 } else { 0.0 };
            }
        }
        let config = ResamplerConfig {
            filter: FilterKernel::Bilinear,
            ..Default::default()
        };
        let dst = Resampler::resize(&src, 4, 4, 2, 2, &config);
        assert_eq!(dst.len(), 4);
    }

    #[test]
    fn test_resize_horizontal_identity() {
        let src: Vec<f32> = (0..12).map(|i| i as f32).collect();
        let dst = Resampler::resize_horizontal(&src, 4, 4, 3, &FilterKernel::Bilinear);
        assert_eq!(dst.len(), 12);
    }

    #[test]
    fn test_resize_vertical_identity() {
        let src: Vec<f32> = (0..12).map(|i| i as f32).collect();
        let dst = Resampler::resize_vertical(&src, 4, 3, 3, &FilterKernel::Bilinear);
        assert_eq!(dst.len(), 12);
    }

    #[test]
    fn test_nearest_filter() {
        let src = vec![1.0f32, 2.0, 3.0, 4.0];
        let config = ResamplerConfig {
            filter: FilterKernel::Nearest,
            ..Default::default()
        };
        let dst = Resampler::resize(&src, 2, 2, 4, 4, &config);
        assert_eq!(dst.len(), 16);
        // Corners should be original values
        assert!((dst[0] - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_mitchell_netravali_center() {
        // MN with B=1/3, C=1/3: peak = (6 - 2B)/6 = (6 - 2/3)/6 = 16/18 ≈ 0.8889
        let expected = (6.0 - 2.0 / 3.0) / 6.0;
        assert!((FilterKernel::MitchellNetravali.evaluate(0.0) - expected).abs() < 0.001);
    }

    #[test]
    fn test_spline16_center() {
        assert!((FilterKernel::Spline16.evaluate(0.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_resize_empty_returns_empty() {
        let src: Vec<f32> = Vec::new();
        let config = ResamplerConfig::default();
        let dst = Resampler::resize(&src, 0, 0, 4, 4, &config);
        assert!(dst.is_empty());
    }

    #[test]
    fn test_sinc_at_zero() {
        assert!((sinc(0.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_lanczos5_support() {
        assert!((FilterKernel::Lanczos5.support() - 5.0).abs() < f32::EPSILON);
        assert!((FilterKernel::Lanczos5.evaluate(5.5)).abs() < f32::EPSILON);
    }
}
