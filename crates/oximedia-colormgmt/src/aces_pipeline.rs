#![allow(dead_code)]
//! ACES (Academy Color Encoding System) pipeline utilities.
//!
//! Provides color space enumerations, transform descriptors, and input-device
//! transforms (IDTs) for building ACES-compliant color pipelines.
//!
//! # Reference
//! - SMPTE ST 2065-1 (ACES2065-1 encoding)
//! - S-2014-006 (ACEScg)
//! - TB-2014-012 (ACES Input Transform methodology)

use std::fmt;

// ─── Color Spaces ────────────────────────────────────────────────────────────

/// Enumerates the principal ACES color encoding spaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AcesColorSpace {
    /// ACES2065-1: scene-linear, AP0 primaries, D60 white point.
    /// Used as the archival interchange color space.
    ACES2065_1,
    /// ACEScg: scene-linear, AP1 primaries, D60 white point.
    /// Preferred working color space for VFX and compositing.
    ACEScg,
    /// ACEScc: logarithmic, AP1 primaries. Designed for color grading.
    ACEScc,
    /// ACEScct: logarithmic with a toe, AP1 primaries. Improved shadow
    /// behaviour compared to ACEScc.
    ACEScct,
    /// ACESproxy: integer log encoding for on-set preview monitors.
    ACESproxy,
}

impl AcesColorSpace {
    /// Returns a short human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::ACES2065_1 => "ACES2065-1",
            Self::ACEScg => "ACEScg",
            Self::ACEScc => "ACEScc",
            Self::ACEScct => "ACEScct",
            Self::ACESproxy => "ACESproxy",
        }
    }

    /// Returns `true` if this encoding is scene-linear (not log).
    #[must_use]
    pub fn is_linear(self) -> bool {
        matches!(self, Self::ACES2065_1 | Self::ACEScg)
    }

    /// Returns `true` if the space uses AP0 primaries (ACES2065-1 only).
    #[must_use]
    pub fn uses_ap0(self) -> bool {
        self == Self::ACES2065_1
    }

    /// Returns `true` if the space uses AP1 primaries.
    #[must_use]
    pub fn uses_ap1(self) -> bool {
        !self.uses_ap0()
    }

    /// Returns the nominal exposure middle-grey value in this encoding.
    #[must_use]
    pub fn middle_grey(self) -> f32 {
        match self {
            Self::ACES2065_1 | Self::ACEScg => 0.18,
            Self::ACEScc => 0.4135884, // log2(0.18)/17.52 + (9.72/17.52) approx
            Self::ACEScct => 0.413,
            Self::ACESproxy => 0.413,
        }
    }
}

impl fmt::Display for AcesColorSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

// ─── AP0 / AP1 matrix coefficients ───────────────────────────────────────────

/// 3 × 3 row-major matrix for converting AP1 (ACEScg) to AP0 (ACES2065-1).
///
/// Source: ACES S-2014-006, Table B.2.
pub const AP1_TO_AP0: [[f32; 3]; 3] = [
    [0.695_452, 0.140_679, 0.163_869],
    [0.044_794, 0.859_671, 0.095_535],
    [-0.005_535, 0.004_062, 1.001_473],
];

/// 3 × 3 row-major matrix for converting AP0 (ACES2065-1) to AP1 (ACEScg).
///
/// Source: ACES S-2014-006, Table B.3.
pub const AP0_TO_AP1: [[f32; 3]; 3] = [
    [1.451_439, -0.236_511, -0.214_929],
    [-0.076_597, 1.176_226, -0.099_629],
    [0.008_332, -0.006_051, 0.997_719],
];

// ─── Matrix helpers ───────────────────────────────────────────────────────────

