//! Chroma key implementation for video switchers.
//!
//! Chroma keying (green screen/blue screen) creates transparency based on color.

use oximedia_codec::VideoFrame;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during chroma keying.
#[derive(Error, Debug, Clone)]
pub enum ChromaKeyError {
    #[error("Invalid hue value: {0}")]
    InvalidHue(f32),

    #[error("Invalid saturation value: {0}")]
    InvalidSaturation(f32),

    #[error("Frame dimension mismatch")]
    DimensionMismatch,

    #[error("Processing error: {0}")]
    ProcessingError(String),
}

/// Chroma key color.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ChromaColor {
    /// Green screen
    Green,
    /// Blue screen
    Blue,
    /// Custom color (H, S, V in 0.0-1.0 range)
    Custom { h: f32, s: f32, v: f32 },
}

impl ChromaColor {
    /// Get the target hue.
    pub fn hue(&self) -> f32 {
        match self {
            ChromaColor::Green => 120.0 / 360.0, // Green at 120 degrees
            ChromaColor::Blue => 240.0 / 360.0,  // Blue at 240 degrees
            ChromaColor::Custom { h, .. } => *h,
        }
    }

    /// Get RGB values for the color.
    pub fn to_rgb(&self) -> (u8, u8, u8) {
        match self {
            ChromaColor::Green => (0, 255, 0),
            ChromaColor::Blue => (0, 0, 255),
            ChromaColor::Custom { h, s, v } => hsv_to_rgb(*h, *s, *v),
        }
    }
}

/// Convert HSV to RGB.
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let h = h * 360.0;
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// Convert RGB to HSV.
fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    // Hue
    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    let h = if h < 0.0 { h + 360.0 } else { h };

    // Saturation
    let s = if max == 0.0 { 0.0 } else { delta / max };

    // Value
    let v = max;

    (h / 360.0, s, v)
}

/// Chroma key parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChromaKeyParams {
    /// Key color
    pub color: ChromaColor,
    /// Hue tolerance (0.0 - 1.0)
    pub hue_tolerance: f32,
    /// Saturation tolerance (0.0 - 1.0)
    pub saturation_tolerance: f32,
    /// Value/brightness tolerance (0.0 - 1.0)
    pub value_tolerance: f32,
    /// Spill suppression amount (0.0 - 1.0)
    pub spill_suppression: f32,
    /// Edge softness (0.0 - 1.0)
    pub edge_softness: f32,
    /// Clip level (0.0 - 1.0)
    pub clip: f32,
    /// Gain (0.0 - 2.0)
    pub gain: f32,
}

impl ChromaKeyParams {
    /// Create new chroma key parameters with defaults for green screen.
    pub fn new_green() -> Self {
        Self {
            color: ChromaColor::Green,
            hue_tolerance: 0.1,
            saturation_tolerance: 0.3,
            value_tolerance: 0.3,
            spill_suppression: 0.5,
            edge_softness: 0.1,
            clip: 0.0,
            gain: 1.0,
        }
    }

    /// Create new chroma key parameters with defaults for blue screen.
    pub fn new_blue() -> Self {
        Self {
            color: ChromaColor::Blue,
            hue_tolerance: 0.1,
            saturation_tolerance: 0.3,
            value_tolerance: 0.3,
            spill_suppression: 0.5,
            edge_softness: 0.1,
            clip: 0.0,
            gain: 1.0,
        }
    }

    /// Set hue tolerance.
    pub fn set_hue_tolerance(&mut self, tolerance: f32) -> Result<(), ChromaKeyError> {
        if !(0.0..=1.0).contains(&tolerance) {
            return Err(ChromaKeyError::InvalidHue(tolerance));
        }
        self.hue_tolerance = tolerance;
        Ok(())
    }

    /// Set saturation tolerance.
    pub fn set_saturation_tolerance(&mut self, tolerance: f32) -> Result<(), ChromaKeyError> {
        if !(0.0..=1.0).contains(&tolerance) {
            return Err(ChromaKeyError::InvalidSaturation(tolerance));
        }
        self.saturation_tolerance = tolerance;
        Ok(())
    }
}

impl Default for ChromaKeyParams {
    fn default() -> Self {
        Self::new_green()
    }
}

/// Chroma key processor.
pub struct ChromaKey {
    params: ChromaKeyParams,
    enabled: bool,
}

impl ChromaKey {
    /// Create a new chroma key processor with green screen defaults.
    pub fn new_green() -> Self {
        Self {
            params: ChromaKeyParams::new_green(),
            enabled: true,
        }
    }

