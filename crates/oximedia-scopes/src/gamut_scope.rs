#![allow(dead_code)]
//! Color gamut scope for visualizing how pixel colors map within a target gamut.
//!
//! This module renders a 2-D chromaticity-based gamut scope that plots each
//! pixel's color onto a CIE xy (or u'v') diagram with the gamut triangle
//! overlay for Rec.709, Rec.2020, or DCI-P3. Pixels falling outside the
//! target gamut are highlighted in a warning color.

/// Target color gamut for the scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetGamut {
    /// ITU-R BT.709 (standard HD).
    Rec709,
    /// ITU-R BT.2020 (UHD / HDR).
    Rec2020,
    /// DCI-P3 (Digital Cinema).
    DciP3,
    /// sRGB (identical primaries to Rec.709 but different transfer).
    Srgb,
}

/// Diagram type used for the gamut scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramKind {
    /// CIE 1931 xy chromaticity diagram.
    CieXy,
    /// CIE 1976 u'v' uniform chromaticity scale diagram.
    CieUpVp,
}

/// Configuration for the gamut scope renderer.
#[derive(Debug, Clone)]
pub struct GamutScopeConfig {
    /// Output scope image width in pixels.
    pub width: u32,
    /// Output scope image height in pixels.
    pub height: u32,
    /// Target gamut to display.
    pub target_gamut: TargetGamut,
    /// Diagram kind (CIE xy or u'v').
    pub diagram_kind: DiagramKind,
    /// Whether to draw the gamut triangle boundary.
    pub show_gamut_triangle: bool,
    /// Whether to highlight out-of-gamut pixels.
    pub highlight_out_of_gamut: bool,
    /// Intensity of pixel dots on the scope (0.0 - 1.0).
    pub dot_intensity: f64,
    /// Whether to draw the spectral locus outline.
    pub show_spectral_locus: bool,
    /// Background brightness (0 = black, 255 = white).
    pub background_level: u8,
}

impl Default for GamutScopeConfig {
    fn default() -> Self {
        Self {
            width: 512,
            height: 512,
            target_gamut: TargetGamut::Rec709,
            diagram_kind: DiagramKind::CieXy,
            show_gamut_triangle: true,
            highlight_out_of_gamut: true,
            dot_intensity: 0.8,
            show_spectral_locus: true,
            background_level: 16,
        }
    }
}

/// CIE xy chromaticity coordinate.
#[derive(Debug, Clone, Copy)]
pub struct ChromaXy {
    /// x coordinate.
    pub x: f64,
    /// y coordinate.
    pub y: f64,
}

impl ChromaXy {
    /// Create a new chromaticity coordinate.
    #[must_use]
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A gamut triangle defined by three primary chromaticities and a white point.
#[derive(Debug, Clone, Copy)]
pub struct GamutTriangle {
    /// Red primary.
    pub red: ChromaXy,
    /// Green primary.
    pub green: ChromaXy,
    /// Blue primary.
    pub blue: ChromaXy,
    /// White point (D65 typically).
    pub white: ChromaXy,
}

impl GamutTriangle {
    /// Get the Rec.709 gamut triangle.
    #[must_use]
    pub fn rec709() -> Self {
        Self {
            red: ChromaXy::new(0.64, 0.33),
            green: ChromaXy::new(0.30, 0.60),
            blue: ChromaXy::new(0.15, 0.06),
            white: ChromaXy::new(0.3127, 0.3290),
        }
    }

    /// Get the Rec.2020 gamut triangle.
    #[must_use]
    pub fn rec2020() -> Self {
        Self {
            red: ChromaXy::new(0.708, 0.292),
            green: ChromaXy::new(0.170, 0.797),
            blue: ChromaXy::new(0.131, 0.046),
            white: ChromaXy::new(0.3127, 0.3290),
        }
    }

    /// Get the DCI-P3 gamut triangle.
    #[must_use]
    pub fn dci_p3() -> Self {
        Self {
            red: ChromaXy::new(0.680, 0.320),
            green: ChromaXy::new(0.265, 0.690),
            blue: ChromaXy::new(0.150, 0.060),
            white: ChromaXy::new(0.314, 0.351),
        }
    }

    /// Get the triangle for a given target gamut.
    #[must_use]
    pub fn for_gamut(gamut: TargetGamut) -> Self {
        match gamut {
            TargetGamut::Rec709 | TargetGamut::Srgb => Self::rec709(),
            TargetGamut::Rec2020 => Self::rec2020(),
            TargetGamut::DciP3 => Self::dci_p3(),
        }
    }

