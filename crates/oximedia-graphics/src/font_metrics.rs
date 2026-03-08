//! Font metrics and text measurement.
//!
//! Provides types for describing font weight, metrics (ascent, descent,
//! line gap, advance widths), and a lightweight text measurer that can
//! estimate layout dimensions without a full rasterisation pass.

#![allow(dead_code)]

/// CSS-style font weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FontWeight {
    /// Thin (100).
    Thin,
    /// Extra-Light (200).
    ExtraLight,
    /// Light (300).
    Light,
    /// Regular (400).
    Regular,
    /// Medium (500).
    Medium,
    /// Semi-Bold (600).
    SemiBold,
    /// Bold (700).
    Bold,
    /// Extra-Bold (800).
    ExtraBold,
    /// Black (900).
    Black,
}

impl FontWeight {
    /// Numeric value (100 -- 900).
    #[must_use]
    pub fn value(&self) -> u16 {
        match self {
            Self::Thin => 100,
            Self::ExtraLight => 200,
            Self::Light => 300,
            Self::Regular => 400,
            Self::Medium => 500,
            Self::SemiBold => 600,
            Self::Bold => 700,
            Self::ExtraBold => 800,
            Self::Black => 900,
        }
    }

    /// Construct from a numeric value, snapping to the nearest named weight.
    #[must_use]
    pub fn from_value(v: u16) -> Self {
        match v {
            0..=149 => Self::Thin,
            150..=249 => Self::ExtraLight,
            250..=349 => Self::Light,
            350..=449 => Self::Regular,
            450..=549 => Self::Medium,
            550..=649 => Self::SemiBold,
            650..=749 => Self::Bold,
            750..=849 => Self::ExtraBold,
            _ => Self::Black,
        }
    }

    /// CSS keyword.
    #[must_use]
    pub fn keyword(&self) -> &'static str {
        match self {
            Self::Thin => "thin",
            Self::ExtraLight => "extra-light",
            Self::Light => "light",
            Self::Regular => "normal",
            Self::Medium => "medium",
            Self::SemiBold => "semi-bold",
            Self::Bold => "bold",
            Self::ExtraBold => "extra-bold",
            Self::Black => "black",
        }
    }

    /// Returns `true` for weights >= `SemiBold`.
    #[must_use]
    pub fn is_bold(&self) -> bool {
        self.value() >= 600
    }
}

/// Core metrics for a single typeface at a given size.
#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    /// Font size in points.
    pub size_pt: f32,
    /// Ascent in pixels (from baseline to top of tallest glyph).
    pub ascent: f32,
    /// Descent in pixels (baseline to bottom of lowest glyph, usually negative).
    pub descent: f32,
    /// Line gap in pixels (extra space between lines).
    pub line_gap: f32,
    /// Average character width in pixels.
    pub avg_char_width: f32,
    /// Maximum character width in pixels.
    pub max_char_width: f32,
    /// Cap-height in pixels.
    pub cap_height: f32,
    /// x-height in pixels.
    pub x_height: f32,
}

impl FontMetrics {
    /// Total line height: ascent + |descent| + `line_gap`.
    #[must_use]
    pub fn line_height(&self) -> f32 {
        self.ascent + self.descent.abs() + self.line_gap
    }

    /// Baseline-to-baseline distance for consecutive lines.
    #[must_use]
    pub fn baseline_distance(&self) -> f32 {
        self.line_height()
    }

    /// Scale all metrics proportionally to a new size in points.
    #[must_use]
    pub fn scaled(&self, new_size_pt: f32) -> Self {
        if self.size_pt <= 0.0 {
            return *self;
        }
        let factor = new_size_pt / self.size_pt;
        Self {
            size_pt: new_size_pt,
            ascent: self.ascent * factor,
            descent: self.descent * factor,
            line_gap: self.line_gap * factor,
            avg_char_width: self.avg_char_width * factor,
            max_char_width: self.max_char_width * factor,
            cap_height: self.cap_height * factor,
            x_height: self.x_height * factor,
        }
    }
}

impl Default for FontMetrics {
    /// Sensible defaults based on a 16pt sans-serif font.
    fn default() -> Self {
        Self {
            size_pt: 16.0,
            ascent: 14.0,
            descent: -4.0,
            line_gap: 2.0,
            avg_char_width: 8.0,
            max_char_width: 14.0,
            cap_height: 11.0,
            x_height: 8.0,
        }
    }
}

/// Result of a text measurement.
#[derive(Debug, Clone, Copy)]
pub struct TextExtent {
    /// Total width in pixels.
    pub width: f32,
    /// Total height in pixels (single line = `line_height`).
    pub height: f32,
    /// Number of lines.
    pub lines: usize,
}

/// Lightweight text measurer that estimates layout bounds from [`FontMetrics`].
#[derive(Debug, Clone)]
pub struct TextMeasurer {
    metrics: FontMetrics,
}

impl TextMeasurer {
    /// Create a measurer for the given font metrics.
    #[must_use]
    pub fn new(metrics: FontMetrics) -> Self {
        Self { metrics }
    }

