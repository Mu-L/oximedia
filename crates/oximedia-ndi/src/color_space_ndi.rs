#![allow(dead_code)]
//! NDI color-space metadata and conversion helpers for `oximedia-ndi`.
//!
//! NDI streams carry colour-space identifiers that receivers must honour in
//! order to display frames correctly.  This module models color primaries,
//! transfer functions, and matrix coefficients as they appear in the NDI
//! protocol, and provides lightweight conversion matrices between common
//! spaces (BT.601 / BT.709 / BT.2020).

#![allow(clippy::cast_precision_loss)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]

// ---------------------------------------------------------------------------
// ColorPrimaries
// ---------------------------------------------------------------------------

/// Color primaries used by an NDI source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorPrimaries {
    /// ITU-R BT.601 (NTSC / PAL SD).
    Bt601,
    /// ITU-R BT.709 (HD).
    Bt709,
    /// ITU-R BT.2020 (UHD / HDR).
    Bt2020,
    /// DCI-P3 (digital cinema).
    DciP3,
    /// Unknown / unspecified.
    Unknown,
}

impl ColorPrimaries {
    /// Return a human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Bt601 => "BT.601",
            Self::Bt709 => "BT.709",
            Self::Bt2020 => "BT.2020",
            Self::DciP3 => "DCI-P3",
            Self::Unknown => "Unknown",
        }
    }

    /// Attempt to parse from a string tag (case-insensitive).
    pub fn from_tag(tag: &str) -> Self {
        match tag.to_ascii_lowercase().as_str() {
            "bt601" | "601" | "smpte170m" => Self::Bt601,
            "bt709" | "709" => Self::Bt709,
            "bt2020" | "2020" => Self::Bt2020,
            "p3" | "dcip3" | "dci-p3" => Self::DciP3,
            _ => Self::Unknown,
        }
    }

    /// Whether the primaries represent an HDR-capable colour space.
    pub fn is_wide_gamut(self) -> bool {
        matches!(self, Self::Bt2020 | Self::DciP3)
    }
}

// ---------------------------------------------------------------------------
// TransferFunction
// ---------------------------------------------------------------------------

/// Electro-optical transfer function (EOTF / gamma).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransferFunction {
    /// Standard BT.709 gamma (~2.2).
    Bt709,
    /// Perceptual Quantizer (SMPTE ST 2084) — used for HDR10.
    Pq,
    /// Hybrid Log-Gamma (ARIB STD-B67) — used for HLG.
    Hlg,
    /// sRGB transfer (≈ gamma 2.2 with linear toe).
    Srgb,
    /// Linear light.
    Linear,
    /// Unknown / unspecified.
    Unknown,
}

impl TransferFunction {
    /// Return a human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Bt709 => "BT.709",
            Self::Pq => "PQ (ST 2084)",
            Self::Hlg => "HLG",
            Self::Srgb => "sRGB",
            Self::Linear => "Linear",
            Self::Unknown => "Unknown",
        }
    }

    /// Whether this transfer function implies HDR content.
    pub fn is_hdr(self) -> bool {
        matches!(self, Self::Pq | Self::Hlg)
    }
}

// ---------------------------------------------------------------------------
// MatrixCoefficients
// ---------------------------------------------------------------------------

/// YCbCr matrix coefficients used for YUV ↔ RGB conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MatrixCoefficients {
    /// BT.601 (SD).
    Bt601,
    /// BT.709 (HD).
    Bt709,
    /// BT.2020 non-constant luminance.
    Bt2020Ncl,
    /// Identity (RGB is already RGB).
    Identity,
    /// Unknown / unspecified.
    Unknown,
}

impl MatrixCoefficients {
    /// Return a human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Bt601 => "BT.601",
            Self::Bt709 => "BT.709",
            Self::Bt2020Ncl => "BT.2020 NCL",
            Self::Identity => "Identity",
            Self::Unknown => "Unknown",
        }
    }
}

// ---------------------------------------------------------------------------
// NdiColorSpace
// ---------------------------------------------------------------------------

/// Full colour-space description attached to an NDI stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NdiColorSpace {
    /// Colour primaries.
    pub primaries: ColorPrimaries,
    /// Transfer function.
    pub transfer: TransferFunction,
    /// Matrix coefficients (for YUV content).
    pub matrix: MatrixCoefficients,
    /// Whether the source signals full-range (0-255) vs limited (16-235).
    pub full_range: bool,
}

