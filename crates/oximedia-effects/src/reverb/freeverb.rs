//! Freeverb - classic algorithmic reverb implementation.
//!
//! Based on the Schroeder reverb architecture with parallel comb filters
//! and series all-pass filters. This is a faithful implementation of the
//! original Freeverb algorithm by Jezar at Dreampoint.

#![allow(clippy::cast_precision_loss)]

use crate::{AudioEffect, ReverbConfig};

/// Number of comb filters per channel.
const NUM_COMBS: usize = 8;
/// Number of all-pass filters per channel.
const NUM_ALLPASSES: usize = 4;

/// Comb filter delays for left channel (samples at 44.1kHz).
const COMB_DELAYS_L: [usize; NUM_COMBS] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
/// Comb filter delays for right channel (samples at 44.1kHz).
const COMB_DELAYS_R: [usize; NUM_COMBS] = [
    1116 + 23,
    1188 + 23,
    1277 + 23,
    1356 + 23,
    1422 + 23,
    1491 + 23,
    1557 + 23,
    1617 + 23,
];

/// All-pass filter delays for left channel.
const ALLPASS_DELAYS_L: [usize; NUM_ALLPASSES] = [556, 441, 341, 225];
/// All-pass filter delays for right channel.
const ALLPASS_DELAYS_R: [usize; NUM_ALLPASSES] = [556 + 23, 441 + 23, 341 + 23, 225 + 23];

/// Comb filter with feedback and damping.
#[derive(Debug, Clone)]
struct CombFilter {
    buffer: Vec<f32>,
    buffer_size: usize,
    buffer_idx: usize,
    filterstore: f32,
    feedback: f32,
    damp1: f32,
    damp2: f32,
}

impl CombFilter {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            buffer_size: size,
            buffer_idx: 0,
            filterstore: 0.0,
            feedback: 0.0,
            damp1: 0.0,
            damp2: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.buffer_idx];

        // Apply one-pole lowpass filter (damping)
        self.filterstore = output * self.damp2 + self.filterstore * self.damp1;

        // Store input + filtered feedback
        self.buffer[self.buffer_idx] = input + self.filterstore * self.feedback;

        // Advance buffer index
        self.buffer_idx += 1;
        if self.buffer_idx >= self.buffer_size {
            self.buffer_idx = 0;
        }

        output
    }

    fn set_feedback(&mut self, val: f32) {
        self.feedback = val;
    }

    fn set_damp(&mut self, val: f32) {
        self.damp1 = val;
        self.damp2 = 1.0 - val;
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.filterstore = 0.0;
        self.buffer_idx = 0;
    }
}

/// All-pass filter for reverb.
#[derive(Debug, Clone)]
struct AllPass {
    buffer: Vec<f32>,
    buffer_size: usize,
    buffer_idx: usize,
}

impl AllPass {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            buffer_size: size,
            buffer_idx: 0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let bufout = self.buffer[self.buffer_idx];
        let output = -input + bufout;
        self.buffer[self.buffer_idx] = input + bufout * 0.5;

        self.buffer_idx += 1;
        if self.buffer_idx >= self.buffer_size {
            self.buffer_idx = 0;
        }

        output
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.buffer_idx = 0;
    }
}

/// Freeverb reverb effect.
///
/// Classic algorithmic reverb based on the Schroeder reverb architecture.
/// Uses parallel comb filters followed by series all-pass filters.
pub struct Freeverb {
    // Left channel filters
    combs_l: Vec<CombFilter>,
    allpasses_l: Vec<AllPass>,

    // Right channel filters
    combs_r: Vec<CombFilter>,
    allpasses_r: Vec<AllPass>,

    // Parameters
    config: ReverbConfig,
    room_size: f32,
    damping: f32,
    wet1: f32,
    wet2: f32,
    dry: f32,

    // Pre-delay buffer
    predelay_buffer: Vec<f32>,
    predelay_write_pos: usize,
    predelay_samples: usize,

    sample_rate: f32,
}

