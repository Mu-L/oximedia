//! Text rendering with effects.

use crate::{Color, EffectParams, Frame, VfxError, VfxResult, VideoEffect};

/// Text configuration.
#[derive(Debug, Clone)]
pub struct TextConfig {
    /// Text content.
    pub text: String,
    /// Font size.
    pub font_size: f32,
    /// Text color.
    pub color: Color,
    /// Position X (0.0 - 1.0).
    pub x: f32,
    /// Position Y (0.0 - 1.0).
    pub y: f32,
    /// Outline width.
    pub outline_width: f32,
    /// Outline color.
    pub outline_color: Color,
    /// Drop shadow offset X.
    pub shadow_x: f32,
    /// Drop shadow offset Y.
    pub shadow_y: f32,
    /// Shadow color.
    pub shadow_color: Color,
}

impl Default for TextConfig {
    fn default() -> Self {
        Self {
            text: String::new(),
            font_size: 48.0,
            color: Color::white(),
            x: 0.5,
            y: 0.5,
            outline_width: 0.0,
            outline_color: Color::black(),
            shadow_x: 0.0,
            shadow_y: 0.0,
            shadow_color: Color::black(),
        }
    }
}

impl TextConfig {
    /// Create a new text config.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    /// Set font size.
    #[must_use]
    pub const fn with_font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set text color.
    #[must_use]
    pub const fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Set position.
    #[must_use]
    pub fn with_position(mut self, x: f32, y: f32) -> Self {
        self.x = x.clamp(0.0, 1.0);
        self.y = y.clamp(0.0, 1.0);
        self
    }

    /// Set outline.
    #[must_use]
    pub const fn with_outline(mut self, width: f32, color: Color) -> Self {
        self.outline_width = width;
        self.outline_color = color;
        self
    }

    /// Set shadow.
    #[must_use]
    pub const fn with_shadow(mut self, offset_x: f32, offset_y: f32, color: Color) -> Self {
        self.shadow_x = offset_x;
        self.shadow_y = offset_y;
        self.shadow_color = color;
        self
    }
}

/// Text renderer.
///
/// # Font rasterization status
///
/// No font rasterizer is wired up yet (`font_data` is always empty). Real
/// glyph rasterization is a large, dependency-sensitive feature (a
/// pure-Rust font engine is required per COOLJAPAN policy) and is not yet
/// implemented. Rather than approximating glyphs with solid-color
/// rectangles and reporting success, [`apply`](VideoEffect::apply) returns
/// an honest `Err` for non-empty text. See `TODO(0.2.x)` there.
pub struct TextRenderer {
    config: TextConfig,
    font_data: Vec<u8>,
}

impl TextRenderer {
    /// Create a new text renderer.
    ///
    /// # Errors
    ///
    /// Returns an error if font loading fails.
    pub fn new(config: TextConfig) -> VfxResult<Self> {
        // TODO(0.2.x): load an embedded or system font via a pure-Rust
        // rasterizer. Until then `font_data` stays empty and `apply`
        // reports an honest error for non-empty text instead of
        // fabricating glyphs (see `VideoEffect::apply` below).
        let font_data = vec![];

        Ok(Self { config, font_data })
    }

    /// Set text content.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.config.text = text.into();
    }

    /// Get reference to config.
    #[must_use]
    pub fn config(&self) -> &TextConfig {
        &self.config
    }

    /// Get mutable reference to config.
    #[must_use]
    pub fn config_mut(&mut self) -> &mut TextConfig {
        &mut self.config
    }
}

impl VideoEffect for TextRenderer {
    fn name(&self) -> &'static str {
        "Text Renderer"
    }

    fn description(&self) -> &'static str {
        "Text rendering (font rasterizer not yet implemented; see TODO(0.2.x))"
    }

    fn apply(
        &mut self,
        input: &Frame,
        output: &mut Frame,
        _params: &EffectParams,
    ) -> VfxResult<()> {
        if !self.config.text.is_empty() {
            // TODO(0.2.x): real glyph rasterization (pure-Rust font engine).
            //
            // This previously drew solid-color rectangles per character and
            // reported `Ok(())`, silently fabricating "rendered text" that
            // was never actual glyphs (`font_data` is always empty — see
            // `TextRenderer::new`). Until a real pure-Rust rasterizer is
            // integrated, report this explicitly rather than lying about
            // the result.
            return Err(VfxError::TextRenderError(
                "text rendering requires a font rasterizer — not yet implemented; \
                 see TODO(0.2.x)"
                    .to_string(),
            ));
        }

        // No text configured: a faithful identity pass-through, not a
        // fabricated rendering result — there is nothing to rasterize.
        for y in 0..output.height {
            for x in 0..output.width {
                output.set_pixel(x, y, input.get_pixel(x, y).unwrap_or([0, 0, 0, 0]));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_config() {
        let config = TextConfig::new("Hello")
            .with_font_size(36.0)
            .with_color(Color::rgb(255, 0, 0))
            .with_position(0.5, 0.5);

        assert_eq!(config.text, "Hello");
        assert_eq!(config.font_size, 36.0);
    }

    #[test]
    fn test_text_renderer_nonempty_text_returns_honest_err_not_fake_rectangles() {
        // With no font rasterizer wired up, non-empty text must not be
        // silently "rendered" as solid-color rectangles; it must report an
        // honest error instead of fabricating success.
        let config = TextConfig::new("Test").with_font_size(24.0);
        let mut renderer = TextRenderer::new(config).expect("should succeed in test");

        let input = Frame::new(200, 100).expect("should succeed in test");
        let mut output = Frame::new(200, 100).expect("should succeed in test");
        let params = EffectParams::new();
        let err = renderer
            .apply(&input, &mut output, &params)
            .expect_err("text rendering without a font rasterizer must fail honestly");

        assert!(
            matches!(err, VfxError::TextRenderError(_)),
            "expected VfxError::TextRenderError, got {err:?}"
        );
    }

    #[test]
    fn test_text_renderer_empty_text_is_identity_passthrough() {
        // Empty text has nothing to rasterize, so it is honestly a no-op:
        // output must exactly equal input, and this must succeed.
        let config = TextConfig::new("");
        let mut renderer = TextRenderer::new(config).expect("should succeed in test");

        let input = Frame::new(8, 6).expect("should succeed in test");
        let mut output = Frame::new(8, 6).expect("should succeed in test");
        let params = EffectParams::new();
        renderer
            .apply(&input, &mut output, &params)
            .expect("empty text must be a harmless identity pass-through");

        for y in 0..output.height {
            for x in 0..output.width {
                assert_eq!(
                    output.get_pixel(x, y).unwrap_or([0, 0, 0, 0]),
                    input.get_pixel(x, y).unwrap_or([0, 0, 0, 0]),
                    "identity pass-through must match input exactly at ({x},{y})"
                );
            }
        }
    }
}
