//! Subtitle burn-in rendering configuration and job management.
//!
//! Provides configuration presets, position computation with safe-area enforcement,
//! and job descriptions for subtitle burn-in workflows.

/// Configuration for subtitle burn-in rendering.
#[derive(Debug, Clone)]
pub struct BurnInConfig {
    /// Font size in pixels.
    pub font_size: u32,
    /// Margin from frame edge in pixels.
    pub margin_px: u32,
    /// Whether to render a semi-transparent background box behind the text.
    pub background_box: bool,
    /// Background box opacity (0 = transparent, 255 = opaque).
    pub background_opacity: u8,
    /// Safe area as a percentage of the frame dimension (e.g. 0.05 = 5%).
    pub safe_area_pct: f32,
}

impl BurnInConfig {
    /// Broadcast-safe configuration: larger text, 10% safe area, background box.
    #[must_use]
    pub fn broadcast() -> Self {
        Self {
            font_size: 72,
            margin_px: 30,
            background_box: true,
            background_opacity: 180,
            safe_area_pct: 0.10,
        }
    }

    /// Web streaming configuration: medium text, 5% safe area, no background box.
    #[must_use]
    pub fn web() -> Self {
        Self {
            font_size: 48,
            margin_px: 20,
            background_box: false,
            background_opacity: 0,
            safe_area_pct: 0.05,
        }
    }
}

/// Alignment options for burn-in positioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BurnInAlignment {
    /// Top-left corner.
    TopLeft,
    /// Top center.
    TopCenter,
    /// Top-right corner.
    TopRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom center (most common for subtitles).
    BottomCenter,
    /// Bottom-right corner.
    BottomRight,
}

impl BurnInAlignment {
    /// Whether this alignment places content near the top of the frame.
    #[must_use]
    pub const fn is_top(&self) -> bool {
        matches!(self, Self::TopLeft | Self::TopCenter | Self::TopRight)
    }

    /// Whether this alignment places content near the left edge.
    #[must_use]
    pub const fn is_left(&self) -> bool {
        matches!(self, Self::TopLeft | Self::BottomLeft)
    }
}

/// Renderer that computes burn-in positions and validates safe areas.
#[derive(Debug, Clone)]
pub struct BurnInRenderer {
    /// The configuration used for this renderer.
    pub config: BurnInConfig,
}

impl BurnInRenderer {
    /// Create a new renderer with the given configuration.
    #[must_use]
    pub fn new(config: BurnInConfig) -> Self {
        Self { config }
    }

    /// Compute the pixel position (x, y) for a text block within a frame.
    ///
    /// - `text_w`, `text_h`: width and height of the rendered text block in pixels.
    /// - `frame_w`, `frame_h`: width and height of the video frame in pixels.
    /// - `align`: desired alignment.
    ///
    /// The position respects `margin_px` and `safe_area_pct`.
    #[must_use]
    pub fn compute_position(
        &self,
        text_w: u32,
        text_h: u32,
        frame_w: u32,
        frame_h: u32,
        align: &BurnInAlignment,
    ) -> (u32, u32) {
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        let safe_x = (frame_w as f32 * self.config.safe_area_pct) as u32;
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        let safe_y = (frame_h as f32 * self.config.safe_area_pct) as u32;

        let margin = self.config.margin_px;

        let x = match align {
            BurnInAlignment::TopLeft | BurnInAlignment::BottomLeft => safe_x + margin,
            BurnInAlignment::TopCenter | BurnInAlignment::BottomCenter => {
                let center = frame_w / 2;
                center.saturating_sub(text_w / 2)
            }
            BurnInAlignment::TopRight | BurnInAlignment::BottomRight => {
                frame_w.saturating_sub(text_w + safe_x + margin)
            }
        };

        let y = if align.is_top() {
            safe_y + margin
        } else {
            frame_h.saturating_sub(text_h + safe_y + margin)
        };

        (x, y)
    }

    /// Check whether a text block at (x, y) with size (w, h) lies within the
    /// safe area of the frame.
    ///
    /// Returns `true` if the block is fully within the safe area.
    #[must_use]
    pub fn validate_safe_area(
        &self,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        frame_w: u32,
        frame_h: u32,
    ) -> bool {
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        let safe_x = (frame_w as f32 * self.config.safe_area_pct) as u32;
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        let safe_y = (frame_h as f32 * self.config.safe_area_pct) as u32;

        x >= safe_x
            && y >= safe_y
            && (x + w) <= frame_w.saturating_sub(safe_x)
            && (y + h) <= frame_h.saturating_sub(safe_y)
    }
}

/// A burn-in job describing input/output paths and configuration.
#[derive(Debug, Clone)]
pub struct BurnInJob {
    /// Path to the subtitle file (SRT, VTT, etc.).
    pub subtitle_path: String,
    /// Path to the source video file.
    pub video_path: String,
    /// Path for the output video file.
    pub output_path: String,
    /// Burn-in rendering configuration.
    pub config: BurnInConfig,
}