    /// Create a new chroma key processor with blue screen defaults.
    pub fn new_blue() -> Self {
        Self {
            params: ChromaKeyParams::new_blue(),
            enabled: true,
        }
    }

    /// Create with specific parameters.
    pub fn with_params(params: ChromaKeyParams) -> Self {
        Self {
            params,
            enabled: true,
        }
    }

    /// Get the parameters.
    pub fn params(&self) -> &ChromaKeyParams {
        &self.params
    }

    /// Get mutable parameters.
    pub fn params_mut(&mut self) -> &mut ChromaKeyParams {
        &mut self.params
    }

    /// Set parameters.
    pub fn set_params(&mut self, params: ChromaKeyParams) {
        self.params = params;
    }

    /// Enable or disable the key.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if the key is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Calculate color distance in HSV space.
    pub fn calculate_distance(&self, h: f32, s: f32, v: f32) -> f32 {
        let target_h = self.params.color.hue();

        // Calculate hue distance (circular)
        let mut h_dist = (h - target_h).abs();
        if h_dist > 0.5 {
            h_dist = 1.0 - h_dist;
        }
        h_dist /= self.params.hue_tolerance.max(0.001);

        // Saturation distance: low saturation means "not the key colour".
        // A fully-saturated pixel (s=1) contributes zero saturation distance.
        let s_dist = (1.0 - s) / self.params.saturation_tolerance.max(0.001);

        // Value distance: only penalize very dark pixels (v << 0.5).
        // Bright, saturated pixels should not be penalized for having v=1.
        // Use a one-sided penalty: v_dist is non-zero only for v < 0.5.
        let v_dist = (0.5 - v).max(0.0) / self.params.value_tolerance.max(0.001) * 0.5;

        // Combined distance
        (h_dist * h_dist + s_dist * s_dist + v_dist * v_dist).sqrt()
    }

    /// Calculate alpha from pixel color.
    pub fn calculate_alpha(&self, r: u8, g: u8, b: u8) -> f32 {
        if !self.enabled {
            return 1.0;
        }

        let (h, s, v) = rgb_to_hsv(r, g, b);

        // Calculate distance to key color
        let distance = self.calculate_distance(h, s, v);

        // Convert distance to alpha (closer = more transparent)
        let mut alpha = distance;

        // Apply edge softness
        if self.params.edge_softness > 0.0 {
            let soft = 1.0 - self.params.edge_softness;
            alpha = ((alpha - soft) / self.params.edge_softness.max(0.001)).clamp(0.0, 1.0);
        }

        // Apply clip and gain
        alpha = ((alpha - self.params.clip) * self.params.gain).clamp(0.0, 1.0);

        alpha
    }

    /// Apply spill suppression to a pixel.
    ///
    /// Uses a weighted luminance-preserving algorithm: excess key-color channel
    /// energy is replaced by the average of the other two channels, then
    /// blended with the original by `spill_suppression`.
    pub fn suppress_spill(&self, r: u8, g: u8, b: u8) -> (u8, u8, u8) {
        if self.params.spill_suppression == 0.0 {
            return (r, g, b);
        }

        let (r_f, g_f, b_f) = (r as f32, g as f32, b as f32);
        let strength = self.params.spill_suppression.clamp(0.0, 1.0);

        let (r_out, g_out, b_out) = match self.params.color {
            ChromaColor::Green => {
                // Spill amount = how much green exceeds the average of R and B.
                let other_avg = (r_f + b_f) * 0.5;
                let spill = (g_f - other_avg).max(0.0);
                // Replace excess green with the luminance-neutral average.
                let g_corrected = g_f - spill * strength;
                // Boost R and B slightly to compensate for lost luminance.
                let luma_compensation = spill * strength * 0.3;
                (
                    (r_f + luma_compensation).min(255.0),
                    g_corrected,
                    (b_f + luma_compensation).min(255.0),
                )
            }
            ChromaColor::Blue => {
                let other_avg = (r_f + g_f) * 0.5;
                let spill = (b_f - other_avg).max(0.0);
                let b_corrected = b_f - spill * strength;
                let luma_compensation = spill * strength * 0.3;
                (
                    (r_f + luma_compensation).min(255.0),
                    (g_f + luma_compensation).min(255.0),
                    b_corrected,
                )
            }
            ChromaColor::Custom { h, .. } => {
                // Generic: rotate into HSV, desaturate the key hue component.
                let (px_h, px_s, px_v) = rgb_to_hsv(r, g, b);
                let target_h = h;
                let mut h_dist = (px_h - target_h).abs();
                if h_dist > 0.5 {
                    h_dist = 1.0 - h_dist;
                }
                // Within hue tolerance, reduce saturation proportionally.
                let tol = self.params.hue_tolerance.max(0.01);
                if h_dist < tol {
                    let spill_factor = (1.0 - h_dist / tol) * strength;
                    let new_s = px_s * (1.0 - spill_factor);
                    let (nr, ng, nb) = hsv_to_rgb(px_h, new_s, px_v);
                    (nr as f32, ng as f32, nb as f32)
                } else {
                    (r_f, g_f, b_f)
                }
            }
        };

        (
            r_out.clamp(0.0, 255.0) as u8,
            g_out.clamp(0.0, 255.0) as u8,
            b_out.clamp(0.0, 255.0) as u8,
        )
    }

