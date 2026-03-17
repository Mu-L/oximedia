//! Color metadata: primaries, transfer characteristics, and matrix coefficients.
//!
//! These enumerations follow the definitions in ITU-T H.273 (ISO/IEC 23091-2)
//! and are used to describe the colour encoding of video frames independent of
//! the pixel storage format.

#![allow(dead_code)]

/// Colour primaries (ITU-T H.273 / ISO 23091-2, Table 2).
///
/// Defines the chromaticity of the red, green, and blue primaries and the
/// white point used by the source capture system or display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum ColorPrimaries {
    /// ITU-R BT.709-6 (HDTV, sRGB). H.273 value 1.
    Bt709,
    /// Unspecified / unknown primaries. H.273 value 2.
    Unspecified,
    /// ITU-R BT.470-6 System M (NTSC). H.273 value 4.
    Bt470M,
    /// ITU-R BT.601-7 625-line (PAL/SECAM). H.273 value 5.
    Bt601_625,
    /// ITU-R BT.601-7 525-line (NTSC). H.273 value 6.
    Bt601_525,
    /// SMPTE ST 240 (1999). H.273 value 7.
    Smpte240,
    /// Generic film (Illuminant C). H.273 value 8.
    GenericFilm,
    /// ITU-R BT.2020-2 (UHDTV / HDR). H.273 value 9.
    Bt2020,
    /// SMPTE ST 428-1 / CIE 1931 XYZ. H.273 value 10.
    Xyz,
    /// SMPTE RP 431-2 (DCI-P3 D65). H.273 value 11.
    DciP3,
    /// Display P3 (Apple, D65 white point). H.273 value 12.
    DisplayP3,
    /// EBU Tech 3213-E (European broadcast). H.273 value 22.
    EbuTech3213,
}

impl Default for ColorPrimaries {
    fn default() -> Self {
        Self::Unspecified
    }
}

impl ColorPrimaries {
    /// Returns the H.273 numeric code for this primaries value.
    #[must_use]
    pub const fn h273_code(self) -> u8 {
        match self {
            Self::Bt709 => 1,
            Self::Unspecified => 2,
            Self::Bt470M => 4,
            Self::Bt601_625 => 5,
            Self::Bt601_525 => 6,
            Self::Smpte240 => 7,
            Self::GenericFilm => 8,
            Self::Bt2020 => 9,
            Self::Xyz => 10,
            Self::DciP3 => 11,
            Self::DisplayP3 => 12,
            Self::EbuTech3213 => 22,
        }
    }

    /// Creates a `ColorPrimaries` from an H.273 numeric code.
    ///
    /// Returns `None` for reserved or unrecognised codes.
    #[must_use]
    pub const fn from_h273_code(code: u8) -> Option<Self> {
        Some(match code {
            1 => Self::Bt709,
            2 => Self::Unspecified,
            4 => Self::Bt470M,
            5 => Self::Bt601_625,
            6 => Self::Bt601_525,
            7 => Self::Smpte240,
            8 => Self::GenericFilm,
            9 => Self::Bt2020,
            10 => Self::Xyz,
            11 => Self::DciP3,
            12 => Self::DisplayP3,
            22 => Self::EbuTech3213,
            _ => return None,
        })
    }

    /// Returns `true` for wide-colour-gamut primaries (BT.2020, DCI-P3, Display P3).
    #[must_use]
    pub const fn is_wide_gamut(self) -> bool {
        matches!(self, Self::Bt2020 | Self::DciP3 | Self::DisplayP3)
    }

    /// Returns the standard name string (lowercase).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bt709 => "bt709",
            Self::Unspecified => "unspecified",
            Self::Bt470M => "bt470m",
            Self::Bt601_625 => "bt601-625",
            Self::Bt601_525 => "bt601-525",
            Self::Smpte240 => "smpte240",
            Self::GenericFilm => "generic-film",
            Self::Bt2020 => "bt2020",
            Self::Xyz => "xyz",
            Self::DciP3 => "dci-p3",
            Self::DisplayP3 => "display-p3",
            Self::EbuTech3213 => "ebu-tech-3213",
        }
    }
}

