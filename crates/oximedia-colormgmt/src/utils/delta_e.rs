//! Color difference (Delta E) calculations.

use crate::xyz::Lab;

/// Calculates CIE Delta E 1976 (CIE76).
///
/// Simple Euclidean distance in Lab* space.
///
/// # Arguments
///
/// * `lab1` - First color
/// * `lab2` - Second color
///
/// # Returns
///
/// Delta E value. < 1 imperceptible, 1-2 just noticeable, 2-10 noticeable, > 10 very different.
#[must_use]
pub fn cie76(lab1: &Lab, lab2: &Lab) -> f64 {
    let dl = lab1.l - lab2.l;
    let da = lab1.a - lab2.a;
    let db = lab1.b - lab2.b;
    (dl * dl + da * da + db * db).sqrt()
}

/// Calculates CIE Delta E 1994 (CIE94).
///
/// Improved formula accounting for perceptual non-uniformity.
///
/// # Arguments
///
/// * `lab1` - First color (reference)
/// * `lab2` - Second color (sample)
/// * `kl` - Weighting factor for lightness (typically 1.0)
/// * `kc` - Weighting factor for chroma (typically 1.0)
/// * `kh` - Weighting factor for hue (typically 1.0)
#[must_use]
pub fn cie94(lab1: &Lab, lab2: &Lab, kl: f64, kc: f64, kh: f64) -> f64 {
    let k1 = 0.045;
    let k2 = 0.015;

    let dl = lab1.l - lab2.l;
    let da = lab1.a - lab2.a;
    let db = lab1.b - lab2.b;

    let c1 = (lab1.a * lab1.a + lab1.b * lab1.b).sqrt();
    let c2 = (lab2.a * lab2.a + lab2.b * lab2.b).sqrt();
    let dc = c1 - c2;

    let dh_sq = da * da + db * db - dc * dc;
    let dh = if dh_sq > 0.0 { dh_sq.sqrt() } else { 0.0 };

    let sl = 1.0;
    let sc = 1.0 + k1 * c1;
    let sh = 1.0 + k2 * c1;

    let term_l = dl / (kl * sl);
    let term_c = dc / (kc * sc);
    let term_h = dh / (kh * sh);

    (term_l * term_l + term_c * term_c + term_h * term_h).sqrt()
}

/// Calculates CIE Delta E 2000 (CIEDE2000).
///
/// Most sophisticated formula, industry standard for color difference.
///
/// # Arguments
///
/// * `lab1` - First color
/// * `lab2` - Second color
#[must_use]
pub fn cie2000(lab1: &Lab, lab2: &Lab) -> f64 {
    crate::delta_e::delta_e_2000(lab1, lab2)
}

/// Calculates CMC l:c Delta E.
///
/// Used in textile industry.
///
/// # Arguments
///
/// * `lab1` - First color (reference)
/// * `lab2` - Second color (sample)
/// * `l` - Lightness factor (typically 2.0 for acceptability, 1.0 for perceptibility)
/// * `c` - Chroma factor (typically 1.0)
#[must_use]
pub fn cmc(lab1: &Lab, lab2: &Lab, l: f64, c: f64) -> f64 {
    let dl = lab1.l - lab2.l;
    let da = lab1.a - lab2.a;
    let db = lab1.b - lab2.b;

    let c1 = (lab1.a * lab1.a + lab1.b * lab1.b).sqrt();
    let c2 = (lab2.a * lab2.a + lab2.b * lab2.b).sqrt();
    let dc = c1 - c2;

    let dh_sq = da * da + db * db - dc * dc;
    let dh = if dh_sq > 0.0 { dh_sq.sqrt() } else { 0.0 };

    let h1 = lab1.b.atan2(lab1.a).to_degrees();
    let h1 = if h1 < 0.0 { h1 + 360.0 } else { h1 };

    let sl = if lab1.l < 16.0 {
        0.511
    } else {
        (0.040975 * lab1.l) / (1.0 + 0.01765 * lab1.l)
    };

    let sc = (0.0638 * c1) / (1.0 + 0.0131 * c1) + 0.638;

    let f = (c1.powi(4)) / ((c1.powi(4)) + 1900.0).sqrt();

    let t = if (164.0..=345.0).contains(&h1) {
        0.56 + (0.2 * ((h1 + 168.0) * std::f64::consts::PI / 180.0).cos()).abs()
    } else {
        0.36 + (0.4 * ((h1 + 35.0) * std::f64::consts::PI / 180.0).cos()).abs()
    };

    let sh = sc * (f * t + 1.0 - f);

    ((dl / (l * sl)).powi(2) + (dc / (c * sc)).powi(2) + (dh / sh).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cie76_same_color() {
        let lab = Lab::new(50.0, 10.0, 20.0);
        assert_eq!(cie76(&lab, &lab), 0.0);
    }

    #[test]
    fn test_cie76_different_colors() {
        let lab1 = Lab::new(50.0, 10.0, 20.0);
        let lab2 = Lab::new(60.0, 10.0, 20.0);
        assert!((cie76(&lab1, &lab2) - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_cie94() {
        let lab1 = Lab::new(50.0, 10.0, 20.0);
        let lab2 = Lab::new(50.0, 10.0, 20.0);
        assert_eq!(cie94(&lab1, &lab2, 1.0, 1.0, 1.0), 0.0);
    }

    #[test]
    fn test_cmc() {
        let lab1 = Lab::new(50.0, 10.0, 20.0);
        let lab2 = Lab::new(50.0, 10.0, 20.0);
        assert_eq!(cmc(&lab1, &lab2, 2.0, 1.0), 0.0);
    }
}