    /// Test whether a chromaticity point is inside this gamut triangle
    /// using barycentric coordinates.
    #[must_use]
    pub fn contains(&self, p: &ChromaXy) -> bool {
        let d1 = sign(p, &self.red, &self.green);
        let d2 = sign(p, &self.green, &self.blue);
        let d3 = sign(p, &self.blue, &self.red);

        let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
        let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);

        !(has_neg && has_pos)
    }

    /// Compute the area of this gamut triangle (in xy units).
    #[must_use]
    pub fn area(&self) -> f64 {
        0.5 * ((self.green.x - self.red.x) * (self.blue.y - self.red.y)
            - (self.blue.x - self.red.x) * (self.green.y - self.red.y))
            .abs()
    }
}

/// Helper for triangle containment test (2-D cross product sign).
fn sign(p: &ChromaXy, a: &ChromaXy, b: &ChromaXy) -> f64 {
    (p.x - b.x) * (a.y - b.y) - (a.x - b.x) * (p.y - b.y)
}

/// Convert linear RGB (0.0 - 1.0) to CIE XYZ using the Rec.709 matrix.
#[must_use]
pub fn linear_rgb_to_xyz(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let x = 0.4124564 * r + 0.3575761 * g + 0.1804375 * b;
    let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
    let z = 0.0193339 * r + 0.1191920 * g + 0.9503041 * b;
    (x, y, z)
}

/// Convert CIE XYZ to CIE xy chromaticity.
#[must_use]
pub fn xyz_to_xy(x: f64, y: f64, z: f64) -> ChromaXy {
    let sum = x + y + z;
    if sum <= 0.0 {
        return ChromaXy::new(0.3127, 0.3290); // D65 white for black pixels
    }
    ChromaXy::new(x / sum, y / sum)
}

/// Convert CIE XYZ to CIE 1976 u'v' coordinates.
#[must_use]
pub fn xyz_to_upvp(x: f64, y: f64, z: f64) -> (f64, f64) {
    let denom = x + 15.0 * y + 3.0 * z;
    if denom <= 0.0 {
        return (0.1978, 0.4683); // D65
    }
    let up = 4.0 * x / denom;
    let vp = 9.0 * y / denom;
    (up, vp)
}

/// Remove sRGB gamma to get linear light.
#[must_use]
pub fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Result of analyzing a frame against a gamut.
#[derive(Debug, Clone)]
pub struct GamutAnalysis {
    /// Total number of pixels analyzed.
    pub total_pixels: u64,
    /// Number of pixels outside the target gamut.
    pub out_of_gamut_pixels: u64,
    /// Percentage of pixels outside gamut (0.0 - 100.0).
    pub out_of_gamut_pct: f64,
    /// Average chromaticity x.
    pub avg_x: f64,
    /// Average chromaticity y.
    pub avg_y: f64,
}

