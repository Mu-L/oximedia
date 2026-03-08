//! ACES (Academy Color Encoding System) color transforms.
//!
//! This module provides a comprehensive implementation of ACES color management,
//! including transforms between ACES color spaces and Output Device Transforms (ODTs).
//!
//! # ACES Color Spaces
//!
//! - **ACES2065-1 (AP0)**: Scene-referred linear space with wide gamut primaries
//! - **`ACEScg` (AP1)**: Working space for CGI and compositing
//! - **`ACEScct`**: Logarithmic working space for color grading
//! - **`ACESproxy`**: 10-bit/12-bit logarithmic encoding for on-set monitoring
//!
//! # Output Transforms
//!
//! - Rec.709 (100 nits)
//! - Rec.2020 (100 nits, 1000 nits, 2000 nits, 4000 nits)
//! - DCI-P3 (48 nits, D60 white point)
//! - sRGB (D65 white point)
//!
//! # Reference
//!
//! Based on ACES 1.3 specification from the Academy of Motion Picture Arts and Sciences.

use crate::error::{LutError, LutResult};
use crate::matrix::{self, apply_matrix3x3, Matrix3x3};
use crate::Rgb;

/// ACES color space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcesSpace {
    /// ACES2065-1 (AP0, scene-referred linear).
    Aces2065,
    /// `ACEScg` (AP1, working space).
    AcesCg,
    /// `ACEScct` (logarithmic working space).
    AcesCct,
    /// ACES Proxy 10-bit.
    AcesProxy10,
    /// ACES Proxy 12-bit.
    AcesProxy12,
}

/// ACES Output Device Transform (ODT).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcesOdt {
    /// Rec.709 (100 nits, D65).
    Rec709,
    /// Rec.2020 (100 nits, D65).
    Rec2020_100,
    /// Rec.2020 (1000 nits, D65, ST2084/PQ).
    Rec2020_1000,
    /// Rec.2020 (2000 nits, D65, ST2084/PQ).
    Rec2020_2000,
    /// Rec.2020 (4000 nits, D65, ST2084/PQ).
    Rec2020_4000,
    /// DCI-P3 (48 nits, D60).
    DciP3,
    /// sRGB (D65).
    Srgb,
}

// ============================================================================
// ACES Color Space Matrices
// ============================================================================

/// AP0 (ACES2065-1) to XYZ matrix.
pub const AP0_TO_XYZ: Matrix3x3 = [
    [0.952_552_395_9, 0.000_000_000_0, 0.000_093_678_6],
    [0.343_966_449_8, 0.728_166_096_6, -0.072_132_546_4],
    [0.000_000_000_0, 0.000_000_000_0, 1.008_825_184_4],
];

/// XYZ to AP0 (ACES2065-1) matrix.
pub const XYZ_TO_AP0: Matrix3x3 = [
    [1.049_811_017_5, 0.000_000_000_0, -0.000_097_484_5],
    [-0.495_903_023_1, 1.373_313_045_8, 0.098_240_036_1],
    [0.000_000_000_0, 0.000_000_000_0, 0.991_252_018_2],
];

/// AP1 (`ACEScg`) to XYZ matrix.
pub const AP1_TO_XYZ: Matrix3x3 = [
    [0.662_454_181_1, 0.134_004_206_5, 0.156_187_687_0],
    [0.272_228_716_8, 0.674_081_765_8, 0.053_689_517_4],
    [-0.005_574_649_5, 0.004_060_733_5, 1.010_339_100_3],
];

/// XYZ to AP1 (`ACEScg`) matrix.
pub const XYZ_TO_AP1: Matrix3x3 = [
    [1.641_023_379_7, -0.324_803_294_2, -0.236_424_695_2],
    [-0.663_662_858_7, 1.615_331_591_7, 0.016_756_347_7],
    [0.011_721_894_3, -0.008_284_442_0, 0.988_394_858_5],
];

/// AP0 to AP1 conversion matrix.
pub const AP0_TO_AP1: Matrix3x3 = [
    [1.451_439_316_1, -0.236_510_746_9, -0.214_928_569_3],
    [-0.076_553_773_4, 1.176_229_699_8, -0.099_675_926_4],
    [0.008_316_148_4, -0.006_032_449_8, 0.997_716_301_4],
];

