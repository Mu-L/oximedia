//! Noise type classification.

use crate::spectral::SpectralFeatures;

/// Noise type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseType {
    /// White noise (flat spectrum)
    White,
    /// Pink noise (1/f spectrum)
    Pink,
    /// Brown/red noise (1/f² spectrum)
    Brown,
    /// Environmental noise
    Environmental,
    /// Hum (power line interference)
    Hum,
    /// Unknown/other
    Unknown,
}

/// Classify noise type from spectral features.
///
/// # Arguments
/// * `spectral` - Spectral features of the noise
///
/// # Returns
/// Classified noise type
#[must_use]
pub fn classify_noise(spectral: &SpectralFeatures) -> NoiseType {
    // White noise: flat spectrum (high flatness)
    if spectral.flatness > 0.9 {
        return NoiseType::White;
    }

    // Hum: very low frequency peak
    if spectral.centroid < 100.0 && spectral.flatness < 0.3 {
        return NoiseType::Hum;
    }

    // Analyze spectral slope
    let slope = estimate_spectral_slope(&spectral.magnitude_spectrum);

    // Pink noise: -3 dB/octave slope
    if slope < -2.0 && slope > -4.0 {
        return NoiseType::Pink;
    }

    // Brown noise: -6 dB/octave slope
    if slope < -5.0 && slope > -7.0 {
        return NoiseType::Brown;
    }

    // Environmental noise: irregular spectrum
    if spectral.flatness > 0.3 && spectral.flatness < 0.7 {
        return NoiseType::Environmental;
    }

    NoiseType::Unknown
}

/// Estimate spectral slope (in dB/octave).
fn estimate_spectral_slope(spectrum: &[f32]) -> f32 {
    if spectrum.len() < 10 {
        return 0.0;
    }

    // Divide spectrum into logarithmic bins (octaves)
    let num_bins = 6;
    let mut bin_energies = vec![0.0; num_bins];
    let mut bin_counts = vec![0; num_bins];

    for (i, &mag) in spectrum.iter().enumerate() {
        if i > 0 {
            let octave = (i as f32).log2() as usize;
            if octave < num_bins {
                bin_energies[octave] += mag * mag;
                bin_counts[octave] += 1;
            }
        }
    }

    // Average energy per bin
    for i in 0..num_bins {
        if bin_counts[i] > 0 {
            bin_energies[i] /= bin_counts[i] as f32;
        }
    }

    // Compute slope via linear regression in log space
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xy = 0.0;
    let mut sum_xx = 0.0;
    let mut n = 0;

    for (i, &energy) in bin_energies.iter().enumerate() {
        if energy > 0.0 {
            let x = i as f32;
            let y = 10.0 * energy.log10();

            sum_x += x;
            sum_y += y;
            sum_xy += x * y;
            sum_xx += x * x;
            n += 1;
        }
    }

    if n > 1 {
        (n as f32 * sum_xy - sum_x * sum_y) / (n as f32 * sum_xx - sum_x * sum_x)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noise_classification() {
        // White noise: flat spectrum
        let white_spectral = SpectralFeatures {
            centroid: 1000.0,
            flatness: 0.95,
            crest: 1.5,
            bandwidth: 2000.0,
            rolloff: 5000.0,
            flux: 0.0,
            magnitude_spectrum: vec![1.0; 100],
        };

        assert_eq!(classify_noise(&white_spectral), NoiseType::White);

        // Hum: low frequency
        let hum_spectral = SpectralFeatures {
            centroid: 60.0,
            flatness: 0.1,
            crest: 5.0,
            bandwidth: 50.0,
            rolloff: 100.0,
            flux: 0.0,
            magnitude_spectrum: vec![0.1; 100],
        };

        assert_eq!(classify_noise(&hum_spectral), NoiseType::Hum);
    }
}