    /// Measure the width of `text` in pixels.
    ///
    /// Simple model: each character contributes `avg_char_width`. Real
    /// implementations would use per-glyph advance widths and kerning.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn measure_width(&self, text: &str) -> f32 {
        text.chars().count() as f32 * self.metrics.avg_char_width
    }

    /// Measure the full extent of a single-line string.
    #[must_use]
    pub fn measure_line(&self, text: &str) -> TextExtent {
        TextExtent {
            width: self.measure_width(text),
            height: self.metrics.line_height(),
            lines: 1,
        }
    }

    /// Measure multi-line text (lines separated by `\n`).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn measure_multiline(&self, text: &str) -> TextExtent {
        let lines: Vec<&str> = text.split('\n').collect();
        let max_width = lines
            .iter()
            .map(|l| self.measure_width(l))
            .fold(0.0_f32, f32::max);
        let height = lines.len() as f32 * self.metrics.line_height();
        TextExtent {
            width: max_width,
            height,
            lines: lines.len(),
        }
    }

    /// Word-wrap text to a maximum pixel width and measure the result.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn measure_wrapped(&self, text: &str, max_width: f32) -> TextExtent {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            return TextExtent {
                width: 0.0,
                height: self.metrics.line_height(),
                lines: 1,
            };
        }

        let space_width = self.metrics.avg_char_width;
        let mut lines = 1usize;
        let mut current_width = 0.0_f32;
        let mut widest = 0.0_f32;

        for word in &words {
            let w = self.measure_width(word);
            let needed = if current_width > 0.0 {
                current_width + space_width + w
            } else {
                w
            };
            if needed > max_width && current_width > 0.0 {
                widest = widest.max(current_width);
                lines += 1;
                current_width = w;
            } else {
                current_width = needed;
            }
        }
        widest = widest.max(current_width);

        TextExtent {
            width: widest,
            height: lines as f32 * self.metrics.line_height(),
            lines,
        }
    }

    /// Return the inner font metrics.
    #[must_use]
    pub fn metrics(&self) -> &FontMetrics {
        &self.metrics
    }
}

// -- unit tests --

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_weight_values() {
        assert_eq!(FontWeight::Thin.value(), 100);
        assert_eq!(FontWeight::Regular.value(), 400);
        assert_eq!(FontWeight::Black.value(), 900);
    }

    #[test]
    fn test_font_weight_from_value() {
        assert_eq!(FontWeight::from_value(400), FontWeight::Regular);
        assert_eq!(FontWeight::from_value(0), FontWeight::Thin);
        assert_eq!(FontWeight::from_value(999), FontWeight::Black);
    }

    #[test]
    fn test_font_weight_is_bold() {
        assert!(!FontWeight::Regular.is_bold());
        assert!(FontWeight::Bold.is_bold());
        assert!(FontWeight::SemiBold.is_bold());
    }

    #[test]
    fn test_font_weight_ordering() {
        assert!(FontWeight::Thin < FontWeight::Regular);
        assert!(FontWeight::Regular < FontWeight::Bold);
    }

    #[test]
    fn test_font_weight_keywords() {
        assert_eq!(FontWeight::Regular.keyword(), "normal");
        assert_eq!(FontWeight::Bold.keyword(), "bold");
    }

    #[test]
    fn test_font_metrics_line_height() {
        let m = FontMetrics::default();
        // ascent=14, descent=-4, gap=2 -> 14+4+2=20
        assert!((m.line_height() - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_font_metrics_scaled() {
        let m = FontMetrics::default(); // 16pt
        let s = m.scaled(32.0);
        assert!((s.ascent - 28.0).abs() < 0.01); // 14 * 2
        assert!((s.avg_char_width - 16.0).abs() < 0.01); // 8 * 2
    }

    #[test]
    fn test_measure_width_empty() {
        let measurer = TextMeasurer::new(FontMetrics::default());
        assert_eq!(measurer.measure_width(""), 0.0);
    }

    #[test]
    fn test_measure_width_single_char() {
        let m = FontMetrics::default();
        let measurer = TextMeasurer::new(m);
        assert_eq!(measurer.measure_width("A"), m.avg_char_width);
    }

    #[test]
    fn test_measure_line() {
        let measurer = TextMeasurer::new(FontMetrics::default());
        let ext = measurer.measure_line("Hello");
        assert_eq!(ext.lines, 1);
        assert!(ext.width > 0.0);
        assert!(ext.height > 0.0);
    }

    #[test]
    fn test_measure_multiline() {
        let measurer = TextMeasurer::new(FontMetrics::default());
        let ext = measurer.measure_multiline("Line1\nLine2\nLine3");
        assert_eq!(ext.lines, 3);
        assert!((ext.height - 3.0 * measurer.metrics().line_height()).abs() < 0.01);
    }

    #[test]
    fn test_measure_wrapped() {
        let measurer = TextMeasurer::new(FontMetrics::default());
        // "Hello World" at 8px/char = 11 chars * 8 = 88px
        // Max width 50 -> should wrap
        let ext = measurer.measure_wrapped("Hello World", 50.0);
        assert!(ext.lines >= 2);
    }

    #[test]
    fn test_measure_wrapped_single_word() {
        let measurer = TextMeasurer::new(FontMetrics::default());
        let ext = measurer.measure_wrapped("Supercalifragilistic", 40.0);
        // Single word wider than max_width -> stays on one line
        assert_eq!(ext.lines, 1);
    }

    #[test]
    fn test_measure_wrapped_empty() {
        let measurer = TextMeasurer::new(FontMetrics::default());
        let ext = measurer.measure_wrapped("", 100.0);
        assert_eq!(ext.lines, 1);
        assert_eq!(ext.width, 0.0);
    }

    #[test]
    fn test_baseline_distance() {
        let m = FontMetrics::default();
        assert_eq!(m.baseline_distance(), m.line_height());
    }
}
