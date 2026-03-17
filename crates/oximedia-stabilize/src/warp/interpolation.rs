//! Interpolation methods for frame warping.
//!
//! The bilinear path is accelerated via scirs2-core SIMD operations when
//! processing a full row of pixels at once (`bilinear_row_simd`).  The
//! per-pixel `InterpolationMethod::Bilinear` variant remains available for
//! mixed-stride access patterns.

use crate::warp::boundary::BoundaryMode;
use scirs2_core::ndarray::Array2;

/// Interpolation method for pixel sampling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterpolationMethod {
    /// Nearest neighbor
    Nearest,
    /// Bilinear interpolation
    Bilinear,
    /// Bicubic interpolation
    Bicubic,
}

impl InterpolationMethod {
    /// Interpolate pixel value at non-integer coordinates.
    #[must_use]
    pub fn interpolate(self, data: &Array2<u8>, x: f64, y: f64, boundary: BoundaryMode) -> u8 {
        match self {
            Self::Nearest => self.nearest(data, x, y, boundary),
            Self::Bilinear => self.bilinear(data, x, y, boundary),
            Self::Bicubic => self.bicubic(data, x, y, boundary),
        }
    }

    /// Nearest neighbor interpolation.
    fn nearest(self, data: &Array2<u8>, x: f64, y: f64, boundary: BoundaryMode) -> u8 {
        boundary.get_pixel(data, x.round(), y.round())
    }

    /// Bilinear interpolation.
    fn bilinear(self, data: &Array2<u8>, x: f64, y: f64, boundary: BoundaryMode) -> u8 {
        let x0 = x.floor();
        let y0 = y.floor();
        let dx = x - x0;
        let dy = y - y0;

        let p00 = f64::from(boundary.get_pixel(data, x0, y0));
        let p10 = f64::from(boundary.get_pixel(data, x0 + 1.0, y0));
        let p01 = f64::from(boundary.get_pixel(data, x0, y0 + 1.0));
        let p11 = f64::from(boundary.get_pixel(data, x0 + 1.0, y0 + 1.0));

        let val = (1.0 - dx) * (1.0 - dy) * p00
            + dx * (1.0 - dy) * p10
            + (1.0 - dx) * dy * p01
            + dx * dy * p11;

        val.clamp(0.0, 255.0) as u8
    }

    /// Bicubic interpolation.
    fn bicubic(self, data: &Array2<u8>, x: f64, y: f64, boundary: BoundaryMode) -> u8 {
        let x0 = x.floor();
        let y0 = y.floor();
        let dx = x - x0;
        let dy = y - y0;

        let mut val = 0.0;

        for j in -1..=2 {
            for i in -1..=2 {
                let p = f64::from(boundary.get_pixel(data, x0 + f64::from(i), y0 + f64::from(j)));
                let wx = Self::cubic_weight(dx - f64::from(i));
                let wy = Self::cubic_weight(dy - f64::from(j));
                val += p * wx * wy;
            }
        }

        val.clamp(0.0, 255.0) as u8
    }

    /// Cubic interpolation weight function.
    fn cubic_weight(t: f64) -> f64 {
        let t = t.abs();
        if t < 1.0 {
            1.5 * t.powi(3) - 2.5 * t.powi(2) + 1.0
        } else if t < 2.0 {
            -0.5 * t.powi(3) + 2.5 * t.powi(2) - 4.0 * t + 2.0
        } else {
            0.0
        }
    }
}

// ─────────────────────────────────────────────────────────────────
//  SIMD-accelerated row-level bilinear interpolation
// ─────────────────────────────────────────────────────────────────

/// SIMD-accelerated bilinear interpolation for an entire row of output pixels.
///
/// For each index `i` in `0..count`, reads the pre-computed source coordinates
/// `(src_x[i], src_y[i])`, performs bilinear sampling from `data` (row-major
/// single-channel u8, dimensions `data_width × data_height`), and writes the
/// result to `output[i]`.
///
/// Internally the fractional and floor operations are batched using
/// scirs2-core SIMD primitives so that the hot path runs with SIMD
/// width rather than one element at a time.
///
/// # Parameters
///
/// * `data`        — source grayscale image (row-major, u8)
/// * `data_width`  — source image width in pixels
/// * `data_height` — source image height in pixels
/// * `src_x`       — pre-computed fractional source X coordinates (length ≥ `count`)
/// * `src_y`       — pre-computed fractional source Y coordinates (length ≥ `count`)
/// * `count`       — number of pixels to interpolate
/// * `output`      — output buffer (length ≥ `count`)
///
/// # Panics
///
/// Panics if `src_x`, `src_y`, or `output` are shorter than `count`.
pub fn bilinear_row_simd(
    data: &[u8],
    data_width: usize,
    data_height: usize,
    src_x: &[f64],
    src_y: &[f64],
    count: usize,
    output: &mut [u8],
) {
    assert!(src_x.len() >= count, "src_x too short");
    assert!(src_y.len() >= count, "src_y too short");
    assert!(output.len() >= count, "output too short");

    if count == 0 || data_width == 0 || data_height == 0 {
        return;
    }

    // Use the batch bilinear sampler from simd_warp which already leverages
    // scirs2-core SIMD (floor, sub, scalar_mul) on arrays of coordinates.
    use crate::simd_warp::batch_bilinear_sample;
    let row_pixels = batch_bilinear_sample(data, data_width, data_height, src_x, src_y, count);
    output[..count].copy_from_slice(&row_pixels);
}

