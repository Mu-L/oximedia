//! Spectral subtraction for noise reduction.

use crate::error::RestoreResult;
use crate::noise::profile::NoiseProfile;
use crate::utils::spectral::{apply_window, FftProcessor, WindowFunction};

/// Spectral subtraction configuration.
#[derive(Debug, Clone)]
pub struct SpectralSubtractionConfig {
    /// Oversubtraction factor (1.0 = basic subtraction, >1.0 = more aggressive).
    pub oversubtraction_factor: f32,
    /// Spectral floor in dB to prevent musical noise.
    pub spectral_floor_db: f32,
    /// Smoothing factor for spectral subtraction (0.0 to 1.0).
    pub smoothing: f32,
}

impl Default for SpectralSubtractionConfig {
    fn default() -> Self {
        Self {
            oversubtraction_factor: 1.5,
            spectral_floor_db: -40.0,
            smoothing: 0.8,
        }
    }
}

/// Spectral subtraction processor.
#[derive(Debug)]
pub struct SpectralSubtraction {
    config: SpectralSubtractionConfig,
    noise_profile: NoiseProfile,
    fft_size: usize,
    hop_size: usize,
    prev_gain: Vec<f32>,
}

impl SpectralSubtraction {
    /// Create a new spectral subtraction processor.
    ///
    /// # Arguments
    ///
    /// * `noise_profile` - Noise profile to subtract
    /// * `hop_size` - Hop size between frames
    /// * `config` - Configuration
    #[must_use]
    pub fn new(
        noise_profile: NoiseProfile,
        hop_size: usize,
        config: SpectralSubtractionConfig,
    ) -> Self {
        let spectrum_size = noise_profile.fft_size / 2 + 1;
        Self {
            config,
            fft_size: noise_profile.fft_size,
            noise_profile,
            hop_size,
            prev_gain: vec![1.0; spectrum_size],
        }
    }

    /// Process samples to remove noise.
    ///
    /// # Arguments
    ///
    /// * `samples` - Input samples
    ///
    /// # Returns
    ///
    /// Noise-reduced samples.
    pub fn process(&mut self, samples: &[f32]) -> RestoreResult<Vec<f32>> {
        if samples.len() < self.fft_size {
            return Ok(samples.to_vec());
        }

        let fft = FftProcessor::new(self.fft_size);
        let mut output = vec![0.0; samples.len()];
        let mut overlap_count = vec![0.0; samples.len()];

        let spectral_floor = db_to_linear(self.config.spectral_floor_db);

        let mut pos = 0;
        while pos + self.fft_size <= samples.len() {
            // Extract and window frame
            let mut frame = samples[pos..pos + self.fft_size].to_vec();
            apply_window(&mut frame, WindowFunction::Hann);

            // Forward FFT
            let spectrum = fft.forward(&frame)?;
            let magnitude = fft.magnitude(&spectrum);
            let phase = fft.phase(&spectrum);

            // Spectral subtraction
            let mut processed_mag = vec![0.0; magnitude.len()];

            for (i, (&mag, &noise_mag)) in magnitude
                .iter()
                .zip(self.noise_profile.magnitude.iter())
                .enumerate()
            {
                // Subtract noise with oversubtraction factor
                let subtracted = mag - self.config.oversubtraction_factor * noise_mag;

                // Apply spectral floor
                let floored = subtracted.max(spectral_floor * mag);

                // Compute gain
                let gain = if mag > f32::EPSILON {
                    floored / mag
                } else {
                    0.0
                };

                // Smooth gain over time to reduce musical noise
                let smoothed_gain = self.config.smoothing * self.prev_gain[i]
                    + (1.0 - self.config.smoothing) * gain;
                self.prev_gain[i] = smoothed_gain;

                processed_mag[i] = mag * smoothed_gain;
            }

            // Reconstruct complex spectrum
            let processed_spectrum = FftProcessor::from_polar(&processed_mag, &phase)?;

            // Inverse FFT
            let processed_frame = fft.inverse(&processed_spectrum)?;

            // Apply window again and overlap-add
            let mut windowed = processed_frame;
            apply_window(&mut windowed, WindowFunction::Hann);

            for (i, &sample) in windowed.iter().enumerate() {
                output[pos + i] += sample;
                overlap_count[pos + i] += 1.0;
            }

            pos += self.hop_size;
        }

        // Normalize by overlap count
        for (i, &count) in overlap_count.iter().enumerate() {
            if count > 0.0 {
                output[i] /= count;
            }
        }

        Ok(output)
    }

    /// Reset processor state.
    pub fn reset(&mut self) {
        self.prev_gain.fill(1.0);
    }
}

/// Adaptive spectral subtraction with VAD.
#[derive(Debug)]
pub struct AdaptiveSpectralSubtraction {
    config: SpectralSubtractionConfig,
    noise_profile: NoiseProfile,
    fft_size: usize,
    hop_size: usize,
    prev_gain: Vec<f32>,
    vad_threshold: f32,
}

