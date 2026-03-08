#![allow(dead_code)]
//! Watermark strength analysis and adaptive strength control.
//!
//! This module provides tools for analysing the optimal embedding strength of a
//! watermark given a host signal, as well as adaptive algorithms that adjust
//! strength on a per-frame basis to balance imperceptibility and robustness.

use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Strength profile
// ---------------------------------------------------------------------------

/// A per-frame strength profile describing how much modification each segment
/// of the audio can tolerate.
#[derive(Debug, Clone)]
pub struct StrengthProfile {
    /// Strength value for each frame in [0.0, 1.0].
    pub values: Vec<f64>,
    /// Frame size that was used for analysis.
    pub frame_size: usize,
    /// Hop size between frames.
    pub hop_size: usize,
}

impl StrengthProfile {
    /// Return the average strength across all frames.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn mean(&self) -> f64 {
        if self.values.is_empty() {
            return 0.0;
        }
        self.values.iter().sum::<f64>() / self.values.len() as f64
    }

    /// Return the minimum strength across all frames.
    pub fn min(&self) -> f64 {
        self.values.iter().copied().fold(f64::INFINITY, f64::min)
    }

    /// Return the maximum strength across all frames.
    pub fn max(&self) -> f64 {
        self.values
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Return the strength at a specific frame index.
    #[must_use]
    pub fn at(&self, frame_index: usize) -> Option<f64> {
        self.values.get(frame_index).copied()
    }

    /// Number of frames.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the profile is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Analyser configuration
// ---------------------------------------------------------------------------

/// Configuration for the strength analyser.
#[derive(Debug, Clone)]
pub struct StrengthAnalyserConfig {
    /// Frame size for analysis windows.
    pub frame_size: usize,
    /// Hop size between frames.
    pub hop_size: usize,
    /// Target minimum SNR in dB after embedding.
    pub target_snr_db: f64,
    /// Maximum allowed strength (cap).
    pub max_strength: f64,
    /// Minimum allowed strength (floor).
    pub min_strength: f64,
    /// Smoothing window (number of frames for temporal smoothing).
    pub smoothing_window: usize,
}

impl Default for StrengthAnalyserConfig {
    fn default() -> Self {
        Self {
            frame_size: 2048,
            hop_size: 1024,
            target_snr_db: 30.0,
            max_strength: 0.3,
            min_strength: 0.001,
            smoothing_window: 5,
        }
    }
}

// ---------------------------------------------------------------------------
// Analyser
// ---------------------------------------------------------------------------

/// Analyses audio to produce an adaptive strength profile.
#[derive(Debug, Clone)]
pub struct StrengthAnalyser {
    /// Analyser configuration.
    pub config: StrengthAnalyserConfig,
}

impl StrengthAnalyser {
    /// Create a new analyser with the given configuration.
    #[must_use]
    pub fn new(config: StrengthAnalyserConfig) -> Self {
        Self { config }
    }

    /// Analyse a signal and produce a per-frame strength profile.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn analyse(&self, samples: &[f64]) -> StrengthProfile {
        let num_frames = if samples.len() >= self.config.frame_size {
            (samples.len() - self.config.frame_size) / self.config.hop_size + 1
        } else {
            0
        };

        let mut raw = Vec::with_capacity(num_frames);
        for f in 0..num_frames {
            let start = f * self.config.hop_size;
            let end = start + self.config.frame_size;
            let frame = &samples[start..end];
            let rms = frame_rms(frame);
            // Strength proportional to RMS: louder frames can hide more
            let target_noise = rms / db_to_linear(self.config.target_snr_db);
            let strength = target_noise.clamp(self.config.min_strength, self.config.max_strength);
            raw.push(strength);
        }

        let smoothed = temporal_smooth(&raw, self.config.smoothing_window);

        StrengthProfile {
            values: smoothed,
            frame_size: self.config.frame_size,
            hop_size: self.config.hop_size,
        }
    }

    /// Quick estimate of the optimal uniform strength for the whole signal.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn optimal_uniform(&self, samples: &[f64]) -> f64 {
        let rms = frame_rms(samples);
        let target_noise = rms / db_to_linear(self.config.target_snr_db);
        target_noise.clamp(self.config.min_strength, self.config.max_strength)
    }
}

// ---------------------------------------------------------------------------
// Strength validator
// ---------------------------------------------------------------------------

/// Validation result for a given embedding strength.
#[derive(Debug, Clone)]
pub struct StrengthValidation {
    /// Whether the strength passes the quality threshold.
    pub passes: bool,
    /// Estimated SNR after embedding with this strength.
    pub estimated_snr_db: f64,
    /// Suggested strength if current one doesn't pass.
    pub suggested_strength: f64,
    /// Per-frame quality estimates.
    pub frame_snrs: Vec<f64>,
}

