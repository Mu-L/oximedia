//! Wiener filtering for noise reduction with optional SIMD-accelerated gain computation.
//!
//! On x86/x86-64 CPUs the inner gain-computation loop uses AVX2 (8-wide f32) when
//! the feature is available at runtime, falling back to SSE2 (4-wide f32) and finally
//! to a scalar path.  The unsafe blocks are restricted to the SIMD kernels and guarded
//! by `is_x86_feature_detected!` at runtime so the binary runs correctly on any CPU.

use crate::error::RestoreResult;
use crate::noise::profile::NoiseProfile;
use crate::utils::spectral::{apply_window, FftProcessor, WindowFunction};

// ---------------------------------------------------------------------------
// SIMD helpers
// ---------------------------------------------------------------------------

/// Scalar fallback: compute Wiener gain for each frequency bin.
///
/// `gain[i] = (snr[i] / (snr[i] + 1)).max(min_gain)`
/// where `snr[i] = signal_pow[i] / noise_pow[i]`
#[inline]
fn compute_wiener_gains_scalar(
    signal_mag: &[f32],
    noise_mag: &[f32],
    min_gain: f32,
    out: &mut [f32],
) {
    for ((&sm, &nm), o) in signal_mag.iter().zip(noise_mag.iter()).zip(out.iter_mut()) {
        let sp = sm * sm;
        let np = nm * nm;
        let snr = if np > f32::EPSILON { sp / np } else { 100.0 };
        *o = (snr / (snr + 1.0)).max(min_gain);
    }
}

/// Compute Wiener gains, choosing the widest SIMD path available at runtime.
///
/// The function always produces the same numerical result as `compute_wiener_gains_scalar`.
#[cfg(target_arch = "x86_64")]
#[allow(unsafe_code)]
fn compute_wiener_gains(signal_mag: &[f32], noise_mag: &[f32], min_gain: f32, out: &mut [f32]) {
    // Runtime dispatch: prefer AVX2, then SSE2, then scalar.
    if is_x86_feature_detected!("avx2") {
        // SAFETY: we have verified AVX2 is available at runtime.
        unsafe { compute_wiener_gains_avx2(signal_mag, noise_mag, min_gain, out) }
    } else if is_x86_feature_detected!("sse2") {
        // SAFETY: we have verified SSE2 is available at runtime.
        unsafe { compute_wiener_gains_sse2(signal_mag, noise_mag, min_gain, out) }
    } else {
        compute_wiener_gains_scalar(signal_mag, noise_mag, min_gain, out);
    }
}

#[cfg(not(target_arch = "x86_64"))]
#[inline]
fn compute_wiener_gains(signal_mag: &[f32], noise_mag: &[f32], min_gain: f32, out: &mut [f32]) {
    compute_wiener_gains_scalar(signal_mag, noise_mag, min_gain, out);
}

/// AVX2 implementation: processes 8 bins per iteration.
///
/// Formula (per bin):
/// ```text
/// snr  = sm*sm / (nm*nm)   [clamped to 100.0 when nm≈0]
/// gain = snr / (snr + 1.0) [clamped to min_gain]
/// ```
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[allow(unsafe_code)]
unsafe fn compute_wiener_gains_avx2(
    signal_mag: &[f32],
    noise_mag: &[f32],
    min_gain: f32,
    out: &mut [f32],
) {
    use std::arch::x86_64::*;

    let n = signal_mag.len().min(noise_mag.len()).min(out.len());
    let chunks = n / 8;

    let eps = _mm256_set1_ps(f32::EPSILON);
    let high_snr = _mm256_set1_ps(100.0_f32);
    let one = _mm256_set1_ps(1.0_f32);
    let mg = _mm256_set1_ps(min_gain);

    for i in 0..chunks {
        let base = i * 8;
        // Load 8 signal and noise magnitudes
        let sm = _mm256_loadu_ps(signal_mag.as_ptr().add(base));
        let nm = _mm256_loadu_ps(noise_mag.as_ptr().add(base));

        let sp = _mm256_mul_ps(sm, sm); // sm²
        let np = _mm256_mul_ps(nm, nm); // nm²

        // mask where np > epsilon
        let np_ok = _mm256_cmp_ps(np, eps, _CMP_GT_OQ);

        // snr = sp / np  (or 100.0 where np ≈ 0)
        let raw_snr = _mm256_div_ps(sp, np);
        let snr = _mm256_blendv_ps(high_snr, raw_snr, np_ok);

        // gain = snr / (snr + 1)
        let denom = _mm256_add_ps(snr, one);
        let gain = _mm256_div_ps(snr, denom);

        // clamp to min_gain
        let clamped = _mm256_max_ps(gain, mg);

        _mm256_storeu_ps(out.as_mut_ptr().add(base), clamped);
    }

    // Scalar tail
    let tail_start = chunks * 8;
    compute_wiener_gains_scalar(
        &signal_mag[tail_start..n],
        &noise_mag[tail_start..n],
        min_gain,
        &mut out[tail_start..n],
    );
}

