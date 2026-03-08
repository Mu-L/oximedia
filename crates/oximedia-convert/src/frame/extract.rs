// Copyright 2025 OxiMedia Contributors
// Licensed under the Apache License, Version 2.0

//! Frame extraction from video files.

use crate::Result;
use std::path::Path;

/// Extractor for video frames as images.
#[derive(Debug, Clone)]
pub struct FrameExtractor {
    format: ImageFormat,
    quality: u32,
}

impl FrameExtractor {
    /// Create a new frame extractor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            format: ImageFormat::Jpeg,
            quality: 90,
        }
    }

    /// Set the output image format.
    #[must_use]
    pub fn with_format(mut self, format: ImageFormat) -> Self {
        self.format = format;
        self
    }

    /// Set the output quality (1-100 for JPEG).
    #[must_use]
    pub fn with_quality(mut self, quality: u32) -> Self {
        self.quality = quality.min(100).max(1);
        self
    }

    /// Extract a single frame at a specific time.
    pub async fn extract_at<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        input: P,
        output: Q,
        time_seconds: f64,
    ) -> Result<()> {
        let _input = input.as_ref();
        let _output = output.as_ref();
        let _time = time_seconds;

        // Placeholder for actual extraction
        Ok(())
    }

    /// Extract frames at regular intervals.
    pub async fn extract_interval<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        input: P,
        output_dir: Q,
        interval_seconds: f64,
    ) -> Result<Vec<std::path::PathBuf>> {
        let _input = input.as_ref();
        let _output_dir = output_dir.as_ref();
        let _interval = interval_seconds;

        // Placeholder
        Ok(Vec::new())
    }

    /// Extract frames at specific times.
    pub async fn extract_multiple<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        input: P,
        output_dir: Q,
        times: &[f64],
    ) -> Result<Vec<std::path::PathBuf>> {
        let _input = input.as_ref();
        let _output_dir = output_dir.as_ref();
        let _times = times;

        // Placeholder
        Ok(Vec::new())
    }

    /// Extract all frames in a time range.
    pub async fn extract_range<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        input: P,
        output_dir: Q,
        range: &super::FrameRange,
    ) -> Result<Vec<std::path::PathBuf>> {
        let _input = input.as_ref();
        let _output_dir = output_dir.as_ref();
        let _range = range;

        // Placeholder
        Ok(Vec::new())
    }

    /// Extract as JPEG.
    #[must_use]
    pub fn as_jpeg(self) -> Self {
        self.with_format(ImageFormat::Jpeg)
    }

    /// Extract as PNG.
    #[must_use]
    pub fn as_png(self) -> Self {
        self.with_format(ImageFormat::Png)
    }

    /// Extract as WebP.
    #[must_use]
    pub fn as_webp(self) -> Self {
        self.with_format(ImageFormat::WebP)
    }
}

impl Default for FrameExtractor {
    fn default() -> Self {
        Self::new()
    }
}

/// Supported image formats for frame extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// JPEG
    Jpeg,
    /// PNG
    Png,
    /// WebP
    WebP,
    /// BMP
    Bmp,
}

impl ImageFormat {
    /// Get the file extension.
    #[must_use]
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::WebP => "webp",
            Self::Bmp => "bmp",
        }
    }

    /// Check if this format supports quality settings.
    #[must_use]
    pub fn supports_quality(&self) -> bool {
        matches!(self, Self::Jpeg | Self::WebP)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extractor_creation() {
        let extractor = FrameExtractor::new();
        assert_eq!(extractor.format, ImageFormat::Jpeg);
        assert_eq!(extractor.quality, 90);
    }

    #[test]
    fn test_extractor_settings() {
        let extractor = FrameExtractor::new()
            .with_format(ImageFormat::Png)
            .with_quality(100);

        assert_eq!(extractor.format, ImageFormat::Png);
        assert_eq!(extractor.quality, 100);
    }

    #[test]
    fn test_quality_clamping() {
        let extractor = FrameExtractor::new().with_quality(150);
        assert_eq!(extractor.quality, 100);

        let extractor = FrameExtractor::new().with_quality(0);
        assert_eq!(extractor.quality, 1);
    }

    #[test]
    fn test_format_extension() {
        assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::WebP.extension(), "webp");
    }

    #[test]
    fn test_format_quality_support() {
        assert!(ImageFormat::Jpeg.supports_quality());
        assert!(ImageFormat::WebP.supports_quality());
        assert!(!ImageFormat::Png.supports_quality());
        assert!(!ImageFormat::Bmp.supports_quality());
    }

    #[test]
    fn test_convenience_methods() {
        let extractor = FrameExtractor::new().as_png();
        assert_eq!(extractor.format, ImageFormat::Png);

        let extractor = FrameExtractor::new().as_webp();
        assert_eq!(extractor.format, ImageFormat::WebP);
    }
}
