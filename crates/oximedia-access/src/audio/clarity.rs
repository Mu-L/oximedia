//! Audio clarity enhancement for hearing impaired users.

use crate::error::AccessResult;
use oximedia_audio::frame::AudioBuffer;

/// Speech intelligibility metrics.
#[derive(Debug, Clone)]
pub struct SpeechIntelligibilityMetrics {
    /// Signal-to-Noise Ratio in dB.
    pub snr_db: f32,
    /// Speech Clarity Index (0.0 to 1.0).
    pub clarity_index: f32,
    /// Speech Transmission Index estimate (0.0 to 1.0).
    pub sti_estimate: f32,
    /// Articulation Index (0.0 to 1.0).
    pub articulation_index: f32,
}

impl SpeechIntelligibilityMetrics {
    /// Interpret the overall intelligibility quality.
    #[must_use]
    pub fn quality_label(&self) -> &'static str {
        match self.sti_estimate {
            s if s >= 0.75 => "Excellent",
            s if s >= 0.60 => "Good",
            s if s >= 0.45 => "Fair",
            s if s >= 0.30 => "Poor",
            _ => "Bad",
        }
    }

    /// Check whether intelligibility meets broadcast minimum (STI >= 0.5).
    #[must_use]
    pub fn meets_broadcast_minimum(&self) -> bool {
        self.sti_estimate >= 0.50
    }
}

/// Enhances audio clarity for better speech intelligibility.
pub struct AudioClarityEnhancer {
    level: f32,
    /// Band-pass filter centre frequency for speech boost (Hz).
    speech_center_hz: f32,
    /// Width of the speech boost band (Hz).
    speech_band_width_hz: f32,
}

impl AudioClarityEnhancer {
    /// Create a new clarity enhancer.
    #[must_use]
    pub fn new(level: f32) -> Self {
        Self {
            level: level.clamp(0.0, 1.0),
            speech_center_hz: 2000.0,
            speech_band_width_hz: 2700.0,
        }
    }

    /// Enhance audio clarity.
    pub fn enhance(&self, audio: &AudioBuffer) -> AccessResult<AudioBuffer> {
        // In production, this would:
        // 1. Apply dynamic range compression
        // 2. Boost speech frequencies (1-4 kHz)
        // 3. Apply adaptive filtering
        // 4. Reduce masking effects

        Ok(audio.clone())
    }

    /// Enhance speech frequencies specifically.
    pub fn enhance_speech(&self, audio: &AudioBuffer) -> AccessResult<AudioBuffer> {
        // Focus on 300Hz - 3400Hz range for speech
        Ok(audio.clone())
    }

    /// Get enhancement level.
    #[must_use]
    pub const fn level(&self) -> f32 {
        self.level
    }

    /// Calculate SNR from signal and noise power estimates.
    ///
    /// `signal_power` and `noise_power` are mean-square values of the
    /// respective signals (linear, not dB).  Returns `f32::INFINITY` when
    /// noise power is effectively zero.
    #[must_use]
    #[allow(dead_code)]
    pub fn calculate_snr(signal_power: f32, noise_power: f32) -> f32 {
        if noise_power <= f32::EPSILON {
            return f32::INFINITY;
        }
        10.0 * (signal_power / noise_power).log10()
    }