/// Multiply a 3×3 row-major matrix by a column vector [r, g, b].
#[must_use]
fn mat3_mul(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

// ─── AcesTransform ───────────────────────────────────────────────────────────

/// Describes a single color-space conversion within the ACES system.
///
/// Only linear-to-linear conversions between AP0 and AP1 are currently
/// implemented; non-linear spaces require an additional tone curve step.
#[derive(Debug, Clone, Copy)]
pub struct AcesTransform {
    /// Input color space.
    pub src: AcesColorSpace,
    /// Output color space.
    pub dst: AcesColorSpace,
}

impl AcesTransform {
    /// Create a new `AcesTransform` between `src` and `dst`.
    #[must_use]
    pub const fn new(src: AcesColorSpace, dst: AcesColorSpace) -> Self {
        Self { src, dst }
    }

    /// Returns `true` when source and destination are the same space (identity).
    #[must_use]
    pub fn is_identity(self) -> bool {
        self.src == self.dst
    }

    /// Apply the transform to a linear scene-referred RGB triplet.
    ///
    /// Currently handles ACES2065-1 ↔ ACEScg via the AP0/AP1 matrices.
    /// All other combinations return the input unchanged (identity pass-through).
    #[must_use]
    pub fn apply(self, rgb: [f32; 3]) -> [f32; 3] {
        match (self.src, self.dst) {
            (AcesColorSpace::ACEScg, AcesColorSpace::ACES2065_1) => mat3_mul(&AP1_TO_AP0, rgb),
            (AcesColorSpace::ACES2065_1, AcesColorSpace::ACEScg) => mat3_mul(&AP0_TO_AP1, rgb),
            _ => rgb,
        }
    }
}

// ─── AcesInputTransform ──────────────────────────────────────────────────────

/// Source device class for an Input Device Transform (IDT).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputDeviceClass {
    /// Standard digital cinema camera (e.g. ARRI, RED, Sony Venice).
    DigitalCinema,
    /// Still-camera raw sensor output.
    RawSensor,
    /// Computer-generated imagery (scene-linear, sRGB primaries).
    CgiLinear,
    /// Legacy video (Rec.709 / BT.1886).
    Video709,
}

impl InputDeviceClass {
    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::DigitalCinema => "Digital Cinema",
            Self::RawSensor => "Raw Sensor",
            Self::CgiLinear => "CGI Linear",
            Self::Video709 => "Video Rec.709",
        }
    }
}

/// An Input Device Transform that converts camera-native footage to ACEScg.
///
/// In a full ACES pipeline the IDT maps raw or log-encoded footage from a
/// specific camera into the ACES scene-linear working space (ACEScg).
#[derive(Debug, Clone)]
pub struct AcesInputTransform {
    /// Descriptive name for the camera / device this IDT targets.
    pub device_name: String,
    /// Class of source device.
    pub device_class: InputDeviceClass,
    /// 3 × 3 input matrix applied before the transfer function (row-major).
    pub input_matrix: [[f32; 3]; 3],
    /// Whether the input data is already in a linear state (no OETF reversal).
    pub input_is_linear: bool,
    /// Exposure compensation applied as a linear gain after the IDT.
    pub exposure_compensation: f32,
}

impl AcesInputTransform {
    /// Create an IDT configured for scene-linear CGI input (identity matrix).
    #[must_use]
    pub fn cgi_linear() -> Self {
        Self {
            device_name: "CGI Linear (sRGB primaries)".to_owned(),
            device_class: InputDeviceClass::CgiLinear,
            input_matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            input_is_linear: true,
            exposure_compensation: 1.0,
        }
    }

    /// Create an IDT for generic Rec.709 video.
    #[must_use]
    pub fn video_rec709() -> Self {
        // Simplified: Rec.709 → AP1 matrix (approximate)
        Self {
            device_name: "Rec.709 Video".to_owned(),
            device_class: InputDeviceClass::Video709,
            input_matrix: [
                [0.613_117, 0.339_496, 0.047_387],
                [0.070_093, 0.916_378, 0.013_529],
                [0.020_535, 0.109_453, 0.870_012],
            ],
            input_is_linear: false,
            exposure_compensation: 1.0,
        }
    }

    /// Apply this IDT to a camera-native RGB triplet.
    ///
    /// Steps:
    /// 1. Apply `input_matrix`.
    /// 2. Apply `exposure_compensation` gain.
    ///
    /// (OETF reversal / log decode is outside the scope of this simplified IDT.)
    #[must_use]
    pub fn apply(&self, rgb: [f32; 3]) -> [f32; 3] {
        let [r, g, b] = mat3_mul(&self.input_matrix, rgb);
        [
            r * self.exposure_compensation,
            g * self.exposure_compensation,
            b * self.exposure_compensation,
        ]
    }

    /// Set the exposure compensation and return `self` for chaining.
    #[must_use]
    pub fn with_exposure(mut self, stops: f32) -> Self {
        self.exposure_compensation = 2.0_f32.powf(stops);
        self
    }

