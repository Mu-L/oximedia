// Copyright 2025 OxiMedia Contributors
// Licensed under the Apache License, Version 2.0

//! Thumbnail generation from video files.

use crate::Result;
use std::path::Path;

/// Generator for video thumbnails.
#[derive(Debug, Clone)]
pub struct ThumbnailGenerator {
    width: u32,
    height: u32,
    time_position: ThumbnailPosition,
    format: super::super::frame::extract::ImageFormat,
}

impl ThumbnailGenerator {
    /// Create a new thumbnail generator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            width: 320,
            height: 180,
            time_position: ThumbnailPosition::Middle,
            format: super::super::frame::extract::ImageFormat::Jpeg,
        }
    }

    /// Set the thumbnail width.
    #[must_use]
    pub fn with_width(mut self, width: u32) -> Self {
        self.width = width;
        self
    }

    /// Set the thumbnail height.
    #[must_use]
    pub fn with_height(mut self, height: u32) -> Self {
        self.height = height;
        self
    }

    /// Set the thumbnail size.
    #[must_use]
    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Set the time position for extraction.
    #[must_use]
    pub fn with_position(mut self, position: ThumbnailPosition) -> Self {
        self.time_position = position;
        self
    }

    /// Set the output format.
    #[must_use]
    pub fn with_format(mut self, format: super::super::frame::extract::ImageFormat) -> Self {
        self.format = format;
        self
    }

    /// Generate a thumbnail from a video.
    pub async fn generate<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        input: P,
        output: Q,
    ) -> Result<()> {
        let _input = input.as_ref();
        let _output = output.as_ref();

        // Placeholder for actual generation
        Ok(())
    }

    /// Generate multiple thumbnails at different positions.
    pub async fn generate_multiple<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        input: P,
        output_dir: Q,
        count: usize,
    ) -> Result<Vec<std::path::PathBuf>> {
        let _input = input.as_ref();
        let _output_dir = output_dir.as_ref();
        let _count = count;

        // Placeholder
        Ok(Vec::new())
    }

    /// Generate thumbnail at a specific time.
    pub async fn generate_at<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        input: P,
        output: Q,
        time_seconds: f64,
    ) -> Result<()> {
        let _input = input.as_ref();
        let _output = output.as_ref();
        let _time = time_seconds;

        // Placeholder
        Ok(())
    }

    /// Create a 16:9 thumbnail.
    #[must_use]
    pub fn widescreen() -> Self {
        Self::new().with_size(640, 360)
    }

    /// Create a 4:3 thumbnail.
    #[must_use]
    pub fn standard() -> Self {
        Self::new().with_size(640, 480)
    }

    /// Create a small thumbnail.
    #[must_use]
    pub fn small() -> Self {
        Self::new().with_size(160, 90)
    }

    /// Create a large thumbnail.
    #[must_use]
    pub fn large() -> Self {
        Self::new().with_size(1280, 720)
    }
}

impl Default for ThumbnailGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Position in the video for thumbnail extraction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThumbnailPosition {
    /// Start of the video
    Start,
    /// Middle of the video
    Middle,
    /// End of the video
    End,
    /// Specific time in seconds
    Time(f64),
    /// Percentage through the video (0.0 to 1.0)
    Percent(f64),
}

impl ThumbnailPosition {
    /// Calculate the actual time based on video duration.
    #[must_use]
    pub fn calculate_time(&self, duration_seconds: f64) -> f64 {
        match self {
            Self::Start => 0.0,
            Self::Middle => duration_seconds / 2.0,
            Self::End => duration_seconds.max(1.0) - 1.0,
            Self::Time(t) => *t,
            Self::Percent(p) => duration_seconds * p.clamp(0.0, 1.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generator_creation() {
        let gen = ThumbnailGenerator::new();
        assert_eq!(gen.width, 320);
        assert_eq!(gen.height, 180);
    }

    #[test]
    fn test_generator_size() {
        let gen = ThumbnailGenerator::new().with_size(640, 480);
        assert_eq!(gen.width, 640);
        assert_eq!(gen.height, 480);
    }

    #[test]
    fn test_presets() {
        let gen = ThumbnailGenerator::widescreen();
        assert_eq!(gen.width, 640);
        assert_eq!(gen.height, 360);

        let gen = ThumbnailGenerator::small();
        assert_eq!(gen.width, 160);
        assert_eq!(gen.height, 90);
    }

    #[test]
    fn test_position_calculation() {
        let duration = 100.0;

        assert_eq!(ThumbnailPosition::Start.calculate_time(duration), 0.0);
        assert_eq!(ThumbnailPosition::Middle.calculate_time(duration), 50.0);
        assert_eq!(ThumbnailPosition::End.calculate_time(duration), 99.0);
        assert_eq!(ThumbnailPosition::Time(25.0).calculate_time(duration), 25.0);
        assert_eq!(
            ThumbnailPosition::Percent(0.75).calculate_time(duration),
            75.0
        );
    }
}
