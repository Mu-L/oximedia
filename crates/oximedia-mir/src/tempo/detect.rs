//! Tempo detection using autocorrelation and comb filtering.

use crate::types::TempoResult;
use crate::utils::{autocorrelation, find_peaks, mean};
use crate::{MirError, MirResult};

/// Tempo detector.
pub struct TempoDetector {
    sample_rate: f32,
    min_bpm: f32,
    max_bpm: f32,
}

impl TempoDetector {
    /// Create a new tempo detector.
    #[must_use]
    pub fn new(sample_rate: f32, min_bpm: f32, max_bpm: f32) -> Self {
        Self {
            sample_rate,
            min_bpm,
            max_bpm,
        }
    }

    /// Detect tempo from audio signal.
    ///
    /// Uses autocorrelation-based method with comb filtering for robustness.
    ///
    /// # Errors
    ///
    /// Returns error if signal is too short or tempo detection fails.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_precision_loss)]
    pub fn detect(&self, signal: &[f32]) -> MirResult<TempoResult> {
        if signal.len() < self.sample_rate as usize {
            return Err(MirError::InsufficientData(
                "Signal too short for tempo detection".to_string(),
            ));
        }

        // Compute onset strength envelope
        let onset_env = self.compute_onset_envelope(signal)?;

        // Compute autocorrelation of onset envelope
        let acf = autocorrelation(&onset_env)?;

        // Convert BPM range to lag range
        let min_lag = self.bpm_to_lag(self.max_bpm);
        let max_lag = self.bpm_to_lag(self.min_bpm).min(acf.len() - 1);

        if min_lag >= max_lag {
            return Err(MirError::InvalidConfig(
                "Invalid BPM range for tempo detection".to_string(),
            ));
        }

        // Find peaks in autocorrelation within valid lag range
        let peaks = find_peaks(&acf[min_lag..max_lag], 10);

        if peaks.is_empty() {
            return Err(MirError::AnalysisFailed("No tempo peaks found".to_string()));
        }

        // Convert peaks to BPM and sort by strength
        let mut candidates: Vec<(f32, f32)> = peaks
            .iter()
            .map(|&peak_idx| {
                let lag = peak_idx + min_lag;
                let bpm = self.lag_to_bpm(lag);
                let strength = acf[lag];
                (bpm, strength)
            })
            .collect();

        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Apply comb filtering to refine estimate
        let primary_bpm = candidates[0].0;
        let refined_bpm = self.refine_with_comb_filter(&onset_env, primary_bpm)?;

        // Compute confidence based on peak prominence
        let max_acf = acf[min_lag..max_lag]
            .iter()
            .fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let confidence = if max_acf > 0.0 {
            candidates[0].1 / max_acf
        } else {
            0.0
        }
        .clamp(0.0, 1.0);

        // Compute tempo stability
        let stability = self.compute_stability(&onset_env, refined_bpm);

        // Get alternative tempo estimates
        let alternatives: Vec<(f32, f32)> = candidates
            .iter()
            .skip(1)
            .take(3)
            .map(|&(bpm, strength)| (bpm, strength / max_acf))
            .collect();

        Ok(TempoResult {
            bpm: refined_bpm,
            confidence,
            stability,
            alternatives,
        })
    }

    /// Compute onset strength envelope.
    fn compute_onset_envelope(&self, signal: &[f32]) -> MirResult<Vec<f32>> {
        let hop_size = 512;
        let window_size = 2048;

        let stft = crate::utils::stft(signal, window_size, hop_size)?;

        let mut onset_env = Vec::with_capacity(stft.len());
        let mut prev_mag = vec![0.0; window_size / 2 + 1];

        for frame in &stft {
            let mag = crate::utils::magnitude_spectrum(frame);

            // Spectral flux (positive differences)
            let flux: f32 = mag
                .iter()
                .zip(&prev_mag)
                .map(|(m, p)| (m - p).max(0.0))
                .sum();

            onset_env.push(flux);
            prev_mag = mag;
        }

        // Normalize onset envelope
        let max_val = onset_env.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        if max_val > 0.0 {
            for val in &mut onset_env {
                *val /= max_val;
            }
        }

        Ok(onset_env)
    }

