//! Color grading filter with lift/gamma/gain, curves, and color wheels.
//!
//! This filter provides professional color grading tools including:
//!
//! - **Lift/Gamma/Gain:** Primary color correction controls
//!   - Lift adjusts shadows (blacks)
//!   - Gamma adjusts midtones
//!   - Gain adjusts highlights (whites)
//!   - Per-channel RGB and master controls
//!   - ASC CDL (Color Decision List) implementation
//!
//! - **Curves:** Precise tonal and color control
//!   - RGB curves (per-channel)
//!   - Luma curve (brightness)
//!   - Hue vs. Saturation curves
//!   - Hue vs. Luminance curves
//!   - Cubic spline interpolation
//!   - Bezier curve support
//!   - Curve presets (S-curve, contrast, etc.)
//!
//! - **Color Wheels:** Intuitive color grading controls
//!   - Shadow/Midtone/Highlight wheels
//!   - Hue shift per region
//!   - Saturation adjustment
//!   - Log/Offset/Power controls
//!   - Temperature/Tint adjustment
//!
//! - **HSL Qualifiers:** Secondary color correction
//!   - Hue range selection
//!   - Saturation range selection
//!   - Luminance range selection
//!   - Soft edge feathering
//!   - Keying and masking
//!
//! # Example
//!
//! ```ignore
//! use oximedia_graph::filters::video::{ColorGradingFilter, ColorGradingConfig};
//! use oximedia_graph::node::NodeId;
//!
//! let config = ColorGradingConfig::new()
//!     .with_lift(0.1, 0.0, -0.1)
//!     .with_gamma(1.2, 1.0, 0.8)
//!     .with_gain(1.1, 1.0, 0.9);
//!
//! let filter = ColorGradingFilter::new(NodeId(0), "grading", config);
//! ```

#![forbid(unsafe_code)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::similar_names)]
#![allow(clippy::many_single_char_names)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::struct_excessive_bools)]
#![allow(dead_code)]

use crate::error::{GraphError, GraphResult};
use crate::frame::FilterFrame;
use crate::node::{Node, NodeId, NodeState, NodeType};
use crate::port::{InputPort, OutputPort, PortId, PortType};
use oximedia_codec::VideoFrame;
use oximedia_core::PixelFormat;

// ============================================================================
// Core Data Structures
// ============================================================================

/// RGB color representation for color grading operations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RgbColor {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

impl RgbColor {
    /// Create a new RGB color.
    #[must_use]
    pub const fn new(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b }
    }

    /// Create a grayscale color.
    #[must_use]
    pub const fn gray(v: f64) -> Self {
        Self { r: v, g: v, b: v }
    }

    /// Clamp color values to [0, 1] range.
    #[must_use]
    pub fn clamp(self) -> Self {
        Self {
            r: self.r.clamp(0.0, 1.0),
            g: self.g.clamp(0.0, 1.0),
            b: self.b.clamp(0.0, 1.0),
        }
    }

    /// Convert to HSL color space.
    #[must_use]
    pub fn to_hsl(self) -> HslColor {
        rgb_to_hsl(self.r, self.g, self.b)
    }

    /// Linear interpolation between two colors.
    #[must_use]
    pub fn lerp(self, other: &Self, t: f64) -> Self {
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
        }
    }
}

/// HSL (Hue, Saturation, Luminance) color representation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HslColor {
    /// Hue (0.0 - 360.0)
    pub h: f64,
    /// Saturation (0.0 - 1.0)
    pub s: f64,
    /// Luminance (0.0 - 1.0)
    pub l: f64,
}

impl HslColor {
    /// Create a new HSL color.
    #[must_use]
    pub const fn new(h: f64, s: f64, l: f64) -> Self {
        Self { h, s, l }
    }

    /// Convert to RGB color space.
    #[must_use]
    pub fn to_rgb(self) -> RgbColor {
        hsl_to_rgb(self.h, self.s, self.l)
    }
}

// ============================================================================
// Lift/Gamma/Gain (Primary Correction)
// ============================================================================

/// Lift/Gamma/Gain color correction parameters.
///
/// These controls provide primary color correction:
/// - Lift: Adjusts the black level (shadows)
/// - Gamma: Adjusts the midtones
/// - Gain: Adjusts the white level (highlights)
#[derive(Clone, Debug, PartialEq)]
pub struct LiftGammaGain {
    /// Lift (shadows) adjustment per channel.
    pub lift: RgbColor,
    /// Gamma (midtones) adjustment per channel.
    pub gamma: RgbColor,
    /// Gain (highlights) adjustment per channel.
    pub gain: RgbColor,
    /// Master lift (affects all channels).
    pub master_lift: f64,
    /// Master gamma (affects all channels).
    pub master_gamma: f64,
    /// Master gain (affects all channels).
    pub master_gain: f64,
    /// Contrast (affects the curve slope).
    pub contrast: f64,
    /// Pivot point for contrast adjustment (0.0-1.0).
    pub pivot: f64,
    /// Enable printer lights mode (legacy film grading).
    pub printer_lights: bool,
}

impl Default for LiftGammaGain {
    fn default() -> Self {
        Self {
            lift: RgbColor::gray(0.0),
            gamma: RgbColor::gray(1.0),
            gain: RgbColor::gray(1.0),
            master_lift: 0.0,
            master_gamma: 1.0,
            master_gain: 1.0,
            contrast: 1.0,
            pivot: 0.5,
            printer_lights: false,
        }
    }
}

impl LiftGammaGain {
    /// Create new lift/gamma/gain with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set lift values (shadows).
    #[must_use]
    pub fn with_lift(mut self, r: f64, g: f64, b: f64) -> Self {
        self.lift = RgbColor::new(r, g, b);
        self
    }

    /// Set gamma values (midtones).
    #[must_use]
    pub fn with_gamma(mut self, r: f64, g: f64, b: f64) -> Self {
        self.gamma = RgbColor::new(r, g, b);
        self
    }

    /// Set gain values (highlights).
    #[must_use]
    pub fn with_gain(mut self, r: f64, g: f64, b: f64) -> Self {
        self.gain = RgbColor::new(r, g, b);
        self
    }

    /// Set master lift.
    #[must_use]
    pub const fn with_master_lift(mut self, lift: f64) -> Self {
        self.master_lift = lift;
        self
    }

    /// Set master gamma.
    #[must_use]
    pub const fn with_master_gamma(mut self, gamma: f64) -> Self {
        self.master_gamma = gamma;
        self
    }

    /// Set master gain.
    #[must_use]
    pub const fn with_master_gain(mut self, gain: f64) -> Self {
        self.master_gain = gain;
        self
    }

    /// Set contrast.
    #[must_use]
    pub const fn with_contrast(mut self, contrast: f64) -> Self {
        self.contrast = contrast;
        self
    }

    /// Set pivot point for contrast.
    #[must_use]
    pub const fn with_pivot(mut self, pivot: f64) -> Self {
        self.pivot = pivot;
        self
    }

    /// Enable printer lights mode.
    #[must_use]
    pub const fn with_printer_lights(mut self, enabled: bool) -> Self {
        self.printer_lights = enabled;
        self
    }

    /// Apply lift/gamma/gain to a color.
    #[must_use]
    pub fn apply(&self, color: RgbColor) -> RgbColor {
        if self.printer_lights {
            return self.apply_printer_lights(color);
        }

        let r = self.apply_channel(color.r, self.lift.r, self.gamma.r, self.gain.r);
        let g = self.apply_channel(color.g, self.lift.g, self.gamma.g, self.gain.g);
        let b = self.apply_channel(color.b, self.lift.b, self.gamma.b, self.gain.b);

        let mut result = RgbColor::new(r, g, b);

        // Apply master controls
        result = self.apply_master(result);

        // Apply contrast around pivot
        result = self.apply_contrast(result);

        result
    }

    /// Apply printer lights mode (legacy film grading).
    /// In this mode, values are interpreted as light points (1 point = 0.025 log exposure).
    fn apply_printer_lights(&self, color: RgbColor) -> RgbColor {
        let points_to_linear = |points: f64| -> f64 {
            if points.abs() < f64::EPSILON {
                1.0
            } else {
                2_f64.powf(points * 0.025 / 0.3) // ~1 point = 1/12 stop
            }
        };

        let r = color.r * points_to_linear(-self.lift.r) * points_to_linear(-self.gain.r);
        let g = color.g * points_to_linear(-self.lift.g) * points_to_linear(-self.gain.g);
        let b = color.b * points_to_linear(-self.lift.b) * points_to_linear(-self.gain.b);

        RgbColor::new(r, g, b)
    }