impl Default for NdiColorSpace {
    fn default() -> Self {
        Self {
            primaries: ColorPrimaries::Bt709,
            transfer: TransferFunction::Bt709,
            matrix: MatrixCoefficients::Bt709,
            full_range: false,
        }
    }
}

impl NdiColorSpace {
    /// Standard BT.709 HD colour space (limited range).
    pub fn bt709() -> Self {
        Self::default()
    }

    /// Standard BT.601 SD colour space (limited range).
    pub fn bt601() -> Self {
        Self {
            primaries: ColorPrimaries::Bt601,
            transfer: TransferFunction::Bt709,
            matrix: MatrixCoefficients::Bt601,
            full_range: false,
        }
    }

    /// BT.2020 PQ HDR colour space.
    pub fn bt2020_pq() -> Self {
        Self {
            primaries: ColorPrimaries::Bt2020,
            transfer: TransferFunction::Pq,
            matrix: MatrixCoefficients::Bt2020Ncl,
            full_range: false,
        }
    }

    /// Whether the colour space is HDR.
    pub fn is_hdr(self) -> bool {
        self.transfer.is_hdr()
    }

    /// Whether the colour space uses wide-gamut primaries.
    pub fn is_wide_gamut(self) -> bool {
        self.primaries.is_wide_gamut()
    }

    /// Whether a conversion is needed between two colour spaces.
    pub fn needs_conversion(self, other: Self) -> bool {
        self != other
    }
}

// ---------------------------------------------------------------------------
// ConversionMatrix3x3
// ---------------------------------------------------------------------------

/// A simple 3x3 matrix used for colour-space conversion.
#[derive(Debug, Clone, Copy)]
pub struct ConversionMatrix3x3 {
    /// Row-major 3x3 values.
    pub m: [[f64; 3]; 3],
}

