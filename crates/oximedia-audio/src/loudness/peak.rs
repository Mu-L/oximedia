//! True peak detection for preventing inter-sample clipping.
//!
//! Implements true peak measurement using oversampling as specified
//! in ITU-R BS.1770-4 and EBU R128.

#![forbid(unsafe_code)]
#![allow(clippy::cast_lossless)]

use std::f64::consts::PI;

/// Oversampling factor for true peak detection.
///
/// ITU-R BS.1770-4 requires at least 4x oversampling.
const OVERSAMPLE_FACTOR: usize = 4;

/// True peak detector with oversampling.
///
/// Detects peaks that occur between samples (inter-sample peaks)
/// by upsampling the signal using a polyphase FIR filter.
#[derive(Clone, Debug)]
pub struct TruePeakDetector {
    /// Sample rate in Hz.
    sample_rate: f64,
    /// Number of audio channels.
    channels: usize,
    /// Per-channel peak detectors.
    detectors: Vec<ChannelPeakDetector>,
}

impl TruePeakDetector {
    /// Create a new true peak detector.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Sample rate in Hz
    /// * `channels` - Number of audio channels
    #[must_use]
    pub fn new(sample_rate: f64, channels: usize) -> Self {
        let detectors = (0..channels)
            .map(|_| ChannelPeakDetector::new(OVERSAMPLE_FACTOR))
            .collect();

        Self {
            sample_rate,
            channels,
            detectors,
        }
    }

    /// Process interleaved samples and detect true peaks.
    ///
    /// # Arguments
    ///
    /// * `samples` - Interleaved audio samples
    ///
    /// # Returns
    ///
    /// Maximum true peak value across all channels (linear scale, 0.0 to 1.0+)
    pub fn process_interleaved(&mut self, samples: &[f64]) -> f64 {
        if samples.is_empty() || self.channels == 0 {
            return 0.0;
        }

        let frames = samples.len() / self.channels;
        let mut max_peak: f64 = 0.0;

        for frame in 0..frames {
            for ch in 0..self.channels {
                let idx = frame * self.channels + ch;
                let sample = samples.get(idx).copied().unwrap_or(0.0);
                let peak = self.detectors[ch].process(sample);
                max_peak = max_peak.max(peak);
            }
        }

        max_peak
    }

    /// Process planar samples and detect true peaks.
    ///
    /// # Arguments
    ///
    /// * `channels` - Slice of per-channel sample buffers
    ///
    /// # Returns
    ///
    /// Maximum true peak value across all channels (linear scale, 0.0 to 1.0+)
    pub fn process_planar(&mut self, channels: &[&[f64]]) -> f64 {
        let mut max_peak: f64 = 0.0;

        for (ch_idx, samples) in channels.iter().enumerate() {
            if ch_idx < self.detectors.len() {
                for &sample in samples.iter() {
                    let peak = self.detectors[ch_idx].process(sample);
                    max_peak = max_peak.max(peak);
                }
            }
        }

        max_peak
    }

    /// Get the current peak for a specific channel.
    ///
    /// # Arguments
    ///
    /// * `channel` - Channel index
    ///
    /// # Returns
    ///
    /// Current peak value (linear scale, 0.0 to 1.0+)
    #[must_use]
    pub fn get_channel_peak(&self, channel: usize) -> f64 {
        self.detectors.get(channel).map_or(0.0, |d| d.peak)
    }

    /// Get peaks for all channels.
    ///
    /// # Returns
    ///
    /// Vector of peak values (linear scale, 0.0 to 1.0+)
    #[must_use]
    pub fn get_all_peaks(&self) -> Vec<f64> {
        self.detectors.iter().map(|d| d.peak).collect()
    }

    /// Reset all peak detectors.
    pub fn reset(&mut self) {
        for detector in &mut self.detectors {
            detector.reset();
        }
    }

    /// Convert linear peak to dBTP (dB True Peak).
    ///
    /// # Arguments
    ///
    /// * `linear` - Linear peak value
    ///
    /// # Returns
    ///
    /// Peak level in dBTP
    #[must_use]
    pub fn linear_to_dbtp(linear: f64) -> f64 {
        if linear <= 0.0 {
            f64::NEG_INFINITY
        } else {
            20.0 * linear.log10()
        }
    }

    /// Convert dBTP to linear peak.
    ///
    /// # Arguments
    ///
    /// * `dbtp` - Peak level in dBTP
    ///
    /// # Returns
    ///
    /// Linear peak value
    #[must_use]
    pub fn dbtp_to_linear(dbtp: f64) -> f64 {
        if dbtp.is_infinite() && dbtp.is_sign_negative() {
            0.0
        } else {
            10.0_f64.powf(dbtp / 20.0)
        }
    }

    /// Get the oversampling factor.
    #[must_use]
    pub fn oversample_factor() -> usize {
        OVERSAMPLE_FACTOR
    }
}

/// Per-channel true peak detector.
#[derive(Clone, Debug)]
struct ChannelPeakDetector {
    /// Oversampling factor.
    oversample: usize,
    /// Polyphase filter banks for oversampling.
    filters: Vec<OversampleFilter>,
    /// Current peak value.
    peak: f64,
}

impl ChannelPeakDetector {
    /// Create a new channel peak detector.
    fn new(oversample: usize) -> Self {
        let filters = (0..oversample)
            .map(|phase| OversampleFilter::new(phase, oversample))
            .collect();

        Self {
            oversample,
            filters,
            peak: 0.0,
        }
    }