/// Validate that a given strength will maintain quality targets.
#[allow(clippy::cast_precision_loss)]
pub fn validate_strength(
    samples: &[f64],
    strength: f64,
    target_snr_db: f64,
    frame_size: usize,
    hop_size: usize,
) -> StrengthValidation {
    let num_frames = if samples.len() >= frame_size {
        (samples.len() - frame_size) / hop_size + 1
    } else {
        0
    };

    let mut frame_snrs = Vec::with_capacity(num_frames);
    for f in 0..num_frames {
        let start = f * hop_size;
        let end = start + frame_size;
        let frame = &samples[start..end];
        let rms = frame_rms(frame);
        let noise_rms = strength;
        let snr = if noise_rms > 0.0 {
            20.0 * (rms / noise_rms).log10()
        } else {
            f64::INFINITY
        };
        frame_snrs.push(snr);
    }

    let min_snr = frame_snrs.iter().copied().fold(f64::INFINITY, f64::min);
    let passes = min_snr >= target_snr_db || frame_snrs.is_empty();

    // Suggest a strength that would achieve the target for the quietest frame
    let min_rms = (0..num_frames)
        .map(|f| {
            let start = f * hop_size;
            let end = start + frame_size;
            frame_rms(&samples[start..end])
        })
        .fold(f64::INFINITY, f64::min);
    let suggested = if min_rms.is_finite() && min_rms > 0.0 {
        min_rms / db_to_linear(target_snr_db)
    } else {
        0.001
    };

    StrengthValidation {
        passes,
        estimated_snr_db: min_snr,
        suggested_strength: suggested,
        frame_snrs,
    }
}

// ---------------------------------------------------------------------------
// Envelope follower
// ---------------------------------------------------------------------------

/// Real-time strength envelope follower that tracks audio level and outputs
/// a smoothed strength value.
#[derive(Debug, Clone)]
pub struct StrengthEnvelopeFollower {
    /// Attack time constant (samples).
    pub attack: f64,
    /// Release time constant (samples).
    pub release: f64,
    /// Current envelope value.
    envelope: f64,
    /// Minimum output strength.
    pub min_strength: f64,
    /// Maximum output strength.
    pub max_strength: f64,
    /// SNR target in dB.
    pub target_snr_db: f64,
}

impl StrengthEnvelopeFollower {
    /// Create a new envelope follower.
    #[must_use]
    pub fn new(attack: f64, release: f64, target_snr_db: f64) -> Self {
        Self {
            attack,
            release,
            envelope: 0.0,
            min_strength: 0.001,
            max_strength: 0.3,
            target_snr_db,
        }
    }

    /// Process one sample and return the recommended strength.
    pub fn process(&mut self, sample: f64) -> f64 {
        let abs_val = sample.abs();
        if abs_val > self.envelope {
            self.envelope += self.attack * (abs_val - self.envelope);
        } else {
            self.envelope += self.release * (abs_val - self.envelope);
        }
        let strength = self.envelope / db_to_linear(self.target_snr_db);
        strength.clamp(self.min_strength, self.max_strength)
    }

    /// Reset the envelope state.
    pub fn reset(&mut self) {
        self.envelope = 0.0;
    }

    /// Get the current envelope level.
    #[must_use]
    pub fn current_envelope(&self) -> f64 {
        self.envelope
    }
}

// ---------------------------------------------------------------------------
// Strength histogram
// ---------------------------------------------------------------------------

/// Histogram of strength values across frames.
#[derive(Debug, Clone)]
pub struct StrengthHistogram {
    /// Bin counts.
    pub bins: Vec<usize>,
    /// Bin edges (len = `bins.len()` + 1).
    pub edges: Vec<f64>,
}

impl StrengthHistogram {
    /// Build a histogram from a strength profile.
    #[must_use]
    pub fn from_profile(profile: &StrengthProfile, num_bins: usize) -> Self {
        let num_bins = num_bins.max(1);
        let lo = profile.min().min(0.0);
        let hi = profile.max().max(lo + 1e-12);
        let bin_width = (hi - lo) / num_bins as f64;

        let mut bins = vec![0usize; num_bins];
        let mut edges = Vec::with_capacity(num_bins + 1);
        for i in 0..=num_bins {
            edges.push(lo + i as f64 * bin_width);
        }

        for &v in &profile.values {
            let idx = ((v - lo) / bin_width) as usize;
            let idx = idx.min(num_bins - 1);
            bins[idx] += 1;
        }

        Self { bins, edges }
    }

