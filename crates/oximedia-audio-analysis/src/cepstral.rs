#![allow(dead_code)]
//! Cepstral analysis for audio signals.
//!
//! Cepstral analysis operates in the "quefrency" domain (the cepstral domain)
//! to separate the excitation source from the vocal tract response. This module
//! provides MFCC extraction, cepstral peak prominence, and liftering utilities.

use std::f64::consts::PI;

/// Configuration for cepstral analysis.
#[derive(Debug, Clone)]
pub struct CepstralConfig {
    /// Number of cepstral coefficients to extract (default 13).
    pub num_coefficients: usize,
    /// Number of Mel filter banks (default 26).
    pub num_filters: usize,
    /// FFT size in samples (default 1024).
    pub fft_size: usize,
    /// Low frequency edge for Mel filter bank in Hz.
    pub low_freq: f64,
    /// High frequency edge for Mel filter bank in Hz.
    pub high_freq: f64,
    /// Whether to apply liftering (default true).
    pub apply_lifter: bool,
    /// Liftering coefficient L (default 22).
    pub lifter_coeff: usize,
}

impl Default for CepstralConfig {
    fn default() -> Self {
        Self {
            num_coefficients: 13,
            num_filters: 26,
            fft_size: 1024,
            low_freq: 20.0,
            high_freq: 8000.0,
            apply_lifter: true,
            lifter_coeff: 22,
        }
    }
}

/// Result of cepstral analysis on a single frame.
#[derive(Debug, Clone)]
pub struct CepstralFrame {
    /// MFCC coefficients for this frame.
    pub mfcc: Vec<f64>,
    /// Cepstral peak prominence (CPP) in dB.
    pub cpp_db: f64,
    /// Index of the dominant cepstral peak (quefrency bin).
    pub peak_quefrency_bin: usize,
    /// Energy of the frame in dB.
    pub energy_db: f64,
}

/// Cepstral analyzer for extracting MFCCs and cepstral features.
#[derive(Debug, Clone)]
pub struct CepstralAnalyzer {
    config: CepstralConfig,
    mel_filters: Vec<Vec<f64>>,
}

/// Convert frequency in Hz to Mel scale.
#[allow(clippy::cast_precision_loss)]
fn hz_to_mel(hz: f64) -> f64 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

/// Convert Mel scale value back to Hz.
#[allow(clippy::cast_precision_loss)]
fn mel_to_hz(mel: f64) -> f64 {
    700.0 * (10.0_f64.powf(mel / 2595.0) - 1.0)
}

/// Build triangular Mel filter bank.
#[allow(clippy::cast_precision_loss, clippy::needless_range_loop)]
fn build_mel_filters(
    num_filters: usize,
    fft_size: usize,
    sample_rate: f64,
    low_freq: f64,
    high_freq: f64,
) -> Vec<Vec<f64>> {
    let num_bins = fft_size / 2 + 1;
    let mel_low = hz_to_mel(low_freq);
    let mel_high = hz_to_mel(high_freq);

    // Create equally spaced Mel points
    let mel_points: Vec<f64> = (0..=(num_filters + 1))
        .map(|i| mel_low + (mel_high - mel_low) * i as f64 / (num_filters + 1) as f64)
        .collect();

    // Convert back to Hz and then to FFT bin indices
    let bin_indices: Vec<usize> = mel_points
        .iter()
        .map(|&m| {
            let hz = mel_to_hz(m);
            let bin = (hz * fft_size as f64 / sample_rate).floor() as usize;
            bin.min(num_bins.saturating_sub(1))
        })
        .collect();

    let mut filters = Vec::with_capacity(num_filters);
    for f in 0..num_filters {
        let mut filter = vec![0.0_f64; num_bins];
        let left = bin_indices[f];
        let center = bin_indices[f + 1];
        let right = bin_indices[f + 2];

        // Rising slope
        if center > left {
            for k in left..=center {
                filter[k] = (k - left) as f64 / (center - left) as f64;
            }
        }
        // Falling slope
        if right > center {
            for k in center..=right.min(num_bins - 1) {
                filter[k] = (right - k) as f64 / (right - center) as f64;
            }
        }
        filters.push(filter);
    }
    filters
}

/// Apply Discrete Cosine Transform (type II) to log Mel energies.
#[allow(clippy::cast_precision_loss)]
fn dct_ii(input: &[f64], num_output: usize) -> Vec<f64> {
    let n = input.len();
    (0..num_output)
        .map(|k| {
            let sum: f64 = input
                .iter()
                .enumerate()
                .map(|(i, &val)| {
                    val * (PI * k as f64 * (2.0 * i as f64 + 1.0) / (2.0 * n as f64)).cos()
                })
                .sum();
            sum
        })
        .collect()
}

/// Apply sinusoidal liftering to cepstral coefficients.
#[allow(clippy::cast_precision_loss)]
fn lifter(coeffs: &mut [f64], lift_coeff: usize) {
    let l = lift_coeff as f64;
    for (i, c) in coeffs.iter_mut().enumerate() {
        *c *= 1.0 + (l / 2.0) * (PI * i as f64 / l).sin();
    }
}