    /// Apply contrast around pivot point.
    fn apply_contrast(&self, color: RgbColor) -> RgbColor {
        if (self.contrast - 1.0).abs() < f64::EPSILON {
            return color;
        }

        // Apply contrast around pivot
        let apply_to_channel = |value: f64| -> f64 {
            let centered = value - self.pivot;
            let contrasted = centered * self.contrast;
            (contrasted + self.pivot).clamp(0.0, 1.0)
        };

        RgbColor::new(
            apply_to_channel(color.r),
            apply_to_channel(color.g),
            apply_to_channel(color.b),
        )
    }

    /// Apply lift/gamma/gain to a single channel.
    fn apply_channel(&self, value: f64, lift: f64, gamma: f64, gain: f64) -> f64 {
        // Lift: adds to the signal (affects shadows most)
        let lifted = value + lift;

        // Gamma: power function (affects midtones)
        let gamma_corrected = if lifted > 0.0 && gamma > 0.0 {
            lifted.powf(1.0 / gamma)
        } else {
            lifted
        };

        // Gain: multiplies the signal (affects highlights most)
        gamma_corrected * gain
    }

    /// Apply master controls.
    fn apply_master(&self, color: RgbColor) -> RgbColor {
        let r = color.r + self.master_lift;
        let g = color.g + self.master_lift;
        let b = color.b + self.master_lift;

        let r = if r > 0.0 && self.master_gamma > 0.0 {
            r.powf(1.0 / self.master_gamma)
        } else {
            r
        };
        let g = if g > 0.0 && self.master_gamma > 0.0 {
            g.powf(1.0 / self.master_gamma)
        } else {
            g
        };
        let b = if b > 0.0 && self.master_gamma > 0.0 {
            b.powf(1.0 / self.master_gamma)
        } else {
            b
        };

        RgbColor::new(
            r * self.master_gain,
            g * self.master_gain,
            b * self.master_gain,
        )
    }
}

/// ASC CDL (Color Decision List) parameters.
///
/// Standard color correction format used in professional workflows.
/// Formula: out = (in * slope + offset)^power
#[derive(Clone, Debug, PartialEq)]
pub struct AscCdl {
    /// Slope (similar to gain).
    pub slope: RgbColor,
    /// Offset (similar to lift).
    pub offset: RgbColor,
    /// Power (similar to gamma).
    pub power: RgbColor,
    /// Saturation adjustment.
    pub saturation: f64,
}

impl Default for AscCdl {
    fn default() -> Self {
        Self {
            slope: RgbColor::gray(1.0),
            offset: RgbColor::gray(0.0),
            power: RgbColor::gray(1.0),
            saturation: 1.0,
        }
    }
}

impl AscCdl {
    /// Create new ASC CDL with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply ASC CDL to a color.
    #[must_use]
    pub fn apply(&self, color: RgbColor) -> RgbColor {
        // Apply slope and offset
        let r = color.r * self.slope.r + self.offset.r;
        let g = color.g * self.slope.g + self.offset.g;
        let b = color.b * self.slope.b + self.offset.b;

        // Clamp to positive values before power
        let r = r.max(0.0);
        let g = g.max(0.0);
        let b = b.max(0.0);

        // Apply power
        let r = r.powf(self.power.r);
        let g = g.powf(self.power.g);
        let b = b.powf(self.power.b);

        let mut result = RgbColor::new(r, g, b);

        // Apply saturation
        if (self.saturation - 1.0).abs() > f64::EPSILON {
            result = apply_saturation(result, self.saturation);
        }

        result
    }
}

// ============================================================================
// Curves
// ============================================================================

/// A point on a curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurvePoint {
    /// X coordinate (input value)
    pub x: f64,
    /// Y coordinate (output value)
    pub y: f64,
}

impl CurvePoint {
    /// Create a new curve point.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Type of curve interpolation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveInterpolation {
    /// Linear interpolation.
    Linear,
    /// Cubic spline interpolation.
    CubicSpline,
    /// Bezier curve interpolation.
    Bezier,
}

/// A color correction curve.
#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    /// Control points defining the curve.
    points: Vec<CurvePoint>,
    /// Interpolation method.
    interpolation: CurveInterpolation,
    /// Cached lookup table for performance.
    lut: Vec<f64>,
}

impl Curve {
    /// Create a new linear (identity) curve.
    #[must_use]
    pub fn linear() -> Self {
        Self {
            points: vec![CurvePoint::new(0.0, 0.0), CurvePoint::new(1.0, 1.0)],
            interpolation: CurveInterpolation::Linear,
            lut: Vec::new(),
        }
    }

    /// Create a new curve with specified points.
    #[must_use]
    pub fn with_points(mut points: Vec<CurvePoint>) -> Self {
        // Sort points by x coordinate
        points.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());

        let mut curve = Self {
            points,
            interpolation: CurveInterpolation::CubicSpline,
            lut: Vec::new(),
        };

