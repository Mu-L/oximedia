#![allow(dead_code)]
//! Audio restoration tools for noise reduction and artifact removal.

use crate::error::{AudioPostError, AudioPostResult};
use oxifft::Complex;

/// Spectral noise reducer
#[derive(Debug)]
pub struct SpectralNoiseReducer {
    sample_rate: u32,
    fft_size: usize,
    noise_profile: Vec<f32>,
    reduction_amount: f32,
}

impl SpectralNoiseReducer {
    /// Create a new spectral noise reducer
    ///
    /// # Errors
    ///
    /// Returns an error if sample rate or FFT size is invalid
    pub fn new(sample_rate: u32, fft_size: usize) -> AudioPostResult<Self> {
        if sample_rate == 0 {
            return Err(AudioPostError::InvalidSampleRate(sample_rate));
        }
        if !fft_size.is_power_of_two() {
            return Err(AudioPostError::InvalidBufferSize(fft_size));
        }

        Ok(Self {
            sample_rate,
            fft_size,
            noise_profile: vec![0.0; fft_size / 2 + 1],
            reduction_amount: 0.8,
        })
    }

    /// Capture noise profile from a noise-only section
    pub fn capture_noise_profile(&mut self, noise_samples: &[f32]) {
        if noise_samples.len() < self.fft_size {
            return;
        }

        let input: Vec<Complex<f32>> = noise_samples
            .iter()
            .take(self.fft_size)
            .map(|&x| Complex::new(x, 0.0))
            .collect();

        let buffer = oxifft::fft(&input);

        // Store magnitude spectrum as noise profile
        for (i, profile_val) in self.noise_profile.iter_mut().enumerate() {
            if i < buffer.len() {
                *profile_val = buffer[i].norm();
            }
        }
    }

    /// Set reduction amount (0.0 to 1.0)
    pub fn set_reduction_amount(&mut self, amount: f32) {
        self.reduction_amount = amount.clamp(0.0, 1.0);
    }

    /// Process audio to reduce noise
    pub fn process(&self, _input: &[f32], output: &mut [f32]) {
        // Placeholder implementation
        // Real implementation would use spectral subtraction
        for (out, &inp) in output.iter_mut().zip(_input.iter()) {
            *out = inp;
        }
    }
}

/// Hiss remover
#[derive(Debug)]
pub struct HissRemover {
    sample_rate: u32,
    threshold: f32,
    reduction: f32,
}

impl HissRemover {
    /// Create a new hiss remover
    ///
    /// # Errors
    ///
    /// Returns an error if sample rate is invalid
    pub fn new(sample_rate: u32) -> AudioPostResult<Self> {
        if sample_rate == 0 {
            return Err(AudioPostError::InvalidSampleRate(sample_rate));
        }

        Ok(Self {
            sample_rate,
            threshold: -40.0,
            reduction: 0.8,
        })
    }

    /// Set threshold in dB
    ///
    /// # Errors
    ///
    /// Returns an error if threshold is invalid
    pub fn set_threshold(&mut self, threshold_db: f32) -> AudioPostResult<()> {
        if threshold_db > 0.0 {
            return Err(AudioPostError::InvalidThreshold(threshold_db));
        }
        self.threshold = threshold_db;
        Ok(())
    }

    /// Set reduction amount (0.0 to 1.0)
    pub fn set_reduction(&mut self, reduction: f32) {
        self.reduction = reduction.clamp(0.0, 1.0);
    }
}

/// Hum remover for removing 50/60 Hz and harmonics
#[derive(Debug)]
pub struct HumRemover {
    sample_rate: u32,
    fundamental_freq: f32,
    num_harmonics: usize,
}

impl HumRemover {
    /// Create a new hum remover
    ///
    /// # Errors
    ///
    /// Returns an error if sample rate is invalid
    pub fn new(sample_rate: u32, fundamental_freq: f32) -> AudioPostResult<Self> {
        if sample_rate == 0 {
            return Err(AudioPostError::InvalidSampleRate(sample_rate));
        }
        if fundamental_freq != 50.0 && fundamental_freq != 60.0 {
            return Err(AudioPostError::InvalidFrequency(fundamental_freq));
        }

        Ok(Self {
            sample_rate,
            fundamental_freq,
            num_harmonics: 10,
        })
    }

    /// Set number of harmonics to remove
    pub fn set_num_harmonics(&mut self, num_harmonics: usize) {
        self.num_harmonics = num_harmonics.clamp(1, 20);
    }

    /// Get harmonic frequencies
    #[must_use]
    pub fn get_harmonic_frequencies(&self) -> Vec<f32> {
        (1..=self.num_harmonics)
            .map(|i| self.fundamental_freq * i as f32)
            .collect()
    }
}

/// Click remover
#[derive(Debug)]
pub struct ClickRemover {
    sample_rate: u32,
    sensitivity: f32,
}

