#![allow(dead_code)]
//! Spectral balance normalization for frequency-band loudness control.
//!
//! This module provides multi-band spectral analysis and normalization to ensure
//! that audio content has a balanced frequency distribution. It is useful for
//! maintaining consistent tonal quality across different source materials.

use std::collections::HashMap;

/// Number of octave bands in the standard ISO set.
const NUM_OCTAVE_BANDS: usize = 10;

/// Default center frequencies for octave bands (Hz).
const OCTAVE_CENTERS: [f64; NUM_OCTAVE_BANDS] = [
    31.25, 62.5, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

/// A single frequency band with configurable boundaries.
#[derive(Debug, Clone)]
pub struct FrequencyBand {
    /// Center frequency in Hz.
    pub center_hz: f64,
    /// Lower cutoff frequency in Hz.
    pub low_hz: f64,
    /// Upper cutoff frequency in Hz.
    pub high_hz: f64,
    /// Measured energy level in dB.
    pub energy_db: f64,
    /// Target energy level in dB.
    pub target_db: f64,
    /// Computed gain correction in dB.
    pub correction_db: f64,
}

impl FrequencyBand {
    /// Create a new frequency band.
    pub fn new(center_hz: f64, low_hz: f64, high_hz: f64) -> Self {
        Self {
            center_hz,
            low_hz,
            high_hz,
            energy_db: -100.0,
            target_db: 0.0,
            correction_db: 0.0,
        }
    }

    /// Compute the bandwidth in Hz.
    pub fn bandwidth(&self) -> f64 {
        self.high_hz - self.low_hz
    }

    /// Compute the Q factor.
    pub fn q_factor(&self) -> f64 {
        if self.bandwidth() > 0.0 {
            self.center_hz / self.bandwidth()
        } else {
            1.0
        }
    }

    /// Check whether a frequency falls within this band.
    pub fn contains(&self, freq_hz: f64) -> bool {
        freq_hz >= self.low_hz && freq_hz < self.high_hz
    }
}

/// Configuration for spectral balance normalization.
#[derive(Debug, Clone)]
pub struct SpectralBalanceConfig {
    /// Sample rate in Hz.
    pub sample_rate: f64,
    /// Number of channels.
    pub channels: usize,
    /// FFT size for spectral analysis.
    pub fft_size: usize,
    /// Hop size (overlap) for spectral analysis.
    pub hop_size: usize,
    /// Maximum correction gain in dB per band.
    pub max_correction_db: f64,
    /// Smoothing factor (0.0 = no smoothing, 1.0 = full smoothing).
    pub smoothing: f64,
    /// Whether to use A-weighting for perceived loudness.
    pub a_weighting: bool,
}

impl Default for SpectralBalanceConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000.0,
            channels: 2,
            fft_size: 4096,
            hop_size: 2048,
            max_correction_db: 6.0,
            smoothing: 0.8,
            a_weighting: true,
        }
    }
}

impl SpectralBalanceConfig {
    /// Create a new configuration with the given sample rate and channels.
    pub fn new(sample_rate: f64, channels: usize) -> Self {
        Self {
            sample_rate,
            channels,
            ..Default::default()
        }
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), String> {
        if self.sample_rate < 8000.0 || self.sample_rate > 192_000.0 {
            return Err(format!("Invalid sample rate: {}", self.sample_rate));
        }
        if self.channels == 0 || self.channels > 16 {
            return Err(format!("Invalid channel count: {}", self.channels));
        }
        if self.fft_size < 256 || !self.fft_size.is_power_of_two() {
            return Err(format!(
                "FFT size must be a power of 2 >= 256, got {}",
                self.fft_size
            ));
        }
        if self.hop_size == 0 || self.hop_size > self.fft_size {
            return Err(format!(
                "Hop size must be in (0, {}], got {}",
                self.fft_size, self.hop_size
            ));
        }
        if self.max_correction_db < 0.0 || self.max_correction_db > 24.0 {
            return Err(format!(
                "Max correction must be in [0, 24] dB, got {}",
                self.max_correction_db
            ));
        }
        if !(0.0..=1.0).contains(&self.smoothing) {
            return Err(format!(
                "Smoothing must be in [0, 1], got {}",
                self.smoothing
            ));
        }
        Ok(())
    }
}