impl ConversionMatrix3x3 {
    /// Identity matrix (no conversion).
    pub fn identity() -> Self {
        Self {
            m: [
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
        }
    }

    /// Multiply a 3-element vector by this matrix.
    pub fn transform(&self, v: [f64; 3]) -> [f64; 3] {
        [
            self.m[0][0] * v[0] + self.m[0][1] * v[1] + self.m[0][2] * v[2],
            self.m[1][0] * v[0] + self.m[1][1] * v[1] + self.m[1][2] * v[2],
            self.m[2][0] * v[0] + self.m[2][1] * v[1] + self.m[2][2] * v[2],
        ]
    }

    /// Compose two matrices (self * other).
    pub fn compose(&self, other: &Self) -> Self {
        let mut out = [[0.0f64; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                out[i][j] = self.m[i][0] * other.m[0][j]
                    + self.m[i][1] * other.m[1][j]
                    + self.m[i][2] * other.m[2][j];
            }
        }
        Self { m: out }
    }

    /// Transpose the matrix.
    pub fn transpose(&self) -> Self {
        Self {
            m: [
                [self.m[0][0], self.m[1][0], self.m[2][0]],
                [self.m[0][1], self.m[1][1], self.m[2][1]],
                [self.m[0][2], self.m[1][2], self.m[2][2]],
            ],
        }
    }

    /// BT.709 YCbCr-to-RGB matrix (limited range).
    pub fn bt709_ycbcr_to_rgb() -> Self {
        Self {
            m: [
                [1.164, 0.000, 1.793],
                [1.164, -0.213, -0.533],
                [1.164, 2.112, 0.000],
            ],
        }
    }

    /// BT.601 YCbCr-to-RGB matrix (limited range).
    pub fn bt601_ycbcr_to_rgb() -> Self {
        Self {
            m: [
                [1.164, 0.000, 1.596],
                [1.164, -0.392, -0.813],
                [1.164, 2.017, 0.000],
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: select conversion matrix
// ---------------------------------------------------------------------------

/// Select the appropriate YCbCr → RGB conversion matrix for a given colour
/// space.
pub fn ycbcr_to_rgb_matrix(cs: &NdiColorSpace) -> ConversionMatrix3x3 {
    match cs.matrix {
        MatrixCoefficients::Bt601 => ConversionMatrix3x3::bt601_ycbcr_to_rgb(),
        MatrixCoefficients::Bt709 => ConversionMatrix3x3::bt709_ycbcr_to_rgb(),
        MatrixCoefficients::Identity => ConversionMatrix3x3::identity(),
        _ => ConversionMatrix3x3::bt709_ycbcr_to_rgb(),
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_primaries_name() {
        assert_eq!(ColorPrimaries::Bt709.name(), "BT.709");
        assert_eq!(ColorPrimaries::Bt2020.name(), "BT.2020");
    }

    #[test]
    fn test_color_primaries_from_tag() {
        assert_eq!(ColorPrimaries::from_tag("bt709"), ColorPrimaries::Bt709);
        assert_eq!(ColorPrimaries::from_tag("2020"), ColorPrimaries::Bt2020);
        assert_eq!(ColorPrimaries::from_tag("P3"), ColorPrimaries::DciP3);
        assert_eq!(ColorPrimaries::from_tag("xyz"), ColorPrimaries::Unknown);
    }

    #[test]
    fn test_wide_gamut() {
        assert!(!ColorPrimaries::Bt709.is_wide_gamut());
        assert!(ColorPrimaries::Bt2020.is_wide_gamut());
        assert!(ColorPrimaries::DciP3.is_wide_gamut());
    }

    #[test]
    fn test_transfer_function_hdr() {
        assert!(!TransferFunction::Bt709.is_hdr());
        assert!(TransferFunction::Pq.is_hdr());
        assert!(TransferFunction::Hlg.is_hdr());
    }

    #[test]
    fn test_default_color_space() {
        let cs = NdiColorSpace::default();
        assert_eq!(cs.primaries, ColorPrimaries::Bt709);
        assert!(!cs.is_hdr());
        assert!(!cs.is_wide_gamut());
    }

    #[test]
    fn test_bt2020_pq() {
        let cs = NdiColorSpace::bt2020_pq();
        assert!(cs.is_hdr());
        assert!(cs.is_wide_gamut());
        assert_eq!(cs.transfer, TransferFunction::Pq);
    }

    #[test]
    fn test_needs_conversion() {
        let a = NdiColorSpace::bt709();
        let b = NdiColorSpace::bt601();
        assert!(a.needs_conversion(b));
        assert!(!a.needs_conversion(a));
    }

    #[test]
    fn test_identity_matrix_transform() {
        let id = ConversionMatrix3x3::identity();
        let v = [0.5, 0.7, 0.3];
        let out = id.transform(v);
        for i in 0..3 {
            assert!((out[i] - v[i]).abs() < 1e-10);
        }
    }

    #[test]
    fn test_matrix_compose_identity() {
        let id = ConversionMatrix3x3::identity();
        let m = ConversionMatrix3x3::bt709_ycbcr_to_rgb();
        let composed = id.compose(&m);
        for i in 0..3 {
            for j in 0..3 {
                assert!((composed.m[i][j] - m.m[i][j]).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn test_matrix_transpose() {
        let m = ConversionMatrix3x3 {
            m: [
                [1.0, 2.0, 3.0],
                [4.0, 5.0, 6.0],
                [7.0, 8.0, 9.0],
            ],
        };
        let t = m.transpose();
        assert!((t.m[0][1] - 4.0).abs() < 1e-10);
        assert!((t.m[1][0] - 2.0).abs() < 1e-10);
        assert!((t.m[2][0] - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_ycbcr_to_rgb_matrix_bt709() {
        let cs = NdiColorSpace::bt709();
        let m = ycbcr_to_rgb_matrix(&cs);
        // First row: Y coefficient should be 1.164
        assert!((m.m[0][0] - 1.164).abs() < 0.001);
    }

    #[test]
    fn test_ycbcr_to_rgb_matrix_bt601() {
        let cs = NdiColorSpace::bt601();
        let m = ycbcr_to_rgb_matrix(&cs);
        assert!((m.m[0][2] - 1.596).abs() < 0.001);
    }

    #[test]
    fn test_matrix_coefficients_name() {
        assert_eq!(MatrixCoefficients::Bt709.name(), "BT.709");
        assert_eq!(MatrixCoefficients::Identity.name(), "Identity");
    }

    #[test]
    fn test_bt601_color_space() {
        let cs = NdiColorSpace::bt601();
        assert_eq!(cs.primaries, ColorPrimaries::Bt601);
        assert_eq!(cs.matrix, MatrixCoefficients::Bt601);
        assert!(!cs.full_range);
    }
}
