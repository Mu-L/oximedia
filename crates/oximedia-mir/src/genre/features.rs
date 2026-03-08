//! Genre-specific feature extraction.

use crate::utils::{mean, std_dev, stft};
use crate::MirResult;

/// Features for genre classification.
#[derive(Debug, Clone)]
pub struct GenreFeatureVector {
    /// Average spectral centroid.
    pub spectral_centroid: f32,
    /// Spectral bandwidth.
    pub spectral_bandwidth: f32,
    /// Zero crossing rate.
    pub zero_crossing_rate: f32,
    /// Overall energy.
    pub energy: f32,
    /// Energy variance.
    pub energy_variance: f32,
    /// Estimated tempo.
    pub tempo: f32,
    /// Beat strength.
    pub beat_strength: f32,
    /// Harmonic complexity.
    pub harmonic_complexity: f32,
}

/// Genre feature extractor.
pub struct GenreFeatures {
    #[allow(dead_code)]
    sample_rate: f32,
}

impl GenreFeatures {
    /// Create a new genre feature extractor.
    #[must_use]
    pub fn new(sample_rate: f32) -> Self {
        Self { sample_rate }
    }

    /// Extract features from audio signal.
    ///
    /// # Errors
    ///
    /// Returns error if feature extraction fails.
    #[allow(clippy::cast_precision_loss)]
    pub fn extract(&self, signal: &[f32]) -> MirResult<GenreFeatureVector> {
        let window_size = 2048;
        let hop_size = 512;

        let frames = stft(signal, window_size, hop_size)?;

        let mut spectral_centroids = Vec::new();
        let mut spectral_bandwidths = Vec::new();
        let mut energies = Vec::new();

        for frame in &frames {
            let mag = crate::utils::magnitude_spectrum(frame);

            let centroid = self.compute_spectral_centroid(&mag);
            let bandwidth = self.compute_spectral_bandwidth(&mag, centroid);
            let energy = mag.iter().map(|m| m * m).sum::<f32>();

            spectral_centroids.push(centroid);
            spectral_bandwidths.push(bandwidth);
            energies.push(energy);
        }

        let spectral_centroid = mean(&spectral_centroids);
        let spectral_bandwidth = mean(&spectral_bandwidths);
        let energy = mean(&energies);
        let energy_variance = std_dev(&energies);

        let zero_crossing_rate = self.compute_zero_crossing_rate(signal);

        // Simplified tempo and beat strength
        let tempo = 120.0; // Would use actual tempo detection
        let beat_strength = 0.5; // Would use actual beat detection

        let harmonic_complexity = spectral_bandwidth / (spectral_centroid + 1.0);

        Ok(GenreFeatureVector {
            spectral_centroid,
            spectral_bandwidth,
            zero_crossing_rate,
            energy,
            energy_variance,
            tempo,
            beat_strength,
            harmonic_complexity,
        })
    }

    /// Compute spectral centroid.
    #[allow(clippy::cast_precision_loss)]
    fn compute_spectral_centroid(&self, spectrum: &[f32]) -> f32 {
        let mut weighted_sum = 0.0;
        let mut total = 0.0;

        for (i, &mag) in spectrum.iter().enumerate() {
            weighted_sum += i as f32 * mag;
            total += mag;
        }

        if total > 0.0 {
            weighted_sum / total
        } else {
            0.0
        }
    }

    /// Compute spectral bandwidth.
    #[allow(clippy::cast_precision_loss)]
    fn compute_spectral_bandwidth(&self, spectrum: &[f32], centroid: f32) -> f32 {
        let mut weighted_sum = 0.0;
        let mut total = 0.0;

        for (i, &mag) in spectrum.iter().enumerate() {
            let diff = i as f32 - centroid;
            weighted_sum += diff * diff * mag;
            total += mag;
        }

        if total > 0.0 {
            (weighted_sum / total).sqrt()
        } else {
            0.0
        }
    }

    /// Compute zero crossing rate.
    #[allow(clippy::cast_precision_loss)]
    fn compute_zero_crossing_rate(&self, signal: &[f32]) -> f32 {
        let mut crossings = 0;

        for i in 1..signal.len() {
            if (signal[i] >= 0.0 && signal[i - 1] < 0.0)
                || (signal[i] < 0.0 && signal[i - 1] >= 0.0)
            {
                crossings += 1;
            }
        }

        crossings as f32 / signal.len() as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genre_features_creation() {
        let features = GenreFeatures::new(44100.0);
        assert_eq!(features.sample_rate, 44100.0);
    }

    #[test]
    fn test_zero_crossing_rate() {
        let features = GenreFeatures::new(44100.0);
        let signal = vec![1.0, -1.0, 1.0, -1.0, 1.0];
        let zcr = features.compute_zero_crossing_rate(&signal);
        assert!(zcr > 0.5);
    }
}
