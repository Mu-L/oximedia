//! Channel vocoder implementation.

use crate::{
    filter::{FilterMode, StateVariableConfig, StateVariableFilter},
    utils::EnvelopeFollower,
    AudioEffect,
};

/// Vocoder configuration.
#[derive(Debug, Clone)]
pub struct VocoderConfig {
    /// Number of frequency bands.
    pub bands: usize,
    /// Attack time in milliseconds.
    pub attack_ms: f32,
    /// Release time in milliseconds.
    pub release_ms: f32,
}

impl Default for VocoderConfig {
    fn default() -> Self {
        Self {
            bands: 16,
            attack_ms: 5.0,
            release_ms: 50.0,
        }
    }
}

/// Vocoder band.
struct VocoderBand {
    modulator_filter: StateVariableFilter,
    carrier_filter: StateVariableFilter,
    envelope: EnvelopeFollower,
}

/// Channel vocoder effect.
///
/// Imposes the spectral characteristics of one signal (modulator)
/// onto another (carrier).
pub struct Vocoder {
    bands: Vec<VocoderBand>,
    #[allow(dead_code)]
    config: VocoderConfig,
}

impl Vocoder {
    /// Create new vocoder.
    #[must_use]
    pub fn new(config: VocoderConfig, sample_rate: f32) -> Self {
        let num_bands = config.bands.clamp(4, 32);

        // Create frequency bands logarithmically spaced
        let min_freq = 100.0_f32;
        let max_freq = 8000.0_f32;

        let bands: Vec<VocoderBand> = (0..num_bands)
            .map(|i| {
                #[allow(clippy::cast_precision_loss)]
                let ratio = i as f32 / (num_bands - 1) as f32;
                let frequency = min_freq * (max_freq / min_freq).powf(ratio);

                let filter_config = StateVariableConfig {
                    frequency,
                    resonance: 2.0,
                    mode: FilterMode::BandPass,
                };

                VocoderBand {
                    modulator_filter: StateVariableFilter::new(filter_config.clone(), sample_rate),
                    carrier_filter: StateVariableFilter::new(filter_config, sample_rate),
                    envelope: EnvelopeFollower::new(
                        config.attack_ms,
                        config.release_ms,
                        sample_rate,
                    ),
                }
            })
            .collect();

        Self { bands, config }
    }

    /// Process modulator and carrier signals.
    pub fn process(&mut self, modulator: f32, carrier: f32) -> f32 {
        let mut output = 0.0;

        #[allow(clippy::cast_precision_loss)]
        let num_bands = self.bands.len() as f32;

        for band in &mut self.bands {
            // Filter modulator and detect envelope
            let mod_filtered = band.modulator_filter.process_sample(modulator);
            let envelope = band.envelope.process(mod_filtered);

            // Filter carrier and apply envelope
            let car_filtered = band.carrier_filter.process_sample(carrier);
            output += car_filtered * envelope;
        }

        output / num_bands
    }
}

impl AudioEffect for Vocoder {
    fn process_sample(&mut self, input: f32) -> f32 {
        // For mono processing, use input as both modulator and carrier
        self.process(input, input)
    }

    fn reset(&mut self) {
        for band in &mut self.bands {
            band.modulator_filter.reset();
            band.carrier_filter.reset();
            band.envelope.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vocoder() {
        let config = VocoderConfig::default();
        let mut vocoder = Vocoder::new(config, 48000.0);

        let output = vocoder.process(0.5, 0.3);
        assert!(output.is_finite());
    }

    #[test]
    fn test_vocoder_bands() {
        let config = VocoderConfig {
            bands: 8,
            ..Default::default()
        };
        let vocoder = Vocoder::new(config, 48000.0);
        assert_eq!(vocoder.bands.len(), 8);
    }
}
