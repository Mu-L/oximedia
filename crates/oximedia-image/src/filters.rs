//! Image filter operations: convolution, thresholding, histogram equalization,
//! and median filtering for professional image processing pipelines.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]

/// A 2-D convolution kernel stored in row-major order.
#[derive(Debug, Clone)]
pub struct ConvolutionKernel {
    /// Kernel coefficients in row-major order.
    pub data: Vec<f32>,
    /// Kernel width in pixels.
    pub width: usize,
    /// Kernel height in pixels.
    pub height: usize,
}

impl ConvolutionKernel {
    /// Create a new kernel from raw data.
    ///
    /// # Panics
    ///
    /// Panics if `data.len() != width * height`.
    #[must_use]
    pub fn new(data: Vec<f32>, width: usize, height: usize) -> Self {
        assert_eq!(data.len(), width * height, "kernel data length mismatch");
        Self {
            data,
            width,
            height,
        }
    }

    /// Divide every coefficient by the sum so the kernel preserves mean brightness.
    ///
    /// If the sum is zero the kernel is left unchanged.
    pub fn normalize(&mut self) {
        let sum: f32 = self.data.iter().sum();
        if sum.abs() > 1e-9 {
            for v in &mut self.data {
                *v /= sum;
            }
        }
    }

    /// Build a separable Gaussian kernel of the given `size` (must be odd) and `sigma`.
    #[must_use]
    pub fn gaussian(sigma: f32, size: usize) -> Self {
        let size = if size % 2 == 0 { size + 1 } else { size };
        let half = (size / 2) as i32;
        let mut data = Vec::with_capacity(size * size);
        for ky in -(half)..=half {
            for kx in -(half)..=half {
                let exp = -((kx * kx + ky * ky) as f32) / (2.0 * sigma * sigma);
                data.push(exp.exp());
            }
        }
        let mut k = Self {
            data,
            width: size,
            height: size,
        };
        k.normalize();
        k
    }

    /// Standard 3×3 unsharp / detail-enhance kernel.
    #[must_use]
    pub fn sharpen() -> Self {
        Self::new(vec![0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0], 3, 3)
    }

    /// Classic 3×3 emboss kernel.
    #[must_use]
    pub fn emboss() -> Self {
        Self::new(vec![-2.0, -1.0, 0.0, -1.0, 1.0, 1.0, 0.0, 1.0, 2.0], 3, 3)
    }
}

/// Apply a 2-D convolution kernel to a single-channel `f32` image.
///
/// `src` and `dst` must both have length `width * height`. Border pixels are
/// handled via clamp-to-edge.
pub fn apply_convolution(
    src: &[f32],
    dst: &mut [f32],
    width: usize,
    height: usize,
    kernel: &ConvolutionKernel,
) {
    let kw = kernel.width as i64;
    let kh = kernel.height as i64;
    let hw = kw / 2;
    let hh = kh / 2;
    let iw = width as i64;
    let ih = height as i64;

    for py in 0..ih {
        for px in 0..iw {
            let mut acc = 0.0f32;
            for ky in 0..kh {
                for kx in 0..kw {
                    let sx = (px + kx - hw).clamp(0, iw - 1) as usize;
                    let sy = (py + ky - hh).clamp(0, ih - 1) as usize;
                    let coeff = kernel.data[(ky * kw + kx) as usize];
                    acc += coeff * src[sy * width + sx];
                }
            }
            dst[py as usize * width + px as usize] = acc;
        }
    }
}

/// Binarize a single-channel `u8` image: pixels < `threshold` → 0, else → 255.
pub fn threshold(src: &[u8], dst: &mut [u8], thr: u8) {
    for (d, &s) in dst.iter_mut().zip(src.iter()) {
        *d = if s < thr { 0 } else { 255 };
    }
}

/// Global histogram equalization for a single-channel `u8` image.
///
/// Remaps pixel intensities so that the cumulative distribution is linear.
pub fn equalize_histogram(src: &[u8], dst: &mut [u8]) {
    let n = src.len();
    if n == 0 {
        return;
    }

    // Build histogram
    let mut hist = [0u32; 256];
    for &p in src {
        hist[p as usize] += 1;
    }

    // Compute CDF
    let mut cdf = [0u32; 256];
    cdf[0] = hist[0];
    for i in 1..256 {
        cdf[i] = cdf[i - 1] + hist[i];
    }

    // Find first non-zero CDF value
    let cdf_min = *cdf.iter().find(|&&v| v > 0).unwrap_or(&0);

    // Build LUT
    let range = (n as u32).saturating_sub(cdf_min);
    let mut lut = [0u8; 256];
    for (i, lut_val) in lut.iter_mut().enumerate() {
        if range == 0 {
            *lut_val = i as u8;
        } else {
            let eq = (cdf[i].saturating_sub(cdf_min) as f32 / range as f32 * 255.0)
                .clamp(0.0, 255.0)
                .round() as u8;
            *lut_val = eq;
        }
    }

    for (d, &s) in dst.iter_mut().zip(src.iter()) {
        *d = lut[s as usize];
    }
}

