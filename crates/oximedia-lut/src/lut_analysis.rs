//! LUT analysis utilities.
//!
//! Provides:
//! * Gamut coverage analysis – what fraction of the LUT output lies within a
//!   specified gamut box.
//! * Clipping analysis – detection of lattice points that clip to the boundary.
//! * Colorimetric shift measurement – mean and peak ΔE-style deviation from
//!   an identity (or reference) LUT.
//! * Statistical summary of LUT outputs (min, max, mean, variance per channel).

use crate::Rgb;

// ---------------------------------------------------------------------------
// Channel statistics
// ---------------------------------------------------------------------------

/// Per-channel statistics for LUT output values.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ChannelStats {
    /// Minimum value across all lattice points.
    pub min: f64,
    /// Maximum value across all lattice points.
    pub max: f64,
    /// Mean value.
    pub mean: f64,
    /// Variance (unbiased).
    pub variance: f64,
}

impl ChannelStats {
    /// Standard deviation.
    #[allow(dead_code)]
    #[must_use]
    pub fn std_dev(&self) -> f64 {
        self.variance.sqrt()
    }

    /// Dynamic range (max − min).
    #[allow(dead_code)]
    #[must_use]
    pub fn range(&self) -> f64 {
        self.max - self.min
    }
}

/// Full LUT output statistics (one `ChannelStats` per RGB channel).
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct LutStats {
    /// Red channel statistics.
    pub r: ChannelStats,
    /// Green channel statistics.
    pub g: ChannelStats,
    /// Blue channel statistics.
    pub b: ChannelStats,
    /// Total number of lattice points analysed.
    pub sample_count: usize,
}

/// Compute statistics over all lattice points of a 3-D LUT.
#[allow(dead_code)]
#[must_use]
pub fn compute_lut_stats(lut: &[Rgb]) -> LutStats {
    let n = lut.len();
    assert!(n > 0, "LUT must not be empty");

    let mut sum = [0.0f64; 3];
    let mut sum_sq = [0.0f64; 3];
    let mut mins = [f64::INFINITY; 3];
    let mut maxs = [f64::NEG_INFINITY; 3];

    for entry in lut {
        for ch in 0..3 {
            let v = entry[ch];
            sum[ch] += v;
            sum_sq[ch] += v * v;
            if v < mins[ch] {
                mins[ch] = v;
            }
            if v > maxs[ch] {
                maxs[ch] = v;
            }
        }
    }

    let nf = n as f64;
    let make_stats = |ch: usize| {
        let mean = sum[ch] / nf;
        let variance = if n > 1 {
            (sum_sq[ch] - sum[ch] * sum[ch] / nf) / (nf - 1.0)
        } else {
            0.0
        };
        ChannelStats {
            min: mins[ch],
            max: maxs[ch],
            mean,
            variance,
        }
    };

    LutStats {
        r: make_stats(0),
        g: make_stats(1),
        b: make_stats(2),
        sample_count: n,
    }
}

// ---------------------------------------------------------------------------
// Gamut coverage
// ---------------------------------------------------------------------------

/// A simple axis-aligned gamut box in RGB space.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct GamutBox {
    /// Minimum red value of the gamut box.
    pub r_min: f64,
    /// Maximum red value of the gamut box.
    pub r_max: f64,
    /// Minimum green value of the gamut box.
    pub g_min: f64,
    /// Maximum green value of the gamut box.
    pub g_max: f64,
    /// Minimum blue value of the gamut box.
    pub b_min: f64,
    /// Maximum blue value of the gamut box.
    pub b_max: f64,
}

impl GamutBox {
    /// Standard `[0, 1]³` SDR gamut box.
    #[allow(dead_code)]
    #[must_use]
    pub const fn sdr() -> Self {
        Self {
            r_min: 0.0,
            r_max: 1.0,
            g_min: 0.0,
            g_max: 1.0,
            b_min: 0.0,
            b_max: 1.0,
        }
    }

    /// Returns true if the colour lies inside the box.
    #[allow(dead_code)]
    #[must_use]
    pub fn contains(self, rgb: &Rgb) -> bool {
        rgb[0] >= self.r_min
            && rgb[0] <= self.r_max
            && rgb[1] >= self.g_min
            && rgb[1] <= self.g_max
            && rgb[2] >= self.b_min
            && rgb[2] <= self.b_max
    }
}

/// Gamut coverage report.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct GamutCoverage {
    /// Number of lattice points whose output lies within the gamut box.
    pub inside_count: usize,
    /// Total lattice points examined.
    pub total_count: usize,
    /// Fraction in `[0, 1]`.
    pub coverage_ratio: f64,
}