impl std::fmt::Display for ColorPrimaries {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Matrix coefficients
// ─────────────────────────────────────────────────────────────────────────────

/// Matrix coefficients describing the YCbCr ↔ RGB conversion (ITU-T H.273, Table 4).
///
/// These coefficients define how luma and chroma values are derived from
/// (or converted back to) linear RGB tristimulus values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum MatrixCoefficients {
    /// Identity (full-range RGB; GBR ordering in containers). H.273 value 0.
    Identity,
    /// ITU-R BT.709 (HDTV). H.273 value 1.
    Bt709,
    /// Unspecified. H.273 value 2.
    Unspecified,
    /// FCC Title 47 CFR 73.682 (a)(20) (US NTSC). H.273 value 4.
    Fcc,
    /// ITU-R BT.601-7 (625-line, PAL). H.273 value 5.
    Bt601_625,
    /// ITU-R BT.601-7 (525-line, NTSC). H.273 value 6.
    Bt601_525,
    /// SMPTE ST 240 (1999). H.273 value 7.
    Smpte240,
    /// YCoCg (Luma + Colour-difference). H.273 value 8.
    YCoCg,
    /// ITU-R BT.2020 non-constant luminance. H.273 value 9.
    Bt2020Ncl,
    /// ITU-R BT.2020 constant luminance. H.273 value 10.
    Bt2020Cl,
    /// SMPTE ST 2085 (Y′D′ZD′X). H.273 value 11.
    Smpte2085,
    /// Chromaticity-derived non-constant luminance. H.273 value 12.
    ChromaDerivedNcl,
    /// Chromaticity-derived constant luminance. H.273 value 13.
    ChromaDerivedCl,
    /// ICtCp (ITU-R BT.2100, Dolby ICtCp). H.273 value 14.
    ICtCp,
}

impl Default for MatrixCoefficients {
    fn default() -> Self {
        Self::Unspecified
    }
}

impl MatrixCoefficients {
    /// Returns the H.273 numeric code for this matrix.
    #[must_use]
    pub const fn h273_code(self) -> u8 {
        match self {
            Self::Identity => 0,
            Self::Bt709 => 1,
            Self::Unspecified => 2,
            Self::Fcc => 4,
            Self::Bt601_625 => 5,
            Self::Bt601_525 => 6,
            Self::Smpte240 => 7,
            Self::YCoCg => 8,
            Self::Bt2020Ncl => 9,
            Self::Bt2020Cl => 10,
            Self::Smpte2085 => 11,
            Self::ChromaDerivedNcl => 12,
            Self::ChromaDerivedCl => 13,
            Self::ICtCp => 14,
        }
    }

    /// Creates a `MatrixCoefficients` from an H.273 numeric code.
    ///
    /// Returns `None` for reserved or unrecognised codes.
    #[must_use]
    pub const fn from_h273_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => Self::Identity,
            1 => Self::Bt709,
            2 => Self::Unspecified,
            4 => Self::Fcc,
            5 => Self::Bt601_625,
            6 => Self::Bt601_525,
            7 => Self::Smpte240,
            8 => Self::YCoCg,
            9 => Self::Bt2020Ncl,
            10 => Self::Bt2020Cl,
            11 => Self::Smpte2085,
            12 => Self::ChromaDerivedNcl,
            13 => Self::ChromaDerivedCl,
            14 => Self::ICtCp,
            _ => return None,
        })
    }

    /// Returns `true` if this matrix is intended for HDR content
    /// (BT.2020 variants, ICtCp, Smpte2085).
    #[must_use]
    pub const fn is_hdr_matrix(self) -> bool {
        matches!(
            self,
            Self::Bt2020Ncl | Self::Bt2020Cl | Self::ICtCp | Self::Smpte2085
        )
    }

    /// Returns the Kb and Kr coefficients used in the YCbCr conversion
    /// matrix, as `(kb, kr)` f64 pairs.  Returns `None` for matrices that
    /// do not use the standard Kb/Kr formulation (Identity, YCoCg, etc.).
    #[must_use]
    pub fn kr_kb(self) -> Option<(f64, f64)> {
        match self {
            Self::Bt709 => Some((0.0722, 0.2126)),
            Self::Bt601_625 | Self::Fcc => Some((0.114, 0.299)),
            Self::Bt601_525 | Self::Smpte240 => Some((0.114, 0.299)),
            Self::Bt2020Ncl | Self::Bt2020Cl => Some((0.0593, 0.2627)),
            _ => None,
        }
    }
}

