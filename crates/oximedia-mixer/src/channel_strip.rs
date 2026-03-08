#![allow(dead_code)]
//! Channel strip processing chain for the mixer.
//!
//! A channel strip represents the complete signal processing chain for a single
//! mixer channel, including input gain, high-pass filter, EQ, dynamics, insert
//! effects, fader, pan, and output routing. This module models the typical
//! analog console channel strip in a digital workflow.

use serde::{Deserialize, Serialize};

/// Processing stage within a channel strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StripStage {
    /// Input gain/trim stage.
    InputGain,
    /// High-pass filter stage.
    HighPass,
    /// Equalizer stage.
    Equalizer,
    /// Dynamics (compressor/gate) stage.
    Dynamics,
    /// Insert effect slot.
    Insert,
    /// Channel fader stage.
    Fader,
    /// Pan control stage.
    Pan,
    /// Output/routing stage.
    Output,
}

/// High-pass filter configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighPassFilter {
    /// Whether the filter is enabled.
    pub enabled: bool,
    /// Cutoff frequency in Hz.
    pub frequency: f32,
    /// Filter slope in dB/octave (typically 6, 12, 18, or 24).
    pub slope_db_per_octave: u32,
    /// Internal state.
    prev_output: f32,
    /// Previous input sample for filtering.
    prev_input: f32,
}

impl Default for HighPassFilter {
    fn default() -> Self {
        Self {
            enabled: false,
            frequency: 80.0,
            slope_db_per_octave: 12,
            prev_output: 0.0,
            prev_input: 0.0,
        }
    }
}

impl HighPassFilter {
    /// Creates a new high-pass filter with the given cutoff frequency.
    #[must_use]
    pub fn new(frequency: f32) -> Self {
        Self {
            enabled: true,
            frequency: frequency.clamp(20.0, 20000.0),
            ..Default::default()
        }
    }

    /// Processes a single sample through the high-pass filter.
    #[allow(clippy::cast_precision_loss)]
    pub fn process(&mut self, input: f32, sample_rate: u32) -> f32 {
        if !self.enabled {
            return input;
        }
        let rc = 1.0 / (2.0 * std::f32::consts::PI * self.frequency);
        let dt = 1.0 / sample_rate as f32;
        let alpha = rc / (rc + dt);
        let output = alpha * (self.prev_output + input - self.prev_input);
        self.prev_input = input;
        self.prev_output = output;
        output
    }

    /// Resets the filter state.
    pub fn reset(&mut self) {
        self.prev_output = 0.0;
        self.prev_input = 0.0;
    }
}

/// Parametric EQ band for the channel strip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripEqBand {
    /// Whether this band is enabled.
    pub enabled: bool,
    /// Center frequency in Hz.
    pub frequency: f32,
    /// Gain in dB (-18.0 to +18.0).
    pub gain_db: f32,
    /// Q factor (bandwidth).
    pub q: f32,
}

impl Default for StripEqBand {
    fn default() -> Self {
        Self {
            enabled: true,
            frequency: 1000.0,
            gain_db: 0.0,
            q: 1.0,
        }
    }
}

/// EQ section with four parametric bands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripEq {
    /// Whether the entire EQ section is enabled.
    pub enabled: bool,
    /// Low band (typically shelf or bell, 20-500 Hz).
    pub low: StripEqBand,
    /// Low-mid band (200-2000 Hz).
    pub low_mid: StripEqBand,
    /// High-mid band (1000-8000 Hz).
    pub high_mid: StripEqBand,
    /// High band (2000-20000 Hz).
    pub high: StripEqBand,
}

impl Default for StripEq {
    fn default() -> Self {
        Self {
            enabled: true,
            low: StripEqBand {
                frequency: 100.0,
                ..Default::default()
            },
            low_mid: StripEqBand {
                frequency: 500.0,
                ..Default::default()
            },
            high_mid: StripEqBand {
                frequency: 3000.0,
                ..Default::default()
            },
            high: StripEqBand {
                frequency: 10000.0,
                ..Default::default()
            },
        }
    }
}

