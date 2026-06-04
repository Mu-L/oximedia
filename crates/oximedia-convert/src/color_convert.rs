// Copyright 2025 OxiMedia Contributors
// Licensed under the Apache License, Version 2.0

//! Color format conversion utilities.
//!
//! Provides BT.601 YUV/RGB conversions, planar YUV420 packing/unpacking,
//! linear/sRGB gamma encoding/decoding, and HDR-to-SDR tone-mapping conversion.

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

// ── HDR-to-SDR tone mapping conversion ───────────────────────────────────────

use oximedia_colormgmt::hdr::tonemapping::ToneCurve;

/// Source HDR transfer function (EOTF).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceTransferFunction {
    /// SMPTE ST 2084 / ITU-R BT.2100 PQ — peak typically 1 000–10 000 nits.
    Pq,
    /// ITU-R BT.2100 HLG (Hybrid Log-Gamma).
    Hlg,
}

/// Configuration for HDR-to-SDR tone-mapping conversion.
#[derive(Debug, Clone)]
pub struct HdrToSdrConfig {
    /// Transfer function used to encode the source HDR signal.
    pub source_tf: SourceTransferFunction,
    /// Reference peak luminance in nits (default 1 000 nits).
    ///
    /// For PQ this is the absolute peak level encoded at signal code 1.0.
    /// For HLG this is the display peak used in the HLG OOTF.
    pub peak_nits: f32,
    /// Tone-mapping operator applied after EOTF linearisation.
    pub tone_curve: ToneCurve,
    /// Apply a Rec.2020 → Rec.709 gamut-clamp after tone mapping.
    pub gamut_map: bool,
}

impl Default for HdrToSdrConfig {
    fn default() -> Self {
        Self {
            source_tf: SourceTransferFunction::Pq,
            peak_nits: 1000.0,
            tone_curve: ToneCurve::FilmicHable,
            gamut_map: true,
        }
    }
}

// ── PQ EOTF (SMPTE ST 2084) ───────────────────────────────────────────────────

/// Decode a single PQ code-value (10-bit, in [0, 1]) to scene-linear nits.
///
/// Per SMPTE ST 2084-2014 Eq. 5.  The output is normalised so that the PQ
/// reference peak (code ≈ 1.0) decodes to `1.0`; absolute nits are obtained
/// by multiplying by `10 000`.
#[inline]
fn pq_eotf_normalised(code: f32) -> f32 {
    const M1: f32 = 0.159_301_758; // 1 / m2_inv = 2610 / 16384 / 4
    const M2: f32 = 78.843_75; // 2523 / 32
    const C1: f32 = 0.835_937_5; // 3424 / 4096
    const C2: f32 = 18.851_562_5; // 2413 / 128
    const C3: f32 = 18.687_5; // 2392 / 128

    let code = code.clamp(0.0, 1.0);
    let v_m2 = code.powf(1.0 / M2);
    let num = (v_m2 - C1).max(0.0);
    let den = C2 - C3 * v_m2;
    if den.abs() < f32::EPSILON {
        return 0.0;
    }
    (num / den).powf(1.0 / M1)
}

// ── HLG OETF inverse (BT.2100) ───────────────────────────────────────────────

/// Decode a single HLG signal value (in [0, 1]) to scene-linear normalised light.
///
/// Per ITU-R BT.2100-2 Table 5.
#[inline]
fn hlg_oetf_inverse(code: f32) -> f32 {
    const A: f32 = 0.178_832_77;
    const B: f32 = 0.284_668_92;
    const C: f32 = 0.559_910_73;

    let code = code.clamp(0.0, 1.0);
    if code <= 0.5 {
        code * code / 3.0
    } else {
        (((code - C) / A).exp() + B) / 12.0
    }
}

/// Reconstruct scene-linear light from a 10-bit HDR code using the given EOTF.
///
/// The returned value is in `[0, 1]` where 1.0 represents `peak_nits` after
/// normalization.
#[inline]
fn decode_hdr_code(code_u16: u16, tf: SourceTransferFunction, peak_nits: f32) -> f32 {
    let code_f = code_u16 as f32 / 1023.0;
    let linear_norm = match tf {
        SourceTransferFunction::Pq => {
            // PQ 10 000-nit peak → normalise to peak_nits
            pq_eotf_normalised(code_f) * (10_000.0 / peak_nits.max(1.0))
        }
        SourceTransferFunction::Hlg => hlg_oetf_inverse(code_f),
    };
    linear_norm.clamp(0.0, 1.0)
}