impl CepstralAnalyzer {
    /// Create a new cepstral analyzer for the given sample rate.
    #[must_use]
    pub fn new(config: CepstralConfig, sample_rate: f64) -> Self {
        let mel_filters = build_mel_filters(
            config.num_filters,
            config.fft_size,
            sample_rate,
            config.low_freq,
            config.high_freq.min(sample_rate / 2.0),
        );
        Self {
            config,
            mel_filters,
        }
    }

    /// Analyze a single frame of power spectrum data.
    ///
    /// `power_spectrum` should contain `fft_size / 2 + 1` bins.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn analyze_frame(&self, power_spectrum: &[f64]) -> CepstralFrame {
        // Apply Mel filter bank
        let mel_energies: Vec<f64> = self
            .mel_filters
            .iter()
            .map(|filter| {
                let energy: f64 = filter
                    .iter()
                    .zip(power_spectrum.iter())
                    .map(|(f, p)| f * p)
                    .sum();
                if energy > 1e-30 {
                    energy.ln()
                } else {
                    -69.0
                } // floor
            })
            .collect();

        // DCT to get MFCCs
        let mut mfcc = dct_ii(&mel_energies, self.config.num_coefficients);

        // Optional liftering
        if self.config.apply_lifter {
            lifter(&mut mfcc, self.config.lifter_coeff);
        }

        // Frame energy
        let total_energy: f64 = power_spectrum.iter().sum();
        let energy_db = if total_energy > 1e-30 {
            10.0 * total_energy.log10()
        } else {
            -100.0
        };

        // Cepstral peak prominence: compute real cepstrum, find peak
        let cepstrum: Vec<f64> = power_spectrum
            .iter()
            .map(|&p| if p > 1e-30 { p.ln() } else { -69.0 })
            .collect();

        let (peak_bin, peak_val) = cepstrum
            .iter()
            .enumerate()
            .skip(2) // skip DC and first bin
            .fold((0_usize, f64::NEG_INFINITY), |(bi, bv), (i, &v)| {
                if v > bv {
                    (i, v)
                } else {
                    (bi, bv)
                }
            });

        // CPP = peak value minus average
        let avg = cepstrum.iter().sum::<f64>() / cepstrum.len().max(1) as f64;
        let cpp_db = peak_val - avg;

        CepstralFrame {
            mfcc,
            cpp_db,
            peak_quefrency_bin: peak_bin,
            energy_db,
        }
    }

    /// Analyze multiple frames and return all cepstral frames.
    #[must_use]
    pub fn analyze_frames(&self, power_spectra: &[Vec<f64>]) -> Vec<CepstralFrame> {
        power_spectra
            .iter()
            .map(|spectrum| self.analyze_frame(spectrum))
            .collect()
    }

    /// Compute delta (first derivative) features from a sequence of MFCC vectors.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn compute_deltas(frames: &[CepstralFrame], context: usize) -> Vec<Vec<f64>> {
        let n = frames.len();
        if n == 0 {
            return Vec::new();
        }
        let num_coeffs = frames[0].mfcc.len();
        let mut deltas = Vec::with_capacity(n);
        let denom: f64 = (1..=context).map(|k| 2.0 * (k as f64) * (k as f64)).sum();
        let denom = if denom > 0.0 { denom } else { 1.0 };

        for t in 0..n {
            let mut delta = vec![0.0_f64; num_coeffs];
            for k in 1..=context {
                let prev = t.saturating_sub(k);
                let next = if t + k < n { t + k } else { n - 1 };
                for (c, d) in delta.iter_mut().enumerate() {
                    *d += k as f64 * (frames[next].mfcc[c] - frames[prev].mfcc[c]);
                }
            }
            for d in &mut delta {
                *d /= denom;
            }
            deltas.push(delta);
        }
        deltas
    }

    /// Return the current configuration.
    #[must_use]
    pub fn config(&self) -> &CepstralConfig {
        &self.config
    }
}

/// Compute cepstral distance between two MFCC vectors (Euclidean).
#[must_use]
pub fn cepstral_distance(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    let sum: f64 = (0..n).map(|i| (a[i] - b[i]).powi(2)).sum();
    sum.sqrt()
}