        curve.rebuild_lut();
        curve
    }

    /// Set interpolation method.
    #[must_use]
    pub fn with_interpolation(mut self, interpolation: CurveInterpolation) -> Self {
        self.interpolation = interpolation;
        self.rebuild_lut();
        self
    }

    /// Add a control point to the curve.
    pub fn add_point(&mut self, x: f64, y: f64) {
        let point = CurvePoint::new(x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
        self.points.push(point);
        self.points.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
        self.rebuild_lut();
    }

    /// Remove a control point.
    pub fn remove_point(&mut self, index: usize) {
        if index < self.points.len() && self.points.len() > 2 {
            self.points.remove(index);
            self.rebuild_lut();
        }
    }

    /// Move a control point.
    pub fn move_point(&mut self, index: usize, x: f64, y: f64) {
        if index < self.points.len() {
            self.points[index] = CurvePoint::new(x.clamp(0.0, 1.0), y.clamp(0.0, 1.0));
            self.points.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
            self.rebuild_lut();
        }
    }

    /// Evaluate the curve at a given x position.
    #[must_use]
    pub fn evaluate(&self, x: f64) -> f64 {
        if !self.lut.is_empty() {
            // Use cached LUT
            let index = (x * (self.lut.len() - 1) as f64).clamp(0.0, (self.lut.len() - 1) as f64);
            let i0 = index as usize;
            let i1 = (i0 + 1).min(self.lut.len() - 1);
            let frac = index - i0 as f64;
            return self.lut[i0] + (self.lut[i1] - self.lut[i0]) * frac;
        }

        // Fallback to direct evaluation
        self.evaluate_direct(x)
    }

    /// Evaluate the curve directly without using LUT.
    fn evaluate_direct(&self, x: f64) -> f64 {
        let x = x.clamp(0.0, 1.0);

        match self.interpolation {
            CurveInterpolation::Linear => self.evaluate_linear(x),
            CurveInterpolation::CubicSpline => self.evaluate_cubic_spline(x),
            CurveInterpolation::Bezier => self.evaluate_bezier(x),
        }
    }

    /// Linear interpolation between points.
    fn evaluate_linear(&self, x: f64) -> f64 {
        if self.points.is_empty() {
            return x;
        }

        if x <= self.points[0].x {
            return self.points[0].y;
        }

        for i in 0..self.points.len() - 1 {
            let p0 = self.points[i];
            let p1 = self.points[i + 1];

            if x >= p0.x && x <= p1.x {
                let t = (x - p0.x) / (p1.x - p0.x);
                return p0.y + (p1.y - p0.y) * t;
            }
        }

        self.points.last().unwrap().y
    }

    /// Cubic spline interpolation using Catmull-Rom splines.
    fn evaluate_cubic_spline(&self, x: f64) -> f64 {
        if self.points.len() < 2 {
            return x;
        }

        if x <= self.points[0].x {
            return self.points[0].y;
        }

        if x >= self.points.last().unwrap().x {
            return self.points.last().unwrap().y;
        }

        // Find the segment containing x
        for i in 0..self.points.len() - 1 {
            let p1 = self.points[i];
            let p2 = self.points[i + 1];

            if x >= p1.x && x <= p2.x {
                // Get neighboring points for Catmull-Rom
                let p0 = if i > 0 {
                    self.points[i - 1]
                } else {
                    // Extrapolate before first point
                    CurvePoint::new(p1.x - (p2.x - p1.x), p1.y - (p2.y - p1.y))
                };

                let p3 = if i + 2 < self.points.len() {
                    self.points[i + 2]
                } else {
                    // Extrapolate after last point
                    CurvePoint::new(p2.x + (p2.x - p1.x), p2.y + (p2.y - p1.y))
                };

                // Normalize t to [0, 1] within segment
                let t = (x - p1.x) / (p2.x - p1.x);

                // Catmull-Rom interpolation
                let t2 = t * t;
                let t3 = t2 * t;

                let y = 0.5
                    * ((2.0 * p1.y)
                        + (-p0.y + p2.y) * t
                        + (2.0 * p0.y - 5.0 * p1.y + 4.0 * p2.y - p3.y) * t2
                        + (-p0.y + 3.0 * p1.y - 3.0 * p2.y + p3.y) * t3);

                return y.clamp(0.0, 1.0);
            }
        }

        x
    }

    /// Bezier curve interpolation using cubic Bezier.
    fn evaluate_bezier(&self, x: f64) -> f64 {
        if self.points.len() < 2 {
            return x;
        }

        // Use cubic Bezier segments between control points
        for i in 0..self.points.len() - 1 {
            let p0 = self.points[i];
            let p3 = self.points[i + 1];

            if x >= p0.x && x <= p3.x {
                // Compute control points for smooth curve
                let dx = p3.x - p0.x;
                let dy = p3.y - p0.y;

                // Smooth tangent (1/3 rule)
                let p1 = CurvePoint::new(p0.x + dx / 3.0, p0.y + dy / 3.0);
                let p2 = CurvePoint::new(p3.x - dx / 3.0, p3.y - dy / 3.0);

                // Binary search for t parameter
                let mut t_min = 0.0;
                let mut t_max = 1.0;
                let mut t = 0.5;

                for _ in 0..20 {
                    // Cubic Bezier x(t)
                    let x_t = (1.0_f64 - t).powi(3) * p0.x
                        + 3.0 * (1.0_f64 - t).powi(2) * t * p1.x
                        + 3.0 * (1.0_f64 - t) * t.powi(2) * p2.x
                        + t.powi(3) * p3.x;

                    if (x_t - x).abs() < 1e-6 {
                        break;
                    }

                    if x_t < x {
                        t_min = t;
                    } else {
                        t_max = t;
                    }

                    t = (t_min + t_max) / 2.0;
                }

                // Compute y(t)
                let y = (1.0 - t).powi(3) * p0.y
                    + 3.0 * (1.0 - t).powi(2) * t * p1.y
                    + 3.0 * (1.0 - t) * t.powi(2) * p2.y
                    + t.powi(3) * p3.y;

                return y.clamp(0.0, 1.0);
            }
        }

        x
    }

    /// Rebuild the lookup table.
    fn rebuild_lut(&mut self) {
        const LUT_SIZE: usize = 1024;
        self.lut.clear();
        self.lut.reserve(LUT_SIZE);

        for i in 0..LUT_SIZE {
            let x = i as f64 / (LUT_SIZE - 1) as f64;
            self.lut.push(self.evaluate_direct(x));
        }
    }

    /// Create an S-curve for contrast adjustment.
    #[must_use]
    pub fn s_curve(strength: f64) -> Self {
        let mid = 0.5;
        let offset = strength * 0.25;

        Self::with_points(vec![
            CurvePoint::new(0.0, 0.0),
            CurvePoint::new(mid - 0.25, mid - offset),
            CurvePoint::new(mid, mid),
            CurvePoint::new(mid + 0.25, mid + offset),
            CurvePoint::new(1.0, 1.0),
        ])
    }

    /// Create a contrast curve.
    #[must_use]
    pub fn contrast(contrast: f64) -> Self {
        let mid = 0.5;
        let y_low = mid - (mid * contrast);
        let y_high = mid + ((1.0 - mid) * contrast);

        Self::with_points(vec![
            CurvePoint::new(0.0, y_low.max(0.0)),
            CurvePoint::new(mid, mid),
            CurvePoint::new(1.0, y_high.min(1.0)),
        ])
    }

    /// Create a brightness curve.
    #[must_use]
    pub fn brightness(amount: f64) -> Self {
        Self::with_points(vec![
            CurvePoint::new(0.0, amount.max(0.0)),
            CurvePoint::new(1.0, (1.0 + amount).min(1.0)),
        ])
    }

    /// Create an exposure curve (logarithmic).
    #[must_use]
    pub fn exposure(stops: f64) -> Self {
        let factor = 2_f64.powf(stops);
        Self::with_points(vec![
            CurvePoint::new(0.0, 0.0),
            CurvePoint::new(0.5, (0.5 * factor).min(1.0)),
            CurvePoint::new(1.0, (1.0 * factor).min(1.0)),
        ])
    }

    /// Create a highlights recovery curve.
    #[must_use]
    pub fn highlights(amount: f64) -> Self {
        Self::with_points(vec![
            CurvePoint::new(0.0, 0.0),
            CurvePoint::new(0.7, 0.7),
            CurvePoint::new(0.85, 0.85 - amount * 0.15),
            CurvePoint::new(1.0, (1.0 - amount * 0.3).max(0.7)),
        ])
    }

    /// Create a shadows lift curve.
    #[must_use]
    pub fn shadows(amount: f64) -> Self {
        Self::with_points(vec![
            CurvePoint::new(0.0, (amount * 0.3).max(0.0)),
            CurvePoint::new(0.15, 0.15 + amount * 0.15),
            CurvePoint::new(0.3, 0.3),
            CurvePoint::new(1.0, 1.0),
        ])
    }

    /// Create an inverted curve.
    #[must_use]
    pub fn invert() -> Self {
        Self::with_points(vec![CurvePoint::new(0.0, 1.0), CurvePoint::new(1.0, 0.0)])
    }

    /// Create a film-like response curve.
    #[must_use]
    pub fn film_response() -> Self {
        Self::with_points(vec![
            CurvePoint::new(0.0, 0.0),
            CurvePoint::new(0.1, 0.15),
            CurvePoint::new(0.3, 0.35),
            CurvePoint::new(0.5, 0.55),
            CurvePoint::new(0.7, 0.73),
            CurvePoint::new(0.9, 0.88),
            CurvePoint::new(1.0, 0.95),
        ])
    }

    /// Create a cross-process effect curve.
    #[must_use]
    pub fn cross_process(strength: f64) -> Self {
        Self::with_points(vec![
            CurvePoint::new(0.0, 0.05 * strength),
            CurvePoint::new(0.25, 0.2 + 0.1 * strength),
            CurvePoint::new(0.5, 0.5),
            CurvePoint::new(0.75, 0.8 - 0.1 * strength),
            CurvePoint::new(1.0, 0.95 - 0.05 * strength),
        ])
    }

    /// Get all control points.
    #[must_use]
    pub fn points(&self) -> &[CurvePoint] {
        &self.points
    }

    /// Get the interpolation method.
    #[must_use]
    pub const fn interpolation(&self) -> CurveInterpolation {
        self.interpolation
    }
}

/// RGB curves for per-channel color correction.
#[derive(Clone, Debug)]
pub struct RgbCurves {
    /// Red channel curve
    pub red: Curve,
    /// Green channel curve
    pub green: Curve,
    /// Blue channel curve
    pub blue: Curve,
    /// Master curve applied to all channels
    pub master: Curve,
}

impl Default for RgbCurves {
    fn default() -> Self {
        Self {
            red: Curve::linear(),
            green: Curve::linear(),
            blue: Curve::linear(),
            master: Curve::linear(),
        }
    }
}

impl RgbCurves {
    /// Create new RGB curves.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply curves to a color.
    #[must_use]
    pub fn apply(&self, color: RgbColor) -> RgbColor {
        // Apply master curve first
        let r = self.master.evaluate(color.r);
        let g = self.master.evaluate(color.g);
        let b = self.master.evaluate(color.b);

        // Apply per-channel curves
        RgbColor::new(
            self.red.evaluate(r),
            self.green.evaluate(g),
            self.blue.evaluate(b),
        )
    }
}

/// Hue vs. Saturation curve.
#[derive(Clone, Debug)]
pub struct HueVsSatCurve {
    curve: Curve,
}

impl Default for HueVsSatCurve {
    fn default() -> Self {
        Self {
            curve: Curve::linear(),
        }
    }
}

impl HueVsSatCurve {
    /// Create a new hue vs. saturation curve.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply the curve to a color.
    #[must_use]
    pub fn apply(&self, color: RgbColor) -> RgbColor {
        let hsl = color.to_hsl();

        // Map hue (0-360) to curve input (0-1)
        let hue_norm = hsl.h / 360.0;
        let sat_mult = self.curve.evaluate(hue_norm);

        // Apply saturation adjustment
        let new_sat = (hsl.s * sat_mult).clamp(0.0, 1.0);

        HslColor::new(hsl.h, new_sat, hsl.l).to_rgb()
    }
}

/// Hue vs. Luminance curve.
#[derive(Clone, Debug)]
pub struct HueVsLumCurve {
    curve: Curve,
}

impl Default for HueVsLumCurve {
    fn default() -> Self {
        Self {
            curve: Curve::linear(),
        }
    }
}

