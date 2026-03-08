// Copyright 2025 OxiMedia Contributors
// Licensed under the Apache License, Version 2.0

//! Color format conversion utilities.
//!
//! Provides BT.601 YUV/RGB conversions, planar YUV420 packing/unpacking,
//! and linear/sRGB gamma encoding/decoding.

/// Convert a single YUV pixel (BT.601) to RGB.
///
/// Returns `[r, g, b]` with each component clamped to `0..=255`.
#[must_use]
#[allow(clippy::many_single_char_names)]
pub fn yuv_to_rgb(y: u8, u: u8, v: u8) -> [u8; 3] {
    let y = i32::from(y);
    let u = i32::from(u) - 128;
    let v = i32::from(v) - 128;

    let r = y + (1_402 * v) / 1_000;
    let g = y - (344_136 * u + 714_136 * v) / 1_000_000;
    let b = y + (1_772 * u) / 1_000;

    [clamp_u8(r), clamp_u8(g), clamp_u8(b)]
}

/// Convert a single RGB pixel to YUV (BT.601).
///
/// Returns `[y, u, v]`.
#[must_use]
#[allow(clippy::many_single_char_names)]
pub fn rgb_to_yuv(r: u8, g: u8, b: u8) -> [u8; 3] {
    let r = i32::from(r);
    let g = i32::from(g);
    let b = i32::from(b);

    let y = (299 * r + 587 * g + 114 * b) / 1_000;
    let u = (-168_736 * r - 331_264 * g + 500_000 * b) / 1_000_000 + 128;
    let v = (500_000 * r - 418_688 * g - 81_312 * b) / 1_000_000 + 128;

    [clamp_u8(y), clamp_u8(u), clamp_u8(v)]
}

/// Convert a planar YUV420 buffer to packed RGB.
///
/// The input layout is: Y plane (width * height bytes), then U plane
/// (width/2 * height/2 bytes), then V plane (width/2 * height/2 bytes).
///
/// Returns packed RGB with 3 bytes per pixel (row-major).
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn yuv420_to_rgb(yuv: &[u8], width: usize, height: usize) -> Vec<u8> {
    let y_size = width * height;
    let uv_size = (width / 2) * (height / 2);

    if yuv.len() < y_size + 2 * uv_size {
        return Vec::new();
    }

    let y_plane = &yuv[..y_size];
    let u_plane = &yuv[y_size..y_size + uv_size];
    let v_plane = &yuv[y_size + uv_size..y_size + 2 * uv_size];

    let mut rgb = Vec::with_capacity(width * height * 3);

    #[allow(clippy::many_single_char_names)]
    for row in 0..height {
        for col in 0..width {
            let y = y_plane[row * width + col];
            let uv_row = row / 2;
            let uv_col = col / 2;
            let uv_width = width / 2;
            let u = u_plane[uv_row * uv_width + uv_col];
            let v = v_plane[uv_row * uv_width + uv_col];
            let [r, g, b] = yuv_to_rgb(y, u, v);
            rgb.push(r);
            rgb.push(g);
            rgb.push(b);
        }
    }

    rgb
}

/// Convert packed RGB to planar YUV420.
///
/// The input is packed RGB with 3 bytes per pixel (row-major).
///
/// Returns: Y plane (width * height bytes) + U plane (w/2 * h/2) + V plane (w/2 * h/2).
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn rgb_to_yuv420(rgb: &[u8], width: usize, height: usize) -> Vec<u8> {
    if rgb.len() < width * height * 3 {
        return Vec::new();
    }

    let uv_width = width / 2;
    let uv_height = height / 2;
    let y_size = width * height;
    let uv_size = uv_width * uv_height;

    let mut y_plane = vec![0u8; y_size];
    let mut u_plane = vec![0u8; uv_size];
    let mut v_plane = vec![0u8; uv_size];

    // Fill Y plane
    for row in 0..height {
        for col in 0..width {
            let idx = (row * width + col) * 3;
            let [y, _, _] = rgb_to_yuv(rgb[idx], rgb[idx + 1], rgb[idx + 2]);
            y_plane[row * width + col] = y;
        }
    }

    // Fill U/V planes (average over 2x2 blocks)
    for uv_row in 0..uv_height {
        for uv_col in 0..uv_width {
            let mut u_sum: i32 = 0;
            let mut v_sum: i32 = 0;
            let mut count = 0i32;
            for dr in 0..2usize {
                for dc in 0..2usize {
                    let row = uv_row * 2 + dr;
                    let col = uv_col * 2 + dc;
                    if row < height && col < width {
                        let idx = (row * width + col) * 3;
                        let [_, u, v] = rgb_to_yuv(rgb[idx], rgb[idx + 1], rgb[idx + 2]);
                        u_sum += i32::from(u);
                        v_sum += i32::from(v);
                        count += 1;
                    }
                }
            }
            if count > 0 {
                u_plane[uv_row * uv_width + uv_col] = clamp_u8(u_sum / count);
                v_plane[uv_row * uv_width + uv_col] = clamp_u8(v_sum / count);
            }
        }
    }

    let mut out = Vec::with_capacity(y_size + 2 * uv_size);
    out.extend_from_slice(&y_plane);
    out.extend_from_slice(&u_plane);
    out.extend_from_slice(&v_plane);
    out
}

