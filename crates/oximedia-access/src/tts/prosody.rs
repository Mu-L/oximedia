//! Prosody control for TTS.

use serde::{Deserialize, Serialize};

/// Prosody configuration for speech synthesis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProsodyConfig {
    /// Speech rate multiplier (0.5 to 2.0).
    pub rate: f32,
    /// Pitch shift in semitones (-12 to 12).
    pub pitch: f32,
    /// Volume level (0.0 to 1.0).
    pub volume: f32,
    /// Emphasis level (0.0 to 1.0).
    pub emphasis: f32,
}

impl Default for ProsodyConfig {
    fn default() -> Self {
        Self {
            rate: 1.0,
            pitch: 0.0,
            volume: 0.8,
            emphasis: 0.5,
        }
    }
}

/// Controls prosody (pitch, rate, volume) of synthesized speech.
pub struct ProsodyControl {
    config: ProsodyConfig,
}

impl ProsodyControl {
    /// Create a new prosody controller.
    #[must_use]
    pub const fn new(config: ProsodyConfig) -> Self {
        Self { config }
    }

    /// Set speech rate.
    pub fn set_rate(&mut self, rate: f32) {
        self.config.rate = rate.clamp(0.5, 2.0);
    }

    /// Set pitch shift.
    pub fn set_pitch(&mut self, pitch: f32) {
        self.config.pitch = pitch.clamp(-12.0, 12.0);
    }

    /// Set volume.
    pub fn set_volume(&mut self, volume: f32) {
        self.config.volume = volume.clamp(0.0, 1.0);
    }

    /// Generate SSML markup for prosody.
    #[must_use]
    pub fn to_ssml(&self, text: &str) -> String {
        format!(
            "<prosody rate=\"{}\" pitch=\"{}st\" volume=\"{}\">{}</prosody>",
            self.config.rate,
            self.config.pitch,
            self.config.volume * 100.0,
            text
        )
    }

    /// Get configuration.
    #[must_use]
    pub const fn config(&self) -> &ProsodyConfig {
        &self.config
    }
}

impl Default for ProsodyControl {
    fn default() -> Self {
        Self::new(ProsodyConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prosody_creation() {
        let prosody = ProsodyControl::default();
        assert!((prosody.config().rate - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_set_rate() {
        let mut prosody = ProsodyControl::default();
        prosody.set_rate(1.5);
        assert!((prosody.config().rate - 1.5).abs() < f32::EPSILON);

        prosody.set_rate(5.0);
        assert!((prosody.config().rate - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_ssml_generation() {
        let prosody = ProsodyControl::default();
        let ssml = prosody.to_ssml("Hello world");
        assert!(ssml.contains("prosody"));
        assert!(ssml.contains("Hello world"));
    }
}
