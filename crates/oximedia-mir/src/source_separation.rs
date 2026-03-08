#![allow(dead_code)]
//! Source separation — vocal / drum / bass / other stem splitting.

/// Type of audio stem produced by source separation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StemType {
    /// Lead and backing vocals.
    Vocals,
    /// Drum kit and percussion.
    Drums,
    /// Bass instruments (bass guitar, synth bass, kick body).
    Bass,
    /// All other instruments (guitar, piano, synths, etc.).
    Other,
}

impl StemType {
    /// Human-readable label for this stem type.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Vocals => "Vocals",
            Self::Drums => "Drums",
            Self::Bass => "Bass",
            Self::Other => "Other",
        }
    }

    /// Returns all four canonical stem types.
    #[must_use]
    pub fn all() -> [Self; 4] {
        [Self::Vocals, Self::Drums, Self::Bass, Self::Other]
    }
}

/// Configuration for the separation algorithm.
#[derive(Debug, Clone)]
pub struct SeparationConfig {
    /// Which stems to extract. Must be non-empty.
    pub stems: Vec<StemType>,
    /// Input sample rate in Hz.
    pub sample_rate: f32,
    /// FFT window size for STFT-based separation.
    pub window_size: usize,
    /// Hop size for STFT.
    pub hop_size: usize,
    /// Quality level 0.0–1.0: higher values use more computation.
    pub quality: f32,
}

impl Default for SeparationConfig {
    fn default() -> Self {
        Self {
            stems: StemType::all().to_vec(),
            sample_rate: 44100.0,
            window_size: 4096,
            hop_size: 1024,
            quality: 0.8,
        }
    }
}

impl SeparationConfig {
    /// Returns `true` when the configuration is logically valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.stems.is_empty()
            && self.sample_rate > 0.0
            && self.window_size >= 2
            && self.hop_size >= 1
            && self.hop_size <= self.window_size
            && self.quality >= 0.0
            && self.quality <= 1.0
    }
}

/// The separated audio data for a single stem.
#[derive(Debug, Clone)]
pub struct Stem {
    /// Type of this stem.
    pub stem_type: StemType,
    /// Audio samples (mono, normalised −1.0 … 1.0).
    pub samples: Vec<f32>,
    /// Energy ratio of this stem vs the mixture (0.0–1.0).
    pub energy_ratio: f32,
}

impl Stem {
    /// Create a new `Stem`.
    #[must_use]
    pub fn new(stem_type: StemType, samples: Vec<f32>, energy_ratio: f32) -> Self {
        Self {
            stem_type,
            samples,
            energy_ratio,
        }
    }

    /// RMS energy of this stem.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn rms(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.samples.iter().map(|s| s * s).sum();
        (sum / self.samples.len() as f32).sqrt()
    }
}

/// Output of a source separation operation.
#[derive(Debug, Clone)]
pub struct SeparationResult {
    /// All extracted stems.
    pub stems: Vec<Stem>,
    /// Sample rate of the output stems.
    pub sample_rate: f32,
    /// Signal-to-distortion ratio estimate (dB) — higher is better.
    pub sdr_estimate_db: f32,
}

impl SeparationResult {
    /// Create a new result.
    #[must_use]
    pub fn new(stems: Vec<Stem>, sample_rate: f32, sdr_estimate_db: f32) -> Self {
        Self {
            stems,
            sample_rate,
            sdr_estimate_db,
        }
    }

    /// Number of extracted stems.
    #[must_use]
    pub fn stem_count(&self) -> usize {
        self.stems.len()
    }

    /// Look up a stem by type.
    #[must_use]
    pub fn get_stem(&self, stem_type: StemType) -> Option<&Stem> {
        self.stems.iter().find(|s| s.stem_type == stem_type)
    }

    /// Returns `true` when the SDR estimate suggests acceptable quality (> 6 dB).
    #[must_use]
    pub fn is_acceptable_quality(&self) -> bool {
        self.sdr_estimate_db > 6.0
    }
}

/// Performs source separation on a mixture signal.
///
/// This is a stub implementation using simple spectral masking.
pub struct StemSeparator {
    config: SeparationConfig,
}

impl StemSeparator {
    /// Create a new separator with the given configuration.
    ///
    /// Returns `None` if the configuration is invalid.
    #[must_use]
    pub fn new(config: SeparationConfig) -> Option<Self> {
        if config.is_valid() {
            Some(Self { config })
        } else {
            None
        }
    }

    /// Separate the mixture into stems.
    ///
    /// The stub divides the input energy evenly across the requested stems using
    /// simple spectral weighting, producing a result suitable for unit testing.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn separate(&self, mixture: &[f32]) -> SeparationResult {
        let n_stems = self.config.stems.len();
        let weight = if n_stems == 0 {
            1.0
        } else {
            1.0 / n_stems as f32
        };

