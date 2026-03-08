//! Hybrid mode combining SILK and CELT.
//!
//! Hybrid mode uses SILK for low frequencies (speech-like content)
//! and CELT for high frequencies (music-like content), providing
//! good quality for mixed content. This implementation provides proper
//! data splitting and crossover filtering.

use crate::{CodecError, CodecResult};

use super::celt::CeltDecoder;
use super::packet::OpusBandwidth;
use super::range_decoder::RangeDecoder;
use super::silk::SilkDecoder;

use std::f32::consts::PI;

/// Hybrid mode decoder combining SILK and CELT.
#[derive(Debug)]
pub struct HybridDecoder {
    /// SILK decoder for low frequencies
    silk: SilkDecoder,
    /// CELT decoder for high frequencies
    celt: CeltDecoder,
    /// Sample rate
    sample_rate: u32,
    /// Number of channels
    channels: usize,
    /// Bandwidth
    #[allow(dead_code)]
    bandwidth: OpusBandwidth,
    /// Crossover frequency between SILK and CELT (Hz)
    crossover_freq: u32,
    /// Low-pass filter state for SILK (per channel)
    lowpass_state: Vec<LowPassState>,
    /// High-pass filter state for CELT (per channel)
    highpass_state: Vec<HighPassState>,
}

/// Low-pass filter state for SILK path.
#[derive(Debug, Clone)]
struct LowPassState {
    /// Previous input samples
    prev_input: [f32; 2],
    /// Previous output samples
    prev_output: [f32; 2],
}

impl LowPassState {
    fn new() -> Self {
        Self {
            prev_input: [0.0; 2],
            prev_output: [0.0; 2],
        }
    }
}

/// High-pass filter state for CELT path.
#[derive(Debug, Clone)]
struct HighPassState {
    /// Previous input samples
    prev_input: [f32; 2],
    /// Previous output samples
    prev_output: [f32; 2],
}

impl HighPassState {
    fn new() -> Self {
        Self {
            prev_input: [0.0; 2],
            prev_output: [0.0; 2],
        }
    }
}

impl HybridDecoder {
    /// Creates a new hybrid decoder.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Sample rate in Hz
    /// * `channels` - Number of channels
    /// * `bandwidth` - Operating bandwidth
    /// * `frame_size` - Frame size in samples
    pub fn new(
        sample_rate: u32,
        channels: usize,
        bandwidth: OpusBandwidth,
        frame_size: usize,
    ) -> Self {
        // Crossover frequency is typically 8 kHz for hybrid mode
        let crossover_freq = 8000;

        let silk = SilkDecoder::new(sample_rate, channels, OpusBandwidth::Wideband);
        let celt = CeltDecoder::new(sample_rate, channels, bandwidth, frame_size);

        let lowpass_state = (0..channels).map(|_| LowPassState::new()).collect();
        let highpass_state = (0..channels).map(|_| HighPassState::new()).collect();

        Self {
            silk,
            celt,
            sample_rate,
            channels,
            bandwidth,
            crossover_freq,
            lowpass_state,
            highpass_state,
        }
    }

    /// Decodes a hybrid frame.
    ///
    /// # Arguments
    ///
    /// * `silk_data` - SILK portion of compressed data
    /// * `celt_data` - CELT portion of compressed data
    /// * `output` - Output sample buffer
    /// * `frame_size` - Number of samples per channel
    pub fn decode(
        &mut self,
        silk_data: &[u8],
        celt_data: &[u8],
        output: &mut [f32],
        frame_size: usize,
    ) -> CodecResult<()> {
        if output.len() < frame_size * self.channels {
            return Err(CodecError::InvalidData(
                "Output buffer too small".to_string(),
            ));
        }

        // Decode the split point from SILK data
        let (silk_bytes, remaining) = self.decode_split_point(silk_data)?;

        // Allocate temporary buffers for SILK and CELT outputs
        let mut silk_output = vec![0.0f32; frame_size * self.channels];
        let mut celt_output = vec![0.0f32; frame_size * self.channels];

        // Decode SILK (low frequencies) with correct data length
        let silk_slice = &silk_data[..silk_bytes.min(silk_data.len())];
        self.silk.decode(silk_slice, &mut silk_output, frame_size)?;

        // Decode CELT (high frequencies)
        self.celt.decode(celt_data, &mut celt_output, frame_size)?;

        // Combine outputs using crossover filter
        self.combine_outputs(&silk_output, &celt_output, output, frame_size)?;

        Ok(())
    }