impl ClickRemover {
    /// Create a new click remover
    ///
    /// # Errors
    ///
    /// Returns an error if sample rate is invalid
    pub fn new(sample_rate: u32) -> AudioPostResult<Self> {
        if sample_rate == 0 {
            return Err(AudioPostError::InvalidSampleRate(sample_rate));
        }

        Ok(Self {
            sample_rate,
            sensitivity: 0.5,
        })
    }

    /// Set sensitivity (0.0 to 1.0)
    pub fn set_sensitivity(&mut self, sensitivity: f32) {
        self.sensitivity = sensitivity.clamp(0.0, 1.0);
    }

    /// Detect clicks in audio
    #[must_use]
    pub fn detect_clicks(&self, audio: &[f32]) -> Vec<usize> {
        let mut clicks = Vec::new();
        let threshold = self.sensitivity * 2.0;

        for i in 1..audio.len() - 1 {
            let diff = (audio[i] - audio[i - 1]).abs();
            if diff > threshold {
                clicks.push(i);
            }
        }

        clicks
    }

    /// Remove clicks from audio
    pub fn process(&self, input: &[f32], output: &mut [f32]) {
        let clicks = self.detect_clicks(input);

        output.copy_from_slice(input);

        // Interpolate over clicks
        for &click_pos in &clicks {
            if click_pos > 0 && click_pos < output.len() - 1 {
                output[click_pos] = (output[click_pos - 1] + output[click_pos + 1]) / 2.0;
            }
        }
    }
}

/// Declipping/decrackle processor
#[derive(Debug)]
pub struct Declipper {
    sample_rate: u32,
    threshold: f32,
}

impl Declipper {
    /// Create a new declipper
    ///
    /// # Errors
    ///
    /// Returns an error if sample rate is invalid
    pub fn new(sample_rate: u32) -> AudioPostResult<Self> {
        if sample_rate == 0 {
            return Err(AudioPostError::InvalidSampleRate(sample_rate));
        }

        Ok(Self {
            sample_rate,
            threshold: 0.95,
        })
    }

    /// Set clipping threshold (0.0 to 1.0)
    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold.clamp(0.0, 1.0);
    }

    /// Detect clipped regions
    #[must_use]
    pub fn detect_clipping(&self, audio: &[f32]) -> Vec<(usize, usize)> {
        let mut regions = Vec::new();
        let mut in_clip = false;
        let mut start = 0;

        for (i, &sample) in audio.iter().enumerate() {
            if sample.abs() >= self.threshold {
                if !in_clip {
                    start = i;
                    in_clip = true;
                }
            } else if in_clip {
                regions.push((start, i));
                in_clip = false;
            }
        }

        if in_clip {
            regions.push((start, audio.len()));
        }

        regions
    }

    /// Process audio to repair clipping
    pub fn process(&self, input: &[f32], output: &mut [f32]) {
        output.copy_from_slice(input);

        let clipped_regions = self.detect_clipping(input);

        for (start, end) in clipped_regions {
            // Simple interpolation (real implementation would be more sophisticated)
            if start > 0 && end < output.len() {
                let start_val = output[start.saturating_sub(1)];
                let end_val = output[end.min(output.len() - 1)];
                let range = end - start;

                for (i, sample) in output.iter_mut().enumerate().take(end).skip(start) {
                    let t = (i - start) as f32 / range as f32;
                    *sample = start_val * (1.0 - t) + end_val * t;
                }
            }
        }
    }
}

/// Spectral repair tool
#[derive(Debug)]
pub struct SpectralRepair {
    sample_rate: u32,
    fft_size: usize,
}

impl SpectralRepair {
    /// Create a new spectral repair tool
    ///
    /// # Errors
    ///
    /// Returns an error if sample rate or FFT size is invalid
    pub fn new(sample_rate: u32, fft_size: usize) -> AudioPostResult<Self> {
        if sample_rate == 0 {
            return Err(AudioPostError::InvalidSampleRate(sample_rate));
        }
        if !fft_size.is_power_of_two() {
            return Err(AudioPostError::InvalidBufferSize(fft_size));
        }

        Ok(Self {
            sample_rate,
            fft_size,
        })
    }

    /// Repair a frequency range using interpolation
    pub fn repair_frequency_range(
        &self,
        _input: &[f32],
        _output: &mut [f32],
        _freq_start: f32,
        _freq_end: f32,
    ) {
        // Placeholder implementation
        // Real implementation would use spectral interpolation
    }
}

// ── Vinyl Click/Pop Removal ───────────────────────────────────────────────────

