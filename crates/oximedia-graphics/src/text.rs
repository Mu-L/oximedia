//! Text rendering engine with advanced typography

use crate::color::Color;
use crate::error::{GraphicsError, Result};
use crate::primitives::{Point, Rect};
use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Font family
#[derive(Debug, Clone)]
pub struct FontFamily {
    /// Regular font
    pub regular: Arc<Vec<u8>>,
    /// Bold font
    pub bold: Option<Arc<Vec<u8>>>,
    /// Italic font
    pub italic: Option<Arc<Vec<u8>>>,
    /// Bold italic font
    pub bold_italic: Option<Arc<Vec<u8>>>,
}

impl FontFamily {
    /// Create from regular font only
    #[must_use]
    pub fn from_regular(data: Vec<u8>) -> Self {
        Self {
            regular: Arc::new(data),
            bold: None,
            italic: None,
            bold_italic: None,
        }
    }

    /// Get font for style
    #[must_use]
    pub fn get_font(&self, style: FontStyle, weight: FontWeight) -> &[u8] {
        match (style, weight) {
            (FontStyle::Italic, FontWeight::Bold) => {
                if let Some(ref font) = self.bold_italic {
                    font
                } else if let Some(ref font) = self.bold {
                    font
                } else if let Some(ref font) = self.italic {
                    font
                } else {
                    &self.regular
                }
            }
            (FontStyle::Italic, _) => {
                if let Some(ref font) = self.italic {
                    font
                } else {
                    &self.regular
                }
            }
            (_, FontWeight::Bold) => {
                if let Some(ref font) = self.bold {
                    font
                } else {
                    &self.regular
                }
            }
            _ => &self.regular,
        }
    }
}

/// Font manager
#[derive(Debug, Clone)]
pub struct FontManager {
    families: HashMap<String, FontFamily>,
}

impl FontManager {
    /// Create a new font manager
    #[must_use]
    pub fn new() -> Self {
        Self {
            families: HashMap::new(),
        }
    }

    /// Add font family
    pub fn add_family(&mut self, name: String, family: FontFamily) {
        self.families.insert(name, family);
    }

    /// Get font family
    #[must_use]
    pub fn get_family(&self, name: &str) -> Option<&FontFamily> {
        self.families.get(name)
    }

    /// Load system font (simplified - would use platform-specific APIs)
    pub fn load_system_font(&mut self, name: &str) -> Result<()> {
        // This is a placeholder - real implementation would use fontconfig/DirectWrite/CoreText
        Err(GraphicsError::FontError(format!(
            "System font loading not implemented: {name}"
        )))
    }
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Font style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FontStyle {
    /// Normal style
    Normal,
    /// Italic style
    Italic,
}

/// Font weight
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FontWeight {
    /// Normal weight
    Normal,
    /// Bold weight
    Bold,
}

/// Text alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    /// Left aligned
    Left,
    /// Center aligned
    Center,
    /// Right aligned
    Right,
    /// Justified
    Justify,
}

/// Vertical text alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerticalAlign {
    /// Top aligned
    Top,
    /// Middle aligned
    Middle,
    /// Bottom aligned
    Bottom,
}

/// Text style
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextStyle {
    /// Font family name
    pub font_family: String,
    /// Font size in pixels
    pub font_size: f32,
    /// Font style
    pub font_style: FontStyle,
    /// Font weight
    pub font_weight: FontWeight,
    /// Text color
    pub color: Color,
    /// Line height multiplier
    pub line_height: f32,
    /// Letter spacing
    pub letter_spacing: f32,
    /// Text alignment
    pub align: TextAlign,
    /// Vertical alignment
    pub vertical_align: VerticalAlign,
}

impl TextStyle {
    /// Create a new text style
    #[must_use]
    pub fn new(font_family: String, font_size: f32, color: Color) -> Self {
        Self {
            font_family,
            font_size,
            font_style: FontStyle::Normal,
            font_weight: FontWeight::Normal,
            color,
            line_height: 1.2,
            letter_spacing: 0.0,
            align: TextAlign::Left,
            vertical_align: VerticalAlign::Top,
        }
    }

    /// Set font style
    #[must_use]
    pub fn with_style(mut self, style: FontStyle) -> Self {
        self.font_style = style;
        self
    }