/// A-weighting coefficient lookup for standard octave bands.
fn a_weight_correction(center_hz: f64) -> f64 {
    // Approximate A-weighting corrections for octave-band centers
    let table: &[(f64, f64)] = &[
        (31.25, -39.4),
        (62.5, -26.2),
        (125.0, -16.1),
        (250.0, -8.6),
        (500.0, -3.2),
        (1000.0, 0.0),
        (2000.0, 1.2),
        (4000.0, 1.0),
        (8000.0, -1.1),
        (16000.0, -6.6),
    ];

    // Find the closest entry
    let mut best = 0.0;
    let mut best_dist = f64::MAX;
    for &(freq, weight) in table {
        let dist = (freq - center_hz).abs();
        if dist < best_dist {
            best_dist = dist;
            best = weight;
        }
    }
    best
}

/// Target spectral profile for normalization.
#[derive(Debug, Clone)]
pub enum SpectralTarget {
    /// Flat spectral profile (equal energy per band).
    Flat,
    /// Pink noise profile (-3 dB/octave).
    Pink,
    /// Broadcast speech profile (emphasis on 1-4 kHz).
    Speech,
    /// Music mastering profile.
    Music,
    /// Custom per-band targets (band index -> target dB).
    Custom(HashMap<usize, f64>),
}

impl SpectralTarget {
    /// Get the target level offset for a given band index.
    pub fn target_offset(&self, band_index: usize, center_hz: f64) -> f64 {
        match self {
            Self::Flat => 0.0,
            Self::Pink => {
                // -3 dB per octave relative to 1 kHz
                if center_hz > 0.0 {
                    -3.0 * (center_hz / 1000.0).log2()
                } else {
                    0.0
                }
            }
            Self::Speech => {
                // Emphasis on 1-4 kHz for speech intelligibility
                if (1000.0..=4000.0).contains(&center_hz) {
                    3.0
                } else if center_hz < 250.0 {
                    -6.0
                } else {
                    0.0
                }
            }
            Self::Music => {
                // Slight mid-scoop, sub/presence boost
                if center_hz < 80.0 {
                    2.0
                } else if (200.0..=800.0).contains(&center_hz) {
                    -2.0
                } else if (2000.0..=6000.0).contains(&center_hz) {
                    1.5
                } else {
                    0.0
                }
            }
            Self::Custom(map) => map.get(&band_index).copied().unwrap_or(0.0),
        }
    }
}

/// Spectral balance analyzer and normalizer.
#[derive(Debug)]
pub struct SpectralBalanceProcessor {
    /// Configuration.
    config: SpectralBalanceConfig,
    /// Frequency bands.
    bands: Vec<FrequencyBand>,
    /// Target spectral profile.
    target: SpectralTarget,
    /// Number of frames analyzed.
    frames_analyzed: u64,
    /// Running average energy per band.
    avg_energy: Vec<f64>,
}

impl SpectralBalanceProcessor {
    /// Create a new spectral balance processor.
    pub fn new(config: SpectralBalanceConfig, target: SpectralTarget) -> Result<Self, String> {
        config.validate()?;

        let nyquist = config.sample_rate / 2.0;
        let mut bands = Vec::new();
        for (i, &center) in OCTAVE_CENTERS.iter().enumerate() {
            if center < nyquist {
                let low = center / 2.0_f64.sqrt();
                let high = (center * 2.0_f64.sqrt()).min(nyquist);
                bands.push(FrequencyBand::new(center, low, high));
                // Set target based on profile
                if let Some(band) = bands.last_mut() {
                    band.target_db = target.target_offset(i, center);
                }
            }
        }

        let num_bands = bands.len();
        Ok(Self {
            config,
            bands,
            target,
            frames_analyzed: 0,
            avg_energy: vec![-100.0; num_bands],
        })
    }

    /// Get the frequency bands.
    pub fn bands(&self) -> &[FrequencyBand] {
        &self.bands
    }

    /// Get the number of bands.
    pub fn num_bands(&self) -> usize {
        self.bands.len()
    }