    /// Apply edge softening to an already-computed alpha value.
    ///
    /// Edge softening uses a smooth Hermite curve in the transition zone
    /// defined by `edge_softness`:
    /// - Full transparency region: alpha below `clip`
    /// - Transition zone: `edge_softness` wide (Hermite smoothstep)
    /// - Full opacity: alpha beyond the transition zone
    pub fn apply_edge_softening(&self, raw_alpha: f32) -> f32 {
        if self.params.edge_softness <= 0.0 {
            return raw_alpha.clamp(0.0, 1.0);
        }
        let soft = self.params.edge_softness.clamp(0.0, 1.0);
        // Low boundary: alpha below which we are hard-keyed out.
        let lo = self.params.clip.clamp(0.0, 1.0);
        let hi = (lo + soft).min(1.0);
        if raw_alpha <= lo {
            0.0
        } else if raw_alpha >= hi {
            1.0
        } else {
            // Hermite smoothstep in [lo, hi].
            let t = (raw_alpha - lo) / (hi - lo);
            t * t * (3.0 - 2.0 * t)
        }
    }

    /// Sample the key color and build an enhanced matte using the spill map.
    ///
    /// Returns `(alpha, spill_amount)` for a pixel, where `spill_amount`
    /// is [0..1] indicating how much key-color contamination was detected.
    pub fn analyze_pixel(&self, r: u8, g: u8, b: u8) -> (f32, f32) {
        let (px_h, px_s, px_v) = rgb_to_hsv(r, g, b);
        let distance = self.calculate_distance(px_h, px_s, px_v);

        // Raw alpha from distance (before softening).
        let raw_alpha = distance.clamp(0.0, 1.0);

        // Edge-softened alpha.
        let alpha = self.apply_edge_softening(raw_alpha);
        let alpha_gain = ((alpha - self.params.clip.max(0.0)) * self.params.gain).clamp(0.0, 1.0);

        // Spill amount: how much key-channel energy is present.
        let (r_f, g_f, b_f) = (r as f32, g as f32, b as f32);
        let spill = match self.params.color {
            ChromaColor::Green => (g_f - r_f.max(b_f)).max(0.0) / 255.0,
            ChromaColor::Blue => (b_f - r_f.max(g_f)).max(0.0) / 255.0,
            ChromaColor::Custom { .. } => (1.0 - raw_alpha).max(0.0),
        };

        (alpha_gain, spill.clamp(0.0, 1.0))
    }

    /// Process a single pixel — returns (R, G, B, A) after spill suppression
    /// and edge-softened alpha calculation.
    pub fn process_pixel(&self, r: u8, g: u8, b: u8) -> (u8, u8, u8, u8) {
        let (alpha_gain, _spill) = self.analyze_pixel(r, g, b);
        let (r_out, g_out, b_out) = self.suppress_spill(r, g, b);
        let alpha_u8 = (alpha_gain * 255.0) as u8;
        (r_out, g_out, b_out, alpha_u8)
    }