/// SSE2 implementation: processes 4 bins per iteration.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
#[allow(unsafe_code)]
unsafe fn compute_wiener_gains_sse2(
    signal_mag: &[f32],
    noise_mag: &[f32],
    min_gain: f32,
    out: &mut [f32],
) {
    use std::arch::x86_64::*;

    let n = signal_mag.len().min(noise_mag.len()).min(out.len());
    let chunks = n / 4;

    let eps = _mm_set1_ps(f32::EPSILON);
    let high_snr = _mm_set1_ps(100.0_f32);
    let one = _mm_set1_ps(1.0_f32);
    let mg = _mm_set1_ps(min_gain);

    for i in 0..chunks {
        let base = i * 4;
        let sm = _mm_loadu_ps(signal_mag.as_ptr().add(base));
        let nm = _mm_loadu_ps(noise_mag.as_ptr().add(base));

        let sp = _mm_mul_ps(sm, sm);
        let np = _mm_mul_ps(nm, nm);

        // SSE2 lacks cmpgt returning a float mask directly; use _mm_cmpgt_ps
        let np_ok = _mm_cmpgt_ps(np, eps);

        let raw_snr = _mm_div_ps(sp, np);
        // blend: if np_ok bit set use raw_snr, else high_snr
        // SSE2 blend via bitwise ops: result = (raw_snr & np_ok) | (high_snr & ~np_ok)
        let selected = _mm_or_ps(_mm_and_ps(raw_snr, np_ok), _mm_andnot_ps(np_ok, high_snr));

        let denom = _mm_add_ps(selected, one);
        let gain = _mm_div_ps(selected, denom);
        let clamped = _mm_max_ps(gain, mg);

        _mm_storeu_ps(out.as_mut_ptr().add(base), clamped);
    }

    // Scalar tail
    let tail_start = chunks * 4;
    compute_wiener_gains_scalar(
        &signal_mag[tail_start..n],
        &noise_mag[tail_start..n],
        min_gain,
        &mut out[tail_start..n],
    );
}

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

            // Compute Wiener filter gains — uses SIMD where available
            let n = magnitude.len().min(self.noise_profile.magnitude.len());
            let mut raw_gains = vec![0.0f32; n];
            compute_wiener_gains(
                &magnitude[..n],
                &self.noise_profile.magnitude[..n],
                self.config.min_gain,
                &mut raw_gains,
            );

            // Smooth gains over time and apply to magnitude
            let mut processed_mag = vec![0.0f32; magnitude.len()];
            for i in 0..n {
                let smoothed = self.config.smoothing * self.prev_gain[i]
                    + (1.0 - self.config.smoothing) * raw_gains[i];
                self.prev_gain[i] = smoothed;
                processed_mag[i] = magnitude[i] * smoothed;
            }
            // Any extra bins (should not occur normally) — pass through at min_gain
            for i in n..magnitude.len() {
                processed_mag[i] = magnitude[i] * self.config.min_gain;
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
        use rand::RngExt;
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
        use rand::RngExt;
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
        use rand::RngExt;
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

    #[test]
    fn test_compute_wiener_gains_scalar_vs_simd() {
        // Verify SIMD path produces numerically equivalent output to scalar path.
        let n = 64_usize;
        let signal_mag: Vec<f32> = (0..n).map(|i| (i + 1) as f32 * 0.1).collect();
        let noise_mag: Vec<f32> = (0..n).map(|i| (i + 1) as f32 * 0.05).collect();
        let min_gain = 0.01_f32;

        let mut scalar_out = vec![0.0_f32; n];
        compute_wiener_gains_scalar(&signal_mag, &noise_mag, min_gain, &mut scalar_out);

        let mut simd_out = vec![0.0_f32; n];
        compute_wiener_gains(&signal_mag, &noise_mag, min_gain, &mut simd_out);

        for (i, (&s, &d)) in scalar_out.iter().zip(simd_out.iter()).enumerate() {
            assert!((s - d).abs() < 1e-5, "bin {i}: scalar={s} simd={d}");
        }
    }

    #[test]
    fn test_compute_wiener_gains_zero_noise() {
        // When noise magnitude is ~0, gain should be clamped to min_gain, not NaN/inf.
        let signal_mag = vec![1.0_f32; 16];
        let noise_mag = vec![0.0_f32; 16];
        let min_gain = 0.01_f32;

        let mut out = vec![0.0_f32; 16];
        compute_wiener_gains_scalar(&signal_mag, &noise_mag, min_gain, &mut out);

        for (i, &g) in out.iter().enumerate() {
            assert!(g.is_finite(), "gain at bin {i} is not finite: {g}");
            assert!(g >= min_gain, "gain at bin {i} below min_gain: {g}");
        }
    }
}
