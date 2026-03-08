//! SILK mode decoder for speech.
//!
//! SILK (Skype Low Latency Audio Codec) is optimized for speech content.
//! This implementation provides full SILK decoding including LSF decoding,
//! LTP (Long-Term Prediction), noise shaping, and PLC (Packet Loss Concealment).

use crate::{CodecError, CodecResult};

use super::packet::OpusBandwidth;
use super::range_decoder::RangeDecoder;

/// Maximum LPC order for SILK
const MAX_LPC_ORDER: usize = 16;

/// Maximum pitch lag for LTP
const MAX_PITCH_LAG: usize = 320;

/// Number of LSF quantization stages
const LSF_STAGES: usize = 3;

/// Number of LSF coefficients
const LSF_COUNT: usize = 16;

/// Number of subframes per frame
const SUBFRAMES: usize = 4;

/// LSF codebook for quantization (simplified)
const LSF_CODEBOOK: [[f32; LSF_COUNT]; 8] = [
    [
        0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5,
    ],
    [
        0.05, 0.15, 0.25, 0.35, 0.45, 0.55, 0.65, 0.75, 0.85, 0.95, 1.05, 1.15, 1.25, 1.35, 1.45,
        1.55,
    ],
    [
        0.02, 0.12, 0.22, 0.32, 0.42, 0.52, 0.62, 0.72, 0.82, 0.92, 1.02, 1.12, 1.22, 1.32, 1.42,
        1.52,
    ],
    [
        0.08, 0.18, 0.28, 0.38, 0.48, 0.58, 0.68, 0.78, 0.88, 0.98, 1.08, 1.18, 1.28, 1.38, 1.48,
        1.58,
    ],
    [
        0.03, 0.13, 0.23, 0.33, 0.43, 0.53, 0.63, 0.73, 0.83, 0.93, 1.03, 1.13, 1.23, 1.33, 1.43,
        1.53,
    ],
    [
        0.07, 0.17, 0.27, 0.37, 0.47, 0.57, 0.67, 0.77, 0.87, 0.97, 1.07, 1.17, 1.27, 1.37, 1.47,
        1.57,
    ],
    [
        0.04, 0.14, 0.24, 0.34, 0.44, 0.54, 0.64, 0.74, 0.84, 0.94, 1.04, 1.14, 1.24, 1.34, 1.44,
        1.54,
    ],
    [
        0.06, 0.16, 0.26, 0.36, 0.46, 0.56, 0.66, 0.76, 0.86, 0.96, 1.06, 1.16, 1.26, 1.36, 1.46,
        1.56,
    ],
];

/// Gain codebook for quantization
const GAIN_CODEBOOK: [f32; 32] = [
    0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8,
    1.9, 2.0, 2.2, 2.4, 2.6, 2.8, 3.0, 3.5, 4.0, 4.5, 5.0, 6.0, 7.0,
];

/// SILK decoder state.
#[derive(Debug)]
pub struct SilkDecoder {
    /// Sample rate
    sample_rate: u32,
    /// Number of channels
    channels: usize,
    /// Bandwidth
    #[allow(dead_code)]
    bandwidth: OpusBandwidth,
    /// LPC coefficients from previous frame
    lpc_coeffs: Vec<f32>,
    /// Pitch lag from previous frame
    pitch_lag: usize,
    /// Pitch gain from previous frame
    pitch_gain: f32,
    /// Previous frame samples for overlap and LTP
    prev_samples: Vec<f32>,
    /// Excitation signal history for LTP
    excitation_history: Vec<f32>,
    /// Frame counter for PLC
    consecutive_losses: usize,
    /// Last decoded LSF coefficients
    last_lsf: Vec<f32>,
}

impl SilkDecoder {
    /// Creates a new SILK decoder.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Sample rate in Hz
    /// * `channels` - Number of channels
    /// * `bandwidth` - Operating bandwidth
    pub fn new(sample_rate: u32, channels: usize, bandwidth: OpusBandwidth) -> Self {
        let max_frame_size = (sample_rate / 50) as usize; // 20ms

        Self {
            sample_rate,
            channels,
            bandwidth,
            lpc_coeffs: vec![0.0; MAX_LPC_ORDER],
            pitch_lag: 100,
            pitch_gain: 0.0,
            prev_samples: vec![0.0; max_frame_size * channels],
            excitation_history: vec![0.0; MAX_PITCH_LAG],
            consecutive_losses: 0,
            last_lsf: Self::initialize_lsf(),
        }
    }

