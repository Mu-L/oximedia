//! K-weighted filters for loudness measurement.
//!
//! Implements the K-weighting filter chain specified in ITU-R BS.1770-4
//! for perceptually accurate loudness measurement.

#![forbid(unsafe_code)]
#![allow(clippy::excessive_precision)]
#![allow(clippy::cast_lossless)]

use std::f64::consts::PI;

/// K-weighted filter cascade for loudness measurement.
///
/// The K-weighting filter consists of two stages:
/// 1. Pre-filter (high-pass) to account for head diffraction effects
/// 2. RLB (Revised Low-frequency B-weighting) high-shelf filter
///
/// This implements the ITU-R BS.1770-4 specification.
#[derive(Clone, Debug)]
pub struct KWeightFilter {
    /// Sample rate in Hz.
    sample_rate: f64,
    /// Pre-filter (stage 1) biquad coefficients and state.
    pre_filter: BiquadFilter,
    /// RLB filter (stage 2) biquad coefficients and state.
    rlb_filter: BiquadFilter,
}

impl KWeightFilter {
    /// Create a new K-weighting filter for the given sample rate.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Sample rate in Hz
    #[must_use]
    pub fn new(sample_rate: f64) -> Self {
        let pre_filter = Self::calculate_pre_filter(sample_rate);
        let rlb_filter = Self::calculate_rlb_filter(sample_rate);

        Self {
            sample_rate,
            pre_filter,
            rlb_filter,
        }
    }

    /// Process a single sample through the K-weighting filter.
    ///
    /// # Arguments
    ///
    /// * `input` - Input sample
    ///
    /// # Returns
    ///
    /// K-weighted output sample
    pub fn process(&mut self, input: f64) -> f64 {
        // Apply pre-filter (stage 1)
        let stage1 = self.pre_filter.process(input);
        // Apply RLB filter (stage 2)
        self.rlb_filter.process(stage1)
    }

    /// Process multiple samples in place.
    ///
    /// # Arguments
    ///
    /// * `samples` - Mutable slice of samples to process
    pub fn process_samples(&mut self, samples: &mut [f64]) {
        for sample in samples {
            *sample = self.process(*sample);
        }
    }

    /// Reset filter state to zero.
    pub fn reset(&mut self) {
        self.pre_filter.reset();
        self.rlb_filter.reset();
    }

    /// Calculate pre-filter coefficients (Stage 1 high-pass filter).
    ///
    /// This is a high-pass filter at ~78 Hz to account for head diffraction effects.
    fn calculate_pre_filter(sample_rate: f64) -> BiquadFilter {
        // Pre-filter parameters from ITU-R BS.1770-4
        let f0 = 1681.974450955533;
        let g = 3.999843853973347;
        let q = 0.7071752369554196;

        let k = (PI * f0 / sample_rate).tan();
        let k_squared = k * k;

        let norm = 1.0 / (1.0 + k / q + k_squared);

        let b0 = norm;
        let b1 = -2.0 * norm;
        let b2 = norm;
        let a1 = 2.0 * (k_squared - 1.0) * norm;
        let a2 = (1.0 - k / q + k_squared) * norm;

        BiquadFilter::new(b0 * g, b1 * g, b2 * g, a1, a2)
    }

    /// Calculate RLB filter coefficients (Stage 2 high-shelf filter).
    ///
    /// This is a high-shelf filter that provides the revised low-frequency B-weighting.
    fn calculate_rlb_filter(sample_rate: f64) -> BiquadFilter {
        // RLB filter parameters from ITU-R BS.1770-4
        let f0 = 38.13547087602444;
        let q = 0.5003270373238773;
        let g = 1.0;

        let k = (PI * f0 / sample_rate).tan();
        let k_squared = k * k;

        let vh = 10.0_f64.powf(g / 20.0);
        let vb = vh.powf(0.4996667741545416);

        let norm = 1.0 / (1.0 + k / q + k_squared);

        let b0 = (vh + vb * k / q + k_squared) * norm;
        let b1 = 2.0 * (k_squared - vh) * norm;
        let b2 = (vh - vb * k / q + k_squared) * norm;
        let a1 = 2.0 * (k_squared - 1.0) * norm;
        let a2 = (1.0 - k / q + k_squared) * norm;

        BiquadFilter::new(b0, b1, b2, a1, a2)
    }

