//! Bicubic interpolation for high-quality image resizing.
//!
//! Uses the Mitchell-Netravali bicubic kernel (B=0, C=0.5) to resample
//! RGB images. Pixel values are clamped to `[0, 255]` at the output.

#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(dead_code)]

// ── Kernel ────────────────────────────────────────────────────────────────────

/// Evaluate the Mitchell-Netravali cubic kernel at position `t`.
///
/// Uses B=0, C=0.5 which is a common trade-off between ringing and blurring.
/// Returns weights in the range `[-0.5, 1.0]`.
#[must_use]
pub fn cubic_weight(t: f64) -> f64 {
    let at = t.abs();
    if at < 1.0 {
        // (12 - 9B - 6C)/6 * |t|^3 + (-18 + 12B + 6C)/6 * |t|^2 + (6-2B)/6
        // with B=0, C=0.5 → (12-3)/6, (-18+3)/6, 6/6
        (9.0 * at * at * at - 15.0 * at * at + 6.0) / 6.0
    } else if at < 2.0 {
        // (-B - 6C)/6 * |t|^3 + (6B + 30C)/6 * |t|^2 + (-12B - 48C)/6 * |t| + (8B + 24C)/6
        // with B=0, C=0.5 → (-3)/6, (15)/6, (-24)/6, (12)/6
        (-3.0 * at * at * at + 15.0 * at * at - 24.0 * at + 12.0) / 6.0
    } else {
        0.0
    }
}

// ── Single-pixel sampling ────────────────────────────────────────────────────

/// Sample a single RGB pixel from an image using bicubic interpolation.
///
/// `src` must hold packed 3-byte RGB pixels in row-major order with dimensions
/// `width × height`. Coordinates `(x, y)` may be fractional; edge pixels are
/// replicated for out-of-bounds indices.
///
/// Returns the interpolated `[R, G, B]` pixel.
#[must_use]
pub fn bicubic_sample(src: &[u8], x: f64, y: f64, width: usize, height: usize) -> [u8; 3] {
    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let fx = x - x0 as f64;
    let fy = y - y0 as f64;

    let w = width as i64;
    let h = height as i64;

    let clamp_x = |v: i64| v.clamp(0, w - 1) as usize;
    let clamp_y = |v: i64| v.clamp(0, h - 1) as usize;

    let mut channels = [0.0f64; 3];

    for ky in -1i64..=2 {
        let wy = cubic_weight(fy - ky as f64);
        if wy.abs() < 1e-10 {
            continue;
        }
        let sy = clamp_y(y0 + ky);
        for kx in -1i64..=2 {
            let wx = cubic_weight(fx - kx as f64);
            let weight = wx * wy;
            if weight.abs() < 1e-10 {
                continue;
            }
            let sx = clamp_x(x0 + kx);
            let base = (sy * width + sx) * 3;
            for c in 0..3 {
                channels[c] += src[base + c] as f64 * weight;
            }
        }
    }

    [
        channels[0].round().clamp(0.0, 255.0) as u8,
        channels[1].round().clamp(0.0, 255.0) as u8,
        channels[2].round().clamp(0.0, 255.0) as u8,
    ]
}

// ── Full image resize ─────────────────────────────────────────────────────────