/// Analyze an RGB24 frame against a target gamut.
///
/// * `frame` — RGB24 pixel data (3 bytes per pixel: R, G, B)
/// * `width` — frame width
/// * `height` — frame height
/// * `gamut` — target gamut to test against
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn analyze_gamut(frame: &[u8], width: u32, height: u32, gamut: TargetGamut) -> GamutAnalysis {
    let triangle = GamutTriangle::for_gamut(gamut);
    let num_pixels = (width as u64) * (height as u64);
    let expected_len = num_pixels as usize * 3;

    if frame.len() < expected_len || num_pixels == 0 {
        return GamutAnalysis {
            total_pixels: num_pixels,
            out_of_gamut_pixels: 0,
            out_of_gamut_pct: 0.0,
            avg_x: 0.3127,
            avg_y: 0.3290,
        };
    }

    let mut out_count = 0_u64;
    let mut sum_x = 0.0_f64;
    let mut sum_y = 0.0_f64;

    for i in 0..num_pixels as usize {
        let r_lin = srgb_to_linear(f64::from(frame[i * 3]) / 255.0);
        let g_lin = srgb_to_linear(f64::from(frame[i * 3 + 1]) / 255.0);
        let b_lin = srgb_to_linear(f64::from(frame[i * 3 + 2]) / 255.0);

        let (x, y, z) = linear_rgb_to_xyz(r_lin, g_lin, b_lin);
        let chroma = xyz_to_xy(x, y, z);

        sum_x += chroma.x;
        sum_y += chroma.y;

        if !triangle.contains(&chroma) {
            out_count += 1;
        }
    }

    let n = num_pixels as f64;
    GamutAnalysis {
        total_pixels: num_pixels,
        out_of_gamut_pixels: out_count,
        out_of_gamut_pct: if num_pixels > 0 {
            (out_count as f64 / n) * 100.0
        } else {
            0.0
        },
        avg_x: sum_x / n,
        avg_y: sum_y / n,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gamut_triangle_rec709() {
        let t = GamutTriangle::rec709();
        assert!((t.red.x - 0.64).abs() < 1e-6);
        assert!((t.green.y - 0.60).abs() < 1e-6);
    }

    #[test]
    fn test_gamut_triangle_rec2020() {
        let t = GamutTriangle::rec2020();
        assert!(t.area() > GamutTriangle::rec709().area());
    }

    #[test]
    fn test_gamut_triangle_contains_white() {
        let t = GamutTriangle::rec709();
        assert!(t.contains(&t.white));
    }

    #[test]
    fn test_gamut_triangle_contains_interior() {
        let t = GamutTriangle::rec709();
        // Midpoint of triangle should be inside
        let mid = ChromaXy::new(
            (t.red.x + t.green.x + t.blue.x) / 3.0,
            (t.red.y + t.green.y + t.blue.y) / 3.0,
        );
        assert!(t.contains(&mid));
    }

    #[test]
    fn test_gamut_triangle_outside() {
        let t = GamutTriangle::rec709();
        let outside = ChromaXy::new(0.0, 0.0);
        assert!(!t.contains(&outside));
    }

    #[test]
    fn test_gamut_area_positive() {
        let t = GamutTriangle::rec709();
        assert!(t.area() > 0.0);
    }

    #[test]
    fn test_linear_rgb_to_xyz_black() {
        let (x, y, z) = linear_rgb_to_xyz(0.0, 0.0, 0.0);
        assert!(x.abs() < 1e-10);
        assert!(y.abs() < 1e-10);
        assert!(z.abs() < 1e-10);
    }

    #[test]
    fn test_xyz_to_xy_d65_white() {
        let (x, y, z) = linear_rgb_to_xyz(1.0, 1.0, 1.0);
        let chroma = xyz_to_xy(x, y, z);
        // Should be near D65 white point
        assert!((chroma.x - 0.3127).abs() < 0.01);
        assert!((chroma.y - 0.3290).abs() < 0.01);
    }

    #[test]
    fn test_xyz_to_xy_black_returns_d65() {
        let chroma = xyz_to_xy(0.0, 0.0, 0.0);
        assert!((chroma.x - 0.3127).abs() < 1e-6);
    }

    #[test]
    fn test_srgb_to_linear_zero() {
        assert!(srgb_to_linear(0.0).abs() < 1e-10);
    }

    #[test]
    fn test_srgb_to_linear_one() {
        assert!((srgb_to_linear(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_xyz_to_upvp_d65() {
        let (x, y, z) = linear_rgb_to_xyz(1.0, 1.0, 1.0);
        let (up, vp) = xyz_to_upvp(x, y, z);
        assert!((up - 0.1978).abs() < 0.01);
        assert!((vp - 0.4683).abs() < 0.01);
    }

    #[test]
    fn test_analyze_gamut_all_black() {
        // 2x2 black frame — all pixels should be in-gamut (mapped to D65)
        let frame = vec![0u8; 2 * 2 * 3];
        let result = analyze_gamut(&frame, 2, 2, TargetGamut::Rec709);
        assert_eq!(result.total_pixels, 4);
        // D65 is inside Rec709
        assert_eq!(result.out_of_gamut_pixels, 0);
    }

    #[test]
    fn test_analyze_gamut_pure_red() {
        // 1x1 pure red pixel
        let frame = vec![255, 0, 0];
        let result = analyze_gamut(&frame, 1, 1, TargetGamut::Rec709);
        assert_eq!(result.total_pixels, 1);
        // Pure Rec.709 red should be on the gamut boundary
    }

    #[test]
    fn test_gamut_scope_config_default() {
        let cfg = GamutScopeConfig::default();
        assert_eq!(cfg.width, 512);
        assert_eq!(cfg.target_gamut, TargetGamut::Rec709);
        assert!(cfg.show_gamut_triangle);
    }

    #[test]
    fn test_for_gamut_srgb_equals_rec709() {
        let srgb = GamutTriangle::for_gamut(TargetGamut::Srgb);
        let r709 = GamutTriangle::for_gamut(TargetGamut::Rec709);
        assert!((srgb.red.x - r709.red.x).abs() < 1e-10);
    }
}