impl Freeverb {
    /// Create a new Freeverb reverb.
    #[must_use]
    pub fn new(config: ReverbConfig, sample_rate: f32) -> Self {
        let scale_factor = sample_rate / 44100.0;

        // Create comb filters
        let combs_l: Vec<CombFilter> = COMB_DELAYS_L
            .iter()
            .map(|&delay| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let scaled_delay = (delay as f32 * scale_factor) as usize;
                CombFilter::new(scaled_delay.max(1))
            })
            .collect();

        let combs_r: Vec<CombFilter> = COMB_DELAYS_R
            .iter()
            .map(|&delay| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let scaled_delay = (delay as f32 * scale_factor) as usize;
                CombFilter::new(scaled_delay.max(1))
            })
            .collect();

        // Create all-pass filters
        let allpasses_l: Vec<AllPass> = ALLPASS_DELAYS_L
            .iter()
            .map(|&delay| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let scaled_delay = (delay as f32 * scale_factor) as usize;
                AllPass::new(scaled_delay.max(1))
            })
            .collect();

        let allpasses_r: Vec<AllPass> = ALLPASS_DELAYS_R
            .iter()
            .map(|&delay| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let scaled_delay = (delay as f32 * scale_factor) as usize;
                AllPass::new(scaled_delay.max(1))
            })
            .collect();

        // Pre-delay buffer
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let predelay_samples = ((config.predelay_ms * sample_rate) / 1000.0) as usize;
        let predelay_buffer = vec![0.0; predelay_samples.max(1)];

        let mut reverb = Self {
            combs_l,
            combs_r,
            allpasses_l,
            allpasses_r,
            config,
            room_size: 0.0,
            damping: 0.0,
            wet1: 0.0,
            wet2: 0.0,
            dry: 0.0,
            predelay_buffer,
            predelay_write_pos: 0,
            predelay_samples,
            sample_rate,
        };

        reverb.update_parameters();
        reverb
    }

    /// Update internal parameters from config.
    fn update_parameters(&mut self) {
        const ROOM_OFFSET: f32 = 0.7;
        const ROOM_SCALE: f32 = 0.28;
        const DAMP_SCALE: f32 = 0.4;

        self.room_size = self.config.room_size * ROOM_SCALE + ROOM_OFFSET;
        self.damping = self.config.damping * DAMP_SCALE;

        // Calculate wet/dry mix
        let wet = self.config.wet;
        self.dry = self.config.dry;

        // Stereo width
        let width = self.config.width;
        self.wet1 = wet * (width / 2.0 + 0.5);
        self.wet2 = wet * ((1.0 - width) / 2.0);

        // Update all comb filters
        for comb in &mut self.combs_l {
            comb.set_feedback(self.room_size);
            comb.set_damp(self.damping);
        }

        for comb in &mut self.combs_r {
            comb.set_feedback(self.room_size);
            comb.set_damp(self.damping);
        }
    }

    /// Set room size (0.0 - 1.0).
    pub fn set_room_size(&mut self, room_size: f32) {
        self.config.room_size = room_size.clamp(0.0, 1.0);
        self.update_parameters();
    }

    /// Set damping (0.0 - 1.0).
    pub fn set_damping(&mut self, damping: f32) {
        self.config.damping = damping.clamp(0.0, 1.0);
        self.update_parameters();
    }

    /// Set wet level (0.0 - 1.0).
    pub fn set_wet(&mut self, wet: f32) {
        self.config.wet = wet.clamp(0.0, 1.0);
        self.update_parameters();
    }

    /// Set dry level (0.0 - 1.0).
    pub fn set_dry(&mut self, dry: f32) {
        self.config.dry = dry.clamp(0.0, 1.0);
        self.update_parameters();
    }

    /// Set stereo width (0.0 - 1.0).
    pub fn set_width(&mut self, width: f32) {
        self.config.width = width.clamp(0.0, 1.0);
        self.update_parameters();
    }

    /// Process a stereo sample pair.
    fn process_sample_internal(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        // Apply pre-delay
        let (delayed_l, delayed_r) = if self.predelay_samples > 0 {
            let delayed = self.predelay_buffer[self.predelay_write_pos];
            self.predelay_buffer[self.predelay_write_pos] = (input_l + input_r) * 0.5;
            self.predelay_write_pos = (self.predelay_write_pos + 1) % self.predelay_samples;
            (delayed, delayed)
        } else {
            (input_l, input_r)
        };

        // Process through comb filters (parallel)
        let mut out_l = 0.0;
        let mut out_r = 0.0;

        for comb in &mut self.combs_l {
            out_l += comb.process(delayed_l);
        }

        for comb in &mut self.combs_r {
            out_r += comb.process(delayed_r);
        }

        // Process through all-pass filters (series)
        for allpass in &mut self.allpasses_l {
            out_l = allpass.process(out_l);
        }

        for allpass in &mut self.allpasses_r {
            out_r = allpass.process(out_r);
        }

        // Mix wet and dry signals
        let wet_l = out_l * self.wet1 + out_r * self.wet2;
        let wet_r = out_r * self.wet1 + out_l * self.wet2;

        let output_l = wet_l + input_l * self.dry;
        let output_r = wet_r + input_r * self.dry;

        (output_l, output_r)
    }
}

