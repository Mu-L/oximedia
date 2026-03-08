//! K-weighting filters for ITU-R BS.1770-4 loudness measurement.
//!
//! This module implements the K-weighting filter chain consisting of:
//! - Stage 1: Pre-filter (high-pass) for head diffraction effects
//! - Stage 2: RLB (Revised Low-frequency B-weighting) high-shelf filter

#![allow(clippy::many_single_char_names)]
#![allow(clippy::similar_names)]

use std::f64::consts::PI;

/// K-weighting filter for single channel.
///
/// Implements ITU-R BS.1770-4 K-weighting using cascaded biquad filters.
#[derive(Clone, Debug)]
pub struct KWeightFilter {
    sample_rate: f64,
    pre_filter: BiquadFilter,
    rlb_filter: BiquadFilter,
}

impl KWeightFilter {
    /// Create a new K-weighting filter.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Sample rate in Hz (8000-192000 Hz)
    pub fn new(sample_rate: f64) -> Self {
        let pre_filter = Self::design_pre_filter(sample_rate);
        let rlb_filter = Self::design_rlb_filter(sample_rate);

        Self {
            sample_rate,
            pre_filter,
            rlb_filter,
        }
    }

    /// Process a single sample.
    ///
    /// # Arguments
    ///
    /// * `input` - Input sample
    ///
    /// # Returns
    ///
    /// K-weighted output sample
    pub fn process(&mut self, input: f64) -> f64 {
        let stage1 = self.pre_filter.process(input);
        self.rlb_filter.process(stage1)
    }

    /// Reset filter state to zero.
    pub fn reset(&mut self) {
        self.pre_filter.reset();
        self.rlb_filter.reset();
    }

    /// Design the pre-filter (Stage 1).
    ///
    /// High-pass filter at 78.5 Hz with Q = 0.707.
    /// Models head diffraction effects.
    fn design_pre_filter(sample_rate: f64) -> BiquadFilter {
        // ITU-R BS.1770-4 pre-filter parameters
        const F0: f64 = 1681.974450955533;
        const G: f64 = 3.999843853973347;
        const Q: f64 = 0.7071752369554196;

        let k = (PI * F0 / sample_rate).tan();
        let k_sq = k * k;
        let norm = 1.0 / (1.0 + k / Q + k_sq);

        let b0 = G * norm;
        let b1 = -2.0 * G * norm;
        let b2 = G * norm;
        let a1 = 2.0 * (k_sq - 1.0) * norm;
        let a2 = (1.0 - k / Q + k_sq) * norm;

        BiquadFilter::new(b0, b1, b2, a1, a2)
    }

    /// Design the RLB filter (Stage 2).
    ///
    /// High-shelf filter for revised low-frequency B-weighting.
    fn design_rlb_filter(sample_rate: f64) -> BiquadFilter {
        // ITU-R BS.1770-4 RLB filter parameters
        const F0: f64 = 38.13547087602444;
        const Q: f64 = 0.5003270373238773;
        const G: f64 = 1.0;

        let k = (PI * F0 / sample_rate).tan();
        let k_sq = k * k;

        let vh = 10.0_f64.powf(G / 20.0);
        let vb = vh.powf(0.4996667741545416);

        let norm = 1.0 / (1.0 + k / Q + k_sq);

        let b0 = (vh + vb * k / Q + k_sq) * norm;
        let b1 = 2.0 * (k_sq - vh) * norm;
        let b2 = (vh - vb * k / Q + k_sq) * norm;
        let a1 = 2.0 * (k_sq - 1.0) * norm;
        let a2 = (1.0 - k / Q + k_sq) * norm;

        BiquadFilter::new(b0, b1, b2, a1, a2)
    }

    /// Get sample rate.
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }
}

/// Second-order IIR biquad filter.
///
/// Implements the difference equation:
/// y[n] = b0*x[n] + b1*x[n-1] + b2*x[n-2] - a1*y[n-1] - a2*y[n-2]
#[derive(Clone, Debug)]
struct BiquadFilter {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl BiquadFilter {
    /// Create a new biquad filter with given coefficients.
    fn new(b0: f64, b1: f64, b2: f64, a1: f64, a2: f64) -> Self {
        Self {
            b0,
            b1,
            b2,
            a1,
            a2,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    /// Process a single sample through the filter.
    fn process(&mut self, input: f64) -> f64 {
        let output = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;

        // Update delay line
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }

    /// Reset filter state.
    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

/// Multi-channel K-weighting filter bank.
///
/// Maintains independent filter states for each audio channel.
#[derive(Clone, Debug)]
pub struct KWeightFilterBank {
    filters: Vec<KWeightFilter>,
    sample_rate: f64,
    channels: usize,
}

impl KWeightFilterBank {
    /// Create a new filter bank.
    ///
    /// # Arguments
    ///
    /// * `channels` - Number of audio channels
    /// * `sample_rate` - Sample rate in Hz
    pub fn new(channels: usize, sample_rate: f64) -> Self {
        let filters = (0..channels)
            .map(|_| KWeightFilter::new(sample_rate))
            .collect();

        Self {
            filters,
            sample_rate,
            channels,
        }
    }

    /// Process interleaved multi-channel audio.
    ///
    /// # Arguments
    ///
    /// * `input` - Interleaved input samples [L, R, L, R, ...]
    /// * `channels` - Number of channels
    /// * `output` - Output buffer for filtered samples
    ///
    /// # Returns
    ///
    /// Number of frames processed
    pub fn process_interleaved(
        &mut self,
        input: &[f64],
        channels: usize,
        output: &mut [f64],
    ) -> usize {
        if channels != self.channels || output.len() < input.len() {
            return 0;
        }

        let frames = input.len() / channels;

        for frame in 0..frames {
            for ch in 0..channels {
                let idx = frame * channels + ch;
                output[idx] = self.filters[ch].process(input[idx]);
            }
        }

        frames
    }

    /// Process planar multi-channel audio.
    ///
    /// # Arguments
    ///
    /// * `channels` - Mutable slice of per-channel sample buffers
    pub fn process_planar(&mut self, channels: &mut [Vec<f64>]) {
        for (ch_idx, samples) in channels.iter_mut().enumerate() {
            if ch_idx < self.filters.len() {
                for sample in samples {
                    *sample = self.filters[ch_idx].process(*sample);
                }
            }
        }
    }

    /// Reset all channel filters.
    pub fn reset(&mut self) {
        for filter in &mut self.filters {
            filter.reset();
        }
    }

    /// Get number of channels.
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// Get sample rate.
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_k_weight_filter_creates() {
        let filter = KWeightFilter::new(48000.0);
        assert_eq!(filter.sample_rate(), 48000.0);
    }

    #[test]
    fn test_k_weight_filter_processes() {
        let mut filter = KWeightFilter::new(48000.0);
        let output = filter.process(0.5);
        assert!(output.is_finite());
    }

    #[test]
    fn test_filter_bank_interleaved() {
        let mut bank = KWeightFilterBank::new(2, 48000.0);
        let input = vec![0.1, 0.2, 0.3, 0.4];
        let mut output = vec![0.0; 4];

        let frames = bank.process_interleaved(&input, 2, &mut output);
        assert_eq!(frames, 2);
        assert!(output.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_filter_reset() {
        let mut filter = KWeightFilter::new(48000.0);
        filter.process(0.5);
        filter.reset();

        // After reset, state should be zero
        let output = filter.process(0.0);
        assert_eq!(output, 0.0);
    }
}
