#![allow(dead_code)]
//! Spectral upsampling from RGB to a spectral representation.
//!
//! Converts tri-stimulus RGB values into a sampled spectral power distribution
//! (SPD) using Smits-style basis functions. This is useful for physically-based
//! rendering and accurate color mixing under different illuminants.

/// Number of spectral bands in a sampled SPD (380 nm to 720 nm in 10 nm steps).
pub const NUM_BANDS: usize = 35;

/// Start wavelength in nanometres.
pub const LAMBDA_MIN: f64 = 380.0;

/// End wavelength in nanometres.
pub const LAMBDA_MAX: f64 = 720.0;

/// Step size between bands in nanometres.
pub const LAMBDA_STEP: f64 = 10.0;

/// A sampled spectral power distribution (SPD).
#[derive(Debug, Clone, PartialEq)]
pub struct SpectralDistribution {
    /// Power values at each band from `LAMBDA_MIN` to `LAMBDA_MAX`.
    pub bands: [f64; NUM_BANDS],
}

impl SpectralDistribution {
    /// Creates a new SPD with all bands set to zero.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            bands: [0.0; NUM_BANDS],
        }
    }

    /// Creates a new SPD with all bands set to the given constant value.
    #[must_use]
    pub fn constant(value: f64) -> Self {
        Self {
            bands: [value; NUM_BANDS],
        }
    }

    /// Returns the wavelength in nm for a given band index.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn wavelength_at(index: usize) -> f64 {
        LAMBDA_MIN + index as f64 * LAMBDA_STEP
    }

    /// Looks up the power at the nearest band for a given wavelength.
    ///
    /// # Arguments
    /// * `wavelength_nm` - Wavelength in nanometres
    ///
    /// # Returns
    /// Power value, or 0.0 if out of range.
    #[must_use]
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn at_wavelength(&self, wavelength_nm: f64) -> f64 {
        if wavelength_nm < LAMBDA_MIN || wavelength_nm > LAMBDA_MAX {
            return 0.0;
        }
        let idx = ((wavelength_nm - LAMBDA_MIN) / LAMBDA_STEP).round() as usize;
        if idx < NUM_BANDS {
            self.bands[idx]
        } else {
            0.0
        }
    }

    /// Scales all bands by a constant factor.
    pub fn scale(&mut self, factor: f64) {
        for b in &mut self.bands {
            *b *= factor;
        }
    }

    /// Adds another SPD to this one (element-wise).
    pub fn add(&mut self, other: &Self) {
        for (a, b) in self.bands.iter_mut().zip(other.bands.iter()) {
            *a += *b;
        }
    }

    /// Multiplies element-wise with another SPD.
    pub fn multiply(&mut self, other: &Self) {
        for (a, b) in self.bands.iter_mut().zip(other.bands.iter()) {
            *a *= *b;
        }
    }

    /// Integrates the SPD using the trapezoidal rule.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn integrate(&self) -> f64 {
        if NUM_BANDS < 2 {
            return 0.0;
        }
        let mut sum = 0.0;
        for i in 0..NUM_BANDS - 1 {
            sum += (self.bands[i] + self.bands[i + 1]) * 0.5 * LAMBDA_STEP;
        }
        sum
    }

    /// Returns the peak wavelength (wavelength with maximum power).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn peak_wavelength(&self) -> f64 {
        let mut max_idx = 0;
        let mut max_val = self.bands[0];
        for (i, &v) in self.bands.iter().enumerate() {
            if v > max_val {
                max_val = v;
                max_idx = i;
            }
        }
        Self::wavelength_at(max_idx)
    }
}

impl Default for SpectralDistribution {
    fn default() -> Self {
        Self::zero()
    }
}

/// Smits-style basis functions for spectral upsampling.
///
/// Each basis function covers a portion of the visible spectrum with smooth
/// transitions to produce a plausible SPD from RGB values.
#[allow(clippy::cast_precision_loss)]
fn red_basis() -> SpectralDistribution {
    let mut spd = SpectralDistribution::zero();
    for i in 0..NUM_BANDS {
        let wl = SpectralDistribution::wavelength_at(i);
        spd.bands[i] = if wl >= 600.0 {
            1.0
        } else if wl >= 560.0 {
            (wl - 560.0) / 40.0
        } else {
            0.0
        };
    }
    spd
}

#[allow(clippy::cast_precision_loss)]
fn green_basis() -> SpectralDistribution {
    let mut spd = SpectralDistribution::zero();
    for i in 0..NUM_BANDS {
        let wl = SpectralDistribution::wavelength_at(i);
        spd.bands[i] = if wl >= 480.0 && wl <= 600.0 {
            let center = 540.0;
            let half_width = 60.0;
            let d = (wl - center).abs();
            if d <= half_width {
                1.0
            } else {
                (1.0 - (d - half_width) / 30.0).max(0.0)
            }
        } else {
            0.0
        };
    }
    spd
}