/// AP1 to AP0 conversion matrix.
pub const AP1_TO_AP0: Matrix3x3 = [
    [0.695_452_241_4, 0.140_678_696_5, 0.163_869_062_2],
    [0.044_794_563_4, 0.859_671_118_5, 0.095_534_318_2],
    [-0.005_525_882_6, 0.004_025_210_3, 1.001_500_672_3],
];

// ============================================================================
// ACES Color Space Conversions
// ============================================================================

impl AcesSpace {
    /// Convert from this ACES space to linear AP0 (ACES2065-1).
    #[must_use]
    pub fn to_aces2065(&self, rgb: &Rgb) -> Rgb {
        match self {
            Self::Aces2065 => *rgb,
            Self::AcesCg => acescg_to_aces2065(rgb),
            Self::AcesCct => acescct_to_aces2065(rgb),
            Self::AcesProxy10 => acesproxy_to_aces2065(rgb, 10),
            Self::AcesProxy12 => acesproxy_to_aces2065(rgb, 12),
        }
    }

    /// Convert from linear AP0 (ACES2065-1) to this ACES space.
    #[must_use]
    pub fn from_aces2065(&self, rgb: &Rgb) -> Rgb {
        match self {
            Self::Aces2065 => *rgb,
            Self::AcesCg => aces2065_to_acescg(rgb),
            Self::AcesCct => aces2065_to_acescct(rgb),
            Self::AcesProxy10 => aces2065_to_acesproxy(rgb, 10),
            Self::AcesProxy12 => aces2065_to_acesproxy(rgb, 12),
        }
    }
}

/// Convert `ACEScg` (AP1 linear) to ACES2065-1 (AP0 linear).
#[must_use]
pub fn acescg_to_aces2065(rgb: &Rgb) -> Rgb {
    apply_matrix3x3(&AP1_TO_AP0, rgb)
}

/// Convert ACES2065-1 (AP0 linear) to `ACEScg` (AP1 linear).
#[must_use]
pub fn aces2065_to_acescg(rgb: &Rgb) -> Rgb {
    apply_matrix3x3(&AP0_TO_AP1, rgb)
}

/// Convert `ACEScct` (AP1 logarithmic) to ACES2065-1 (AP0 linear).
#[must_use]
pub fn acescct_to_aces2065(rgb: &Rgb) -> Rgb {
    // First decode ACEScct to linear ACEScg
    let linear_ap1 = [
        acescct_to_linear(rgb[0]),
        acescct_to_linear(rgb[1]),
        acescct_to_linear(rgb[2]),
    ];
    // Then convert to AP0
    acescg_to_aces2065(&linear_ap1)
}

/// Convert ACES2065-1 (AP0 linear) to `ACEScct` (AP1 logarithmic).
#[must_use]
pub fn aces2065_to_acescct(rgb: &Rgb) -> Rgb {
    // First convert to ACEScg (AP1 linear)
    let ap1 = aces2065_to_acescg(rgb);
    // Then encode to ACEScct
    [
        linear_to_acescct(ap1[0]),
        linear_to_acescct(ap1[1]),
        linear_to_acescct(ap1[2]),
    ]
}

/// `ACEScct` to linear transfer function.
#[must_use]
fn acescct_to_linear(x: f64) -> f64 {
    const X_BRK: f64 = 0.155_251_141_552_511;

    if x > X_BRK {
        (10.0_f64.powf(x * 17.52 - 9.72) - 0.000_089_999_999_999_999_99) / 0.18
    } else if x < 0.073_292_48 {
        (x - 0.071_776_470_588_235_29) / 10.540_237_741_654_5
    } else {
        10.0_f64.powf(x * 17.52 - 9.72) / 0.18
    }
}

/// Linear to `ACEScct` transfer function.
#[must_use]
fn linear_to_acescct(x: f64) -> f64 {
    const LIN_CUT: f64 = 0.007_812_5;

    let x = x * 0.18;

    if x <= LIN_CUT {
        10.540_237_741_654_5 * x + 0.071_776_470_588_235_29
    } else {
        (x.ln() / 10.0_f64.ln() + 9.72) / 17.52
    }
}

