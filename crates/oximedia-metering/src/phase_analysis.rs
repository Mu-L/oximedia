//! Phase and polarity metering: correlation, goniometer, stereo width analysis.
//!
//! This module provides per-block phase analysis functions, complementing the
//! existing `phase.rs` streaming meters with functional-style APIs.

#![allow(dead_code)]

/// Result of a phase analysis operation.
#[derive(Clone, Debug)]
pub struct PhaseResult {
    /// Pearson correlation coefficient between L and R (-1.0 to +1.0).
    pub correlation: f64,
    /// Goniometer (M/S rotated) scatter points for display.
    pub goniometer_points: Vec<(f64, f64)>,
    /// True if correlation > 0 (channels are in phase).
    pub is_in_phase: bool,
    /// Mono compatibility score (0.0 = completely out of phase, 1.0 = fully mono-compatible).
    pub mono_compatibility: f64,
}

/// Compute the Pearson correlation coefficient between two channels.
///
/// Returns a value in [-1.0, +1.0]:
/// - +1.0: identical signals (fully in phase)
/// -  0.0: uncorrelated signals
/// - -1.0: inverted signals (fully out of phase)
///
/// # Arguments
///
/// * `left` - Left channel samples
/// * `right` - Right channel samples
#[must_use]
pub fn compute_phase_correlation(left: &[f64], right: &[f64]) -> f64 {
    let n = left.len().min(right.len());
    if n == 0 {
        return 0.0;
    }

    let mut sum_l = 0.0f64;
    let mut sum_r = 0.0f64;

    for i in 0..n {
        sum_l += left[i];
        sum_r += right[i];
    }

    let nf = n as f64;
    let mean_l = sum_l / nf;
    let mean_r = sum_r / nf;

    let mut cov = 0.0f64;
    let mut var_l = 0.0f64;
    let mut var_r = 0.0f64;

    for i in 0..n {
        let dl = left[i] - mean_l;
        let dr = right[i] - mean_r;
        cov += dl * dr;
        var_l += dl * dl;
        var_r += dr * dr;
    }

    let denom = (var_l * var_r).sqrt();
    if denom < 1e-15 {
        // Avoid division by zero; if both channels are constant, return 1.0
        if var_l < 1e-15 && var_r < 1e-15 {
            1.0
        } else {
            0.0
        }
    } else {
        (cov / denom).clamp(-1.0, 1.0)
    }
}

/// Compute a mono compatibility score.
///
/// Measures the ratio of mono sum energy to stereo difference energy.
/// A score of 1.0 indicates perfect mono compatibility; 0.0 indicates
/// complete phase cancellation.
///
/// # Arguments
///
/// * `left` - Left channel samples
/// * `right` - Right channel samples
#[must_use]
pub fn mono_compatibility_score(left: &[f64], right: &[f64]) -> f64 {
    let n = left.len().min(right.len());
    if n == 0 {
        return 1.0;
    }

    let mut sum_energy = 0.0f64;
    let mut diff_energy = 0.0f64;

    for i in 0..n {
        let sum = left[i] + right[i];
        let diff = left[i] - right[i];
        sum_energy += sum * sum;
        diff_energy += diff * diff;
    }

    let total = sum_energy + diff_energy;
    if total < 1e-15 {
        return 1.0;
    }
    sum_energy / total
}

/// Convert a stereo L/R sample pair to M/S coordinates for goniometer display.
///
/// Rotates by 45 degrees: M = (L + R) / sqrt(2), S = (L - R) / sqrt(2).
///
/// # Arguments
///
/// * `left` - Left channel sample
/// * `right` - Right channel sample
///
/// # Returns
///
/// `(mid, side)` coordinate pair for goniometer display
#[must_use]
pub fn goniometer_sample(left: f64, right: f64) -> (f64, f64) {
    let mid = (left + right) / std::f64::consts::SQRT_2;
    let side = (left - right) / std::f64::consts::SQRT_2;
    (mid, side)
}