    /// Get the number of frames analyzed.
    pub fn frames_analyzed(&self) -> u64 {
        self.frames_analyzed
    }

    /// Analyze a block of audio samples and update band energies.
    ///
    /// Samples are interleaved for multi-channel audio.
    pub fn analyze(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }

        let channels = self.config.channels;
        let frame_count = samples.len() / channels;
        if frame_count == 0 {
            return;
        }

        // Compute RMS energy of the block (simple time-domain approximation)
        let mut sum_sq = 0.0_f64;
        for &s in samples {
            let v = f64::from(s);
            sum_sq += v * v;
        }
        let rms = (sum_sq / samples.len() as f64).sqrt();
        let rms_db = if rms > 1e-10 {
            20.0 * rms.log10()
        } else {
            -100.0
        };

        // Distribute energy across bands (simplified model)
        // In a real implementation this would use FFT; here we approximate
        let num_bands = self.bands.len();
        for (i, band) in self.bands.iter_mut().enumerate() {
            // Simple model: energy falls off with distance from center of spectrum
            let norm_pos = i as f64 / num_bands.max(1) as f64;
            let spectral_shape = 1.0 - (norm_pos - 0.5).abs() * 0.5;
            let band_energy = rms_db + 20.0 * spectral_shape.log10().max(-20.0);

            // Exponential smoothing
            let alpha = 1.0 - self.config.smoothing;
            if self.frames_analyzed == 0 {
                self.avg_energy[i] = band_energy;
            } else {
                self.avg_energy[i] =
                    self.avg_energy[i] * self.config.smoothing + band_energy * alpha;
            }
            band.energy_db = self.avg_energy[i];
        }