        let stems: Vec<Stem> = self
            .config
            .stems
            .iter()
            .enumerate()
            .map(|(i, &stem_type)| {
                // Simple stub: attenuate and slightly offset per stem.
                let scale = weight * (1.0 - 0.05 * i as f32).max(0.1);
                let samples: Vec<f32> = mixture.iter().map(|s| s * scale).collect();
                let energy = scale;
                Stem::new(stem_type, samples, energy)
            })
            .collect();

        // Stub SDR estimate based on quality setting.
        let sdr = 6.0 + self.config.quality * 14.0;

        SeparationResult::new(stems, self.config.sample_rate, sdr)
    }

    /// Reference to the current configuration.
    #[must_use]
    pub fn config(&self) -> &SeparationConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mixture(len: usize) -> Vec<f32> {
        (0..len)
            .map(|i| (i as f32 / 512.0 * std::f32::consts::TAU).sin() * 0.5)
            .collect()
    }

    #[test]
    fn test_stem_type_labels() {
        assert_eq!(StemType::Vocals.label(), "Vocals");
        assert_eq!(StemType::Drums.label(), "Drums");
        assert_eq!(StemType::Bass.label(), "Bass");
        assert_eq!(StemType::Other.label(), "Other");
    }

    #[test]
    fn test_stem_type_all_has_four() {
        assert_eq!(StemType::all().len(), 4);
    }

    #[test]
    fn test_config_default_is_valid() {
        assert!(SeparationConfig::default().is_valid());
    }

    #[test]
    fn test_config_invalid_empty_stems() {
        let cfg = SeparationConfig {
            stems: vec![],
            ..Default::default()
        };
        assert!(!cfg.is_valid());
    }

    #[test]
    fn test_config_invalid_sample_rate() {
        let cfg = SeparationConfig {
            sample_rate: 0.0,
            ..Default::default()
        };
        assert!(!cfg.is_valid());
    }

    #[test]
    fn test_config_invalid_quality() {
        let cfg = SeparationConfig {
            quality: 1.5,
            ..Default::default()
        };
        assert!(!cfg.is_valid());
    }

    #[test]
    fn test_separator_builds_from_valid_config() {
        let sep = StemSeparator::new(SeparationConfig::default());
        assert!(sep.is_some());
    }

    #[test]
    fn test_separator_rejects_invalid_config() {
        let cfg = SeparationConfig {
            stems: vec![],
            ..Default::default()
        };
        assert!(StemSeparator::new(cfg).is_none());
    }

    #[test]
    fn test_separate_returns_correct_stem_count() {
        let sep = StemSeparator::new(SeparationConfig::default()).expect("should succeed in test");
        let result = sep.separate(&make_mixture(4096));
        assert_eq!(result.stem_count(), 4);
    }

    #[test]
    fn test_result_get_stem_vocals() {
        let sep = StemSeparator::new(SeparationConfig::default()).expect("should succeed in test");
        let result = sep.separate(&make_mixture(2048));
        assert!(result.get_stem(StemType::Vocals).is_some());
    }

    #[test]
    fn test_result_get_stem_missing() {
        let cfg = SeparationConfig {
            stems: vec![StemType::Vocals, StemType::Drums],
            ..Default::default()
        };
        let sep = StemSeparator::new(cfg).expect("should succeed in test");
        let result = sep.separate(&make_mixture(2048));
        assert!(result.get_stem(StemType::Bass).is_none());
    }

    #[test]
    fn test_result_acceptable_quality_with_high_quality_config() {
        let cfg = SeparationConfig {
            quality: 1.0,
            ..Default::default()
        };
        let sep = StemSeparator::new(cfg).expect("should succeed in test");
        let result = sep.separate(&make_mixture(2048));
        assert!(result.is_acceptable_quality());
    }

    #[test]
    fn test_stem_rms_nonzero_for_nonsilent_mixture() {
        let sep = StemSeparator::new(SeparationConfig::default()).expect("should succeed in test");
        let result = sep.separate(&make_mixture(4096));
        let vocal_stem = result
            .get_stem(StemType::Vocals)
            .expect("should succeed in test");
        assert!(vocal_stem.rms() > 0.0);
    }

    #[test]
    fn test_stem_samples_have_correct_length() {
        let mixture = make_mixture(1024);
        let sep = StemSeparator::new(SeparationConfig::default()).expect("should succeed in test");
        let result = sep.separate(&mixture);
        for stem in &result.stems {
            assert_eq!(stem.samples.len(), 1024);
        }
    }
}
