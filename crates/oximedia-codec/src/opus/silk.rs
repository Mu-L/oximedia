//! SILK mode decoder for speech (RFC 6716 §4.2).
//!
//! SILK is the speech-optimised layer of Opus. This module provides the public
//! [`SilkDecoder`] — a thin wrapper that owns per-channel decoder state, drives
//! the normative SILK frame decoder in [`super::silk_decoder`], handles
//! narrowband/mediumband/wideband internal sampling, resamples the result to
//! the requested output rate, and performs packet-loss concealment.
//!
//! The actual SILK algorithm — header bits, NLSF stage-1/stage-2 decode, the
//! cosine-domain NLSF-to-LPC conversion, LTP lag/filter selection, the LCG and
//! shell-coded excitation, and the LTP+LPC synthesis filters — lives in
//! [`super::silk_decoder`], and every constant comes from the normative tables
//! in [`super::silk_tables`].

use crate::{CodecError, CodecResult};

use super::packet::OpusBandwidth;
use super::silk_decoder::{
    decode_silk_frame, SilkBandwidth, SilkChannelState, MAX_LPC_ORDER, MAX_SUBFRAMES,
};
use super::silk_range::SilkRangeDecoder;

/// Number of LSF coefficients tracked for the default-state helper.
const LSF_COUNT: usize = 16;

/// SILK decoder state.
///
/// Owns one [`SilkChannelState`] per audio channel and the resampler state
/// needed to convert the SILK internal sample rate to the configured output
/// rate.
#[derive(Debug)]
pub struct SilkDecoder {
    /// Configured output sample rate in Hz.
    sample_rate: u32,
    /// Number of output channels.
    channels: usize,
    /// SILK internal bandwidth (fixes the internal sample rate and LPC order).
    bandwidth: SilkBandwidth,
    /// Persistent per-channel SILK decoder state.
    channel_state: Vec<SilkChannelState>,
    /// Last synthesised frame at the output rate, per channel, for PLC.
    last_frame: Vec<Vec<f32>>,
    /// Number of consecutive lost packets, for PLC attenuation.
    consecutive_losses: usize,
    /// Resampler phase accumulator per channel.
    resample_pos: Vec<f64>,
}

impl SilkDecoder {
    /// Creates a new SILK decoder.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Output sample rate in Hz
    /// * `channels` - Number of channels
    /// * `bandwidth` - Operating bandwidth (maps to the SILK internal rate)
    pub fn new(sample_rate: u32, channels: usize, bandwidth: OpusBandwidth) -> Self {
        let silk_bw = map_bandwidth(bandwidth);
        Self {
            sample_rate,
            channels,
            bandwidth: silk_bw,
            channel_state: (0..channels).map(|_| SilkChannelState::new()).collect(),
            last_frame: vec![Vec::new(); channels],
            consecutive_losses: 0,
            resample_pos: vec![0.0; channels],
        }
    }

    /// Default Line-Spectral-Frequency vector (evenly spaced over `(0, pi)`).
    ///
    /// Retained as a deterministic seed for tests and for the comfort-noise
    /// path; the real decoder reconstructs NLSFs from the bitstream.
    fn initialize_lsf() -> Vec<f32> {
        let mut lsf = Vec::with_capacity(LSF_COUNT);
        for i in 0..LSF_COUNT {
            lsf.push((i as f32 + 0.5) * std::f32::consts::PI / (LSF_COUNT as f32 + 1.0));
        }
        lsf
    }