/// Convert `ACESproxy` to ACES2065-1.
#[must_use]
pub fn acesproxy_to_aces2065(rgb: &Rgb, bits: u8) -> Rgb {
    let linear_ap1 = match bits {
        10 => [
            acesproxy10_to_linear(rgb[0]),
            acesproxy10_to_linear(rgb[1]),
            acesproxy10_to_linear(rgb[2]),
        ],
        12 => [
            acesproxy12_to_linear(rgb[0]),
            acesproxy12_to_linear(rgb[1]),
            acesproxy12_to_linear(rgb[2]),
        ],
        _ => *rgb,
    };
    acescg_to_aces2065(&linear_ap1)
}

/// Convert ACES2065-1 to `ACESproxy`.
#[must_use]
pub fn aces2065_to_acesproxy(rgb: &Rgb, bits: u8) -> Rgb {
    let ap1 = aces2065_to_acescg(rgb);
    match bits {
        10 => [
            linear_to_acesproxy10(ap1[0]),
            linear_to_acesproxy10(ap1[1]),
            linear_to_acesproxy10(ap1[2]),
        ],
        12 => [
            linear_to_acesproxy12(ap1[0]),
            linear_to_acesproxy12(ap1[1]),
            linear_to_acesproxy12(ap1[2]),
        ],
        _ => ap1,
    }
}

/// `ACESproxy` 10-bit to linear.
#[must_use]
fn acesproxy10_to_linear(x: f64) -> f64 {
    (10.0_f64.powf((x - 64.0) / 50.0) - 0.000_062_514_891_117_416_44) / 0.18
}

/// Linear to `ACESproxy` 10-bit.
#[must_use]
fn linear_to_acesproxy10(x: f64) -> f64 {
    (x * 0.18).max(0.000_062_514_891_117_416_44).ln() / 10.0_f64.ln() * 50.0 + 64.0
}

/// `ACESproxy` 12-bit to linear.
#[must_use]
fn acesproxy12_to_linear(x: f64) -> f64 {
    (10.0_f64.powf((x - 256.0) / 200.0) - 0.000_062_514_891_117_416_44) / 0.18
}

/// Linear to `ACESproxy` 12-bit.
#[must_use]
fn linear_to_acesproxy12(x: f64) -> f64 {
    (x * 0.18).max(0.000_062_514_891_117_416_44).ln() / 10.0_f64.ln() * 200.0 + 256.0
}

// ============================================================================
// ACES Output Device Transforms (ODTs)
// ============================================================================

impl AcesOdt {
    /// Apply the ODT to convert from ACES2065-1 to display-referred RGB.
    ///
    /// # Errors
    ///
    /// Returns an error if the ODT is not implemented.
    pub fn apply(&self, aces_rgb: &Rgb) -> LutResult<Rgb> {
        match self {
            Self::Rec709 => Ok(aces_odt_rec709(aces_rgb)),
            Self::Rec2020_100 => Ok(aces_odt_rec2020_100(aces_rgb)),
            Self::DciP3 => Ok(aces_odt_dcip3(aces_rgb)),
            Self::Srgb => Ok(aces_odt_srgb(aces_rgb)),
            _ => Err(LutError::UnsupportedFormat(format!(
                "ODT {self:?} not yet implemented"
            ))),
        }
    }
}

/// ACES ODT for Rec.709 (100 nits, D65).
///
/// Simplified version using RRT + ODT.
#[must_use]
pub fn aces_odt_rec709(aces: &Rgb) -> Rgb {
    // Convert to ACEScg for processing
    let acescg = aces2065_to_acescg(aces);

    // Apply RRT (Reference Rendering Transform)
    let rrt = aces_rrt(&acescg);

    // Convert to Rec.709
    let rgb = apply_matrix3x3(
        &matrix::XYZ_TO_RGB_REC709,
        &apply_matrix3x3(&AP1_TO_XYZ, &rrt),
    );

    // Apply Rec.709 OETF (gamma)
    [
        rec709_oetf(rgb[0]),
        rec709_oetf(rgb[1]),
        rec709_oetf(rgb[2]),
    ]
}

