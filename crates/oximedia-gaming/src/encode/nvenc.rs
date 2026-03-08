//! NVIDIA NVENC hardware encoder.
//!
//! Provides NVIDIA GPU hardware-accelerated encoding.

use crate::{GamingError, GamingResult};

/// NVIDIA NVENC encoder.
#[allow(dead_code)]
pub struct NvencEncoder {
    preset: NvencPreset,
    available: bool,
}

/// NVENC encoding presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NvencPreset {
    /// P1 - Fastest, lowest quality
    P1,
    /// P2 - Very fast
    P2,
    /// P3 - Fast
    P3,
    /// P4 - Medium (balanced)
    P4,
    /// P5 - Slow (high quality)
    P5,
    /// P6 - Slower
    P6,
    /// P7 - Slowest, highest quality
    P7,
}

/// NVENC capabilities.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct NvencCapabilities {
    /// GPU name
    pub gpu_name: String,
    /// Maximum resolution width
    pub max_width: u32,
    /// Maximum resolution height
    pub max_height: u32,
    /// Maximum framerate
    pub max_framerate: u32,
    /// Supports AV1 encoding
    pub supports_av1: bool,
    /// Supports VP9 encoding
    pub supports_vp9: bool,
    /// Supports hardware B-frame encoding
    pub supports_b_frames: bool,
}

impl NvencEncoder {
    /// Create a new NVENC encoder.
    ///
    /// # Errors
    ///
    /// Returns error if NVENC is not available.
    pub fn new(preset: NvencPreset) -> GamingResult<Self> {
        let available = Self::is_available();

        if !available {
            return Err(GamingError::HardwareAccelNotAvailable(
                "NVENC not available on this system".to_string(),
            ));
        }

        Ok(Self { preset, available })
    }

    /// Check if NVENC is available on this system.
    #[must_use]
    pub fn is_available() -> bool {
        // In a real implementation, this would check for NVIDIA GPU and driver
        false
    }

    /// Get NVENC capabilities.
    ///
    /// # Errors
    ///
    /// Returns error if capabilities cannot be queried.
    pub fn get_capabilities() -> GamingResult<NvencCapabilities> {
        if !Self::is_available() {
            return Err(GamingError::HardwareAccelNotAvailable(
                "NVENC not available".to_string(),
            ));
        }

        Ok(NvencCapabilities {
            gpu_name: "Unknown NVIDIA GPU".to_string(),
            max_width: 8192,
            max_height: 8192,
            max_framerate: 240,
            supports_av1: true,
            supports_vp9: true,
            supports_b_frames: true,
        })
    }

    /// Get recommended preset for game streaming.
    #[must_use]
    pub fn recommended_preset_for_latency(target_latency_ms: u32) -> NvencPreset {
        if target_latency_ms < 50 {
            NvencPreset::P1 // Ultra-low latency
        } else if target_latency_ms < 100 {
            NvencPreset::P2 // Low latency
        } else {
            NvencPreset::P3 // Balanced
        }
    }

    /// Get current preset.
    #[must_use]
    pub fn preset(&self) -> NvencPreset {
        self.preset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nvenc_availability() {
        // In test environment, NVENC is typically not available
        assert!(!NvencEncoder::is_available());
    }

    #[test]
    fn test_nvenc_creation_fails_when_unavailable() {
        if !NvencEncoder::is_available() {
            let result = NvencEncoder::new(NvencPreset::P3);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_recommended_preset() {
        assert_eq!(
            NvencEncoder::recommended_preset_for_latency(30),
            NvencPreset::P1
        );
        assert_eq!(
            NvencEncoder::recommended_preset_for_latency(80),
            NvencPreset::P2
        );
        assert_eq!(
            NvencEncoder::recommended_preset_for_latency(150),
            NvencPreset::P3
        );
    }
}
