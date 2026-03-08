//! Wiener filtering for noise reduction.

use crate::error::RestoreResult;
use crate::noise::profile::NoiseProfile;
use crate::utils::spectral::{apply_window, FftProcessor, WindowFunction};

/// Wiener filter configuration.
#[derive(Debug, Clone)]
pub struct WienerFilterConfig {
    /// Minimum gain to apply (prevents over-suppression).
    pub min_gain: f32,
    /// Smoothing factor for gain estimates (0.0 to 1.0).
    pub smoothing: f32,
}

impl Default for WienerFilterConfig {
    fn default() -> Self {
        Self {
            min_gain: 0.01,
            smoothing: 0.9,
        }
    }
}

/// Wiener filter for noise reduction.
#[derive(Debug)]
pub struct WienerFilter {
    config: WienerFilterConfig,
    noise_profile: NoiseProfile,
    fft_size: usize,
    hop_size: usize,
    prev_gain: Vec<f32>,
}

impl WienerFilter {
    /// Create a new Wiener filter.
    ///
    /// # Arguments
    ///
    /// * `noise_profile` - Noise profile
    /// * `hop_size` - Hop size between frames
    /// * `config` - Configuration
    #[must_use]
    pub fn new(noise_profile: NoiseProfile, hop_size: usize, config: WienerFilterConfig) -> Self {
        let spectrum_size = noise_profile.fft_size / 2 + 1;
        Self {
            config,
            fft_size: noise_profile.fft_size,
            noise_profile,
            hop_size,
            prev_gain: vec![1.0; spectrum_size],
        }
    }

    /// Process samples using Wiener filtering.
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

        let mut pos = 0;
        while pos + self.fft_size <= samples.len() {
            // Extract and window frame
            let mut frame = samples[pos..pos + self.fft_size].to_vec();
            apply_window(&mut frame, WindowFunction::Hann);

            // Forward FFT
            let spectrum = fft.forward(&frame)?;
            let magnitude = fft.magnitude(&spectrum);
            let phase = fft.phase(&spectrum);

            // Compute Wiener filter gains
            let mut processed_mag = vec![0.0; magnitude.len()];

            for (i, (&signal_mag, &noise_mag)) in magnitude
                .iter()
                .zip(self.noise_profile.magnitude.iter())
                .enumerate()
            {
                // Estimate SNR
                let signal_power = signal_mag * signal_mag;
                let noise_power = noise_mag * noise_mag;

                // Wiener gain: SNR / (SNR + 1)
                let snr = if noise_power > f32::EPSILON {
                    signal_power / noise_power
                } else {
                    100.0 // Very high SNR if no noise
                };

                let gain = (snr / (snr + 1.0)).max(self.config.min_gain);

                // Smooth gain over time
                let smoothed_gain = self.config.smoothing * self.prev_gain[i]
                    + (1.0 - self.config.smoothing) * gain;
                self.prev_gain[i] = smoothed_gain;

                processed_mag[i] = signal_mag * smoothed_gain;
            }

            // Reconstruct complex spectrum
            let processed_spectrum = FftProcessor::from_polar(&processed_mag, &phase)?;

            // Inverse FFT
            let processed_frame = fft.inverse(&processed_spectrum)?;

            // Apply window and overlap-add
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

    /// Reset filter state.
    pub fn reset(&mut self) {
        self.prev_gain.fill(1.0);
    }
}

/// MMSE (Minimum Mean Square Error) Wiener filter.
///
/// More sophisticated than basic Wiener filter, uses a priori SNR estimation.
#[derive(Debug)]
pub struct MmseFilter {
    noise_profile: NoiseProfile,
    fft_size: usize,
    hop_size: usize,
    min_gain: f32,
    smoothing: f32,
    prev_gain: Vec<f32>,
    prev_snr: Vec<f32>,
}

impl MmseFilter {
    /// Create a new MMSE filter.
    #[must_use]
    pub fn new(
        noise_profile: NoiseProfile,
        hop_size: usize,
        min_gain: f32,
        smoothing: f32,
    ) -> Self {
        let spectrum_size = noise_profile.fft_size / 2 + 1;
        Self {
            fft_size: noise_profile.fft_size,
            noise_profile,
            hop_size,
            min_gain,
            smoothing,
            prev_gain: vec![1.0; spectrum_size],
            prev_snr: vec![1.0; spectrum_size],
        }
    }

