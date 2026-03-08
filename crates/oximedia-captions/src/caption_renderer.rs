//! Caption rendering configuration and target selection.
//!
//! Provides abstractions for rendering captions to different output surfaces,
//! configuring font metrics, safe-area margins, and background styling.

#![allow(dead_code)]

/// Output surface to which captions are rendered.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RenderTarget {
    /// Software RGBA frame buffer at the given dimensions.
    FrameBuffer {
        /// Frame width in pixels.
        width: u32,
        /// Frame height in pixels.
        height: u32,
    },
    /// Burn-in directly onto an encoded video stream.
    VideoBurnIn,
    /// HTML/CSS overlay for web players.
    WebOverlay,
    /// Native platform accessibility layer (e.g. macOS `VoiceOver`).
    AccessibilityLayer,
    /// Dedicated sidecar subtitle stream (not burned in).
    SidecarStream {
        /// MIME type of the sidecar format (e.g. `"text/vtt"`).
        mime_type: String,
    },
}

impl RenderTarget {
    /// Returns `true` when captions are composited onto the image.
    #[must_use]
    pub fn is_burned_in(&self) -> bool {
        matches!(self, Self::FrameBuffer { .. } | Self::VideoBurnIn)
    }

    /// Returns `true` when the target produces a separate stream.
    #[must_use]
    pub fn is_sidecar(&self) -> bool {
        matches!(self, Self::SidecarStream { .. } | Self::WebOverlay)
    }
}

/// RGBA colour with components in `[0, 255]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbaColor {
    /// Red component.
    pub r: u8,
    /// Green component.
    pub g: u8,
    /// Blue component.
    pub b: u8,
    /// Alpha component (`0` = transparent, `255` = opaque).
    pub a: u8,
}

impl RgbaColor {
    /// Opaque white.
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    /// Semi-transparent black (typical caption background).
    pub const CAPTION_BG: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 192,
    };

    /// Create a new colour.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Returns `true` when the colour is fully transparent.
    #[must_use]
    pub fn is_transparent(self) -> bool {
        self.a == 0
    }
}

/// Safe-area inset expressed as a fraction of the frame dimension (`0.0`–`1.0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SafeAreaInsets {
    /// Left inset as a fraction.
    pub left: f32,
    /// Right inset as a fraction.
    pub right: f32,
    /// Top inset as a fraction.
    pub top: f32,
    /// Bottom inset as a fraction.
    pub bottom: f32,
}

impl SafeAreaInsets {
    /// Standard 10% EBU/SMPTE safe area.
    #[must_use]
    pub fn standard() -> Self {
        Self {
            left: 0.1,
            right: 0.1,
            top: 0.1,
            bottom: 0.1,
        }
    }

    /// No insets (fill the full frame).
    #[must_use]
    pub fn none() -> Self {
        Self {
            left: 0.0,
            right: 0.0,
            top: 0.0,
            bottom: 0.0,
        }
    }
}

impl Default for SafeAreaInsets {
    fn default() -> Self {
        Self::standard()
    }
}

/// Configuration for the caption renderer.
#[derive(Debug, Clone)]
pub struct CaptionRenderConfig {
    /// Target output surface.
    pub target: RenderTarget,
    /// Font size in points.
    pub font_size_pt: f32,
    /// Font family name.
    pub font_family: String,
    /// Default text colour.
    pub text_color: RgbaColor,
    /// Background box colour (`TRANSPARENT` to disable).
    pub background_color: RgbaColor,
    /// Safe-area insets applied to caption positioning.
    pub safe_area: SafeAreaInsets,
    /// Whether to enable drop-shadow for readability.
    pub drop_shadow: bool,
    /// Maximum number of caption rows displayed simultaneously.
    pub max_rows: u8,
}

impl Default for CaptionRenderConfig {
    fn default() -> Self {
        Self {
            target: RenderTarget::FrameBuffer {
                width: 1920,
                height: 1080,
            },
            font_size_pt: 36.0,
            font_family: "Arial".to_string(),
            text_color: RgbaColor::WHITE,
            background_color: RgbaColor::CAPTION_BG,
            safe_area: SafeAreaInsets::standard(),
            drop_shadow: true,
            max_rows: 3,
        }
    }
}

/// A rendered caption item ready for compositing.
#[derive(Debug, Clone)]
pub struct RenderedCaption {
    /// The caption text after layout.
    pub text: String,
    /// Normalised x position within the safe area (`0.0`–`1.0`).
    pub x: f32,
    /// Normalised y position within the safe area (`0.0`–`1.0`).
    pub y: f32,
    /// Text colour used during rendering.
    pub color: RgbaColor,
}

/// Prepares caption text for compositing given a render configuration.
///
/// In a production system this would invoke a font rasteriser; here it
/// performs the layout calculations needed to position captions.
#[derive(Debug)]
pub struct CaptionRenderer {
    config: CaptionRenderConfig,
}

impl CaptionRenderer {
    /// Create a new renderer with the given configuration.
    #[must_use]
    pub fn new(config: CaptionRenderConfig) -> Self {
        Self { config }
    }

    /// Access the current render configuration.
    #[must_use]
    pub fn config(&self) -> &CaptionRenderConfig {
        &self.config
    }