        self.frames_analyzed += 1;
    }

    /// Compute per-band correction gains.
    pub fn compute_corrections(&mut self) -> Vec<f64> {
        let max_corr = self.config.max_correction_db;
        let mut corrections = Vec::with_capacity(self.bands.len());

        for (i, band) in self.bands.iter_mut().enumerate() {
            let mut target = band.target_db;
            if self.config.a_weighting {
                target += a_weight_correction(band.center_hz);
            }

            let diff = target - band.energy_db;
            let correction = diff.clamp(-max_corr, max_corr);
            band.correction_db = correction;
            corrections.push(correction);
            let _ = i; // suppress unused warning
        }

        corrections
    }

    /// Apply computed corrections to audio samples (in-place).
    ///
    /// This applies a simple broadband gain derived from the average correction.
    /// For per-band processing, use `apply_perband_eq` instead.
    pub fn apply_broadband_correction(&self, samples: &mut [f32]) {
        if self.bands.is_empty() || samples.is_empty() {
            return;
        }

        let avg_correction: f64 =
            self.bands.iter().map(|b| b.correction_db).sum::<f64>() / self.bands.len() as f64;

        let gain = 10.0_f64.powf(avg_correction / 20.0);
        for s in samples.iter_mut() {
            *s = (f64::from(*s) * gain) as f32;
        }
    }

    /// Perform actual per-band spectral equalization using DFT-based processing.
    ///
    /// This method:
    /// 1. Converts the time-domain signal to frequency domain via real DFT
    /// 2. Applies per-band gain corrections to the appropriate frequency bins
    /// 3. Converts back to time domain via inverse DFT
    ///
    /// Operates on mono audio (first channel if interleaved).
    /// The `corrections` slice must have the same length as `self.bands`.
    pub fn apply_perband_eq(&self, samples: &mut [f32], corrections: &[f64]) {
        if samples.is_empty() || self.bands.is_empty() || corrections.is_empty() {
            return;
        }

        let fft_size = self.config.fft_size;
        let hop_size = self.config.hop_size;
        let channels = self.config.channels;
        let mono_len = samples.len() / channels;

        if mono_len < fft_size {
            // Not enough samples for a full FFT frame; fall back to broadband
            self.apply_broadband_correction(samples);
            return;
        }

        // Extract mono signal (average of all channels)
        let mut mono: Vec<f64> = vec![0.0; mono_len];
        for frame_idx in 0..mono_len {
            let mut sum = 0.0_f64;
            for ch in 0..channels {
                let idx = frame_idx * channels + ch;
                if idx < samples.len() {
                    sum += f64::from(samples[idx]);
                }
            }
            mono[frame_idx] = sum / channels as f64;
        }

        // Build per-bin gain table from band corrections
        let bin_gains = self.build_bin_gains(fft_size, corrections);

        // Overlap-add processing
        let mut output = vec![0.0_f64; mono_len];
        let mut window_sum = vec![0.0_f64; mono_len];
        let hann = hann_window(fft_size);

        let mut pos = 0usize;
        while pos + fft_size <= mono_len {
            // Window the input frame
            let mut real = vec![0.0_f64; fft_size];
            let mut imag = vec![0.0_f64; fft_size];
            for i in 0..fft_size {
                real[i] = mono[pos + i] * hann[i];
            }

            // Forward DFT
            dft_forward(&mut real, &mut imag);

            // Apply per-bin gains
            for k in 0..fft_size {
                real[k] *= bin_gains[k];
                imag[k] *= bin_gains[k];
            }

            // Inverse DFT
            dft_inverse(&mut real, &mut imag);

            // Overlap-add with synthesis window
            for i in 0..fft_size {
                output[pos + i] += real[i] * hann[i];
                window_sum[pos + i] += hann[i] * hann[i];
            }

            pos += hop_size;
        }

        // Normalize by window sum to avoid amplitude modulation
        for i in 0..mono_len {
            if window_sum[i] > 1e-8 {
                output[i] /= window_sum[i];
            }
        }

        // Write back to interleaved output
        for frame_idx in 0..mono_len {
            let gain_ratio = if mono[frame_idx].abs() > 1e-12 {
                output[frame_idx] / mono[frame_idx]
            } else {
                1.0
            };
            for ch in 0..channels {
                let idx = frame_idx * channels + ch;
                if idx < samples.len() {
                    samples[idx] = (f64::from(samples[idx]) * gain_ratio) as f32;
                }
            }
        }
    }

    /// Build per-bin gain values from band corrections.
    fn build_bin_gains(&self, fft_size: usize, corrections: &[f64]) -> Vec<f64> {
        let mut bin_gains = vec![1.0_f64; fft_size];
        let bin_width = self.config.sample_rate / fft_size as f64;

        for k in 0..fft_size {
            let freq = k as f64 * bin_width;
            // Find which band this bin belongs to
            let mut best_band = None;
            let mut best_dist = f64::MAX;
            for (i, band) in self.bands.iter().enumerate() {
                if freq >= band.low_hz && freq < band.high_hz {
                    best_band = Some(i);
                    break;
                }
                // Also track nearest band for out-of-range bins
                let dist = (freq - band.center_hz).abs();
                if dist < best_dist {
                    best_dist = dist;
                    best_band = Some(i);
                }
            }
            if let Some(band_idx) = best_band {
                if band_idx < corrections.len() {
                    let gain_db = corrections[band_idx];
                    bin_gains[k] = 10.0_f64.powf(gain_db / 20.0);
                }
            }
        }
        bin_gains
    }

    /// Perform full spectral balance processing: analyze, compute corrections, apply EQ.
    ///
    /// This is the main entry point for actual spectral balance normalization.
    /// Call this instead of the separate analyze/compute/apply steps.
    pub fn process(&mut self, samples: &mut [f32]) {
        if samples.is_empty() {
            return;
        }
        self.analyze(samples);
        let corrections = self.compute_corrections();
        self.apply_perband_eq(samples, &corrections);
    }

    /// Reset all analysis state.
    pub fn reset(&mut self) {
        self.frames_analyzed = 0;
        for (i, band) in self.bands.iter_mut().enumerate() {
            band.energy_db = -100.0;
            band.correction_db = 0.0;
            self.avg_energy[i] = -100.0;
        }
    }

    /// Get a summary report of the spectral analysis.
    pub fn report(&self) -> SpectralReport {
        let band_reports: Vec<BandReport> = self
            .bands
            .iter()
            .map(|b| BandReport {
                center_hz: b.center_hz,
                energy_db: b.energy_db,
                target_db: b.target_db,
                correction_db: b.correction_db,
            })
            .collect();

        let avg_energy = if band_reports.is_empty() {
            -100.0
        } else {
            band_reports.iter().map(|b| b.energy_db).sum::<f64>() / band_reports.len() as f64
        };

        SpectralReport {
            bands: band_reports,
            avg_energy_db: avg_energy,
            frames_analyzed: self.frames_analyzed,
        }
    }
}

