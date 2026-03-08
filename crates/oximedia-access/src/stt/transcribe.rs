//! Speech-to-text transcription.

use crate::error::AccessResult;
use crate::stt::SttConfig;
use crate::transcript::{Transcript, TranscriptEntry};
use bytes::Bytes;
use oximedia_audio::frame::AudioBuffer;

/// Speech-to-text transcriber.
pub struct SpeechToText {
    config: SttConfig,
}

impl SpeechToText {
    /// Create a new STT transcriber.
    #[must_use]
    pub const fn new(config: SttConfig) -> Self {
        Self { config }
    }

    /// Transcribe audio to text.
    ///
    /// Integration point for STT services:
    /// - `OpenAI` Whisper
    /// - Google Cloud Speech-to-Text
    /// - Amazon Transcribe
    /// - Microsoft Azure Speech
    /// - `AssemblyAI`
    /// - Local models (Vosk, `DeepSpeech`, etc.)
    pub fn transcribe(&self, _audio: &AudioBuffer) -> AccessResult<Transcript> {
        // Placeholder: Call STT service
        // In production, this would call external STT API

        let mut transcript = Transcript::new();
        transcript.metadata.language = self.config.language.clone();

        // Example transcription result
        transcript.add_entry(TranscriptEntry::new(
            0,
            2000,
            "Example transcription result.".to_string(),
        ));

        Ok(transcript)
    }

    /// Transcribe with real-time streaming.
    pub fn transcribe_stream(&self, _audio_chunks: &[AudioBuffer]) -> AccessResult<Transcript> {
        // Streaming transcription for real-time use
        self.transcribe(&AudioBuffer::Interleaved(Bytes::new()))
    }

    /// Get configuration.
    #[must_use]
    pub const fn config(&self) -> &SttConfig {
        &self.config
    }
}

impl Default for SpeechToText {
    fn default() -> Self {
        Self::new(SttConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stt_creation() {
        let stt = SpeechToText::default();
        assert_eq!(stt.config().language, "en");
    }

    #[test]
    fn test_transcribe() {
        let stt = SpeechToText::default();
        let bytes: Vec<u8> = vec![0u8; 48000 * 4];
        let audio = AudioBuffer::Interleaved(Bytes::from(bytes));
        let result = stt.transcribe(&audio);
        assert!(result.is_ok());
    }
}