/// Encode a linear light value (0.0..=1.0) to sRGB gamma.
///
/// Values outside `0.0..=1.0` are clamped before encoding.
#[must_use]
pub fn linear_to_srgb(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Decode an sRGB gamma-encoded value (0.0..=1.0) to linear light.
///
/// Values outside `0.0..=1.0` are clamped before decoding.
#[must_use]
pub fn srgb_to_linear(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
fn clamp_u8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- yuv_to_rgb / rgb_to_yuv ---

    #[test]
    fn test_yuv_to_rgb_white() {
        // Y=235, U=128, V=128 → near white
        let [r, g, b] = yuv_to_rgb(235, 128, 128);
        assert!(r > 200, "r={r}");
        assert!(g > 200, "g={g}");
        assert!(b > 200, "b={b}");
    }

    #[test]
    fn test_yuv_to_rgb_black() {
        let [r, g, b] = yuv_to_rgb(16, 128, 128);
        assert!(r < 20, "r={r}");
        assert!(g < 20, "g={g}");
        assert!(b < 20, "b={b}");
    }

    #[test]
    fn test_rgb_to_yuv_white() {
        let [y, u, v] = rgb_to_yuv(255, 255, 255);
        assert!(y > 200, "y={y}");
        // U and V should be near 128 for achromatic
        assert!((i32::from(u) - 128).abs() < 15, "u={u}");
        assert!((i32::from(v) - 128).abs() < 15, "v={v}");
    }

    #[test]
    fn test_rgb_to_yuv_black() {
        let [y, u, v] = rgb_to_yuv(0, 0, 0);
        assert!(y < 10, "y={y}");
        assert!((i32::from(u) - 128).abs() < 5, "u={u}");
        assert!((i32::from(v) - 128).abs() < 5, "v={v}");
    }

    #[test]
    fn test_yuv_rgb_roundtrip_gray() {
        // Gray: round-trip should stay near-gray
        let original = [128u8, 128, 128];
        let [y, u, v] = rgb_to_yuv(original[0], original[1], original[2]);
        let [r2, g2, b2] = yuv_to_rgb(y, u, v);
        // Allow small rounding error
        assert!((i32::from(r2) - i32::from(original[0])).abs() <= 5);
        assert!((i32::from(g2) - i32::from(original[1])).abs() <= 5);
        assert!((i32::from(b2) - i32::from(original[2])).abs() <= 5);
    }

    #[test]
    fn test_yuv_to_rgb_no_overflow() {
        // Extreme values should not panic
        // Note: r, g, b are u8 types, so clamping is guaranteed by clamp_u8 function.
        // No need for explicit bounds checks.
        let [r, g, b] = yuv_to_rgb(0, 0, 0);
        let _ = (r, g, b);
        let [r, g, b] = yuv_to_rgb(255, 255, 255);
        let _ = (r, g, b);
    }

    #[test]
    fn test_rgb_to_yuv_pure_red() {
        let [y, _u, v] = rgb_to_yuv(255, 0, 0);
        // Red should produce high V and significant Y
        assert!(y > 50, "y={y}");
        assert!(v > 128, "v={v}");
    }

    // --- yuv420_to_rgb / rgb_to_yuv420 ---

    #[test]
    fn test_yuv420_to_rgb_size() {
        let width = 4;
        let height = 4;
        let y_size = width * height;
        let uv_size = (width / 2) * (height / 2);
        let yuv = vec![128u8; y_size + 2 * uv_size];
        let rgb = yuv420_to_rgb(&yuv, width, height);
        assert_eq!(rgb.len(), width * height * 3);
    }

    #[test]
    fn test_yuv420_to_rgb_empty_on_short_input() {
        let rgb = yuv420_to_rgb(&[0u8; 4], 4, 4);
        assert!(rgb.is_empty());
    }

    #[test]
    fn test_rgb_to_yuv420_size() {
        let width = 4;
        let height = 4;
        let rgb = vec![128u8; width * height * 3];
        let yuv = rgb_to_yuv420(&rgb, width, height);
        let expected = width * height + 2 * (width / 2) * (height / 2);
        assert_eq!(yuv.len(), expected);
    }

    #[test]
    fn test_rgb_to_yuv420_empty_on_short_input() {
        let yuv = rgb_to_yuv420(&[0u8; 10], 4, 4);
        assert!(yuv.is_empty());
    }

    // --- linear_to_srgb / srgb_to_linear ---

    #[test]
    fn test_linear_to_srgb_zero() {
        assert!((linear_to_srgb(0.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_linear_to_srgb_one() {
        assert!((linear_to_srgb(1.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_srgb_to_linear_zero() {
        assert!((srgb_to_linear(0.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_srgb_to_linear_one() {
        assert!((srgb_to_linear(1.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_gamma_roundtrip() {
        let vals = [0.0f32, 0.01, 0.1, 0.5, 0.9, 1.0];
        for v in vals {
            let encoded = linear_to_srgb(v);
            let decoded = srgb_to_linear(encoded);
            assert!(
                (decoded - v).abs() < 1e-4,
                "roundtrip failed for v={v}: decoded={decoded}"
            );
        }
    }

    #[test]
    fn test_linear_to_srgb_clamps_negative() {
        assert!((linear_to_srgb(-1.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_linear_to_srgb_clamps_above_one() {
        assert!((linear_to_srgb(2.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_srgb_midpoint_gamma() {
        // sRGB mid-gray (0.5) → linear should be lower
        let linear = srgb_to_linear(0.5);
        assert!(linear < 0.5, "linear={linear}");
        assert!(linear > 0.1, "linear={linear}");
    }
}
