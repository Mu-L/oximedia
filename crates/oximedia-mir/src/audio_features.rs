//! Audio feature extraction for Music Information Retrieval.
//!
//! Provides MFCC coefficient accumulation, a simplified log-mel spectrogram
//! computation, and chroma vector analysis.

#![allow(dead_code)]

// ── MfccCoeffs ────────────────────────────────────────────────────────────────

/// Accumulates MFCC (Mel-Frequency Cepstral Coefficient) frames and provides
/// basic statistics across those frames.
#[derive(Debug, Clone)]
pub struct MfccCoeffs {
    /// Stored coefficient frames (each frame is a `Vec<f32>` of length `num_mfcc`).
    pub coefficients: Vec<Vec<f32>>,
    /// Number of MFCC coefficients per frame.
    pub num_mfcc: usize,
}

impl MfccCoeffs {
    /// Create a new, empty `MfccCoeffs` accumulator.
    ///
    /// # Arguments
    /// * `num_mfcc` – number of MFCC coefficients expected per frame.
    #[must_use]
    pub fn new(num_mfcc: usize) -> Self {
        Self {
            coefficients: Vec::new(),
            num_mfcc,
        }
    }

    /// Add a single MFCC frame.  Silently ignores frames whose length ≠ `num_mfcc`.
    pub fn add_frame(&mut self, coeffs: &[f32]) {
        if coeffs.len() == self.num_mfcc {
            self.coefficients.push(coeffs.to_vec());
        }
    }

    /// Compute the per-coefficient mean across all stored frames.
    ///
    /// Returns a zero vector of length `num_mfcc` if no frames have been added.
    #[must_use]
    pub fn mean(&self) -> Vec<f32> {
        if self.coefficients.is_empty() {
            return vec![0.0; self.num_mfcc];
        }
        let n = self.coefficients.len() as f32;
        let mut result = vec![0.0f32; self.num_mfcc];
        for frame in &self.coefficients {
            for (i, &v) in frame.iter().enumerate() {
                result[i] += v;
            }
        }
        for x in &mut result {
            *x /= n;
        }
        result
    }

    /// Compute the per-coefficient variance across all stored frames.
    ///
    /// Returns a zero vector of length `num_mfcc` if fewer than 2 frames exist.
    #[must_use]
    pub fn variance(&self) -> Vec<f32> {
        if self.coefficients.len() < 2 {
            return vec![0.0; self.num_mfcc];
        }
        let mean = self.mean();
        let n = self.coefficients.len() as f32;
        let mut var = vec![0.0f32; self.num_mfcc];
        for frame in &self.coefficients {
            for (i, &v) in frame.iter().enumerate() {
                let diff = v - mean[i];
                var[i] += diff * diff;
            }
        }
        for x in &mut var {
            *x /= n;
        }
        var
    }

    /// Compute the delta (first-order difference) for coefficient at `idx`.
    ///
    /// Uses the simple backward difference: `frame[last][idx] - frame[0][idx]`.
    /// Returns `0.0` if fewer than 2 frames are stored or `idx` is out of range.
    #[must_use]
    pub fn delta(&self, idx: usize) -> f32 {
        if self.coefficients.len() < 2 || idx >= self.num_mfcc {
            return 0.0;
        }
        let last = self.coefficients.len() - 1;
        self.coefficients[last][idx] - self.coefficients[0][idx]
    }
}

// ── compute_log_mel_spectrogram ────────────────────────────────────────────────

