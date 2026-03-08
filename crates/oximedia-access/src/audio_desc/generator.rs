//! Audio description generation.

use crate::audio_desc::script::AudioDescriptionScript;
use crate::audio_desc::{AudioDescriptionQuality, AudioDescriptionType};
use crate::error::{AccessError, AccessResult};
use bytes::Bytes;
use oximedia_audio::frame::AudioBuffer;
use serde::{Deserialize, Serialize};

/// Configuration for audio description generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDescriptionConfig {
    /// Type of audio description.
    pub ad_type: AudioDescriptionType,
    /// Quality level.
    pub quality: AudioDescriptionQuality,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Number of channels.
    pub channels: u16,
    /// Voice name for TTS (if using synthetic voice).
    pub voice: Option<String>,
    /// Speech rate (0.5 to 2.0, 1.0 is normal).
    pub speech_rate: f32,
    /// Pitch adjustment in semitones (-12 to 12).
    pub pitch: f32,
    /// Volume level (0.0 to 1.0).
    pub volume: f32,
}

impl Default for AudioDescriptionConfig {
    fn default() -> Self {
        Self {
            ad_type: AudioDescriptionType::Standard,
            quality: AudioDescriptionQuality::Standard,
            sample_rate: 48000,
            channels: 2,
            voice: None,
            speech_rate: 1.0,
            pitch: 0.0,
            volume: 0.8,
        }
    }
}

impl AudioDescriptionConfig {
    /// Create a new configuration.
    #[must_use]
    pub fn new(ad_type: AudioDescriptionType, quality: AudioDescriptionQuality) -> Self {
        Self {
            ad_type,
            quality,
            ..Default::default()
        }
    }

    /// Set voice name.
    #[must_use]
    pub fn with_voice(mut self, voice: String) -> Self {
        self.voice = Some(voice);
        self
    }

    /// Set speech rate.
    #[must_use]
    pub fn with_speech_rate(mut self, rate: f32) -> Self {
        self.speech_rate = rate.clamp(0.5, 2.0);
        self
    }

    /// Set pitch adjustment.
    #[must_use]
    pub fn with_pitch(mut self, pitch: f32) -> Self {
        self.pitch = pitch.clamp(-12.0, 12.0);
        self
    }

    /// Set volume level.
    #[must_use]
    pub fn with_volume(mut self, volume: f32) -> Self {
        self.volume = volume.clamp(0.0, 1.0);
        self
    }

    /// Validate configuration.
    pub fn validate(&self) -> AccessResult<()> {
        if self.sample_rate < 8000 || self.sample_rate > 192000 {
            return Err(AccessError::AudioDescriptionFailed(
                "Invalid sample rate".to_string(),
            ));
        }

        if self.channels == 0 || self.channels > 8 {
            return Err(AccessError::AudioDescriptionFailed(
                "Invalid channel count".to_string(),
            ));
        }

        Ok(())
    }
}

/// Audio description generator.
///
/// Generates audio descriptions from text scripts using text-to-speech
/// or pre-recorded audio segments.
pub struct AudioDescriptionGenerator {
    config: AudioDescriptionConfig,
}

impl AudioDescriptionGenerator {
    /// Create a new audio description generator.
    #[must_use]
    pub fn new(config: AudioDescriptionConfig) -> Self {
        Self { config }
    }

    /// Create generator with default configuration.
    #[must_use]
    pub fn default() -> Self {
        Self::new(AudioDescriptionConfig::default())
    }

    /// Generate audio description from script.
    ///
    /// This is an integration point for external TTS services.
    /// In production, this would call services like:
    /// - Amazon Polly
    /// - Google Cloud Text-to-Speech
    /// - Microsoft Azure Speech
    /// - Local TTS engines (eSpeak, Festival, etc.)
    pub fn generate(&self, script: &AudioDescriptionScript) -> AccessResult<Vec<AudioSegment>> {
        self.config.validate()?;

        let mut segments = Vec::new();

        for entry in script.entries() {
            let audio = self.synthesize_text(&entry.text)?;

            segments.push(AudioSegment {
                start_time_ms: entry.start_time_ms,
                end_time_ms: entry.end_time_ms,
                audio,
                metadata: SegmentMetadata {
                    text: entry.text.clone(),
                    voice: self.config.voice.clone(),
                    duration_ms: entry.duration_ms(),
                },
            });
        }

        Ok(segments)
    }