    /// Return the bin with the most counts (mode).
    #[must_use]
    pub fn mode_bin(&self) -> usize {
        self.bins
            .iter()
            .enumerate()
            .max_by_key(|(_, &c)| c)
            .map_or(0, |(i, _)| i)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// RMS of a frame.
#[allow(clippy::cast_precision_loss)]
fn frame_rms(frame: &[f64]) -> f64 {
    if frame.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = frame.iter().map(|s| s * s).sum();
    (sum_sq / frame.len() as f64).sqrt()
}

/// Convert decibels to linear amplitude ratio.
fn db_to_linear(db: f64) -> f64 {
    10.0f64.powf(db / 20.0)
}

/// Temporal smoothing via moving average.
fn temporal_smooth(values: &[f64], window: usize) -> Vec<f64> {
    if window <= 1 || values.is_empty() {
        return values.to_vec();
    }
    let mut result = Vec::with_capacity(values.len());
    let mut buf: VecDeque<f64> = VecDeque::with_capacity(window);
    let mut running_sum = 0.0f64;

    for &v in values {
        buf.push_back(v);
        running_sum += v;
        if buf.len() > window {
            running_sum -= buf.pop_front().unwrap_or(0.0);
        }
        #[allow(clippy::cast_precision_loss)]
        let avg = running_sum / buf.len() as f64;
        result.push(avg);
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = StrengthAnalyserConfig::default();
        assert_eq!(cfg.frame_size, 2048);
        assert_eq!(cfg.hop_size, 1024);
        assert!((cfg.target_snr_db - 30.0).abs() < 1e-9);
    }

    #[test]
    fn test_frame_rms_silence() {
        let frame = vec![0.0f64; 512];
        assert!(frame_rms(&frame).abs() < 1e-12);
    }

    #[test]
    fn test_frame_rms_constant() {
        let frame = vec![0.5f64; 1000];
        let rms = frame_rms(&frame);
        assert!((rms - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_db_to_linear() {
        assert!((db_to_linear(0.0) - 1.0).abs() < 1e-9);
        assert!((db_to_linear(20.0) - 10.0).abs() < 1e-6);
        assert!((db_to_linear(40.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn test_temporal_smooth_passthrough() {
        let vals = vec![1.0, 2.0, 3.0, 4.0];
        let smoothed = temporal_smooth(&vals, 1);
        assert_eq!(smoothed, vals);
    }

    #[test]
    fn test_temporal_smooth_window() {
        let vals = vec![0.0, 0.0, 10.0, 0.0, 0.0];
        let smoothed = temporal_smooth(&vals, 3);
        // After the spike, smoothing should spread it
        assert!(smoothed[2] > smoothed[0]);
    }

    #[test]
    fn test_analyser_produces_profile() {
        let config = StrengthAnalyserConfig {
            frame_size: 256,
            hop_size: 128,
            smoothing_window: 3,
            ..Default::default()
        };
        let analyser = StrengthAnalyser::new(config);
        let signal: Vec<f64> = (0..4096).map(|i| (i as f64 * 0.01).sin()).collect();
        let profile = analyser.analyse(&signal);
        assert!(!profile.is_empty());
        assert!(profile.len() > 10);
        for &v in &profile.values {
            assert!(v >= 0.001); // min_strength
            assert!(v <= 0.3); // max_strength
        }
    }

    #[test]
    fn test_optimal_uniform() {
        let analyser = StrengthAnalyser::new(StrengthAnalyserConfig::default());
        let signal = vec![0.5f64; 4096];
        let strength = analyser.optimal_uniform(&signal);
        assert!(strength > 0.0);
        assert!(strength <= 0.3);
    }

    #[test]
    fn test_validate_strength_passes() {
        let signal: Vec<f64> = vec![1.0; 4096];
        let v = validate_strength(&signal, 0.001, 30.0, 256, 128);
        assert!(v.passes);
        assert!(v.estimated_snr_db > 30.0);
    }

    #[test]
    fn test_validate_strength_fails() {
        let signal: Vec<f64> = vec![0.001; 4096];
        let v = validate_strength(&signal, 0.5, 60.0, 256, 128);
        assert!(!v.passes);
        assert!(v.suggested_strength < 0.5);
    }

    #[test]
    fn test_envelope_follower() {
        let mut follower = StrengthEnvelopeFollower::new(0.1, 0.01, 30.0);
        let s1 = follower.process(0.8);
        assert!(s1 > 0.0);
        let s2 = follower.process(0.0);
        // Release is slow, so strength should still be positive
        assert!(s2 > 0.0);
        follower.reset();
        assert!(follower.current_envelope().abs() < 1e-12);
    }

    #[test]
    fn test_strength_profile_stats() {
        let profile = StrengthProfile {
            values: vec![0.1, 0.2, 0.3, 0.4, 0.5],
            frame_size: 256,
            hop_size: 128,
        };
        assert!((profile.mean() - 0.3).abs() < 1e-9);
        assert!((profile.min() - 0.1).abs() < 1e-9);
        assert!((profile.max() - 0.5).abs() < 1e-9);
        assert_eq!(profile.at(2), Some(0.3));
        assert_eq!(profile.at(99), None);
        assert_eq!(profile.len(), 5);
        assert!(!profile.is_empty());
    }

    #[test]
    fn test_histogram_from_profile() {
        let profile = StrengthProfile {
            values: vec![0.1, 0.1, 0.1, 0.5, 0.5],
            frame_size: 256,
            hop_size: 128,
        };
        let hist = StrengthHistogram::from_profile(&profile, 4);
        assert_eq!(hist.bins.len(), 4);
        assert_eq!(hist.edges.len(), 5);
        let total: usize = hist.bins.iter().sum();
        assert_eq!(total, 5);
        // Mode should be the first bin (contains three 0.1 values)
        assert_eq!(hist.mode_bin(), 0);
    }

    #[test]
    fn test_empty_signal() {
        let analyser = StrengthAnalyser::new(StrengthAnalyserConfig::default());
        let profile = analyser.analyse(&[]);
        assert!(profile.is_empty());
    }
}