/// Compute a simplified log-mel spectrogram from a mono audio signal.
///
/// This is an energy-based approximation: the signal is split into overlapping
/// frames, the RMS energy of each frame is computed, and the result is spread
/// across `n_mels` mel bins using equal interpolation.
///
/// # Arguments
/// * `samples`     – mono audio samples (f32, any range).
/// * `sample_rate` – sample rate in Hz (used for documentation / future use).
/// * `n_mels`      – number of mel filter banks.
/// * `hop_length`  – hop length between frames in samples.
///
/// # Returns
///
/// A `Vec<Vec<f32>>` with shape `[n_frames][n_mels]`, log-energy values.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn compute_log_mel_spectrogram(
    samples: &[f32],
    _sample_rate: u32,
    n_mels: usize,
    hop_length: usize,
) -> Vec<Vec<f32>> {
    if samples.is_empty() || n_mels == 0 || hop_length == 0 {
        return Vec::new();
    }

    let hop = hop_length;
    let n_frames = if samples.len() >= hop {
        (samples.len() - 1) / hop + 1
    } else {
        1
    };

    let mut spectrogram = Vec::with_capacity(n_frames);

    for frame_idx in 0..n_frames {
        let start = frame_idx * hop;
        let end = (start + hop).min(samples.len());
        let frame = &samples[start..end];

        // Compute RMS energy of the frame
        let rms = if frame.is_empty() {
            0.0f32
        } else {
            let sum_sq: f32 = frame.iter().map(|&s| s * s).sum();
            (sum_sq / frame.len() as f32).sqrt()
        };

        let log_energy = (rms + 1e-9).ln();

        // Spread the log energy across all mel bins (simplified approximation)
        let mel_frame = vec![log_energy; n_mels];
        spectrogram.push(mel_frame);
    }

    spectrogram
}

// ── ChromaVector ──────────────────────────────────────────────────────────────

/// A 12-element chroma vector representing the energy distribution across
/// the 12 pitch classes (C, C#, D, D#, E, F, F#, G, G#, A, A#, B).
#[derive(Debug, Clone)]
pub struct ChromaVector {
    /// Raw chroma values, one per pitch class.
    pub chroma: [f32; 12],
}

impl ChromaVector {
    /// Return a new `ChromaVector` normalized so that its maximum value is 1.0.
    ///
    /// If all values are zero (or negative), the original vector is returned unchanged.
    #[must_use]
    pub fn normalize(&self) -> Self {
        let max = self
            .chroma
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);

        if max <= 0.0 {
            return self.clone();
        }