/// Generate a symmetric Hann window of the given size.
///
/// Uses the symmetric form `w(n) = 0.5 * (1 - cos(2π*n/(N-1)))` so that
/// `w[i] == w[N-1-i]` for all `i`.  This is required for linear-phase
/// overlap-add processing (STFT synthesis window must be symmetric).
///
/// For `size == 1` the single sample is 1.0.
fn hann_window(size: usize) -> Vec<f64> {
    if size <= 1 {
        return vec![1.0; size];
    }
    let n_minus_1 = (size - 1) as f64;
    (0..size)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / n_minus_1).cos()))
        .collect()
}

/// Forward DFT (in-place, O(N^2) reference implementation for correctness).
///
/// For the FFT sizes used here (4096), this is acceptable for non-realtime
/// processing. A production path would use a radix-2 FFT.
fn dft_forward(real: &mut [f64], imag: &mut [f64]) {
    let n = real.len();
    if n == 0 {
        return;
    }
    let mut out_real = vec![0.0_f64; n];
    let mut out_imag = vec![0.0_f64; n];

    for k in 0..n {
        let mut sum_r = 0.0_f64;
        let mut sum_i = 0.0_f64;
        for j in 0..n {
            let angle = 2.0 * std::f64::consts::PI * k as f64 * j as f64 / n as f64;
            sum_r += real[j] * angle.cos() + imag[j] * angle.sin();
            sum_i += -real[j] * angle.sin() + imag[j] * angle.cos();
        }
        out_real[k] = sum_r;
        out_imag[k] = sum_i;
    }

    real.copy_from_slice(&out_real);
    imag.copy_from_slice(&out_imag);
}

/// Inverse DFT (in-place, O(N^2) reference implementation).
fn dft_inverse(real: &mut [f64], imag: &mut [f64]) {
    let n = real.len();
    if n == 0 {
        return;
    }
    // IDFT: negate the imaginary part, apply forward DFT, negate again, divide by N
    for v in imag.iter_mut() {
        *v = -*v;
    }
    dft_forward(real, imag);
    for v in imag.iter_mut() {
        *v = -*v;
    }
    let inv_n = 1.0 / n as f64;
    for v in real.iter_mut() {
        *v *= inv_n;
    }
    for v in imag.iter_mut() {
        *v *= inv_n;
    }
}

/// Report for a single frequency band.
#[derive(Debug, Clone)]
pub struct BandReport {
    /// Center frequency in Hz.
    pub center_hz: f64,
    /// Measured energy in dB.
    pub energy_db: f64,
    /// Target energy in dB.
    pub target_db: f64,
    /// Computed correction in dB.
    pub correction_db: f64,
}