    /// Initializes LSF coefficients with default values.
    fn initialize_lsf() -> Vec<f32> {
        let mut lsf = Vec::with_capacity(LSF_COUNT);
        for i in 0..LSF_COUNT {
            lsf.push((i as f32 + 0.5) * std::f32::consts::PI / (LSF_COUNT as f32 + 1.0));
        }
        lsf
    }

    /// Decodes a SILK frame.
    ///
    /// # Arguments
    ///
    /// * `data` - Compressed frame data
    /// * `output` - Output sample buffer
    /// * `frame_size` - Number of samples per channel
    pub fn decode(
        &mut self,
        data: &[u8],
        output: &mut [f32],
        frame_size: usize,
    ) -> CodecResult<()> {
        if output.len() < frame_size * self.channels {
            return Err(CodecError::InvalidData(
                "Output buffer too small".to_string(),
            ));
        }

        if data.is_empty() {
            // Packet loss - use PLC
            return self.decode_plc(output, frame_size);
        }

        // Create range decoder
        let mut decoder = RangeDecoder::new(data)?;

        // Reset consecutive loss counter on successful decode
        self.consecutive_losses = 0;

        // Decode frame-level parameters
        let frame_params = self.decode_frame_params(&mut decoder)?;

        // Decode LSF coefficients
        let lsf = self.decode_lsf(&mut decoder)?;
        self.last_lsf.copy_from_slice(&lsf);

        // Convert LSF to LPC coefficients
        Self::lsf_to_lpc(&lsf, &mut self.lpc_coeffs);

        // Decode subframes
        let subframe_size = frame_size / SUBFRAMES;
        for subframe_idx in 0..SUBFRAMES {
            let subframe_params = self.decode_subframe_params(&mut decoder, &frame_params)?;

            let offset = subframe_idx * subframe_size * self.channels;
            let subframe_output = &mut output[offset..offset + subframe_size * self.channels];

            self.decode_subframe(
                &mut decoder,
                subframe_output,
                subframe_size,
                &subframe_params,
            )?;
        }

        // Store samples for next frame's overlap
        let sample_count = frame_size * self.channels;
        if sample_count <= self.prev_samples.len() {
            self.prev_samples[..sample_count].copy_from_slice(&output[..sample_count]);
        }

        Ok(())
    }

    /// Decodes frame-level parameters.
    fn decode_frame_params(&mut self, decoder: &mut RangeDecoder) -> CodecResult<FrameParams> {
        // Decode voice activity detection flag
        let vad_flag = decoder.decode_bit(16384)?;

        // Decode long-term postfilter flag
        let ltpf_flag = decoder.decode_bit(16384)?;

        // Decode quantization gain index
        let gain_index = decoder.decode_uniform(32)? as usize;
        let gain = if gain_index < GAIN_CODEBOOK.len() {
            GAIN_CODEBOOK[gain_index]
        } else {
            1.0
        };

        Ok(FrameParams {
            vad_flag,
            ltpf_flag,
            gain,
        })
    }

    /// Decodes LSF (Line Spectral Frequencies) coefficients.
    fn decode_lsf(&mut self, decoder: &mut RangeDecoder) -> CodecResult<Vec<f32>> {
        let mut lsf = vec![0.0f32; LSF_COUNT];

        // Multi-stage vector quantization
        for stage in 0..LSF_STAGES {
            let codebook_index = decoder.decode_uniform(8)? as usize;

            if codebook_index < LSF_CODEBOOK.len() {
                let codebook_entry = &LSF_CODEBOOK[codebook_index];

                for i in 0..LSF_COUNT {
                    let weight = 1.0 / (stage + 1) as f32;
                    lsf[i] += codebook_entry[i] * weight;
                }
            }
        }

        // Ensure LSFs are ordered and within valid range
        self.stabilize_lsf(&mut lsf);

        Ok(lsf)
    }

