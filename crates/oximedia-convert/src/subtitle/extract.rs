// Copyright 2025 OxiMedia Contributors
// Licensed under the Apache License, Version 2.0

//! Subtitle extraction from media files.

use crate::{ConversionError, Result};
use std::path::Path;

/// Extractor for subtitle streams from media files.
#[derive(Debug, Clone)]
pub struct SubtitleExtractor {
    convert_format: Option<super::convert::SubtitleFormat>,
}

impl SubtitleExtractor {
    /// Create a new subtitle extractor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            convert_format: None,
        }
    }

    /// Set the output format for extracted subtitles.
    #[must_use]
    pub fn with_format(mut self, format: super::convert::SubtitleFormat) -> Self {
        self.convert_format = Some(format);
        self
    }

    /// Extract subtitle stream from a media file.
    pub async fn extract<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        input: P,
        output: Q,
        stream_index: usize,
    ) -> Result<()> {
        let _input = input.as_ref();
        let _output = output.as_ref();
        let _index = stream_index;

        // Placeholder for actual extraction
        // In a real implementation, this would use oximedia-subtitle
        Ok(())
    }

    /// Extract all subtitle streams from a media file.
    pub async fn extract_all<P: AsRef<Path>>(
        &self,
        input: P,
        output_dir: P,
    ) -> Result<Vec<SubtitleStreamInfo>> {
        let _input = input.as_ref();
        let _output_dir = output_dir.as_ref();

        // Placeholder for actual extraction
        // In a real implementation, this would detect and extract all subtitle streams
        Ok(Vec::new())
    }

    /// List subtitle streams in a media file.
    pub fn list_streams<P: AsRef<Path>>(&self, input: P) -> Result<Vec<SubtitleStreamInfo>> {
        let _input = input.as_ref();

        // Placeholder for actual stream detection
        // In a real implementation, this would use oximedia-core
        Ok(Vec::new())
    }

    /// Extract subtitle by language code.
    pub async fn extract_by_language<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        input: P,
        output: Q,
        language: &str,
    ) -> Result<()> {
        let streams = self.list_streams(&input)?;

        let stream = streams
            .iter()
            .find(|s| s.language.as_deref() == Some(language))
            .ok_or_else(|| {
                ConversionError::InvalidInput(format!(
                    "No subtitle stream found for language: {language}"
                ))
            })?;

        self.extract(input, output, stream.index).await
    }

    /// Extract the first subtitle stream.
    pub async fn extract_first<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        input: P,
        output: Q,
    ) -> Result<()> {
        self.extract(input, output, 0).await
    }
}

impl Default for SubtitleExtractor {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a subtitle stream.
#[derive(Debug, Clone)]
pub struct SubtitleStreamInfo {
    /// Stream index
    pub index: usize,
    /// Codec name
    pub codec: String,
    /// Language code (e.g., "eng", "spa")
    pub language: Option<String>,
    /// Stream title/description
    pub title: Option<String>,
    /// Whether this is the default stream
    pub is_default: bool,
    /// Whether this is a forced subtitle
    pub is_forced: bool,
}

impl SubtitleStreamInfo {
    /// Create a new subtitle stream info.
    #[must_use]
    pub fn new(index: usize, codec: String) -> Self {
        Self {
            index,
            codec,
            language: None,
            title: None,
            is_default: false,
            is_forced: false,
        }
    }

    /// Set the language.
    pub fn with_language<S: Into<String>>(mut self, language: S) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Set the title.
    pub fn with_title<S: Into<String>>(mut self, title: S) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set whether this is the default stream.
    #[must_use]
    pub fn with_default(mut self, is_default: bool) -> Self {
        self.is_default = is_default;
        self
    }

    /// Set whether this is a forced subtitle.
    #[must_use]
    pub fn with_forced(mut self, is_forced: bool) -> Self {
        self.is_forced = is_forced;
        self
    }

    /// Get a description of this stream.
    #[must_use]
    pub fn description(&self) -> String {
        let mut parts = vec![format!("Stream {}", self.index)];

        if let Some(lang) = &self.language {
            parts.push(format!("Language: {lang}"));
        }

        if let Some(title) = &self.title {
            parts.push(format!("\"{title}\""));
        }

        if self.is_default {
            parts.push("(default)".to_string());
        }

        if self.is_forced {
            parts.push("(forced)".to_string());
        }

        parts.join(" - ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extractor_creation() {
        let extractor = SubtitleExtractor::new();
        assert!(extractor.convert_format.is_none());
    }

    #[test]
    fn test_extractor_with_format() {
        let extractor =
            SubtitleExtractor::new().with_format(super::super::convert::SubtitleFormat::Srt);

        assert!(extractor.convert_format.is_some());
    }

    #[test]
    fn test_stream_info_creation() {
        let info = SubtitleStreamInfo::new(0, "srt".to_string());
        assert_eq!(info.index, 0);
        assert_eq!(info.codec, "srt");
    }

    #[test]
    fn test_stream_info_builder() {
        let info = SubtitleStreamInfo::new(0, "srt".to_string())
            .with_language("eng")
            .with_title("English")
            .with_default(true)
            .with_forced(false);

        assert_eq!(info.language, Some("eng".to_string()));
        assert_eq!(info.title, Some("English".to_string()));
        assert!(info.is_default);
        assert!(!info.is_forced);
    }

    #[test]
    fn test_stream_description() {
        let info = SubtitleStreamInfo::new(0, "srt".to_string())
            .with_language("eng")
            .with_title("English")
            .with_default(true);

        let desc = info.description();
        assert!(desc.contains("Stream 0"));
        assert!(desc.contains("eng"));
        assert!(desc.contains("English"));
        assert!(desc.contains("default"));
    }
}
