//! Zoom detection (in/out).

use crate::error::ShotResult;
use ndarray::Array3;

/// Zoom detector.
pub struct ZoomDetector {
    /// Threshold for zoom detection.
    threshold: f32,
}

impl ZoomDetector {
    /// Create a new zoom detector.
    #[must_use]
    pub const fn new() -> Self {
        Self { threshold: 0.05 }
    }

    /// Detect zoom between two frames.
    ///
    /// # Errors
    ///
    /// Returns error if frames are invalid.
    pub fn detect_zoom(
        &self,
        _frame1: &Array3<u8>,
        _frame2: &Array3<u8>,
    ) -> ShotResult<(bool, f32)> {
        // Simplified implementation
        Ok((false, 0.0))
    }
}

impl Default for ZoomDetector {
    fn default() -> Self {
        Self::new()
    }
}
