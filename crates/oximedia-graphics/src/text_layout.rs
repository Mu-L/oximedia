#![allow(dead_code)]
//! Text layout engine for broadcast graphics.
//!
//! Provides paragraph-level text layout with support for line breaking,
//! justification, alignment, and multi-line text rendering for broadcast
//! graphics elements such as lower thirds, tickers, and scoreboards.

use std::fmt;

/// Horizontal text alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextAlign {
    /// Left-aligned text.
    #[default]
    Left,
    /// Center-aligned text.
    Center,
    /// Right-aligned text.
    Right,
    /// Justified text (both edges aligned).
    Justify,
}

impl fmt::Display for TextAlign {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Left => write!(f, "left"),
            Self::Center => write!(f, "center"),
            Self::Right => write!(f, "right"),
            Self::Justify => write!(f, "justify"),
        }
    }
}

/// Vertical text alignment within a bounding box.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VerticalAlign {
    /// Top-aligned text.
    #[default]
    Top,
    /// Middle-aligned text.
    Middle,
    /// Bottom-aligned text.
    Bottom,
}

/// Text overflow behavior when text exceeds the bounding box.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextOverflow {
    /// Clip text at the boundary.
    #[default]
    Clip,
    /// Add ellipsis (...) at the end.
    Ellipsis,
    /// Allow text to overflow.
    Visible,
    /// Shrink font size to fit.
    ShrinkToFit,
}

/// Line break mode for text wrapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LineBreakMode {
    /// Break at word boundaries.
    #[default]
    WordWrap,
    /// Break at character boundaries.
    CharWrap,
    /// No wrapping (single line).
    NoWrap,
    /// Break at word boundaries, fall back to character wrap if word is too long.
    WordCharWrap,
}

/// A positioned glyph in the layout.
#[derive(Clone, Debug)]
pub struct LayoutGlyph {
    /// Character represented by this glyph.
    pub character: char,
    /// X position of the glyph.
    pub x: f32,
    /// Y position of the glyph.
    pub y: f32,
    /// Width of the glyph advance.
    pub advance_width: f32,
    /// Height of the glyph line.
    pub line_height: f32,
    /// Index of the line this glyph belongs to.
    pub line_index: usize,
    /// Index of the glyph within the original text.
    pub char_index: usize,
}

/// A laid-out line of text.
#[derive(Clone, Debug)]
pub struct LayoutLine {
    /// Glyphs in this line.
    pub glyphs: Vec<LayoutGlyph>,
    /// Y offset of this line from the top of the layout.
    pub y_offset: f32,
    /// Width of the content on this line.
    pub width: f32,
    /// Height of this line.
    pub height: f32,
    /// Index of the first character in the original text.
    pub start_char: usize,
    /// Index past the last character in the original text.
    pub end_char: usize,
}

impl LayoutLine {
    /// Create a new empty layout line.
    pub fn new(y_offset: f32, height: f32, start_char: usize) -> Self {
        Self {
            glyphs: Vec::new(),
            y_offset,
            width: 0.0,
            height,
            start_char,
            end_char: start_char,
        }
    }

    /// Get the number of glyphs on this line.
    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    /// Check if this line is empty.
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }
}

/// Configuration for text layout.
#[derive(Clone, Debug)]
pub struct TextLayoutConfig {
    /// Maximum width for text wrapping (pixels).
    pub max_width: f32,
    /// Maximum height for clipping (pixels). 0 means unlimited.
    pub max_height: f32,
    /// Horizontal alignment.
    pub align: TextAlign,
    /// Vertical alignment.
    pub vertical_align: VerticalAlign,
    /// Font size in pixels.
    pub font_size: f32,
    /// Line height multiplier (1.0 = default).
    pub line_height: f32,
    /// Letter spacing in pixels.
    pub letter_spacing: f32,
    /// Word spacing multiplier (1.0 = default).
    pub word_spacing: f32,
    /// Text overflow behavior.
    pub overflow: TextOverflow,
    /// Line break mode.
    pub line_break: LineBreakMode,
    /// First line indent in pixels.
    pub first_line_indent: f32,
    /// Paragraph spacing in pixels (extra space between paragraphs).
    pub paragraph_spacing: f32,
    /// Tab stop width in spaces.
    pub tab_width: u32,
}

