// Copyright 2025 OxiMedia Contributors
// Licensed under the Apache License, Version 2.0

//! Audio extraction from media files.

use crate::Result;
use std::path::Path;

/// Extractor for audio from media files.
#[derive(Debug, Clone)]
pub struct AudioExtractor {
    format: AudioFormat,
    bitrate: Option<u64>,
    sample_rate: Option<u32>,
    channels: Option<u32>,
}

impl AudioExtractor {
    /// Create a new audio extractor with default settings (MP3, 192kbps).
    #[must_use]
    pub fn new() -> Self {
        Self {
            format: AudioFormat::Mp3,
            bitrate: Some(192_000),
            sample_rate: None,
            channels: None,
        }
    }

    /// Set the output audio format.
    #[must_use]
    pub fn with_format(mut self, format: AudioFormat) -> Self {
        self.format = format;
        self
    }

    /// Set the output bitrate.
    #[must_use]
    pub fn with_bitrate(mut self, bitrate: u64) -> Self {
        self.bitrate = Some(bitrate);
        self
    }

    /// Set the output sample rate.
    #[must_use]
    pub fn with_sample_rate(mut self, sample_rate: u32) -> Self {
        self.sample_rate = Some(sample_rate);
        self
    }

    /// Set the number of output channels.
    #[must_use]
    pub fn with_channels(mut self, channels: u32) -> Self {
        self.channels = Some(channels);
        self
    }

    /// Extract audio from a video file.
    pub async fn extract<P: AsRef<Path>, Q: AsRef<Path>>(&self, input: P, output: Q) -> Result<()> {
        let _input = input.as_ref();
        let _output = output.as_ref();

        // Placeholder for actual extraction
        // In a real implementation, this would use oximedia-transcode
        Ok(())
    }

    /// Extract audio with specific track index.
    pub async fn extract_track<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        input: P,
        output: Q,
        track_index: usize,
    ) -> Result<()> {
        let _input = input.as_ref();
        let _output = output.as_ref();
        let _track = track_index;

        // Placeholder for actual extraction
        Ok(())
    }

    /// Extract audio and normalize volume.
    pub async fn extract_normalized<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        input: P,
        output: Q,
        target_level: f32,
    ) -> Result<()> {
        let _input = input.as_ref();
        let _output = output.as_ref();
        let _level = target_level;

        // Placeholder for actual extraction with normalization
        Ok(())
    }

    /// Extract audio segment by time range.
    pub async fn extract_segment<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        input: P,
        output: Q,
        start_seconds: f64,
        duration_seconds: f64,
    ) -> Result<()> {
        let _input = input.as_ref();
        let _output = output.as_ref();
        let _start = start_seconds;
        let _duration = duration_seconds;

        // Placeholder for actual segment extraction
        Ok(())
    }

    /// Convert to mono.
    #[must_use]
    pub fn to_mono(self) -> Self {
        self.with_channels(1)
    }

    /// Convert to stereo.
    #[must_use]
    pub fn to_stereo(self) -> Self {
        self.with_channels(2)
    }

    /// Extract as MP3.
    #[must_use]
    pub fn as_mp3(self) -> Self {
        self.with_format(AudioFormat::Mp3)
    }

    /// Extract as AAC.
    #[must_use]
    pub fn as_aac(self) -> Self {
        self.with_format(AudioFormat::Aac)
    }

    /// Extract as FLAC (lossless).
    #[must_use]
    pub fn as_flac(self) -> Self {
        self.with_format(AudioFormat::Flac).with_bitrate(0)
    }

    /// Extract as Opus.
    #[must_use]
    pub fn as_opus(self) -> Self {
        self.with_format(AudioFormat::Opus)
    }
}

impl Default for AudioExtractor {
    fn default() -> Self {
        Self::new()
    }
}

/// Supported audio formats for extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    /// MP3
    Mp3,
    /// AAC
    Aac,
    /// FLAC (lossless)
    Flac,
    /// Opus
    Opus,
    /// Vorbis
    Vorbis,
    /// WAV (uncompressed)
    Wav,
}

impl AudioFormat {
    /// Get the file extension for this format.
    #[must_use]
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Aac => "m4a",
            Self::Flac => "flac",
            Self::Opus => "opus",
            Self::Vorbis => "ogg",
            Self::Wav => "wav",
        }
    }

    /// Get the codec name.
    #[must_use]
    pub fn codec(&self) -> &'static str {
        match self {
            Self::Mp3 => "libmp3lame",
            Self::Aac => "aac",
            Self::Flac => "flac",
            Self::Opus => "libopus",
            Self::Vorbis => "libvorbis",
            Self::Wav => "pcm_s16le",
        }
    }

    /// Check if this is a lossless format.
    #[must_use]
    pub fn is_lossless(&self) -> bool {
        matches!(self, Self::Flac | Self::Wav)
    }

    /// Get the default bitrate for this format.
    #[must_use]
    pub fn default_bitrate(&self) -> Option<u64> {
        match self {
            Self::Mp3 => Some(192_000),
            Self::Aac => Some(192_000),
            Self::Flac => None,
            Self::Opus => Some(128_000),
            Self::Vorbis => Some(192_000),
            Self::Wav => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extractor_creation() {
        let extractor = AudioExtractor::new();
        assert_eq!(extractor.format, AudioFormat::Mp3);
        assert_eq!(extractor.bitrate, Some(192_000));
    }

    #[test]
    fn test_format_settings() {
        let extractor = AudioExtractor::new()
            .with_format(AudioFormat::Flac)
            .with_bitrate(256_000)
            .with_sample_rate(48000)
            .with_channels(2);

        assert_eq!(extractor.format, AudioFormat::Flac);
        assert_eq!(extractor.bitrate, Some(256_000));
        assert_eq!(extractor.sample_rate, Some(48000));
        assert_eq!(extractor.channels, Some(2));
    }

    #[test]
    fn test_format_extension() {
        assert_eq!(AudioFormat::Mp3.extension(), "mp3");
        assert_eq!(AudioFormat::Aac.extension(), "m4a");
        assert_eq!(AudioFormat::Flac.extension(), "flac");
    }

    #[test]
    fn test_format_codec() {
        assert_eq!(AudioFormat::Mp3.codec(), "libmp3lame");
        assert_eq!(AudioFormat::Aac.codec(), "aac");
    }

    #[test]
    fn test_lossless_formats() {
        assert!(AudioFormat::Flac.is_lossless());
        assert!(AudioFormat::Wav.is_lossless());
        assert!(!AudioFormat::Mp3.is_lossless());
        assert!(!AudioFormat::Aac.is_lossless());
    }

    #[test]
    fn test_default_bitrate() {
        assert_eq!(AudioFormat::Mp3.default_bitrate(), Some(192_000));
        assert_eq!(AudioFormat::Flac.default_bitrate(), None);
    }

    #[test]
    fn test_convenience_methods() {
        let extractor = AudioExtractor::new().as_flac().to_mono();

        assert_eq!(extractor.format, AudioFormat::Flac);
        assert_eq!(extractor.channels, Some(1));

        let extractor = AudioExtractor::new().as_aac().to_stereo();

        assert_eq!(extractor.format, AudioFormat::Aac);
        assert_eq!(extractor.channels, Some(2));
    }
}