impl std::fmt::Display for MatrixCoefficients {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Identity => "identity",
            Self::Bt709 => "bt709",
            Self::Unspecified => "unspecified",
            Self::Fcc => "fcc",
            Self::Bt601_625 => "bt601-625",
            Self::Bt601_525 => "bt601-525",
            Self::Smpte240 => "smpte240",
            Self::YCoCg => "ycocg",
            Self::Bt2020Ncl => "bt2020-ncl",
            Self::Bt2020Cl => "bt2020-cl",
            Self::Smpte2085 => "smpte2085",
            Self::ChromaDerivedNcl => "chroma-derived-ncl",
            Self::ChromaDerivedCl => "chroma-derived-cl",
            Self::ICtCp => "ictcp",
        };
        f.write_str(name)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Colour space descriptor
// ─────────────────────────────────────────────────────────────────────────────

/// Full colour space descriptor combining primaries and matrix coefficients.
///
/// Attach this to a frame or stream to fully characterise its colour encoding.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub struct ColorSpace {
    /// Chromaticity of the display primaries.
    pub primaries: ColorPrimaries,
    /// Matrix coefficients for YCbCr ↔ RGB conversion.
    pub matrix: MatrixCoefficients,
    /// Whether the signal uses full range (0–255) or limited/studio range (16–235/240).
    pub full_range: bool,
}

impl ColorSpace {
    /// Constructs a new `ColorSpace` descriptor.
    #[must_use]
    pub const fn new(
        primaries: ColorPrimaries,
        matrix: MatrixCoefficients,
        full_range: bool,
    ) -> Self {
        Self {
            primaries,
            matrix,
            full_range,
        }
    }

    /// Returns the canonical BT.709 (sRGB / HDTV) colour space with limited range.
    ///
    /// This is the default for most HD content.
    #[must_use]
    pub const fn bt709() -> Self {
        Self::new(ColorPrimaries::Bt709, MatrixCoefficients::Bt709, false)
    }

    /// Returns the BT.2020 (UHDTV / HDR) colour space with limited range.
    #[must_use]
    pub const fn bt2020() -> Self {
        Self::new(ColorPrimaries::Bt2020, MatrixCoefficients::Bt2020Ncl, false)
    }

    /// Returns the BT.601 (SD PAL) colour space with limited range.
    #[must_use]
    pub const fn bt601_625() -> Self {
        Self::new(
            ColorPrimaries::Bt601_625,
            MatrixCoefficients::Bt601_625,
            false,
        )
    }

    /// Returns the BT.601 (SD NTSC) colour space with limited range.
    #[must_use]
    pub const fn bt601_525() -> Self {
        Self::new(
            ColorPrimaries::Bt601_525,
            MatrixCoefficients::Bt601_525,
            false,
        )
    }

    /// Returns the sRGB colour space (BT.709 primaries, identity matrix, full range).
    ///
    /// Suitable for web / desktop image output.
    #[must_use]
    pub const fn srgb() -> Self {
        Self::new(ColorPrimaries::Bt709, MatrixCoefficients::Identity, true)
    }

    /// Returns `true` if this colour space is HDR-capable.
    #[must_use]
    pub fn is_hdr(&self) -> bool {
        self.primaries.is_wide_gamut() || self.matrix.is_hdr_matrix()
    }
}