impl GamutCoverage {
    /// Percentage (0–100).
    #[allow(dead_code)]
    #[must_use]
    pub fn coverage_percent(&self) -> f64 {
        self.coverage_ratio * 100.0
    }
}

/// Analyse what fraction of LUT output values fall within `gamut`.
#[allow(dead_code)]
#[must_use]
pub fn analyse_gamut_coverage(lut: &[Rgb], gamut: GamutBox) -> GamutCoverage {
    let total = lut.len();
    let inside = lut.iter().filter(|rgb| gamut.contains(rgb)).count();
    let ratio = if total == 0 {
        0.0
    } else {
        inside as f64 / total as f64
    };
    GamutCoverage {
        inside_count: inside,
        total_count: total,
        coverage_ratio: ratio,
    }
}

// ---------------------------------------------------------------------------
// Clipping analysis
// ---------------------------------------------------------------------------

/// Clipping analysis result.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ClippingReport {
    /// Number of lattice points clipped to 0 in at least one channel.
    pub clipped_black: usize,
    /// Number of lattice points clipped to 1 in at least one channel.
    pub clipped_white: usize,
    /// Number of lattice points clipped to either extreme.
    pub total_clipped: usize,
    /// Fraction of all points that are clipped.
    pub clip_ratio: f64,
}

/// Detect clipping in a LUT (values outside `[clip_lo, clip_hi]`).
#[allow(dead_code)]
#[must_use]
pub fn analyse_clipping(lut: &[Rgb], clip_lo: f64, clip_hi: f64) -> ClippingReport {
    let total = lut.len();
    let mut clipped_black = 0usize;
    let mut clipped_white = 0usize;

    for rgb in lut {
        let lo = rgb.iter().any(|&v| v < clip_lo);
        let hi = rgb.iter().any(|&v| v > clip_hi);
        if lo {
            clipped_black += 1;
        }
        if hi {
            clipped_white += 1;
        }
    }

    let tc = clipped_black + clipped_white;
    let ratio = if total == 0 {
        0.0
    } else {
        tc as f64 / total as f64
    };
    ClippingReport {
        clipped_black,
        clipped_white,
        total_clipped: tc,
        clip_ratio: ratio,
    }
}

// ---------------------------------------------------------------------------
// Colorimetric shift measurement
// ---------------------------------------------------------------------------

/// Colorimetric shift statistics between a LUT and a reference.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ColorimetricShift {
    /// Mean Euclidean distance between output and reference.
    pub mean_delta_e: f64,
    /// Maximum Euclidean distance (worst case).
    pub peak_delta_e: f64,
    /// Root-mean-square distance.
    pub rms_delta_e: f64,
    /// Number of samples measured.
    pub sample_count: usize,
}

/// Measure the colorimetric shift between `lut` and `reference`.
///
/// Both slices must have the same length. The metric used is the Euclidean
/// distance in (linear) RGB space (analogous to a simplified ΔE).
#[allow(dead_code)]
#[must_use]
pub fn measure_colorimetric_shift(lut: &[Rgb], reference: &[Rgb]) -> ColorimetricShift {
    assert_eq!(
        lut.len(),
        reference.len(),
        "slices must have the same length"
    );
    let n = lut.len();
    if n == 0 {
        return ColorimetricShift {
            mean_delta_e: 0.0,
            peak_delta_e: 0.0,
            rms_delta_e: 0.0,
            sample_count: 0,
        };
    }

    let mut sum = 0.0f64;
    let mut sum_sq = 0.0f64;
    let mut peak = 0.0f64;

    for (a, b) in lut.iter().zip(reference.iter()) {
        let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        sum += d;
        sum_sq += d * d;
        if d > peak {
            peak = d;
        }
    }

    let nf = n as f64;
    ColorimetricShift {
        mean_delta_e: sum / nf,
        peak_delta_e: peak,
        rms_delta_e: (sum_sq / nf).sqrt(),
        sample_count: n,
    }
}