    /// Refine BPM estimate using comb filtering.
    #[allow(clippy::unnecessary_wraps)]
    fn refine_with_comb_filter(&self, onset_env: &[f32], initial_bpm: f32) -> MirResult<f32> {
        // Try small variations around the initial estimate
        let mut best_bpm = initial_bpm;
        let mut best_score = 0.0;

        for factor in [-0.05, -0.02, 0.0, 0.02, 0.05] {
            let test_bpm = initial_bpm * (1.0 + factor);
            let test_period = self.bpm_to_lag(test_bpm);

            // Compute comb filter response
            let mut score = 0.0;
            let mut count = 0;

            for i in 0..onset_env.len() {
                let mut comb_sum = 0.0;
                for harmonic in 1..=4 {
                    let offset = harmonic * test_period;
                    if i >= offset && i < onset_env.len() {
                        comb_sum += onset_env[i - offset];
                    }
                }
                score += onset_env[i] * comb_sum;
                count += 1;
            }

            if count > 0 {
                score /= count as f32;
            }

            if score > best_score {
                best_score = score;
                best_bpm = test_bpm;
            }
        }

        Ok(best_bpm)
    }

    /// Compute tempo stability.
    fn compute_stability(&self, onset_env: &[f32], bpm: f32) -> f32 {
        let period = self.bpm_to_lag(bpm);

        if period == 0 || onset_env.len() < period * 4 {
            return 0.5;
        }

        // Measure consistency of onset strength at regular intervals
        let mut intervals = Vec::new();
        for i in (period..onset_env.len()).step_by(period) {
            intervals.push(onset_env[i]);
        }

        if intervals.is_empty() {
            return 0.5;
        }

        // Stability is inversely related to variance
        let mean_val = mean(&intervals);
        if mean_val == 0.0 {
            return 0.5;
        }

        let variance: f32 = intervals
            .iter()
            .map(|v| (v - mean_val).powi(2))
            .sum::<f32>()
            / intervals.len() as f32;

        let cv = variance.sqrt() / mean_val; // Coefficient of variation

        // Convert to stability score (lower CV = higher stability)
        (1.0 - cv.min(1.0)).clamp(0.0, 1.0)
    }

    /// Convert BPM to lag in samples (for onset envelope at hop rate).
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    fn bpm_to_lag(&self, bpm: f32) -> usize {
        let hop_size = 512.0;
        let beats_per_second = bpm / 60.0;
        let samples_per_beat = self.sample_rate / beats_per_second;
        let frames_per_beat = samples_per_beat / hop_size;
        frames_per_beat.round() as usize
    }

    /// Convert lag to BPM.
    #[allow(clippy::cast_precision_loss)]
    fn lag_to_bpm(&self, lag: usize) -> f32 {
        let hop_size = 512.0;
        let samples = lag as f32 * hop_size;
        let beats_per_second = self.sample_rate / samples;
        beats_per_second * 60.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tempo_detector_creation() {
        let detector = TempoDetector::new(44100.0, 60.0, 200.0);
        assert_eq!(detector.sample_rate, 44100.0);
    }

    #[test]
    fn test_bpm_lag_conversion() {
        let detector = TempoDetector::new(44100.0, 60.0, 200.0);
        let bpm = 120.0;
        let lag = detector.bpm_to_lag(bpm);
        let recovered_bpm = detector.lag_to_bpm(lag);
        assert!((recovered_bpm - bpm).abs() < 1.0);
    }

    #[test]
    fn test_detect_insufficient_data() {
        let detector = TempoDetector::new(44100.0, 60.0, 200.0);
        let signal = vec![0.0; 1000]; // Too short
        let result = detector.detect(&signal);
        assert!(result.is_err());
    }
}