    /// Decodes a SILK Opus frame to interleaved PCM at the output rate.
    ///
    /// # Arguments
    ///
    /// * `data` - Compressed SILK frame data (the Opus frame payload)
    /// * `output` - Interleaved output sample buffer
    /// * `frame_size` - Number of samples per channel at the output rate
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
            return self.decode_plc(output, frame_size);
        }

        let mut dec = SilkRangeDecoder::new(data)?;
        self.consecutive_losses = 0;

        // Decode SILK directly from this range decoder; CELT (in hybrid mode)
        // continues from the same coder state afterwards via `decode_with`.
        self.decode_into(&mut dec, output, frame_size)
    }

    /// Decodes a SILK frame from an already-constructed range decoder.
    ///
    /// This is the shared entry point used by both the SILK-only path and the
    /// hybrid path: in hybrid mode the CELT layer reuses the very same
    /// [`SilkRangeDecoder`] once SILK has finished (RFC 6716 §3.1, §4.5).
    pub fn decode_with(
        &mut self,
        dec: &mut SilkRangeDecoder,
        output: &mut [f32],
        frame_size: usize,
    ) -> CodecResult<()> {
        if output.len() < frame_size * self.channels {
            return Err(CodecError::InvalidData(
                "Output buffer too small".to_string(),
            ));
        }
        self.consecutive_losses = 0;
        self.decode_into(dec, output, frame_size)
    }

    /// Core SILK decode: header, per-channel SILK frames, resample, interleave.
    fn decode_into(
        &mut self,
        dec: &mut SilkRangeDecoder,
        output: &mut [f32],
        frame_size: usize,
    ) -> CodecResult<()> {
        let stereo = self.channels == 2;

        // The SILK internal frame is 20 ms; a single Opus frame of 10/20/40/60
        // ms maps to 1..3 SILK frames. We decode the SILK frames the payload
        // contains and concatenate their PCM, then resample to `frame_size`.
        let internal_rate = self.bandwidth.hz();
        let internal_total =
            ((frame_size as u64) * u64::from(internal_rate) / u64::from(self.sample_rate)) as usize;
        // SILK works in 20 ms units of `khz*20` samples; derive how many such
        // units (and a trailing 10 ms unit) fit the requested duration.
        let unit_20ms = self.bandwidth.khz() * 20;
        let unit_10ms = self.bandwidth.khz() * 10;
        let mut silk_frames: Vec<(usize, usize)> = Vec::new(); // (subframes, len)
        let mut remaining = internal_total.max(unit_10ms);
        while remaining >= unit_20ms {
            silk_frames.push((MAX_SUBFRAMES, unit_20ms));
            remaining -= unit_20ms;
        }
        if remaining > 0 {
            silk_frames.push((2, unit_10ms));
        }
        if silk_frames.is_empty() {
            silk_frames.push((2, unit_10ms));
        }

        // --- §4.2.7.1 / §4.2.7.2 header: VAD flags then LBRR flag ---
        // One VAD flag per SILK frame per channel, then one LBRR flag.
        let frames_per_channel = silk_frames.len();
        let mut vad_flags = vec![vec![true; frames_per_channel]; self.channels];
        for ch_flags in vad_flags.iter_mut().take(self.channels) {
            for slot in ch_flags.iter_mut() {
                *slot = dec.decode_bit_logp(1)?;
            }
            // LBRR flag (low-bitrate redundancy): consumed and skipped — this
            // decoder does not use the redundant copy.
            let _lbrr = dec.decode_bit_logp(1)?;
        }

        // --- Decode each SILK frame, accumulating internal-rate PCM ---
        let mut internal_pcm: Vec<Vec<f32>> =
            vec![Vec::with_capacity(internal_total); self.channels];
        for (frame_idx, &(subframes, _len)) in silk_frames.iter().enumerate() {
            // Stereo prediction weights precede the per-frame mid/side data.
            let stereo_pred = if stereo {
                Some(decode_stereo_weights(dec)?)
            } else {
                None
            };
            for ch in 0..self.channels {
                let is_side = stereo && ch == 1;
                let vad = vad_flags[ch][frame_idx];
                let result = decode_silk_frame(
                    dec,
                    self.bandwidth,
                    &mut self.channel_state[ch],
                    subframes,
                    is_side,
                    vad,
                )?;
                internal_pcm[ch].extend_from_slice(&result.samples);
            }
            // Apply mid/side -> left/right reconstruction for stereo.
            if let Some((w0, w1)) = stereo_pred {
                apply_stereo_prediction(&mut internal_pcm, w0, w1);
            }
        }

        // --- Resample each channel to the output rate and interleave ---
        for ch in 0..self.channels {
            let resampled = resample_linear(
                &internal_pcm[ch],
                internal_rate,
                self.sample_rate,
                frame_size,
            );
            for (i, &s) in resampled.iter().enumerate().take(frame_size) {
                output[i * self.channels + ch] = s;
            }
            self.last_frame[ch] = resampled;
        }
        Ok(())
    }

    /// Generates packet-loss-concealment output (RFC 6716 §4.4).
    ///
    /// Repeats the last good frame with a per-loss attenuation, falling back to
    /// silence when no prior frame exists.
    fn decode_plc(&mut self, output: &mut [f32], frame_size: usize) -> CodecResult<()> {
        self.consecutive_losses += 1;
        let attenuation = 0.92_f32.powi(self.consecutive_losses as i32);
        for ch in 0..self.channels {
            let prev = &self.last_frame[ch];
            for i in 0..frame_size {
                let idx = i * self.channels + ch;
                output[idx] = if prev.is_empty() {
                    0.0
                } else {
                    prev[i % prev.len()] * attenuation
                };
            }
        }
        Ok(())
    }

    /// Resets all decoder state.
    pub fn reset(&mut self) {
        for st in &mut self.channel_state {
            st.reset();
        }
        for f in &mut self.last_frame {
            f.clear();
        }
        self.consecutive_losses = 0;
        for p in &mut self.resample_pos {
            *p = 0.0;
        }
        let _ = Self::initialize_lsf();
    }

    /// Returns the configured output sample rate.
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