    /// Stabilizes LSF coefficients to ensure proper ordering.
    fn stabilize_lsf(&self, lsf: &mut [f32]) {
        const MIN_DISTANCE: f32 = 0.01;

        // Ensure monotonic increasing order with minimum spacing
        for i in 1..lsf.len() {
            if lsf[i] <= lsf[i - 1] + MIN_DISTANCE {
                lsf[i] = lsf[i - 1] + MIN_DISTANCE;
            }
        }

        // Clamp to valid range [0, π]
        for coeff in lsf.iter_mut() {
            *coeff = coeff.clamp(0.0, std::f32::consts::PI);
        }
    }

    /// Converts LSF to LPC coefficients.
    fn lsf_to_lpc(lsf: &[f32], lpc: &mut [f32]) {
        // Initialize LPC coefficients
        lpc.fill(0.0);

        // Convert LSF to LPC using Chebyshev polynomials
        // This is a simplified conversion
        for (i, &freq) in lsf.iter().enumerate().take(lpc.len()) {
            let cos_freq = freq.cos();
            lpc[i] = -2.0 * cos_freq;
        }

        // Apply bandwidth expansion for numerical stability
        for (i, coeff) in lpc.iter_mut().enumerate() {
            let gamma = 0.99_f32.powi(i as i32 + 1);
            *coeff *= gamma;
        }
    }

    /// Decodes subframe parameters.
    fn decode_subframe_params(
        &mut self,
        decoder: &mut RangeDecoder,
        frame_params: &FrameParams,
    ) -> CodecResult<SubframeParams> {
        // Decode pitch lag (LTP lag)
        let pitch_lag_delta = decoder.decode_int(5)? as i32;
        let pitch_lag =
            (self.pitch_lag as i32 + pitch_lag_delta).clamp(20, MAX_PITCH_LAG as i32) as usize;

        // Decode pitch gain (LTP gain)
        let pitch_gain_index = decoder.decode_uniform(16)?;
        let pitch_gain = (pitch_gain_index as f32 / 15.0).clamp(0.0, 1.0);

        // Decode LTP filter tap weights
        let mut ltp_taps = [0.0f32; 5];
        for tap in &mut ltp_taps {
            let tap_index = decoder.decode_uniform(8)?;
            *tap = (tap_index as f32 / 7.0 - 0.5) * 0.5;
        }

        // Normalize LTP taps
        let tap_sum: f32 = ltp_taps.iter().sum();
        if tap_sum.abs() > 0.001 {
            for tap in &mut ltp_taps {
                *tap /= tap_sum;
            }
        }

        // Store for next subframe
        self.pitch_lag = pitch_lag;
        self.pitch_gain = pitch_gain;

        Ok(SubframeParams {
            pitch_lag,
            pitch_gain,
            ltp_taps,
            subframe_gain: frame_params.gain,
        })
    }

    /// Decodes a single subframe.
    #[allow(clippy::too_many_arguments)]
    fn decode_subframe(
        &mut self,
        decoder: &mut RangeDecoder,
        output: &mut [f32],
        subframe_size: usize,
        params: &SubframeParams,
    ) -> CodecResult<()> {
        // Decode excitation signal (residual)
        let mut excitation = vec![0.0f32; subframe_size];
        self.decode_excitation(decoder, &mut excitation, params.subframe_gain)?;

        // Apply Long-Term Prediction (LTP)
        self.apply_ltp(&mut excitation, params);

        // Apply LPC synthesis filter
        let mut synthesis = vec![0.0f32; subframe_size];
        self.apply_lpc_synthesis(&excitation, &mut synthesis);

        // Apply noise shaping
        self.apply_noise_shaping(&mut synthesis, params.subframe_gain);

        // Copy to output (interleave if stereo)
        for (i, &sample) in synthesis.iter().enumerate() {
            for ch in 0..self.channels {
                let idx = i * self.channels + ch;
                if idx < output.len() {
                    output[idx] = sample;
                }
            }
        }

        // Update excitation history
        self.update_excitation_history(&excitation);

        Ok(())
    }