    /// Process samples using MMSE filtering.
    pub fn process(&mut self, samples: &[f32]) -> RestoreResult<Vec<f32>> {
        if samples.len() < self.fft_size {
            return Ok(samples.to_vec());
        }

        let fft = FftProcessor::new(self.fft_size);
        let mut output = vec![0.0; samples.len()];
        let mut overlap_count = vec![0.0; samples.len()];

        let mut pos = 0;
        while pos + self.fft_size <= samples.len() {
            let mut frame = samples[pos..pos + self.fft_size].to_vec();
            apply_window(&mut frame, WindowFunction::Hann);

            let spectrum = fft.forward(&frame)?;
            let magnitude = fft.magnitude(&spectrum);
            let phase = fft.phase(&spectrum);

            let mut processed_mag = vec![0.0; magnitude.len()];

            for (i, (&signal_mag, &noise_mag)) in magnitude
                .iter()
                .zip(self.noise_profile.magnitude.iter())
                .enumerate()
            {
                let signal_power = signal_mag * signal_mag;
                let noise_power = noise_mag * noise_mag;

                // A posteriori SNR
                let gamma = if noise_power > f32::EPSILON {
                    signal_power / noise_power
                } else {
                    100.0
                };

                // A priori SNR (using decision-directed approach)
                let xi = self.smoothing * self.prev_gain[i].powi(2) * self.prev_snr[i]
                    + (1.0 - self.smoothing) * (gamma - 1.0).max(0.0);

                self.prev_snr[i] = xi;

                // MMSE gain function
                let gain = if xi > f32::EPSILON {
                    (xi / (1.0 + xi)).sqrt()
                } else {
                    self.min_gain
                };

                let clamped_gain = gain.max(self.min_gain);
                self.prev_gain[i] = clamped_gain;

                processed_mag[i] = signal_mag * clamped_gain;
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

    /// Reset filter state.
    pub fn reset(&mut self) {
        self.prev_gain.fill(1.0);
        self.prev_snr.fill(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wiener_filter() {
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

        let mut filter = WienerFilter::new(profile, 1024, WienerFilterConfig::default());
        let output = filter.process(&signal).expect("should succeed in test");

        assert_eq!(output.len(), signal.len());
    }

    #[test]
    fn test_mmse_filter() {
        use rand::Rng;
        let mut rng = rand::rng();

        let noise_samples: Vec<f32> = (0..8192).map(|_| rng.random_range(-0.1..0.1)).collect();
        let profile =
            NoiseProfile::learn(&noise_samples, 2048, 1024).expect("should succeed in test");

        let signal: Vec<f32> = (0..8192).map(|_| rng.random_range(-0.2..0.2)).collect();

        let mut filter = MmseFilter::new(profile, 1024, 0.01, 0.9);
        let output = filter.process(&signal).expect("should succeed in test");

        assert_eq!(output.len(), signal.len());
    }

    #[test]
    fn test_reset() {
        use rand::Rng;
        let mut rng = rand::rng();

        let noise_samples: Vec<f32> = (0..8192).map(|_| rng.random_range(-0.1..0.1)).collect();
        let profile =
            NoiseProfile::learn(&noise_samples, 2048, 1024).expect("should succeed in test");

        let mut filter = WienerFilter::new(profile, 1024, WienerFilterConfig::default());
        let samples = vec![0.5; 4096];
        let _ = filter.process(&samples).expect("should succeed in test");

        filter.reset();
        assert!(filter.prev_gain.iter().all(|&g| (g - 1.0).abs() < 1e-6));
    }

    #[test]
    fn test_config_default() {
        let config = WienerFilterConfig::default();
        assert!(config.min_gain > 0.0 && config.min_gain < 1.0);
        assert!(config.smoothing >= 0.0 && config.smoothing <= 1.0);
    }
}