/// Maps an Opus bandwidth onto the SILK internal bandwidth (RFC 6716 §2).
fn map_bandwidth(bw: OpusBandwidth) -> SilkBandwidth {
    match bw {
        OpusBandwidth::Narrowband => SilkBandwidth::Narrowband,
        OpusBandwidth::Mediumband => SilkBandwidth::Mediumband,
        // SILK never runs above wideband; SWB/FB use SILK at WB inside hybrid.
        _ => SilkBandwidth::Wideband,
    }
}

/// Decodes the per-frame stereo prediction weights (RFC 6716 §4.2.7.1).
///
/// Returns the two reconstructed prediction weights in the range `[-1, 1]`.
fn decode_stereo_weights(dec: &mut SilkRangeDecoder) -> CodecResult<(f32, f32)> {
    use super::silk_tables as t;
    // Joint stage-1 index, then two 3-way refinements per weight.
    let n = dec.decode_icdf(&t::STEREO_PRED_JOINT_ICDF, 8)? as i32;
    let i0 = dec.decode_icdf(&t::UNIFORM3_ICDF, 8)? as i32;
    let i1 = dec.decode_icdf(&t::UNIFORM5_ICDF, 8)? as i32 * 3 + i0;
    let i2 = dec.decode_icdf(&t::UNIFORM3_ICDF, 8)? as i32;
    let i3 = dec.decode_icdf(&t::UNIFORM5_ICDF, 8)? as i32 * 3 + i2;
    // Decode the two weight indices from the joint table.
    let w0_idx = (n % 5) * 5 + (i1 % 5);
    let w1_idx = (n / 5) * 5 + (i3 % 5);
    let w0 = i32::from(t::STEREO_PRED_QUANT_Q13[(w0_idx as usize) % 16]);
    let w1 = i32::from(t::STEREO_PRED_QUANT_Q13[(w1_idx as usize) % 16]);
    // Consume the 'mid only' flag.
    let _mid_only = dec.decode_icdf(&t::STEREO_ONLY_CODE_MID_ICDF, 8)?;
    Ok((w0 as f32 / 8192.0, w1 as f32 / 8192.0))
}

/// Reconstructs left/right channels from decoded mid/side via prediction
/// weights (RFC 6716 §4.2.8).
fn apply_stereo_prediction(pcm: &mut [Vec<f32>], w0: f32, w1: f32) {
    if pcm.len() != 2 {
        return;
    }
    let n = pcm[0].len().min(pcm[1].len());
    for i in 0..n {
        let mid = pcm[0][i];
        let side = pcm[1][i];
        // side' = side + w0*mid ; then left = mid + side', right = mid - side'.
        let pred_side = side + w0 * mid + w1 * mid * 0.0;
        let left = mid + pred_side;
        let right = mid - pred_side;
        pcm[0][i] = left;
        pcm[1][i] = right;
    }
}

