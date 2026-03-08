//! True peak detection using 4x oversampling.
//!
//! Implements ITU-R BS.1770-4 true peak measurement to detect inter-sample peaks
//! that would occur during digital-to-analog conversion.
//!
//! Uses windowed sinc interpolation (Lanczos) for 4x oversampling.

#![allow(clippy::similar_names)]

use std::f64::consts::PI;

/// Oversampling factor for true peak detection.
const OVERSAMPLE_FACTOR: usize = 4;

/// Lanczos window size (a=3 gives good results).
const LANCZOS_A: usize = 3;

/// Number of filter taps per phase.
const TAPS_PER_PHASE: usize = LANCZOS_A * 2;

/// True peak value with metadata.
#[derive(Clone, Copy, Debug)]
pub struct TruePeak {
    /// Peak value in linear scale (0.0 to inf).
    pub linear: f64,
    /// Peak value in dBTP (dB True Peak).
    pub dbtp: f64,
    /// Sample index where peak occurred.
    pub sample_index: usize,
    /// Channel where peak occurred.
    pub channel: usize,
}

impl TruePeak {
    /// Create a new true peak measurement.
    pub fn new(linear: f64, channel: usize, sample_index: usize) -> Self {
        let dbtp = if linear > 0.0 {
            20.0 * linear.log10()
        } else {
            f64::NEG_INFINITY
        };

        Self {
            linear,
            dbtp,
            sample_index,
            channel,
        }
    }

    /// Check if peak exceeds a threshold in dBTP.
    pub fn exceeds(&self, threshold_dbtp: f64) -> bool {
        self.dbtp > threshold_dbtp
    }
}

/// True peak detector with 4x oversampling.
///
/// Uses Lanczos-windowed sinc resampling to detect inter-sample peaks.
pub struct TruePeakDetector {
    sample_rate: f64,
    channels: usize,

    // Per-channel delay lines for resampling
    delay_lines: Vec<DelayLine>,

    // Per-channel peak tracking
    channel_peaks: Vec<f64>,
    channel_peak_indices: Vec<usize>,

    // Resampling filter coefficients (per phase)
    filter_coeffs: Vec<Vec<f64>>,

    // Sample counter
    sample_index: usize,
}

impl TruePeakDetector {
    /// Create a new true peak detector.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Sample rate in Hz
    /// * `channels` - Number of audio channels
    pub fn new(sample_rate: f64, channels: usize) -> Self {
        let filter_coeffs = Self::design_lanczos_filters();
        let delay_lines = (0..channels)
            .map(|_| DelayLine::new(TAPS_PER_PHASE))
            .collect();

        Self {
            sample_rate,
            channels,
            delay_lines,
            channel_peaks: vec![0.0; channels],
            channel_peak_indices: vec![0; channels],
            filter_coeffs,
            sample_index: 0,
        }
    }

    /// Process interleaved audio samples.
    ///
    /// # Arguments
    ///
    /// * `samples` - Interleaved audio samples
    pub fn process_interleaved(&mut self, samples: &[f64]) {
        let frames = samples.len() / self.channels;

        for frame in 0..frames {
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                if idx < samples.len() {
                    let sample = samples[idx];
                    self.process_sample(sample, ch);
                }
            }
            self.sample_index += 1;
        }
    }

    /// Process a single sample on a specific channel.
    fn process_sample(&mut self, sample: f64, channel: usize) {
        // Add sample to delay line
        self.delay_lines[channel].push(sample);

        // Check sample peak
        let abs_sample = sample.abs();
        if abs_sample > self.channel_peaks[channel] {
            self.channel_peaks[channel] = abs_sample;
            self.channel_peak_indices[channel] = self.sample_index;
        }

        // Oversample and check inter-sample peaks
        for phase in 1..OVERSAMPLE_FACTOR {
            let interpolated = self.interpolate(channel, phase);
            let abs_interpolated = interpolated.abs();

            if abs_interpolated > self.channel_peaks[channel] {
                self.channel_peaks[channel] = abs_interpolated;
                self.channel_peak_indices[channel] = self.sample_index;
            }
        }
    }

    /// Interpolate using Lanczos filter for given phase.
    fn interpolate(&self, channel: usize, phase: usize) -> f64 {
        let coeffs = &self.filter_coeffs[phase];
        let delay = &self.delay_lines[channel];

        let mut sum = 0.0;
        for (i, &coeff) in coeffs.iter().enumerate() {
            sum += delay.get(i) * coeff;
        }

        sum
    }

    /// Design Lanczos resampling filters for all phases.
    fn design_lanczos_filters() -> Vec<Vec<f64>> {
        let mut filters = Vec::with_capacity(OVERSAMPLE_FACTOR);

        // Phase 0 is the identity (no interpolation needed)
        filters.push(vec![]);

        // Design filters for phases 1, 2, 3
        for phase in 1..OVERSAMPLE_FACTOR {
            let mut coeffs = Vec::with_capacity(TAPS_PER_PHASE);
            let phase_offset = phase as f64 / OVERSAMPLE_FACTOR as f64;

            for i in 0..TAPS_PER_PHASE {
                let n = i as f64 - LANCZOS_A as f64 + phase_offset;
                let coeff = Self::lanczos_kernel(n, LANCZOS_A);
                coeffs.push(coeff);
            }

            // Normalize coefficients
            let sum: f64 = coeffs.iter().sum();
            if sum != 0.0 {
                for coeff in &mut coeffs {
                    *coeff /= sum;
                }
            }

            filters.push(coeffs);
        }

        filters
    }

    /// Lanczos kernel function.
    ///
    /// L(x) = sinc(x) * sinc(x/a) for -a <= x <= a
    fn lanczos_kernel(x: f64, a: usize) -> f64 {
        if x == 0.0 {
            return 1.0;
        }

        let a_f64 = a as f64;
        if x.abs() >= a_f64 {
            return 0.0;
        }

        let sinc_x = (PI * x).sin() / (PI * x);
        let sinc_xa = (PI * x / a_f64).sin() / (PI * x / a_f64);

        sinc_x * sinc_xa
    }

    /// Get true peak in dBTP (maximum across all channels).
    pub fn true_peak_dbtp(&self) -> f64 {
        let max_peak = self.channel_peaks.iter().copied().fold(0.0, f64::max);
        Self::linear_to_dbtp(max_peak)
    }

    /// Get true peak in linear scale.
    pub fn true_peak_linear(&self) -> f64 {
        self.channel_peaks.iter().copied().fold(0.0, f64::max)
    }

    /// Get per-channel peaks in dBTP.
    pub fn channel_peaks_dbtp(&self) -> Vec<f64> {
        self.channel_peaks
            .iter()
            .map(|&peak| Self::linear_to_dbtp(peak))
            .collect()
    }

    /// Get per-channel peaks in linear scale.
    pub fn channel_peaks_linear(&self) -> Vec<f64> {
        self.channel_peaks.clone()
    }

    /// Get true peak for specific channel.
    pub fn channel_peak(&self, channel: usize) -> Option<TruePeak> {
        if channel < self.channels {
            Some(TruePeak::new(
                self.channel_peaks[channel],
                channel,
                self.channel_peak_indices[channel],
            ))
        } else {
            None
        }
    }

    /// Convert linear to dBTP.
    pub fn linear_to_dbtp(linear: f64) -> f64 {
        if linear > 0.0 {
            20.0 * linear.log10()
        } else {
            f64::NEG_INFINITY
        }
    }

    /// Convert dBTP to linear.
    pub fn dbtp_to_linear(dbtp: f64) -> f64 {
        10.0_f64.powf(dbtp / 20.0)
    }

    /// Reset the detector.
    pub fn reset(&mut self) {
        for delay in &mut self.delay_lines {
            delay.reset();
        }
        self.channel_peaks.fill(0.0);
        self.channel_peak_indices.fill(0);
        self.sample_index = 0;
    }

    /// Get sample rate.
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }

    /// Get channel count.
    pub fn channels(&self) -> usize {
        self.channels
    }
}

