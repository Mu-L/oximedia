//! EAS audio alert insertion.

use crate::{AutomationError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info};

/// EAS attention tone configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionToneConfig {
    /// Frequency in Hz (standard: 853 Hz and 960 Hz)
    pub frequencies: Vec<f32>,
    /// Duration in seconds
    pub duration: f32,
    /// Volume (0.0 - 1.0)
    pub volume: f32,
}

impl Default for AttentionToneConfig {
    fn default() -> Self {
        Self {
            frequencies: vec![853.0, 960.0],
            duration: 8.0,
            volume: 0.8,
        }
    }
}

/// EAS audio configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EasAudioConfig {
    /// Attention tone configuration
    pub attention_tone: AttentionToneConfig,
    /// Path to TTS audio files
    pub tts_audio_path: Option<PathBuf>,
    /// Enable end-of-message tones
    pub enable_eom_tones: bool,
}

impl Default for EasAudioConfig {
    fn default() -> Self {
        Self {
            attention_tone: AttentionToneConfig::default(),
            tts_audio_path: None,
            enable_eom_tones: true,
        }
    }
}

/// EAS audio insertion handler.
pub struct EasAudioInsertion {
    config: EasAudioConfig,
}

impl EasAudioInsertion {
    /// Create a new EAS audio insertion handler.
    pub fn new(config: EasAudioConfig) -> Self {
        info!("Creating EAS audio insertion handler");

        Self { config }
    }

    /// Generate attention tone.
    pub fn generate_attention_tone(&self) -> Result<Vec<f32>> {
        debug!("Generating EAS attention tone");

        let sample_rate = 48000.0;
        let duration = self.config.attention_tone.duration;
        let num_samples = (sample_rate * duration) as usize;

        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / sample_rate;
            let mut sample = 0.0;

            // Generate dual-tone
            for &freq in &self.config.attention_tone.frequencies {
                sample += (2.0 * std::f32::consts::PI * freq * t).sin();
            }

            // Normalize and apply volume
            sample *= self.config.attention_tone.volume
                / self.config.attention_tone.frequencies.len() as f32;

            samples.push(sample);
        }