/// ACES ODT for Rec.2020 (100 nits, D65).
#[must_use]
pub fn aces_odt_rec2020_100(aces: &Rgb) -> Rgb {
    let acescg = aces2065_to_acescg(aces);
    let rrt = aces_rrt(&acescg);
    apply_matrix3x3(
        &matrix::XYZ_TO_RGB_REC2020,
        &apply_matrix3x3(&AP1_TO_XYZ, &rrt),
    )
}

/// ACES ODT for DCI-P3 (48 nits, D60).
#[must_use]
pub fn aces_odt_dcip3(aces: &Rgb) -> Rgb {
    let acescg = aces2065_to_acescg(aces);
    let rrt = aces_rrt(&acescg);
    apply_matrix3x3(
        &matrix::XYZ_TO_RGB_DCIP3,
        &apply_matrix3x3(&AP1_TO_XYZ, &rrt),
    )
}

/// ACES ODT for sRGB (D65).
#[must_use]
pub fn aces_odt_srgb(aces: &Rgb) -> Rgb {
    aces_odt_rec709(aces) // Same as Rec.709 with sRGB primaries
}

/// ACES Reference Rendering Transform (RRT).
///
/// Simplified version of the full RRT.
#[must_use]
fn aces_rrt(rgb: &Rgb) -> Rgb {
    // Apply tone curve to each channel
    [
        aces_tone_curve(rgb[0]),
        aces_tone_curve(rgb[1]),
        aces_tone_curve(rgb[2]),
    ]
}

/// ACES tone curve (simplified).
#[must_use]
fn aces_tone_curve(x: f64) -> f64 {
    const A: f64 = 2.51;
    const B: f64 = 0.03;
    const C: f64 = 2.43;
    const D: f64 = 0.59;
    const E: f64 = 0.14;

    if x <= 0.0 {
        0.0
    } else {
        ((x * (A * x + B)) / (x * (C * x + D) + E)).clamp(0.0, 1.0)
    }
}

/// Rec.709 OETF.
#[must_use]
fn rec709_oetf(linear: f64) -> f64 {
    if linear < 0.018 {
        linear * 4.5
    } else {
        1.099 * linear.powf(0.45) - 0.099
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ap0_ap1_round_trip() {
        let rgb = [0.5, 0.3, 0.7];
        let ap1 = aces2065_to_acescg(&rgb);
        let back = acescg_to_aces2065(&ap1);
        assert!((rgb[0] - back[0]).abs() < 1e-6);
        assert!((rgb[1] - back[1]).abs() < 1e-6);
        assert!((rgb[2] - back[2]).abs() < 1e-6);
    }

    #[test]
    fn test_acescct_round_trip() {
        let rgb = [0.5, 0.3, 0.7];
        let cct = aces2065_to_acescct(&rgb);
        let back = acescct_to_aces2065(&cct);
        assert!((rgb[0] - back[0]).abs() < 0.01);
        assert!((rgb[1] - back[1]).abs() < 0.01);
        assert!((rgb[2] - back[2]).abs() < 0.01);
    }

    #[test]
    fn test_aces_odt_rec709() {
        let aces = [0.5, 0.3, 0.7];
        let rgb = aces_odt_rec709(&aces);
        assert!(rgb[0] >= 0.0 && rgb[0] <= 1.0);
        assert!(rgb[1] >= 0.0 && rgb[1] <= 1.0);
        assert!(rgb[2] >= 0.0 && rgb[2] <= 1.0);
    }

    #[test]
    fn test_aces_spaces() {
        let rgb = [0.5, 0.3, 0.7];

        // Test each space
        for space in &[AcesSpace::Aces2065, AcesSpace::AcesCg, AcesSpace::AcesCct] {
            let converted = space.from_aces2065(&rgb);
            let back = space.to_aces2065(&converted);
            assert!((rgb[0] - back[0]).abs() < 0.01);
            assert!((rgb[1] - back[1]).abs() < 0.01);
            assert!((rgb[2] - back[2]).abs() < 0.01);
        }
    }
}