/// Vinyl click and pop detector/remover for analogue-sourced audio.
///
/// The algorithm operates in two stages:
///
/// 1. **Detection** – Computes a local derivative (first difference) of the
///    signal and flags samples where the absolute derivative exceeds a
///    threshold derived from the local short-term variance of the signal.
///    Consecutive flagged samples are merged into click *regions*.
///
/// 2. **Interpolation** – Each click region is replaced by a cubic Hermite
///    spline interpolated from the samples immediately before and after the
///    region, preserving continuity of value and gradient across the boundary.
#[derive(Debug)]
pub struct VinylClickRemover {
    /// Sample rate.
    pub sample_rate: u32,
    /// Sensitivity multiplier.  A smaller value is more sensitive (detects more
    /// clicks); a larger value is less sensitive.  Default: 6.0.
    pub sensitivity: f32,
    /// Maximum region length (samples) that will be treated as a click.
    /// Longer artefacts are treated as pops and may be repaired with a broader
    /// interpolation window.  Default: 64.
    pub max_click_samples: usize,
    /// Minimum gap (samples) between two click regions before they are merged.
    /// Default: 4.
    pub merge_gap: usize,
    /// Whether to use adaptive variance for threshold computation (slower but
    /// more accurate on material with large dynamic swings).  Default: true.
    pub adaptive_threshold: bool,
    /// Local variance estimation window (samples).  Default: 256.
    pub variance_window: usize,
}

impl VinylClickRemover {
    /// Create a new vinyl click remover with default parameters.
    ///
    /// # Errors
    ///
    /// Returns [`AudioPostError::InvalidSampleRate`] for a zero sample rate.
    pub fn new(sample_rate: u32) -> crate::error::AudioPostResult<Self> {
        if sample_rate == 0 {
            return Err(AudioPostError::InvalidSampleRate(sample_rate));
        }
        Ok(Self {
            sample_rate,
            sensitivity: 6.0,
            max_click_samples: 64,
            merge_gap: 4,
            adaptive_threshold: true,
            variance_window: 256,
        })
    }

    /// Detect click/pop regions in `audio`.
    ///
    /// Returns a `Vec<(start, end)>` of sample index ranges (exclusive end).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn detect_clicks(&self, audio: &[f32]) -> Vec<(usize, usize)> {
        let n = audio.len();
        if n < 4 {
            return vec![];
        }

        // Compute first-difference (derivative) signal.
        let mut diff: Vec<f32> = vec![0.0; n];
        for i in 1..n {
            diff[i] = audio[i] - audio[i - 1];
        }

        // Compute local variance in a sliding window (or global if not adaptive).
        let variance: Vec<f32> = if self.adaptive_threshold {
            let half = self.variance_window / 2;
            let mut var_vec = vec![0.0f32; n];
            for center in 0..n {
                let start = center.saturating_sub(half);
                let end = (center + half).min(n);
                let len = end - start;
                if len == 0 {
                    continue;
                }
                let mean: f32 = diff[start..end].iter().sum::<f32>() / len as f32;
                let variance_val: f32 = diff[start..end]
                    .iter()
                    .map(|&x| {
                        let d = x - mean;
                        d * d
                    })
                    .sum::<f32>()
                    / len as f32;
                var_vec[center] = variance_val;
            }
            var_vec
        } else {
            let mean: f32 = diff.iter().sum::<f32>() / n as f32;
            let global_var = diff.iter().map(|&x| (x - mean) * (x - mean)).sum::<f32>() / n as f32;
            vec![global_var; n]
        };

        // Flag samples where |diff| > sensitivity * sqrt(variance).
        let mut flagged = vec![false; n];
        for i in 0..n {
            let threshold = self.sensitivity * variance[i].sqrt().max(1e-9);
            if diff[i].abs() > threshold {
                flagged[i] = true;
            }
        }

        // Merge consecutive flagged samples into regions.
        let mut regions: Vec<(usize, usize)> = Vec::new();
        let mut in_region = false;
        let mut region_start = 0;

        for i in 0..n {
            if flagged[i] {
                if !in_region {
                    region_start = i;
                    in_region = true;
                }
            } else if in_region {
                regions.push((region_start, i));
                in_region = false;
            }
        }
        if in_region {
            regions.push((region_start, n));
        }

        // Merge nearby regions separated by less than `merge_gap`.
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (start, end) in regions {
            if let Some(last) = merged.last_mut() {
                if start <= last.1 + self.merge_gap {
                    last.1 = last.1.max(end);
                    continue;
                }
            }
            merged.push((start, end));
        }

        // Filter out regions exceeding max_click_samples (treat separately as pops).
        merged
            .into_iter()
            .filter(|(s, e)| e - s <= self.max_click_samples)
            .collect()
    }

    /// Remove clicks from `input` and write the result to `output` using
    /// cubic Hermite spline interpolation across each detected click region.
    ///
    /// `output` must be the same length as `input`.
    pub fn process(&self, input: &[f32], output: &mut [f32]) {
        let n = input.len();
        output[..n].copy_from_slice(&input[..n]);

        let regions = self.detect_clicks(input);

        for (start, end) in regions {
            // Boundary samples for Hermite interpolation.
            // We need the sample before `start` and the sample at `end`.
            let x0 = start.saturating_sub(1);
            let x3 = end.min(n - 1);

            if x0 >= x3 || x3 >= n {
                continue;
            }

            let p0 = output[x0];
            let p1 = if x3 > 0 { output[x3] } else { p0 };
            // Derivatives at boundaries (central difference where possible).
            let m0 = if x0 > 0 && x0 + 1 < n {
                (output[x0 + 1] - output[x0.saturating_sub(1)]) * 0.5
            } else {
                0.0
            };
            let m1 = if x3 > 0 && x3 + 1 < n {
                (output[x3 + 1] - output[x3 - 1]) * 0.5
            } else {
                0.0
            };

            let region_len = (x3 - x0).max(1) as f32;

            for i in (x0 + 1)..x3 {
                let t = (i - x0) as f32 / region_len;
                // Cubic Hermite basis functions.
                let t2 = t * t;
                let t3 = t2 * t;
                let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
                let h10 = t3 - 2.0 * t2 + t;
                let h01 = -2.0 * t3 + 3.0 * t2;
                let h11 = t3 - t2;
                output[i] = h00 * p0 + h10 * m0 * region_len + h01 * p1 + h11 * m1 * region_len;
            }
        }
    }

    /// Process audio in-place.
    pub fn process_inplace(&self, audio: &mut [f32]) {
        let input = audio.to_vec();
        self.process(&input, audio);
    }
}