    /// Compute the Speech Clarity Index from per-octave-band SNR values.
    ///
    /// Uses a simplified model where each band is weighted by its contribution
    /// to speech articulation.  `band_snrs` contains (`centre_freq_hz`, `snr_db`)
    /// pairs for octave bands from 125 Hz to 8 kHz.
    #[must_use]
    #[allow(dead_code)]
    pub fn speech_clarity_index(band_snrs: &[(f32, f32)]) -> f32 {
        if band_snrs.is_empty() {
            return 0.0;
        }
        // IEC 60268-16 simplified weights for speech frequencies
        let weights: &[(f32, f32)] = &[
            (125.0, 0.002),
            (250.0, 0.015),
            (500.0, 0.075),
            (1000.0, 0.140),
            (2000.0, 0.200),
            (4000.0, 0.185),
            (8000.0, 0.110),
        ];

        let mut weighted_sum = 0.0_f32;
        let mut total_weight = 0.0_f32;

        for (centre, snr_db) in band_snrs {
            // Find closest octave-band weight
            let w = weights
                .iter()
                .min_by(|(fa, _), (fb, _)| {
                    (fa - centre)
                        .abs()
                        .partial_cmp(&(fb - centre).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map_or(0.0, |(_, w)| *w);

            // Clip SNR to [-15, 15] dB range (IEC 60268-16)
            let clipped = snr_db.clamp(-15.0, 15.0);
            // Normalise to [0, 1]
            let normalised = (clipped + 15.0) / 30.0;

            weighted_sum += w * normalised;
            total_weight += w;
        }

        if total_weight <= 0.0 {
            return 0.0;
        }
        (weighted_sum / total_weight).clamp(0.0, 1.0)
    }

    /// Estimate Speech Transmission Index from band SNR values.
    ///
    /// This is a simplified estimation based on the STI definition
    /// in IEC 60268-16.  `band_snrs` contains (`centre_freq_hz`, `snr_db`)
    /// pairs.
    #[must_use]
    #[allow(dead_code)]
    pub fn estimate_sti(band_snrs: &[(f32, f32)]) -> f32 {
        // Compute apparent SNR per band, clip to [-15, 15] dB
        // and map to Transmission Index: TI = (SNR + 15) / 30
        // STI is the weighted average of TI values across seven octave bands.
        let octave_weights: &[(f32, f32)] = &[
            (125.0, 0.130),
            (250.0, 0.140),
            (500.0, 0.150),
            (1000.0, 0.175),
            (2000.0, 0.175),
            (4000.0, 0.140),
            (8000.0, 0.090),
        ];

        let mut sti = 0.0_f32;
        let mut total_weight = 0.0_f32;

        for (centre_freq, weight) in octave_weights {
            // Find the closest measured band
            if let Some((_, snr_db)) = band_snrs.iter().min_by(|(fa, _), (fb, _)| {
                (fa - centre_freq)
                    .abs()
                    .partial_cmp(&(fb - centre_freq).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            }) {
                let ti = (snr_db.clamp(-15.0, 15.0) + 15.0) / 30.0;
                sti += weight * ti;
                total_weight += weight;
            }
        }

        if total_weight <= 0.0 {
            return 0.0;
        }
        (sti / total_weight).clamp(0.0, 1.0)
    }

    /// Compute speech intelligibility metrics for raw PCM samples.
    ///
    /// `samples` is a slice of f32 PCM samples in the range [-1.0, 1.0].
    /// `noise_floor` is the estimated noise power (mean square).
    /// `sample_rate` is the audio sample rate in Hz.
    ///
    /// Returns `SpeechIntelligibilityMetrics` with all computed values.
    #[must_use]
    pub fn compute_metrics(
        samples: &[f32],
        noise_floor: f32,
        sample_rate: u32,
    ) -> SpeechIntelligibilityMetrics {
        if samples.is_empty() {
            return SpeechIntelligibilityMetrics {
                snr_db: 0.0,
                clarity_index: 0.0,
                sti_estimate: 0.0,
                articulation_index: 0.0,
            };
        }

        // Signal power (mean square)
        let signal_power: f32 = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;

        // SNR in dB
        let snr_db = Self::calculate_snr(signal_power, noise_floor);
        let snr_clipped = snr_db.clamp(-30.0, 60.0);

        // Approximate octave-band SNRs using the wideband SNR as a proxy.
        // In production, a proper filterbank would be used here.
        let nyquist = sample_rate as f32 / 2.0;
        let octave_centres = [125.0_f32, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0];
        let band_snrs: Vec<(f32, f32)> = octave_centres
            .iter()
            .filter(|&&f| f < nyquist)
            .map(|&f| {
                // Speech-critical bands (500–4000 Hz) get a small boost
                let offset = if (500.0..=4000.0).contains(&f) {
                    3.0
                } else {
                    -3.0
                };
                (f, (snr_clipped + offset).clamp(-15.0, 15.0))
            })
            .collect();

        let clarity_index = Self::speech_clarity_index(&band_snrs);
        let sti_estimate = Self::estimate_sti(&band_snrs);

        // Articulation Index (AI): simplified per ANSI S3.5
        // AI = sum over bands of [ weight * fractional_level ]
        let ai = clarity_index * 0.9; // proportional approximation

        SpeechIntelligibilityMetrics {
            snr_db: snr_clipped,
            clarity_index,
            sti_estimate,
            articulation_index: ai,
        }
    }

    /// Get the configured speech centre frequency.
    #[must_use]
    pub const fn speech_center_hz(&self) -> f32 {
        self.speech_center_hz
    }

    /// Get the configured speech band width.
    #[must_use]
    pub const fn speech_band_width_hz(&self) -> f32 {
        self.speech_band_width_hz
    }
}

impl Default for AudioClarityEnhancer {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn test_enhancer_creation() {
        let enhancer = AudioClarityEnhancer::new(0.7);
        assert!((enhancer.level() - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_enhance() {
        let enhancer = AudioClarityEnhancer::default();
        let audio = AudioBuffer::Interleaved(Bytes::from(vec![0u8; 48000 * 4]));
        let result = enhancer.enhance(&audio);
        assert!(result.is_ok());
    }

    #[test]
    fn test_calculate_snr_typical() {
        // 0 dB SNR: signal power == noise power
        let snr = AudioClarityEnhancer::calculate_snr(1.0, 1.0);
        assert!((snr - 0.0).abs() < 1e-4, "Expected 0 dB, got {snr}");
    }

    #[test]
    fn test_calculate_snr_zero_noise() {
        let snr = AudioClarityEnhancer::calculate_snr(1.0, 0.0);
        assert!(snr.is_infinite(), "Expected infinity for zero noise");
    }

    #[test]
    fn test_calculate_snr_positive() {
        // 10 dB SNR: signal 10x noise
        let snr = AudioClarityEnhancer::calculate_snr(10.0, 1.0);
        assert!((snr - 10.0).abs() < 1e-3, "Expected ~10 dB, got {snr}");
    }

    #[test]
    fn test_speech_clarity_index_empty() {
        let ci = AudioClarityEnhancer::speech_clarity_index(&[]);
        assert!((ci - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_speech_clarity_index_high_snr() {
        // All bands at 15 dB (maximum) → CI should approach 1.0
        let bands: Vec<(f32, f32)> = vec![
            (125.0, 15.0),
            (250.0, 15.0),
            (500.0, 15.0),
            (1000.0, 15.0),
            (2000.0, 15.0),
            (4000.0, 15.0),
            (8000.0, 15.0),
        ];
        let ci = AudioClarityEnhancer::speech_clarity_index(&bands);
        assert!(ci > 0.95, "Expected CI close to 1.0, got {ci}");
    }

    #[test]
    fn test_speech_clarity_index_low_snr() {
        // All bands at -15 dB (minimum) → CI should be close to 0.0
        let bands: Vec<(f32, f32)> = vec![
            (125.0, -15.0),
            (250.0, -15.0),
            (500.0, -15.0),
            (1000.0, -15.0),
            (2000.0, -15.0),
            (4000.0, -15.0),
            (8000.0, -15.0),
        ];
        let ci = AudioClarityEnhancer::speech_clarity_index(&bands);
        assert!(ci < 0.05, "Expected CI close to 0.0, got {ci}");
    }

    #[test]
    fn test_estimate_sti_full_range() {
        let high_bands: Vec<(f32, f32)> = vec![
            (125.0, 15.0),
            (250.0, 15.0),
            (500.0, 15.0),
            (1000.0, 15.0),
            (2000.0, 15.0),
            (4000.0, 15.0),
            (8000.0, 15.0),
        ];
        let sti_high = AudioClarityEnhancer::estimate_sti(&high_bands);
        assert!(sti_high > 0.95, "Expected high STI, got {sti_high}");

        let low_bands: Vec<(f32, f32)> = vec![
            (125.0, -15.0),
            (250.0, -15.0),
            (500.0, -15.0),
            (1000.0, -15.0),
            (2000.0, -15.0),
            (4000.0, -15.0),
            (8000.0, -15.0),
        ];
        let sti_low = AudioClarityEnhancer::estimate_sti(&low_bands);
        assert!(sti_low < 0.05, "Expected low STI, got {sti_low}");
    }

    #[test]
    fn test_compute_metrics_silent() {
        let samples = vec![0.0_f32; 4800];
        let m = AudioClarityEnhancer::compute_metrics(&samples, 0.001, 48000);
        // SNR for near-silent signal vs noise_floor = 0.001 should be negative
        assert!(m.snr_db < 0.0 || m.snr_db == -30.0);
    }

    #[test]
    fn test_compute_metrics_sine() {
        // Generate a 1 kHz sine wave at full scale
        let sample_rate = 48000u32;
        let samples: Vec<f32> = (0..sample_rate)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sample_rate as f32).sin())
            .collect();
        let noise_floor = 1e-6_f32;
        let m = AudioClarityEnhancer::compute_metrics(&samples, noise_floor, sample_rate);
        assert!(m.snr_db > 30.0, "Expected high SNR, got {}", m.snr_db);
        assert!(m.clarity_index > 0.5);
        assert!(m.sti_estimate > 0.5);
    }

    #[test]
    fn test_metrics_quality_label() {
        let good = SpeechIntelligibilityMetrics {
            snr_db: 20.0,
            clarity_index: 0.8,
            sti_estimate: 0.75,
            articulation_index: 0.72,
        };
        assert_eq!(good.quality_label(), "Excellent");
        assert!(good.meets_broadcast_minimum());

        let poor = SpeechIntelligibilityMetrics {
            snr_db: -5.0,
            clarity_index: 0.2,
            sti_estimate: 0.35,
            articulation_index: 0.18,
        };
        assert_eq!(poor.quality_label(), "Poor");
        assert!(!poor.meets_broadcast_minimum());
    }

    #[test]
    fn test_enhancer_speech_params() {
        let enhancer = AudioClarityEnhancer::new(0.8);
        assert!((enhancer.speech_center_hz() - 2000.0).abs() < f32::EPSILON);
        assert!(enhancer.speech_band_width_hz() > 0.0);
    }

    #[test]
    fn test_enhance_speech() {
        let enhancer = AudioClarityEnhancer::new(0.6);
        let audio = AudioBuffer::Interleaved(Bytes::from(vec![0u8; 48000 * 4]));
        let result = enhancer.enhance_speech(&audio);
        assert!(result.is_ok());
    }
}