/// Compute the stereo width of a stereo signal.
///
/// Returns a value where:
/// - 0.0 = pure mono (no stereo content)
/// - 1.0 = full normal stereo
/// - >1.0 = super-wide (M/S encoded or artificially widened)
///
/// # Arguments
///
/// * `left` - Left channel samples
/// * `right` - Right channel samples
#[must_use]
pub fn stereo_width(left: &[f64], right: &[f64]) -> f64 {
    let n = left.len().min(right.len());
    if n == 0 {
        return 0.0;
    }

    let mut mid_energy = 0.0f64;
    let mut side_energy = 0.0f64;

    for i in 0..n {
        let mid = (left[i] + right[i]) / 2.0;
        let side = (left[i] - right[i]) / 2.0;
        mid_energy += mid * mid;
        side_energy += side * side;
    }

    let total = mid_energy + side_energy;
    if total < 1e-15 {
        return 0.0;
    }
    if mid_energy < 1e-15 {
        // Pure antiphase: infinite width conceptually, cap at 4.0
        return 4.0;
    }
    // Width = side_energy / mid_energy (0 = mono, 1 = equal energy, >1 = super-wide)
    (side_energy / mid_energy).sqrt().min(4.0)
}

/// Detect if the right channel appears to be a polarity-inverted copy of the left.
///
/// Returns true if the correlation is strongly negative (< -0.8).
///
/// # Arguments
///
/// * `left` - Left channel samples
/// * `right` - Right channel samples
#[must_use]
pub fn detect_polarity_inversion(left: &[f64], right: &[f64]) -> bool {
    compute_phase_correlation(left, right) < -0.8
}

/// Streaming phase analyzer with windowed history.
pub struct PhaseAnalyzer {
    /// Window size in samples.
    pub window_size: usize,
    /// History of correlation values.
    pub history: Vec<f64>,
}

impl PhaseAnalyzer {
    /// Create a new phase analyzer.
    ///
    /// # Arguments
    ///
    /// * `window_size` - Number of samples per analysis window
    #[must_use]
    pub fn new(window_size: usize) -> Self {
        let size = window_size.max(1);
        Self {
            window_size: size,
            history: Vec::new(),
        }
    }

    /// Update the analyzer with a new block of stereo samples and return a `PhaseResult`.
    ///
    /// # Arguments
    ///
    /// * `left` - Left channel samples
    /// * `right` - Right channel samples
    pub fn update(&mut self, left: &[f64], right: &[f64]) -> PhaseResult {
        let correlation = compute_phase_correlation(left, right);
        self.history.push(correlation);
        // Keep only recent history
        if self.history.len() > 100 {
            self.history.remove(0);
        }

        let n = left.len().min(right.len());
        let goniometer_points: Vec<(f64, f64)> = (0..n.min(256))
            .map(|i| goniometer_sample(left[i], right[i]))
            .collect();

        let mono_compat = mono_compatibility_score(left, right);

        PhaseResult {
            correlation,
            goniometer_points,
            is_in_phase: correlation >= 0.0,
            mono_compatibility: mono_compat,
        }
    }