#[allow(clippy::cast_precision_loss)]
fn blue_basis() -> SpectralDistribution {
    let mut spd = SpectralDistribution::zero();
    for i in 0..NUM_BANDS {
        let wl = SpectralDistribution::wavelength_at(i);
        spd.bands[i] = if wl <= 480.0 {
            1.0
        } else if wl <= 520.0 {
            1.0 - (wl - 480.0) / 40.0
        } else {
            0.0
        };
    }
    spd
}

/// Upsamples an RGB triple to a spectral power distribution.
///
/// Uses Smits-style additive basis functions. The input RGB is assumed to be
/// in linear light, non-negative.
///
/// # Arguments
/// * `rgb` - `[R, G, B]` in linear light (0..1 typical range, clamped to >= 0)
///
/// # Returns
/// A [`SpectralDistribution`] approximating the input color.
#[must_use]
pub fn rgb_to_spectral(rgb: [f64; 3]) -> SpectralDistribution {
    let r = rgb[0].max(0.0);
    let g = rgb[1].max(0.0);
    let b = rgb[2].max(0.0);

    let r_basis = red_basis();
    let g_basis = green_basis();
    let b_basis = blue_basis();

    let mut result = SpectralDistribution::zero();
    for i in 0..NUM_BANDS {
        result.bands[i] = r * r_basis.bands[i] + g * g_basis.bands[i] + b * b_basis.bands[i];
    }
    result
}

/// CIE 1931 2-degree observer x-bar approximation.
#[allow(clippy::cast_precision_loss)]
fn cie_xbar(wavelength: f64) -> f64 {
    let t1 = (wavelength - 442.0) * (if wavelength < 442.0 { 0.0624 } else { 0.0374 });
    let t2 = (wavelength - 599.8) * (if wavelength < 599.8 { 0.0264 } else { 0.0323 });
    let t3 = (wavelength - 501.1) * (if wavelength < 501.1 { 0.0490 } else { 0.0382 });
    0.362 * (-0.5 * t1 * t1).exp() + 1.056 * (-0.5 * t2 * t2).exp()
        - 0.065 * (-0.5 * t3 * t3).exp()
}

/// CIE 1931 2-degree observer y-bar approximation.
#[allow(clippy::cast_precision_loss)]
fn cie_ybar(wavelength: f64) -> f64 {
    let t1 = (wavelength - 568.8) * (if wavelength < 568.8 { 0.0213 } else { 0.0247 });
    let t2 = (wavelength - 530.9) * (if wavelength < 530.9 { 0.0613 } else { 0.0322 });
    0.821 * (-0.5 * t1 * t1).exp() + 0.286 * (-0.5 * t2 * t2).exp()
}

/// CIE 1931 2-degree observer z-bar approximation.
#[allow(clippy::cast_precision_loss)]
fn cie_zbar(wavelength: f64) -> f64 {
    let t1 = (wavelength - 437.0) * (if wavelength < 437.0 { 0.0845 } else { 0.0278 });
    let t2 = (wavelength - 459.0) * (if wavelength < 459.0 { 0.0385 } else { 0.0725 });
    1.217 * (-0.5 * t1 * t1).exp() + 0.681 * (-0.5 * t2 * t2).exp()
}

/// Converts an SPD to CIE XYZ tri-stimulus values.
///
/// Uses a simple summation with the CIE 1931 2-degree observer approximation.
///
/// # Arguments
/// * `spd` - The spectral power distribution to convert
///
/// # Returns
/// `[X, Y, Z]` tri-stimulus values.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn spectral_to_xyz(spd: &SpectralDistribution) -> [f64; 3] {
    let mut x = 0.0;
    let mut y = 0.0;
    let mut z = 0.0;
    for i in 0..NUM_BANDS {
        let wl = SpectralDistribution::wavelength_at(i);
        let power = spd.bands[i];
        x += power * cie_xbar(wl) * LAMBDA_STEP;
        y += power * cie_ybar(wl) * LAMBDA_STEP;
        z += power * cie_zbar(wl) * LAMBDA_STEP;
    }
    // Normalise by the integral of y_bar for equal-energy illuminant
    let mut y_norm = 0.0;
    for i in 0..NUM_BANDS {
        let wl = SpectralDistribution::wavelength_at(i);
        y_norm += cie_ybar(wl) * LAMBDA_STEP;
    }
    if y_norm > 1e-12 {
        x /= y_norm;
        y /= y_norm;
        z /= y_norm;
    }
    [x, y, z]
}

