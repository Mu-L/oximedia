//! Low-Frequency Oscillator (LFO) implementation.
//!
//! Provides oscillators for modulating effect parameters. Supports multiple
//! waveforms and is optimized for real-time use.

use std::f32::consts::TAU;

/// Waveform types for LFO.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LfoWaveform {
    /// Sine wave - smooth, natural modulation.
    Sine,
    /// Triangle wave - linear rise and fall.
    Triangle,
    /// Sawtooth wave - linear rise with instant reset.
    Sawtooth,
    /// Square wave - instant switching between min and max.
    Square,
    /// Random sample-and-hold.
    Random,
}

/// Low-Frequency Oscillator.
///
/// Generates periodic control signals for modulating effect parameters.
#[derive(Debug, Clone)]
pub struct Lfo {
    /// Current phase (0.0 - 1.0).
    phase: f32,
    /// Phase increment per sample.
    phase_inc: f32,
    /// Waveform type.
    waveform: LfoWaveform,
    /// Sample rate.
    sample_rate: f32,
    /// Random state for Random waveform.
    random_state: u32,
    /// Previous random value.
    random_value: f32,
}

impl Lfo {
    /// Create a new LFO.
    ///
    /// # Arguments
    ///
    /// * `frequency` - LFO frequency in Hz
    /// * `sample_rate` - Audio sample rate
    /// * `waveform` - Waveform type
    #[must_use]
    pub fn new(frequency: f32, sample_rate: f32, waveform: LfoWaveform) -> Self {
        let phase_inc = frequency / sample_rate;
        Self {
            phase: 0.0,
            phase_inc,
            waveform,
            sample_rate,
            random_state: 0x1234_5678,
            random_value: 0.0,
        }
    }

    /// Get the next LFO value (range: -1.0 to 1.0).
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f32 {
        let value = match self.waveform {
            LfoWaveform::Sine => (self.phase * TAU).sin(),
            LfoWaveform::Triangle => {
                if self.phase < 0.5 {
                    4.0 * self.phase - 1.0
                } else {
                    3.0 - 4.0 * self.phase
                }
            }
            LfoWaveform::Sawtooth => 2.0 * self.phase - 1.0,
            LfoWaveform::Square => {
                if self.phase < 0.5 {
                    -1.0
                } else {
                    1.0
                }
            }
            LfoWaveform::Random => {
                // Update on phase wrap
                if self.phase < self.phase_inc {
                    self.random_state = self
                        .random_state
                        .wrapping_mul(1_103_515_245)
                        .wrapping_add(12_345);
                    #[allow(clippy::cast_precision_loss)]
                    let random_val = (self.random_state as f32) / (u32::MAX as f32);
                    self.random_value = 2.0 * random_val - 1.0;
                }
                self.random_value
            }
        };

        self.phase += self.phase_inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        value
    }

    /// Get LFO value in unipolar range (0.0 to 1.0).
    pub fn next_unipolar(&mut self) -> f32 {
        (self.next() + 1.0) * 0.5
    }

    /// Set the LFO frequency.
    pub fn set_frequency(&mut self, frequency: f32) {
        self.phase_inc = frequency / self.sample_rate;
    }

    /// Set the waveform.
    pub fn set_waveform(&mut self, waveform: LfoWaveform) {
        self.waveform = waveform;
    }

    /// Reset the LFO phase to 0.
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// Set the phase (0.0 - 1.0).
    pub fn set_phase(&mut self, phase: f32) {
        self.phase = phase.rem_euclid(1.0);
    }

    /// Get current phase (0.0 - 1.0).
    #[must_use]
    pub fn phase(&self) -> f32 {
        self.phase
    }
}

/// Stereo LFO with independent or linked phases.
#[derive(Debug, Clone)]
pub struct StereoLfo {
    /// Left channel LFO.
    pub left: Lfo,
    /// Right channel LFO.
    pub right: Lfo,
}

impl StereoLfo {
    /// Create a new stereo LFO with a phase offset between channels.
    ///
    /// # Arguments
    ///
    /// * `frequency` - LFO frequency in Hz
    /// * `sample_rate` - Audio sample rate
    /// * `waveform` - Waveform type
    /// * `phase_offset` - Phase offset for right channel (0.0 - 1.0)
    #[must_use]
    pub fn new(frequency: f32, sample_rate: f32, waveform: LfoWaveform, phase_offset: f32) -> Self {
        let left = Lfo::new(frequency, sample_rate, waveform);
        let mut right = Lfo::new(frequency, sample_rate, waveform);
        right.set_phase(phase_offset);

        Self { left, right }
    }

