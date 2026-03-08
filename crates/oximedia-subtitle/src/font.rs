//! Font loading and glyph management.

use crate::{SubtitleError, SubtitleResult};
use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
use fontdue::{Font as FontdueFont, FontSettings};
use std::collections::HashMap;

/// A loaded font for subtitle rendering.
pub struct Font {
    inner: FontdueFont,
}

impl Font {
    /// Load a font from bytes (TTF or OTF).
    ///
    /// # Errors
    ///
    /// Returns error if the font data is invalid.
    pub fn from_bytes(data: Vec<u8>) -> SubtitleResult<Self> {
        let inner = FontdueFont::from_bytes(data, FontSettings::default())
            .map_err(|e| SubtitleError::FontError(format!("Failed to load font: {e}")))?;

        Ok(Self { inner })
    }

    /// Load a font from a file.
    ///
    /// # Errors
    ///
    /// Returns error if the file cannot be read or the font is invalid.
    pub fn from_file(path: &str) -> SubtitleResult<Self> {
        let data = std::fs::read(path)
            .map_err(|e| SubtitleError::FontError(format!("Failed to read font file: {e}")))?;
        Self::from_bytes(data)
    }

    /// Get the font's internal representation.
    #[must_use]
    pub(crate) fn inner(&self) -> &FontdueFont {
        &self.inner
    }

    /// Get font metrics for a given size.
    #[must_use]
    pub fn metrics(&self, size: f32) -> FontMetrics {
        let metrics = self
            .inner
            .horizontal_line_metrics(size)
            .unwrap_or(fontdue::LineMetrics {
                ascent: 0.0,
                descent: 0.0,
                line_gap: 0.0,
                new_line_size: 0.0,
            });

        FontMetrics {
            ascent: metrics.ascent,
            descent: metrics.descent,
            line_gap: metrics.line_gap,
            new_line_size: metrics.new_line_size,
        }
    }

    /// Measure text width at given size.
    #[must_use]
    pub fn measure_text(&self, text: &str, size: f32) -> f32 {
        let mut width = 0.0;
        for c in text.chars() {
            let metrics = self.inner.metrics(c, size);
            width += metrics.advance_width;
        }
        width
    }
}

/// Font metrics for a specific size.
#[derive(Clone, Copy, Debug)]
pub struct FontMetrics {
    /// Ascent (pixels above baseline).
    pub ascent: f32,
    /// Descent (pixels below baseline).
    pub descent: f32,
    /// Gap between lines.
    pub line_gap: f32,
    /// Recommended line height.
    pub new_line_size: f32,
}

/// Cached glyph bitmap.
#[derive(Clone, Debug)]
pub struct CachedGlyph {
    /// Rasterized glyph bitmap (grayscale).
    pub bitmap: Vec<u8>,
    /// Bitmap width in pixels.
    pub width: usize,
    /// Bitmap height in pixels.
    pub height: usize,
    /// Horizontal offset from origin.
    pub offset_x: f32,
    /// Vertical offset from origin.
    pub offset_y: f32,
    /// Advance width to next glyph.
    pub advance_width: f32,
}

/// Glyph cache key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct GlyphKey {
    /// Character code point.
    codepoint: u32,
    /// Font size (scaled to avoid floating point key).
    size_scaled: u32,
}

impl GlyphKey {
    fn new(codepoint: char, size: f32) -> Self {
        Self {
            codepoint: codepoint as u32,
            size_scaled: (size * 100.0) as u32,
        }
    }
}

/// Cache for rasterized glyphs to avoid re-rendering.
pub struct GlyphCache {
    cache: HashMap<GlyphKey, CachedGlyph>,
    font: Font,
}

impl GlyphCache {
    /// Create a new glyph cache for the given font.
    #[must_use]
    pub fn new(font: Font) -> Self {
        Self {
            cache: HashMap::new(),
            font,
        }
    }

    /// Get or rasterize a glyph.
    pub fn get_glyph(&mut self, c: char, size: f32) -> &CachedGlyph {
        let key = GlyphKey::new(c, size);

        self.cache.entry(key).or_insert_with(|| {
            let (metrics, bitmap) = self.font.inner.rasterize(c, size);

            CachedGlyph {
                bitmap,
                width: metrics.width,
                height: metrics.height,
                offset_x: metrics.xmin as f32,
                offset_y: metrics.ymin as f32,
                advance_width: metrics.advance_width,
            }
        })
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Get cache size (number of cached glyphs).
    #[must_use]
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Get the underlying font.
    #[must_use]
    pub fn font(&self) -> &Font {
        &self.font
    }
}

/// Simple layout engine using fontdue's layout.
pub struct SimpleLayoutEngine {
    layout: Layout,
}

impl SimpleLayoutEngine {
    /// Create a new layout engine.
    #[must_use]
    pub fn new() -> Self {
        Self {
            layout: Layout::new(CoordinateSystem::PositiveYDown),
        }
    }

    /// Layout text and return glyph positions.
    pub fn layout_text(
        &mut self,
        font: &Font,
        text: &str,
        size: f32,
        max_width: Option<f32>,
    ) -> Vec<GlyphPosition> {
        self.layout.reset(&LayoutSettings {
            x: 0.0,
            y: 0.0,
            max_width,
            max_height: None,
            horizontal_align: fontdue::layout::HorizontalAlign::Left,
            vertical_align: fontdue::layout::VerticalAlign::Top,
            line_height: size * 1.2,
            wrap_style: fontdue::layout::WrapStyle::Word,
            wrap_hard_breaks: true,
        });

        let fonts = &[font.inner()];
        self.layout.append(fonts, &TextStyle::new(text, size, 0));

        self.layout
            .glyphs()
            .iter()
            .map(|g| GlyphPosition {
                c: g.parent,
                x: g.x,
                y: g.y,
                width: g.width as f32,
                height: g.height as f32,
            })
            .collect()
    }

    /// Get layout bounds (width, height).
    #[must_use]
    pub fn bounds(&self) -> (f32, f32) {
        let glyphs = self.layout.glyphs();
        if glyphs.is_empty() {
            return (0.0, 0.0);
        }

        let max_x = glyphs
            .iter()
            .map(|g| g.x + g.width as f32)
            .fold(0.0_f32, f32::max);
        let max_y = glyphs
            .iter()
            .map(|g| g.y + g.height as f32)
            .fold(0.0_f32, f32::max);

        (max_x, max_y)
    }
}

impl Default for SimpleLayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Positioned glyph from layout.
#[derive(Clone, Copy, Debug)]
pub struct GlyphPosition {
    /// Character.
    pub c: char,
    /// X position.
    pub x: f32,
    /// Y position.
    pub y: f32,
    /// Glyph width.
    pub width: f32,
    /// Glyph height.
    pub height: f32,
}