    /// Generate from pre-recorded audio files.
    pub fn generate_from_audio(
        &self,
        script: &AudioDescriptionScript,
        audio_files: &[String],
    ) -> AccessResult<Vec<AudioSegment>> {
        if script.entries().len() != audio_files.len() {
            return Err(AccessError::AudioDescriptionFailed(
                "Script entries and audio files count mismatch".to_string(),
            ));
        }

        let mut segments = Vec::new();

        for (entry, _audio_file) in script.entries().iter().zip(audio_files.iter()) {
            // In production, load audio from file
            let audio = self.load_audio_file(_audio_file)?;

            segments.push(AudioSegment {
                start_time_ms: entry.start_time_ms,
                end_time_ms: entry.end_time_ms,
                audio,
                metadata: SegmentMetadata {
                    text: entry.text.clone(),
                    voice: None,
                    duration_ms: entry.duration_ms(),
                },
            });
        }

        Ok(segments)
    }

    /// Synthesize text to speech.
    ///
    /// Integration point for TTS services.
    fn synthesize_text(&self, text: &str) -> AccessResult<AudioBuffer> {
        if text.is_empty() {
            return Err(AccessError::AudioDescriptionFailed(
                "Empty text for synthesis".to_string(),
            ));
        }

        // Placeholder: In production, call TTS service
        // Example: AWS Polly, Google TTS, Azure Speech, etc.

        // Create silent audio buffer as placeholder
        let duration_samples = text.len() * 100; // Rough estimate
        let samples = vec![0.0f32; duration_samples * self.config.channels as usize];

        // Encode f32 samples as bytes (little-endian)
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();

        Ok(AudioBuffer::Interleaved(Bytes::from(bytes)))
    }

    /// Load audio from file.
    ///
    /// Integration point for audio file loading.
    fn load_audio_file(&self, _path: &str) -> AccessResult<AudioBuffer> {
        // Placeholder: In production, load and decode audio file
        // Example: Use oximedia-codec to decode the file

        // Create silent stereo audio (48000 samples * 2 channels * 4 bytes per f32)
        let bytes = vec![0u8; 48000 * 2 * 4];
        Ok(AudioBuffer::Interleaved(Bytes::from(bytes)))
    }

    /// Get configuration.
    #[must_use]
    pub const fn config(&self) -> &AudioDescriptionConfig {
        &self.config
    }

    /// Validate script timing against quality constraints.
    pub fn validate_script(&self, script: &AudioDescriptionScript) -> AccessResult<()> {
        let min_duration = self.config.quality.min_duration_ms();

        for entry in script.entries() {
            if entry.duration_ms() < min_duration {
                return Err(AccessError::AudioDescriptionFailed(format!(
                    "Description at {}ms is too short ({}ms < {}ms minimum)",
                    entry.start_time_ms,
                    entry.duration_ms(),
                    min_duration
                )));
            }
        }

        Ok(())
    }
}

/// Audio description segment.
#[derive(Debug, Clone)]
pub struct AudioSegment {
    /// Start time in milliseconds.
    pub start_time_ms: i64,
    /// End time in milliseconds.
    pub end_time_ms: i64,
    /// Audio data.
    pub audio: AudioBuffer,
    /// Metadata.
    pub metadata: SegmentMetadata,
}

/// Metadata for an audio segment.
#[derive(Debug, Clone)]
pub struct SegmentMetadata {
    /// Original text.
    pub text: String,
    /// Voice used for synthesis.
    pub voice: Option<String>,
    /// Duration in milliseconds.
    pub duration_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_desc::script::AudioDescriptionEntry;

    #[test]
    fn test_config_default() {
        let config = AudioDescriptionConfig::default();
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.channels, 2);
        assert_eq!(config.volume, 0.8);
    }

    #[test]
    fn test_config_validation() {
        let mut config = AudioDescriptionConfig::default();
        assert!(config.validate().is_ok());

        config.sample_rate = 4000;
        assert!(config.validate().is_err());

        config.sample_rate = 48000;
        config.channels = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_builder() {
        let config = AudioDescriptionConfig::default()
            .with_voice("en-US-Neural".to_string())
            .with_speech_rate(1.2)
            .with_pitch(2.0)
            .with_volume(0.9);

        assert_eq!(config.voice.as_deref(), Some("en-US-Neural"));
        assert!((config.speech_rate - 1.2).abs() < f32::EPSILON);
        assert!((config.pitch - 2.0).abs() < f32::EPSILON);
        assert!((config.volume - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn test_generator_creation() {
        let generator = AudioDescriptionGenerator::default();
        assert_eq!(generator.config().sample_rate, 48000);
    }

    #[test]
    fn test_script_validation() {
        let generator = AudioDescriptionGenerator::new(AudioDescriptionConfig::new(
            AudioDescriptionType::Standard,
            AudioDescriptionQuality::Standard,
        ));

        let mut script = AudioDescriptionScript::new();
        script.add_entry(AudioDescriptionEntry::new(
            1000,
            2200,
            "Valid description".to_string(),
        ));

        assert!(generator.validate_script(&script).is_ok());

        script.add_entry(AudioDescriptionEntry::new(
            3000,
            3200,
            "Too short".to_string(),
        ));

        assert!(generator.validate_script(&script).is_err());
    }
}