/// Phase correction tool
#[derive(Debug)]
pub struct PhaseCorrector {
    sample_rate: u32,
}

impl PhaseCorrector {
    /// Create a new phase corrector
    ///
    /// # Errors
    ///
    /// Returns an error if sample rate is invalid
    pub fn new(sample_rate: u32) -> AudioPostResult<Self> {
        if sample_rate == 0 {
            return Err(AudioPostError::InvalidSampleRate(sample_rate));
        }

        Ok(Self { sample_rate })
    }

    /// Analyze phase correlation between stereo channels
    #[must_use]
    pub fn analyze_phase_correlation(&self, left: &[f32], right: &[f32]) -> f32 {
        if left.len() != right.len() || left.is_empty() {
            return 0.0;
        }

        let mut correlation = 0.0;
        for (l, r) in left.iter().zip(right.iter()) {
            correlation += l * r;
        }

        correlation / left.len() as f32
    }

    /// Correct phase issues
    pub fn correct_phase(&self, input: &[f32], output: &mut [f32]) {
        output.copy_from_slice(input);
        // Placeholder - real implementation would apply phase correction
    }
}

/// Stereo enhancement
#[derive(Debug)]
pub struct StereoEnhancer {
    sample_rate: u32,
    width: f32,
}

impl StereoEnhancer {
    /// Create a new stereo enhancer
    ///
    /// # Errors
    ///
    /// Returns an error if sample rate is invalid
    pub fn new(sample_rate: u32) -> AudioPostResult<Self> {
        if sample_rate == 0 {
            return Err(AudioPostError::InvalidSampleRate(sample_rate));
        }

        Ok(Self {
            sample_rate,
            width: 1.0,
        })
    }

    /// Set stereo width (0.0 = mono, 1.0 = normal, >1.0 = enhanced)
    pub fn set_width(&mut self, width: f32) {
        self.width = width.max(0.0);
    }

    /// Process stereo audio
    pub fn process(
        &self,
        left: &[f32],
        right: &[f32],
        out_left: &mut [f32],
        out_right: &mut [f32],
    ) {
        let len = left
            .len()
            .min(right.len())
            .min(out_left.len())
            .min(out_right.len());

        for (_i, ((l, r), (ol, or))) in left
            .iter()
            .zip(right.iter())
            .zip(out_left.iter_mut().zip(out_right.iter_mut()))
            .enumerate()
            .take(len)
        {
            let mid = (l + r) / 2.0;
            let side = (l - r) / 2.0;

            *ol = mid + side * self.width;
            *or = mid - side * self.width;
        }
    }
}

// ── Free-function DSP: declick + spectral subtraction ────────────────────────

/// Configuration for the MAD-based declick algorithm.
#[derive(Debug, Clone)]
pub struct DeclickConfig {
    /// Threshold multiplier over the Median Absolute Deviation (default 8.0).
    pub mad_threshold: f32,
    /// Number of surrounding samples used for cubic-spline interpolation (default 50).
    pub interpolation_radius: usize,
}

impl Default for DeclickConfig {
    fn default() -> Self {
        Self {
            mad_threshold: 8.0,
            interpolation_radius: 50,
        }
    }
}

