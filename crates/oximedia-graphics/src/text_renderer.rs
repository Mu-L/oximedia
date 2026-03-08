#![allow(dead_code)]
//! Text rendering primitives for broadcast graphics.
//!
//! Provides text alignment, font weight, and style types along with a
//! `TextRenderer` that measures text geometry and a `TextStyle` builder.

/// Horizontal or vertical alignment of rendered text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextAlignment {
    /// Left-align text within the bounding box.
    Left,
    /// Centre-align text horizontally.
    Center,
    /// Right-align text.
    Right,
    /// Justify text to both edges.
    Justify,
    /// Align text to the top of its container.
    Top,
    /// Align text to the vertical centre of its container.
    Middle,
    /// Align text to the bottom of its container.
    Bottom,
}

impl TextAlignment {
    /// Returns `true` for alignments that operate on the horizontal axis.
    pub fn is_horizontal(&self) -> bool {
        matches!(
            self,
            TextAlignment::Left
                | TextAlignment::Center
                | TextAlignment::Right
                | TextAlignment::Justify
        )
    }

    /// Returns `true` for alignments that operate on the vertical axis.
    pub fn is_vertical(&self) -> bool {
        !self.is_horizontal()
    }
}

/// Font weight descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    /// Thin (100).
    Thin,
    /// Extra-light (200).
    ExtraLight,
    /// Light (300).
    Light,
    /// Regular (400).
    Regular,
    /// Medium (500).
    Medium,
    /// Semi-bold (600).
    SemiBold,
    /// Bold (700).
    Bold,
    /// Extra-bold (800).
    ExtraBold,
    /// Black (900).
    Black,
}

impl FontWeight {
    /// Returns `true` for weights ≥ Bold (700).
    pub fn is_bold(&self) -> bool {
        matches!(
            self,
            FontWeight::Bold | FontWeight::ExtraBold | FontWeight::Black
        )
    }

    /// Numeric CSS-style weight value.
    pub fn numeric_value(&self) -> u16 {
        match self {
            FontWeight::Thin => 100,
            FontWeight::ExtraLight => 200,
            FontWeight::Light => 300,
            FontWeight::Regular => 400,
            FontWeight::Medium => 500,
            FontWeight::SemiBold => 600,
            FontWeight::Bold => 700,
            FontWeight::ExtraBold => 800,
            FontWeight::Black => 900,
        }
    }
}

/// Complete style specification for a block of rendered text.
#[derive(Debug, Clone)]
pub struct TextStyle {
    /// Font family name.
    pub font_family: String,
    /// Font size in points.
    pub font_size_pt: f32,
    /// Font weight.
    pub weight: FontWeight,
    /// Horizontal alignment.
    pub alignment: TextAlignment,
    /// Text opacity in `[0.0, 1.0]`.
    pub opacity: f32,
    /// Whether italic rendering is requested.
    pub italic: bool,
    /// RGBA packed colour.
    pub color: u32,
    /// Letter-spacing in pixels (positive = wider).
    pub letter_spacing_px: f32,
    /// Line-height multiplier (1.0 = normal).
    pub line_height: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: "Arial".to_string(),
            font_size_pt: 24.0,
            weight: FontWeight::Regular,
            alignment: TextAlignment::Left,
            opacity: 1.0,
            italic: false,
            color: 0xFFFF_FFFF,
            letter_spacing_px: 0.0,
            line_height: 1.2,
        }
    }
}

impl TextStyle {
    /// Create a style with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` when the text will be rendered visibly (opacity > 0).
    pub fn is_visible(&self) -> bool {
        self.opacity > 0.0
    }

    /// Returns `true` when the style uses a bold font weight.
    pub fn is_bold(&self) -> bool {
        self.weight.is_bold()
    }

    /// Returns `true` when the style uses italic rendering.
    pub fn is_italic(&self) -> bool {
        self.italic
    }

    /// Builder-style setter for font family.
    pub fn with_font(mut self, family: &str) -> Self {
        self.font_family = family.to_string();
        self
    }

    /// Builder-style setter for font size.
    pub fn with_size(mut self, pt: f32) -> Self {
        self.font_size_pt = pt;
        self
    }

    /// Builder-style setter for weight.
    pub fn with_weight(mut self, weight: FontWeight) -> Self {
        self.weight = weight;
        self
    }

    /// Builder-style setter for opacity.
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }
}

/// Simple text renderer that provides measurement and layout utilities.
///
/// This is a lightweight, dependency-free model renderer. Actual pixel rasterisation
/// is delegated to the GPU layer; this struct provides geometry metrics only.
#[derive(Debug, Clone)]
pub struct TextRenderer {
    /// Pixels per point scaling factor.
    pub px_per_pt: f32,
    /// Average glyph width as a fraction of `font_size_pt` (simplistic model).
    pub avg_glyph_width_ratio: f32,
    /// Maximum bounding-box width in pixels.
    pub max_width_px: f32,
}

impl Default for TextRenderer {
    fn default() -> Self {
        Self {
            px_per_pt: 1.333_333,
            avg_glyph_width_ratio: 0.55,
            max_width_px: 1920.0,
        }
    }
}

impl TextRenderer {
    /// Create a renderer with default parameters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a renderer for a specific display.
    pub fn with_display(px_per_pt: f32, max_width_px: f32) -> Self {
        Self {
            px_per_pt,
            max_width_px,
            ..Default::default()
        }
    }

