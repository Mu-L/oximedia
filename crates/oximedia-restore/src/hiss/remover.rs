//! Hiss removal using spectral gating.

use crate::error::RestoreResult;
use crate::utils::spectral::{apply_window, FftProcessor, WindowFunction};

/// Hiss remover configuration.
#[derive(Debug, Clone)]
pub struct HissRemoverConfig {
    /// High-pass filter frequency (Hz).
    pub highpass_freq: f32,
    /// Threshold for hiss gating (dB).
    pub threshold_db: f32,
    /// Reduction amount (dB).
    pub reduction_db: f32,
}

impl Default for HissRemoverConfig {
    fn default() -> Self {
        Self {
            highpass_freq: 4000.0,
            threshold_db: -50.0,
            reduction_db: -60.0,
        }
    }
}

/// Hiss remover.
#[derive(Debug)]
pub struct HissRemover {
    config: HissRemoverConfig,
    fft_size: usize,
    hop_size: usize,
}

impl HissRemover {
    /// Create a new hiss remover.
    #[must_use]
    pub fn new(config: HissRemoverConfig, fft_size: usize, hop_size: usize) -> Self {
        Self {
            config,
            fft_size,
            hop_size,
        }
    }

    /// Remove hiss from samples.
    pub fn process(&self, samples: &[f32], sample_rate: u32) -> RestoreResult<Vec<f32>> {
        if samples.len() < self.fft_size {
            return Ok(samples.to_vec());
        }

        let fft = FftProcessor::new(self.fft_size);

        let mut output = vec![0.0; samples.len()];
        let mut overlap_count = vec![0.0; samples.len()];

        #[allow(clippy::cast_precision_loss)]
        let bin_width = sample_rate as f32 / self.fft_size as f32;
        let highpass_bin = (self.config.highpass_freq / bin_width) as usize;

        let mut pos = 0;
        while pos + self.fft_size <= samples.len() {
            let mut frame = samples[pos..pos + self.fft_size].to_vec();
            apply_window(&mut frame, WindowFunction::Hann);

            let spectrum = fft.forward(&frame)?;
            let mut magnitude = fft.magnitude(&spectrum);
            let phase = fft.phase(&spectrum);

            // Apply spectral gating only to high frequencies
            for i in highpass_bin..magnitude.len() {
                magnitude[i] *= db_to_linear(self.config.reduction_db);
            }

            let processed_spectrum = FftProcessor::from_polar(&magnitude, &phase)?;
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

#[must_use]
fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hiss_remover() {
        use rand::Rng;
        let mut rng = rand::rng();

        let samples: Vec<f32> = (0..8192).map(|_| rng.random_range(-0.1..0.1)).collect();

        let remover = HissRemover::new(HissRemoverConfig::default(), 2048, 1024);
        let output = remover
            .process(&samples, 44100)
            .expect("should succeed in test");

        assert_eq!(output.len(), samples.len());
    }
}