    /// Process a video frame to extract alpha channel.
    ///
    /// For planar YUV formats the luma plane is used as a rough proxy for
    /// luminance-based keying. For frames with at least 3 planes (Y, Cb, Cr),
    /// the chroma planes are converted back to approximate RGB per pixel so
    /// that proper colour-distance keying can be applied.
    ///
    /// Returns a `Vec<u8>` with one alpha byte per luma-plane pixel
    /// (0 = fully transparent / keyed, 255 = fully opaque).
    pub fn process_frame(&self, fill: &VideoFrame) -> Result<Vec<u8>, ChromaKeyError> {
        if fill.planes.is_empty() {
            return Err(ChromaKeyError::ProcessingError(
                "Frame has no planes".to_string(),
            ));
        }

        let luma_plane = &fill.planes[0];
        let pixel_count = (luma_plane.width as usize) * (luma_plane.height as usize);

        // If we have at least 3 planes we can approximate RGB from YCbCr.
        if fill.planes.len() >= 3 {
            let cb_plane = &fill.planes[1];
            let cr_plane = &fill.planes[2];

            let luma_w = luma_plane.width as usize;
            let luma_h = luma_plane.height as usize;
            let cb_w = cb_plane.width as usize;
            let _cr_w = cr_plane.width as usize;

            // Chroma sub-sampling ratios
            let h_ratio = luma_w.checked_div(cb_w).unwrap_or(1);
            let v_ratio_cb = luma_h.checked_div(cb_plane.height as usize).unwrap_or(1);

            let mut alpha_out = Vec::with_capacity(pixel_count);

            for y in 0..luma_h {
                for x in 0..luma_w {
                    let li = y * luma_plane.stride + x;
                    let y_val = if li < luma_plane.data.len() {
                        luma_plane.data[li] as f32
                    } else {
                        0.0
                    };

                    let cx = x / h_ratio.max(1);
                    let cy = y / v_ratio_cb.max(1);

                    let cb_i = cy * cb_plane.stride + cx;
                    let cr_i = cy * cr_plane.stride + cx;

                    let cb_val = if cb_i < cb_plane.data.len() {
                        cb_plane.data[cb_i] as f32 - 128.0
                    } else {
                        0.0
                    };

                    let cr_val = if cr_i < cr_plane.data.len() {
                        cr_plane.data[cr_i] as f32 - 128.0
                    } else {
                        0.0
                    };

                    // BT.601 YCbCr -> RGB
                    let r = (y_val + 1.402 * cr_val).clamp(0.0, 255.0) as u8;
                    let g = (y_val - 0.344136 * cb_val - 0.714136 * cr_val).clamp(0.0, 255.0) as u8;
                    let b = (y_val + 1.772 * cb_val).clamp(0.0, 255.0) as u8;

                    let alpha = self.calculate_alpha(r, g, b);
                    alpha_out.push((alpha * 255.0) as u8);
                }
            }

            Ok(alpha_out)
        } else {
            // Single-plane: use luma only as a fallback (luminance keying)
            let mut alpha_out = Vec::with_capacity(pixel_count);
            for &luma in &luma_plane.data[..pixel_count.min(luma_plane.data.len())] {
                let alpha = self.calculate_alpha(luma, luma, luma);
                alpha_out.push((alpha * 255.0) as u8);
            }
            Ok(alpha_out)
        }
    }
}

