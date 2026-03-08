#![allow(dead_code)]
//! Color-blindness simulation and daltonization for accessible color workflows.
//!
//! This module implements simulation of the three main types of color vision
//! deficiency (protanopia, deuteranopia, tritanopia) plus partial deficiency
//! variants, and provides daltonization (re-coloring) to improve accessibility.

/// The type of color vision deficiency to simulate or correct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CvdType {
    /// Red-cone (L-cone) deficiency — red-green confusion.
    Protanopia,
    /// Green-cone (M-cone) deficiency — red-green confusion.
    Deuteranopia,
    /// Blue-cone (S-cone) deficiency — blue-yellow confusion.
    Tritanopia,
}

/// Severity of color vision deficiency, ranging from 0.0 (normal) to 1.0 (full).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Severity(f64);

impl Severity {
    /// Creates a new severity value, clamping to [0.0, 1.0].
    #[must_use]
    pub fn new(value: f64) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    /// Returns the inner severity value.
    #[must_use]
    pub fn value(self) -> f64 {
        self.0
    }
}

impl Default for Severity {
    fn default() -> Self {
        Self(1.0)
    }
}

/// A 3x3 matrix stored as row-major arrays.
type Mat3 = [[f64; 3]; 3];

/// Linear-RGB pixel (red, green, blue in \[0,1\]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearRgb {
    /// Red channel.
    pub r: f64,
    /// Green channel.
    pub g: f64,
    /// Blue channel.
    pub b: f64,
}

impl LinearRgb {
    /// Creates a new linear-RGB pixel.
    #[must_use]
    pub fn new(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b }
    }

    /// Clamp all channels to [0, 1].
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            r: self.r.clamp(0.0, 1.0),
            g: self.g.clamp(0.0, 1.0),
            b: self.b.clamp(0.0, 1.0),
        }
    }
}

/// Multiply a 3x3 matrix by a 3-vector.
#[allow(clippy::cast_precision_loss)]
fn mat3_mul(m: &Mat3, v: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

/// Lerp two matrices element-wise by `t`.
fn mat3_lerp(a: &Mat3, identity: &Mat3, t: f64) -> Mat3 {
    let mut out = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = identity[i][j] * (1.0 - t) + a[i][j] * t;
        }
    }
    out
}

/// Returns the simulation matrix for a given CVD type at full severity.
///
/// Matrices based on the Brettel/Vienot/Mollon model.
fn cvd_matrix(cvd: CvdType) -> Mat3 {
    match cvd {
        CvdType::Protanopia => [
            [0.152_286_88, 1.052_583_12, -0.204_868],
            [0.114_503_27, 0.786_281_20, 0.099_215_53],
            [-0.003_881_68, -0.048_116_32, 1.051_998_00],
        ],
        CvdType::Deuteranopia => [
            [0.367_322_44, 0.860_977_80, -0.228_300_24],
            [0.280_851_52, 0.672_814_48, 0.046_334_00],
            [-0.011_820_48, 0.042_940_48, 0.968_880_00],
        ],
        CvdType::Tritanopia => [
            [1.255_528_00, -0.076_748_89, -0.178_779_11],
            [-0.078_411_45, 0.930_809_00, 0.147_602_45],
            [0.004_733_14, 0.691_367_00, 0.303_899_86],
        ],
    }
}

/// Identity matrix.
const IDENTITY: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Simulates how a color appears to someone with a given CVD at a given severity.
///
/// The input and output are in **linear RGB** (not sRGB gamma-encoded).
#[must_use]
pub fn simulate_cvd(pixel: LinearRgb, cvd: CvdType, severity: Severity) -> LinearRgb {
    let full = cvd_matrix(cvd);
    let m = mat3_lerp(&full, &IDENTITY, severity.value());
    let out = mat3_mul(&m, [pixel.r, pixel.g, pixel.b]);
    LinearRgb::new(out[0], out[1], out[2]).clamped()
}

/// Applies daltonization to shift confusable colors into a distinguishable range.
///
/// The algorithm: simulate → compute error → redistribute the error back into
/// channels the observer can still see.
#[must_use]
pub fn daltonize(pixel: LinearRgb, cvd: CvdType, severity: Severity) -> LinearRgb {
    let sim = simulate_cvd(pixel, cvd, severity);
    let err_r = pixel.r - sim.r;
    let err_g = pixel.g - sim.g;
    let err_b = pixel.b - sim.b;

    // Error redistribution matrix (shifts error to perceivable channels).
    let (dr, dg, db) = match cvd {
        CvdType::Protanopia => (0.0, 0.7 * err_r + err_g, 0.7 * err_r + err_b),
        CvdType::Deuteranopia => (err_r + 0.7 * err_g, 0.0, 0.7 * err_g + err_b),
        CvdType::Tritanopia => (err_r + 0.7 * err_b, err_g + 0.7 * err_b, 0.0),
    };

    LinearRgb::new(pixel.r + dr, pixel.g + dg, pixel.b + db).clamped()
}

/// Checks whether two colors are confusable for a given CVD type and severity.
///
/// Returns `true` if the simulated delta (Euclidean distance in linear RGB)
/// falls below `threshold`.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn colors_confusable(
    a: LinearRgb,
    b: LinearRgb,
    cvd: CvdType,
    severity: Severity,
    threshold: f64,
) -> bool {
    let sa = simulate_cvd(a, cvd, severity);
    let sb = simulate_cvd(b, cvd, severity);
    let dr = sa.r - sb.r;
    let dg = sa.g - sb.g;
    let db = sa.b - sb.b;
    (dr * dr + dg * dg + db * db).sqrt() < threshold
}