    /// Update the render configuration.
    pub fn set_config(&mut self, config: CaptionRenderConfig) {
        self.config = config;
    }

    /// Lay out a caption text string for rendering.
    ///
    /// Returns a `RenderedCaption` positioned at the bottom-centre of the
    /// safe area (the standard broadcast position).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn render(&self, text: &str, row: u8) -> RenderedCaption {
        // Centre horizontally; position from bottom of safe area.
        let row_offset = f32::from(row) * 0.08;
        let y = (1.0_f32 - self.config.safe_area.bottom - row_offset).clamp(0.0, 1.0);

        RenderedCaption {
            text: text.to_string(),
            x: 0.5,
            y,
            color: self.config.text_color,
        }
    }

    /// Render multiple lines, clamped to `max_rows`.
    #[must_use]
    pub fn render_lines(&self, lines: &[&str]) -> Vec<RenderedCaption> {
        lines
            .iter()
            .take(self.config.max_rows as usize)
            .enumerate()
            .map(|(i, line)| {
                #[allow(clippy::cast_possible_truncation)]
                self.render(line, i as u8)
            })
            .collect()
    }

    /// Returns `true` when the current target burns captions into the image.
    #[must_use]
    pub fn is_burned_in(&self) -> bool {
        self.config.target.is_burned_in()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_target_is_burned_in() {
        assert!(RenderTarget::FrameBuffer {
            width: 1920,
            height: 1080
        }
        .is_burned_in());
        assert!(RenderTarget::VideoBurnIn.is_burned_in());
        assert!(!RenderTarget::WebOverlay.is_burned_in());
    }

    #[test]
    fn test_render_target_is_sidecar() {
        assert!(RenderTarget::WebOverlay.is_sidecar());
        assert!(RenderTarget::SidecarStream {
            mime_type: "text/vtt".to_string()
        }
        .is_sidecar());
        assert!(!RenderTarget::VideoBurnIn.is_sidecar());
    }

    #[test]
    fn test_rgba_constants() {
        assert_eq!(RgbaColor::WHITE.a, 255);
        assert_eq!(RgbaColor::TRANSPARENT.a, 0);
        assert!(!RgbaColor::CAPTION_BG.is_transparent());
    }

    #[test]
    fn test_rgba_is_transparent() {
        assert!(RgbaColor::TRANSPARENT.is_transparent());
        assert!(!RgbaColor::WHITE.is_transparent());
    }

    #[test]
    fn test_safe_area_standard() {
        let sa = SafeAreaInsets::standard();
        assert!((sa.left - 0.1).abs() < 1e-6);
        assert!((sa.bottom - 0.1).abs() < 1e-6);
    }

    #[test]
    fn test_safe_area_none() {
        let sa = SafeAreaInsets::none();
        assert_eq!(sa.left, 0.0);
        assert_eq!(sa.top, 0.0);
    }

    #[test]
    fn test_default_config() {
        let cfg = CaptionRenderConfig::default();
        assert_eq!(cfg.max_rows, 3);
        assert!(cfg.drop_shadow);
    }

    #[test]
    fn test_renderer_render_row_0() {
        let renderer = CaptionRenderer::new(CaptionRenderConfig::default());
        let cap = renderer.render("Hello world", 0);
        assert_eq!(cap.text, "Hello world");
        // row 0: y = 1 - 0.1 - 0 = 0.9
        assert!((cap.y - 0.9).abs() < 1e-5, "y={}", cap.y);
        assert!((cap.x - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_renderer_render_row_1_lower() {
        let renderer = CaptionRenderer::new(CaptionRenderConfig::default());
        let row0 = renderer.render("line0", 0);
        let row1 = renderer.render("line1", 1);
        // row 1 should be higher on screen (smaller y)
        assert!(row1.y < row0.y);
    }

    #[test]
    fn test_render_lines_respects_max_rows() {
        let renderer = CaptionRenderer::new(CaptionRenderConfig::default());
        let lines = vec!["a", "b", "c", "d", "e"];
        let rendered = renderer.render_lines(&lines);
        assert_eq!(rendered.len(), 3); // max_rows = 3
    }

    #[test]
    fn test_render_lines_fewer_than_max() {
        let renderer = CaptionRenderer::new(CaptionRenderConfig::default());
        let rendered = renderer.render_lines(&["only"]);
        assert_eq!(rendered.len(), 1);
    }

    #[test]
    fn test_is_burned_in_true() {
        let renderer = CaptionRenderer::new(CaptionRenderConfig::default());
        assert!(renderer.is_burned_in());
    }

    #[test]
    fn test_is_burned_in_false_for_web() {
        let mut cfg = CaptionRenderConfig::default();
        cfg.target = RenderTarget::WebOverlay;
        let renderer = CaptionRenderer::new(cfg);
        assert!(!renderer.is_burned_in());
    }

    #[test]
    fn test_set_config() {
        let mut renderer = CaptionRenderer::new(CaptionRenderConfig::default());
        let mut new_cfg = CaptionRenderConfig::default();
        new_cfg.font_size_pt = 48.0;
        renderer.set_config(new_cfg);
        assert!((renderer.config().font_size_pt - 48.0).abs() < 1e-6);
    }
}