    /// Set font weight
    #[must_use]
    pub fn with_weight(mut self, weight: FontWeight) -> Self {
        self.font_weight = weight;
        self
    }

    /// Set line height
    #[must_use]
    pub fn with_line_height(mut self, line_height: f32) -> Self {
        self.line_height = line_height;
        self
    }

    /// Set alignment
    #[must_use]
    pub fn with_align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self::new("Arial".to_string(), 16.0, Color::BLACK)
    }
}

/// Text shadow effect
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TextShadow {
    /// Offset X
    pub offset_x: f32,
    /// Offset Y
    pub offset_y: f32,
    /// Blur radius
    pub blur: f32,
    /// Shadow color
    pub color: Color,
}

impl TextShadow {
    /// Create a new text shadow
    #[must_use]
    pub fn new(offset_x: f32, offset_y: f32, blur: f32, color: Color) -> Self {
        Self {
            offset_x,
            offset_y,
            blur,
            color,
        }
    }
}

/// Text outline
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TextOutline {
    /// Outline width
    pub width: f32,
    /// Outline color
    pub color: Color,
}

impl TextOutline {
    /// Create a new text outline
    #[must_use]
    pub fn new(width: f32, color: Color) -> Self {
        Self { width, color }
    }
}

/// Rich text segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSegment {
    /// Text content
    pub text: String,
    /// Style for this segment
    pub style: TextStyle,
}

impl TextSegment {
    /// Create a new text segment
    #[must_use]
    pub fn new(text: String, style: TextStyle) -> Self {
        Self { text, style }
    }
}

/// Multi-line text layout
#[derive(Debug, Clone)]
pub struct TextLayout {
    /// Text segments
    pub segments: Vec<TextSegment>,
    /// Maximum width (for wrapping)
    pub max_width: Option<f32>,
    /// Text shadow
    pub shadow: Option<TextShadow>,
    /// Text outline
    pub outline: Option<TextOutline>,
}

impl TextLayout {
    /// Create a new text layout
    #[must_use]
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            max_width: None,
            shadow: None,
            outline: None,
        }
    }

    /// Add a text segment
    pub fn add_segment(&mut self, segment: TextSegment) -> &mut Self {
        self.segments.push(segment);
        self
    }

    /// Set maximum width
    #[must_use]
    pub fn with_max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Set shadow
    #[must_use]
    pub fn with_shadow(mut self, shadow: TextShadow) -> Self {
        self.shadow = Some(shadow);
        self
    }

    /// Set outline
    #[must_use]
    pub fn with_outline(mut self, outline: TextOutline) -> Self {
        self.outline = Some(outline);
        self
    }

    /// Measure text bounds
    pub fn measure(&self, font_manager: &FontManager) -> Result<Rect> {
        let mut max_width = 0.0_f32;
        let mut total_height = 0.0_f32;

        for segment in &self.segments {
            let family = font_manager
                .get_family(&segment.style.font_family)
                .ok_or_else(|| {
                    GraphicsError::FontError(format!(
                        "Font family not found: {}",
                        segment.style.font_family
                    ))
                })?;

            let font_data = family.get_font(segment.style.font_style, segment.style.font_weight);
            let font = FontRef::try_from_slice(font_data)
                .map_err(|e| GraphicsError::FontError(format!("Failed to parse font: {e}")))?;

            let scale = PxScale::from(segment.style.font_size);
            let scaled_font = font.as_scaled(scale);

            let mut line_width = 0.0_f32;
            let mut prev_glyph_id = None;

            for ch in segment.text.chars() {
                let glyph_id = scaled_font.glyph_id(ch);
                let advance_width = scaled_font.h_advance(glyph_id);

                if let Some(prev_id) = prev_glyph_id {
                    line_width += scaled_font.kern(prev_id, glyph_id);
                }

                line_width += advance_width + segment.style.letter_spacing;
                prev_glyph_id = Some(glyph_id);
            }

            max_width = max_width.max(line_width);
            total_height += segment.style.font_size * segment.style.line_height;
        }

        Ok(Rect::new(0.0, 0.0, max_width, total_height))
    }
}

impl Default for TextLayout {
    fn default() -> Self {
        Self::new()
    }
}