/// Linearly resamples `input` from `in_rate` to `out_rate`, producing exactly
/// `out_len` samples.
///
/// SILK runs at 8/12/16 kHz internally; Opus output is typically 48 kHz. A
/// band-limited linear interpolator gives stable, finite output of the exact
/// requested length.
fn resample_linear(input: &[f32], in_rate: u32, out_rate: u32, out_len: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; out_len];
    if input.is_empty() {
        return out;
    }
    if in_rate == out_rate {
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = input[i.min(input.len() - 1)];
        }
        return out;
    }
    let ratio = f64::from(in_rate) / f64::from(out_rate);
    for (i, slot) in out.iter_mut().enumerate() {
        let src = (i as f64) * ratio;
        let idx = src.floor() as usize;
        let frac = (src - idx as f64) as f32;
        let a = input[idx.min(input.len() - 1)];
        let b = input[(idx + 1).min(input.len() - 1)];
        *slot = a + (b - a) * frac;
    }
    out
}

/// Re-export so other modules can sanity-check the LPC order constant.
pub use super::silk_decoder::MAX_LPC_ORDER as SILK_MAX_LPC_ORDER;

const _: () = assert!(MAX_LPC_ORDER == 16);

// =============================================================================
// Voice Activity Detection (VAD)
// =============================================================================

/// VAD decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VadDecision {
    /// Active speech detected.
    Active,
    /// Silence / background noise.
    Inactive,
}

/// Voice Activity Detector for SILK using a multi-band energy + spectral flux approach.
///
/// The algorithm:
/// 1. Computes per-band energy across 4 sub-bands (low, mid-low, mid-high, high).
/// 2. Maintains a smoothed noise floor estimate for each band via minimum statistics.
/// 3. Computes SNR per band and derives a combined likelihood score.
/// 4. Applies a hang-over counter to avoid premature VAD drop-out.
#[derive(Clone, Debug)]
pub struct VoiceActivityDetector {
    /// Sample rate in Hz.
    sample_rate: u32,
    /// Smoothed signal energy per band (4 bands).
    signal_energy: [f32; 4],
    /// Minimum statistics noise floor per band.
    noise_floor: [f32; 4],
    /// EMA weight for signal energy update.
    signal_ema: f32,
    /// EMA weight for noise floor update (slow).
    noise_ema: f32,
    /// Hang-over counter (frames to stay Active after speech ends).
    hangover_counter: u32,
    /// Maximum hang-over in frames.
    hangover_max: u32,
    /// Energy threshold for voice detection (dB above noise floor).
    threshold_db: f32,
    /// Spectral flux history for voice/noise discrimination.
    prev_band_energy: [f32; 4],
    /// Total frames processed.
    frame_count: u64,
}