impl StripEq {
    /// Applies the EQ to a sample (simplified single-sample processing).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn process(&self, input: f32) -> f32 {
        if !self.enabled {
            return input;
        }
        let mut output = input;
        for band in &[&self.low, &self.low_mid, &self.high_mid, &self.high] {
            if band.enabled && band.gain_db.abs() > 0.01 {
                let gain_linear = 10.0_f32.powf(band.gain_db / 20.0);
                output *= gain_linear;
            }
        }
        output
    }
}

/// Dynamics section (simplified compressor + gate).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripDynamics {
    /// Whether the dynamics section is enabled.
    pub enabled: bool,
    /// Compressor threshold in dB.
    pub comp_threshold_db: f32,
    /// Compressor ratio (e.g., 4.0 = 4:1).
    pub comp_ratio: f32,
    /// Compressor attack time in milliseconds.
    pub comp_attack_ms: f32,
    /// Compressor release time in milliseconds.
    pub comp_release_ms: f32,
    /// Gate threshold in dB.
    pub gate_threshold_db: f32,
    /// Whether the gate is enabled.
    pub gate_enabled: bool,
    /// Internal envelope follower state.
    envelope: f32,
}

impl Default for StripDynamics {
    fn default() -> Self {
        Self {
            enabled: false,
            comp_threshold_db: -20.0,
            comp_ratio: 4.0,
            comp_attack_ms: 10.0,
            comp_release_ms: 100.0,
            gate_threshold_db: -60.0,
            gate_enabled: false,
            envelope: 0.0,
        }
    }
}

impl StripDynamics {
    /// Processes a sample through the dynamics chain.
    #[allow(clippy::cast_precision_loss)]
    pub fn process(&mut self, input: f32, sample_rate: u32) -> f32 {
        if !self.enabled {
            return input;
        }
        let level_db = if input.abs() > 1e-10 {
            20.0 * input.abs().log10()
        } else {
            -100.0
        };

        // Gate
        if self.gate_enabled && level_db < self.gate_threshold_db {
            return 0.0;
        }

        // Compressor
        if level_db > self.comp_threshold_db {
            let over = level_db - self.comp_threshold_db;
            let compressed_over = over / self.comp_ratio;
            let target_db = self.comp_threshold_db + compressed_over;
            let gain_db = target_db - level_db;

            // Smooth envelope
            let attack_coeff = (-1.0 / (self.comp_attack_ms * 0.001 * sample_rate as f32)).exp();
            let release_coeff = (-1.0 / (self.comp_release_ms * 0.001 * sample_rate as f32)).exp();
            let coeff = if gain_db < self.envelope {
                attack_coeff
            } else {
                release_coeff
            };
            self.envelope = coeff * self.envelope + (1.0 - coeff) * gain_db;

            let gain_linear = 10.0_f32.powf(self.envelope / 20.0);
            return input * gain_linear;
        }

        input
    }

    /// Resets the dynamics state.
    pub fn reset(&mut self) {
        self.envelope = 0.0;
    }
}

/// Complete channel strip configuration and processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelStrip {
    /// Channel strip name.
    pub name: String,
    /// Input gain/trim in dB.
    pub input_gain_db: f32,
    /// Phase invert.
    pub phase_invert: bool,
    /// High-pass filter.
    pub high_pass: HighPassFilter,
    /// Parametric EQ.
    pub eq: StripEq,
    /// Dynamics (compressor + gate).
    pub dynamics: StripDynamics,
    /// Fader level (0.0 = -inf, 1.0 = unity).
    pub fader: f32,
    /// Pan position (-1.0 = left, 0.0 = center, 1.0 = right).
    pub pan: f32,
    /// Mute state.
    pub mute: bool,
    /// Solo state.
    pub solo: bool,
    /// Sample rate.
    sample_rate: u32,
    /// Processing stage order (for reordering).
    stage_order: Vec<StripStage>,
}