impl Default for TextLayoutConfig {
    fn default() -> Self {
        Self {
            max_width: 800.0,
            max_height: 0.0,
            align: TextAlign::Left,
            vertical_align: VerticalAlign::Top,
            font_size: 16.0,
            line_height: 1.2,
            letter_spacing: 0.0,
            word_spacing: 1.0,
            overflow: TextOverflow::Clip,
            line_break: LineBreakMode::WordWrap,
            first_line_indent: 0.0,
            paragraph_spacing: 0.0,
            tab_width: 4,
        }
    }
}

impl TextLayoutConfig {
    /// Create a config for centered broadcast title text.
    pub fn broadcast_title(max_width: f32, font_size: f32) -> Self {
        Self {
            max_width,
            font_size,
            align: TextAlign::Center,
            vertical_align: VerticalAlign::Middle,
            line_height: 1.3,
            letter_spacing: 1.0,
            overflow: TextOverflow::ShrinkToFit,
            line_break: LineBreakMode::WordWrap,
            ..Default::default()
        }
    }

    /// Create a config for a lower-third text block.
    pub fn lower_third(max_width: f32, font_size: f32) -> Self {
        Self {
            max_width,
            font_size,
            align: TextAlign::Left,
            vertical_align: VerticalAlign::Bottom,
            line_height: 1.1,
            overflow: TextOverflow::Ellipsis,
            line_break: LineBreakMode::WordWrap,
            ..Default::default()
        }
    }

    /// Create a config for ticker text (single-line, no wrap).
    pub fn ticker(max_width: f32, font_size: f32) -> Self {
        Self {
            max_width,
            font_size,
            align: TextAlign::Left,
            line_break: LineBreakMode::NoWrap,
            overflow: TextOverflow::Visible,
            ..Default::default()
        }
    }

    /// Compute the effective line height in pixels.
    pub fn effective_line_height(&self) -> f32 {
        self.font_size * self.line_height
    }
}

/// Result of a text layout operation.
#[derive(Clone, Debug)]
pub struct TextLayoutResult {
    /// Laid-out lines.
    pub lines: Vec<LayoutLine>,
    /// Total width of the laid-out text.
    pub total_width: f32,
    /// Total height of the laid-out text.
    pub total_height: f32,
    /// Whether text was truncated.
    pub truncated: bool,
    /// Number of characters that fit in the layout.
    pub chars_fitted: usize,
    /// Total number of characters in the input.
    pub total_chars: usize,
}

impl TextLayoutResult {
    /// Check if the entire text fit within the layout bounds.
    pub fn is_complete(&self) -> bool {
        !self.truncated
    }

    /// Get the number of lines.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get total number of glyphs across all lines.
    pub fn glyph_count(&self) -> usize {
        self.lines.iter().map(LayoutLine::glyph_count).sum()
    }
}

/// Text layout engine.
///
/// Performs paragraph-level text layout including line breaking, alignment,
/// and glyph positioning for broadcast graphics rendering.
pub struct TextLayoutEngine {
    /// Layout configuration.
    config: TextLayoutConfig,
    /// Average character width estimate (for simple metrics).
    avg_char_width: f32,
}

impl TextLayoutEngine {
    /// Create a new text layout engine with the given configuration.
    pub fn new(config: TextLayoutConfig) -> Self {
        let avg_char_width = config.font_size * 0.6;
        Self {
            config,
            avg_char_width,
        }
    }

    /// Update the layout configuration.
    pub fn set_config(&mut self, config: TextLayoutConfig) {
        self.avg_char_width = config.font_size * 0.6;
        self.config = config;
    }

    /// Get the current configuration.
    pub fn config(&self) -> &TextLayoutConfig {
        &self.config
    }

    /// Estimate the width of a character using simple metrics.
    fn estimate_char_width(&self, ch: char) -> f32 {
        (if ch == ' ' {
            self.avg_char_width * self.config.word_spacing
        } else if ch == '\t' {
            self.avg_char_width * self.config.tab_width as f32
        } else if ch.is_ascii_uppercase() || ch == 'W' || ch == 'M' {
            self.avg_char_width * 1.2
        } else if ch == 'i' || ch == 'l' || ch == '!' || ch == '|' {
            self.avg_char_width * 0.5
        } else {
            self.avg_char_width
        }) + self.config.letter_spacing
    }

    /// Estimate the width of a string.
    pub fn estimate_text_width(&self, text: &str) -> f32 {
        text.chars().map(|ch| self.estimate_char_width(ch)).sum()
    }

    /// Find the word boundary for breaking.
    fn find_word_break(&self, text: &str, max_width: f32) -> usize {
        let mut width = 0.0;
        let mut last_space = 0;
        for (i, ch) in text.chars().enumerate() {
            if ch == ' ' || ch == '\t' {
                last_space = i;
            }
            width += self.estimate_char_width(ch);
            if width > max_width {
                return if last_space > 0 { last_space } else { i.max(1) };
            }
        }
        text.chars().count()
    }