/// Compute mean MFCC vector over a set of frames.
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn mean_mfcc(frames: &[CepstralFrame]) -> Vec<f64> {
    if frames.is_empty() {
        return Vec::new();
    }
    let n = frames[0].mfcc.len();
    let mut mean = vec![0.0_f64; n];
    for frame in frames {
        for (i, &v) in frame.mfcc.iter().enumerate() {
            if i < n {
                mean[i] += v;
            }
        }
    }
    let count = frames.len() as f64;
    for m in &mut mean {
        *m /= count;
    }
    mean
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_flat_spectrum(num_bins: usize, value: f64) -> Vec<f64> {
        vec![value; num_bins]
    }

    #[test]
    fn test_hz_to_mel_and_back() {
        let hz = 1000.0;
        let mel = hz_to_mel(hz);
        let back = mel_to_hz(mel);
        assert!((hz - back).abs() < 1e-6);
    }

    #[test]
    fn test_hz_to_mel_zero() {
        assert!((hz_to_mel(0.0)).abs() < 1e-6);
    }

    #[test]
    fn test_mel_filters_shape() {
        let filters = build_mel_filters(26, 1024, 16000.0, 20.0, 8000.0);
        assert_eq!(filters.len(), 26);
        for f in &filters {
            assert_eq!(f.len(), 513); // 1024/2+1
        }
    }

    #[test]
    fn test_mel_filters_non_negative() {
        let filters = build_mel_filters(10, 512, 16000.0, 0.0, 8000.0);
        for f in &filters {
            for &v in f {
                assert!(v >= 0.0);
            }
        }
    }

    #[test]
    fn test_dct_ii_length() {
        let input = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let output = dct_ii(&input, 3);
        assert_eq!(output.len(), 3);
    }

    #[test]
    fn test_lifter_modifies_coefficients() {
        let mut coeffs = vec![1.0; 13];
        let original = coeffs.clone();
        lifter(&mut coeffs, 22);
        // First coefficient (i=0) stays the same (sin(0)=0, so multiplier=1.0)
        assert!((coeffs[0] - original[0]).abs() < 1e-10);
        // Others should be different
        assert!((coeffs[6] - original[6]).abs() > 0.01);
    }

    #[test]
    fn test_analyzer_creation() {
        let config = CepstralConfig::default();
        let analyzer = CepstralAnalyzer::new(config, 16000.0);
        assert_eq!(analyzer.config().num_coefficients, 13);
        assert_eq!(analyzer.config().num_filters, 26);
    }

    #[test]
    fn test_analyze_frame_mfcc_count() {
        let config = CepstralConfig {
            num_coefficients: 13,
            num_filters: 26,
            fft_size: 512,
            ..CepstralConfig::default()
        };
        let analyzer = CepstralAnalyzer::new(config, 16000.0);
        let spectrum = make_flat_spectrum(257, 0.01);
        let frame = analyzer.analyze_frame(&spectrum);
        assert_eq!(frame.mfcc.len(), 13);
    }

    #[test]
    fn test_analyze_frame_energy() {
        let config = CepstralConfig {
            fft_size: 512,
            ..CepstralConfig::default()
        };
        let analyzer = CepstralAnalyzer::new(config, 16000.0);
        let loud = make_flat_spectrum(257, 1.0);
        let quiet = make_flat_spectrum(257, 0.001);
        let frame_loud = analyzer.analyze_frame(&loud);
        let frame_quiet = analyzer.analyze_frame(&quiet);
        assert!(frame_loud.energy_db > frame_quiet.energy_db);
    }

    #[test]
    fn test_analyze_frames_multiple() {
        let config = CepstralConfig {
            fft_size: 512,
            ..CepstralConfig::default()
        };
        let analyzer = CepstralAnalyzer::new(config, 16000.0);
        let spectra = vec![
            make_flat_spectrum(257, 0.01),
            make_flat_spectrum(257, 0.02),
            make_flat_spectrum(257, 0.03),
        ];
        let results = analyzer.analyze_frames(&spectra);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_compute_deltas() {
        let config = CepstralConfig {
            fft_size: 512,
            num_coefficients: 5,
            num_filters: 10,
            ..CepstralConfig::default()
        };
        let analyzer = CepstralAnalyzer::new(config, 16000.0);
        let spectra: Vec<Vec<f64>> = (1..=5)
            .map(|i| make_flat_spectrum(257, 0.01 * i as f64))
            .collect();
        let frames = analyzer.analyze_frames(&spectra);
        let deltas = CepstralAnalyzer::compute_deltas(&frames, 2);
        assert_eq!(deltas.len(), 5);
        assert_eq!(deltas[0].len(), 5);
    }

    #[test]
    fn test_cepstral_distance_zero() {
        let a = vec![1.0, 2.0, 3.0];
        assert!((cepstral_distance(&a, &a)).abs() < 1e-10);
    }

    #[test]
    fn test_cepstral_distance_nonzero() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 0.0, 0.0];
        assert!((cepstral_distance(&a, &b) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_mean_mfcc_empty() {
        let result = mean_mfcc(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_mean_mfcc_single() {
        let frame = CepstralFrame {
            mfcc: vec![1.0, 2.0, 3.0],
            cpp_db: 0.0,
            peak_quefrency_bin: 0,
            energy_db: 0.0,
        };
        let mean = mean_mfcc(&[frame]);
        assert!((mean[0] - 1.0).abs() < 1e-10);
        assert!((mean[1] - 2.0).abs() < 1e-10);
        assert!((mean[2] - 3.0).abs() < 1e-10);
    }
}