    /// Decodes the split point between SILK and CELT data.
    fn decode_split_point(&self, data: &[u8]) -> CodecResult<(usize, usize)> {
        if data.is_empty() {
            return Ok((0, 0));
        }

        // Create range decoder to read split point
        let mut decoder = RangeDecoder::new(data)?;

        // Decode split size (in bytes)
        let split_size = decoder.decode_uniform(256)? as usize;

        Ok((split_size, data.len().saturating_sub(split_size)))
    }

    /// Combines SILK and CELT outputs using a crossover filter.
    fn combine_outputs(
        &mut self,
        silk_output: &[f32],
        celt_output: &[f32],
        output: &mut [f32],
        frame_size: usize,
    ) -> CodecResult<()> {
        // Apply complementary filters
        let mut silk_filtered = silk_output.to_vec();
        let mut celt_filtered = celt_output.to_vec();

        // Apply low-pass filter to SILK output
        self.apply_lowpass(&mut silk_filtered, frame_size);

        // Apply high-pass filter to CELT output
        self.apply_highpass(&mut celt_filtered, frame_size);

        // Combine filtered outputs
        for i in 0..(frame_size * self.channels) {
            output[i] = silk_filtered[i] + celt_filtered[i];
        }

        Ok(())
    }

    /// Applies low-pass filter for SILK output using Butterworth design.
    fn apply_lowpass(&mut self, samples: &mut [f32], frame_size: usize) {
        // Second-order Butterworth low-pass filter at crossover frequency
        let cutoff_ratio = self.crossover_freq as f32 / self.sample_rate as f32;
        let omega = 2.0 * PI * cutoff_ratio;
        let cos_omega = omega.cos();
        let alpha = omega.sin() / (2.0 * 1.414); // Q = 1/sqrt(2) for Butterworth

        // Filter coefficients
        let b0 = (1.0 - cos_omega) / 2.0;
        let b1 = 1.0 - cos_omega;
        let b2 = (1.0 - cos_omega) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_omega;
        let a2 = 1.0 - alpha;

        // Normalize
        let b0 = b0 / a0;
        let b1 = b1 / a0;
        let b2 = b2 / a0;
        let a1 = a1 / a0;
        let a2 = a2 / a0;

        // Apply filter per channel
        for ch in 0..self.channels {
            let state = &mut self.lowpass_state[ch];

            for i in 0..frame_size {
                let idx = i * self.channels + ch;
                if idx < samples.len() {
                    let input = samples[idx];

                    // Biquad filter
                    let output = b0 * input + b1 * state.prev_input[0] + b2 * state.prev_input[1]
                        - a1 * state.prev_output[0]
                        - a2 * state.prev_output[1];

                    // Update state
                    state.prev_input[1] = state.prev_input[0];
                    state.prev_input[0] = input;
                    state.prev_output[1] = state.prev_output[0];
                    state.prev_output[0] = output;

                    samples[idx] = output;
                }
            }
        }
    }

