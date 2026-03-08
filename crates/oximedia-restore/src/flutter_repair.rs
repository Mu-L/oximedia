//! Wow and flutter correction for tape recordings.
//!
//! "Wow" refers to slow (< 6 Hz) speed variations; "flutter" refers to faster
//! (6–100 Hz) variations.  Both create pitch modulation that degrades
//! perceived quality.  This module provides detection via short-time
//! autocorrelation and correction via time-domain resampling.

/// Measures of wow and flutter for a recording.
#[derive(Debug, Clone)]
pub struct FlutterMeasurement {
    /// Dominant wow frequency in Hz (< 6 Hz).
    pub wow_hz: f32,
    /// Dominant flutter frequency in Hz (6–100 Hz).
    pub flutter_hz: f32,
    /// Combined depth of modulation as a percentage (0–100).
    pub depth_pct: f32,
    /// Wow severity score (0 = perfect, 1 = severe).
    pub wow_score: f32,
    /// Flutter severity score (0 = perfect, 1 = severe).
    pub flutter_score: f32,
}

/// Qualitative classification of wow/flutter severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlutterSeverity {
    /// Depth < 0.1 % – inaudible.
    Excellent,
    /// Depth 0.1–0.3 % – barely audible.
    Good,
    /// Depth 0.3–1.0 % – clearly audible.
    Fair,
    /// Depth > 1.0 % – strongly audible.
    Poor,
}

impl FlutterMeasurement {
    /// Classify the severity of the measured wow/flutter.
    #[must_use]
    pub fn classify(&self) -> FlutterSeverity {
        match self.depth_pct {
            d if d < 0.1 => FlutterSeverity::Excellent,
            d if d < 0.3 => FlutterSeverity::Good,
            d if d < 1.0 => FlutterSeverity::Fair,
            _ => FlutterSeverity::Poor,
        }
    }
}

/// Detects wow and flutter via frame-wise instantaneous frequency estimation.
pub struct FlutterDetector;

impl FlutterDetector {
    /// Estimate the instantaneous frequency per frame using short-time
    /// autocorrelation.
    ///
    /// Returns a vector with one frequency estimate (Hz) per analysis frame.
    /// The frame step is fixed at 512 samples; frames shorter than the lag
    /// range are skipped.
    ///
    /// # Arguments
    /// * `samples`     - Mono input buffer.
    /// * `sample_rate` - Sample rate in Hz (e.g. 44100).
    #[must_use]
    pub fn detect_instantaneous_frequency(samples: &[f32], sample_rate: u32) -> Vec<f32> {
        let frame_size = 1024_usize;
        let hop = 512_usize;
        let sr = sample_rate as f32;

        // Search lags corresponding to 50 Hz – 8 kHz
        let lag_min = (sr / 8000.0).ceil() as usize;
        let lag_max = (sr / 50.0).ceil() as usize;

        if samples.len() < frame_size || lag_min >= lag_max {
            return Vec::new();
        }

        let mut freqs = Vec::new();
        let mut pos = 0;

        while pos + frame_size <= samples.len() {
            let frame = &samples[pos..pos + frame_size];

            // Short-time autocorrelation
            let mut best_lag = lag_min;
            let mut best_corr = f32::NEG_INFINITY;

            let max_lag = lag_max.min(frame_size - 1);
            for lag in lag_min..=max_lag {
                let corr: f32 = frame[..frame_size - lag]
                    .iter()
                    .zip(&frame[lag..])
                    .map(|(&a, &b)| a * b)
                    .sum();
                if corr > best_corr {
                    best_corr = corr;
                    best_lag = lag;
                }
            }

            let freq = if best_lag > 0 {
                sr / best_lag as f32
            } else {
                0.0
            };
            freqs.push(freq);
            pos += hop;
        }

        freqs
    }
}

/// Estimates fractional speed variation relative to nominal.
pub struct SpeedVariationEstimator;

impl SpeedVariationEstimator {
    /// Estimate per-frame speed as a fraction of nominal (1.0 = correct speed).
    ///
    /// The method computes the instantaneous frequency via autocorrelation and
    /// divides by the expected `reference_freq`.
    ///
    /// # Arguments
    /// * `samples`       - Mono input buffer.
    /// * `reference_freq`- The expected fundamental frequency (Hz).
    /// * `sample_rate`   - Sample rate in Hz.
    #[must_use]
    pub fn estimate(samples: &[f32], reference_freq: f32, sample_rate: u32) -> Vec<f32> {
        if reference_freq <= 0.0 {
            return Vec::new();
        }
        let inst_freqs = FlutterDetector::detect_instantaneous_frequency(samples, sample_rate);
        inst_freqs
            .iter()
            .map(|&f| if f > 0.0 { f / reference_freq } else { 1.0 })
            .collect()
    }
}

/// Corrects wow and flutter via time-domain resampling.
pub struct FlutterCorrector;

