//! Overdrive distortion - soft clipping.

use crate::AudioEffect;

/// Overdrive configuration.
#[derive(Debug, Clone)]
pub struct OverdriveConfig {
    /// Drive amount (1.0 - 100.0).
    pub drive: f32,
    /// Tone control (0.0 - 1.0, higher = brighter).
    pub tone: f32,
    /// Output level (0.0 - 2.0).
    pub level: f32,
}

impl Default for OverdriveConfig {
    fn default() -> Self {
        Self {
            drive: 5.0,
            tone: 0.5,
            level: 0.5,
        }
    }
}

/// Overdrive effect with soft clipping.
pub struct Overdrive {
    config: OverdriveConfig,
    tone_filter: f32,
}

impl Overdrive {
    /// Create new overdrive effect.
    #[must_use]
    pub fn new(config: OverdriveConfig) -> Self {
        Self {
            config,
            tone_filter: 0.0,
        }
    }

    /// Soft clipping function (tanh-like).
    fn soft_clip(x: f32) -> f32 {
        if x > 1.0 {
            2.0 / 3.0
        } else if x < -1.0 {
            -2.0 / 3.0
        } else {
            x - (x * x * x) / 3.0
        }
    }
}

impl AudioEffect for Overdrive {
    fn process_sample(&mut self, input: f32) -> f32 {
        // Apply drive
        let driven = input * self.config.drive;

        // Soft clip
        let clipped = Self::soft_clip(driven);

        // Apply tone control (simple one-pole lowpass)
        let tone_coeff = 1.0 - self.config.tone;
        self.tone_filter = clipped * (1.0 - tone_coeff) + self.tone_filter * tone_coeff;

        // Apply output level
        self.tone_filter * self.config.level
    }

    fn reset(&mut self) {
        self.tone_filter = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overdrive() {
        let config = OverdriveConfig::default();
        let mut overdrive = Overdrive::new(config);
        let output = overdrive.process_sample(0.5);
        assert!(output.is_finite());
    }

    #[test]
    fn test_soft_clip() {
        assert!(Overdrive::soft_clip(0.0).abs() < 0.01);
        assert!(Overdrive::soft_clip(0.5).abs() < 1.0);
        assert!(Overdrive::soft_clip(2.0) <= 1.0);
    }
}