/// 3×3 median filter for a single-channel `u8` image.
///
/// Border pixels are clamped to the nearest valid source pixel.
pub fn median_filter_3x3(src: &[u8], dst: &mut [u8], width: usize, height: usize) {
    let iw = width as i64;
    let ih = height as i64;

    for py in 0..ih {
        for px in 0..iw {
            let mut window = [0u8; 9];
            let mut count = 0usize;
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let sy = (py + dy).clamp(0, ih - 1) as usize;
                    let sx = (px + dx).clamp(0, iw - 1) as usize;
                    window[count] = src[sy * width + sx];
                    count += 1;
                }
            }
            window[..count].sort_unstable();
            dst[py as usize * width + px as usize] = window[count / 2];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- ConvolutionKernel ----

    #[test]
    fn test_sharpen_kernel_size() {
        let k = ConvolutionKernel::sharpen();
        assert_eq!(k.width, 3);
        assert_eq!(k.height, 3);
        assert_eq!(k.data.len(), 9);
    }

    #[test]
    fn test_sharpen_kernel_sum() {
        let k = ConvolutionKernel::sharpen();
        let sum: f32 = k.data.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6, "sum = {sum}");
    }

    #[test]
    fn test_emboss_kernel_size() {
        let k = ConvolutionKernel::emboss();
        assert_eq!(k.data.len(), 9);
    }

    #[test]
    fn test_gaussian_kernel_sum_near_one() {
        let k = ConvolutionKernel::gaussian(1.0, 5);
        let sum: f32 = k.data.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "gaussian sum = {sum}");
    }

    #[test]
    fn test_gaussian_kernel_odd_size_enforced() {
        // Even size should be bumped to odd
        let k = ConvolutionKernel::gaussian(1.0, 4);
        assert_eq!(k.width, 5);
    }

    #[test]
    fn test_normalize_divides_by_sum() {
        let mut k = ConvolutionKernel::new(vec![1.0, 1.0, 1.0, 1.0], 2, 2);
        k.normalize();
        let sum: f32 = k.data.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_zero_sum_unchanged() {
        let mut k = ConvolutionKernel::new(vec![1.0, -1.0], 2, 1);
        k.normalize(); // sum == 0, should be no-op
        assert!((k.data[0] - 1.0).abs() < 1e-6);
    }

    // ---- apply_convolution ----

    #[test]
    fn test_apply_convolution_identity() {
        // Identity kernel (1 at center) should copy src to dst
        let src = vec![0.1f32, 0.5, 0.9, 0.3];
        let mut dst = vec![0.0f32; 4];
        let kernel = ConvolutionKernel::new(vec![1.0], 1, 1);
        apply_convolution(&src, &mut dst, 2, 2, &kernel);
        for (s, d) in src.iter().zip(dst.iter()) {
            assert!((s - d).abs() < 1e-6);
        }
    }

    #[test]
    fn test_apply_convolution_output_length() {
        let src = vec![0.5f32; 4 * 4];
        let mut dst = vec![0.0f32; 4 * 4];
        let k = ConvolutionKernel::sharpen();
        apply_convolution(&src, &mut dst, 4, 4, &k);
        assert_eq!(dst.len(), 16);
    }

    #[test]
    fn test_apply_convolution_uniform_sharpen() {
        // Sharpen applied to a flat image should leave every pixel at 0.5
        let src = vec![0.5f32; 9];
        let mut dst = vec![0.0f32; 9];
        let k = ConvolutionKernel::sharpen();
        apply_convolution(&src, &mut dst, 3, 3, &k);
        for &v in &dst {
            assert!((v - 0.5).abs() < 1e-5, "v = {v}");
        }
    }

    // ---- threshold ----

    #[test]
    fn test_threshold_basic() {
        let src = vec![50u8, 100, 150, 200];
        let mut dst = vec![0u8; 4];
        threshold(&src, &mut dst, 128);
        assert_eq!(dst, vec![0, 0, 255, 255]);
    }

    #[test]
    fn test_threshold_boundary() {
        let src = vec![127u8, 128];
        let mut dst = vec![0u8; 2];
        threshold(&src, &mut dst, 128);
        assert_eq!(dst[0], 0); // 127 < 128
        assert_eq!(dst[1], 255); // 128 >= 128
    }

    // ---- equalize_histogram ----

    #[test]
    fn test_equalize_histogram_length() {
        let src = vec![10u8, 20, 30, 40];
        let mut dst = vec![0u8; 4];
        equalize_histogram(&src, &mut dst);
        assert_eq!(dst.len(), 4);
    }

    #[test]
    fn test_equalize_histogram_uniform_input() {
        // All-same input: all outputs should also be the same
        let src = vec![128u8; 16];
        let mut dst = vec![0u8; 16];
        equalize_histogram(&src, &mut dst);
        assert!(dst.iter().all(|&v| v == dst[0]));
    }

    #[test]
    fn test_equalize_histogram_empty() {
        let src: Vec<u8> = Vec::new();
        let mut dst: Vec<u8> = Vec::new();
        equalize_histogram(&src, &mut dst); // should not panic
    }

    // ---- median_filter_3x3 ----

    #[test]
    fn test_median_filter_length() {
        let src = vec![0u8; 4 * 4];
        let mut dst = vec![0u8; 4 * 4];
        median_filter_3x3(&src, &mut dst, 4, 4);
        assert_eq!(dst.len(), 16);
    }

    #[test]
    fn test_median_filter_uniform_unchanged() {
        let src = vec![128u8; 3 * 3];
        let mut dst = vec![0u8; 3 * 3];
        median_filter_3x3(&src, &mut dst, 3, 3);
        assert!(dst.iter().all(|&v| v == 128));
    }

    #[test]
    fn test_median_filter_removes_spike() {
        // 3×3 image: all 10 except center pixel which is 200.
        let mut src = vec![10u8; 9];
        src[4] = 200; // center spike
        let mut dst = vec![0u8; 9];
        median_filter_3x3(&src, &mut dst, 3, 3);
        // Center pixel's 3×3 neighborhood (all clamped to same 3×3 image):
        // sorted window is dominated by 10s, so median should be 10.
        assert_eq!(dst[4], 10);
    }
}
