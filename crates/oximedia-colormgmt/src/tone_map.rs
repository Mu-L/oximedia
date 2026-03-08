//! HDR to SDR tone mapping operators.
//!
//! Provides industry-standard tone mapping functions for converting high dynamic
//! range (HDR) content to standard dynamic range (SDR) display output.

#![allow(dead_code)]

use std::f64::consts::PI;

/// Tone mapping operator selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToneMapOperator {
    /// Reinhard simple operator (1 / (1 + x)).
    Reinhard,
    /// Extended Reinhard with configurable white point.
    ReinhardExtended,
    /// Hable / Uncharted 2 filmic tone mapper.
    HableFilimic,
    /// ACES filmic tone mapping approximation.
    AcesFilmic,
    /// Drago logarithmic operator.
    DragoLogarithmic,
}

/// Parameters controlling tone mapping behaviour.
#[derive(Debug, Clone)]
pub struct ToneMapParams {
    /// Tone mapping operator.
    pub operator: ToneMapOperator,
    /// HDR peak luminance in nits (e.g. 1000.0 for HDR10).
    pub peak_luminance: f64,
    /// SDR target luminance in nits (e.g. 100.0).
    pub target_luminance: f64,
    /// Output gamma (e.g. 2.2 for standard display).
    pub gamma: f64,
    /// Pre-exposure adjustment (multiplied before tone mapping). 1.0 = neutral.
    pub exposure: f64,
}

impl Default for ToneMapParams {
    fn default() -> Self {
        Self {
            operator: ToneMapOperator::AcesFilmic,
            peak_luminance: 1000.0,
            target_luminance: 100.0,
            gamma: 2.2,
            exposure: 1.0,
        }
    }
}

// ── Operator implementations ─────────────────────────────────────────────────

/// Reinhard tone mapping operator: `x / (1 + x)`.
///
/// Maps `[0, ∞)` to `[0, 1)`. Simple and fast, but can look flat in shadows.
#[must_use]
#[inline]
pub fn reinhard(x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    x / (1.0 + x)
}

/// Extended Reinhard operator with a white point `peak`.
///
/// `x * (1 + x / peak²) / (1 + x)`
#[must_use]
#[inline]
pub fn reinhard_extended(x: f64, peak: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let peak_sq = peak * peak;
    (x * (1.0 + x / peak_sq)) / (1.0 + x)
}

/// Hable / Uncharted 2 filmic tone mapping operator.
///
/// Developed by John Hable. Provides a warm, cinematic look with controlled
/// shoulder and toe regions.
#[must_use]
pub fn hable_filmic(x: f64) -> f64 {
    const A: f64 = 0.15;
    const B: f64 = 0.50;
    const C: f64 = 0.10;
    const D: f64 = 0.20;
    const E: f64 = 0.02;
    const F: f64 = 0.30;

    fn hable_partial(v: f64) -> f64 {
        (v * (A * v + C * B) + D * E) / (v * (A * v + B) + D * F) - E / F
    }

    const W: f64 = 11.2; // linear white point
    if x <= 0.0 {
        return 0.0;
    }
    let curr = hable_partial(x * 2.0);
    let white_scale = 1.0 / hable_partial(W);
    (curr * white_scale).clamp(0.0, 1.0)
}

/// ACES filmic tone mapping approximation by Krzysztof Narkowicz.
///
/// Provides an ACES-like response with a fast analytical formula.
#[must_use]
pub fn aces_filmic(x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    const A: f64 = 2.51;
    const B: f64 = 0.03;
    const C: f64 = 2.43;
    const D: f64 = 0.59;
    const E: f64 = 0.14;
    ((x * (A * x + B)) / (x * (C * x + D) + E)).clamp(0.0, 1.0)
}

/// Drago logarithmic tone mapping operator.
///
/// Based on Drago et al. "Adaptive logarithmic mapping for displaying high
/// contrast scenes". Uses a logarithm-based roll-off suitable for outdoor scenes.
#[must_use]
pub fn drago_logarithmic(x: f64, peak_luminance: f64, bias: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let bias = bias.clamp(0.1, 0.9);
    let log_bias = (bias / 0.5_f64).log10();
    let log_peak = (peak_luminance + 1.0).log10();
    let log_x = (x + 1.0).log10();
    let mapped = log_x / (log_peak * (log_bias / (PI / 2.0).log10()).exp());
    mapped.clamp(0.0, 1.0)
}

// ── Frame-level apply functions ───────────────────────────────────────────────

/// Apply tone mapping to a single RGB pixel.
///
/// The input is assumed to be in linear light, scene-referred.
/// Output is display-referred in [0.0, 1.0].
#[must_use]
pub fn apply_tone_map(rgb: (f64, f64, f64), params: &ToneMapParams) -> (f64, f64, f64) {
    let (r, g, b) = rgb;

    // Scale by exposure and normalize to [0, 1] using peak luminance
    let scale = params.exposure / params.peak_luminance * params.target_luminance;
    let r = r * scale;
    let g = g * scale;
    let b = b * scale;

    let map = |v: f64| -> f64 {
        let mapped = match params.operator {
            ToneMapOperator::Reinhard => reinhard(v),
            ToneMapOperator::ReinhardExtended => reinhard_extended(v, 1.0),
            ToneMapOperator::HableFilimic => hable_filmic(v),
            ToneMapOperator::AcesFilmic => aces_filmic(v),
            ToneMapOperator::DragoLogarithmic => {
                drago_logarithmic(v, params.peak_luminance / params.target_luminance, 0.85)
            }
        };
        // Apply inverse gamma for display
        if mapped <= 0.0 {
            return 0.0;
        }
        mapped.powf(1.0 / params.gamma).clamp(0.0, 1.0)
    };

    (map(r), map(g), map(b))
}