/// Remove impulsive clicks from `samples` and return the cleaned audio.
///
/// Algorithm:
/// 1. Compute first-differences of the input.
/// 2. Estimate the Median Absolute Deviation (MAD) of those differences.
/// 3. Flag any sample whose first-difference exceeds `config.mad_threshold × MAD`.
/// 4. Replace each flagged sample by cubic Hermite interpolation from the
///    `config.interpolation_radius` nearest non-flagged neighbours.
///
/// # Errors
///
/// Returns `AudioPostError::InvalidBufferSize` if the input is empty.
#[allow(clippy::cast_precision_loss)]
pub fn declick(samples: &[f32], config: &DeclickConfig) -> AudioPostResult<Vec<f32>> {
    if samples.is_empty() {
        return Err(AudioPostError::InvalidBufferSize(0));
    }
    let n = samples.len();
    let mut output = samples.to_vec();

    if n < 3 {
        return Ok(output);
    }

    // Compute first-differences.
    let mut diffs: Vec<f32> = vec![0.0; n];
    for i in 1..n {
        diffs[i] = samples[i] - samples[i - 1];
    }

    // Median Absolute Deviation of diffs[1..].
    let mut sorted_diffs: Vec<f32> = diffs[1..].iter().map(|&x| x.abs()).collect();
    sorted_diffs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = {
        let m = sorted_diffs.len();
        if m == 0 {
            1e-9_f32
        } else if m % 2 == 1 {
            sorted_diffs[m / 2]
        } else {
            (sorted_diffs[m / 2 - 1] + sorted_diffs[m / 2]) / 2.0
        }
    };
    let mad = median.max(1e-9);
    let threshold = config.mad_threshold * mad;

    // Flag click positions (indices into the original signal).
    let mut flagged = vec![false; n];
    for i in 1..n {
        if diffs[i].abs() > threshold {
            flagged[i] = true;
        }
    }

    // Repair each flagged position with cubic Hermite interpolation.
    let radius = config.interpolation_radius;
    let mut i = 0;
    while i < n {
        if flagged[i] {
            // Find the extent of consecutive flagged samples.
            let region_start = i;
            while i < n && flagged[i] {
                i += 1;
            }
            let region_end = i; // exclusive

            // Anchor points: last non-flagged before and first non-flagged after.
            let p0_idx = region_start.saturating_sub(1);
            let p1_idx = region_end.min(n - 1);

            // Derivatives (central-difference or zero at boundaries).
            let m0 = if p0_idx > 0 && p0_idx + 1 < n {
                let lo = p0_idx.saturating_sub(radius.min(p0_idx));
                let hi = (p0_idx + radius).min(n - 1);
                (samples[hi] - samples[lo]) / (2.0 * (hi - lo).max(1) as f32)
            } else {
                0.0
            };
            let m1 = if p1_idx + 1 < n {
                let lo = p1_idx.saturating_sub(radius.min(p1_idx));
                let hi = (p1_idx + radius).min(n - 1);
                (samples[hi] - samples[lo]) / (2.0 * (hi - lo).max(1) as f32)
            } else {
                0.0
            };

            let v0 = output[p0_idx];
            let v1 = output[p1_idx];
            let span = (p1_idx - p0_idx).max(1) as f32;

            for j in region_start..region_end {
                let t = (j - p0_idx) as f32 / span;
                let t2 = t * t;
                let t3 = t2 * t;
                let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
                let h10 = t3 - 2.0 * t2 + t;
                let h01 = -2.0 * t3 + 3.0 * t2;
                let h11 = t3 - t2;
                output[j] = h00 * v0 + h10 * m0 * span + h01 * v1 + h11 * m1 * span;
            }
        } else {
            i += 1;
        }
    }

    Ok(output)
}

/// Configuration for Boll (1979) spectral subtraction + Wiener post-filter.
#[derive(Debug, Clone)]
pub struct SpectralSubtractionConfig {
    /// STFT frame size (must be a power of two, default 1024).
    pub fft_size: usize,
    /// STFT hop size (default 512 → 50 % overlap).
    pub hop_size: usize,
    /// Fraction of the quietest frames used to estimate the noise PSD
    /// (default 0.05 → quietest 5 %).
    pub noise_percentile: f32,
    /// Over-subtraction factor α (default 2.0).
    pub alpha: f32,
    /// Spectral floor factor β (default 0.05).
    pub beta: f32,
}

impl Default for SpectralSubtractionConfig {
    fn default() -> Self {
        Self {
            fft_size: 1024,
            hop_size: 512,
            noise_percentile: 0.05,
            alpha: 2.0,
            beta: 0.05,
        }
    }
}