/// Measure the shift of `lut` from the identity LUT of the same size.
#[allow(dead_code)]
#[must_use]
pub fn shift_from_identity(lut: &[Rgb], size: usize) -> ColorimetricShift {
    assert_eq!(lut.len(), size * size * size, "LUT length mismatch");
    let scale = (size - 1) as f64;
    let reference: Vec<Rgb> = (0..size)
        .flat_map(|r| {
            (0..size).flat_map(move |g| {
                (0..size).map(move |b| [r as f64 / scale, g as f64 / scale, b as f64 / scale])
            })
        })
        .collect();
    measure_colorimetric_shift(lut, &reference)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn identity_lut(size: usize) -> Vec<Rgb> {
        let scale = (size - 1) as f64;
        let mut lut = Vec::with_capacity(size * size * size);
        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    lut.push([r as f64 / scale, g as f64 / scale, b as f64 / scale]);
                }
            }
        }
        lut
    }

    #[test]
    fn test_stats_identity_lut() {
        let lut = identity_lut(3);
        let stats = compute_lut_stats(&lut);
        assert!((stats.r.min - 0.0).abs() < 1e-9);
        assert!((stats.r.max - 1.0).abs() < 1e-9);
        assert_eq!(stats.sample_count, 27);
    }

    #[test]
    fn test_stats_mean_identity() {
        let lut = identity_lut(3);
        let stats = compute_lut_stats(&lut);
        // Identity LUT mean per channel should be 0.5.
        assert!((stats.r.mean - 0.5).abs() < 0.01);
        assert!((stats.g.mean - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_stats_std_dev() {
        let lut = identity_lut(3);
        let stats = compute_lut_stats(&lut);
        // Std dev should be > 0.
        assert!(stats.r.std_dev() > 0.0);
    }

    #[test]
    fn test_gamut_coverage_identity_sdr() {
        let lut = identity_lut(3);
        let cov = analyse_gamut_coverage(&lut, GamutBox::sdr());
        assert!((cov.coverage_ratio - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_gamut_coverage_half_sdr() {
        // LUT that maps all outputs to 2.0 (out of SDR gamut).
        let lut: Vec<Rgb> = vec![[2.0, 2.0, 2.0]; 27];
        let cov = analyse_gamut_coverage(&lut, GamutBox::sdr());
        assert_eq!(cov.inside_count, 0);
        assert!((cov.coverage_ratio - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_gamut_box_contains() {
        let gb = GamutBox::sdr();
        assert!(gb.contains(&[0.5, 0.5, 0.5]));
        assert!(!gb.contains(&[1.1, 0.5, 0.5]));
    }

    #[test]
    fn test_coverage_percent() {
        let lut = identity_lut(3);
        let cov = analyse_gamut_coverage(&lut, GamutBox::sdr());
        assert!((cov.coverage_percent() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_clipping_identity_no_clip() {
        let lut = identity_lut(3);
        let report = analyse_clipping(&lut, 0.0, 1.0);
        assert_eq!(report.total_clipped, 0);
    }

    #[test]
    fn test_clipping_detects_over() {
        let mut lut = identity_lut(3);
        lut[0] = [2.0, 0.5, 0.5]; // White clip.
        let report = analyse_clipping(&lut, 0.0, 1.0);
        assert!(report.clipped_white >= 1);
    }

    #[test]
    fn test_clipping_detects_under() {
        let mut lut = identity_lut(3);
        lut[0] = [-0.1, 0.5, 0.5]; // Black clip.
        let report = analyse_clipping(&lut, 0.0, 1.0);
        assert!(report.clipped_black >= 1);
    }

    #[test]
    fn test_shift_from_identity_zero() {
        let lut = identity_lut(3);
        let shift = shift_from_identity(&lut, 3);
        assert!(shift.mean_delta_e < 1e-9);
        assert!(shift.peak_delta_e < 1e-9);
    }

    #[test]
    fn test_shift_from_identity_non_zero() {
        let mut lut = identity_lut(3);
        // Offset every point slightly.
        for p in &mut lut {
            p[0] = (p[0] + 0.1).min(1.0);
        }
        let shift = shift_from_identity(&lut, 3);
        assert!(shift.mean_delta_e > 0.0);
    }

    #[test]
    fn test_colorimetric_shift_symmetric() {
        let lut = identity_lut(3);
        let mut lut2 = lut.clone();
        for p in &mut lut2 {
            p[1] = (p[1] + 0.05).min(1.0);
        }
        let fwd = measure_colorimetric_shift(&lut, &lut2);
        let rev = measure_colorimetric_shift(&lut2, &lut);
        assert!((fwd.mean_delta_e - rev.mean_delta_e).abs() < 1e-9);
    }

    #[test]
    fn test_channel_stats_range() {
        let lut: Vec<Rgb> = vec![[0.2, 0.3, 0.4], [0.8, 0.7, 0.6]];
        let stats = compute_lut_stats(&lut);
        assert!((stats.r.range() - 0.6).abs() < 1e-9);
    }
}