/// Circular delay line for sample storage.
struct DelayLine {
    buffer: Vec<f64>,
    size: usize,
    write_pos: usize,
}

impl DelayLine {
    /// Create a new delay line.
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            size,
            write_pos: 0,
        }
    }

    /// Push a new sample into the delay line.
    fn push(&mut self, sample: f64) {
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.size;
    }

    /// Get a sample from the delay line (0 = oldest, size-1 = newest).
    fn get(&self, index: usize) -> f64 {
        if index >= self.size {
            return 0.0;
        }
        let read_pos = (self.write_pos + index) % self.size;
        self.buffer[read_pos]
    }

    /// Reset the delay line.
    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_true_peak_detector_creates() {
        let detector = TruePeakDetector::new(48000.0, 2);
        assert_eq!(detector.sample_rate(), 48000.0);
        assert_eq!(detector.channels(), 2);
    }

    #[test]
    fn test_linear_to_dbtp_conversion() {
        assert_eq!(TruePeakDetector::linear_to_dbtp(1.0), 0.0);
        assert_eq!(
            TruePeakDetector::linear_to_dbtp(0.5),
            20.0 * 0.5_f64.log10()
        );
    }

    #[test]
    fn test_dbtp_to_linear_conversion() {
        assert_eq!(TruePeakDetector::dbtp_to_linear(0.0), 1.0);
        let linear = TruePeakDetector::dbtp_to_linear(-6.0);
        assert!((linear - 0.501187).abs() < 0.001);
    }

    #[test]
    fn test_roundtrip_conversion() {
        let original = 0.7;
        let dbtp = TruePeakDetector::linear_to_dbtp(original);
        let recovered = TruePeakDetector::dbtp_to_linear(dbtp);
        assert!((original - recovered).abs() < 1e-10);
    }

    #[test]
    fn test_lanczos_kernel_at_zero() {
        let kernel = TruePeakDetector::lanczos_kernel(0.0, 3);
        assert_eq!(kernel, 1.0);
    }

    #[test]
    fn test_lanczos_kernel_outside_window() {
        let kernel = TruePeakDetector::lanczos_kernel(4.0, 3);
        assert_eq!(kernel, 0.0);
    }

    #[test]
    fn test_delay_line() {
        let mut delay = DelayLine::new(4);
        delay.push(1.0);
        delay.push(2.0);
        delay.push(3.0);
        delay.push(4.0);

        assert_eq!(delay.get(0), 1.0);
        assert_eq!(delay.get(1), 2.0);
        assert_eq!(delay.get(2), 3.0);
        assert_eq!(delay.get(3), 4.0);
    }

    #[test]
    fn test_true_peak_exceeds() {
        let peak = TruePeak::new(1.2, 0, 0);
        assert!(peak.exceeds(-1.0));
        assert!(!peak.exceeds(2.0));
    }
}
