//! Basic delay effect with feedback and filtering.

use crate::{
    utils::{DelayLine, ParameterSmoother},
    AudioEffect,
};

/// Configuration for delay effect.
#[derive(Debug, Clone)]
pub struct DelayConfig {
    /// Delay time in milliseconds.
    pub delay_ms: f32,
    /// Feedback amount (0.0 - 1.0).
    pub feedback: f32,
    /// Wet signal level (0.0 - 1.0).
    pub wet: f32,
    /// Dry signal level (0.0 - 1.0).
    pub dry: f32,
    /// Low-pass filter cutoff for feedback (0.0 = no filtering, 1.0 = maximum filtering).
    pub tone: f32,
}

impl Default for DelayConfig {
    fn default() -> Self {
        Self {
            delay_ms: 500.0,
            feedback: 0.4,
            wet: 0.5,
            dry: 0.5,
            tone: 0.0,
        }
    }
}

impl DelayConfig {
    /// Create a new delay configuration.
    #[must_use]
    pub fn new(delay_ms: f32, feedback: f32, wet: f32) -> Self {
        Self {
            delay_ms: delay_ms.max(0.0),
            feedback: feedback.clamp(0.0, 0.99),
            wet: wet.clamp(0.0, 1.0),
            dry: (1.0 - wet).clamp(0.0, 1.0),
            tone: 0.0,
        }
    }

    /// Slapback delay preset (short, single echo).
    #[must_use]
    pub fn slapback() -> Self {
        Self::new(100.0, 0.0, 0.3)
    }

    /// Dotted eighth note delay preset (at 120 BPM).
    #[must_use]
    pub fn dotted_eighth() -> Self {
        Self::new(375.0, 0.35, 0.4)
    }

    /// Long ambient delay preset.
    #[must_use]
    pub fn ambient() -> Self {
        Self::new(750.0, 0.6, 0.5).with_tone(0.4)
    }

    /// Set tone control.
    #[must_use]
    pub fn with_tone(mut self, tone: f32) -> Self {
        self.tone = tone.clamp(0.0, 1.0);
        self
    }
}

/// Simple mono delay effect.
pub struct MonoDelay {
    delay_line: DelayLine,
    delay_samples: usize,
    config: DelayConfig,
    tone_filter: f32,
    tone_smoother: ParameterSmoother,
    sample_rate: f32,
}

impl MonoDelay {
    /// Create a new mono delay.
    #[must_use]
    pub fn new(config: DelayConfig, sample_rate: f32) -> Self {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let max_delay_samples = ((2000.0 * sample_rate) / 1000.0) as usize; // 2 second max

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let delay_samples = ((config.delay_ms * sample_rate) / 1000.0) as usize;

        Self {
            delay_line: DelayLine::new(max_delay_samples),
            delay_samples,
            config,
            tone_filter: 0.0,
            tone_smoother: ParameterSmoother::new(10.0, sample_rate),
            sample_rate,
        }
    }

    /// Set delay time in milliseconds.
    pub fn set_delay_ms(&mut self, delay_ms: f32) {
        self.config.delay_ms = delay_ms.max(0.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let delay_samp = ((delay_ms * self.sample_rate) / 1000.0) as usize;
        self.delay_samples = delay_samp.min(self.delay_line.max_delay());
    }

    /// Set feedback amount.
    pub fn set_feedback(&mut self, feedback: f32) {
        self.config.feedback = feedback.clamp(0.0, 0.99);
    }

    /// Set wet level.
    pub fn set_wet(&mut self, wet: f32) {
        self.config.wet = wet.clamp(0.0, 1.0);
    }

    /// Set dry level.
    pub fn set_dry(&mut self, dry: f32) {
        self.config.dry = dry.clamp(0.0, 1.0);
    }

    /// Set tone (low-pass filter for feedback).
    pub fn set_tone(&mut self, tone: f32) {
        self.config.tone = tone.clamp(0.0, 1.0);
        self.tone_smoother.set_target(self.config.tone);
    }
}

impl AudioEffect for MonoDelay {
    fn process_sample(&mut self, input: f32) -> f32 {
        // Read delayed sample
        let delayed = self.delay_line.read(self.delay_samples);

        // Apply tone filter to feedback (simple one-pole lowpass)
        let tone = self.tone_smoother.next();
        self.tone_filter = delayed * (1.0 - tone) + self.tone_filter * tone;

        // Write input + filtered feedback to delay line
        let feedback_signal = self.tone_filter * self.config.feedback;
        self.delay_line.write(input + feedback_signal);

        // Mix wet and dry
        delayed * self.config.wet + input * self.config.dry
    }