    /// Process a single sample and update peak.
    fn process(&mut self, sample: f64) -> f64 {
        // Feed sample to all polyphase filters
        for filter in &mut self.filters {
            let upsampled = filter.process(sample);
            let abs_val = upsampled.abs();
            self.peak = self.peak.max(abs_val);
        }

        self.peak
    }

    /// Reset peak detector.
    fn reset(&mut self) {
        self.peak = 0.0;
        for filter in &mut self.filters {
            filter.reset();
        }
    }
}

/// Polyphase FIR filter for oversampling.
///
/// Uses a windowed-sinc interpolation filter to upsample the signal.
#[derive(Clone, Debug)]
struct OversampleFilter {
    /// Filter coefficients.
    coeffs: Vec<f64>,
    /// Delay line (sample history).
    delay_line: Vec<f64>,
    /// Write position in delay line.
    write_pos: usize,
}

impl OversampleFilter {
    /// Create a new oversampling filter for a specific phase.
    ///
    /// # Arguments
    ///
    /// * `phase` - Phase index (0 to oversample-1)
    /// * `oversample` - Oversampling factor
    fn new(phase: usize, oversample: usize) -> Self {
        // Design windowed-sinc filter
        // Filter length: 12 taps per polyphase branch (48 total for 4x)
        let taps_per_phase = 12;
        let coeffs = Self::design_filter(phase, oversample, taps_per_phase);
        let delay_line = vec![0.0; taps_per_phase];

        Self {
            coeffs,
            delay_line,
            write_pos: 0,
        }
    }

    /// Design windowed-sinc interpolation filter.
    fn design_filter(phase: usize, oversample: usize, length: usize) -> Vec<f64> {
        let mut coeffs = Vec::with_capacity(length);
        let center = (length - 1) as f64 / 2.0;

        for i in 0..length {
            let x = i as f64 - center + phase as f64 / oversample as f64;

            // Sinc function
            let sinc = if x.abs() < 1e-10 {
                1.0
            } else {
                let pi_x = PI * x;
                pi_x.sin() / pi_x
            };

            // Hamming window
            let window = 0.54 - 0.46 * (2.0 * PI * i as f64 / (length - 1) as f64).cos();

            coeffs.push(sinc * window);
        }

        // Normalize
        let sum: f64 = coeffs.iter().sum();
        if sum.abs() > 1e-10 {
            for coeff in &mut coeffs {
                *coeff /= sum;
            }
        }

        coeffs
    }

    /// Process a single sample through the filter.
    fn process(&mut self, sample: f64) -> f64 {
        // Add sample to delay line
        self.delay_line[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.delay_line.len();

        // Convolve with filter coefficients
        let mut output = 0.0;
        let mut read_pos = self.write_pos;

        for &coeff in &self.coeffs {
            output += coeff * self.delay_line[read_pos];
            read_pos = (read_pos + 1) % self.delay_line.len();
        }

        output
    }

    /// Reset filter state.
    fn reset(&mut self) {
        self.delay_line.fill(0.0);
        self.write_pos = 0;
    }
}

/// Simple peak detector (sample peak, not true peak).
///
/// Detects the maximum absolute sample value without oversampling.
/// This is faster but may miss inter-sample peaks.
#[derive(Clone, Debug, Default)]
pub struct SamplePeakDetector {
    /// Per-channel peak values.
    peaks: Vec<f64>,
}

impl SamplePeakDetector {
    /// Create a new sample peak detector.
    ///
    /// # Arguments
    ///
    /// * `channels` - Number of audio channels
    #[must_use]
    pub fn new(channels: usize) -> Self {
        Self {
            peaks: vec![0.0; channels],
        }
    }

    /// Process interleaved samples.
    ///
    /// # Arguments
    ///
    /// * `samples` - Interleaved audio samples
    /// * `channels` - Number of channels
    pub fn process_interleaved(&mut self, samples: &[f64], channels: usize) {
        if channels == 0 || channels != self.peaks.len() {
            return;
        }

        let frames = samples.len() / channels;

        for frame in 0..frames {
            for ch in 0..channels {
                let idx = frame * channels + ch;
                if let Some(&sample) = samples.get(idx) {
                    self.peaks[ch] = self.peaks[ch].max(sample.abs());
                }
            }
        }
    }

    /// Process planar samples.
    ///
    /// # Arguments
    ///
    /// * `channels` - Slice of per-channel sample buffers
    pub fn process_planar(&mut self, channels: &[&[f64]]) {
        for (ch_idx, samples) in channels.iter().enumerate() {
            if ch_idx < self.peaks.len() {
                for &sample in samples.iter() {
                    self.peaks[ch_idx] = self.peaks[ch_idx].max(sample.abs());
                }
            }
        }
    }

    /// Get peak for a specific channel.
    ///
    /// # Arguments
    ///
    /// * `channel` - Channel index
    ///
    /// # Returns
    ///
    /// Peak value (linear scale, 0.0 to 1.0+)
    #[must_use]
    pub fn get_peak(&self, channel: usize) -> f64 {
        self.peaks.get(channel).copied().unwrap_or(0.0)
    }

    /// Get maximum peak across all channels.
    ///
    /// # Returns
    ///
    /// Maximum peak value (linear scale, 0.0 to 1.0+)
    #[must_use]
    pub fn max_peak(&self) -> f64 {
        self.peaks.iter().copied().fold(0.0, f64::max)
    }

    /// Get all channel peaks.
    ///
    /// # Returns
    ///
    /// Vector of peak values (linear scale, 0.0 to 1.0+)
    #[must_use]
    pub fn get_all_peaks(&self) -> &[f64] {
        &self.peaks
    }

    /// Reset all peaks to zero.
    pub fn reset(&mut self) {
        self.peaks.fill(0.0);
    }
}