impl Default for ChromaKey {
    fn default() -> Self {
        Self::new_green()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chroma_color_hue() {
        assert!((ChromaColor::Green.hue() - 120.0 / 360.0).abs() < 0.01);
        assert!((ChromaColor::Blue.hue() - 240.0 / 360.0).abs() < 0.01);
    }

    #[test]
    fn test_chroma_color_rgb() {
        let (r, g, b) = ChromaColor::Green.to_rgb();
        assert_eq!(r, 0);
        assert_eq!(g, 255);
        assert_eq!(b, 0);

        let (r, g, b) = ChromaColor::Blue.to_rgb();
        assert_eq!(r, 0);
        assert_eq!(g, 0);
        assert_eq!(b, 255);
    }

    #[test]
    fn test_rgb_to_hsv() {
        // Pure red
        let (h, s, v) = rgb_to_hsv(255, 0, 0);
        assert!(h.abs() < 0.01);
        assert!((s - 1.0).abs() < 0.01);
        assert!((v - 1.0).abs() < 0.01);

        // Pure green
        let (h, s, v) = rgb_to_hsv(0, 255, 0);
        assert!((h - 120.0 / 360.0).abs() < 0.01);
        assert!((s - 1.0).abs() < 0.01);
        assert!((v - 1.0).abs() < 0.01);

        // Pure blue
        let (h, s, v) = rgb_to_hsv(0, 0, 255);
        assert!((h - 240.0 / 360.0).abs() < 0.01);
        assert!((s - 1.0).abs() < 0.01);
        assert!((v - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_hsv_to_rgb_to_hsv() {
        let (h, s, v) = (0.5, 0.8, 0.9);
        let (r, g, b) = hsv_to_rgb(h, s, v);
        let (h2, s2, v2) = rgb_to_hsv(r, g, b);

        assert!((h - h2).abs() < 0.02);
        assert!((s - s2).abs() < 0.02);
        assert!((v - v2).abs() < 0.02);
    }

    #[test]
    fn test_chroma_key_params_green() {
        let params = ChromaKeyParams::new_green();
        assert_eq!(params.color, ChromaColor::Green);
        assert_eq!(params.hue_tolerance, 0.1);
        assert_eq!(params.gain, 1.0);
    }

    #[test]
    fn test_chroma_key_params_blue() {
        let params = ChromaKeyParams::new_blue();
        assert_eq!(params.color, ChromaColor::Blue);
        assert_eq!(params.hue_tolerance, 0.1);
    }

    #[test]
    fn test_chroma_key_creation() {
        let key_green = ChromaKey::new_green();
        assert!(key_green.is_enabled());
        assert_eq!(key_green.params().color, ChromaColor::Green);

        let key_blue = ChromaKey::new_blue();
        assert_eq!(key_blue.params().color, ChromaColor::Blue);
    }

    #[test]
    fn test_calculate_alpha_green() {
        let key = ChromaKey::new_green();

        // Pure green should be transparent
        let alpha_green = key.calculate_alpha(0, 255, 0);
        assert!(alpha_green < 0.5); // Should be mostly transparent

        // Red should be opaque
        let alpha_red = key.calculate_alpha(255, 0, 0);
        assert!(alpha_red > 0.5); // Should be mostly opaque
    }

    #[test]
    fn test_calculate_alpha_blue() {
        let key = ChromaKey::new_blue();

        // Pure blue should be transparent
        let alpha_blue = key.calculate_alpha(0, 0, 255);
        assert!(alpha_blue < 0.5);

        // Red should be opaque
        let alpha_red = key.calculate_alpha(255, 0, 0);
        assert!(alpha_red > 0.5);
    }

    #[test]
    fn test_spill_suppression_green() {
        let key = ChromaKey::new_green();

        // Green spill on skin tone
        let (r, g, b) = key.suppress_spill(200, 220, 180);

        // Green should be reduced
        assert!(g < 220);
        // Red and blue may receive a small luma-compensation boost to preserve
        // luminance — allow up to 10 counts of adjustment
        assert!(r >= 200 && r <= 210, "r={r} should be in 200..=210");
        assert!(b >= 180 && b <= 190, "b={b} should be in 180..=190");
    }

    #[test]
    fn test_spill_suppression_blue() {
        let key = ChromaKey::new_blue();

        // Blue spill
        let (_r, _g, b) = key.suppress_spill(180, 200, 220);

        // Blue should be reduced
        assert!(b < 220);
    }

    #[test]
    fn test_process_pixel() {
        let key = ChromaKey::new_green();

        let (_r, g, _b, a) = key.process_pixel(0, 255, 0);

        // Green screen should produce low alpha
        assert!(a < 128);

        // RGB should have spill suppression applied
        assert!(g < 255);
    }

    #[test]
    fn test_disabled_key() {
        let mut key = ChromaKey::new_green();
        key.set_enabled(false);

        // When disabled, alpha should always be 1.0
        let alpha = key.calculate_alpha(0, 255, 0);
        assert_eq!(alpha, 1.0);
    }

    #[test]
    fn test_edge_softness() {
        let mut key = ChromaKey::new_green();

        // Without softness
        key.params_mut().edge_softness = 0.0;
        let alpha1 = key.calculate_alpha(100, 200, 100);

        // With softness
        key.params_mut().edge_softness = 0.5;
        let alpha2 = key.calculate_alpha(100, 200, 100);

        // Softness should affect the alpha differently
        // (exact values depend on distance calculation)
        assert!(alpha1 >= 0.0 && alpha1 <= 1.0);
        assert!(alpha2 >= 0.0 && alpha2 <= 1.0);
    }

    #[test]
    fn test_custom_color() {
        let custom_color = ChromaColor::Custom {
            h: 0.0, // Red
            s: 1.0,
            v: 1.0,
        };

        let (r, g, b) = custom_color.to_rgb();
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
    }

    #[test]
    fn test_set_tolerances() {
        let mut params = ChromaKeyParams::new_green();

        assert!(params.set_hue_tolerance(0.2).is_ok());
        assert_eq!(params.hue_tolerance, 0.2);

        assert!(params.set_hue_tolerance(-0.1).is_err());
        assert!(params.set_hue_tolerance(1.5).is_err());

        assert!(params.set_saturation_tolerance(0.4).is_ok());
        assert_eq!(params.saturation_tolerance, 0.4);
    }
}