impl HueVsLumCurve {
    /// Create a new hue vs. luminance curve.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply the curve to a color.
    #[must_use]
    pub fn apply(&self, color: RgbColor) -> RgbColor {
        let hsl = color.to_hsl();

        // Map hue (0-360) to curve input (0-1)
        let hue_norm = hsl.h / 360.0;
        let lum_mult = self.curve.evaluate(hue_norm);

        // Apply luminance adjustment
        let new_lum = (hsl.l * lum_mult).clamp(0.0, 1.0);

        HslColor::new(hsl.h, hsl.s, new_lum).to_rgb()
    }
}

// ============================================================================
// Color Wheels
// ============================================================================

/// Color wheel parameters for a specific tonal region.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorWheel {
    /// Hue shift in degrees (-180 to 180).
    pub hue: f64,
    /// Saturation adjustment (0.0 = grayscale, 1.0 = no change, 2.0 = double).
    pub saturation: f64,
    /// Luminance adjustment (-1.0 to 1.0).
    pub luminance: f64,
}

impl Default for ColorWheel {
    fn default() -> Self {
        Self {
            hue: 0.0,
            saturation: 1.0,
            luminance: 0.0,
        }
    }
}

impl ColorWheel {
    /// Create a new color wheel.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set hue shift.
    #[must_use]
    pub const fn with_hue(mut self, hue: f64) -> Self {
        self.hue = hue;
        self
    }

    /// Set saturation.
    #[must_use]
    pub const fn with_saturation(mut self, saturation: f64) -> Self {
        self.saturation = saturation;
        self
    }

    /// Set luminance.
    #[must_use]
    pub const fn with_luminance(mut self, luminance: f64) -> Self {
        self.luminance = luminance;
        self
    }

    /// Apply color wheel to a color with given weight.
    #[must_use]
    pub fn apply(&self, color: RgbColor, weight: f64) -> RgbColor {
        if weight <= 0.0 {
            return color;
        }

        let mut hsl = color.to_hsl();

        // Apply hue shift
        hsl.h = (hsl.h + self.hue * weight).rem_euclid(360.0);

        // Apply saturation
        hsl.s = (hsl.s * (1.0 + (self.saturation - 1.0) * weight)).clamp(0.0, 1.0);

        // Apply luminance
        hsl.l = (hsl.l + self.luminance * weight).clamp(0.0, 1.0);

        hsl.to_rgb()
    }
}

/// Shadow/Midtone/Highlight color wheels.
#[derive(Clone, Debug)]
pub struct ColorWheels {
    /// Shadow color wheel adjustments
    pub shadows: ColorWheel,
    /// Midtone color wheel adjustments
    pub midtones: ColorWheel,
    /// Highlight color wheel adjustments
    pub highlights: ColorWheel,
    /// Shadow/highlight split point (0.0 - 1.0).
    pub shadow_max: f64,
    /// Midtone/highlight split point (0.0 - 1.0).
    pub highlight_min: f64,
    /// Enable offset mode (affects entire tonal range equally).
    pub offset_mode: bool,
    /// Global saturation multiplier.
    pub global_saturation: f64,
}

impl Default for ColorWheels {
    fn default() -> Self {
        Self {
            shadows: ColorWheel::default(),
            midtones: ColorWheel::default(),
            highlights: ColorWheel::default(),
            shadow_max: 0.33,
            highlight_min: 0.67,
            offset_mode: false,
            global_saturation: 1.0,
        }
    }
}

impl ColorWheels {
    /// Create new color wheels.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply color wheels to a color.
    #[must_use]
    pub fn apply(&self, color: RgbColor) -> RgbColor {
        let mut result = color;

        if self.offset_mode {
            // Offset mode: apply all wheels equally (no luminance-based weighting)
            result = self.shadows.apply(result, 1.0);
            result = self.midtones.apply(result, 1.0);
            result = self.highlights.apply(result, 1.0);
        } else {
            // Normal mode: weight by luminance
            let luma = 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;

            // Calculate weights for each region
            let shadow_weight = self.calculate_shadow_weight(luma);
            let midtone_weight = self.calculate_midtone_weight(luma);
            let highlight_weight = self.calculate_highlight_weight(luma);

            // Apply each wheel with its weight
            result = self.shadows.apply(result, shadow_weight);
            result = self.midtones.apply(result, midtone_weight);
            result = self.highlights.apply(result, highlight_weight);
        }

        // Apply global saturation
        if (self.global_saturation - 1.0).abs() > f64::EPSILON {
            result = apply_saturation(result, self.global_saturation);
        }

        result
    }

    /// Set offset mode.
    #[must_use]
    pub const fn with_offset_mode(mut self, enabled: bool) -> Self {
        self.offset_mode = enabled;
        self
    }

    /// Set global saturation.
    #[must_use]
    pub const fn with_global_saturation(mut self, saturation: f64) -> Self {
        self.global_saturation = saturation;
        self
    }

    /// Set shadow/midtone/highlight split points.
    #[must_use]
    pub const fn with_split_points(mut self, shadow_max: f64, highlight_min: f64) -> Self {
        self.shadow_max = shadow_max;
        self.highlight_min = highlight_min;
        self
    }

    /// Calculate shadow weight based on luminance.
    fn calculate_shadow_weight(&self, luma: f64) -> f64 {
        if luma <= self.shadow_max {
            1.0
        } else if luma >= self.highlight_min {
            0.0
        } else {
            // Smooth transition
            let t = (luma - self.shadow_max) / (self.highlight_min - self.shadow_max);
            (1.0 - t).max(0.0)
        }
    }

    /// Calculate midtone weight based on luminance.
    fn calculate_midtone_weight(&self, luma: f64) -> f64 {
        if luma <= self.shadow_max || luma >= self.highlight_min {
            0.0
        } else {
            // Peak at midpoint
            let mid = (self.shadow_max + self.highlight_min) / 2.0;
            let range = self.highlight_min - self.shadow_max;
            1.0 - ((luma - mid).abs() / (range / 2.0))
        }
    }

    /// Calculate highlight weight based on luminance.
    fn calculate_highlight_weight(&self, luma: f64) -> f64 {
        if luma >= self.highlight_min {
            1.0
        } else if luma <= self.shadow_max {
            0.0
        } else {
            // Smooth transition
            let t = (luma - self.shadow_max) / (self.highlight_min - self.shadow_max);
            t.max(0.0)
        }
    }
}

/// Log/Offset/Power color correction.
#[derive(Clone, Debug, PartialEq)]
pub struct LogOffsetPower {
    /// Logarithmic adjustment
    pub log: RgbColor,
    /// Linear offset adjustment
    pub offset: RgbColor,
    /// Power/gamma adjustment
    pub power: RgbColor,
}

impl Default for LogOffsetPower {
    fn default() -> Self {
        Self {
            log: RgbColor::gray(0.0),
            offset: RgbColor::gray(0.0),
            power: RgbColor::gray(1.0),
        }
    }
}

impl LogOffsetPower {
    /// Create new log/offset/power.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply log/offset/power to a color.
    #[must_use]
    pub fn apply(&self, color: RgbColor) -> RgbColor {
        let r = self.apply_channel(color.r, self.log.r, self.offset.r, self.power.r);
        let g = self.apply_channel(color.g, self.log.g, self.offset.g, self.power.g);
        let b = self.apply_channel(color.b, self.log.b, self.offset.b, self.power.b);

        RgbColor::new(r, g, b)
    }

    /// Apply to a single channel.
    fn apply_channel(&self, value: f64, log: f64, offset: f64, power: f64) -> f64 {
        // Log adjustment (affects overall brightness)
        let logged = if value > 0.0 {
            value * 2_f64.powf(log)
        } else {
            value
        };

        // Offset (adds to signal)
        let offsetted = logged + offset;

        // Power (gamma-like adjustment)
        if offsetted > 0.0 && power > 0.0 {
            offsetted.powf(power)
        } else {
            offsetted
        }
    }
}

/// Temperature and tint adjustment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TemperatureTint {
    /// Temperature adjustment in Kelvin offset (-10000 to 10000).
    pub temperature: f64,
    /// Tint adjustment (magenta/green) (-1.0 to 1.0).
    pub tint: f64,
}

impl Default for TemperatureTint {
    fn default() -> Self {
        Self {
            temperature: 0.0,
            tint: 0.0,
        }
    }
}

impl TemperatureTint {
    /// Create new temperature/tint adjustment.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply temperature and tint to a color.
    #[must_use]
    pub fn apply(&self, color: RgbColor) -> RgbColor {
        let mut result = color;

        // Apply temperature (blue/orange shift)
        if self.temperature.abs() > f64::EPSILON {
            let temp_factor = self.temperature / 10000.0;
            result.r += temp_factor;
            result.b -= temp_factor;
        }

        // Apply tint (magenta/green shift)
        if self.tint.abs() > f64::EPSILON {
            result.g += self.tint * 0.5;
            result.r -= self.tint * 0.25;
            result.b -= self.tint * 0.25;
        }

        result
    }
}