/// D65 illuminant SPD (normalised, simplified).
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn d65_illuminant() -> SpectralDistribution {
    // Simplified D65 approximation: relatively flat with slight blue emphasis
    let mut spd = SpectralDistribution::zero();
    for i in 0..NUM_BANDS {
        let wl = SpectralDistribution::wavelength_at(i);
        // Simple model: peak near 460nm, secondary peak near 560nm
        let t1 = ((wl - 460.0) / 30.0).powi(2);
        let t2 = ((wl - 560.0) / 50.0).powi(2);
        spd.bands[i] = 0.7 + 0.3 * (-0.5 * t1).exp() + 0.15 * (-0.5 * t2).exp();
    }
    spd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spd_zero() {
        let spd = SpectralDistribution::zero();
        for &v in &spd.bands {
            assert!((v).abs() < 1e-15);
        }
    }

    #[test]
    fn test_spd_constant() {
        let spd = SpectralDistribution::constant(0.5);
        for &v in &spd.bands {
            assert!((v - 0.5).abs() < 1e-15);
        }
    }

    #[test]
    fn test_wavelength_at() {
        assert!((SpectralDistribution::wavelength_at(0) - 380.0).abs() < 1e-9);
        assert!((SpectralDistribution::wavelength_at(34) - 720.0).abs() < 1e-9);
    }

    #[test]
    fn test_at_wavelength_in_range() {
        let spd = SpectralDistribution::constant(1.0);
        assert!((spd.at_wavelength(550.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_at_wavelength_out_of_range() {
        let spd = SpectralDistribution::constant(1.0);
        assert!((spd.at_wavelength(300.0)).abs() < 1e-12);
        assert!((spd.at_wavelength(800.0)).abs() < 1e-12);
    }

    #[test]
    fn test_scale() {
        let mut spd = SpectralDistribution::constant(2.0);
        spd.scale(0.5);
        for &v in &spd.bands {
            assert!((v - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn test_add() {
        let mut a = SpectralDistribution::constant(1.0);
        let b = SpectralDistribution::constant(2.0);
        a.add(&b);
        for &v in &a.bands {
            assert!((v - 3.0).abs() < 1e-12);
        }
    }

    #[test]
    fn test_multiply() {
        let mut a = SpectralDistribution::constant(3.0);
        let b = SpectralDistribution::constant(2.0);
        a.multiply(&b);
        for &v in &a.bands {
            assert!((v - 6.0).abs() < 1e-12);
        }
    }

    #[test]
    fn test_integrate_constant() {
        let spd = SpectralDistribution::constant(1.0);
        let area = spd.integrate();
        // 34 intervals of 10nm each, constant 1.0 => 340
        let expected = (NUM_BANDS - 1) as f64 * LAMBDA_STEP;
        assert!(
            (area - expected).abs() < 1e-6,
            "Expected {expected}, got {area}"
        );
    }

    #[test]
    fn test_peak_wavelength() {
        let mut spd = SpectralDistribution::zero();
        // Set a single peak at band 10 (480 nm)
        spd.bands[10] = 5.0;
        assert!((spd.peak_wavelength() - 480.0).abs() < 1e-9);
    }

    #[test]
    fn test_rgb_to_spectral_black() {
        let spd = rgb_to_spectral([0.0, 0.0, 0.0]);
        for &v in &spd.bands {
            assert!(v.abs() < 1e-12);
        }
    }

    #[test]
    fn test_rgb_to_spectral_white_non_negative() {
        let spd = rgb_to_spectral([1.0, 1.0, 1.0]);
        for &v in &spd.bands {
            assert!(v >= 0.0, "Negative band value: {v}");
        }
    }

    #[test]
    fn test_spectral_to_xyz_white() {
        let spd = SpectralDistribution::constant(1.0);
        let xyz = spectral_to_xyz(&spd);
        // Y should be ~1.0 for a flat spectrum normalised by y_bar integral
        assert!(
            (xyz[1] - 1.0).abs() < 0.15,
            "Y should be near 1.0 for flat spectrum, got {}",
            xyz[1]
        );
    }

    #[test]
    fn test_d65_illuminant_positive() {
        let d65 = d65_illuminant();
        for &v in &d65.bands {
            assert!(v > 0.0, "D65 should be positive everywhere");
        }
    }

    #[test]
    fn test_red_upsampling_peaks_long_wavelength() {
        let spd = rgb_to_spectral([1.0, 0.0, 0.0]);
        let peak = spd.peak_wavelength();
        assert!(
            peak >= 600.0,
            "Red peak should be >= 600nm, got {peak}"
        );
    }
}