/// Apply tone mapping to an entire frame of pixels in-place.
pub fn apply_tone_map_frame(pixels: &mut [(f64, f64, f64)], params: &ToneMapParams) {
    for pixel in pixels.iter_mut() {
        *pixel = apply_tone_map(*pixel, params);
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reinhard_zero() {
        assert_eq!(reinhard(0.0), 0.0);
    }

    #[test]
    fn test_reinhard_one() {
        let v = reinhard(1.0);
        assert!((v - 0.5).abs() < 1e-10, "reinhard(1)=0.5, got {v}");
    }

    #[test]
    fn test_reinhard_negative() {
        assert_eq!(reinhard(-1.0), 0.0, "Negative input should return 0");
    }

    #[test]
    fn test_reinhard_large() {
        let v = reinhard(1000.0);
        assert!(
            v > 0.99 && v < 1.0,
            "reinhard(1000) should be near 1: got {v}"
        );
    }

    #[test]
    fn test_reinhard_extended_at_peak() {
        // At x == peak, should be close to 1.0
        let v = reinhard_extended(100.0, 100.0);
        assert!(v > 0.9, "Extended Reinhard at peak: {v}");
    }

    #[test]
    fn test_reinhard_extended_monotone() {
        let v1 = reinhard_extended(1.0, 10.0);
        let v2 = reinhard_extended(5.0, 10.0);
        assert!(
            v2 > v1,
            "Extended Reinhard should be monotonically increasing"
        );
    }

    #[test]
    fn test_hable_filmic_zero() {
        let v = hable_filmic(0.0);
        assert_eq!(v, 0.0);
    }

    #[test]
    fn test_hable_filmic_range() {
        for x in [0.1, 0.5, 1.0, 2.0, 5.0, 10.0] {
            let v = hable_filmic(x);
            assert!(v >= 0.0 && v <= 1.0, "hable_filmic({x}) = {v} out of [0,1]");
        }
    }

    #[test]
    fn test_aces_filmic_zero() {
        let v = aces_filmic(0.0);
        assert_eq!(v, 0.0);
    }

    #[test]
    fn test_aces_filmic_range() {
        for x in [0.1, 0.5, 1.0, 2.0, 5.0] {
            let v = aces_filmic(x);
            assert!(v >= 0.0 && v <= 1.0, "aces_filmic({x}) = {v} out of [0,1]");
        }
    }

    #[test]
    fn test_aces_filmic_one_maps_below_one() {
        let v = aces_filmic(1.0);
        assert!(
            v < 1.0 && v > 0.0,
            "aces_filmic(1.0) should be in (0,1): {v}"
        );
    }

    #[test]
    fn test_apply_tone_map_output_range() {
        let params = ToneMapParams::default();
        for rgb in [
            (0.0, 0.0, 0.0),
            (500.0, 300.0, 100.0),
            (1000.0, 1000.0, 1000.0),
        ] {
            let (r, g, b) = apply_tone_map(rgb, &params);
            assert!(r >= 0.0 && r <= 1.0, "r={r}");
            assert!(g >= 0.0 && g <= 1.0, "g={g}");
            assert!(b >= 0.0 && b <= 1.0, "b={b}");
        }
    }

    #[test]
    fn test_apply_tone_map_black() {
        let params = ToneMapParams::default();
        let (r, g, b) = apply_tone_map((0.0, 0.0, 0.0), &params);
        assert_eq!((r, g, b), (0.0, 0.0, 0.0));
    }

    #[test]
    fn test_apply_tone_map_frame_all_ops() {
        let ops = [
            ToneMapOperator::Reinhard,
            ToneMapOperator::ReinhardExtended,
            ToneMapOperator::HableFilimic,
            ToneMapOperator::AcesFilmic,
            ToneMapOperator::DragoLogarithmic,
        ];
        for op in ops {
            let params = ToneMapParams {
                operator: op,
                ..Default::default()
            };
            let mut pixels = vec![(500.0, 300.0, 100.0); 10];
            apply_tone_map_frame(&mut pixels, &params);
            for (r, g, b) in &pixels {
                assert!(*r >= 0.0 && *r <= 1.0);
                assert!(*g >= 0.0 && *g <= 1.0);
                assert!(*b >= 0.0 && *b <= 1.0);
            }
        }
    }

    #[test]
    fn test_tone_map_params_default() {
        let p = ToneMapParams::default();
        assert_eq!(p.operator, ToneMapOperator::AcesFilmic);
        assert_eq!(p.peak_luminance, 1000.0);
        assert_eq!(p.gamma, 2.2);
    }

    #[test]
    fn test_drago_logarithmic_zero() {
        assert_eq!(drago_logarithmic(0.0, 1000.0, 0.85), 0.0);
    }

    #[test]
    fn test_drago_logarithmic_positive() {
        let v = drago_logarithmic(100.0, 1000.0, 0.85);
        assert!(v > 0.0 && v <= 1.0, "drago({}) = {v}", 100.0);
    }
}