    fn reset(&mut self) {
        self.delay_line.clear();
        self.tone_filter = 0.0;
        self.tone_smoother.reset(0.0);
    }

    fn latency_samples(&self) -> usize {
        0 // Zero latency (delay is part of the effect)
    }
}

/// Stereo delay effect.
pub struct StereoDelay {
    left: MonoDelay,
    right: MonoDelay,
    cross_feedback: f32,
}

impl StereoDelay {
    /// Create a new stereo delay.
    #[must_use]
    pub fn new(config: DelayConfig, sample_rate: f32) -> Self {
        Self {
            left: MonoDelay::new(config.clone(), sample_rate),
            right: MonoDelay::new(config, sample_rate),
            cross_feedback: 0.0,
        }
    }

    /// Create with different delay times for left and right.
    #[must_use]
    pub fn new_dual(left_config: DelayConfig, right_config: DelayConfig, sample_rate: f32) -> Self {
        Self {
            left: MonoDelay::new(left_config, sample_rate),
            right: MonoDelay::new(right_config, sample_rate),
            cross_feedback: 0.0,
        }
    }

    /// Set cross-feedback amount (feedback from left to right and vice versa).
    pub fn set_cross_feedback(&mut self, amount: f32) {
        self.cross_feedback = amount.clamp(0.0, 0.99);
    }

    /// Set delay time for both channels.
    pub fn set_delay_ms(&mut self, delay_ms: f32) {
        self.left.set_delay_ms(delay_ms);
        self.right.set_delay_ms(delay_ms);
    }

    /// Set delay time for left channel.
    pub fn set_left_delay_ms(&mut self, delay_ms: f32) {
        self.left.set_delay_ms(delay_ms);
    }

    /// Set delay time for right channel.
    pub fn set_right_delay_ms(&mut self, delay_ms: f32) {
        self.right.set_delay_ms(delay_ms);
    }

    fn process_sample_internal(&mut self, input_l: f32, input_r: f32) -> (f32, f32) {
        let out_l = self.left.process_sample(input_l);
        let out_r = self.right.process_sample(input_r);

        // Apply cross-feedback if enabled
        if self.cross_feedback > 0.0 {
            let cross_l = out_r * self.cross_feedback;
            let cross_r = out_l * self.cross_feedback;
            (out_l + cross_l, out_r + cross_r)
        } else {
            (out_l, out_r)
        }
    }
}

impl AudioEffect for StereoDelay {
    fn process_sample(&mut self, input: f32) -> f32 {
        let (left, _right) = self.process_sample_internal(input, input);
        left
    }

    fn process_sample_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.process_sample_internal(left, right)
    }

    fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_config() {
        let config = DelayConfig::default();
        assert_eq!(config.delay_ms, 500.0);
        assert_eq!(config.feedback, 0.4);
    }

    #[test]
    fn test_delay_presets() {
        let slapback = DelayConfig::slapback();
        assert!(slapback.delay_ms < 200.0);

        let ambient = DelayConfig::ambient();
        assert!(ambient.delay_ms > 500.0);
    }

    #[test]
    fn test_mono_delay() {
        let config = DelayConfig::new(100.0, 0.5, 0.5);
        let mut delay = MonoDelay::new(config, 48000.0);

        // Process impulse
        let out1 = delay.process_sample(1.0);
        assert!((out1 - 0.5).abs() < 0.01); // Should be mostly dry initially

        // Process silence - should get delayed echo
        for _ in 0..4799 {
            delay.process_sample(0.0);
        }

        let echo = delay.process_sample(0.0);
        assert!(echo.abs() > 0.1); // Should have echo now
    }

    #[test]
    fn test_stereo_delay() {
        let config = DelayConfig::default();
        let mut delay = StereoDelay::new(config, 48000.0);

        let (out_l, out_r) = delay.process_sample_stereo(1.0, 0.5);
        assert!(out_l != out_r); // Different inputs should give different outputs
    }

    #[test]
    fn test_delay_reset() {
        let config = DelayConfig::new(100.0, 0.9, 0.5);
        let mut delay = MonoDelay::new(config, 48000.0);

        // Fill delay line
        for _ in 0..1000 {
            delay.process_sample(1.0);
        }

        delay.reset();

        // After reset, delay line should be clear
        let output = delay.process_sample(0.0);
        assert!(output.abs() < 0.01);
    }
}