// ============================================================================
// HSL Qualifiers (Secondary Color Correction)
// ============================================================================

/// HSL range qualifier for secondary color correction.
#[derive(Clone, Debug)]
pub struct HslQualifier {
    /// Hue range (min, max) in degrees (0-360).
    pub hue_range: (f64, f64),
    /// Saturation range (min, max) (0.0-1.0).
    pub sat_range: (f64, f64),
    /// Luminance range (min, max) (0.0-1.0).
    pub lum_range: (f64, f64),
    /// Soft edge feathering amount (0.0-1.0).
    pub feather: f64,
    /// Enable hue qualification.
    pub qualify_hue: bool,
    /// Enable saturation qualification.
    pub qualify_sat: bool,
    /// Enable luminance qualification.
    pub qualify_lum: bool,
    /// Invert the mask.
    pub invert: bool,
    /// Blur radius for mask softening.
    pub blur_radius: f64,
    /// Denoise strength for mask cleanup.
    pub denoise: f64,
    /// Matte black point (0.0-1.0).
    pub black_point: f64,
    /// Matte white point (0.0-1.0).
    pub white_point: f64,
}

impl Default for HslQualifier {
    fn default() -> Self {
        Self {
            hue_range: (0.0, 360.0),
            sat_range: (0.0, 1.0),
            lum_range: (0.0, 1.0),
            feather: 0.1,
            qualify_hue: false,
            qualify_sat: false,
            qualify_lum: false,
            invert: false,
            blur_radius: 0.0,
            denoise: 0.0,
            black_point: 0.0,
            white_point: 1.0,
        }
    }
}

impl HslQualifier {
    /// Create a new HSL qualifier.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set hue range.
    #[must_use]
    pub fn with_hue_range(mut self, min: f64, max: f64) -> Self {
        self.hue_range = (min, max);
        self.qualify_hue = true;
        self
    }

    /// Set saturation range.
    #[must_use]
    pub fn with_sat_range(mut self, min: f64, max: f64) -> Self {
        self.sat_range = (min, max);
        self.qualify_sat = true;
        self
    }

    /// Set luminance range.
    #[must_use]
    pub fn with_lum_range(mut self, min: f64, max: f64) -> Self {
        self.lum_range = (min, max);
        self.qualify_lum = true;
        self
    }

    /// Set feathering amount.
    #[must_use]
    pub const fn with_feather(mut self, feather: f64) -> Self {
        self.feather = feather;
        self
    }

    /// Set invert flag.
    #[must_use]
    pub const fn with_invert(mut self, invert: bool) -> Self {
        self.invert = invert;
        self
    }

    /// Set blur radius.
    #[must_use]
    pub const fn with_blur(mut self, radius: f64) -> Self {
        self.blur_radius = radius;
        self
    }

    /// Set denoise strength.
    #[must_use]
    pub const fn with_denoise(mut self, strength: f64) -> Self {
        self.denoise = strength;
        self
    }

    /// Set matte black and white points.
    #[must_use]
    pub const fn with_matte_range(mut self, black: f64, white: f64) -> Self {
        self.black_point = black;
        self.white_point = white;
        self
    }

    /// Calculate the mask value for a color (0.0 = no effect, 1.0 = full effect).
    #[must_use]
    pub fn calculate_mask(&self, color: RgbColor) -> f64 {
        let hsl = color.to_hsl();

        let mut mask = 1.0;

        // Check hue range
        if self.qualify_hue {
            let hue_mask = self.calculate_hue_mask(hsl.h);
            mask *= hue_mask;
        }

        // Check saturation range
        if self.qualify_sat {
            let sat_mask = self.calculate_range_mask(hsl.s, self.sat_range.0, self.sat_range.1);
            mask *= sat_mask;
        }

        // Check luminance range
        if self.qualify_lum {
            let lum_mask = self.calculate_range_mask(hsl.l, self.lum_range.0, self.lum_range.1);
            mask *= lum_mask;
        }

        // Apply matte refinement (levels adjustment)
        mask = self.refine_matte(mask);

        // Invert if requested
        if self.invert {
            mask = 1.0 - mask;
        }

        mask
    }

    /// Refine the matte using black and white point adjustments.
    fn refine_matte(&self, mask: f64) -> f64 {
        if (self.black_point - 0.0).abs() < f64::EPSILON
            && (self.white_point - 1.0).abs() < f64::EPSILON
        {
            return mask;
        }

        // Levels adjustment
        if mask <= self.black_point {
            0.0
        } else if mask >= self.white_point {
            1.0
        } else {
            // Linear remap from [black, white] to [0, 1]
            (mask - self.black_point) / (self.white_point - self.black_point)
        }
    }

    /// Apply denoise to mask (simple threshold-based).
    fn denoise_mask(&self, mask: f64) -> f64 {
        if self.denoise <= 0.0 {
            return mask;
        }

        // Simple threshold denoising
        let threshold = self.denoise * 0.1;
        if mask < threshold {
            0.0
        } else if mask > 1.0 - threshold {
            1.0
        } else {
            // Smooth transition
            let t = (mask - threshold) / (1.0 - 2.0 * threshold);
            t.clamp(0.0, 1.0)
        }
    }

    /// Calculate hue mask with wrapping support.
    fn calculate_hue_mask(&self, hue: f64) -> f64 {
        let (min, max) = self.hue_range;

        if min <= max {
            // Normal range
            self.calculate_range_mask(hue, min, max)
        } else {
            // Wrapped range (e.g., 350-10 degrees)
            let mask1 = self.calculate_range_mask(hue, min, 360.0);
            let mask2 = self.calculate_range_mask(hue, 0.0, max);
            mask1.max(mask2)
        }
    }

    /// Calculate mask for a linear range with feathering.
    fn calculate_range_mask(&self, value: f64, min: f64, max: f64) -> f64 {
        if value < min {
            // Below range
            if self.feather > 0.0 {
                let dist = min - value;
                let fade = (1.0 - (dist / self.feather)).clamp(0.0, 1.0);
                fade * fade // Smooth falloff
            } else {
                0.0
            }
        } else if value > max {
            // Above range
            if self.feather > 0.0 {
                let dist = value - max;
                let fade = (1.0 - (dist / self.feather)).clamp(0.0, 1.0);
                fade * fade // Smooth falloff
            } else {
                0.0
            }
        } else {
            // Within range
            1.0
        }
    }

    /// Apply a correction to a color using this qualifier.
    #[must_use]
    pub fn apply<F>(&self, color: RgbColor, correction: F) -> RgbColor
    where
        F: Fn(RgbColor) -> RgbColor,
    {
        let mask = self.calculate_mask(color);

        if mask <= 0.0 {
            return color;
        }

        let corrected = correction(color);

        if mask >= 1.0 {
            corrected
        } else {
            // Blend based on mask
            color.lerp(&corrected, mask)
        }
    }
}

/// Multiple HSL qualifiers that can be combined.
#[derive(Clone, Debug)]
pub struct MultiQualifier {
    /// Individual qualifiers.
    qualifiers: Vec<HslQualifier>,
    /// Combination mode.
    mode: QualifierCombineMode,
}

/// Mode for combining multiple qualifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualifierCombineMode {
    /// Union (OR) - pixel matches if it matches any qualifier.
    Union,
    /// Intersection (AND) - pixel matches only if it matches all qualifiers.
    Intersection,
    /// Difference - first minus others.
    Difference,
}

impl MultiQualifier {
    /// Create a new multi-qualifier.
    #[must_use]
    pub fn new() -> Self {
        Self {
            qualifiers: Vec::new(),
            mode: QualifierCombineMode::Union,
        }
    }

    /// Add a qualifier.
    pub fn add_qualifier(&mut self, qualifier: HslQualifier) {
        self.qualifiers.push(qualifier);
    }

    /// Set combine mode.
    pub fn set_mode(&mut self, mode: QualifierCombineMode) {
        self.mode = mode;
    }

    /// Calculate combined mask.
    #[must_use]
    pub fn calculate_mask(&self, color: RgbColor) -> f64 {
        if self.qualifiers.is_empty() {
            return 1.0;
        }

        let masks: Vec<f64> = self
            .qualifiers
            .iter()
            .map(|q| q.calculate_mask(color))
            .collect();

        match self.mode {
            QualifierCombineMode::Union => masks.iter().fold(0.0_f64, |acc, &m| acc.max(m)),
            QualifierCombineMode::Intersection => masks.iter().fold(1.0_f64, |acc, &m| acc.min(m)),
            QualifierCombineMode::Difference => {
                if masks.is_empty() {
                    return 0.0;
                }
                let first = masks[0];
                let others_max = masks.iter().skip(1).fold(0.0_f64, |acc, &m| acc.max(m));
                (first - others_max).max(0.0)
            }
        }
    }
}