/// Spectral subtraction noise reduction (Boll, 1979) with Wiener post-filter.
///
/// Signal flow per frame:
/// 1. Apply Hann window.
/// 2. Forward FFT (OxiFFT).
/// 3. Compute per-bin signal PSD.
/// 4. Subtract `α × noisePSD`; floor at `β × noisePSD`.
/// 5. Compute Wiener gain = `signalPSD / (signalPSD + noisePSD)`, clamped to `[0, 1]`.
/// 6. Apply gain to complex spectrum; IFFT; overlap-add.
///
/// Noise PSD is estimated from the quietest `noise_percentile` frames.
///
/// # Errors
///
/// Returns `AudioPostError::InvalidBufferSize` if the buffer size is wrong or
/// `AudioPostError::Generic` on internal failures.
#[allow(clippy::cast_precision_loss)]
pub fn spectral_subtract(
    samples: &[f32],
    config: &SpectralSubtractionConfig,
) -> AudioPostResult<Vec<f32>> {
    if samples.is_empty() {
        return Err(AudioPostError::InvalidBufferSize(0));
    }
    if !config.fft_size.is_power_of_two() || config.fft_size < 4 {
        return Err(AudioPostError::InvalidBufferSize(config.fft_size));
    }
    if config.hop_size == 0 || config.hop_size > config.fft_size {
        return Err(AudioPostError::InvalidBufferSize(config.hop_size));
    }

    let n = samples.len();
    let fft_size = config.fft_size;
    let hop = config.hop_size;

    // Pre-compute Hann window.
    let hann: Vec<f32> = (0..fft_size)
        .map(|i| {
            let t = i as f32 / (fft_size - 1) as f32;
            0.5 * (1.0 - (2.0 * std::f32::consts::PI * t).cos())
        })
        .collect();

    // ── Pass 1: collect all frame PSDs to estimate the noise floor ───────────
    let num_frames = (n + hop - 1) / hop;
    let mut frame_energies: Vec<(f32, usize)> = Vec::with_capacity(num_frames);
    let mut frame_psds: Vec<Vec<f32>> = Vec::with_capacity(num_frames);

    for frame_idx in 0..num_frames {
        let start = frame_idx * hop;

        // Build windowed, zero-padded complex input.
        let input: Vec<Complex<f32>> = (0..fft_size)
            .map(|j| {
                let sample_idx = start + j;
                let s = if sample_idx < n {
                    samples[sample_idx]
                } else {
                    0.0
                };
                Complex::new(s * hann[j], 0.0)
            })
            .collect();

        let spectrum = oxifft::fft(&input);
        let psd: Vec<f32> = spectrum.iter().map(|c| c.norm_sqr()).collect();
        let energy: f32 = psd.iter().sum();
        frame_energies.push((energy, frame_idx));
        frame_psds.push(psd);
    }

    // Sort frames by energy to find the quietest ones.
    let mut sorted_energies = frame_energies.clone();
    sorted_energies.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let noise_frames = ((num_frames as f32 * config.noise_percentile).ceil() as usize).max(1);
    let mut noise_psd = vec![0.0_f32; fft_size];
    for &(_, fi) in sorted_energies.iter().take(noise_frames) {
        for (bin, &p) in noise_psd.iter_mut().zip(frame_psds[fi].iter()) {
            *bin += p;
        }
    }
    let noise_scale = 1.0 / noise_frames as f32;
    for p in &mut noise_psd {
        *p *= noise_scale;
    }

    // ── Pass 2: process each frame with spectral subtraction + Wiener ────────
    let mut output = vec![0.0_f32; n + fft_size];
    // Overlap-add normalisation weights.
    let mut ola_weights = vec![0.0_f32; n + fft_size];

    for frame_idx in 0..num_frames {
        let start = frame_idx * hop;

        // Re-build windowed input.
        let input: Vec<Complex<f32>> = (0..fft_size)
            .map(|j| {
                let sample_idx = start + j;
                let s = if sample_idx < n {
                    samples[sample_idx]
                } else {
                    0.0
                };
                Complex::new(s * hann[j], 0.0)
            })
            .collect();

        let mut spectrum = oxifft::fft(&input);

        // Spectral subtraction + Wiener gain per bin.
        for (bin_idx, spec_bin) in spectrum.iter_mut().enumerate() {
            let signal_psd = spec_bin.norm_sqr();
            let noise_p = noise_psd[bin_idx];

            // Over-subtraction with spectral floor.
            let enhanced_psd = (signal_psd - config.alpha * noise_p).max(config.beta * noise_p);

            // Wiener gain = signal / (signal + noise), clamped [0, 1].
            let gain = if signal_psd + noise_p > 1e-30 {
                (enhanced_psd / (enhanced_psd + noise_p)).clamp(0.0, 1.0)
            } else {
                0.0
            };

            *spec_bin = *spec_bin * gain;
        }

        // IFFT and overlap-add.
        // oxifft::ifft() is already normalized (divides by N internally),
        // so no additional inv_fft_size scaling is needed here.
        let recovered = oxifft::ifft(&spectrum);

        for j in 0..fft_size {
            let out_idx = start + j;
            if out_idx < output.len() {
                output[out_idx] += recovered[j].re * hann[j];
                ola_weights[out_idx] += hann[j] * hann[j];
            }
        }
    }

    // Normalise by OLA weights and truncate to input length.
    // For 50% overlap with a Hann window, the COLA constant is ~0.5.
    // Samples where the accumulated weight is below 0.25 (edge samples where
    // only one frame's Hann lobe contributes near its zero-crossing) produce
    // enormous values when divided by the tiny weight.  Zero those samples
    // instead — at most fft_size/2 samples are affected at each boundary.
    for (s, &w) in output.iter_mut().zip(ola_weights.iter()) {
        if w >= 0.25 {
            *s /= w;
        } else {
            *s = 0.0;
        }
    }
    output.truncate(n);

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that oxifft::ifft is normalized (roundtrip recovers input).
    #[test]
    fn debug_oxifft_normalization() {
        let x: Vec<oxifft::Complex<f32>> = (0..8_usize)
            .map(|i| oxifft::Complex::new(i as f32, 0.0))
            .collect();
        let f = oxifft::fft(&x);
        let r = oxifft::ifft(&f);
        eprintln!("input[7]={}, recovered[7]={}", x[7].re, r[7].re);
        // If normalized, recovered[7] ≈ 7.0; if not normalized, recovered[7] ≈ 56.0.
        let diff = (r[7].re - x[7].re).abs();
        assert!(
            diff < 0.01,
            "oxifft::ifft is not normalized! input[7]={}, recovered[7]={}",
            x[7].re,
            r[7].re
        );
    }

    /// Diagnostic: check actual SNR values in spectral subtraction.
    #[test]
    fn debug_spectral_subtract_snr() {
        use std::f32::consts::PI;
        let sample_rate = 48_000_u32;
        let n = 4096_usize;
        let tone_amp = 0.5_f32;
        let noise_amp = 0.3_f32;
        let tone: Vec<f32> = (0..n)
            .map(|i| tone_amp * (2.0 * PI * 1000.0 * i as f32 / sample_rate as f32).sin())
            .collect();
        let noise: Vec<f32> = (0..n)
            .map(|i| {
                let x = (i as f32 * 12.9898 + 78.233).sin() * 43758.5453;
                (x - x.floor() - 0.5) * 2.0 * noise_amp
            })
            .collect();
        let mixed: Vec<f32> = tone.iter().zip(noise.iter()).map(|(t, s)| t + s).collect();
        let config = SpectralSubtractionConfig {
            fft_size: 1024,
            hop_size: 512,
            noise_percentile: 0.05,
            alpha: 2.0,
            beta: 0.05,
        };
        let result = spectral_subtract(&mixed, &config).expect("spectral_subtract ok");
        let tone_rms = (tone.iter().map(|x| x * x).sum::<f32>() / n as f32).sqrt();
        let result_rms = (result.iter().map(|x| x * x).sum::<f32>() / n as f32).sqrt();
        let residual: Vec<f32> = result.iter().zip(tone.iter()).map(|(r, t)| r - t).collect();
        let residual_power: f32 = residual.iter().map(|x| x * x).sum::<f32>() / n as f32;
        let tone_power: f32 = tone.iter().map(|x| x * x).sum::<f32>() / n as f32;
        let noise_power: f32 = noise.iter().map(|x| x * x).sum::<f32>() / n as f32;
        let input_snr = 10.0 * (tone_power / noise_power).log10();
        let output_snr = 10.0 * (tone_power / residual_power.max(1e-30)).log10();
        eprintln!("tone_rms={tone_rms:.4}, result_rms={result_rms:.4}");
        eprintln!("input_snr={input_snr:.2} dB, output_snr={output_snr:.2} dB");
        eprintln!("result[100..105]: {:?}", &result[100..105]);
        eprintln!("tone[100..105]: {:?}", &tone[100..105]);
    }

    #[test]
    fn test_spectral_noise_reducer() {
        let mut reducer = SpectralNoiseReducer::new(48000, 1024).expect("failed to create");
        let noise = vec![0.01_f32; 2048];
        reducer.capture_noise_profile(&noise);
        reducer.set_reduction_amount(0.7);
        assert_eq!(reducer.reduction_amount, 0.7);
    }

    #[test]
    fn test_hiss_remover() {
        let mut hiss_remover = HissRemover::new(48000).expect("failed to create");
        assert!(hiss_remover.set_threshold(-30.0).is_ok());
        hiss_remover.set_reduction(0.6);
        assert_eq!(hiss_remover.reduction, 0.6);
    }

    #[test]
    fn test_hum_remover() {
        let hum_remover = HumRemover::new(48000, 60.0).expect("failed to create");
        let harmonics = hum_remover.get_harmonic_frequencies();
        assert_eq!(harmonics[0], 60.0);
        assert_eq!(harmonics[1], 120.0);
    }

    #[test]
    fn test_invalid_fundamental_freq() {
        assert!(HumRemover::new(48000, 55.0).is_err());
    }

    #[test]
    fn test_click_remover() {
        let mut click_remover = ClickRemover::new(48000).expect("failed to create");
        click_remover.set_sensitivity(0.7);

        let mut audio = vec![0.0_f32; 100];
        audio[50] = 10.0; // Create a click

        let clicks = click_remover.detect_clicks(&audio);
        assert!(!clicks.is_empty());
    }

    #[test]
    fn test_click_removal() {
        let click_remover = ClickRemover::new(48000).expect("failed to create");
        let mut input = vec![0.0_f32; 100];
        input[50] = 10.0;

        let mut output = vec![0.0_f32; 100];
        click_remover.process(&input, &mut output);

        assert!(output[50].abs() < input[50].abs());
    }

    #[test]
    fn test_declipper() {
        let mut declipper = Declipper::new(48000).expect("failed to create");
        declipper.set_threshold(0.9);

        let mut audio = vec![0.5_f32; 100];
        audio[50] = 1.0; // Clipped sample

        let regions = declipper.detect_clipping(&audio);
        assert!(!regions.is_empty());
    }

    #[test]
    fn test_declipping_process() {
        let declipper = Declipper::new(48000).expect("failed to create");
        let mut input = vec![0.0_f32; 100];
        input[50] = 1.0;
        input[51] = 1.0;

        let mut output = vec![0.0_f32; 100];
        declipper.process(&input, &mut output);

        assert!(output[50] < 1.0);
    }

    #[test]
    fn test_spectral_repair() {
        let repair = SpectralRepair::new(48000, 2048).expect("failed to create");
        assert_eq!(repair.fft_size, 2048);
    }

    #[test]
    fn test_phase_corrector() {
        let corrector = PhaseCorrector::new(48000).expect("failed to create");
        let left = vec![1.0_f32; 100];
        let right = vec![1.0_f32; 100];

        let correlation = corrector.analyze_phase_correlation(&left, &right);
        assert!(correlation > 0.0);
    }

    #[test]
    fn test_stereo_enhancer() {
        let mut enhancer = StereoEnhancer::new(48000).expect("failed to create");
        enhancer.set_width(1.5);
        assert_eq!(enhancer.width, 1.5);
    }

    #[test]
    fn test_stereo_enhancement() {
        let enhancer = StereoEnhancer::new(48000).expect("failed to create");
        let left = vec![1.0_f32; 100];
        let right = vec![-1.0_f32; 100];
        let mut out_left = vec![0.0_f32; 100];
        let mut out_right = vec![0.0_f32; 100];

        enhancer.process(&left, &right, &mut out_left, &mut out_right);
        assert!(out_left[0] != 0.0);
    }

    #[test]
    fn test_invalid_fft_size() {
        assert!(SpectralNoiseReducer::new(48000, 1000).is_err());
    }

    // ── VinylClickRemover tests ───────────────────────────────────────────────

    #[test]
    fn test_vinyl_click_remover_creation() {
        let remover = VinylClickRemover::new(48000).expect("failed to create");
        assert_eq!(remover.sensitivity, 6.0);
    }

    #[test]
    fn test_vinyl_click_remover_invalid_sr() {
        assert!(VinylClickRemover::new(0).is_err());
    }

    #[test]
    fn test_vinyl_click_remover_detects_click() {
        let remover = VinylClickRemover::new(48000).expect("failed to create");
        // Create a smooth sine with a single impulse click.
        let mut audio: Vec<f32> = (0..512).map(|i| (i as f32 * 0.05).sin() * 0.3).collect();
        audio[256] += 5.0; // large click
        let clicks = remover.detect_clicks(&audio);
        assert!(!clicks.is_empty(), "Should detect the click");
    }

    #[test]
    fn test_vinyl_click_remover_no_false_positives_on_sine() {
        let mut remover = VinylClickRemover::new(48000).expect("failed to create");
        remover.sensitivity = 10.0; // high threshold
                                    // A clean sine wave should have few or zero detected clicks.
        let audio: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.05).sin()).collect();
        let clicks = remover.detect_clicks(&audio);
        assert!(
            clicks.len() < 5,
            "Too many false positives: {}",
            clicks.len()
        );
    }

    #[test]
    fn test_vinyl_click_remover_output_reduced_at_click() {
        let remover = VinylClickRemover::new(48000).expect("failed to create");
        let mut input: Vec<f32> = vec![0.0f32; 200];
        input[100] += 8.0; // very large click
        let mut output = vec![0.0f32; 200];
        remover.process(&input, &mut output);
        // After removal, the click sample should be reduced.
        assert!(
            output[100].abs() < input[100].abs(),
            "Click should be reduced; input={}, output={}",
            input[100],
            output[100]
        );
    }

    #[test]
    fn test_vinyl_click_remover_empty_input() {
        let remover = VinylClickRemover::new(48000).expect("failed to create");
        let input: Vec<f32> = vec![];
        let clicks = remover.detect_clicks(&input);
        assert!(clicks.is_empty());
    }

    #[test]
    fn test_vinyl_click_remover_process_inplace() {
        let remover = VinylClickRemover::new(48000).expect("failed to create");
        let mut audio = vec![0.0f32; 100];
        audio[50] += 9.0; // click
        let original_click = audio[50];
        remover.process_inplace(&mut audio);
        assert!(audio[50].abs() < original_click.abs());
    }

    #[test]
    fn test_restoration_noise_reduction_with_synthetic_profile() {
        let mut reducer = SpectralNoiseReducer::new(48000, 1024).expect("failed to create");
        // Generate synthetic noise profile from white noise.
        let noise: Vec<f32> = (0..2048)
            .map(|i| ((i as f32 * 17.3).sin()) * 0.02)
            .collect();
        reducer.capture_noise_profile(&noise);
        reducer.set_reduction_amount(0.9);

        // Process a signal that contains added noise.
        let signal: Vec<f32> = (0..1024)
            .map(|i| (i as f32 * 0.05).sin() * 0.3 + noise[i] * 0.1)
            .collect();
        let mut output = vec![0.0f32; 1024];
        reducer.process(&signal, &mut output);

        // Verify the profile was captured (non-zero).
        assert!(reducer.noise_profile.iter().any(|&v| v > 0.0));
    }
}