    /// Returns `true` when the IDT is purely an identity (unit matrix, no gain).
    #[must_use]
    pub fn is_identity(&self) -> bool {
        let id = [[1.0_f32, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        self.input_matrix == id && (self.exposure_compensation - 1.0).abs() < 1e-6
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── AcesColorSpace ──

    #[test]
    fn test_label_non_empty() {
        for cs in [
            AcesColorSpace::ACES2065_1,
            AcesColorSpace::ACEScg,
            AcesColorSpace::ACEScc,
            AcesColorSpace::ACEScct,
            AcesColorSpace::ACESproxy,
        ] {
            assert!(!cs.label().is_empty(), "label empty for {cs:?}");
        }
    }

    #[test]
    fn test_linear_spaces() {
        assert!(AcesColorSpace::ACES2065_1.is_linear());
        assert!(AcesColorSpace::ACEScg.is_linear());
        assert!(!AcesColorSpace::ACEScc.is_linear());
        assert!(!AcesColorSpace::ACEScct.is_linear());
    }

    #[test]
    fn test_primaries_ap0_ap1() {
        assert!(AcesColorSpace::ACES2065_1.uses_ap0());
        assert!(!AcesColorSpace::ACES2065_1.uses_ap1());
        assert!(AcesColorSpace::ACEScg.uses_ap1());
        assert!(!AcesColorSpace::ACEScg.uses_ap0());
    }

    #[test]
    fn test_middle_grey_linear() {
        let mg = AcesColorSpace::ACEScg.middle_grey();
        assert!((mg - 0.18).abs() < 1e-5);
    }

    #[test]
    fn test_display_trait() {
        let s = format!("{}", AcesColorSpace::ACEScg);
        assert_eq!(s, "ACEScg");
    }

    // ── AcesTransform ──

    #[test]
    fn test_identity_transform() {
        let t = AcesTransform::new(AcesColorSpace::ACEScg, AcesColorSpace::ACEScg);
        assert!(t.is_identity());
        let rgb = [0.18, 0.18, 0.18];
        let out = t.apply(rgb);
        assert!((out[0] - 0.18).abs() < 1e-6);
    }

    #[test]
    fn test_roundtrip_ap1_ap0() {
        let to_ap0 = AcesTransform::new(AcesColorSpace::ACEScg, AcesColorSpace::ACES2065_1);
        let to_ap1 = AcesTransform::new(AcesColorSpace::ACES2065_1, AcesColorSpace::ACEScg);
        let rgb = [0.5, 0.3, 0.2];
        let out = to_ap1.apply(to_ap0.apply(rgb));
        assert!(
            (out[0] - rgb[0]).abs() < 1e-3,
            "R round-trip error too large"
        );
        assert!(
            (out[1] - rgb[1]).abs() < 1e-3,
            "G round-trip error too large"
        );
        assert!(
            (out[2] - rgb[2]).abs() < 1e-3,
            "B round-trip error too large"
        );
    }

    #[test]
    fn test_ap1_to_ap0_non_identity() {
        let t = AcesTransform::new(AcesColorSpace::ACEScg, AcesColorSpace::ACES2065_1);
        assert!(!t.is_identity());
        let rgb = [1.0, 0.0, 0.0];
        let out = t.apply(rgb);
        // Pure red in AP1 maps to a different tristimulus in AP0
        assert!((out[0] - 1.0).abs() > 1e-3, "should differ from identity");
    }

    #[test]
    fn test_unknown_pair_passthrough() {
        let t = AcesTransform::new(AcesColorSpace::ACEScc, AcesColorSpace::ACEScct);
        let rgb = [0.4, 0.4, 0.4];
        let out = t.apply(rgb);
        assert!((out[0] - 0.4).abs() < 1e-9);
    }

    // ── AcesInputTransform ──

    #[test]
    fn test_cgi_linear_is_identity() {
        let idt = AcesInputTransform::cgi_linear();
        assert!(idt.is_identity());
    }

    #[test]
    fn test_cgi_linear_apply_passthrough() {
        let idt = AcesInputTransform::cgi_linear();
        let rgb = [0.3, 0.5, 0.8];
        let out = idt.apply(rgb);
        assert!((out[0] - 0.3).abs() < 1e-6);
        assert!((out[1] - 0.5).abs() < 1e-6);
        assert!((out[2] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_rec709_is_not_identity() {
        let idt = AcesInputTransform::video_rec709();
        assert!(!idt.is_identity());
    }

    #[test]
    fn test_with_exposure_positive_stop() {
        let idt = AcesInputTransform::cgi_linear().with_exposure(1.0);
        // +1 stop → gain = 2.0
        assert!((idt.exposure_compensation - 2.0).abs() < 1e-5);
        assert!(!idt.is_identity());
    }

    #[test]
    fn test_with_exposure_zero_stop() {
        let idt = AcesInputTransform::cgi_linear().with_exposure(0.0);
        assert!((idt.exposure_compensation - 1.0).abs() < 1e-5);
        assert!(idt.is_identity());
    }

    #[test]
    fn test_input_device_label_non_empty() {
        for cls in [
            InputDeviceClass::DigitalCinema,
            InputDeviceClass::RawSensor,
            InputDeviceClass::CgiLinear,
            InputDeviceClass::Video709,
        ] {
            assert!(!cls.label().is_empty());
        }
    }

    #[test]
    fn test_device_name_stored() {
        let idt = AcesInputTransform::cgi_linear();
        assert!(!idt.device_name.is_empty());
    }
}
