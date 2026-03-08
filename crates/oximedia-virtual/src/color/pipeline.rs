//! Color pipeline management
//!
//! Manages the complete color pipeline from camera input to LED output.

use super::{lut::LutProcessor, match_color::ColorMatcher};
use crate::Result;
use serde::{Deserialize, Serialize};

/// Color pipeline configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorPipelineConfig {
    /// Enable color matching
    pub color_matching: bool,
    /// Enable LUT application
    pub lut_enabled: bool,
    /// Input color space
    pub input_color_space: String,
    /// Output color space
    pub output_color_space: String,
}

impl Default for ColorPipelineConfig {
    fn default() -> Self {
        Self {
            color_matching: true,
            lut_enabled: false,
            input_color_space: "Rec709".to_string(),
            output_color_space: "Rec709".to_string(),
        }
    }
}

/// Color pipeline
pub struct ColorPipeline {
    config: ColorPipelineConfig,
    color_matcher: Option<ColorMatcher>,
    lut_processor: Option<LutProcessor>,
}

impl ColorPipeline {
    /// Create new color pipeline
    pub fn new(config: ColorPipelineConfig) -> Result<Self> {
        let color_matcher = if config.color_matching {
            Some(ColorMatcher::new()?)
        } else {
            None
        };

        let lut_processor = if config.lut_enabled {
            Some(LutProcessor::new()?)
        } else {
            None
        };

        Ok(Self {
            config,
            color_matcher,
            lut_processor,
        })
    }

    /// Process frame through color pipeline
    pub fn process(&mut self, frame: &[u8], width: usize, height: usize) -> Result<Vec<u8>> {
        let mut output = frame.to_vec();

        // Apply color matching
        if let Some(matcher) = &mut self.color_matcher {
            output = matcher.process(&output, width, height)?;
        }

        // Apply LUT
        if let Some(lut) = &mut self.lut_processor {
            output = lut.apply(&output, width, height)?;
        }

        Ok(output)
    }

    /// Get configuration
    #[must_use]
    pub fn config(&self) -> &ColorPipelineConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_pipeline() {
        let config = ColorPipelineConfig::default();
        let pipeline = ColorPipeline::new(config);
        assert!(pipeline.is_ok());
    }
}