impl Default for MultiQualifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Vectorscope analyzer for color distribution analysis.
#[derive(Clone, Debug)]
pub struct VectorscopeAnalyzer {
    /// Resolution of the vectorscope (size x size).
    resolution: usize,
    /// Accumulated data.
    data: Vec<f64>,
}

impl VectorscopeAnalyzer {
    /// Create a new vectorscope analyzer.
    #[must_use]
    pub fn new(resolution: usize) -> Self {
        Self {
            resolution,
            data: vec![0.0; resolution * resolution],
        }
    }

    /// Add a color sample to the vectorscope.
    pub fn add_sample(&mut self, color: RgbColor) {
        let hsl = color.to_hsl();

        // Convert hue and saturation to vectorscope coordinates
        let angle = hsl.h * std::f64::consts::PI / 180.0;
        let radius = hsl.s * 0.5; // Map saturation to radius

        let x = 0.5 + radius * angle.cos();
        let y = 0.5 + radius * angle.sin();

        let ix = (x * self.resolution as f64) as usize;
        let iy = (y * self.resolution as f64) as usize;

        if ix < self.resolution && iy < self.resolution {
            let index = iy * self.resolution + ix;
            self.data[index] += 1.0;
        }
    }

    /// Get the vectorscope data.
    #[must_use]
    pub fn data(&self) -> &[f64] {
        &self.data
    }

    /// Clear the vectorscope data.
    pub fn clear(&mut self) {
        self.data.fill(0.0);
    }

    /// Get peak saturation in a hue range.
    #[must_use]
    pub fn peak_saturation_in_range(&self, hue_min: f64, hue_max: f64) -> f64 {
        let mut max_sat: f64 = 0.0;

        for y in 0..self.resolution {
            for x in 0..self.resolution {
                let fx = x as f64 / self.resolution as f64 - 0.5;
                let fy = y as f64 / self.resolution as f64 - 0.5;

                let radius = (fx * fx + fy * fy).sqrt();
                let angle = fy.atan2(fx) * 180.0 / std::f64::consts::PI;
                let hue = (angle + 360.0).rem_euclid(360.0);

                if hue >= hue_min && hue <= hue_max {
                    let sat = radius * 2.0;
                    max_sat = max_sat.max(sat);
                }
            }
        }

        max_sat.min(1.0)
    }
}

/// Waveform monitor for luminance analysis.
#[derive(Clone, Debug)]
pub struct WaveformMonitor {
    /// Width of the waveform.
    width: usize,
    /// Height of the waveform (represents luminance range 0-1).
    height: usize,
    /// Accumulated data.
    data: Vec<f64>,
}

impl WaveformMonitor {
    /// Create a new waveform monitor.
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![0.0; width * height],
        }
    }

    /// Add a sample at a specific x position.
    pub fn add_sample(&mut self, x_pos: f64, luma: f64) {
        let x = (x_pos * self.width as f64) as usize;
        let y = ((1.0 - luma.clamp(0.0, 1.0)) * (self.height - 1) as f64) as usize;

        if x < self.width && y < self.height {
            let index = y * self.width + x;
            self.data[index] += 1.0;
        }
    }

    /// Get the waveform data.
    #[must_use]
    pub fn data(&self) -> &[f64] {
        &self.data
    }

    /// Clear the waveform data.
    pub fn clear(&mut self) {
        self.data.fill(0.0);
    }

    /// Get average luminance in a horizontal range.
    #[must_use]
    pub fn average_luma_in_range(&self, x_min: f64, x_max: f64) -> f64 {
        let x_start = (x_min * self.width as f64) as usize;
        let x_end = (x_max * self.width as f64) as usize;

        let mut sum = 0.0;
        let mut count = 0;

        for y in 0..self.height {
            for x in x_start..x_end.min(self.width) {
                let index = y * self.width + x;
                if self.data[index] > 0.0 {
                    let luma = 1.0 - (y as f64 / (self.height - 1) as f64);
                    sum += luma * self.data[index];
                    count += self.data[index] as usize;
                }
            }
        }

        if count > 0 {
            sum / count as f64
        } else {
            0.0
        }
    }
}

/// Histogram for analyzing color channel distributions.
#[derive(Clone, Debug)]
pub struct ColorHistogram {
    /// Number of bins.
    bins: usize,
    /// Red channel histogram.
    pub red: Vec<u32>,
    /// Green channel histogram.
    pub green: Vec<u32>,
    /// Blue channel histogram.
    pub blue: Vec<u32>,
    /// Luminance histogram.
    pub luma: Vec<u32>,
}

impl ColorHistogram {
    /// Create a new histogram.
    #[must_use]
    pub fn new(bins: usize) -> Self {
        Self {
            bins,
            red: vec![0; bins],
            green: vec![0; bins],
            blue: vec![0; bins],
            luma: vec![0; bins],
        }
    }

    /// Add a color sample.
    pub fn add_sample(&mut self, color: RgbColor) {
        let r_bin = (color.r.clamp(0.0, 1.0) * (self.bins - 1) as f64) as usize;
        let g_bin = (color.g.clamp(0.0, 1.0) * (self.bins - 1) as f64) as usize;
        let b_bin = (color.b.clamp(0.0, 1.0) * (self.bins - 1) as f64) as usize;

        let luma = 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;
        let l_bin = (luma.clamp(0.0, 1.0) * (self.bins - 1) as f64) as usize;

        self.red[r_bin] += 1;
        self.green[g_bin] += 1;
        self.blue[b_bin] += 1;
        self.luma[l_bin] += 1;
    }

    /// Clear the histogram.
    pub fn clear(&mut self) {
        self.red.fill(0);
        self.green.fill(0);
        self.blue.fill(0);
        self.luma.fill(0);
    }

    /// Get the median value for a channel.
    #[must_use]
    pub fn median(&self, channel: ColorChannel) -> f64 {
        let histogram = match channel {
            ColorChannel::Red => &self.red,
            ColorChannel::Green => &self.green,
            ColorChannel::Blue => &self.blue,
            ColorChannel::Luma => &self.luma,
        };

        let total: u32 = histogram.iter().sum();
        if total == 0 {
            return 0.5;
        }

        let half = total / 2;
        let mut acc = 0;

        for (i, &count) in histogram.iter().enumerate() {
            acc += count;
            if acc >= half {
                return i as f64 / (self.bins - 1) as f64;
            }
        }

        0.5
    }

    /// Get the mean value for a channel.
    #[must_use]
    pub fn mean(&self, channel: ColorChannel) -> f64 {
        let histogram = match channel {
            ColorChannel::Red => &self.red,
            ColorChannel::Green => &self.green,
            ColorChannel::Blue => &self.blue,
            ColorChannel::Luma => &self.luma,
        };

        let total: u32 = histogram.iter().sum();
        if total == 0 {
            return 0.5;
        }

        let mut sum = 0.0;
        for (i, &count) in histogram.iter().enumerate() {
            sum += (i as f64 / (self.bins - 1) as f64) * count as f64;
        }

        sum / total as f64
    }

    /// Get percentile value for a channel.
    #[must_use]
    pub fn percentile(&self, channel: ColorChannel, percentile: f64) -> f64 {
        let histogram = match channel {
            ColorChannel::Red => &self.red,
            ColorChannel::Green => &self.green,
            ColorChannel::Blue => &self.blue,
            ColorChannel::Luma => &self.luma,
        };

        let total: u32 = histogram.iter().sum();
        if total == 0 {
            return 0.5;
        }

        let target = (total as f64 * percentile.clamp(0.0, 1.0)) as u32;
        let mut acc = 0;

        for (i, &count) in histogram.iter().enumerate() {
            acc += count;
            if acc >= target {
                return i as f64 / (self.bins - 1) as f64;
            }
        }

        1.0
    }
}

/// Color channel for histogram operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorChannel {
    Red,
    Green,
    Blue,
    Luma,
}

// ============================================================================
// Main Color Grading Configuration
// ============================================================================