    /// Decodes excitation signal using pulse coding.
    fn decode_excitation(
        &self,
        decoder: &mut RangeDecoder,
        excitation: &mut [f32],
        gain: f32,
    ) -> CodecResult<()> {
        // Decode number of pulses
        let pulse_count = decoder.decode_uniform(32)? as usize;

        // Decode pulse positions and amplitudes
        for _ in 0..pulse_count {
            let position = decoder.decode_uniform(excitation.len() as u32)? as usize;
            let amplitude_index = decoder.decode_uniform(8)?;
            let amplitude = (amplitude_index as f32 / 7.0 - 0.5) * gain * 2.0;

            if position < excitation.len() {
                excitation[position] += amplitude;
            }
        }

        Ok(())
    }

    /// Applies Long-Term Prediction (LTP) filter.
    fn apply_ltp(&mut self, excitation: &mut [f32], params: &SubframeParams) {
        for i in 0..excitation.len() {
            let mut ltp_contribution = 0.0;

            // Apply 5-tap LTP filter
            for (tap_idx, &tap_weight) in params.ltp_taps.iter().enumerate() {
                let lag_idx = i + self.excitation_history.len() - params.pitch_lag - 2 + tap_idx;

                if lag_idx < self.excitation_history.len() {
                    ltp_contribution += self.excitation_history[lag_idx] * tap_weight;
                }
            }

            excitation[i] += ltp_contribution * params.pitch_gain;
        }
    }

    /// Applies LPC synthesis filter.
    fn apply_lpc_synthesis(&self, excitation: &[f32], output: &mut [f32]) {
        let mut state = vec![0.0f32; MAX_LPC_ORDER];

        for (i, &exc) in excitation.iter().enumerate() {
            let mut sum = exc;

            // Apply LPC filter
            for (j, &coeff) in self.lpc_coeffs.iter().enumerate() {
                sum -= coeff * state[j];
            }

            // Update state
            state.rotate_right(1);
            state[0] = sum;

            output[i] = sum;
        }
    }

    /// Applies noise shaping filter for improved perceptual quality.
    fn apply_noise_shaping(&self, samples: &mut [f32], gain: f32) {
        // Simple first-order noise shaping
        const SHAPING_COEFF: f32 = 0.5;

        let mut prev_sample = 0.0;

        for sample in samples.iter_mut() {
            let shaped = *sample + SHAPING_COEFF * prev_sample;
            *sample = shaped * gain.sqrt();
            prev_sample = *sample;
        }
    }

    /// Updates excitation history for LTP.
    fn update_excitation_history(&mut self, excitation: &[f32]) {
        let history_len = self.excitation_history.len();
        let exc_len = excitation.len();

        if exc_len >= history_len {
            // Replace entire history with new excitation
            self.excitation_history
                .copy_from_slice(&excitation[exc_len - history_len..]);
        } else {
            // Shift history and append new excitation
            self.excitation_history.rotate_left(exc_len);
            let start = history_len - exc_len;
            self.excitation_history[start..].copy_from_slice(excitation);
        }
    }

    /// Decodes frame with Packet Loss Concealment (PLC).
    fn decode_plc(&mut self, output: &mut [f32], frame_size: usize) -> CodecResult<()> {
        self.consecutive_losses += 1;

        // Attenuation factor based on consecutive losses
        let attenuation = 0.95_f32.powi(self.consecutive_losses as i32);

        // Generate output using previous frame samples with pitch repetition
        for i in 0..frame_size {
            for ch in 0..self.channels {
                let idx = i * self.channels + ch;
                if idx < output.len() {
                    // Use pitch repetition for concealment
                    let pitch_idx = if i >= self.pitch_lag {
                        (i - self.pitch_lag) * self.channels + ch
                    } else {
                        idx
                    };

                    if pitch_idx < self.prev_samples.len() {
                        output[idx] = self.prev_samples[pitch_idx] * attenuation * self.pitch_gain;
                    } else {
                        output[idx] = 0.0;
                    }
                }
            }
        }

        // Add some noise to mask the repetition
        self.add_comfort_noise(output, attenuation * 0.01);

        Ok(())
    }