impl BurnInJob {
    /// Create a new burn-in job.
    #[must_use]
    pub fn new(
        subtitle_path: impl Into<String>,
        video_path: impl Into<String>,
        output_path: impl Into<String>,
        config: BurnInConfig,
    ) -> Self {
        Self {
            subtitle_path: subtitle_path.into(),
            video_path: video_path.into(),
            output_path: output_path.into(),
            config,
        }
    }

    /// Estimated processing time in milliseconds for a video of the given duration.
    ///
    /// Uses a simple heuristic: 1.5× real-time for broadcast config, 1.0× for web.
    #[must_use]
    pub fn estimated_processing_ms(&self, duration_ms: u64) -> u64 {
        if self.config.background_box {
            // Broadcast-style: slightly more expensive
            duration_ms + duration_ms / 2
        } else {
            // Web-style: roughly real-time
            duration_ms
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broadcast_config_font_size() {
        let cfg = BurnInConfig::broadcast();
        assert_eq!(cfg.font_size, 72);
        assert!(cfg.background_box);
    }

    #[test]
    fn test_web_config_no_background() {
        let cfg = BurnInConfig::web();
        assert!(!cfg.background_box);
        assert_eq!(cfg.font_size, 48);
    }

    #[test]
    fn test_alignment_is_top() {
        assert!(BurnInAlignment::TopLeft.is_top());
        assert!(BurnInAlignment::TopCenter.is_top());
        assert!(!BurnInAlignment::BottomRight.is_top());
    }

    #[test]
    fn test_alignment_is_left() {
        assert!(BurnInAlignment::TopLeft.is_left());
        assert!(BurnInAlignment::BottomLeft.is_left());
        assert!(!BurnInAlignment::TopCenter.is_left());
        assert!(!BurnInAlignment::TopRight.is_left());
    }

    #[test]
    fn test_compute_position_bottom_center() {
        let renderer = BurnInRenderer::new(BurnInConfig::web());
        let (x, y) = renderer.compute_position(200, 50, 1920, 1080, &BurnInAlignment::BottomCenter);
        // x should be near center
        let expected_x = 1920 / 2 - 200 / 2;
        assert_eq!(x, expected_x);
        // y should be near the bottom
        assert!(y > 1080 / 2, "y={y} should be in the lower half");
    }

    #[test]
    fn test_compute_position_top_left() {
        let renderer = BurnInRenderer::new(BurnInConfig::web());
        let (x, y) = renderer.compute_position(100, 50, 1920, 1080, &BurnInAlignment::TopLeft);
        // Should be a small positive value
        assert!(x < 200, "x={x}");
        assert!(y < 200, "y={y}");
    }

    #[test]
    fn test_compute_position_bottom_right() {
        let renderer = BurnInRenderer::new(BurnInConfig::web());
        let (x, _y) = renderer.compute_position(200, 50, 1920, 1080, &BurnInAlignment::BottomRight);
        // Should be near the right side
        assert!(x > 1920 / 2, "x={x}");
    }

    #[test]
    fn test_validate_safe_area_inside() {
        let renderer = BurnInRenderer::new(BurnInConfig::web());
        // safe_area_pct = 0.05 → safe_x = 96, safe_y = 54 for 1920x1080
        let ok = renderer.validate_safe_area(100, 60, 200, 50, 1920, 1080);
        assert!(ok, "Should be inside safe area");
    }

    #[test]
    fn test_validate_safe_area_outside_left() {
        let renderer = BurnInRenderer::new(BurnInConfig::web());
        // x=0 is outside the 5% safe area
        let ok = renderer.validate_safe_area(0, 60, 200, 50, 1920, 1080);
        assert!(!ok, "x=0 should be outside safe area");
    }

    #[test]
    fn test_validate_safe_area_outside_right() {
        let renderer = BurnInRenderer::new(BurnInConfig::web());
        // x + w exceeds frame_w - safe_x
        let ok = renderer.validate_safe_area(1800, 60, 200, 50, 1920, 1080);
        assert!(!ok, "Right edge outside safe area");
    }

    #[test]
    fn test_burn_in_job_estimated_broadcast() {
        let job = BurnInJob::new("a.srt", "v.mp4", "out.mp4", BurnInConfig::broadcast());
        assert_eq!(job.estimated_processing_ms(10_000), 15_000);
    }

    #[test]
    fn test_burn_in_job_estimated_web() {
        let job = BurnInJob::new("a.srt", "v.mp4", "out.mp4", BurnInConfig::web());
        assert_eq!(job.estimated_processing_ms(10_000), 10_000);
    }

    #[test]
    fn test_burn_in_job_fields() {
        let job = BurnInJob::new("sub.srt", "video.mp4", "output.mp4", BurnInConfig::web());
        assert_eq!(job.subtitle_path, "sub.srt");
        assert_eq!(job.video_path, "video.mp4");
        assert_eq!(job.output_path, "output.mp4");
    }
}
