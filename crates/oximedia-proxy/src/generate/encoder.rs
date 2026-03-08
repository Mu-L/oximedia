//! Proxy encoder implementation.

use super::settings::ProxyGenerationSettings;
use crate::{ProxyError, Result};
use std::path::Path;

/// Proxy encoder with support for various codecs and settings.
pub struct ProxyEncoder {
    settings: ProxyGenerationSettings,
}

impl ProxyEncoder {
    /// Create a new proxy encoder with the given settings.
    pub fn new(settings: ProxyGenerationSettings) -> Result<Self> {
        settings.validate()?;
        Ok(Self { settings })
    }

    /// Encode a proxy from the input file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Input file does not exist or cannot be read
    /// - Output path is invalid
    /// - Encoding fails
    pub async fn encode(&self, input: &Path, output: &Path) -> Result<ProxyEncodeResult> {
        // Validate input
        if !input.exists() {
            return Err(ProxyError::FileNotFound(input.display().to_string()));
        }

        // Create output directory if needed
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // For now, this is a placeholder implementation
        // In a real implementation, this would use oximedia-transcode
        // to perform the actual encoding with the specified settings

        tracing::info!(
            "Encoding proxy: {} -> {} (scale: {}, codec: {})",
            input.display(),
            output.display(),
            self.settings.scale_factor,
            self.settings.codec
        );

        Ok(ProxyEncodeResult {
            output_path: output.to_path_buf(),
            file_size: 0,
            duration: 0.0,
            bitrate: self.settings.bitrate,
            codec: self.settings.codec.clone(),
            resolution: (0, 0),
            encoding_time: 0.0,
        })
    }

    /// Get the current settings.
    #[must_use]
    pub fn settings(&self) -> &ProxyGenerationSettings {
        &self.settings
    }

    /// Update the settings.
    pub fn set_settings(&mut self, settings: ProxyGenerationSettings) -> Result<()> {
        settings.validate()?;
        self.settings = settings;
        Ok(())
    }

    /// Calculate the target resolution for the given input resolution.
    #[must_use]
    pub fn calculate_target_resolution(&self, input_width: u32, input_height: u32) -> (u32, u32) {
        let target_width = ((input_width as f32) * self.settings.scale_factor) as u32;
        let target_height = ((input_height as f32) * self.settings.scale_factor) as u32;

        // Ensure dimensions are even (required for most video codecs)
        let target_width = target_width & !1;
        let target_height = target_height & !1;

        (target_width, target_height)
    }
}

/// Result of proxy encoding operation.
#[derive(Debug, Clone)]
pub struct ProxyEncodeResult {
    /// Output file path.
    pub output_path: std::path::PathBuf,

    /// File size in bytes.
    pub file_size: u64,

    /// Duration in seconds.
    pub duration: f64,

    /// Actual bitrate in bits per second.
    pub bitrate: u64,

    /// Codec used.
    pub codec: String,

    /// Output resolution (width, height).
    pub resolution: (u32, u32),

    /// Encoding time in seconds.
    pub encoding_time: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_creation() {
        let settings = ProxyGenerationSettings::quarter_res_h264();
        let encoder = ProxyEncoder::new(settings);
        assert!(encoder.is_ok());
    }

    #[test]
    fn test_invalid_settings() {
        let mut settings = ProxyGenerationSettings::default();
        settings.scale_factor = 0.0;
        let encoder = ProxyEncoder::new(settings);
        assert!(encoder.is_err());
    }

    #[test]
    fn test_resolution_calculation() {
        let settings = ProxyGenerationSettings::quarter_res_h264();
        let encoder = ProxyEncoder::new(settings).expect("should succeed in test");

        let (width, height) = encoder.calculate_target_resolution(1920, 1080);
        assert_eq!(width, 480);
        assert_eq!(height, 270);

        let (width, height) = encoder.calculate_target_resolution(3840, 2160);
        assert_eq!(width, 960);
        assert_eq!(height, 540);
    }

    #[test]
    fn test_resolution_even_alignment() {
        let settings = ProxyGenerationSettings::default().with_scale_factor(0.3);
        let encoder = ProxyEncoder::new(settings).expect("should succeed in test");

        let (width, height) = encoder.calculate_target_resolution(1920, 1080);
        // Should be even numbers
        assert_eq!(width % 2, 0);
        assert_eq!(height % 2, 0);
    }
}