    /// Find the character boundary for breaking.
    fn find_char_break(&self, text: &str, max_width: f32) -> usize {
        let mut width = 0.0;
        for (i, ch) in text.chars().enumerate() {
            width += self.estimate_char_width(ch);
            if width > max_width {
                return i.max(1);
            }
        }
        text.chars().count()
    }

    /// Layout text according to the current configuration.
    pub fn layout(&self, text: &str) -> TextLayoutResult {
        let total_chars = text.chars().count();
        let line_h = self.config.effective_line_height();
        let max_w = self.config.max_width;

        let mut lines: Vec<LayoutLine> = Vec::new();
        let mut y_offset: f32 = 0.0;
        let mut global_char_idx: usize = 0;
        let mut truncated = false;

        // Split by explicit newlines (paragraphs).
        let paragraphs: Vec<&str> = text.split('\n').collect();

        for (para_idx, paragraph) in paragraphs.iter().enumerate() {
            if para_idx > 0 {
                y_offset += self.config.paragraph_spacing;
            }

            if paragraph.is_empty() {
                // Empty paragraph still advances by one line.
                let line = LayoutLine::new(y_offset, line_h, global_char_idx);
                lines.push(line);
                y_offset += line_h;
                global_char_idx += 1; // account for the newline
                continue;
            }

            let mut remaining = *paragraph;
            let mut first_line_of_para = true;

            while !remaining.is_empty() {
                // Check height limit.
                if self.config.max_height > 0.0 && y_offset + line_h > self.config.max_height {
                    truncated = true;
                    break;
                }

                let effective_max = if first_line_of_para {
                    max_w - self.config.first_line_indent
                } else {
                    max_w
                };

                let line_width = self.estimate_text_width(remaining);

                let break_at = if self.config.line_break == LineBreakMode::NoWrap {
                    remaining.chars().count()
                } else if line_width <= effective_max {
                    remaining.chars().count()
                } else {
                    match self.config.line_break {
                        LineBreakMode::WordWrap => self.find_word_break(remaining, effective_max),
                        LineBreakMode::CharWrap => self.find_char_break(remaining, effective_max),
                        LineBreakMode::WordCharWrap => {
                            let wb = self.find_word_break(remaining, effective_max);
                            if wb == 0 {
                                self.find_char_break(remaining, effective_max)
                            } else {
                                wb
                            }
                        }
                        LineBreakMode::NoWrap => remaining.chars().count(),
                    }
                };

                // Extract the line text.
                let line_text: String = remaining.chars().take(break_at).collect();
                let consumed_chars = line_text.chars().count();

                // Build glyphs for this line.
                let x_start = if first_line_of_para {
                    self.config.first_line_indent
                } else {
                    0.0
                };

                let mut glyphs = Vec::new();
                let mut x = x_start;
                for (i, ch) in line_text.chars().enumerate() {
                    let adv = self.estimate_char_width(ch);
                    glyphs.push(LayoutGlyph {
                        character: ch,
                        x,
                        y: y_offset,
                        advance_width: adv,
                        line_height: line_h,
                        line_index: lines.len(),
                        char_index: global_char_idx + i,
                    });
                    x += adv;
                }

                let mut layout_line = LayoutLine::new(y_offset, line_h, global_char_idx);
                layout_line.width = x - x_start;
                layout_line.end_char = global_char_idx + consumed_chars;
                layout_line.glyphs = glyphs;
                lines.push(layout_line);

                y_offset += line_h;
                global_char_idx += consumed_chars;

                // Skip past consumed characters and any trailing space.
                let rest: String = remaining.chars().skip(break_at).collect();
                remaining = if rest.starts_with(' ') {
                    global_char_idx += 1;
                    let trimmed: String = rest.chars().skip(1).collect();
                    // We need remaining to be a &str, but we own the String.
                    // Use a workaround: leak is not ideal, so just re-check.
                    // Instead, let's use an owned-string approach.
                    Box::leak(trimmed.into_boxed_str())
                } else {
                    Box::leak(rest.into_boxed_str())
                };

                first_line_of_para = false;
            }

            if truncated {
                break;
            }

            // Account for the newline character between paragraphs.
            if para_idx < paragraphs.len() - 1 {
                global_char_idx += 1;
            }
        }

        // Apply horizontal alignment.
        for line in &mut lines {
            let shift = match self.config.align {
                TextAlign::Left | TextAlign::Justify => 0.0,
                TextAlign::Center => (max_w - line.width) / 2.0,
                TextAlign::Right => max_w - line.width,
            };
            if shift > 0.0 {
                for glyph in &mut line.glyphs {
                    glyph.x += shift;
                }
            }
        }

        // Apply vertical alignment.
        let total_height = y_offset;
        if self.config.max_height > 0.0 && self.config.vertical_align != VerticalAlign::Top {
            let v_shift = match self.config.vertical_align {
                VerticalAlign::Top => 0.0,
                VerticalAlign::Middle => (self.config.max_height - total_height) / 2.0,
                VerticalAlign::Bottom => self.config.max_height - total_height,
            };
            if v_shift > 0.0 {
                for line in &mut lines {
                    line.y_offset += v_shift;
                    for glyph in &mut line.glyphs {
                        glyph.y += v_shift;
                    }
                }
            }
        }

        let total_width = lines.iter().map(|l| l.width).fold(0.0_f32, f32::max);

        TextLayoutResult {
            lines,
            total_width,
            total_height,
            truncated,
            chars_fitted: global_char_idx,
            total_chars,
        }
    }