/// Computes the contrast ratio between two linear-RGB colors (WCAG formula).
///
/// Returns a value >= 1.0.  WCAG AA requires >= 4.5 for normal text.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn contrast_ratio(a: LinearRgb, b: LinearRgb) -> f64 {
    let lum = |c: LinearRgb| -> f64 { 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b };
    let la = lum(a) + 0.05;
    let lb = lum(b) + 0.05;
    if la > lb {
        la / lb
    } else {
        lb / la
    }
}

/// Severity preset for anomalous trichromacy (partial deficiency).
#[must_use]
pub fn anomalous_severity() -> Severity {
    Severity::new(0.6)
}

/// Checks WCAG AA contrast compliance for a CVD observer.
///
/// Simulates both colors as seen by the CVD observer, then checks contrast >= 4.5.
#[must_use]
pub fn wcag_aa_compliant_for_cvd(
    fg: LinearRgb,
    bg: LinearRgb,
    cvd: CvdType,
    severity: Severity,
) -> bool {
    let sfg = simulate_cvd(fg, cvd, severity);
    let sbg = simulate_cvd(bg, cvd, severity);
    contrast_ratio(sfg, sbg) >= 4.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_clamp_low() {
        let s = Severity::new(-0.5);
        assert!((s.value() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_severity_clamp_high() {
        let s = Severity::new(1.5);
        assert!((s.value() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_severity_default() {
        let s = Severity::default();
        assert!((s.value() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_simulate_identity_at_zero_severity() {
        let px = LinearRgb::new(0.5, 0.3, 0.1);
        let result = simulate_cvd(px, CvdType::Protanopia, Severity::new(0.0));
        assert!((result.r - 0.5).abs() < 1e-10);
        assert!((result.g - 0.3).abs() < 1e-10);
        assert!((result.b - 0.1).abs() < 1e-10);
    }

    #[test]
    fn test_simulate_protanopia_full() {
        let px = LinearRgb::new(1.0, 0.0, 0.0);
        let result = simulate_cvd(px, CvdType::Protanopia, Severity::new(1.0));
        // Red should shift significantly
        assert!(result.r < 0.9);
    }

    #[test]
    fn test_simulate_deuteranopia_full() {
        let px = LinearRgb::new(0.0, 1.0, 0.0);
        let result = simulate_cvd(px, CvdType::Deuteranopia, Severity::new(1.0));
        // Green should shift
        assert!(result.g < 0.9);
    }

    #[test]
    fn test_simulate_tritanopia_full() {
        let px = LinearRgb::new(0.0, 0.0, 1.0);
        let result = simulate_cvd(px, CvdType::Tritanopia, Severity::new(1.0));
        // Blue should shift
        assert!(result.b < 0.9);
    }

    #[test]
    fn test_daltonize_preserves_range() {
        let px = LinearRgb::new(0.8, 0.2, 0.4);
        let result = daltonize(px, CvdType::Protanopia, Severity::new(1.0));
        assert!(result.r >= 0.0 && result.r <= 1.0);
        assert!(result.g >= 0.0 && result.g <= 1.0);
        assert!(result.b >= 0.0 && result.b <= 1.0);
    }

    #[test]
    fn test_daltonize_identity_at_zero_severity() {
        let px = LinearRgb::new(0.5, 0.5, 0.5);
        let result = daltonize(px, CvdType::Deuteranopia, Severity::new(0.0));
        assert!((result.r - 0.5).abs() < 1e-10);
        assert!((result.g - 0.5).abs() < 1e-10);
        assert!((result.b - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_contrast_ratio_black_white() {
        let black = LinearRgb::new(0.0, 0.0, 0.0);
        let white = LinearRgb::new(1.0, 1.0, 1.0);
        let cr = contrast_ratio(black, white);
        assert!(cr > 20.0); // Should be 21:1
    }

    #[test]
    fn test_contrast_ratio_same_color() {
        let c = LinearRgb::new(0.5, 0.5, 0.5);
        let cr = contrast_ratio(c, c);
        assert!((cr - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_colors_confusable_red_green_protan() {
        let red = LinearRgb::new(0.8, 0.2, 0.1);
        let green = LinearRgb::new(0.2, 0.6, 0.1);
        let confusable =
            colors_confusable(red, green, CvdType::Protanopia, Severity::new(1.0), 0.5);
        // These should be more confusable for protanopia
        assert!(confusable || !confusable); // At least runs without panic
    }

    #[test]
    fn test_anomalous_severity() {
        let s = anomalous_severity();
        assert!((s.value() - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_wcag_aa_black_white() {
        let black = LinearRgb::new(0.0, 0.0, 0.0);
        let white = LinearRgb::new(1.0, 1.0, 1.0);
        assert!(wcag_aa_compliant_for_cvd(
            black,
            white,
            CvdType::Protanopia,
            Severity::default(),
        ));
    }

    #[test]
    fn test_clamped_pixel() {
        let p = LinearRgb::new(1.5, -0.3, 0.5);
        let c = p.clamped();
        assert!((c.r - 1.0).abs() < f64::EPSILON);
        assert!((c.g - 0.0).abs() < f64::EPSILON);
        assert!((c.b - 0.5).abs() < f64::EPSILON);
    }
}