    /// Estimate the rendered pixel width of `text` with the given style.
    #[allow(clippy::cast_precision_loss)]
    pub fn measure_width(&self, text: &str, style: &TextStyle) -> f32 {
        let char_count = text.chars().count() as f32;
        let glyph_width_px = style.font_size_pt * self.px_per_pt * self.avg_glyph_width_ratio;
        let spacing_total = style.letter_spacing_px * (char_count - 1.0).max(0.0);
        (glyph_width_px * char_count + spacing_total).min(self.max_width_px)
    }

    /// Estimate the number of wrapped lines for `text` inside `container_width_px`.
    ///
    /// Uses a word-wrap model: splits on whitespace and accumulates widths.
    #[allow(clippy::cast_precision_loss)]
    pub fn line_count(&self, text: &str, style: &TextStyle, container_width_px: f32) -> usize {
        if text.is_empty() {
            return 0;
        }
        let glyph_width_px = style.font_size_pt * self.px_per_pt * self.avg_glyph_width_ratio;
        let space_width = glyph_width_px;
        let mut lines = 1usize;
        let mut x = 0.0f32;
        for word in text.split_whitespace() {
            let word_width = word.chars().count() as f32 * glyph_width_px;
            if x > 0.0 && x + space_width + word_width > container_width_px {
                lines += 1;
                x = word_width;
            } else {
                if x > 0.0 {
                    x += space_width;
                }
                x += word_width;
            }
        }
        lines
    }

    /// Estimate the rendered pixel height of `text` using line metrics.
    #[allow(clippy::cast_precision_loss)]
    pub fn measure_height(&self, text: &str, style: &TextStyle, container_width_px: f32) -> f32 {
        let lines = self.line_count(text, style, container_width_px) as f32;
        let line_height_px = style.font_size_pt * self.px_per_pt * style.line_height;
        lines * line_height_px
    }

    /// Returns `true` when the estimated text width fits within `container_width_px`.
    pub fn fits_on_one_line(&self, text: &str, style: &TextStyle, container_width_px: f32) -> bool {
        self.measure_width(text, style) <= container_width_px
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment_left_is_horizontal() {
        assert!(TextAlignment::Left.is_horizontal());
    }

    #[test]
    fn test_alignment_center_is_horizontal() {
        assert!(TextAlignment::Center.is_horizontal());
    }

    #[test]
    fn test_alignment_top_is_not_horizontal() {
        assert!(!TextAlignment::Top.is_horizontal());
    }

    #[test]
    fn test_alignment_middle_is_vertical() {
        assert!(TextAlignment::Middle.is_vertical());
    }

    #[test]
    fn test_font_weight_bold_is_bold() {
        assert!(FontWeight::Bold.is_bold());
    }

    #[test]
    fn test_font_weight_regular_not_bold() {
        assert!(!FontWeight::Regular.is_bold());
    }

    #[test]
    fn test_font_weight_black_is_bold() {
        assert!(FontWeight::Black.is_bold());
    }

    #[test]
    fn test_font_weight_numeric_regular() {
        assert_eq!(FontWeight::Regular.numeric_value(), 400);
    }

    #[test]
    fn test_font_weight_numeric_bold() {
        assert_eq!(FontWeight::Bold.numeric_value(), 700);
    }

    #[test]
    fn test_text_style_visible_default() {
        let style = TextStyle::default();
        assert!(style.is_visible());
    }

    #[test]
    fn test_text_style_invisible_when_zero_opacity() {
        let style = TextStyle::default().with_opacity(0.0);
        assert!(!style.is_visible());
    }

    #[test]
    fn test_text_style_builder_font() {
        let style = TextStyle::new().with_font("Helvetica");
        assert_eq!(style.font_family, "Helvetica");
    }

    #[test]
    fn test_text_style_builder_weight() {
        let style = TextStyle::new().with_weight(FontWeight::Bold);
        assert!(style.is_bold());
    }

    #[test]
    fn test_renderer_measure_width_empty() {
        let r = TextRenderer::new();
        let s = TextStyle::default();
        assert_eq!(r.measure_width("", &s), 0.0);
    }

    #[test]
    fn test_renderer_measure_width_positive() {
        let r = TextRenderer::new();
        let s = TextStyle::default();
        assert!(r.measure_width("Hello World", &s) > 0.0);
    }

    #[test]
    fn test_renderer_line_count_empty() {
        let r = TextRenderer::new();
        let s = TextStyle::default();
        assert_eq!(r.line_count("", &s, 1920.0), 0);
    }

    #[test]
    fn test_renderer_line_count_single_short_word() {
        let r = TextRenderer::new();
        let s = TextStyle::default();
        assert_eq!(r.line_count("Hi", &s, 1920.0), 1);
    }

    #[test]
    fn test_renderer_line_count_wraps_on_narrow_container() {
        let r = TextRenderer::new();
        let s = TextStyle::default();
        // Very narrow container forces wrapping
        let lines = r.line_count("This is a long line of text that should wrap", &s, 50.0);
        assert!(lines > 1);
    }

    #[test]
    fn test_renderer_fits_on_one_line_short_text() {
        let r = TextRenderer::new();
        let s = TextStyle::default();
        assert!(r.fits_on_one_line("Hi", &s, 1920.0));
    }

    #[test]
    fn test_renderer_does_not_fit_narrow_container() {
        let r = TextRenderer::new();
        let s = TextStyle::default();
        assert!(!r.fits_on_one_line("A very long title that will not fit", &s, 10.0));
    }
}