/// Glyph position
#[derive(Debug, Clone)]
pub struct GlyphPosition {
    /// Character
    pub ch: char,
    /// Position
    pub position: Point,
    /// Font size
    pub size: f32,
    /// Color
    pub color: Color,
}

/// Text renderer
pub struct TextRenderer {
    font_manager: FontManager,
}

impl TextRenderer {
    /// Create a new text renderer
    #[must_use]
    pub fn new(font_manager: FontManager) -> Self {
        Self { font_manager }
    }

    /// Layout text and get glyph positions
    pub fn layout_glyphs(
        &self,
        layout: &TextLayout,
        position: Point,
    ) -> Result<Vec<GlyphPosition>> {
        let mut glyphs = Vec::new();
        let mut y = position.y;

        for segment in &layout.segments {
            let family = self
                .font_manager
                .get_family(&segment.style.font_family)
                .ok_or_else(|| {
                    GraphicsError::FontError(format!(
                        "Font family not found: {}",
                        segment.style.font_family
                    ))
                })?;

            let font_data = family.get_font(segment.style.font_style, segment.style.font_weight);
            let font = FontRef::try_from_slice(font_data)
                .map_err(|e| GraphicsError::FontError(format!("Failed to parse font: {e}")))?;

            let scale = PxScale::from(segment.style.font_size);
            let scaled_font = font.as_scaled(scale);

            let mut x = position.x;
            let mut prev_glyph_id = None;

            for ch in segment.text.chars() {
                if ch == '\n' {
                    y += segment.style.font_size * segment.style.line_height;
                    x = position.x;
                    prev_glyph_id = None;
                    continue;
                }

                let glyph_id = scaled_font.glyph_id(ch);

                if let Some(prev_id) = prev_glyph_id {
                    x += scaled_font.kern(prev_id, glyph_id);
                }

                glyphs.push(GlyphPosition {
                    ch,
                    position: Point::new(x, y),
                    size: segment.style.font_size,
                    color: segment.style.color,
                });

                x += scaled_font.h_advance(glyph_id) + segment.style.letter_spacing;
                prev_glyph_id = Some(glyph_id);
            }

            y += segment.style.font_size * segment.style.line_height;
        }

        Ok(glyphs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_family() {
        let family = FontFamily::from_regular(vec![0; 100]);
        assert!(family.bold.is_none());
        assert!(family.italic.is_none());
    }

    #[test]
    fn test_font_manager() {
        let mut manager = FontManager::new();
        let family = FontFamily::from_regular(vec![0; 100]);
        manager.add_family("Test".to_string(), family);
        assert!(manager.get_family("Test").is_some());
        assert!(manager.get_family("NotFound").is_none());
    }

    #[test]
    fn test_text_style() {
        let style = TextStyle::new("Arial".to_string(), 16.0, Color::BLACK)
            .with_style(FontStyle::Italic)
            .with_weight(FontWeight::Bold)
            .with_line_height(1.5)
            .with_align(TextAlign::Center);

        assert_eq!(style.font_size, 16.0);
        assert_eq!(style.font_style, FontStyle::Italic);
        assert_eq!(style.font_weight, FontWeight::Bold);
        assert_eq!(style.line_height, 1.5);
        assert_eq!(style.align, TextAlign::Center);
    }

    #[test]
    fn test_text_shadow() {
        let shadow = TextShadow::new(2.0, 2.0, 4.0, Color::new(0, 0, 0, 128));
        assert_eq!(shadow.offset_x, 2.0);
        assert_eq!(shadow.offset_y, 2.0);
        assert_eq!(shadow.blur, 4.0);
    }

    #[test]
    fn test_text_outline() {
        let outline = TextOutline::new(2.0, Color::BLACK);
        assert_eq!(outline.width, 2.0);
        assert_eq!(outline.color, Color::BLACK);
    }

    #[test]
    fn test_text_layout() {
        let mut layout = TextLayout::new();
        let style = TextStyle::default();
        layout.add_segment(TextSegment::new("Hello".to_string(), style));
        assert_eq!(layout.segments.len(), 1);
    }

    #[test]
    fn test_text_segment() {
        let style = TextStyle::default();
        let segment = TextSegment::new("Test".to_string(), style);
        assert_eq!(segment.text, "Test");
    }
}