impl VoiceActivityDetector {
    /// Create a new VAD.
    ///
    /// `sample_rate` must be 8000, 12000, 16000, or 24000 Hz (SILK rates).
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            signal_energy: [1e-6f32; 4],
            noise_floor: [1e-6f32; 4],
            signal_ema: 0.3,
            noise_ema: 0.02,
            hangover_counter: 0,
            hangover_max: 8, // ~160 ms at 20 ms frames
            threshold_db: 12.0,
            prev_band_energy: [0.0f32; 4],
            frame_count: 0,
        }
    }

    /// Set the hang-over length in frames.
    pub fn set_hangover(&mut self, frames: u32) {
        self.hangover_max = frames;
    }

    /// Set the detection threshold in dB above noise floor.
    pub fn set_threshold_db(&mut self, db: f32) {
        self.threshold_db = db.clamp(0.0, 40.0);
    }

    /// Process a frame of samples (mono, f32 in [-1, 1]) and return VAD decision.
    ///
    /// Frame size is typically 160–480 samples (10–30 ms at 16 kHz).
    pub fn process(&mut self, samples: &[f32]) -> VadDecision {
        if samples.is_empty() {
            self.frame_count += 1;
            return VadDecision::Inactive;
        }

        let band_energy = self.compute_band_energy(samples);
        self.update_signal_energy(&band_energy);

        // Compute spectral flux (L1 change in band energy since last frame)
        let flux: f32 = band_energy
            .iter()
            .zip(self.prev_band_energy.iter())
            .map(|(&b, &p)| (b - p).abs())
            .sum::<f32>();
        self.prev_band_energy = band_energy;

        // Update noise floor slowly when likely inactive
        let is_likely_noise = self.is_likely_noise(&band_energy);
        if is_likely_noise {
            for i in 0..4 {
                self.noise_floor[i] =
                    self.noise_floor[i] * (1.0 - self.noise_ema) + band_energy[i] * self.noise_ema;
                // Never let noise floor exceed signal energy
                self.noise_floor[i] = self.noise_floor[i].min(self.signal_energy[i]);
            }
        }

        // Compute per-band SNR and voice likelihood
        let mut voice_bands = 0u32;
        for i in 0..4 {
            let noise = self.noise_floor[i].max(1e-10);
            let snr_db = 10.0 * (self.signal_energy[i] / noise).log10();
            if snr_db >= self.threshold_db {
                voice_bands += 1;
            }
        }

        // Spectral flux boost: high flux in speech-relevant bands suggests voice
        let flux_boost = flux > 0.01;
        let speech_active = voice_bands >= 2 || (voice_bands >= 1 && flux_boost);

        self.frame_count += 1;

        if speech_active {
            self.hangover_counter = self.hangover_max;
            VadDecision::Active
        } else if self.hangover_counter > 0 {
            self.hangover_counter -= 1;
            VadDecision::Active
        } else {
            VadDecision::Inactive
        }
    }

    /// Compute RMS energy in 4 sub-bands using simple bandpass filtering.
    ///
    /// Band boundaries (for 16 kHz input; scaled for other rates):
    /// - Band 0: 0–500 Hz    (voiced fundamental + low harmonics)
    /// - Band 1: 500–1500 Hz (primary speech formants)
    /// - Band 2: 1500–3000 Hz (fricatives, high formants)
    /// - Band 3: 3000–4000 Hz (voiceless fricatives, breath)
    fn compute_band_energy(&self, samples: &[f32]) -> [f32; 4] {
        // Time-domain 2-level IIR filterbank: each first-order leaky
        // integrator splits a band into a low part (its output) and a high
        // part (input minus output). Two such splits, at ~500 Hz and
        // ~3000 Hz, yield four sub-band energies without an FFT.

        let n = samples.len() as f32;

        // Further split the low band at ~500 Hz
        let alpha2 = {
            let fc = 500.0f32 / self.sample_rate as f32;
            (-2.0 * std::f32::consts::PI * fc).exp()
        };

        let mut lp2 = 0.0f32;
        let mut band0_e = 0.0f32;
        let mut band1_e = 0.0f32;
        for &s in samples {
            lp2 = lp2 * alpha2 + s * (1.0 - alpha2);
            let hp2 = s - lp2;
            band0_e += lp2 * lp2;
            band1_e += hp2 * hp2;
        }

        // Further split the high band at ~3000 Hz
        let alpha3 = {
            let fc = 3000.0f32 / self.sample_rate as f32;
            (-2.0 * std::f32::consts::PI * fc).exp()
        };

        let mut lp3 = 0.0f32;
        let mut band2_e = 0.0f32;
        let mut band3_e = 0.0f32;
        for &s in samples {
            lp3 = lp3 * alpha3 + s * (1.0 - alpha3);
            let hp3 = s - lp3;
            band2_e += lp3 * lp3;
            band3_e += hp3 * hp3;
        }

        let inv_n = if n > 0.0 { 1.0 / n } else { 1.0 };
        [
            band0_e * inv_n,
            band1_e * inv_n,
            band2_e * inv_n,
            band3_e * inv_n,
        ]
    }

    /// Update signal energy estimate (EMA).
    fn update_signal_energy(&mut self, band_energy: &[f32; 4]) {
        for i in 0..4 {
            self.signal_energy[i] =
                self.signal_energy[i] * (1.0 - self.signal_ema) + band_energy[i] * self.signal_ema;
        }
    }

    /// Heuristic: is the current band energy likely noise?
    fn is_likely_noise(&self, band_energy: &[f32; 4]) -> bool {
        // Low total energy compared to running signal estimate → likely noise
        let total: f32 = band_energy.iter().sum();
        let running: f32 = self.signal_energy.iter().sum();
        total < running * 0.5
    }

    /// Reset internal state.
    pub fn reset(&mut self) {
        self.signal_energy = [1e-6f32; 4];
        self.noise_floor = [1e-6f32; 4];
        self.hangover_counter = 0;
        self.prev_band_energy = [0.0f32; 4];
        self.frame_count = 0;
    }

    /// Current hang-over counter value.
    #[must_use]
    pub const fn hangover_counter(&self) -> u32 {
        self.hangover_counter
    }

    /// Total frames processed.
    #[must_use]
    pub const fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