/// Complete color grading configuration.
#[derive(Clone, Debug)]
pub struct ColorGradingConfig {
    /// Lift/Gamma/Gain correction.
    pub lgg: LiftGammaGain,
    /// ASC CDL correction.
    pub cdl: AscCdl,
    /// RGB curves.
    pub rgb_curves: RgbCurves,
    /// Luma curve.
    pub luma_curve: Curve,
    /// Hue vs. Saturation curve.
    pub hue_vs_sat: HueVsSatCurve,
    /// Hue vs. Luminance curve.
    pub hue_vs_lum: HueVsLumCurve,
    /// Color wheels.
    pub color_wheels: ColorWheels,
    /// Log/Offset/Power.
    pub lop: LogOffsetPower,
    /// Temperature/Tint.
    pub temp_tint: TemperatureTint,
    /// HSL qualifier for secondary correction.
    pub qualifier: HslQualifier,
    /// Enable lift/gamma/gain.
    pub enable_lgg: bool,
    /// Enable ASC CDL.
    pub enable_cdl: bool,
    /// Enable RGB curves.
    pub enable_rgb_curves: bool,
    /// Enable luma curve.
    pub enable_luma_curve: bool,
    /// Enable hue vs. sat curve.
    pub enable_hue_vs_sat: bool,
    /// Enable hue vs. lum curve.
    pub enable_hue_vs_lum: bool,
    /// Enable color wheels.
    pub enable_color_wheels: bool,
    /// Enable log/offset/power.
    pub enable_lop: bool,
    /// Enable temperature/tint.
    pub enable_temp_tint: bool,
    /// Enable HSL qualifier.
    pub enable_qualifier: bool,
}

impl Default for ColorGradingConfig {
    fn default() -> Self {
        Self {
            lgg: LiftGammaGain::default(),
            cdl: AscCdl::default(),
            rgb_curves: RgbCurves::default(),
            luma_curve: Curve::linear(),
            hue_vs_sat: HueVsSatCurve::default(),
            hue_vs_lum: HueVsLumCurve::default(),
            color_wheels: ColorWheels::default(),
            lop: LogOffsetPower::default(),
            temp_tint: TemperatureTint::default(),
            qualifier: HslQualifier::default(),
            enable_lgg: false,
            enable_cdl: false,
            enable_rgb_curves: false,
            enable_luma_curve: false,
            enable_hue_vs_sat: false,
            enable_hue_vs_lum: false,
            enable_color_wheels: false,
            enable_lop: false,
            enable_temp_tint: false,
            enable_qualifier: false,
        }
    }
}

impl ColorGradingConfig {
    /// Create a new color grading configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable and set lift values.
    #[must_use]
    pub fn with_lift(mut self, r: f64, g: f64, b: f64) -> Self {
        self.lgg.lift = RgbColor::new(r, g, b);
        self.enable_lgg = true;
        self
    }

    /// Enable and set gamma values.
    #[must_use]
    pub fn with_gamma(mut self, r: f64, g: f64, b: f64) -> Self {
        self.lgg.gamma = RgbColor::new(r, g, b);
        self.enable_lgg = true;
        self
    }

    /// Enable and set gain values.
    #[must_use]
    pub fn with_gain(mut self, r: f64, g: f64, b: f64) -> Self {
        self.lgg.gain = RgbColor::new(r, g, b);
        self.enable_lgg = true;
        self
    }

    /// Apply all color grading to a color.
    #[must_use]
    pub fn apply(&self, color: RgbColor) -> RgbColor {
        let mut result = color;

        // Apply temperature/tint first (white balance)
        if self.enable_temp_tint {
            result = self.temp_tint.apply(result);
        }

        // Apply lift/gamma/gain
        if self.enable_lgg {
            result = self.lgg.apply(result);
        }

        // Apply ASC CDL
        if self.enable_cdl {
            result = self.cdl.apply(result);
        }

        // Apply log/offset/power
        if self.enable_lop {
            result = self.lop.apply(result);
        }

        // Apply RGB curves
        if self.enable_rgb_curves {
            result = self.rgb_curves.apply(result);
        }

        // Apply luma curve
        if self.enable_luma_curve {
            let luma = 0.2126 * result.r + 0.7152 * result.g + 0.0722 * result.b;
            let new_luma = self.luma_curve.evaluate(luma);
            let scale = if luma > 0.0 { new_luma / luma } else { 1.0 };
            result = RgbColor::new(result.r * scale, result.g * scale, result.b * scale);
        }

        // Apply color wheels
        if self.enable_color_wheels {
            result = self.color_wheels.apply(result);
        }

        // Apply hue vs. saturation
        if self.enable_hue_vs_sat {
            result = self.hue_vs_sat.apply(result);
        }

        // Apply hue vs. luminance
        if self.enable_hue_vs_lum {
            result = self.hue_vs_lum.apply(result);
        }

        // Apply qualifier if enabled
        if self.enable_qualifier {
            let mask = self.qualifier.calculate_mask(color);
            result = color.lerp(&result, mask);
        }

        result
    }
}

// ============================================================================
// Color Grading Filter
// ============================================================================

/// Color grading filter node.
pub struct ColorGradingFilter {
    id: NodeId,
    name: String,
    state: NodeState,
    input: InputPort,
    output: OutputPort,
    config: ColorGradingConfig,
}

impl ColorGradingFilter {
    /// Create a new color grading filter.
    #[must_use]
    pub fn new(id: NodeId, name: &str, config: ColorGradingConfig) -> Self {
        Self {
            id,
            name: name.to_string(),
            state: NodeState::Idle,
            input: InputPort::new(PortId(0), "input", PortType::Video),
            output: OutputPort::new(PortId(1), "output", PortType::Video),
            config,
        }
    }

    /// Process a video frame.
    fn process_frame(&self, frame: VideoFrame) -> GraphResult<VideoFrame> {
        // Only process RGB formats for now
        match frame.format {
            PixelFormat::Rgb24 | PixelFormat::Rgba32 => self.process_rgb_frame(frame),
            _ => {
                // Return frame unchanged for unsupported formats
                Ok(frame)
            }
        }
    }

    /// Process an RGB frame.
    fn process_rgb_frame(&self, mut frame: VideoFrame) -> GraphResult<VideoFrame> {
        let width = frame.width;
        let height = frame.height;
        let planes = &mut frame.planes;

        if planes.is_empty() {
            return Ok(frame);
        }

        let plane = &mut planes[0];
        let stride = plane.stride;
        let data = plane.data.as_mut_slice();

        let bytes_per_pixel = match frame.format {
            PixelFormat::Rgb24 => 3,
            PixelFormat::Rgba32 => 4,
            _ => return Ok(frame),
        };

        for y in 0..height as usize {
            for x in 0..width as usize {
                let offset = y * stride + x * bytes_per_pixel;

                // Read color
                let r = data[offset] as f64 / 255.0;
                let g = data[offset + 1] as f64 / 255.0;
                let b = data[offset + 2] as f64 / 255.0;

                let color = RgbColor::new(r, g, b);

                // Apply color grading
                let graded = self.config.apply(color).clamp();

                // Write back
                data[offset] = (graded.r * 255.0) as u8;
                data[offset + 1] = (graded.g * 255.0) as u8;
                data[offset + 2] = (graded.b * 255.0) as u8;
            }
        }

        Ok(frame)
    }
}

impl Node for ColorGradingFilter {
    fn id(&self) -> NodeId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn node_type(&self) -> NodeType {
        NodeType::Filter
    }

    fn state(&self) -> NodeState {
        self.state
    }

    fn set_state(&mut self, state: NodeState) -> GraphResult<()> {
        self.state = state;
        Ok(())
    }

    fn inputs(&self) -> &[InputPort] {
        std::slice::from_ref(&self.input)
    }

    fn outputs(&self) -> &[OutputPort] {
        std::slice::from_ref(&self.output)
    }

    fn process(&mut self, input: Option<FilterFrame>) -> GraphResult<Option<FilterFrame>> {
        match input {
            Some(FilterFrame::Video(video_frame)) => {
                let processed = self.process_frame(video_frame)?;
                Ok(Some(FilterFrame::Video(processed)))
            }
            Some(_) => Err(GraphError::ProcessingError {
                node: self.id,
                message: "Color grading filter expects video input".to_string(),
            }),
            None => Ok(None),
        }
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Convert RGB to HSL color space.
fn rgb_to_hsl(r: f64, g: f64, b: f64) -> HslColor {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let l = (max + min) / 2.0;

    if delta < f64::EPSILON {
        return HslColor::new(0.0, 0.0, l);
    }

    let s = if l < 0.5 {
        delta / (max + min)
    } else {
        delta / (2.0 - max - min)
    };

    let h = if (r - max).abs() < f64::EPSILON {
        ((g - b) / delta).rem_euclid(6.0)
    } else if (g - max).abs() < f64::EPSILON {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };

    HslColor::new(h * 60.0, s, l)
}

/// Convert HSL to RGB color space.
fn hsl_to_rgb(h: f64, s: f64, l: f64) -> RgbColor {
    if s < f64::EPSILON {
        return RgbColor::gray(l);
    }

    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - ((h_prime % 2.0) - 1.0).abs());

    let (r1, g1, b1) = match h_prime as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    let m = l - c / 2.0;

    RgbColor::new(r1 + m, g1 + m, b1 + m)
}

/// Apply saturation adjustment to a color.
fn apply_saturation(color: RgbColor, saturation: f64) -> RgbColor {
    let luma = 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;

    RgbColor::new(
        luma + (color.r - luma) * saturation,
        luma + (color.g - luma) * saturation,
        luma + (color.b - luma) * saturation,
    )
}

// ============================================================================
// Preset Color Grading Looks
// ============================================================================

/// Preset color grading configurations for common looks.
pub mod presets {
    use super::*;