        Ok(samples)
    }

    /// Generate end-of-message tone.
    pub fn generate_eom_tone(&self) -> Result<Vec<f32>> {
        debug!("Generating EAS end-of-message tone");

        let sample_rate = 48000.0;
        let duration = 3.0; // 3 seconds for EOM
        let num_samples = (sample_rate * duration) as usize;

        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / sample_rate;
            // EOM is typically 853 Hz
            let sample =
                (2.0 * std::f32::consts::PI * 853.0 * t).sin() * self.config.attention_tone.volume;
            samples.push(sample);
        }

        Ok(samples)
    }

    /// Load TTS audio for message.
    ///
    /// # Safety / honesty note
    ///
    /// This is **not implemented**. An earlier revision returned
    /// `Ok(Vec::new())` (silence) for any message, which let
    /// [`Self::compose_message`] ship a complete-looking Emergency Alert
    /// System audio message — correct attention tone, correct
    /// end-of-message tone — around a **silent gap** where the actual
    /// spoken emergency message belongs. For EAS this is worse than a
    /// visible failure: a downstream broadcaster could air a "successful"
    /// alert that tells the public nothing. This now fails loudly instead
    /// of fabricating silence.
    ///
    /// # Errors
    ///
    /// Always returns [`AutomationError::Eas`]: real TTS synthesis/loading
    /// is not implemented.
    // TODO(0.2.x): real TTS integration — synthesize or load pre-recorded
    // audio for `message` (see `config.tts_audio_path`). An EAS message
    // MUST NOT ship with a silent spoken-message gap.
    pub fn load_tts_audio(&self, message: &str) -> Result<Vec<f32>> {
        debug!("Loading TTS audio for message: {}", message);

        Err(AutomationError::Eas(
            "TTS audio not available: real text-to-speech synthesis/loading is not \
             implemented"
                .to_string(),
        ))
    }

    /// Compose complete EAS audio message.
    ///
    /// # Safety / honesty note
    ///
    /// The spoken message is always requested via [`Self::load_tts_audio`]
    /// — composition is **not** gated on `config.tts_audio_path` being
    /// set, because a missing/unset path is not a legitimate reason to
    /// ship an EAS message with a silent body. Until real TTS exists (see
    /// [`Self::load_tts_audio`]), this function always propagates that
    /// error and therefore cannot return `Ok` with a silent gap where the
    /// emergency message belongs.
    ///
    /// # Errors
    ///
    /// Propagates the [`AutomationError::Eas`] from
    /// [`Self::load_tts_audio`] since real TTS is not implemented.
    pub fn compose_message(&self, message: &str) -> Result<Vec<f32>> {
        info!("Composing complete EAS audio message");

        let mut audio = Vec::new();

        // Add attention tone
        audio.extend(self.generate_attention_tone()?);

        // Add silence (1 second)
        audio.extend(vec![0.0; 48000]);

        // Add TTS message. Unconditional (not gated on tts_audio_path being
        // set): an EAS message must never ship with a silently-skipped
        // spoken body.
        audio.extend(self.load_tts_audio(message)?);

        // Add silence (1 second)
        audio.extend(vec![0.0; 48000]);

        // Add end-of-message tone
        if self.config.enable_eom_tones {
            audio.extend(self.generate_eom_tone()?);
        }

        Ok(audio)
    }

    /// Set attention tone volume.
    pub fn set_volume(&mut self, volume: f32) {
        self.config.attention_tone.volume = volume.clamp(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_insertion_creation() {
        let config = EasAudioConfig::default();
        let insertion = EasAudioInsertion::new(config);
        assert_eq!(insertion.config.attention_tone.frequencies.len(), 2);
    }

    #[test]
    fn test_generate_attention_tone() {
        let config = EasAudioConfig::default();
        let insertion = EasAudioInsertion::new(config);

        let tone = insertion
            .generate_attention_tone()
            .expect("generate_attention_tone should succeed");
        assert!(!tone.is_empty());
        assert_eq!(tone.len(), 48000 * 8); // 8 seconds at 48kHz
    }

    #[test]
    fn test_generate_eom_tone() {
        let config = EasAudioConfig::default();
        let insertion = EasAudioInsertion::new(config);

        let tone = insertion
            .generate_eom_tone()
            .expect("generate_eom_tone should succeed");
        assert!(!tone.is_empty());
        assert_eq!(tone.len(), 48000 * 3); // 3 seconds at 48kHz
    }

    #[test]
    fn test_compose_message_is_honest_err_not_silent_success() {
        // CHANGED: this test previously pinned the fabricated (and, for an
        // *Emergency Alert System*, unsafe) behavior — compose_message()
        // used to return Ok() with a complete-looking tone/silence/tone
        // structure while the actual spoken emergency message was silent,
        // because load_tts_audio() fabricated empty samples. Real TTS is
        // not implemented, so compose_message() must now fail loudly
        // instead of shipping a silent "successful" alert.
        let config = EasAudioConfig::default();
        let insertion = EasAudioInsertion::new(config);

        let result = insertion.compose_message("Test message");

        assert!(
            result.is_err(),
            "an EAS message with a silent spoken body must never report success"
        );
        assert!(matches!(result.unwrap_err(), AutomationError::Eas(_)));
    }

    #[test]
    fn test_load_tts_audio_is_honest_err() {
        let config = EasAudioConfig::default();
        let insertion = EasAudioInsertion::new(config);

        let result = insertion.load_tts_audio("Test message");

        assert!(
            result.is_err(),
            "load_tts_audio must not fabricate empty/silent samples as if TTS succeeded"
        );
    }

    #[test]
    fn test_compose_message_fails_even_without_configured_tts_path() {
        // The spoken message is mandatory regardless of whether
        // `tts_audio_path` is configured — an unset path is not a
        // legitimate reason to silently skip the emergency message.
        let mut config = EasAudioConfig::default();
        config.tts_audio_path = None;
        let insertion = EasAudioInsertion::new(config);

        assert!(insertion.compose_message("Test message").is_err());
    }
}