/// Resize an RGB image using bicubic interpolation.
///
/// `src` is a packed 3-bytes-per-pixel row-major RGB buffer of size
/// `src_w × src_h`. Returns a new buffer of size `dst_w × dst_h × 3`.
///
/// Returns an empty vector if any dimension is zero.
#[must_use]
pub fn bicubic_resize(
    src: &[u8],
    src_w: usize,
    src_h: usize,
    dst_w: usize,
    dst_h: usize,
) -> Vec<u8> {
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return Vec::new();
    }

    let scale_x = src_w as f64 / dst_w as f64;
    let scale_y = src_h as f64 / dst_h as f64;

    let mut dst = vec![0u8; dst_w * dst_h * 3];
    for dy in 0..dst_h {
        let sy = (dy as f64 + 0.5) * scale_y - 0.5;
        for dx in 0..dst_w {
            let sx = (dx as f64 + 0.5) * scale_x - 0.5;
            let pixel = bicubic_sample(src, sx, sy, src_w, src_h);
            let base = (dy * dst_w + dx) * 3;
            dst[base] = pixel[0];
            dst[base + 1] = pixel[1];
            dst[base + 2] = pixel[2];
        }
    }
    dst
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cubic_weight_at_zero() {
        // Center of the kernel should be 1.0.
        assert!((cubic_weight(0.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_cubic_weight_at_two() {
        // Beyond radius 2 the kernel is zero.
        assert!((cubic_weight(2.0)).abs() < 1e-10);
        assert!((cubic_weight(-2.0)).abs() < 1e-10);
        assert!((cubic_weight(3.0)).abs() < 1e-10);
    }

    #[test]
    fn test_cubic_weight_symmetric() {
        for i in 1..20 {
            let t = i as f64 * 0.1;
            let diff = (cubic_weight(t) - cubic_weight(-t)).abs();
            assert!(diff < 1e-10, "asymmetric at t={t}: diff={diff}");
        }
    }

    #[test]
    fn test_cubic_weight_continuous_at_one() {
        // The kernel should be continuous at |t| = 1.
        let left = cubic_weight(1.0 - 1e-9);
        let right = cubic_weight(1.0 + 1e-9);
        assert!(
            (left - right).abs() < 1e-4,
            "discontinuity at 1: left={left} right={right}"
        );
    }

    #[test]
    fn test_bicubic_sample_exact_pixel() {
        // Sampling at the exact integer coordinate of a pixel should return that pixel.
        let src = vec![
            255u8, 0, 0, // (0,0) red
            0, 255, 0, // (1,0) green
            0, 0, 255, // (0,1) blue
            128, 128, 0,
        ]; // (1,1) yellow
        let p = bicubic_sample(&src, 0.0, 0.0, 2, 2);
        // With small contributions from neighbours, red channel should dominate.
        assert!(
            p[0] > p[1] && p[0] > p[2],
            "expected reddish pixel, got {:?}",
            p
        );
    }

    #[test]
    fn test_bicubic_sample_clamped_oob() {
        // Out-of-bounds coordinates should not panic (edge clamped).
        let src = vec![200u8, 100, 50, 80, 40, 20, 160, 80, 40, 64, 32, 16];
        let _ = bicubic_sample(&src, -5.0, -5.0, 2, 2);
        let _ = bicubic_sample(&src, 100.0, 100.0, 2, 2);
    }

    #[test]
    fn test_bicubic_resize_returns_correct_size() {
        let src = vec![128u8; 4 * 4 * 3]; // 4×4 grey image
        let dst = bicubic_resize(&src, 4, 4, 8, 8);
        assert_eq!(dst.len(), 8 * 8 * 3);
    }

    #[test]
    fn test_bicubic_resize_uniform_image() {
        // A uniform image should remain uniform after resizing.
        let src = vec![100u8; 4 * 4 * 3];
        let dst = bicubic_resize(&src, 4, 4, 8, 8);
        for (i, &v) in dst.iter().enumerate() {
            assert!((v as i32 - 100).abs() <= 2, "pixel {i}: got {v}");
        }
    }

    #[test]
    fn test_bicubic_resize_same_size() {
        let src: Vec<u8> = (0..27).map(|i| (i * 9) as u8).collect();
        let dst = bicubic_resize(&src, 3, 3, 3, 3);
        assert_eq!(dst.len(), 27);
    }

    #[test]
    fn test_bicubic_resize_zero_dimension_returns_empty() {
        let src = vec![0u8; 16 * 3];
        assert!(bicubic_resize(&src, 0, 4, 8, 8).is_empty());
        assert!(bicubic_resize(&src, 4, 4, 0, 8).is_empty());
    }

    #[test]
    fn test_bicubic_resize_downscale_2x() {
        let src = vec![200u8; 8 * 8 * 3];
        let dst = bicubic_resize(&src, 8, 8, 4, 4);
        assert_eq!(dst.len(), 4 * 4 * 3);
        for &v in &dst {
            assert!((v as i32 - 200).abs() <= 3, "got {v}");
        }
    }

    #[test]
    fn test_bicubic_resize_single_pixel() {
        let src = vec![42u8, 84, 126];
        let dst = bicubic_resize(&src, 1, 1, 3, 3);
        assert_eq!(dst.len(), 27);
        // Every output pixel should equal the only input pixel.
        for chunk in dst.chunks(3) {
            assert_eq!(chunk[0], 42);
            assert_eq!(chunk[1], 84);
            assert_eq!(chunk[2], 126);
        }
    }

    #[test]
    fn test_bicubic_resize_channels_independent() {
        // Red channel = 255, others = 0.
        let src: Vec<u8> = (0..16).flat_map(|_| [255u8, 0, 0]).collect();
        let dst = bicubic_resize(&src, 4, 4, 4, 4);
        for chunk in dst.chunks(3) {
            assert!(chunk[0] > 200, "R should be high, got {}", chunk[0]);
        }
    }
}
