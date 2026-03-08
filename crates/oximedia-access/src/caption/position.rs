//! Smart caption positioning.

use serde::{Deserialize, Serialize};

/// Caption position on screen.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum CaptionPosition {
    /// Bottom center (default).
    #[default]
    BottomCenter,
    /// Top center.
    TopCenter,
    /// Bottom left.
    BottomLeft,
    /// Bottom right.
    BottomRight,
    /// Custom position (x, y as percentage 0-100).
    Custom(f32, f32),
}

/// Smart caption positioner to avoid overlapping important content.
pub struct CaptionPositioner {
    default_position: CaptionPosition,
    avoid_bottom_percent: f32,
}

impl CaptionPositioner {
    /// Create a new positioner.
    #[must_use]
    pub const fn new(default_position: CaptionPosition) -> Self {
        Self {
            default_position,
            avoid_bottom_percent: 20.0,
        }
    }

    /// Set percentage of screen bottom to avoid.
    #[must_use]
    pub const fn with_avoid_bottom(mut self, percent: f32) -> Self {
        self.avoid_bottom_percent = percent;
        self
    }

    /// Calculate optimal position based on frame content.
    #[must_use]
    pub fn calculate_position(&self, _frame_height: u32) -> CaptionPosition {
        // In production, this would analyze the frame to:
        // - Detect faces and avoid covering them
        // - Detect on-screen text
        // - Detect important action areas
        // - Use saliency detection

        self.default_position
    }

    /// Get position coordinates as pixel offsets.
    #[must_use]
    pub fn get_coordinates(
        &self,
        position: &CaptionPosition,
        width: u32,
        height: u32,
    ) -> (i32, i32) {
        match position {
            CaptionPosition::BottomCenter => (width as i32 / 2, height as i32 - 100),
            CaptionPosition::TopCenter => (width as i32 / 2, 100),
            CaptionPosition::BottomLeft => (50, height as i32 - 100),
            CaptionPosition::BottomRight => (width as i32 - 50, height as i32 - 100),
            CaptionPosition::Custom(x, y) => (
                (width as f32 * x / 100.0) as i32,
                (height as f32 * y / 100.0) as i32,
            ),
        }
    }
}

impl Default for CaptionPositioner {
    fn default() -> Self {
        Self::new(CaptionPosition::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_position() {
        let pos = CaptionPosition::default();
        assert_eq!(pos, CaptionPosition::BottomCenter);
    }

    #[test]
    fn test_positioner() {
        let positioner = CaptionPositioner::default();
        let (x, y) = positioner.get_coordinates(&CaptionPosition::BottomCenter, 1920, 1080);
        assert_eq!(x, 960);
        assert_eq!(y, 980);
    }

    #[test]
    fn test_custom_position() {
        let positioner = CaptionPositioner::default();
        let (x, y) = positioner.get_coordinates(&CaptionPosition::Custom(50.0, 50.0), 1920, 1080);
        assert_eq!(x, 960);
        assert_eq!(y, 540);
    }
}
