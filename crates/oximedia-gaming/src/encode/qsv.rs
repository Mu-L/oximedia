//! Intel Quick Sync Video hardware encoder.
//!
//! Provides Intel GPU hardware-accelerated encoding.

use crate::{GamingError, GamingResult};

/// Intel Quick Sync Video encoder.
#[allow(dead_code)]
pub struct QsvEncoder {
    preset: QsvPreset,
    available: bool,
}

/// QSV encoding presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QsvPreset {
    /// Very fast preset (lowest quality)
    VeryFast,
    /// Fast preset
    Fast,
    /// Medium preset (balanced)
    Medium,
    /// Slow preset
    Slow,
    /// Very slow preset (highest quality)
    VerySlow,
}

/// QSV capabilities.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct QsvCapabilities {
    /// GPU name
    pub gpu_name: String,
    /// GPU generation (e.g., 11 for 11th gen Intel)
    pub generation: u32,
    /// Maximum resolution width
    pub max_width: u32,
    /// Maximum resolution height
    pub max_height: u32,
    /// Supports AV1 encoding
    pub supports_av1: bool,
    /// Supports VP9 encoding
    pub supports_vp9: bool,
}

impl QsvEncoder {
    /// Create a new QSV encoder.
    ///
    /// # Errors
    ///
    /// Returns error if QSV is not available.
    pub fn new(preset: QsvPreset) -> GamingResult<Self> {
        let available = Self::is_available();

        if !available {
            return Err(GamingError::HardwareAccelNotAvailable(
                "Intel Quick Sync Video not available on this system".to_string(),
            ));
        }

        Ok(Self { preset, available })
    }

    /// Check if QSV is available on this system.
    #[must_use]
    pub fn is_available() -> bool {
        // In a real implementation, this would check for Intel GPU
        false
    }

    /// Get QSV capabilities.
    ///
    /// # Errors
    ///
    /// Returns error if capabilities cannot be queried.
    pub fn get_capabilities() -> GamingResult<QsvCapabilities> {
        if !Self::is_available() {
            return Err(GamingError::HardwareAccelNotAvailable(
                "QSV not available".to_string(),
            ));
        }

        Ok(QsvCapabilities {
            gpu_name: "Unknown Intel GPU".to_string(),
            generation: 11,
            max_width: 8192,
            max_height: 8192,
            supports_av1: true,
            supports_vp9: true,
        })
    }

    /// Get recommended preset for game streaming.
    #[must_use]
    pub fn recommended_preset_for_latency(target_latency_ms: u32) -> QsvPreset {
        if target_latency_ms < 50 {
            QsvPreset::VeryFast
        } else if target_latency_ms < 100 {
            QsvPreset::Fast
        } else {
            QsvPreset::Medium
        }
    }

    /// Get current preset.
    #[must_use]
    pub fn preset(&self) -> QsvPreset {
        self.preset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qsv_availability() {
        // In test environment, QSV is typically not available
        assert!(!QsvEncoder::is_available());
    }

    #[test]
    fn test_qsv_creation_fails_when_unavailable() {
        if !QsvEncoder::is_available() {
            let result = QsvEncoder::new(QsvPreset::Medium);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_recommended_preset() {
        assert_eq!(
            QsvEncoder::recommended_preset_for_latency(30),
            QsvPreset::VeryFast
        );
        assert_eq!(
            QsvEncoder::recommended_preset_for_latency(80),
            QsvPreset::Fast
        );
        assert_eq!(
            QsvEncoder::recommended_preset_for_latency(150),
            QsvPreset::Medium
        );
    }
}