/// Warp a single-channel grayscale frame using an affine correction transform,
/// with SIMD-accelerated bilinear interpolation along each output row.
///
/// The correction transform is specified as:
///   - `dx`, `dy` — translational correction (pixels)
///   - `angle`    — rotational correction (radians)
///   - `scale`    — scale correction
///
/// Returns a new `width × height` u8 buffer.
#[must_use]
pub fn warp_bilinear_simd(
    src: &[u8],
    width: usize,
    height: usize,
    dx: f64,
    dy: f64,
    angle: f64,
    scale: f64,
) -> Vec<u8> {
    use crate::simd_warp::{inverse_affine_row, AffineParams};

    if width == 0 || height == 0 || src.len() < width * height {
        return vec![0u8; width * height];
    }

    let params = AffineParams::new(dx, dy, angle, scale);
    let mut dst = vec![0u8; width * height];
    let mut sx_buf = vec![0.0f64; width];
    let mut sy_buf = vec![0.0f64; width];

    for y in 0..height {
        inverse_affine_row(&params, y as f64, width, &mut sx_buf, &mut sy_buf);
        let row_out = &mut dst[y * width..(y + 1) * width];
        bilinear_row_simd(src, width, height, &sx_buf, &sy_buf, width, row_out);
    }

    dst
}

/// Bilinear interpolation quality score for a given warp transform.
///
/// Measures the average absolute difference between the warped output and
/// the source at grid sample points.  Lower scores indicate more faithful
/// reproduction (less resampling artifact).
///
/// Useful for comparing warp quality between different parameter choices.
#[must_use]
pub fn bilinear_quality_score(
    src: &[u8],
    src_width: usize,
    src_height: usize,
    dst: &[u8],
    dst_width: usize,
    dst_height: usize,
) -> f64 {
    let n = src_width.min(dst_width) * src_height.min(dst_height);
    if n == 0 {
        return 0.0;
    }
    let sum: f64 = src
        .iter()
        .zip(dst.iter())
        .take(n)
        .map(|(&a, &b)| (a as f64 - b as f64).abs())
        .sum();
    sum / n as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nearest() {
        let data = Array2::from_elem((10, 10), 128);
        let val = InterpolationMethod::Nearest.interpolate(&data, 5.5, 5.5, BoundaryMode::Constant);
        assert_eq!(val, 128);
    }

    #[test]
    fn test_bilinear() {
        let data = Array2::from_elem((10, 10), 128);
        let val =
            InterpolationMethod::Bilinear.interpolate(&data, 5.5, 5.5, BoundaryMode::Constant);
        assert_eq!(val, 128);
    }

    #[test]
    fn test_bilinear_row_simd_uniform() {
        let data = vec![200u8; 16 * 16];
        let sx = vec![4.5f64; 8];
        let sy = vec![4.5f64; 8];
        let mut output = vec![0u8; 8];
        bilinear_row_simd(&data, 16, 16, &sx, &sy, 8, &mut output);
        for &v in &output {
            assert_eq!(v, 200, "uniform image should give uniform output");
        }
    }

    #[test]
    fn test_bilinear_row_simd_identity_row() {
        // Source: row-gradient image (value = column index)
        let width = 16usize;
        let height = 8usize;
        let data: Vec<u8> = (0..height)
            .flat_map(|_| (0..width).map(|x| x as u8))
            .collect();

        // Sample at exactly integer pixel positions → should reproduce source
        let sx: Vec<f64> = (0..width).map(|x| x as f64).collect();
        let sy = vec![2.0f64; width]; // row 2, no fractional part
        let mut output = vec![0u8; width];
        bilinear_row_simd(&data, width, height, &sx, &sy, width, &mut output);

        for (i, &v) in output.iter().enumerate() {
            assert_eq!(
                v, i as u8,
                "integer coordinates should reproduce source pixel"
            );
        }
    }

    #[test]
    fn test_bilinear_row_simd_zero_count() {
        let data = vec![99u8; 4];
        let mut output = vec![0u8; 0];
        // Should not panic with count=0
        bilinear_row_simd(&data, 2, 2, &[], &[], 0, &mut output);
    }

    #[test]
    fn test_warp_bilinear_simd_identity() {
        let src = vec![128u8; 16 * 16];
        let dst = warp_bilinear_simd(&src, 16, 16, 0.0, 0.0, 0.0, 1.0);
        assert_eq!(dst.len(), src.len());
        for &v in &dst {
            assert_eq!(
                v, 128,
                "identity warp on uniform image should preserve value"
            );
        }
    }

    #[test]
    fn test_warp_bilinear_simd_empty() {
        let result = warp_bilinear_simd(&[], 0, 0, 0.0, 0.0, 0.0, 1.0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_bilinear_quality_score_identical() {
        let data = vec![100u8; 8 * 8];
        let score = bilinear_quality_score(&data, 8, 8, &data, 8, 8);
        assert!((score - 0.0).abs() < 1e-9, "identical images → score 0");
    }

    #[test]
    fn test_bilinear_quality_score_different() {
        let src = vec![0u8; 8 * 8];
        let dst = vec![128u8; 8 * 8];
        let score = bilinear_quality_score(&src, 8, 8, &dst, 8, 8);
        assert!(score > 0.0, "different images → positive score");
    }
}
