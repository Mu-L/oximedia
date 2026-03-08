#![allow(dead_code)]
//! Ring modulation audio effect.
//!
//! Ring modulation multiplies the input signal by a carrier oscillator,
//! producing sum and difference frequencies. This creates metallic, robotic,
//! or bell-like timbres commonly used in sound design, electronic music,
//! and sci-fi audio production.

use std::f32::consts::PI;

/// Carrier oscillator waveform for the ring modulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CarrierWaveform {
    /// Pure sine wave carrier.
    #[default]
    Sine,
    /// Square wave carrier.
    Square,
    /// Triangle wave carrier.
    Triangle,
    /// Sawtooth wave carrier.
    Sawtooth,
}

/// Configuration for the ring modulator effect.
#[derive(Debug, Clone)]
pub struct RingModConfig {
    /// Carrier frequency in Hz.
    pub frequency_hz: f32,
    /// Mix between dry and wet signal (0.0 = dry, 1.0 = full ring mod).
    pub mix: f32,
    /// Carrier waveform shape.
    pub waveform: CarrierWaveform,
    /// Carrier amplitude (0.0 - 1.0).
    pub carrier_level: f32,
    /// Sample rate in Hz.
    pub sample_rate: f32,
    /// Fine-tune frequency offset in Hz.
    pub detune_hz: f32,
}

impl Default for RingModConfig {
    fn default() -> Self {
        Self {
            frequency_hz: 440.0,
            mix: 1.0,
            waveform: CarrierWaveform::Sine,
            carrier_level: 1.0,
            sample_rate: 48000.0,
            detune_hz: 0.0,
        }
    }
}

/// Ring modulator audio effect processor.
#[derive(Debug)]
pub struct RingModulator {
    /// Current configuration.
    config: RingModConfig,
    /// Current carrier phase (0.0 - 1.0).
    phase: f32,
    /// Phase increment per sample.
    phase_inc: f32,
}

impl RingModulator {
    /// Create a new ring modulator with the given configuration.
    #[must_use]
    pub fn new(config: RingModConfig) -> Self {
        let freq = config.frequency_hz + config.detune_hz;
        let phase_inc = freq / config.sample_rate;
        Self {
            config,
            phase: 0.0,
            phase_inc,
        }
    }

    /// Create a ring modulator with default settings.
    #[must_use]
    pub fn default_effect() -> Self {
        Self::new(RingModConfig::default())
    }

    /// Update the carrier frequency.
    pub fn set_frequency(&mut self, frequency_hz: f32) {
        self.config.frequency_hz = frequency_hz.max(0.001);
        self.update_phase_inc();
    }

    /// Update the detune amount.
    pub fn set_detune(&mut self, detune_hz: f32) {
        self.config.detune_hz = detune_hz;
        self.update_phase_inc();
    }

    /// Set the wet/dry mix.
    pub fn set_mix(&mut self, mix: f32) {
        self.config.mix = mix.clamp(0.0, 1.0);
    }

    /// Set the carrier waveform.
    pub fn set_waveform(&mut self, waveform: CarrierWaveform) {
        self.config.waveform = waveform;
    }