impl std::fmt::Display for ColorSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "primaries={} matrix={} range={}",
            self.primaries,
            self.matrix,
            if self.full_range { "full" } else { "limited" }
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_primaries_h273_round_trip() {
        let cases = [
            ColorPrimaries::Bt709,
            ColorPrimaries::Unspecified,
            ColorPrimaries::Bt470M,
            ColorPrimaries::Bt601_625,
            ColorPrimaries::Bt601_525,
            ColorPrimaries::Smpte240,
            ColorPrimaries::Bt2020,
            ColorPrimaries::DciP3,
            ColorPrimaries::DisplayP3,
        ];
        for p in cases {
            let code = p.h273_code();
            let decoded = ColorPrimaries::from_h273_code(code)
                .unwrap_or_else(|| panic!("failed to decode code {code} for {p:?}"));
            assert_eq!(decoded, p, "round-trip failed for {p:?}");
        }
    }

    #[test]
    fn test_color_primaries_unknown_code() {
        assert!(ColorPrimaries::from_h273_code(255).is_none());
        assert!(ColorPrimaries::from_h273_code(3).is_none());
    }

    #[test]
    fn test_color_primaries_wide_gamut() {
        assert!(ColorPrimaries::Bt2020.is_wide_gamut());
        assert!(ColorPrimaries::DciP3.is_wide_gamut());
        assert!(ColorPrimaries::DisplayP3.is_wide_gamut());
        assert!(!ColorPrimaries::Bt709.is_wide_gamut());
        assert!(!ColorPrimaries::Bt601_625.is_wide_gamut());
    }

    #[test]
    fn test_color_primaries_display() {
        assert_eq!(format!("{}", ColorPrimaries::Bt709), "bt709");
        assert_eq!(format!("{}", ColorPrimaries::Bt2020), "bt2020");
        assert_eq!(format!("{}", ColorPrimaries::DciP3), "dci-p3");
    }

    #[test]
    fn test_matrix_coefficients_h273_round_trip() {
        let cases = [
            MatrixCoefficients::Identity,
            MatrixCoefficients::Bt709,
            MatrixCoefficients::Unspecified,
            MatrixCoefficients::Bt601_625,
            MatrixCoefficients::Bt601_525,
            MatrixCoefficients::Bt2020Ncl,
            MatrixCoefficients::Bt2020Cl,
            MatrixCoefficients::ICtCp,
        ];
        for m in cases {
            let code = m.h273_code();
            let decoded = MatrixCoefficients::from_h273_code(code)
                .unwrap_or_else(|| panic!("failed to decode code {code} for {m:?}"));
            assert_eq!(decoded, m, "round-trip failed for {m:?}");
        }
    }

    #[test]
    fn test_matrix_unknown_code() {
        assert!(MatrixCoefficients::from_h273_code(200).is_none());
    }

    #[test]
    fn test_matrix_hdr() {
        assert!(MatrixCoefficients::Bt2020Ncl.is_hdr_matrix());
        assert!(MatrixCoefficients::Bt2020Cl.is_hdr_matrix());
        assert!(MatrixCoefficients::ICtCp.is_hdr_matrix());
        assert!(!MatrixCoefficients::Bt709.is_hdr_matrix());
        assert!(!MatrixCoefficients::Bt601_625.is_hdr_matrix());
    }

    #[test]
    fn test_matrix_kr_kb() {
        let (kb, kr) = MatrixCoefficients::Bt709
            .kr_kb()
            .expect("should have kr/kb");
        assert!((kb - 0.0722).abs() < 1e-6);
        assert!((kr - 0.2126).abs() < 1e-6);

        let (kb2020, kr2020) = MatrixCoefficients::Bt2020Ncl
            .kr_kb()
            .expect("should have kr/kb");
        assert!((kb2020 - 0.0593).abs() < 1e-6);
        assert!((kr2020 - 0.2627).abs() < 1e-6);

        assert!(MatrixCoefficients::Identity.kr_kb().is_none());
        assert!(MatrixCoefficients::YCoCg.kr_kb().is_none());
    }

    #[test]
    fn test_color_space_presets() {
        let bt709 = ColorSpace::bt709();
        assert_eq!(bt709.primaries, ColorPrimaries::Bt709);
        assert_eq!(bt709.matrix, MatrixCoefficients::Bt709);
        assert!(!bt709.full_range);
        assert!(!bt709.is_hdr());

        let bt2020 = ColorSpace::bt2020();
        assert!(bt2020.is_hdr());
        assert!(bt2020.primaries.is_wide_gamut());

        let srgb = ColorSpace::srgb();
        assert!(srgb.full_range);
        assert_eq!(srgb.matrix, MatrixCoefficients::Identity);
    }

    #[test]
    fn test_color_space_display() {
        let cs = ColorSpace::bt709();
        let s = format!("{cs}");
        assert!(s.contains("bt709"));
        assert!(s.contains("limited"));
    }

    #[test]
    fn test_color_space_default() {
        let cs = ColorSpace::default();
        assert_eq!(cs.primaries, ColorPrimaries::Unspecified);
        assert_eq!(cs.matrix, MatrixCoefficients::Unspecified);
        assert!(!cs.full_range);
    }

    #[test]
    fn test_bt601_presets() {
        let pal = ColorSpace::bt601_625();
        assert_eq!(pal.primaries, ColorPrimaries::Bt601_625);

        let ntsc = ColorSpace::bt601_525();
        assert_eq!(ntsc.primaries, ColorPrimaries::Bt601_525);
    }
}