impl AdaptiveSpectralSubtraction {
    /// Create a new adaptive spectral subtraction processor.
    #[must_use]
    pub fn new(
        noise_profile: NoiseProfile,
        hop_size: usize,
        config: SpectralSubtractionConfig,
        vad_threshold: f32,
    ) -> Self {
        let spectrum_size = noise_profile.fft_size / 2 + 1;
        Self {
            config,
            fft_size: noise_profile.fft_size,
            noise_profile,
            hop_size,
            prev_gain: vec![1.0; spectrum_size],
            vad_threshold,
        }
    }

    /// Process samples with adaptive noise profile update.
    pub fn process(&mut self, samples: &[f32]) -> RestoreResult<Vec<f32>> {
        if samples.len() < self.fft_size {
            return Ok(samples.to_vec());
        }

        let fft = FftProcessor::new(self.fft_size);
        let mut output = vec![0.0; samples.len()];
        let mut overlap_count = vec![0.0; samples.len()];

        let spectral_floor = db_to_linear(self.config.spectral_floor_db);

        let mut pos = 0;
        while pos + self.fft_size <= samples.len() {
            let mut frame = samples[pos..pos + self.fft_size].to_vec();
            apply_window(&mut frame, WindowFunction::Hann);

            let spectrum = fft.forward(&frame)?;
            let magnitude = fft.magnitude(&spectrum);
            let phase = fft.phase(&spectrum);

            // Simple VAD: check frame energy
            let energy: f32 = magnitude.iter().map(|&m| m * m).sum();
            let is_speech = energy > self.vad_threshold;

            // Update noise profile during non-speech
            if !is_speech {
                let alpha = 0.05; // Update rate
                for (i, &mag) in magnitude.iter().enumerate() {
                    if i < self.noise_profile.magnitude.len() {
                        self.noise_profile.magnitude[i] =
                            alpha * mag + (1.0 - alpha) * self.noise_profile.magnitude[i];
                    }
                }
            }

            // Spectral subtraction
            let mut processed_mag = vec![0.0; magnitude.len()];

            for (i, (&mag, &noise_mag)) in magnitude
                .iter()
                .zip(self.noise_profile.magnitude.iter())
                .enumerate()
            {
                let subtracted = mag - self.config.oversubtraction_factor * noise_mag;
                let floored = subtracted.max(spectral_floor * mag);

                let gain = if mag > f32::EPSILON {
                    floored / mag
                } else {
                    0.0
                };

                let smoothed_gain = self.config.smoothing * self.prev_gain[i]
                    + (1.0 - self.config.smoothing) * gain;
                self.prev_gain[i] = smoothed_gain;

                processed_mag[i] = mag * smoothed_gain;
            }

            let processed_spectrum = FftProcessor::from_polar(&processed_mag, &phase)?;
            let processed_frame = fft.inverse(&processed_spectrum)?;

            let mut windowed = processed_frame;
            apply_window(&mut windowed, WindowFunction::Hann);

            for (i, &sample) in windowed.iter().enumerate() {
                output[pos + i] += sample;
                overlap_count[pos + i] += 1.0;
            }

            pos += self.hop_size;
        }

        for (i, &count) in overlap_count.iter().enumerate() {
            if count > 0.0 {
                output[i] /= count;
            }
        }

        Ok(output)
    }
}

/// Convert dB to linear scale.
#[must_use]
fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spectral_subtraction() {
        use rand::Rng;
        let mut rng = rand::rng();

        // Create noise profile
        let noise_samples: Vec<f32> = (0..8192).map(|_| rng.random_range(-0.1..0.1)).collect();
        let profile =
            NoiseProfile::learn(&noise_samples, 2048, 1024).expect("should succeed in test");

        // Create noisy signal
        let mut signal: Vec<f32> = (0..8192)
            .map(|i| {
                use std::f32::consts::PI;
                (2.0 * PI * 440.0 * i as f32 / 44100.0).sin()
            })
            .collect();

        for i in 0..signal.len() {
            signal[i] += rng.random_range(-0.1..0.1);
        }

        let mut processor =
            SpectralSubtraction::new(profile, 1024, SpectralSubtractionConfig::default());
        let output = processor.process(&signal).expect("should succeed in test");

        assert_eq!(output.len(), signal.len());
    }

    #[test]
    fn test_adaptive_spectral_subtraction() {
        use rand::Rng;
        let mut rng = rand::rng();

        let noise_samples: Vec<f32> = (0..8192).map(|_| rng.random_range(-0.1..0.1)).collect();
        let profile =
            NoiseProfile::learn(&noise_samples, 2048, 1024).expect("should succeed in test");

        let signal: Vec<f32> = (0..8192).map(|_| rng.random_range(-0.2..0.2)).collect();

        let mut processor = AdaptiveSpectralSubtraction::new(
            profile,
            1024,
            SpectralSubtractionConfig::default(),
            0.1,
        );
        let output = processor.process(&signal).expect("should succeed in test");

        assert_eq!(output.len(), signal.len());
    }

    #[test]
    fn test_db_to_linear() {
        assert!((db_to_linear(0.0) - 1.0).abs() < 1e-5);
        assert!((db_to_linear(-20.0) - 0.1).abs() < 1e-3);
    }

    #[test]
    fn test_config_default() {
        let config = SpectralSubtractionConfig::default();
        assert!(config.oversubtraction_factor > 1.0);
        assert!(config.spectral_floor_db < 0.0);
    }
}