    /// Get the sample rate this filter was configured for.
    #[must_use]
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }
}

/// Second-order IIR (biquad) filter.
///
/// Implements the difference equation:
/// `y[n] = b0*x[n] + b1*x[n-1] + b2*x[n-2] - a1*y[n-1] - a2*y[n-2]`
#[derive(Clone, Debug)]
struct BiquadFilter {
    /// Feed-forward coefficient 0.
    b0: f64,
    /// Feed-forward coefficient 1.
    b1: f64,
    /// Feed-forward coefficient 2.
    b2: f64,
    /// Feed-back coefficient 1.
    a1: f64,
    /// Feed-back coefficient 2.
    a2: f64,
    /// Previous input sample (x[n-1]).
    x1: f64,
    /// Previous previous input sample (x[n-2]).
    x2: f64,
    /// Previous output sample (y[n-1]).
    y1: f64,
    /// Previous previous output sample (y[n-2]).
    y2: f64,
}

impl BiquadFilter {
    /// Create a new biquad filter with the given coefficients.
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

        // Shift delay line
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }

    /// Reset filter state to zero.
    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

/// Multi-channel K-weighted filter bank.
///
/// Maintains separate filter states for each audio channel.
#[derive(Clone, Debug)]
pub struct KWeightFilterBank {
    /// Per-channel filters.
    filters: Vec<KWeightFilter>,
    /// Sample rate in Hz.
    sample_rate: f64,
}

impl KWeightFilterBank {
    /// Create a new K-weight filter bank for the given number of channels.
    ///
    /// # Arguments
    ///
    /// * `channels` - Number of audio channels
    /// * `sample_rate` - Sample rate in Hz
    #[must_use]
    pub fn new(channels: usize, sample_rate: f64) -> Self {
        let filters = (0..channels)
            .map(|_| KWeightFilter::new(sample_rate))
            .collect();

        Self {
            filters,
            sample_rate,
        }
    }

    /// Process multi-channel audio samples.
    ///
    /// # Arguments
    ///
    /// * `samples` - Interleaved samples [L, R, L, R, ...]
    /// * `channels` - Number of channels
    /// * `output` - Output buffer for filtered samples
    ///
    /// # Returns
    ///
    /// Number of samples processed per channel
    pub fn process_interleaved(
        &mut self,
        samples: &[f64],
        channels: usize,
        output: &mut [f64],
    ) -> usize {
        if channels == 0 || channels != self.filters.len() {
            return 0;
        }

        let frames = samples.len() / channels;
        if output.len() < samples.len() {
            return 0;
        }

        for frame in 0..frames {
            for ch in 0..channels {
                let idx = frame * channels + ch;
                output[idx] = self.filters[ch].process(samples[idx]);
            }
        }

        frames
    }

    /// Process planar multi-channel audio samples.
    ///
    /// # Arguments
    ///
    /// * `channels` - Slice of per-channel sample buffers
    pub fn process_planar(&mut self, channels: &mut [Vec<f64>]) {
        for (ch, samples) in channels.iter_mut().enumerate() {
            if ch < self.filters.len() {
                self.filters[ch].process_samples(samples);
            }
        }
    }

    /// Reset all channel filters.
    pub fn reset(&mut self) {
        for filter in &mut self.filters {
            filter.reset();
        }
    }

    /// Get the number of channels.
    #[must_use]
    pub fn channels(&self) -> usize {
        self.filters.len()
    }

    /// Get the sample rate.
    #[must_use]
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }
}