    /// Set the sample rate and recalculate phase increment.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.config.sample_rate = sample_rate.max(1.0);
        self.update_phase_inc();
    }

    /// Reset the oscillator phase.
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// Recalculate the phase increment from current settings.
    fn update_phase_inc(&mut self) {
        let freq = self.config.frequency_hz + self.config.detune_hz;
        self.phase_inc = freq / self.config.sample_rate;
    }

    /// Get the current carrier oscillator value.
    fn carrier_value(&self) -> f32 {
        let p = self.phase;
        let raw = match self.config.waveform {
            CarrierWaveform::Sine => (2.0 * PI * p).sin(),
            CarrierWaveform::Square => {
                if p < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            CarrierWaveform::Triangle => {
                if p < 0.5 {
                    4.0 * p - 1.0
                } else {
                    3.0 - 4.0 * p
                }
            }
            CarrierWaveform::Sawtooth => 2.0 * p - 1.0,
        };
        raw * self.config.carrier_level
    }

    /// Advance the oscillator by one sample.
    fn advance(&mut self) {
        self.phase += self.phase_inc;
        while self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        while self.phase < 0.0 {
            self.phase += 1.0;
        }
    }

    /// Process a single mono sample.
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let carrier = self.carrier_value();
        self.advance();

        let wet = input * carrier;
        input * (1.0 - self.config.mix) + wet * self.config.mix
    }

    /// Process a mono buffer in-place.
    pub fn process(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    /// Process stereo buffers in-place.
    pub fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = left.len().min(right.len());
        for i in 0..len {
            let carrier = self.carrier_value();
            self.advance();

            let wet_l = left[i] * carrier;
            let wet_r = right[i] * carrier;
            left[i] = left[i] * (1.0 - self.config.mix) + wet_l * self.config.mix;
            right[i] = right[i] * (1.0 - self.config.mix) + wet_r * self.config.mix;
        }
    }

    /// Get the current carrier frequency including detune.
    #[must_use]
    pub fn effective_frequency(&self) -> f32 {
        self.config.frequency_hz + self.config.detune_hz
    }

    /// Compute the expected output frequencies for a given input frequency.
    #[must_use]
    pub fn output_frequencies(&self, input_freq: f32) -> (f32, f32) {
        let carrier_freq = self.effective_frequency();
        let sum = input_freq + carrier_freq;
        let diff = (input_freq - carrier_freq).abs();
        (sum, diff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = RingModConfig::default();
        assert!((cfg.frequency_hz - 440.0).abs() < 1e-6);
        assert!((cfg.mix - 1.0).abs() < 1e-6);
        assert_eq!(cfg.waveform, CarrierWaveform::Sine);
    }

    #[test]
    fn test_carrier_waveform_default() {
        assert_eq!(CarrierWaveform::default(), CarrierWaveform::Sine);
    }

    #[test]
    fn test_ring_mod_creation() {
        let rm = RingModulator::default_effect();
        assert!((rm.phase - 0.0).abs() < 1e-9);
        assert!(rm.phase_inc > 0.0);
    }

    #[test]
    fn test_set_frequency() {
        let mut rm = RingModulator::default_effect();
        rm.set_frequency(1000.0);
        assert!((rm.config.frequency_hz - 1000.0).abs() < 1e-6);
        let expected_inc = 1000.0 / 48000.0;
        assert!((rm.phase_inc - expected_inc).abs() < 1e-9);
    }

    #[test]
    fn test_set_mix() {
        let mut rm = RingModulator::default_effect();
        rm.set_mix(0.5);
        assert!((rm.config.mix - 0.5).abs() < 1e-6);
        rm.set_mix(2.0);
        assert!((rm.config.mix - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_dry_mix_passthrough() {
        let mut rm = RingModulator::new(RingModConfig {
            mix: 0.0,
            ..Default::default()
        });
        let input = 0.75f32;
        let output = rm.process_sample(input);
        assert!((output - input).abs() < 1e-6);
    }

    #[test]
    fn test_silence_input() {
        let mut rm = RingModulator::default_effect();
        let mut buffer = vec![0.0f32; 256];
        rm.process(&mut buffer);
        for &s in &buffer {
            assert!(s.abs() < 1e-9, "Ring mod of silence should be silence");
        }
    }

    #[test]
    fn test_carrier_sine_range() {
        let mut rm = RingModulator::new(RingModConfig {
            frequency_hz: 100.0,
            sample_rate: 1000.0,
            ..Default::default()
        });
        for _ in 0..1000 {
            let v = rm.carrier_value();
            assert!(v >= -1.0 && v <= 1.0);
            rm.advance();
        }
    }

    #[test]
    fn test_carrier_square() {
        let rm = RingModulator::new(RingModConfig {
            waveform: CarrierWaveform::Square,
            frequency_hz: 100.0,
            sample_rate: 1000.0,
            ..Default::default()
        });
        let v = rm.carrier_value();
        assert!(v.abs() > 0.99, "Square wave should be at extremes");
    }

    #[test]
    fn test_process_buffer() {
        let mut rm = RingModulator::default_effect();
        let mut buffer = vec![0.5f32; 128];
        rm.process(&mut buffer);
        // Output should not be all identical (carrier modulates)
        let all_same = buffer.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-9);
        assert!(!all_same, "Ring mod should produce varying output");
    }

    #[test]
    fn test_process_stereo() {
        let mut rm = RingModulator::default_effect();
        let mut left = vec![0.5f32; 64];
        let mut right = vec![0.5f32; 64];
        rm.process_stereo(&mut left, &mut right);
        // Both channels should be affected
        assert!(left.iter().any(|&s| (s - 0.5).abs() > 0.01));
    }

    #[test]
    fn test_output_frequencies() {
        let rm = RingModulator::new(RingModConfig {
            frequency_hz: 1000.0,
            ..Default::default()
        });
        let (sum, diff) = rm.output_frequencies(440.0);
        assert!((sum - 1440.0).abs() < 1e-3);
        assert!((diff - 560.0).abs() < 1e-3);
    }

    #[test]
    fn test_detune() {
        let mut rm = RingModulator::default_effect();
        rm.set_detune(5.0);
        assert!((rm.effective_frequency() - 445.0).abs() < 1e-6);
    }

    #[test]
    fn test_reset() {
        let mut rm = RingModulator::default_effect();
        for _ in 0..1000 {
            rm.advance();
        }
        assert!(rm.phase > 0.0);
        rm.reset();
        assert!((rm.phase - 0.0).abs() < 1e-9);
    }
}