        let mut normalized = self.chroma;
        for v in &mut normalized {
            *v /= max;
        }
        Self { chroma: normalized }
    }

    /// Return the index (0–11) of the pitch class with the highest energy.
    #[must_use]
    pub fn dominant_class(&self) -> usize {
        self.chroma
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map_or(0, |(i, _)| i)
    }

    /// Return the sharpness of the chroma vector, defined as `max - min`.
    ///
    /// A high value indicates a strongly peaked distribution (clear pitch class),
    /// while a low value indicates a flat, noisy distribution.
    #[must_use]
    pub fn sharpness(&self) -> f32 {
        let max = self
            .chroma
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);
        let min = self.chroma.iter().copied().fold(f32::INFINITY, f32::min);
        max - min
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── MfccCoeffs ─────────────────────────────────────────────────────────────

    #[test]
    fn test_mfcc_new_empty() {
        let m = MfccCoeffs::new(13);
        assert_eq!(m.num_mfcc, 13);
        assert!(m.coefficients.is_empty());
    }

    #[test]
    fn test_mfcc_add_frame_correct_length() {
        let mut m = MfccCoeffs::new(3);
        m.add_frame(&[1.0, 2.0, 3.0]);
        assert_eq!(m.coefficients.len(), 1);
    }

    #[test]
    fn test_mfcc_add_frame_wrong_length_ignored() {
        let mut m = MfccCoeffs::new(3);
        m.add_frame(&[1.0, 2.0]); // wrong length
        assert!(m.coefficients.is_empty());
    }

    #[test]
    fn test_mfcc_mean_single_frame() {
        let mut m = MfccCoeffs::new(2);
        m.add_frame(&[4.0, 6.0]);
        let mean = m.mean();
        assert!((mean[0] - 4.0).abs() < 1e-5);
        assert!((mean[1] - 6.0).abs() < 1e-5);
    }

    #[test]
    fn test_mfcc_mean_two_frames() {
        let mut m = MfccCoeffs::new(2);
        m.add_frame(&[2.0, 4.0]);
        m.add_frame(&[4.0, 8.0]);
        let mean = m.mean();
        assert!((mean[0] - 3.0).abs() < 1e-5);
        assert!((mean[1] - 6.0).abs() < 1e-5);
    }

    #[test]
    fn test_mfcc_mean_empty_returns_zeros() {
        let m = MfccCoeffs::new(4);
        let mean = m.mean();
        assert_eq!(mean, vec![0.0; 4]);
    }

    #[test]
    fn test_mfcc_variance_two_frames() {
        let mut m = MfccCoeffs::new(1);
        m.add_frame(&[2.0]);
        m.add_frame(&[4.0]);
        // mean = 3.0; var = ((2-3)^2 + (4-3)^2) / 2 = 1.0
        let var = m.variance();
        assert!((var[0] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_mfcc_variance_one_frame_returns_zeros() {
        let mut m = MfccCoeffs::new(2);
        m.add_frame(&[1.0, 2.0]);
        let var = m.variance();
        assert_eq!(var, vec![0.0; 2]);
    }

    #[test]
    fn test_mfcc_delta_two_frames() {
        let mut m = MfccCoeffs::new(2);
        m.add_frame(&[1.0, 3.0]);
        m.add_frame(&[5.0, 7.0]);
        assert!((m.delta(0) - 4.0).abs() < 1e-5);
        assert!((m.delta(1) - 4.0).abs() < 1e-5);
    }

    #[test]
    fn test_mfcc_delta_one_frame_returns_zero() {
        let mut m = MfccCoeffs::new(2);
        m.add_frame(&[1.0, 2.0]);
        assert!((m.delta(0) - 0.0).abs() < 1e-5);
    }

    // ── compute_log_mel_spectrogram ─────────────────────────────────────────────

    #[test]
    fn test_log_mel_spectrogram_empty_input() {
        let result = compute_log_mel_spectrogram(&[], 44100, 40, 512);
        assert!(result.is_empty());
    }

    #[test]
    fn test_log_mel_spectrogram_output_shape() {
        let samples = vec![0.1f32; 2048];
        let result = compute_log_mel_spectrogram(&samples, 44100, 40, 512);
        // should produce multiple frames, each with 40 mel bins
        assert!(!result.is_empty());
        assert_eq!(result[0].len(), 40);
    }

    #[test]
    fn test_log_mel_spectrogram_zero_input_is_log_epsilon() {
        let samples = vec![0.0f32; 512];
        let result = compute_log_mel_spectrogram(&samples, 44100, 10, 512);
        assert!(!result.is_empty());
        // log(0 + 1e-9) < 0
        for &v in &result[0] {
            assert!(v < 0.0);
        }
    }

    // ── ChromaVector ────────────────────────────────────────────────────────────

    #[test]
    fn test_chroma_normalize_max_becomes_one() {
        let cv = ChromaVector {
            chroma: [0.0, 2.0, 1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        };
        let norm = cv.normalize();
        assert!((norm.chroma[1] - 1.0).abs() < 1e-5);
        assert!((norm.chroma[2] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_chroma_normalize_all_zeros_unchanged() {
        let cv = ChromaVector {
            chroma: [0.0f32; 12],
        };
        let norm = cv.normalize();
        assert_eq!(norm.chroma, [0.0f32; 12]);
    }

    #[test]
    fn test_chroma_dominant_class() {
        let mut chroma = [0.0f32; 12];
        chroma[7] = 3.5; // G (index 7) is dominant
        let cv = ChromaVector { chroma };
        assert_eq!(cv.dominant_class(), 7);
    }

    #[test]
    fn test_chroma_sharpness() {
        let cv = ChromaVector {
            chroma: [0.2, 0.9, 0.1, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        };
        let sharpness = cv.sharpness();
        assert!((sharpness - 0.9).abs() < 1e-5);
    }

    #[test]
    fn test_chroma_sharpness_uniform_is_zero() {
        let cv = ChromaVector {
            chroma: [1.0f32; 12],
        };
        assert!((cv.sharpness() - 0.0).abs() < 1e-5);
    }
}