impl AudioEffect for Freeverb {
    fn process_sample(&mut self, input: f32) -> f32 {
        let (left, _right) = self.process_sample_internal(input, input);
        left
    }

    fn process_sample_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.process_sample_internal(left, right)
    }

    fn reset(&mut self) {
        for comb in &mut self.combs_l {
            comb.clear();
        }
        for comb in &mut self.combs_r {
            comb.clear();
        }
        for ap in &mut self.allpasses_l {
            ap.clear();
        }
        for ap in &mut self.allpasses_r {
            ap.clear();
        }
        self.predelay_buffer.fill(0.0);
        self.predelay_write_pos = 0;
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        // Recreate the reverb with new sample rate
        *self = Self::new(self.config.clone(), sample_rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_freeverb_creation() {
        let config = ReverbConfig::default();
        let reverb = Freeverb::new(config, 48000.0);
        assert_eq!(reverb.combs_l.len(), NUM_COMBS);
        assert_eq!(reverb.allpasses_l.len(), NUM_ALLPASSES);
    }

    #[test]
    fn test_freeverb_process() {
        let config = ReverbConfig::default();
        let mut reverb = Freeverb::new(config, 48000.0);

        // Process impulse
        let output = reverb.process_sample(1.0);
        assert!(output.is_finite());

        // Process more samples - just verify no crashes
        for _ in 0..1000 {
            let out = reverb.process_sample(0.0);
            assert!(out.is_finite());
        }
    }

    #[test]
    fn test_freeverb_stereo() {
        let config = ReverbConfig::default().with_width(1.0);
        let mut reverb = Freeverb::new(config, 48000.0);

        let (out_l, out_r) = reverb.process_sample_stereo(1.0, 0.0);

        // With stereo width, left and right should be different
        assert!(out_l != out_r);
    }

    #[test]
    fn test_freeverb_parameters() {
        let config = ReverbConfig::default();
        let mut reverb = Freeverb::new(config, 48000.0);

        reverb.set_room_size(0.9);
        reverb.set_damping(0.3);
        reverb.set_wet(0.5);
        reverb.set_dry(0.5);

        assert_eq!(reverb.config.room_size, 0.9);
        assert_eq!(reverb.config.damping, 0.3);
    }

    #[test]
    fn test_freeverb_reset() {
        let config = ReverbConfig::default();
        let mut reverb = Freeverb::new(config, 48000.0);

        // Generate reverb tail
        reverb.process_sample(1.0);
        for _ in 0..100 {
            reverb.process_sample(0.0);
        }

        // Reset
        reverb.reset();

        // After reset, output should be much quieter
        let output = reverb.process_sample(0.0);
        assert!(output.abs() < 0.001);
    }
}