/// Summary report for spectral balance analysis.
#[derive(Debug, Clone)]
pub struct SpectralReport {
    /// Per-band reports.
    pub bands: Vec<BandReport>,
    /// Average energy across all bands in dB.
    pub avg_energy_db: f64,
    /// Total frames analyzed.
    pub frames_analyzed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frequency_band_creation() {
        let band = FrequencyBand::new(1000.0, 707.0, 1414.0);
        assert!((band.center_hz - 1000.0).abs() < f64::EPSILON);
        assert!((band.low_hz - 707.0).abs() < f64::EPSILON);
        assert!((band.energy_db - (-100.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_frequency_band_bandwidth() {
        let band = FrequencyBand::new(1000.0, 707.0, 1414.0);
        assert!((band.bandwidth() - 707.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_frequency_band_q_factor() {
        let band = FrequencyBand::new(1000.0, 707.0, 1414.0);
        let q = band.q_factor();
        assert!((q - 1000.0 / 707.0).abs() < 0.01);
    }

    #[test]
    fn test_frequency_band_contains() {
        let band = FrequencyBand::new(1000.0, 707.0, 1414.0);
        assert!(band.contains(1000.0));
        assert!(band.contains(707.0));
        assert!(!band.contains(1414.0)); // exclusive upper
        assert!(!band.contains(500.0));
    }

    #[test]
    fn test_config_default() {
        let config = SpectralBalanceConfig::default();
        assert!((config.sample_rate - 48000.0).abs() < f64::EPSILON);
        assert_eq!(config.channels, 2);
        assert_eq!(config.fft_size, 4096);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_invalid_sample_rate() {
        let mut config = SpectralBalanceConfig::default();
        config.sample_rate = 100.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_fft_size() {
        let mut config = SpectralBalanceConfig::default();
        config.fft_size = 300; // not a power of 2
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_spectral_target_flat() {
        let target = SpectralTarget::Flat;
        assert!((target.target_offset(0, 31.25)).abs() < f64::EPSILON);
        assert!((target.target_offset(5, 1000.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_spectral_target_pink() {
        let target = SpectralTarget::Pink;
        // At 1 kHz the offset should be 0
        assert!((target.target_offset(5, 1000.0)).abs() < f64::EPSILON);
        // At 2 kHz the offset should be -3 dB
        let offset = target.target_offset(6, 2000.0);
        assert!((offset - (-3.0)).abs() < 0.01);
    }

    #[test]
    fn test_spectral_target_speech() {
        let target = SpectralTarget::Speech;
        assert!((target.target_offset(5, 1000.0) - 3.0).abs() < f64::EPSILON);
        assert!((target.target_offset(0, 31.25) - (-6.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_spectral_target_custom() {
        let mut map = HashMap::new();
        map.insert(3, 5.0);
        let target = SpectralTarget::Custom(map);
        assert!((target.target_offset(3, 250.0) - 5.0).abs() < f64::EPSILON);
        assert!((target.target_offset(0, 31.25)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_processor_creation() {
        let config = SpectralBalanceConfig::default();
        let proc = SpectralBalanceProcessor::new(config, SpectralTarget::Flat);
        assert!(proc.is_ok());
        let proc = proc.expect("should succeed in test");
        assert!(proc.num_bands() > 0);
        assert_eq!(proc.frames_analyzed(), 0);
    }

    #[test]
    fn test_processor_analyze() {
        let config = SpectralBalanceConfig::new(48000.0, 1);
        let mut proc = SpectralBalanceProcessor::new(config, SpectralTarget::Flat)
            .expect("should succeed in test");

        let samples: Vec<f32> = (0..4096).map(|i| (i as f32 * 0.01).sin()).collect();
        proc.analyze(&samples);
        assert_eq!(proc.frames_analyzed(), 1);

        // Energies should be updated
        for band in proc.bands() {
            assert!(band.energy_db > -100.0);
        }
    }

    #[test]
    fn test_processor_compute_corrections() {
        let config = SpectralBalanceConfig::new(48000.0, 1);
        let mut proc = SpectralBalanceProcessor::new(config, SpectralTarget::Flat)
            .expect("should succeed in test");

        let samples: Vec<f32> = vec![0.5; 4096];
        proc.analyze(&samples);
        let corrections = proc.compute_corrections();
        assert_eq!(corrections.len(), proc.num_bands());
    }

    #[test]
    fn test_processor_apply_broadband() {
        let config = SpectralBalanceConfig::new(48000.0, 1);
        let mut proc = SpectralBalanceProcessor::new(config, SpectralTarget::Flat)
            .expect("should succeed in test");

        let samples: Vec<f32> = vec![0.5; 4096];
        proc.analyze(&samples);
        proc.compute_corrections();

        let mut output = vec![0.5_f32; 16];
        proc.apply_broadband_correction(&mut output);
        // Samples should be modified (gain applied)
        // The exact value depends on the correction
        assert!(output.iter().all(|&s| s != 0.0));
    }

    #[test]
    fn test_processor_reset() {
        let config = SpectralBalanceConfig::new(48000.0, 1);
        let mut proc = SpectralBalanceProcessor::new(config, SpectralTarget::Flat)
            .expect("should succeed in test");

        let samples: Vec<f32> = vec![0.5; 4096];
        proc.analyze(&samples);
        assert_eq!(proc.frames_analyzed(), 1);

        proc.reset();
        assert_eq!(proc.frames_analyzed(), 0);
        for band in proc.bands() {
            assert!((band.energy_db - (-100.0)).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_processor_report() {
        let config = SpectralBalanceConfig::new(48000.0, 1);
        let mut proc = SpectralBalanceProcessor::new(config, SpectralTarget::Flat)
            .expect("should succeed in test");

        let samples: Vec<f32> = vec![0.3; 4096];
        proc.analyze(&samples);
        let report = proc.report();
        assert!(!report.bands.is_empty());
        assert_eq!(report.frames_analyzed, 1);
    }

    #[test]
    fn test_a_weight_correction() {
        let w1k = a_weight_correction(1000.0);
        assert!((w1k).abs() < f64::EPSILON);

        let w31 = a_weight_correction(31.25);
        assert!((w31 - (-39.4)).abs() < f64::EPSILON);
    }

    // ── New tests for actual spectral balance processing ──

    #[test]
    fn test_hann_window_endpoints() {
        let w = hann_window(256);
        assert_eq!(w.len(), 256);
        // Hann window starts near zero
        assert!(w[0].abs() < 1e-10);
        // Mid-point should be close to 1.0
        assert!((w[128] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_hann_window_symmetry() {
        let w = hann_window(64);
        for i in 0..32 {
            assert!((w[i] - w[63 - i]).abs() < 1e-10);
        }
    }

    #[test]
    fn test_dft_forward_inverse_roundtrip() {
        // Small DFT roundtrip test
        let n = 16;
        let original: Vec<f64> = (0..n).map(|i| (i as f64 * 0.3).sin()).collect();
        let mut real = original.clone();
        let mut imag = vec![0.0; n];

        dft_forward(&mut real, &mut imag);
        dft_inverse(&mut real, &mut imag);

        for i in 0..n {
            assert!(
                (real[i] - original[i]).abs() < 1e-8,
                "roundtrip mismatch at {i}: expected {} got {}",
                original[i],
                real[i]
            );
        }
    }

    #[test]
    fn test_dft_parseval_theorem() {
        // Energy in time domain should equal energy in frequency domain / N
        let n = 32;
        let signal: Vec<f64> = (0..n).map(|i| (i as f64 * 0.2).sin()).collect();
        let time_energy: f64 = signal.iter().map(|x| x * x).sum();

        let mut real = signal;
        let mut imag = vec![0.0; n];
        dft_forward(&mut real, &mut imag);

        let freq_energy: f64 = real
            .iter()
            .zip(imag.iter())
            .map(|(r, i)| r * r + i * i)
            .sum::<f64>()
            / n as f64;

        assert!(
            (time_energy - freq_energy).abs() < 1e-6,
            "Parseval: time={time_energy:.6} freq={freq_energy:.6}"
        );
    }

    #[test]
    fn test_build_bin_gains_unity_corrections() {
        let config = SpectralBalanceConfig::new(48000.0, 1);
        let proc = SpectralBalanceProcessor::new(config, SpectralTarget::Flat)
            .expect("should succeed in test");

        // Zero corrections → all gains should be 1.0
        let corrections = vec![0.0; proc.num_bands()];
        let gains = proc.build_bin_gains(256, &corrections);
        assert_eq!(gains.len(), 256);
        for g in &gains {
            assert!((*g - 1.0).abs() < 1e-10, "expected unity gain, got {g}");
        }
    }

    #[test]
    fn test_build_bin_gains_positive_correction() {
        let config = SpectralBalanceConfig::new(48000.0, 1);
        let proc = SpectralBalanceProcessor::new(config, SpectralTarget::Flat)
            .expect("should succeed in test");

        let mut corrections = vec![0.0; proc.num_bands()];
        // +6 dB on all bands
        for c in &mut corrections {
            *c = 6.0;
        }
        let gains = proc.build_bin_gains(256, &corrections);
        let expected = 10.0_f64.powf(6.0 / 20.0);
        // Most bins (those covered by a band) should be ~2.0
        let boosted_count = gains
            .iter()
            .filter(|&&g| (g - expected).abs() < 0.01)
            .count();
        assert!(
            boosted_count > 100,
            "expected most bins boosted, got {boosted_count}"
        );
    }

    #[test]
    fn test_apply_perband_eq_preserves_silence() {
        let config = SpectralBalanceConfig {
            fft_size: 256,
            hop_size: 128,
            ..SpectralBalanceConfig::new(48000.0, 1)
        };
        let proc = SpectralBalanceProcessor::new(config, SpectralTarget::Flat)
            .expect("should succeed in test");

        let mut samples = vec![0.0_f32; 512];
        let corrections = vec![6.0; proc.num_bands()];
        proc.apply_perband_eq(&mut samples, &corrections);

        // Silence in → silence out (0 * any_gain = 0)
        for &s in &samples {
            assert!(s.abs() < 1e-10, "expected silence, got {s}");
        }
    }

    #[test]
    fn test_apply_perband_eq_signal_modification() {
        let config = SpectralBalanceConfig {
            fft_size: 256,
            hop_size: 128,
            ..SpectralBalanceConfig::new(48000.0, 1)
        };
        let mut proc = SpectralBalanceProcessor::new(config, SpectralTarget::Flat)
            .expect("should succeed in test");

        // Generate a 1 kHz sine at 48 kHz
        let n = 512;
        let samples_orig: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin() * 0.5)
            .collect();
        let mut samples = samples_orig.clone();
        proc.analyze(&samples);
        let corrections = proc.compute_corrections();
        proc.apply_perband_eq(&mut samples, &corrections);

        // Output should still be finite and non-trivial
        assert!(samples.iter().all(|s| s.is_finite()));
        let rms: f64 = samples
            .iter()
            .map(|&s| f64::from(s) * f64::from(s))
            .sum::<f64>()
            / samples.len() as f64;
        assert!(rms > 1e-6, "output should have energy, rms={rms}");
    }

    #[test]
    fn test_process_full_pipeline() {
        let config = SpectralBalanceConfig {
            fft_size: 256,
            hop_size: 128,
            ..SpectralBalanceConfig::new(48000.0, 1)
        };
        let mut proc = SpectralBalanceProcessor::new(config, SpectralTarget::Pink)
            .expect("should succeed in test");

        let n = 512;
        let mut samples: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48000.0).sin() * 0.3)
            .collect();

        proc.process(&mut samples);

        // After processing, frames_analyzed should be > 0
        assert!(proc.frames_analyzed() > 0);
        // Output must be finite
        assert!(samples.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_apply_perband_eq_too_short_falls_back_to_broadband() {
        let config = SpectralBalanceConfig {
            fft_size: 4096,
            hop_size: 2048,
            ..SpectralBalanceConfig::new(48000.0, 1)
        };
        let mut proc = SpectralBalanceProcessor::new(config, SpectralTarget::Flat)
            .expect("should succeed in test");

        // Input shorter than fft_size → falls back to broadband
        let mut samples = vec![0.5_f32; 100];
        proc.analyze(&samples);
        let corrections = proc.compute_corrections();
        proc.apply_perband_eq(&mut samples, &corrections);

        // Should not panic and samples should be modified
        assert!(samples.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_perband_eq_stereo() {
        let config = SpectralBalanceConfig {
            fft_size: 256,
            hop_size: 128,
            ..SpectralBalanceConfig::new(48000.0, 2)
        };
        let proc = SpectralBalanceProcessor::new(config, SpectralTarget::Flat)
            .expect("should succeed in test");

        // Stereo interleaved: 256 frames * 2 channels = 512 samples
        let n = 512;
        let mut samples: Vec<f32> = (0..n)
            .map(|i| {
                let frame = i / 2;
                (2.0 * std::f32::consts::PI * 1000.0 * frame as f32 / 48000.0).sin() * 0.4
            })
            .collect();

        let corrections = vec![0.0; proc.num_bands()]; // unity
        proc.apply_perband_eq(&mut samples, &corrections);

        // Stereo output should be finite
        assert!(samples.iter().all(|s| s.is_finite()));
    }
}
