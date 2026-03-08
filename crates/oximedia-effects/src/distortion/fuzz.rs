//! Fuzz distortion - hard clipping.

use crate::AudioEffect;

/// Fuzz configuration.
#[derive(Debug, Clone)]
pub struct FuzzConfig {
    /// Fuzz amount (1.0 - 100.0).
    pub fuzz: f32,
    /// Output level (0.0 - 1.0).
    pub level: f32,
}

impl Default for FuzzConfig {
    fn default() -> Self {
        Self {
            fuzz: 10.0,
            level: 0.3,
        }
    }
}

/// Fuzz distortion effect.
pub struct Fuzz {
    config: FuzzConfig,
}

impl Fuzz {
    /// Create new fuzz effect.
    #[must_use]
    pub fn new(config: FuzzConfig) -> Self {
        Self { config }
    }

    /// Hard clipping function.
    fn hard_clip(x: f32) -> f32 {
        x.clamp(-1.0, 1.0)
    }
}

impl AudioEffect for Fuzz {
    fn process_sample(&mut self, input: f32) -> f32 {
        let fuzzed = input * self.config.fuzz;
        Self::hard_clip(fuzzed) * self.config.level
    }

    fn reset(&mut self) {
        // No state to reset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzz() {
        let config = FuzzConfig::default();
        let mut fuzz = Fuzz::new(config);
        let output = fuzz.process_sample(0.5);
        assert!(output.is_finite());
        assert!(output.abs() <= 1.0);
    }
}