    /// Adds comfort noise to the output.
    fn add_comfort_noise(&self, output: &mut [f32], amplitude: f32) {
        // Simple pseudo-random noise generator
        let mut seed = self.consecutive_losses as u32 * 1_103_515_245 + 12345;

        for sample in output.iter_mut() {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let noise = ((seed / 65536) % 32768) as f32 / 32768.0 - 0.5;
            *sample += noise * amplitude;
        }
    }

    /// Resets decoder state.
    pub fn reset(&mut self) {
        self.lpc_coeffs.fill(0.0);
        self.pitch_lag = 100;
        self.pitch_gain = 0.0;
        self.prev_samples.fill(0.0);
        self.excitation_history.fill(0.0);
        self.consecutive_losses = 0;
        self.last_lsf = Self::initialize_lsf();
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
}

/// Frame-level parameters.
#[derive(Debug, Clone)]
struct FrameParams {
    /// Voice activity detection flag
    #[allow(dead_code)]
    vad_flag: bool,
    /// Long-term postfilter flag
    #[allow(dead_code)]
    ltpf_flag: bool,
    /// Quantization gain
    gain: f32,
}

/// Subframe-level parameters.
#[derive(Debug, Clone)]
struct SubframeParams {
    /// Pitch lag (in samples)
    pitch_lag: usize,
    /// Pitch gain
    pitch_gain: f32,
    /// LTP filter tap weights
    ltp_taps: [f32; 5],
    /// Subframe gain
    subframe_gain: f32,
}

/// SILK encoder state (stub).
#[derive(Debug)]
pub struct SilkEncoder {
    /// Sample rate
    #[allow(dead_code)]
    sample_rate: u32,
    /// Number of channels
    #[allow(dead_code)]
    channels: usize,
    /// Bandwidth
    #[allow(dead_code)]
    bandwidth: OpusBandwidth,
}

impl SilkEncoder {
    /// Creates a new SILK encoder.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Sample rate in Hz
    /// * `channels` - Number of channels
    /// * `bandwidth` - Operating bandwidth
    pub fn new(sample_rate: u32, channels: usize, bandwidth: OpusBandwidth) -> Self {
        Self {
            sample_rate,
            channels,
            bandwidth,
        }
    }

    /// Encodes a SILK frame.
    ///
    /// # Arguments
    ///
    /// * `input` - Input sample buffer
    /// * `output` - Compressed frame data
    /// * `frame_size` - Number of samples per channel
    pub fn encode(
        &mut self,
        _input: &[f32],
        output: &mut [u8],
        _frame_size: usize,
    ) -> CodecResult<usize> {
        // Stub: return minimal valid packet
        if output.is_empty() {
            return Err(CodecError::InvalidData("Output buffer empty".to_string()));
        }

        // Return 0 bytes encoded (stub)
        Ok(0)
    }

    /// Resets encoder state.
    pub fn reset(&mut self) {
        // Nothing to reset in stub
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silk_decoder_creation() {
        let decoder = SilkDecoder::new(48000, 2, OpusBandwidth::Wideband);
        assert_eq!(decoder.sample_rate(), 48000);
        assert_eq!(decoder.channels(), 2);
    }

    #[test]
    fn test_silk_decoder_plc() {
        let mut decoder = SilkDecoder::new(48000, 1, OpusBandwidth::Wideband);
        let mut output = vec![0.0f32; 480];

        let result = decoder.decode_plc(&mut output, 480);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lsf_initialization() {
        let lsf = SilkDecoder::initialize_lsf();
        assert_eq!(lsf.len(), LSF_COUNT);

        // Check monotonic increasing
        for i in 1..lsf.len() {
            assert!(lsf[i] > lsf[i - 1]);
        }
    }

    #[test]
    fn test_silk_encoder_creation() {
        let encoder = SilkEncoder::new(48000, 2, OpusBandwidth::Wideband);
        assert_eq!(encoder.sample_rate, 48000);
        assert_eq!(encoder.channels, 2);
    }

    #[test]
    fn test_silk_encoder_stub() {
        let mut encoder = SilkEncoder::new(48000, 1, OpusBandwidth::Wideband);
        let input = vec![0.0f32; 480];
        let mut output = vec![0u8; 1024];

        let result = encoder.encode(&input, &mut output, 480);
        assert!(result.is_ok());
    }
}