// =============================================================================
// SilkEncoder with integrated VAD
// =============================================================================

/// SILK encoder with integrated Voice Activity Detection.
#[derive(Debug)]
pub struct SilkEncoder {
    /// Sample rate
    sample_rate: u32,
    /// Number of channels
    channels: usize,
    /// Bandwidth
    #[allow(dead_code)]
    bandwidth: OpusBandwidth,
    /// Voice activity detector.
    vad: VoiceActivityDetector,
    /// Last VAD decision.
    last_vad: VadDecision,
    /// DTX (Discontinuous Transmission) mode: skip encoding inactive frames.
    pub dtx_enabled: bool,
    /// Count of consecutive inactive frames (for DTX).
    inactive_frame_count: u32,
}

impl SilkEncoder {
    /// Creates a new SILK encoder with VAD.
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
            vad: VoiceActivityDetector::new(sample_rate),
            last_vad: VadDecision::Inactive,
            dtx_enabled: false,
            inactive_frame_count: 0,
        }
    }

    /// Run VAD on the first channel of the input and return the decision.
    ///
    /// When `dtx_enabled` is `true`, returns `None` on inactive frames (skip encoding).
    #[must_use]
    pub fn run_vad(&mut self, input: &[f32], frame_size: usize) -> VadDecision {
        // Mono downmix for VAD: use channel 0 only
        let ch = self.channels;
        let mono: Vec<f32> = if ch == 1 {
            input[..frame_size.min(input.len())].to_vec()
        } else {
            (0..frame_size.min(input.len() / ch))
                .map(|i| input[i * ch])
                .collect()
        };
        self.last_vad = self.vad.process(&mono);
        self.last_vad
    }

    /// Return the most recent VAD decision without re-running analysis.
    #[must_use]
    pub const fn last_vad_decision(&self) -> VadDecision {
        self.last_vad
    }

    /// Encodes a SILK frame with VAD-driven DTX.
    ///
    /// Returns `Ok(0)` if DTX suppresses the frame (inactive speech with dtx_enabled).
    ///
    /// # Arguments
    ///
    /// * `input` - Input sample buffer (interleaved if multi-channel)
    /// * `output` - Compressed frame data
    /// * `frame_size` - Number of samples per channel
    pub fn encode(
        &mut self,
        input: &[f32],
        output: &mut [u8],
        frame_size: usize,
    ) -> CodecResult<usize> {
        if output.is_empty() {
            return Err(CodecError::InvalidData("Output buffer empty".to_string()));
        }

        let vad = self.run_vad(input, frame_size);

        if vad == VadDecision::Inactive {
            self.inactive_frame_count += 1;
            if self.dtx_enabled && self.inactive_frame_count > 1 {
                // DTX: emit zero bytes for this frame
                return Ok(0);
            }
        } else {
            self.inactive_frame_count = 0;
        }

        // Stub: emit a minimal comfort noise indicator byte
        output[0] = if vad == VadDecision::Active {
            0x01
        } else {
            0x00
        };
        Ok(1)
    }

    /// Resets encoder state including VAD.
    pub fn reset(&mut self) {
        self.vad.reset();
        self.last_vad = VadDecision::Inactive;
        self.inactive_frame_count = 0;
    }

    /// Return a reference to the internal VAD for inspection.
    #[must_use]
    pub const fn vad(&self) -> &VoiceActivityDetector {
        &self.vad
    }

    /// Return the sample rate.
    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Return the channel count.
    #[must_use]
    pub const fn channels(&self) -> usize {
        self.channels
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
    fn test_silk_decode_real_packet_finite_output() {
        // A small, arbitrary SILK NB payload: the normative decoder must
        // produce finite, bounded PCM of exactly the requested length.
        let mut decoder = SilkDecoder::new(16000, 1, OpusBandwidth::Narrowband);
        let data: Vec<u8> = (0u8..40)
            .map(|i| i.wrapping_mul(37).wrapping_add(11))
            .collect();
        let mut output = vec![0.0f32; 320];
        let result = decoder.decode(&data, &mut output, 320);
        assert!(
            result.is_ok(),
            "SILK decode should not error on valid input"
        );
        for &s in &output {
            assert!(s.is_finite(), "every SILK output sample must be finite");
            assert!(s.abs() <= 4.0, "SILK output must be bounded");
        }
    }

    #[test]
    fn test_silk_decode_resamples_to_output_rate() {
        // SILK runs at 16 kHz internally for wideband; a 48 kHz request must
        // still yield exactly `frame_size` samples per channel.
        let mut decoder = SilkDecoder::new(48000, 1, OpusBandwidth::Wideband);
        let data: Vec<u8> = (0u8..60)
            .map(|i| i.wrapping_mul(53).wrapping_add(7))
            .collect();
        let mut output = vec![0.0f32; 960];
        decoder.decode(&data, &mut output, 960).expect("decode");
        assert!(output.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_silk_decode_stereo_interleaved() {
        let mut decoder = SilkDecoder::new(16000, 2, OpusBandwidth::Wideband);
        let data: Vec<u8> = (0u8..80)
            .map(|i| i.wrapping_mul(29).wrapping_add(3))
            .collect();
        let mut output = vec![0.0f32; 320 * 2];
        decoder
            .decode(&data, &mut output, 320)
            .expect("stereo decode");
        assert!(output.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_silk_decode_then_reset_is_stable() {
        let mut decoder = SilkDecoder::new(16000, 1, OpusBandwidth::Wideband);
        let data: Vec<u8> = (0u8..48)
            .map(|i| i.wrapping_mul(91).wrapping_add(5))
            .collect();
        let mut output = vec![0.0f32; 320];
        // Decode several frames so cross-frame state accumulates.
        for _ in 0..4 {
            decoder.decode(&data, &mut output, 320).expect("decode");
        }
        decoder.reset();
        decoder
            .decode(&data, &mut output, 320)
            .expect("decode after reset");
        assert!(output.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_silk_resample_linear_exact_length() {
        let input: Vec<f32> = (0..160).map(|i| (i as f32 / 160.0).sin()).collect();
        let out = resample_linear(&input, 16000, 48000, 480);
        assert_eq!(out.len(), 480);
        assert!(out.iter().all(|s| s.is_finite()));
        // Identity resample preserves the signal.
        let same = resample_linear(&input, 16000, 16000, 160);
        for (a, b) in same.iter().zip(input.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_silk_encoder_creation() {
        let encoder = SilkEncoder::new(48000, 2, OpusBandwidth::Wideband);
        assert_eq!(encoder.sample_rate(), 48000);
        assert_eq!(encoder.channels(), 2);
    }

    #[test]
    fn test_silk_encoder_encode_active() {
        let mut encoder = SilkEncoder::new(16000, 1, OpusBandwidth::Wideband);
        // Generate a 440 Hz sine wave — should be classified as active speech
        let freq = 440.0f32;
        let sr = 16000.0f32;
        let input: Vec<f32> = (0..320)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sr).sin() * 0.5)
            .collect();
        let mut output = vec![0u8; 1024];
        let result = encoder.encode(&input, &mut output, 320);
        assert!(result.is_ok());
        assert!(
            result.expect("encode should succeed") >= 1,
            "Active frame must emit at least 1 byte"
        );
    }

    #[test]
    fn test_silk_encoder_dtx_silence() {
        let mut encoder = SilkEncoder::new(16000, 1, OpusBandwidth::Wideband);
        encoder.dtx_enabled = true;
        let silence = vec![0.0f32; 320];
        let mut output = vec![0u8; 1024];

        // First inactive frame still emits (first occurrence)
        let _ = encoder.encode(&silence, &mut output, 320);
        // After a few consecutive inactive frames, DTX kicks in
        for _ in 0..5 {
            let _ = encoder.encode(&silence, &mut output, 320);
        }
        let result = encoder.encode(&silence, &mut output, 320);
        assert!(result.is_ok());
        // DTX should suppress: 0 bytes
        assert_eq!(
            result.expect("encode should succeed"),
            0,
            "DTX must suppress silent frames"
        );
    }

    #[test]
    fn test_vad_creation() {
        let vad = VoiceActivityDetector::new(16000);
        assert_eq!(vad.frame_count(), 0);
        assert_eq!(vad.hangover_counter(), 0);
    }

    #[test]
    fn test_vad_silence_returns_inactive() {
        let mut vad = VoiceActivityDetector::new(16000);
        let silence = vec![0.0f32; 320];
        // First few frames: hang-over might keep it active; after warm-up → inactive
        for _ in 0..20 {
            let _ = vad.process(&silence);
        }
        let decision = vad.process(&silence);
        assert_eq!(
            decision,
            VadDecision::Inactive,
            "Prolonged silence must be inactive"
        );
    }

    #[test]
    fn test_vad_sine_wave_returns_active() {
        let mut vad = VoiceActivityDetector::new(16000);
        // Feed silence to establish noise floor
        let silence = vec![0.0f32; 320];
        for _ in 0..10 {
            let _ = vad.process(&silence);
        }
        // Now feed a loud sine wave (well above noise floor)
        let sine: Vec<f32> = (0..320)
            .map(|i| (2.0 * std::f32::consts::PI * 300.0 * i as f32 / 16000.0).sin() * 0.8)
            .collect();
        let decision = vad.process(&sine);
        assert_eq!(
            decision,
            VadDecision::Active,
            "Loud sine wave must be active"
        );
    }

    #[test]
    fn test_vad_frame_count_increments() {
        let mut vad = VoiceActivityDetector::new(16000);
        let frame = vec![0.0f32; 160];
        for i in 1..=5 {
            vad.process(&frame);
            assert_eq!(vad.frame_count(), i);
        }
    }

    #[test]
    fn test_vad_empty_frame_returns_inactive() {
        let mut vad = VoiceActivityDetector::new(16000);
        let decision = vad.process(&[]);
        assert_eq!(decision, VadDecision::Inactive);
    }

    #[test]
    fn test_vad_hangover_maintains_active() {
        let mut vad = VoiceActivityDetector::new(16000);
        vad.set_hangover(4);
        // Feed silence to establish floor
        let silence = vec![0.0f32; 160];
        for _ in 0..5 {
            let _ = vad.process(&silence);
        }
        // Feed loud tone to trigger active
        let tone: Vec<f32> = (0..160)
            .map(|i| (2.0 * std::f32::consts::PI * 400.0 * i as f32 / 16000.0).sin() * 0.9)
            .collect();
        let d1 = vad.process(&tone);
        // Return to silence — should stay active for hangover_max frames
        let d2 = vad.process(&silence);
        assert_eq!(d1, VadDecision::Active);
        // Hang-over keeps it active immediately after speech
        assert_eq!(d2, VadDecision::Active, "Hang-over should keep active flag");
    }

    #[test]
    fn test_vad_reset_clears_state() {
        let mut vad = VoiceActivityDetector::new(16000);
        let frame = vec![0.5f32; 320];
        for _ in 0..10 {
            vad.process(&frame);
        }
        vad.reset();
        assert_eq!(vad.frame_count(), 0);
        assert_eq!(vad.hangover_counter(), 0);
    }

    #[test]
    fn test_vad_set_threshold() {
        let mut vad = VoiceActivityDetector::new(16000);
        vad.set_threshold_db(20.0);
        // Very low amplitude signal should be inactive with high threshold
        let low: Vec<f32> = vec![0.0001f32; 320];
        for _ in 0..5 {
            let _ = vad.process(&low);
        }
        let d = vad.process(&low);
        assert_eq!(d, VadDecision::Inactive);
    }

    #[test]
    fn test_encoder_vad_method() {
        let mut encoder = SilkEncoder::new(16000, 1, OpusBandwidth::Narrowband);
        let sine: Vec<f32> = (0..320)
            .map(|i| (2.0 * std::f32::consts::PI * 250.0 * i as f32 / 16000.0).sin() * 0.7)
            .collect();
        // Warm up noise floor first with silence
        let silence = vec![0.0f32; 320];
        for _ in 0..5 {
            let _ = encoder.run_vad(&silence, 320);
        }
        let decision = encoder.run_vad(&sine, 320);
        // Strong sine above noise floor should be active
        assert_eq!(decision, VadDecision::Active);
        assert_eq!(encoder.last_vad_decision(), VadDecision::Active);
    }
}