    /// Get the next stereo LFO values.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> (f32, f32) {
        (self.left.next(), self.right.next())
    }

    /// Get stereo values in unipolar range.
    pub fn next_unipolar(&mut self) -> (f32, f32) {
        (self.left.next_unipolar(), self.right.next_unipolar())
    }

    /// Set frequency for both channels.
    pub fn set_frequency(&mut self, frequency: f32) {
        self.left.set_frequency(frequency);
        self.right.set_frequency(frequency);
    }

    /// Set waveform for both channels.
    pub fn set_waveform(&mut self, waveform: LfoWaveform) {
        self.left.set_waveform(waveform);
        self.right.set_waveform(waveform);
    }

    /// Reset both LFOs.
    pub fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
    }
}

/// Parameter smoother for avoiding clicks and pops.
///
/// Uses a one-pole lowpass filter to smooth parameter changes.
#[derive(Debug, Clone)]
pub struct ParameterSmoother {
    /// Current value.
    current: f32,
    /// Target value.
    target: f32,
    /// Smoothing coefficient (0.0 - 1.0).
    coefficient: f32,
}

impl ParameterSmoother {
    /// Create a new parameter smoother.
    ///
    /// # Arguments
    ///
    /// * `time_constant_ms` - Time constant in milliseconds
    /// * `sample_rate` - Audio sample rate
    #[must_use]
    pub fn new(time_constant_ms: f32, sample_rate: f32) -> Self {
        let coefficient = (-1000.0 / (time_constant_ms * sample_rate)).exp();
        Self {
            current: 0.0,
            target: 0.0,
            coefficient,
        }
    }

    /// Set the target value.
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Get the next smoothed value.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f32 {
        self.current = self.target + self.coefficient * (self.current - self.target);
        self.current
    }

    /// Reset to a specific value instantly.
    pub fn reset(&mut self, value: f32) {
        self.current = value;
        self.target = value;
    }

    /// Check if the smoother has reached its target.
    #[must_use]
    pub fn is_stable(&self) -> bool {
        (self.current - self.target).abs() < 1e-3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lfo_sine() {
        let mut lfo = Lfo::new(1.0, 100.0, LfoWaveform::Sine);

        // At phase 0, sine should be 0
        let v0 = lfo.next();
        assert!(v0.abs() < 0.1);

        // At phase 0.25, sine should be ~1.0
        for _ in 0..24 {
            lfo.next();
        }
        let v25 = lfo.next();
        assert!((v25 - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_lfo_triangle() {
        let mut lfo = Lfo::new(1.0, 100.0, LfoWaveform::Triangle);

        // At phase 0, triangle should be -1
        let v0 = lfo.next();
        assert!((v0 + 1.0).abs() < 0.1);

        // Triangle increases linearly
        let v1 = lfo.next();
        assert!(v1 > v0);
    }

    #[test]
    fn test_lfo_square() {
        let mut lfo = Lfo::new(1.0, 100.0, LfoWaveform::Square);

        // Square wave should only output -1.0 or 1.0
        for _ in 0..100 {
            let val = lfo.next();
            assert!(
                val == -1.0 || val == 1.0,
                "Square wave should only be -1.0 or 1.0"
            );
        }
    }

    #[test]
    fn test_lfo_unipolar() {
        let mut lfo = Lfo::new(1.0, 100.0, LfoWaveform::Sine);

        for _ in 0..100 {
            let val = lfo.next_unipolar();
            assert!(val >= 0.0 && val <= 1.0);
        }
    }

    #[test]
    fn test_stereo_lfo() {
        let mut lfo = StereoLfo::new(1.0, 100.0, LfoWaveform::Sine, 0.5);

        let (l, r) = lfo.next();
        // With 0.5 phase offset (180 degrees), values should be opposite
        assert!((l + r).abs() < 0.1);
    }

    #[test]
    fn test_parameter_smoother() {
        let mut smoother = ParameterSmoother::new(10.0, 48000.0);
        smoother.reset(0.0);
        smoother.set_target(1.0);

        // Should gradually approach target
        let v1 = smoother.next();
        let v2 = smoother.next();
        let v3 = smoother.next();

        assert!(v1 > 0.0 && v1 < 1.0);
        assert!(v2 > v1);
        assert!(v3 > v2);

        // After many samples, should be close to target
        for _ in 0..100000 {
            smoother.next();
        }
        assert!(smoother.is_stable());
    }

    #[test]
    fn test_lfo_reset() {
        let mut lfo = Lfo::new(1.0, 100.0, LfoWaveform::Sine);

        // Advance phase
        for _ in 0..25 {
            lfo.next();
        }
        assert!(lfo.phase() > 0.0);

        // Reset should bring back to 0
        lfo.reset();
        assert_eq!(lfo.phase(), 0.0);
    }
}