    /// Get the average correlation over all history.
    #[must_use]
    pub fn average_correlation(&self) -> f64 {
        if self.history.is_empty() {
            return 0.0;
        }
        self.history.iter().sum::<f64>() / self.history.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_wave(freq: f64, sample_rate: f64, n: usize, phase_offset: f64) -> Vec<f64> {
        (0..n)
            .map(|i| {
                let t = i as f64 / sample_rate;
                (2.0 * std::f64::consts::PI * freq * t + phase_offset).sin()
            })
            .collect()
    }

    #[test]
    fn test_phase_correlation_identical() {
        let left = sine_wave(440.0, 48000.0, 4800, 0.0);
        let right = left.clone();
        let corr = compute_phase_correlation(&left, &right);
        assert!((corr - 1.0).abs() < 1e-6, "corr = {corr}");
    }

    #[test]
    fn test_phase_correlation_inverted() {
        let left = sine_wave(440.0, 48000.0, 4800, 0.0);
        let right: Vec<f64> = left.iter().map(|&x| -x).collect();
        let corr = compute_phase_correlation(&left, &right);
        assert!((corr + 1.0).abs() < 1e-6, "corr = {corr}");
    }

    #[test]
    fn test_phase_correlation_orthogonal() {
        // 90-degree out of phase: sine vs cosine
        let left = sine_wave(440.0, 48000.0, 4800, 0.0);
        let right = sine_wave(440.0, 48000.0, 4800, std::f64::consts::PI / 2.0);
        let corr = compute_phase_correlation(&left, &right);
        assert!(corr.abs() < 0.05, "corr = {corr}");
    }

    #[test]
    fn test_phase_correlation_empty() {
        assert_eq!(compute_phase_correlation(&[], &[]), 0.0);
    }

    #[test]
    fn test_mono_compatibility_mono_signal() {
        let signal: Vec<f64> = (0..480).map(|i| (i as f64 / 480.0).sin()).collect();
        let score = mono_compatibility_score(&signal, &signal);
        // L+R = 2*signal, L-R = 0 -> fully mono compatible -> score = 1.0
        assert!((score - 1.0).abs() < 1e-6, "score = {score}");
    }

    #[test]
    fn test_mono_compatibility_inverted() {
        let signal: Vec<f64> = (0..480).map(|i| (i as f64 * 0.1).sin()).collect();
        let inverted: Vec<f64> = signal.iter().map(|&x| -x).collect();
        let score = mono_compatibility_score(&signal, &inverted);
        // L+R = 0 -> no mono energy -> score = 0.0
        assert!(score < 0.01, "score = {score}");
    }

    #[test]
    fn test_goniometer_sample_mono() {
        let (mid, side) = goniometer_sample(0.5, 0.5);
        // mid = 0.5*sqrt(2), side = 0
        assert!((mid - 0.5 * std::f64::consts::SQRT_2).abs() < 1e-10);
        assert!(side.abs() < 1e-10);
    }

    #[test]
    fn test_goniometer_sample_pure_left() {
        let (mid, side) = goniometer_sample(1.0, 0.0);
        assert!((mid - 1.0 / std::f64::consts::SQRT_2).abs() < 1e-10);
        assert!((side - 1.0 / std::f64::consts::SQRT_2).abs() < 1e-10);
    }

    #[test]
    fn test_stereo_width_mono_is_zero() {
        let signal: Vec<f64> = (0..480).map(|i| (i as f64 * 0.01).sin()).collect();
        let width = stereo_width(&signal, &signal);
        assert!(width < 0.01, "width = {width}");
    }

    #[test]
    fn test_stereo_width_antiphase_is_wide() {
        let left: Vec<f64> = (0..480).map(|i| (i as f64 * 0.01).sin()).collect();
        let right: Vec<f64> = left.iter().map(|&x| -x).collect();
        let width = stereo_width(&left, &right);
        assert!(width > 1.0, "width = {width}");
    }

    #[test]
    fn test_stereo_width_empty() {
        assert_eq!(stereo_width(&[], &[]), 0.0);
    }

    #[test]
    fn test_detect_polarity_inversion_true() {
        let left = sine_wave(1000.0, 48000.0, 4800, 0.0);
        let right: Vec<f64> = left.iter().map(|&x| -x).collect();
        assert!(detect_polarity_inversion(&left, &right));
    }

    #[test]
    fn test_detect_polarity_inversion_false() {
        let left = sine_wave(1000.0, 48000.0, 4800, 0.0);
        let right = left.clone();
        assert!(!detect_polarity_inversion(&left, &right));
    }

    #[test]
    fn test_phase_analyzer_update() {
        let mut analyzer = PhaseAnalyzer::new(4800);
        let left = sine_wave(440.0, 48000.0, 4800, 0.0);
        let right = left.clone();
        let result = analyzer.update(&left, &right);
        assert!((result.correlation - 1.0).abs() < 1e-6);
        assert!(result.is_in_phase);
        assert!(!result.goniometer_points.is_empty());
    }

    #[test]
    fn test_phase_analyzer_history() {
        let mut analyzer = PhaseAnalyzer::new(480);
        let left = sine_wave(440.0, 48000.0, 480, 0.0);
        let right = left.clone();
        for _ in 0..5 {
            analyzer.update(&left, &right);
        }
        assert_eq!(analyzer.history.len(), 5);
        let avg = analyzer.average_correlation();
        assert!((avg - 1.0).abs() < 1e-6);
    }
}
