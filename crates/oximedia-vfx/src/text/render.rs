//! Text rendering with effects.

use crate::{Color, EffectParams, Frame, VfxResult, VideoEffect};

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
        // Use embedded font or load from system
        // For now, we'll use a simple placeholder
        let font_data = vec![]; // Placeholder - would load real font

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

    fn draw_text_simple(&self, frame: &mut Frame, x: i32, y: i32, color: Color) {
        // Simple text rendering without actual font
        // This is a placeholder implementation
        let char_width = self.config.font_size as i32 / 2;
        let char_height = self.config.font_size as i32;

        for (i, _ch) in self.config.text.chars().enumerate() {
            let char_x = x + i as i32 * char_width;

            // Draw simple rectangle for each character (placeholder)
            for dy in 0..char_height {
                for dx in 0..char_width - 2 {
                    let px = char_x + dx;
                    let py = y + dy;

                    if px >= 0 && px < frame.width as i32 && py >= 0 && py < frame.height as i32 {
                        frame.set_pixel(px as u32, py as u32, color.to_rgba());
                    }
                }
            }
        }
    }
}

impl VideoEffect for TextRenderer {
    fn name(&self) -> &'static str {
        "Text Renderer"
    }

    fn description(&self) -> &'static str {
        "High-quality text rendering"
    }

    fn apply(
        &mut self,
        input: &Frame,
        output: &mut Frame,
        _params: &EffectParams,
    ) -> VfxResult<()> {
        // Copy input to output
        for y in 0..output.height {
            for x in 0..output.width {
                output.set_pixel(x, y, input.get_pixel(x, y).unwrap_or([0, 0, 0, 0]));
            }
        }

        let x = (self.config.x * output.width as f32) as i32;
        let y = (self.config.y * output.height as f32) as i32;

        // Draw shadow if enabled
        if self.config.shadow_x != 0.0 || self.config.shadow_y != 0.0 {
            let shadow_x = x + self.config.shadow_x as i32;
            let shadow_y = y + self.config.shadow_y as i32;
            self.draw_text_simple(output, shadow_x, shadow_y, self.config.shadow_color);
        }

        // Draw outline if enabled
        if self.config.outline_width > 0.0 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx != 0 || dy != 0 {
                        self.draw_text_simple(output, x + dx, y + dy, self.config.outline_color);
                    }
                }
            }
        }

        // Draw main text
        self.draw_text_simple(output, x, y, self.config.color);

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
    fn test_text_renderer() {
        let config = TextConfig::new("Test").with_font_size(24.0);
        let mut renderer = TextRenderer::new(config).expect("should succeed in test");

        let input = Frame::new(200, 100).expect("should succeed in test");
        let mut output = Frame::new(200, 100).expect("should succeed in test");
        let params = EffectParams::new();
        renderer
            .apply(&input, &mut output, &params)
            .expect("should succeed in test");
    }
}