/// Apply a simple BT.2020 → BT.709 gamut-clip (3×3 matrix + clamp).
///
/// Matrix from ITU-R BT.2087-0.
#[inline]
fn rec2020_to_rec709_clip(r: f32, g: f32, b: f32) -> [f32; 3] {
    // BT.2020 → BT.709 via XYZ intermediate (D65 cat)
    let r709 = 1.660_491 * r - 0.587_641 * g - 0.072_850 * b;
    let g709 = -0.124_550 * r + 1.132_900 * g - 0.008_350 * b;
    let b709 = -0.018_151 * r - 0.100_579 * g + 1.118_730 * b;
    [
        r709.clamp(0.0, 1.0),
        g709.clamp(0.0, 1.0),
        b709.clamp(0.0, 1.0),
    ]
}

/// Convert a single HDR pixel (normalised scene-linear Y, Cb, Cr in [-0.5, 0.5])
/// to SDR `(R, G, B)` as `u8` values.
///
/// The input Y is the normalised scene-linear luma (0..1 maps to 0..`peak_nits`).
/// Cb/Cr are differential chroma signals in `[-0.5, 0.5]`.
///
/// # Processing pipeline
///
/// 1. PQ or HLG EOTF decoding (already done by the caller; `y_linear` is the result).
/// 2. Chroma-to-RGB reconstruct (simplified BT.2020 YCbCr).
/// 3. Tone mapping via `cfg.tone_curve`.
/// 4. Optional Rec.2020 → Rec.709 gamut clip.
/// 5. Gamma 2.2 encode + scale to [0, 255].
#[must_use]
pub fn convert_hdr_to_sdr_pixel(
    y_linear: f32,
    cb: f32,
    cr: f32,
    cfg: &HdrToSdrConfig,
) -> (u8, u8, u8) {
    // Reconstruct linear R, G, B from Y, Cb, Cr (BT.2020 matrix)
    // BT.2020 YCbCr coefficients Kr=0.2627, Kb=0.0593
    let r_linear = y_linear + 1.474_600 * cr;
    let g_linear = y_linear - 0.164_553 * cb - 0.571_353 * cr;
    let b_linear = y_linear + 1.881_400 * cb;

    // Tone map
    let rgb_in = [r_linear.max(0.0), g_linear.max(0.0), b_linear.max(0.0)];
    let rgb_tm = cfg.tone_curve.map(rgb_in);

    // Optional gamut mapping: Rec.2020 → Rec.709
    let [r_sdr, g_sdr, b_sdr] = if cfg.gamut_map {
        rec2020_to_rec709_clip(rgb_tm[0], rgb_tm[1], rgb_tm[2])
    } else {
        [
            rgb_tm[0].clamp(0.0, 1.0),
            rgb_tm[1].clamp(0.0, 1.0),
            rgb_tm[2].clamp(0.0, 1.0),
        ]
    };

    // Gamma 2.2 encode
    let encode = |v: f32| -> u8 { (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0 + 0.5) as u8 };

    (encode(r_sdr), encode(g_sdr), encode(b_sdr))
}