    /// Cinematic look with teal shadows and orange highlights.
    #[must_use]
    pub fn cinematic_teal_orange() -> ColorGradingConfig {
        let mut config = ColorGradingConfig::new();

        // Teal shadows, orange highlights
        config.color_wheels.shadows.hue = 180.0; // Teal
        config.color_wheels.shadows.saturation = 1.3;
        config.color_wheels.highlights.hue = 30.0; // Orange
        config.color_wheels.highlights.saturation = 1.2;

        config.enable_color_wheels = true;

        // Add some contrast
        config.lgg.contrast = 1.15;
        config.enable_lgg = true;

        config
    }

    /// Vintage film look with faded blacks and warm tones.
    #[must_use]
    pub fn vintage_film() -> ColorGradingConfig {
        let mut config = ColorGradingConfig::new();

        // Lift blacks (faded look)
        config.lgg.lift = RgbColor::new(0.1, 0.08, 0.05);
        config.lgg.gamma = RgbColor::new(1.1, 1.0, 0.95);
        config.enable_lgg = true;

        // Warm tones
        config.temp_tint.temperature = 500.0;
        config.enable_temp_tint = true;

        // Reduce saturation slightly
        config.color_wheels.global_saturation = 0.85;
        config.enable_color_wheels = true;

        // Film response curve
        config.rgb_curves.master = Curve::film_response();
        config.enable_rgb_curves = true;

        config
    }

    /// High-contrast black and white.
    #[must_use]
    pub fn black_and_white_contrast() -> ColorGradingConfig {
        let mut config = ColorGradingConfig::new();

        // Remove all color
        config.color_wheels.global_saturation = 0.0;
        config.enable_color_wheels = true;

        // High contrast
        config.lgg.contrast = 1.4;
        config.lgg.pivot = 0.45;
        config.enable_lgg = true;

        // Crushing blacks slightly
        config.lgg.lift = RgbColor::new(-0.05, -0.05, -0.05);

        config
    }

    /// Bleach bypass look (desaturated with crushed highlights).
    #[must_use]
    pub fn bleach_bypass() -> ColorGradingConfig {
        let mut config = ColorGradingConfig::new();

        // Reduced saturation
        config.color_wheels.global_saturation = 0.6;
        config.enable_color_wheels = true;

        // High contrast
        config.lgg.contrast = 1.35;
        config.enable_lgg = true;

        // Crush highlights
        config.rgb_curves.master = Curve::highlights(-0.3);
        config.enable_rgb_curves = true;

        config
    }

    /// Warm sunset look.
    #[must_use]
    pub fn warm_sunset() -> ColorGradingConfig {
        let mut config = ColorGradingConfig::new();

        // Warm temperature
        config.temp_tint.temperature = 2000.0;
        config.enable_temp_tint = true;

        // Orange/red highlights
        config.color_wheels.highlights.hue = 15.0;
        config.color_wheels.highlights.saturation = 1.4;
        config.enable_color_wheels = true;

        // Lift shadows slightly
        config.lgg.master_lift = 0.05;
        config.enable_lgg = true;

        config
    }

    /// Cool moonlight look.
    #[must_use]
    pub fn cool_moonlight() -> ColorGradingConfig {
        let mut config = ColorGradingConfig::new();

        // Cool temperature
        config.temp_tint.temperature = -3000.0;
        config.enable_temp_tint = true;

        // Blue shadows
        config.color_wheels.shadows.hue = 220.0; // Blue
        config.color_wheels.shadows.saturation = 1.3;
        config.enable_color_wheels = true;

        // Reduce overall saturation
        config.color_wheels.global_saturation = 0.75;

        // Darken slightly
        config.lgg.master_lift = -0.1;
        config.enable_lgg = true;

        config
    }

    /// Cross-processed look (shifted colors).
    #[must_use]
    pub fn cross_process() -> ColorGradingConfig {
        let mut config = ColorGradingConfig::new();

        // Shifted color curves
        config.rgb_curves.red = Curve::cross_process(0.8);
        config.rgb_curves.green = Curve::cross_process(0.6);
        config.rgb_curves.blue = Curve::cross_process(0.7);
        config.enable_rgb_curves = true;

        // Increased saturation
        config.color_wheels.global_saturation = 1.3;
        config.enable_color_wheels = true;

        config
    }

    /// HDR look (enhanced dynamic range appearance).
    #[must_use]
    pub fn hdr_look() -> ColorGradingConfig {
        let mut config = ColorGradingConfig::new();

        // Lift shadows significantly
        config.lgg.lift = RgbColor::new(0.15, 0.15, 0.15);
        config.enable_lgg = true;

        // Compress highlights
        config.rgb_curves.master = Curve::highlights(-0.4);
        config.enable_rgb_curves = true;

        // Boost saturation
        config.color_wheels.global_saturation = 1.4;
        config.enable_color_wheels = true;

        config
    }

    /// Day for night conversion.
    #[must_use]
    pub fn day_for_night() -> ColorGradingConfig {
        let mut config = ColorGradingConfig::new();

        // Darken significantly
        config.lgg.master_gain = 0.4;
        config.enable_lgg = true;

        // Cool blue tones
        config.temp_tint.temperature = -4000.0;
        config.enable_temp_tint = true;

        // Blue everything
        config.color_wheels.midtones.hue = 220.0;
        config.color_wheels.midtones.saturation = 1.2;
        config.color_wheels.offset_mode = true;
        config.enable_color_wheels = true;

        // Reduce overall saturation
        config.color_wheels.global_saturation = 0.6;

        config
    }

    /// Sepia tone (classic brown/yellow tinted look).
    #[must_use]
    pub fn sepia() -> ColorGradingConfig {
        let mut config = ColorGradingConfig::new();

        // Remove color first
        config.color_wheels.global_saturation = 0.0;
        config.enable_color_wheels = true;

        // Add sepia tones through offset mode
        config.color_wheels.offset_mode = true;
        config.color_wheels.midtones.hue = 40.0; // Yellow-orange
        config.color_wheels.midtones.saturation = 1.5;
        config.color_wheels.midtones.luminance = 0.0;

        // Lift blacks for faded look
        config.lgg.lift = RgbColor::new(0.08, 0.06, 0.03);
        config.enable_lgg = true;

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_to_hsl_and_back() {
        let rgb = RgbColor::new(0.5, 0.3, 0.7);
        let hsl = rgb.to_hsl();
        let rgb2 = hsl.to_rgb();

        assert!((rgb.r - rgb2.r).abs() < 0.01);
        assert!((rgb.g - rgb2.g).abs() < 0.01);
        assert!((rgb.b - rgb2.b).abs() < 0.01);
    }

    #[test]
    fn test_lift_gamma_gain() {
        let lgg = LiftGammaGain::new()
            .with_lift(0.1, 0.0, 0.0)
            .with_gamma(1.2, 1.0, 1.0)
            .with_gain(1.1, 1.0, 1.0);

        let color = RgbColor::new(0.5, 0.5, 0.5);
        let result = lgg.apply(color);

        // Lift should increase shadows
        assert!(result.r > color.r);
    }

    #[test]
    fn test_curve_evaluation() {
        let curve = Curve::linear();
        assert!((curve.evaluate(0.0) - 0.0).abs() < 0.01);
        assert!((curve.evaluate(0.5) - 0.5).abs() < 0.01);
        assert!((curve.evaluate(1.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_hsl_qualifier() {
        let qualifier = HslQualifier::new()
            .with_hue_range(0.0, 60.0)
            .with_feather(10.0);

        // Red color (hue ~0)
        let red = RgbColor::new(1.0, 0.0, 0.0);
        let mask_red = qualifier.calculate_mask(red);
        assert!(mask_red > 0.5);

        // Blue color (hue ~240)
        let blue = RgbColor::new(0.0, 0.0, 1.0);
        let mask_blue = qualifier.calculate_mask(blue);
        assert!(mask_blue < 0.5);
    }
}