impl ChannelStrip {
    /// Creates a new channel strip with the given name and sample rate.
    #[must_use]
    pub fn new(name: &str, sample_rate: u32) -> Self {
        Self {
            name: name.to_string(),
            input_gain_db: 0.0,
            phase_invert: false,
            high_pass: HighPassFilter::default(),
            eq: StripEq::default(),
            dynamics: StripDynamics::default(),
            fader: 1.0,
            pan: 0.0,
            mute: false,
            solo: false,
            sample_rate,
            stage_order: vec![
                StripStage::InputGain,
                StripStage::HighPass,
                StripStage::Equalizer,
                StripStage::Dynamics,
                StripStage::Fader,
                StripStage::Pan,
            ],
        }
    }

    /// Processes a mono sample through the entire channel strip.
    pub fn process_sample(&mut self, input: f32) -> f32 {
        if self.mute {
            return 0.0;
        }

        let mut sample = input;

        // Input gain
        let gain_linear = 10.0_f32.powf(self.input_gain_db / 20.0);
        sample *= gain_linear;

        // Phase invert
        if self.phase_invert {
            sample = -sample;
        }

        // High-pass filter
        sample = self.high_pass.process(sample, self.sample_rate);

        // EQ
        sample = self.eq.process(sample);

        // Dynamics
        sample = self.dynamics.process(sample, self.sample_rate);

        // Fader
        sample *= self.fader;

        sample
    }

    /// Processes a mono sample and returns stereo (left, right) after panning.
    pub fn process_sample_stereo(&mut self, input: f32) -> (f32, f32) {
        let mono = self.process_sample(input);
        let pan_norm = (self.pan + 1.0) * 0.5; // 0..1
        let left = mono * (1.0 - pan_norm).sqrt();
        let right = mono * pan_norm.sqrt();
        (left, right)
    }

    /// Processes a block of mono samples in-place.
    pub fn process_block(&mut self, samples: &mut [f32]) {
        for s in samples.iter_mut() {
            *s = self.process_sample(*s);
        }
    }

    /// Sets the input gain in dB.
    pub fn set_input_gain_db(&mut self, gain_db: f32) {
        self.input_gain_db = gain_db.clamp(-60.0, 24.0);
    }

    /// Gets the input gain in dB.
    #[must_use]
    pub fn input_gain_db(&self) -> f32 {
        self.input_gain_db
    }

    /// Sets the fader level (0.0 = -inf, 1.0 = unity, >1.0 = boost).
    pub fn set_fader(&mut self, level: f32) {
        self.fader = level.clamp(0.0, 2.0);
    }

    /// Gets the fader level.
    #[must_use]
    pub fn fader(&self) -> f32 {
        self.fader
    }

    /// Sets the pan position (-1.0 left, 0.0 center, 1.0 right).
    pub fn set_pan(&mut self, pan: f32) {
        self.pan = pan.clamp(-1.0, 1.0);
    }

    /// Gets the pan position.
    #[must_use]
    pub fn pan(&self) -> f32 {
        self.pan
    }

    /// Toggles mute state, returns new state.
    pub fn toggle_mute(&mut self) -> bool {
        self.mute = !self.mute;
        self.mute
    }

    /// Toggles solo state, returns new state.
    pub fn toggle_solo(&mut self) -> bool {
        self.solo = !self.solo;
        self.solo
    }

    /// Resets all processing state (filters, dynamics envelope, etc.).
    pub fn reset(&mut self) {
        self.high_pass.reset();
        self.dynamics.reset();
    }