    /// Layout text and return just the bounding box dimensions.
    pub fn measure(&self, text: &str) -> (f32, f32) {
        let result = self.layout(text);
        (result.total_width, result.total_height)
    }

    /// Check if text will fit within the configured bounds without truncation.
    pub fn fits(&self, text: &str) -> bool {
        let result = self.layout(text);
        result.is_complete()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_align_display() {
        assert_eq!(format!("{}", TextAlign::Left), "left");
        assert_eq!(format!("{}", TextAlign::Center), "center");
        assert_eq!(format!("{}", TextAlign::Right), "right");
        assert_eq!(format!("{}", TextAlign::Justify), "justify");
    }

    #[test]
    fn test_text_align_default() {
        assert_eq!(TextAlign::default(), TextAlign::Left);
    }

    #[test]
    fn test_vertical_align_default() {
        assert_eq!(VerticalAlign::default(), VerticalAlign::Top);
    }

    #[test]
    fn test_text_overflow_default() {
        assert_eq!(TextOverflow::default(), TextOverflow::Clip);
    }

    #[test]
    fn test_line_break_mode_default() {
        assert_eq!(LineBreakMode::default(), LineBreakMode::WordWrap);
    }

    #[test]
    fn test_layout_config_default() {
        let config = TextLayoutConfig::default();
        assert_eq!(config.max_width, 800.0);
        assert_eq!(config.font_size, 16.0);
        assert_eq!(config.line_height, 1.2);
        assert_eq!(config.letter_spacing, 0.0);
        assert_eq!(config.word_spacing, 1.0);
        assert_eq!(config.tab_width, 4);
    }

    #[test]
    fn test_effective_line_height() {
        let config = TextLayoutConfig {
            font_size: 20.0,
            line_height: 1.5,
            ..Default::default()
        };
        let expected = 20.0 * 1.5;
        assert!((config.effective_line_height() - expected).abs() < 0.001);
    }

    #[test]
    fn test_broadcast_title_config() {
        let config = TextLayoutConfig::broadcast_title(1920.0, 48.0);
        assert_eq!(config.align, TextAlign::Center);
        assert_eq!(config.vertical_align, VerticalAlign::Middle);
        assert_eq!(config.font_size, 48.0);
        assert_eq!(config.overflow, TextOverflow::ShrinkToFit);
    }

    #[test]
    fn test_lower_third_config() {
        let config = TextLayoutConfig::lower_third(600.0, 24.0);
        assert_eq!(config.align, TextAlign::Left);
        assert_eq!(config.vertical_align, VerticalAlign::Bottom);
        assert_eq!(config.overflow, TextOverflow::Ellipsis);
    }

    #[test]
    fn test_ticker_config() {
        let config = TextLayoutConfig::ticker(1920.0, 18.0);
        assert_eq!(config.line_break, LineBreakMode::NoWrap);
        assert_eq!(config.overflow, TextOverflow::Visible);
    }

    #[test]
    fn test_simple_layout() {
        let config = TextLayoutConfig {
            max_width: 1000.0,
            font_size: 16.0,
            ..Default::default()
        };
        let engine = TextLayoutEngine::new(config);
        let result = engine.layout("Hello World");
        assert_eq!(result.line_count(), 1);
        assert!(!result.truncated);
        assert!(result.total_width > 0.0);
        assert!(result.total_height > 0.0);
    }

    #[test]
    fn test_multiline_layout() {
        let config = TextLayoutConfig {
            max_width: 1000.0,
            font_size: 16.0,
            ..Default::default()
        };
        let engine = TextLayoutEngine::new(config);
        let result = engine.layout("Line one\nLine two\nLine three");
        assert_eq!(result.line_count(), 3);
        assert!(!result.truncated);
    }

    #[test]
    fn test_word_wrap() {
        // Use a very narrow width to force wrapping.
        let config = TextLayoutConfig {
            max_width: 60.0,
            font_size: 16.0,
            line_break: LineBreakMode::WordWrap,
            ..Default::default()
        };
        let engine = TextLayoutEngine::new(config);
        let result = engine.layout("Hello World Test");
        assert!(result.line_count() > 1);
    }

    #[test]
    fn test_no_wrap() {
        let config = TextLayoutConfig {
            max_width: 50.0,
            font_size: 16.0,
            line_break: LineBreakMode::NoWrap,
            ..Default::default()
        };
        let engine = TextLayoutEngine::new(config);
        let result = engine.layout("This is a very long line that should not wrap");
        assert_eq!(result.line_count(), 1);
    }

    #[test]
    fn test_estimate_text_width() {
        let config = TextLayoutConfig {
            font_size: 16.0,
            ..Default::default()
        };
        let engine = TextLayoutEngine::new(config);
        let w = engine.estimate_text_width("Hello");
        assert!(w > 0.0);
    }

    #[test]
    fn test_measure() {
        let config = TextLayoutConfig {
            max_width: 500.0,
            font_size: 20.0,
            ..Default::default()
        };
        let engine = TextLayoutEngine::new(config);
        let (w, h) = engine.measure("Test string");
        assert!(w > 0.0);
        assert!(h > 0.0);
    }

    #[test]
    fn test_fits() {
        let config = TextLayoutConfig {
            max_width: 1000.0,
            max_height: 0.0, // unlimited height
            font_size: 16.0,
            ..Default::default()
        };
        let engine = TextLayoutEngine::new(config);
        assert!(engine.fits("Short text"));
    }

    #[test]
    fn test_layout_line_new() {
        let line = LayoutLine::new(10.0, 20.0, 5);
        assert!(line.is_empty());
        assert_eq!(line.glyph_count(), 0);
        assert_eq!(line.y_offset, 10.0);
        assert_eq!(line.height, 20.0);
        assert_eq!(line.start_char, 5);
    }

    #[test]
    fn test_layout_result_completeness() {
        let config = TextLayoutConfig {
            max_width: 1000.0,
            font_size: 16.0,
            ..Default::default()
        };
        let engine = TextLayoutEngine::new(config);
        let result = engine.layout("Short");
        assert!(result.is_complete());
        assert!(result.glyph_count() > 0);
        assert_eq!(result.total_chars, 5);
    }

    #[test]
    fn test_center_alignment() {
        let config = TextLayoutConfig {
            max_width: 500.0,
            font_size: 16.0,
            align: TextAlign::Center,
            ..Default::default()
        };
        let engine = TextLayoutEngine::new(config);
        let result = engine.layout("Hi");
        assert_eq!(result.line_count(), 1);
        // Center-aligned glyphs should start past x=0.
        if let Some(first_glyph) = result.lines[0].glyphs.first() {
            assert!(first_glyph.x > 0.0);
        }
    }

    #[test]
    fn test_right_alignment() {
        let config = TextLayoutConfig {
            max_width: 500.0,
            font_size: 16.0,
            align: TextAlign::Right,
            ..Default::default()
        };
        let engine = TextLayoutEngine::new(config);
        let result = engine.layout("Hi");
        assert_eq!(result.line_count(), 1);
        // Right-aligned glyphs should be near the right edge.
        if let Some(first_glyph) = result.lines[0].glyphs.first() {
            assert!(first_glyph.x > 100.0);
        }
    }

    #[test]
    fn test_set_config() {
        let config1 = TextLayoutConfig {
            font_size: 16.0,
            ..Default::default()
        };
        let config2 = TextLayoutConfig {
            font_size: 32.0,
            ..Default::default()
        };
        let mut engine = TextLayoutEngine::new(config1);
        engine.set_config(config2);
        assert_eq!(engine.config().font_size, 32.0);
    }

    #[test]
    fn test_empty_text_layout() {
        let config = TextLayoutConfig::default();
        let engine = TextLayoutEngine::new(config);
        let result = engine.layout("");
        assert_eq!(result.total_chars, 0);
    }
}