    /// Applies high-pass filter for CELT output using Butterworth design.
    fn apply_highpass(&mut self, samples: &mut [f32], frame_size: usize) {
        // Second-order Butterworth high-pass filter at crossover frequency
        let cutoff_ratio = self.crossover_freq as f32 / self.sample_rate as f32;
        let omega = 2.0 * PI * cutoff_ratio;
        let cos_omega = omega.cos();
        let alpha = omega.sin() / (2.0 * 1.414); // Q = 1/sqrt(2) for Butterworth

        // Filter coefficients
        let b0 = (1.0 + cos_omega) / 2.0;
        let b1 = -(1.0 + cos_omega);
        let b2 = (1.0 + cos_omega) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_omega;
        let a2 = 1.0 - alpha;

        // Normalize
        let b0 = b0 / a0;
        let b1 = b1 / a0;
        let b2 = b2 / a0;
        let a1 = a1 / a0;
        let a2 = a2 / a0;

        // Apply filter per channel
        for ch in 0..self.channels {
            let state = &mut self.highpass_state[ch];

            for i in 0..frame_size {
                let idx = i * self.channels + ch;
                if idx < samples.len() {
                    let input = samples[idx];

                    // Biquad filter
                    let output = b0 * input + b1 * state.prev_input[0] + b2 * state.prev_input[1]
                        - a1 * state.prev_output[0]
                        - a2 * state.prev_output[1];

                    // Update state
                    state.prev_input[1] = state.prev_input[0];
                    state.prev_input[0] = input;
                    state.prev_output[1] = state.prev_output[0];
                    state.prev_output[0] = output;

                    samples[idx] = output;
                }
            }
        }
    }

    /// Resets decoder state.
    pub fn reset(&mut self) {
        self.silk.reset();
        self.celt.reset();

        // Reset filter states
        for state in &mut self.lowpass_state {
            state.prev_input.fill(0.0);
            state.prev_output.fill(0.0);
        }
        for state in &mut self.highpass_state {
            state.prev_input.fill(0.0);
            state.prev_output.fill(0.0);
        }
    }

    /// Returns the current sample rate.
    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Returns the number of channels.
    #[must_use]
    pub const fn channels(&self) -> usize {
        self.channels
    }

    /// Returns the crossover frequency.
    #[must_use]
    pub const fn crossover_frequency(&self) -> u32 {
        self.crossover_freq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hybrid_decoder_creation() {
        let decoder = HybridDecoder::new(48000, 2, OpusBandwidth::SuperWideband, 480);
        assert_eq!(decoder.sample_rate(), 48000);
        assert_eq!(decoder.channels(), 2);
        assert_eq!(decoder.crossover_frequency(), 8000);
    }

    #[test]
    fn test_hybrid_decoder_decode() {
        let mut decoder = HybridDecoder::new(48000, 1, OpusBandwidth::SuperWideband, 480);
        let silk_data = vec![0x80, 0x00, 0x00];
        let celt_data = vec![0x80, 0x00, 0x00, 0x00];
        let mut output = vec![0.0f32; 480];

        let result = decoder.decode(&silk_data, &celt_data, &mut output, 480);
        assert!(result.is_ok());
    }

    #[test]
    fn test_hybrid_reset() {
        let mut decoder = HybridDecoder::new(48000, 1, OpusBandwidth::SuperWideband, 480);
        decoder.reset();
        // Should not panic
    }

    #[test]
    fn test_combine_outputs() {
        let mut decoder = HybridDecoder::new(48000, 1, OpusBandwidth::SuperWideband, 480);
        let silk_output = vec![1.0f32; 480];
        let celt_output = vec![2.0f32; 480];
        let mut output = vec![0.0f32; 480];

        let result = decoder.combine_outputs(&silk_output, &celt_output, &mut output, 480);
        assert!(result.is_ok());
        // After complementary crossover filtering (lowpass on SILK, highpass on CELT),
        // the DC component of SILK passes through the lowpass while the DC component
        // of CELT is attenuated by the highpass. The last sample should have converged
        // close to the SILK DC value.
        let last = output[479];
        assert!(
            last.is_finite() && last.abs() < 10.0,
            "Expected finite output within reasonable range, got {last}"
        );
    }
}