    /// Gets the current stage order.
    #[must_use]
    pub fn stage_order(&self) -> &[StripStage] {
        &self.stage_order
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_strip_creation() {
        let strip = ChannelStrip::new("Vocal", 48000);
        assert_eq!(strip.name, "Vocal");
        assert!((strip.fader - 1.0).abs() < f32::EPSILON);
        assert!(strip.pan.abs() < f32::EPSILON);
        assert!(!strip.mute);
        assert!(!strip.solo);
    }

    #[test]
    fn test_channel_strip_mute() {
        let mut strip = ChannelStrip::new("Test", 48000);
        let out = strip.process_sample(1.0);
        assert!(out.abs() > 0.0);
        strip.toggle_mute();
        assert!(strip.mute);
        let out = strip.process_sample(1.0);
        assert!(out.abs() < f32::EPSILON);
    }

    #[test]
    fn test_channel_strip_phase_invert() {
        let mut strip = ChannelStrip::new("Test", 48000);
        strip.phase_invert = true;
        let out = strip.process_sample(1.0);
        assert!(out < 0.0);
    }

    #[test]
    fn test_channel_strip_pan_center() {
        let mut strip = ChannelStrip::new("Test", 48000);
        strip.set_pan(0.0);
        let (left, right) = strip.process_sample_stereo(1.0);
        assert!((left - right).abs() < 0.01);
    }

    #[test]
    fn test_channel_strip_pan_left() {
        let mut strip = ChannelStrip::new("Test", 48000);
        strip.set_pan(-1.0);
        let (left, right) = strip.process_sample_stereo(1.0);
        assert!(left > right);
        assert!(right.abs() < 0.01);
    }

    #[test]
    fn test_channel_strip_pan_right() {
        let mut strip = ChannelStrip::new("Test", 48000);
        strip.set_pan(1.0);
        let (left, right) = strip.process_sample_stereo(1.0);
        assert!(right > left);
        assert!(left.abs() < 0.01);
    }

    #[test]
    fn test_channel_strip_input_gain() {
        let mut strip = ChannelStrip::new("Test", 48000);
        strip.set_input_gain_db(6.0);
        assert!((strip.input_gain_db() - 6.0).abs() < f32::EPSILON);
        let out = strip.process_sample(0.5);
        // +6 dB ~ 2x gain, so output should be roughly 1.0
        assert!(out > 0.9 && out < 1.1);
    }

    #[test]
    fn test_channel_strip_fader() {
        let mut strip = ChannelStrip::new("Test", 48000);
        strip.set_fader(0.5);
        assert!((strip.fader() - 0.5).abs() < f32::EPSILON);
        let out = strip.process_sample(1.0);
        assert!((out - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_channel_strip_fader_clamping() {
        let mut strip = ChannelStrip::new("Test", 48000);
        strip.set_fader(5.0);
        assert!((strip.fader() - 2.0).abs() < f32::EPSILON);
        strip.set_fader(-1.0);
        assert!(strip.fader().abs() < f32::EPSILON);
    }

    #[test]
    fn test_high_pass_filter() {
        let mut hpf = HighPassFilter::new(1000.0);
        assert!(hpf.enabled);
        assert!((hpf.frequency - 1000.0).abs() < f32::EPSILON);
        // Process some samples
        for _ in 0..100 {
            hpf.process(0.5, 48000);
        }
        hpf.reset();
    }

    #[test]
    fn test_strip_eq_bypass() {
        let mut eq = StripEq::default();
        eq.enabled = false;
        let result = eq.process(0.5);
        assert!((result - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_strip_dynamics_bypass() {
        let mut dyn_proc = StripDynamics::default();
        dyn_proc.enabled = false;
        let result = dyn_proc.process(0.5, 48000);
        assert!((result - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_channel_strip_process_block() {
        let mut strip = ChannelStrip::new("Test", 48000);
        strip.set_fader(0.5);
        let mut block = vec![1.0_f32; 64];
        strip.process_block(&mut block);
        for s in &block {
            assert!((*s - 0.5).abs() < 0.01);
        }
    }

    #[test]
    fn test_channel_strip_reset() {
        let mut strip = ChannelStrip::new("Test", 48000);
        strip.high_pass = HighPassFilter::new(100.0);
        strip.dynamics.enabled = true;
        for _ in 0..100 {
            strip.process_sample(0.5);
        }
        strip.reset();
        // After reset, internal states should be zeroed
    }

    #[test]
    fn test_channel_strip_toggle() {
        let mut strip = ChannelStrip::new("Test", 48000);
        assert!(!strip.mute);
        assert!(strip.toggle_mute());
        assert!(strip.mute);
        assert!(!strip.toggle_mute());

        assert!(!strip.solo);
        assert!(strip.toggle_solo());
        assert!(strip.solo);
    }

    #[test]
    fn test_stage_order() {
        let strip = ChannelStrip::new("Test", 48000);
        let order = strip.stage_order();
        assert!(!order.is_empty());
        assert_eq!(order[0], StripStage::InputGain);
    }
}
