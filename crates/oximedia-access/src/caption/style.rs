//! Caption styling and formatting.

use oximedia_subtitle::style::SubtitleStyle;
use serde::{Deserialize, Serialize};

/// Caption style configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptionStyle {
    /// Font size in pixels.
    pub font_size: u32,
    /// Font family.
    pub font_family: String,
    /// Text color (RGBA).
    pub text_color: (u8, u8, u8, u8),
    /// Background color (RGBA).
    pub background_color: (u8, u8, u8, u8),
    /// Edge/outline style.
    pub edge_style: EdgeStyle,
    /// Edge color (RGBA).
    pub edge_color: (u8, u8, u8, u8),
    /// Opacity (0-255).
    pub opacity: u8,
}

/// Edge style for caption text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeStyle {
    /// No edge.
    None,
    /// Raised edge (3D effect).
    Raised,
    /// Depressed edge (3D effect).
    Depressed,
    /// Uniform drop shadow.
    DropShadow,
    /// Outline around text.
    Outline,
}

/// Preset caption styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptionStylePreset {
    /// Standard white text on black background.
    Standard,
    /// High contrast for better readability.
    HighContrast,
    /// Yellow text on black (common for DVDs).
    YellowOnBlack,
    /// Black text on white (for light backgrounds).
    BlackOnWhite,
    /// Transparent background.
    Transparent,
}

impl Default for CaptionStyle {
    fn default() -> Self {
        Self {
            font_size: 42,
            font_family: "Arial".to_string(),
            text_color: (255, 255, 255, 255),
            background_color: (0, 0, 0, 200),
            edge_style: EdgeStyle::Outline,
            edge_color: (0, 0, 0, 255),
            opacity: 255,
        }
    }
}

impl CaptionStyle {
    /// Create from preset.
    #[must_use]
    pub fn from_preset(preset: CaptionStylePreset) -> Self {
        match preset {
            CaptionStylePreset::Standard => Self::default(),
            CaptionStylePreset::HighContrast => Self {
                text_color: (255, 255, 255, 255),
                background_color: (0, 0, 0, 255),
                edge_style: EdgeStyle::Outline,
                edge_color: (0, 0, 0, 255),
                ..Default::default()
            },
            CaptionStylePreset::YellowOnBlack => Self {
                text_color: (255, 255, 0, 255),
                background_color: (0, 0, 0, 200),
                edge_style: EdgeStyle::None,
                ..Default::default()
            },
            CaptionStylePreset::BlackOnWhite => Self {
                text_color: (0, 0, 0, 255),
                background_color: (255, 255, 255, 200),
                edge_style: EdgeStyle::None,
                ..Default::default()
            },
            CaptionStylePreset::Transparent => Self {
                background_color: (0, 0, 0, 0),
                edge_style: EdgeStyle::DropShadow,
                ..Default::default()
            },
        }
    }

    /// Set font size.
    #[must_use]
    pub const fn with_font_size(mut self, size: u32) -> Self {
        self.font_size = size;
        self
    }

    /// Set text color.
    #[must_use]
    pub const fn with_text_color(mut self, r: u8, g: u8, b: u8, a: u8) -> Self {
        self.text_color = (r, g, b, a);
        self
    }

    /// Set background color.
    #[must_use]
    pub const fn with_background_color(mut self, r: u8, g: u8, b: u8, a: u8) -> Self {
        self.background_color = (r, g, b, a);
        self
    }

    /// Set edge style.
    #[must_use]
    pub const fn with_edge_style(mut self, style: EdgeStyle) -> Self {
        self.edge_style = style;
        self
    }

    /// Convert to subtitle style.
    #[must_use]
    pub fn to_subtitle_style(&self) -> SubtitleStyle {
        SubtitleStyle::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_style() {
        let style = CaptionStyle::default();
        assert_eq!(style.font_size, 42);
        assert_eq!(style.text_color, (255, 255, 255, 255));
    }

    #[test]
    fn test_preset_styles() {
        let standard = CaptionStyle::from_preset(CaptionStylePreset::Standard);
        let high_contrast = CaptionStyle::from_preset(CaptionStylePreset::HighContrast);

        assert_eq!(high_contrast.background_color.3, 255);
        assert_eq!(standard.text_color, (255, 255, 255, 255));
    }

    #[test]
    fn test_style_builder() {
        let style = CaptionStyle::default()
            .with_font_size(48)
            .with_text_color(255, 0, 0, 255)
            .with_edge_style(EdgeStyle::DropShadow);

        assert_eq!(style.font_size, 48);
        assert_eq!(style.text_color, (255, 0, 0, 255));
        assert_eq!(style.edge_style, EdgeStyle::DropShadow);
    }
}