impl FlutterCorrector {
    /// Apply flutter correction to `samples`.
    ///
    /// Uses the [`FlutterMeasurement`] to build a sinusoidal time-warp
    /// function that counteracts the measured frequency modulation, then
    /// resamples accordingly using linear interpolation.
    ///
    /// # Arguments
    /// * `samples`     - Input audio samples.
    /// * `sample_rate` - Sample rate in Hz.
    /// * `measurement` - Measurement from [`FlutterDetector`] or similar.
    #[must_use]
    pub fn correct(
        samples: &[f32],
        sample_rate: u32,
        measurement: &FlutterMeasurement,
    ) -> Vec<f32> {
        let n = samples.len();
        if n == 0 {
            return Vec::new();
        }

        let sr = sample_rate as f32;

        // Build a time-warp map: for each output sample i, the source position
        // is offset by a sinusoidal correction proportional to depth.
        let depth = measurement.depth_pct / 100.0;
        let wow_omega = 2.0 * std::f32::consts::PI * measurement.wow_hz / sr;
        let flutter_omega = 2.0 * std::f32::consts::PI * measurement.flutter_hz / sr;

        let mut output = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32;
            // Correction offset (in samples): counteract by negating measured modulation
            let warp =
                -depth * sr * (wow_omega * t).sin() - depth * sr * 0.5 * (flutter_omega * t).sin();

            let src_pos = (t + warp).max(0.0).min((n - 1) as f32);
            let lo = src_pos.floor() as usize;
            let hi = (lo + 1).min(n - 1);
            let frac = src_pos - lo as f32;
            let val = samples[lo] * (1.0 - frac) + samples[hi] * frac;
            output.push(val);
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn sine(freq_hz: f32, sample_rate: u32, n: usize) -> Vec<f32> {
        let sr = sample_rate as f32;
        (0..n)
            .map(|i| (2.0 * PI * freq_hz * i as f32 / sr).sin())
            .collect()
    }

    #[test]
    fn test_detect_instantaneous_frequency_sine() {
        let sr = 44100u32;
        let samples = sine(440.0, sr, 44100);
        let freqs = FlutterDetector::detect_instantaneous_frequency(&samples, sr);
        assert!(!freqs.is_empty(), "should return frequency estimates");
        // Each estimate should be in a reasonable audio range
        for f in &freqs {
            assert!(*f > 0.0 && *f < 20000.0, "frequency {f} Hz out of range");
        }
    }

    #[test]
    fn test_detect_instantaneous_frequency_empty() {
        let freqs = FlutterDetector::detect_instantaneous_frequency(&[], 44100);
        assert!(freqs.is_empty());
    }

    #[test]
    fn test_detect_instantaneous_frequency_short() {
        let samples = vec![0.0f32; 100];
        let freqs = FlutterDetector::detect_instantaneous_frequency(&samples, 44100);
        assert!(freqs.is_empty());
    }

    #[test]
    fn test_flutter_severity_excellent() {
        let m = FlutterMeasurement {
            wow_hz: 0.5,
            flutter_hz: 10.0,
            depth_pct: 0.05,
            wow_score: 0.1,
            flutter_score: 0.1,
        };
        assert_eq!(m.classify(), FlutterSeverity::Excellent);
    }

    #[test]
    fn test_flutter_severity_good() {
        let m = FlutterMeasurement {
            wow_hz: 1.0,
            flutter_hz: 15.0,
            depth_pct: 0.2,
            wow_score: 0.2,
            flutter_score: 0.2,
        };
        assert_eq!(m.classify(), FlutterSeverity::Good);
    }

    #[test]
    fn test_flutter_severity_fair() {
        let m = FlutterMeasurement {
            wow_hz: 2.0,
            flutter_hz: 20.0,
            depth_pct: 0.7,
            wow_score: 0.5,
            flutter_score: 0.5,
        };
        assert_eq!(m.classify(), FlutterSeverity::Fair);
    }

    #[test]
    fn test_flutter_severity_poor() {
        let m = FlutterMeasurement {
            wow_hz: 3.0,
            flutter_hz: 30.0,
            depth_pct: 2.0,
            wow_score: 0.9,
            flutter_score: 0.9,
        };
        assert_eq!(m.classify(), FlutterSeverity::Poor);
    }

    #[test]
    fn test_flutter_corrector_preserves_length() {
        let sr = 44100u32;
        let samples = sine(440.0, sr, 4410);
        let measurement = FlutterMeasurement {
            wow_hz: 1.0,
            flutter_hz: 10.0,
            depth_pct: 0.3,
            wow_score: 0.3,
            flutter_score: 0.3,
        };
        let corrected = FlutterCorrector::correct(&samples, sr, &measurement);
        assert_eq!(corrected.len(), samples.len());
    }

    #[test]
    fn test_flutter_corrector_empty() {
        let measurement = FlutterMeasurement {
            wow_hz: 1.0,
            flutter_hz: 10.0,
            depth_pct: 0.3,
            wow_score: 0.3,
            flutter_score: 0.3,
        };
        let result = FlutterCorrector::correct(&[], 44100, &measurement);
        assert!(result.is_empty());
    }

    #[test]
    fn test_flutter_corrector_zero_depth_identity() {
        let sr = 44100u32;
        let samples = sine(440.0, sr, 4410);
        let measurement = FlutterMeasurement {
            wow_hz: 1.0,
            flutter_hz: 10.0,
            depth_pct: 0.0, // no modulation
            wow_score: 0.0,
            flutter_score: 0.0,
        };
        let corrected = FlutterCorrector::correct(&samples, sr, &measurement);
        // With zero depth the warp is zero so output ≈ input
        for (a, b) in samples.iter().zip(corrected.iter()) {
            assert!((a - b).abs() < 1e-5, "deviation {}", (a - b).abs());
        }
    }

    #[test]
    fn test_speed_variation_estimator_unity() {
        let sr = 44100u32;
        // Pure 440 Hz sine
        let samples = sine(440.0, sr, 44100);
        let speeds = SpeedVariationEstimator::estimate(&samples, 440.0, sr);
        assert!(!speeds.is_empty());
        // All speed estimates should be positive and finite
        for s in &speeds {
            assert!(
                s.is_finite() && *s > 0.0,
                "speed {s} should be positive and finite"
            );
        }
    }

    #[test]
    fn test_speed_variation_estimator_invalid_ref() {
        let result = SpeedVariationEstimator::estimate(&[0.0; 100], 0.0, 44100);
        assert!(result.is_empty());
    }
}