/// Convert a full HDR frame (10-bit luma-coded PQ or HLG, YCbCr 4:2:0) to a
/// packed SDR RGB `u8` frame.
///
/// The output is packed as `[R, G, B, R, G, B, …]` in row-major order with the
/// same dimensions as the input.
///
/// # Arguments
///
/// * `frame_y`  — luma plane (`w * h` 10-bit samples as `u16`).
/// * `frame_cb` — Cb chroma plane (`(w/2) * (h/2)` 10-bit samples).
/// * `frame_cr` — Cr chroma plane (`(w/2) * (h/2)` 10-bit samples).
/// * `w`, `h`   — frame dimensions.
/// * `cfg`      — HDR-to-SDR configuration.
///
/// # Returns
///
/// Packed RGB `u8` buffer of length `w * h * 3`, or an empty `Vec` if the
/// inputs are too short.
#[must_use]
pub fn convert_hdr_to_sdr_frame(
    frame_y: &[u16],
    frame_cb: &[u16],
    frame_cr: &[u16],
    w: u32,
    h: u32,
    cfg: &HdrToSdrConfig,
) -> Vec<u8> {
    let width = w as usize;
    let height = h as usize;
    let y_len = width * height;
    let uv_len = (width / 2) * (height / 2);

    if frame_y.len() < y_len || frame_cb.len() < uv_len || frame_cr.len() < uv_len {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(y_len * 3);

    for row in 0..height {
        for col in 0..width {
            let y_code = frame_y[row * width + col];
            let uv_row = row / 2;
            let uv_col = col / 2;
            let uv_width = width / 2;
            let cb_code = frame_cb[uv_row * uv_width + uv_col];
            let cr_code = frame_cr[uv_row * uv_width + uv_col];

            // Decode HDR codes to normalised linear scene light
            let y_linear = decode_hdr_code(y_code, cfg.source_tf, cfg.peak_nits);
            // Chroma: 10-bit code centre 512 → offset to [-0.5, 0.5]
            let cb_linear = cb_code as f32 / 1023.0 - 0.5;
            let cr_linear = cr_code as f32 / 1023.0 - 0.5;

            let (r, g, b) = convert_hdr_to_sdr_pixel(y_linear, cb_linear, cr_linear, cfg);
            out.push(r);
            out.push(g);
            out.push(b);
        }
    }

    out
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

    // ── HDR-to-SDR tests ─────────────────────────────────────────────────────

    #[test]
    fn test_hdr_to_sdr_black_stays_black() {
        // Zero PQ code → all zero output
        let cfg = HdrToSdrConfig {
            source_tf: SourceTransferFunction::Pq,
            peak_nits: 1000.0,
            tone_curve: ToneCurve::FilmicHable,
            gamut_map: false,
        };
        let (r, g, b) = convert_hdr_to_sdr_pixel(0.0, 0.0, 0.0, &cfg);
        assert_eq!(r, 0, "r={r}");
        assert_eq!(g, 0, "g={g}");
        assert_eq!(b, 0, "b={b}");
    }

    #[test]
    fn test_hdr_to_sdr_pq_white_near_white() {
        // Peak white PQ luma ≈ 1.0 normalised → SDR should be substantially above zero.
        // With Hable tone curve, 1.0 scene-linear → ~148/255 (≈58% of SDR range).
        let cfg = HdrToSdrConfig {
            source_tf: SourceTransferFunction::Pq,
            peak_nits: 1000.0,
            tone_curve: ToneCurve::FilmicHable,
            gamut_map: false,
        };
        // y_linear=1.0 means peak-white signal
        let (r, _g, _b) = convert_hdr_to_sdr_pixel(1.0, 0.0, 0.0, &cfg);
        assert!(
            r > 100,
            "peak-white luma should produce significant SDR output, got r={r}"
        );
    }

    #[test]
    fn test_hdr_to_sdr_hlg_black_zero() {
        let cfg = HdrToSdrConfig {
            source_tf: SourceTransferFunction::Hlg,
            peak_nits: 1000.0,
            tone_curve: ToneCurve::ReinhardSimple,
            gamut_map: false,
        };
        let (r, g, b) = convert_hdr_to_sdr_pixel(0.0, 0.0, 0.0, &cfg);
        assert_eq!((r, g, b), (0, 0, 0), "HLG black should map to zero");
    }

    #[test]
    fn test_hdr_to_sdr_frame_size() {
        // 4×4 frame → output must be 4*4*3 = 48 bytes
        let w = 4u32;
        let h = 4u32;
        let y = vec![512u16; 16];
        let cb = vec![512u16; 4];
        let cr = vec![512u16; 4];
        let cfg = HdrToSdrConfig::default();
        let out = convert_hdr_to_sdr_frame(&y, &cb, &cr, w, h, &cfg);
        assert_eq!(out.len(), 48, "output length should be w*h*3");
    }

    #[test]
    fn test_hdr_to_sdr_frame_empty_on_short_input() {
        let cfg = HdrToSdrConfig::default();
        let out = convert_hdr_to_sdr_frame(&[], &[], &[], 4, 4, &cfg);
        assert!(out.is_empty(), "short input should produce empty output");
    }

    #[test]
    fn test_pq_eotf_black() {
        // PQ code 0 → linear 0
        assert!(pq_eotf_normalised(0.0).abs() < 1e-6);
    }

    #[test]
    fn test_hlg_oetf_inverse_black() {
        // HLG code 0 → linear 0
        assert!(hlg_oetf_inverse(0.0).abs() < 1e-6);
    }

    #[test]
    fn test_hlg_oetf_inverse_midpoint() {
        // HLG 0.5 → linear 0.25/3 ≈ 0.0833
        let v = hlg_oetf_inverse(0.5);
        assert!((v - 0.0833).abs() < 1e-3, "hlg(0.5) ≈ 0.0833, got {v}");
    }
}
