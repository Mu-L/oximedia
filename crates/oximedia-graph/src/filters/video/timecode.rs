//! Timecode and metadata burn-in filter.
//!
//! This filter overlays timecode and metadata information onto video frames
//! with support for multiple formats, positions, and styling options.
//!
//! # Overview
//!
//! The timecode filter provides professional-grade burn-in capabilities for video
//! post-production, broadcast, and quality control workflows. It supports:
//!
//! - Multiple SMPTE timecode formats (23.976, 24, 25, 29.97 DF/NDF, 30, 60)
//! - Alternative time representations (frame count, milliseconds, HH:MM:SS)
//! - Flexible positioning with 9 preset positions plus custom coordinates
//! - Rich text styling (fonts, colors, backgrounds, outlines, shadows)
//! - Multiple simultaneous overlays with independent styling
//! - Progress bar visualization
//! - Safe area margins for broadcast compliance
//! - Template system for reusable configurations
//!
//! # Basic Usage
//!
//! ```ignore
//! use oximedia_graph::filters::video::{TimecodeFilter, TimecodeFormat, presets};
//! use oximedia_graph::node::NodeId;
//!
//! // Load a font (e.g., from system fonts)
//! let font_data = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf")?;
//!
//! // Create a simple timecode overlay using a preset
//! let config = presets::simple_timecode(TimecodeFormat::Smpte25);
//! let filter = TimecodeFilter::new(NodeId(0), "timecode", config, font_data)?;
//! ```
//!
//! # Advanced Usage
//!
//! ```ignore
//! use oximedia_graph::filters::video::{
//!     TimecodeFilter, TimecodeConfig, TimecodeFormat,
//!     MetadataField, OverlayElement, Position, TextStyle, Color,
//! };
//! use oximedia_graph::node::NodeId;
//!
//! // Create a custom configuration
//! let mut config = TimecodeConfig::new(TimecodeFormat::Smpte2997Df);
//!
//! // Customize text style
//! let mut style = TextStyle::default();
//! style.font_size = 32.0;
//! style.foreground = Color::yellow();
//! style.background = Color::new(0, 0, 0, 200);
//! style.draw_outline = true;
//! style.outline_width = 2.0;
//!
//! // Add multiple elements
//! config = config
//!     .with_element(
//!         OverlayElement::new(MetadataField::Timecode, Position::TopLeft)
//!             .with_style(style.clone())
//!     )
//!     .with_element(
//!         OverlayElement::new(MetadataField::FrameNumber, Position::TopRight)
//!             .with_style(style)
//!     );
//!
//! let font_data = std::fs::read("path/to/font.ttf")?;
//! let filter = TimecodeFilter::new(NodeId(0), "timecode", config, font_data)?;
//! ```
//!
//! # Presets
//!
//! The [`presets`] module provides ready-to-use configurations:
//!
//! - [`presets::simple_timecode`] - Basic timecode in top-left corner
//! - [`presets::full_metadata`] - Four-corner metadata display
//! - [`presets::broadcast_timecode`] - Broadcast-standard timecode
//! - [`presets::production_overlay`] - Comprehensive production metadata
//! - [`presets::qc_overlay`] - Quality control review overlay
//! - [`presets::streaming_overlay`] - Live streaming information
//! - [`presets::minimal_corner`] - Small, unobtrusive corner timecode
//!
//! # Templates
//!
//! The [`templates`] module provides reusable layout templates:
//!
//! - [`templates::four_corner_metadata`] - Four-corner layout
//! - [`templates::center_focused`] - Center-focused display
//! - [`templates::top_bar`] - Top bar with multiple fields
//!
//! # Supported Metadata Fields
//!
//! The filter can display various metadata fields via [`MetadataField`]:
//!
//! - `Timecode` - SMPTE timecode or alternative time format
//! - `FrameNumber` - Sequential frame count
//! - `Filename` - Source filename
//! - `Resolution` - Frame resolution (e.g., "1920x1080")
//! - `Framerate` - Frames per second
//! - `Codec` - Codec information
//! - `Bitrate` - Current bitrate in Mbps
//! - `Date` - Current date (YYYY-MM-DD)
//! - `Time` - Current time (HH:MM:SS)
//! - `Custom(String)` - Custom static text
//!
//! # Performance Considerations
//!
//! - Glyph caching: The filter caches rasterized glyphs for better performance
//! - Font size: Larger font sizes require more rendering time
//! - Number of elements: Each overlay element adds processing overhead
//! - Pixel format: RGB formats are faster than YUV for compositing
//!
//! # Thread Safety
//!
//! The filter is designed to be used in a multi-threaded filter graph. Internal
//! state is managed safely, and the glyph cache is contained within the filter
//! instance.
//!
//! # Error Handling
//!
//! The filter returns [`GraphResult`] for operations that can fail, such as:
//!
//! - Font loading errors
//! - Invalid configuration
//! - State transition errors
//!
//! # Compatibility
//!
//! The filter works with all common pixel formats:
//!
//! - RGB24, RGBA32 (fastest compositing)
//! - YUV420p, YUV422p, YUV444p (with color space conversion)
//! - Other formats via fallback conversion

#![forbid(unsafe_code)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::similar_names)]
#![allow(clippy::many_single_char_names)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::unused_self)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::bool_to_int_with_if)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::no_effect_underscore_binding)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::too_many_arguments)]
#![allow(dead_code)]

use fontdue::{Font, FontSettings};
use std::collections::HashMap;

use crate::error::{GraphError, GraphResult};
use crate::frame::FilterFrame;
use crate::node::{Node, NodeId, NodeState, NodeType};
use crate::port::{InputPort, OutputPort, PortFormat, PortId, PortType, VideoPortFormat};
use oximedia_codec::{Plane, VideoFrame};
use oximedia_core::{PixelFormat, Rational};

/// SMPTE timecode format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimecodeFormat {
    /// SMPTE timecode at 23.976 fps (non-drop-frame).
    Smpte23976,
    /// SMPTE timecode at 24 fps.
    Smpte24,
    /// SMPTE timecode at 25 fps (PAL).
    Smpte25,
    /// SMPTE timecode at 29.97 fps drop-frame.
    Smpte2997Df,
    /// SMPTE timecode at 29.97 fps non-drop-frame.
    Smpte2997Ndf,
    /// SMPTE timecode at 30 fps.
    Smpte30,
    /// SMPTE timecode at 60 fps.
    Smpte60,
    /// Frame count (0, 1, 2, ...).
    FrameCount,
    /// Milliseconds (0, 16, 33, ...).
    Milliseconds,
    /// Seconds (0.000, 0.016, 0.033, ...).
    Seconds,
    /// HH:MM:SS.mmm format.
    HhMmSsMmm,
}

impl TimecodeFormat {
    /// Get the frame rate for this timecode format.
    #[must_use]
    pub fn framerate(&self) -> Rational {
        match self {
            Self::Smpte23976 => Rational::new(24000, 1001),
            Self::Smpte24 => Rational::new(24, 1),
            Self::Smpte25 => Rational::new(25, 1),
            Self::Smpte2997Df | Self::Smpte2997Ndf => Rational::new(30000, 1001),
            Self::Smpte30 => Rational::new(30, 1),
            Self::Smpte60 => Rational::new(60, 1),
            Self::FrameCount | Self::Milliseconds | Self::Seconds | Self::HhMmSsMmm => {
                Rational::new(1, 1)
            }
        }
    }

    /// Check if this format uses drop-frame timecode.
    #[must_use]
    pub fn is_drop_frame(&self) -> bool {
        matches!(self, Self::Smpte2997Df)
    }

    /// Format a frame number as timecode.
    #[must_use]
    pub fn format_timecode(&self, frame_number: u64, fps: &Rational) -> String {
        match self {
            Self::Smpte23976
            | Self::Smpte24
            | Self::Smpte25
            | Self::Smpte2997Ndf
            | Self::Smpte30
            | Self::Smpte60 => {
                let fps_val = fps.to_f64();
                let total_frames = frame_number;
                let hours = total_frames / (fps_val * 3600.0) as u64;
                let minutes = (total_frames / (fps_val * 60.0) as u64) % 60;
                let seconds = (total_frames / fps_val as u64) % 60;
                let frames = total_frames % fps_val as u64;
                format!("{hours:02}:{minutes:02}:{seconds:02}:{frames:02}")
            }
            Self::Smpte2997Df => {
                let fps_val = 30.0;
                let drop_frames = 2;
                let frames_per_minute = (fps_val * 60.0) as u64 - drop_frames;
                let frames_per_10_minutes = frames_per_minute * 10 + drop_frames;

                let mut total_frames = frame_number;
                let tens_of_minutes = total_frames / frames_per_10_minutes;
                total_frames %= frames_per_10_minutes;

                let mut minutes = if total_frames < drop_frames {
                    0
                } else {
                    (total_frames - drop_frames) / frames_per_minute + 1
                };
                let mut frames_in_minute = if total_frames < drop_frames {
                    total_frames
                } else {
                    (total_frames - drop_frames) % frames_per_minute + drop_frames
                };

                let hours = (tens_of_minutes * 10 + minutes) / 60;
                minutes = (tens_of_minutes * 10 + minutes) % 60;
                let seconds = frames_in_minute / fps_val as u64;
                frames_in_minute %= fps_val as u64;

                format!(
                    "{:02}:{:02}:{:02};{:02}",
                    hours, minutes, seconds, frames_in_minute
                )
            }
            Self::FrameCount => format!("{frame_number}"),
            Self::Milliseconds => {
                let ms = (frame_number as f64 / fps.to_f64() * 1000.0) as u64;
                format!("{ms}")
            }
            Self::Seconds => {
                let seconds = frame_number as f64 / fps.to_f64();
                format!("{seconds:.3}")
            }
            Self::HhMmSsMmm => {
                let total_ms = (frame_number as f64 / fps.to_f64() * 1000.0) as u64;
                let hours = total_ms / 3_600_000;
                let minutes = (total_ms / 60_000) % 60;
                let seconds = (total_ms / 1_000) % 60;
                let milliseconds = total_ms % 1_000;
                format!("{hours:02}:{minutes:02}:{seconds:02}.{milliseconds:03}")
            }
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for TimecodeFormat {
    fn default() -> Self {
        Self::Smpte25
    }
}

/// Position for overlay elements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    /// Top-left corner.
    TopLeft,
    /// Top-center.
    TopCenter,
    /// Top-right corner.
    TopRight,
    /// Center-left.
    CenterLeft,
    /// Center.
    Center,
    /// Center-right.
    CenterRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom-center.
    BottomCenter,
    /// Bottom-right corner.
    BottomRight,
    /// Custom X/Y coordinates (absolute pixels).
    Custom(i32, i32),
}

impl Position {
    /// Calculate the actual position in pixels.
    #[must_use]
    pub fn calculate(
        &self,
        frame_width: u32,
        frame_height: u32,
        element_width: u32,
        element_height: u32,
        margin: u32,
    ) -> (i32, i32) {
        match self {
            Self::TopLeft => (margin as i32, margin as i32),
            Self::TopCenter => (
                (frame_width.saturating_sub(element_width) / 2) as i32,
                margin as i32,
            ),
            Self::TopRight => (
                (frame_width
                    .saturating_sub(element_width)
                    .saturating_sub(margin)) as i32,
                margin as i32,
            ),
            Self::CenterLeft => (
                margin as i32,
                (frame_height.saturating_sub(element_height) / 2) as i32,
            ),
            Self::Center => (
                (frame_width.saturating_sub(element_width) / 2) as i32,
                (frame_height.saturating_sub(element_height) / 2) as i32,
            ),
            Self::CenterRight => (
                (frame_width
                    .saturating_sub(element_width)
                    .saturating_sub(margin)) as i32,
                (frame_height.saturating_sub(element_height) / 2) as i32,
            ),
            Self::BottomLeft => (
                margin as i32,
                (frame_height
                    .saturating_sub(element_height)
                    .saturating_sub(margin)) as i32,
            ),
            Self::BottomCenter => (
                (frame_width.saturating_sub(element_width) / 2) as i32,
                (frame_height
                    .saturating_sub(element_height)
                    .saturating_sub(margin)) as i32,
            ),
            Self::BottomRight => (
                (frame_width
                    .saturating_sub(element_width)
                    .saturating_sub(margin)) as i32,
                (frame_height
                    .saturating_sub(element_height)
                    .saturating_sub(margin)) as i32,
            ),
            Self::Custom(x, y) => (*x, *y),
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for Position {
    fn default() -> Self {
        Self::TopLeft
    }
}

/// Color representation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    /// Red component (0-255).
    pub r: u8,
    /// Green component (0-255).
    pub g: u8,
    /// Blue component (0-255).
    pub b: u8,
    /// Alpha component (0-255, 255 = opaque).
    pub a: u8,
}

impl Color {
    /// Create a new color.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Create a fully opaque color.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    /// White color.
    #[must_use]
    pub const fn white() -> Self {
        Self::rgb(255, 255, 255)
    }

    /// Black color.
    #[must_use]
    pub const fn black() -> Self {
        Self::rgb(0, 0, 0)
    }

    /// Transparent color.
    #[must_use]
    pub const fn transparent() -> Self {
        Self::new(0, 0, 0, 0)
    }

    /// Yellow color.
    #[must_use]
    pub const fn yellow() -> Self {
        Self::rgb(255, 255, 0)
    }

    /// Red color.
    #[must_use]
    pub const fn red() -> Self {
        Self::rgb(255, 0, 0)
    }

    /// Green color.
    #[must_use]
    pub const fn green() -> Self {
        Self::rgb(0, 255, 0)
    }

    /// Blue color.
    #[must_use]
    pub const fn blue() -> Self {
        Self::rgb(0, 0, 255)
    }
}

/// Text styling options.
#[derive(Clone, Debug)]
pub struct TextStyle {
    /// Font size in points.
    pub font_size: f32,
    /// Foreground color.
    pub foreground: Color,
    /// Background color.
    pub background: Color,
    /// Outline color.
    pub outline: Color,
    /// Outline width in pixels.
    pub outline_width: f32,
    /// Background box padding in pixels.
    pub padding: u32,
    /// Drop shadow offset (x, y) in pixels.
    pub shadow_offset: (i32, i32),
    /// Drop shadow color.
    pub shadow_color: Color,
    /// Enable background box.
    pub draw_background: bool,
    /// Enable outline.
    pub draw_outline: bool,
    /// Enable drop shadow.
    pub draw_shadow: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_size: 24.0,
            foreground: Color::white(),
            background: Color::new(0, 0, 0, 192),
            outline: Color::black(),
            outline_width: 1.0,
            padding: 4,
            shadow_offset: (2, 2),
            shadow_color: Color::new(0, 0, 0, 128),
            draw_background: true,
            draw_outline: false,
            draw_shadow: true,
        }
    }
}

/// Metadata field type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MetadataField {
    /// Timecode.
    Timecode,
    /// Frame number.
    FrameNumber,
    /// Filename.
    Filename,
    /// Resolution (e.g., "1920x1080").
    Resolution,
    /// Framerate (e.g., "25.00 fps").
    Framerate,
    /// Codec information.
    Codec,
    /// Current bitrate.
    Bitrate,
    /// Current date.
    Date,
    /// Current time.
    Time,
    /// Custom text field.
    Custom(String),
}

impl MetadataField {
    /// Get the display value for this field.
    #[must_use]
    pub fn value(&self, context: &FrameContext) -> String {
        match self {
            Self::Timecode => context.timecode.clone(),
            Self::FrameNumber => format!("Frame: {}", context.frame_number),
            Self::Filename => context.filename.clone(),
            Self::Resolution => format!("{}x{}", context.width, context.height),
            Self::Framerate => format!("{:.2} fps", context.framerate.to_f64()),
            Self::Codec => context.codec.clone(),
            Self::Bitrate => {
                if context.bitrate > 0 {
                    format!("{:.2} Mbps", context.bitrate as f64 / 1_000_000.0)
                } else {
                    "N/A".to_string()
                }
            }
            Self::Date => {
                // Simple date calculation (Gregorian calendar approximation)
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let days_since_epoch = now / 86400;

                // Calculate year, month, day (simplified algorithm)
                let mut year = 1970;
                let mut remaining_days = days_since_epoch;

                loop {
                    let days_in_year = if is_leap_year(year) { 366 } else { 365 };
                    if remaining_days < days_in_year {
                        break;
                    }
                    remaining_days -= days_in_year;
                    year += 1;
                }

                let mut month = 1;
                for m in 1..=12 {
                    let days_in_month = days_in_month_gregorian(m, year);
                    if remaining_days < days_in_month {
                        month = m;
                        break;
                    }
                    remaining_days -= days_in_month;
                }

                let day = remaining_days + 1;
                format!("{year:04}-{month:02}-{day:02}")
            }
            Self::Time => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let hours = (now / 3600) % 24;
                let minutes = (now / 60) % 60;
                let seconds = now % 60;
                format!("{hours:02}:{minutes:02}:{seconds:02}")
            }
            Self::Custom(text) => text.clone(),
        }
    }
}

/// Frame context for metadata templating.
#[derive(Clone, Debug)]
pub struct FrameContext {
    /// Current timecode string.
    pub timecode: String,
    /// Frame number.
    pub frame_number: u64,
    /// Filename.
    pub filename: String,
    /// Frame width.
    pub width: u32,
    /// Frame height.
    pub height: u32,
    /// Framerate.
    pub framerate: Rational,
    /// Codec name.
    pub codec: String,
    /// Current bitrate in bits per second.
    pub bitrate: u64,
}

impl Default for FrameContext {
    fn default() -> Self {
        Self {
            timecode: "00:00:00:00".to_string(),
            frame_number: 0,
            filename: "unknown.mp4".to_string(),
            width: 1920,
            height: 1080,
            framerate: Rational::new(25, 1),
            codec: "Unknown".to_string(),
            bitrate: 0,
        }
    }
}

/// A single overlay element.
#[derive(Clone, Debug)]
pub struct OverlayElement {
    /// Metadata field to display.
    pub field: MetadataField,
    /// Position on the frame.
    pub position: Position,
    /// Text styling.
    pub style: TextStyle,
    /// Element is enabled.
    pub enabled: bool,
}

impl OverlayElement {
    /// Create a new overlay element.
    #[must_use]
    pub fn new(field: MetadataField, position: Position) -> Self {
        Self {
            field,
            position,
            style: TextStyle::default(),
            enabled: true,
        }
    }

    /// Set the text style.
    #[must_use]
    pub fn with_style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    /// Enable or disable the element.
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Progress bar visualization.
#[derive(Clone, Debug)]
pub struct ProgressBar {
    /// Position on the frame.
    pub position: Position,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Foreground color.
    pub foreground: Color,
    /// Background color.
    pub background: Color,
    /// Total duration in frames.
    pub total_frames: u64,
    /// Show percentage text.
    pub show_percentage: bool,
    /// Enabled.
    pub enabled: bool,
}

impl Default for ProgressBar {
    fn default() -> Self {
        Self {
            position: Position::BottomCenter,
            width: 400,
            height: 8,
            foreground: Color::green(),
            background: Color::new(64, 64, 64, 192),
            total_frames: 1000,
            show_percentage: true,
            enabled: false,
        }
    }
}

/// Configuration for the timecode filter.
#[derive(Clone, Debug)]
pub struct TimecodeConfig {
    /// Timecode format.
    pub timecode_format: TimecodeFormat,
    /// Overlay elements.
    pub elements: Vec<OverlayElement>,
    /// Safe area margin in pixels.
    pub safe_margin: u32,
    /// Progress bar configuration.
    pub progress_bar: ProgressBar,
    /// Frame context for metadata.
    pub context: FrameContext,
}

impl Default for TimecodeConfig {
    fn default() -> Self {
        Self {
            timecode_format: TimecodeFormat::default(),
            elements: vec![
                OverlayElement::new(MetadataField::Timecode, Position::TopLeft),
                OverlayElement::new(MetadataField::FrameNumber, Position::TopRight),
            ],
            safe_margin: 10,
            progress_bar: ProgressBar::default(),
            context: FrameContext::default(),
        }
    }
}

impl TimecodeConfig {
    /// Create a new timecode configuration.
    #[must_use]
    pub fn new(format: TimecodeFormat) -> Self {
        Self {
            timecode_format: format,
            ..Default::default()
        }
    }

    /// Add an overlay element.
    #[must_use]
    pub fn with_element(mut self, element: OverlayElement) -> Self {
        self.elements.push(element);
        self
    }

    /// Set the safe area margin.
    #[must_use]
    pub fn with_safe_margin(mut self, margin: u32) -> Self {
        self.safe_margin = margin;
        self
    }

    /// Set the progress bar.
    #[must_use]
    pub fn with_progress_bar(mut self, progress_bar: ProgressBar) -> Self {
        self.progress_bar = progress_bar;
        self
    }

    /// Set the frame context.
    #[must_use]
    pub fn with_context(mut self, context: FrameContext) -> Self {
        self.context = context;
        self
    }
}

/// Timecode burn-in filter.
///
/// This filter overlays timecode and metadata information onto video frames.
pub struct TimecodeFilter {
    id: NodeId,
    name: String,
    state: NodeState,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
    config: TimecodeConfig,
    font: Font,
    frame_count: u64,
    /// Glyph cache for performance.
    glyph_cache: HashMap<(char, u32), CachedGlyph>,
}

/// Cached glyph for text rendering.
#[derive(Clone, Debug)]
struct CachedGlyph {
    bitmap: Vec<u8>,
    width: usize,
    height: usize,
    advance: f32,
    offset_x: f32,
    offset_y: f32,
}

impl TimecodeFilter {
    /// Create a new timecode filter with a provided font.
    ///
    /// # Errors
    ///
    /// Returns error if the font data is invalid.
    pub fn new(
        id: NodeId,
        name: impl Into<String>,
        config: TimecodeConfig,
        font_data: Vec<u8>,
    ) -> GraphResult<Self> {
        let font = Font::from_bytes(font_data.as_slice(), FontSettings::default())
            .map_err(|e| GraphError::ConfigurationError(format!("Invalid font data: {e}")))?;

        Ok(Self {
            id,
            name: name.into(),
            state: NodeState::Idle,
            inputs: vec![InputPort::new(PortId(0), "input", PortType::Video)
                .with_format(PortFormat::Video(VideoPortFormat::any()))],
            outputs: vec![OutputPort::new(PortId(0), "output", PortType::Video)
                .with_format(PortFormat::Video(VideoPortFormat::any()))],
            config,
            font,
            frame_count: 0,
            glyph_cache: HashMap::new(),
        })
    }

    /// Load a font from a file path and create the filter.
    ///
    /// # Errors
    ///
    /// Returns error if the font file cannot be read or is invalid.
    pub fn from_font_file(
        id: NodeId,
        name: impl Into<String>,
        config: TimecodeConfig,
        font_path: &str,
    ) -> GraphResult<Self> {
        let font_data = std::fs::read(font_path).map_err(|e| {
            GraphError::ConfigurationError(format!("Failed to read font file: {e}"))
        })?;
        Self::new(id, name, config, font_data)
    }

    /// Get the current configuration.
    #[must_use]
    pub fn config(&self) -> &TimecodeConfig {
        &self.config
    }

    /// Update the configuration.
    pub fn set_config(&mut self, config: TimecodeConfig) {
        self.config = config;
    }

    /// Update the frame context.
    pub fn set_context(&mut self, context: FrameContext) {
        self.config.context = context;
    }

    /// Get or cache a glyph.
    fn get_glyph(&mut self, c: char, font_size_int: u32) -> CachedGlyph {
        let key = (c, font_size_int);
        if let Some(cached) = self.glyph_cache.get(&key) {
            return cached.clone();
        }

        let font_size = font_size_int as f32;
        let (metrics, bitmap) = self.font.rasterize(c, font_size);

        let glyph = CachedGlyph {
            bitmap,
            width: metrics.width,
            height: metrics.height,
            advance: metrics.advance_width,
            offset_x: metrics.xmin as f32,
            offset_y: metrics.ymin as f32,
        };

        self.glyph_cache.insert(key, glyph.clone());
        glyph
    }

    /// Measure text dimensions.
    fn measure_text(&mut self, text: &str, font_size: f32) -> (u32, u32) {
        let font_size_int = font_size as u32;
        let mut width = 0.0f32;
        let mut max_height = 0.0f32;

        for c in text.chars() {
            let glyph = self.get_glyph(c, font_size_int);
            width += glyph.advance;
            max_height = max_height.max(glyph.height as f32);
        }

        (width.ceil() as u32, max_height.ceil() as u32)
    }

    /// Render text to an RGBA buffer.
    fn render_text(&mut self, text: &str, style: &TextStyle) -> (Vec<u8>, u32, u32) {
        let (text_width, text_height) = self.measure_text(text, style.font_size);
        let padding = style.padding;
        let buffer_width = text_width + padding * 2;
        let buffer_height = text_height + padding * 2;

        let mut buffer = vec![0u8; (buffer_width * buffer_height * 4) as usize];

        // Draw background
        if style.draw_background {
            for pixel in buffer.chunks_exact_mut(4) {
                pixel[0] = style.background.r;
                pixel[1] = style.background.g;
                pixel[2] = style.background.b;
                pixel[3] = style.background.a;
            }
        }

        // Render glyphs
        let font_size_int = style.font_size as u32;
        let mut x_pos = padding as f32;
        let baseline = (padding + text_height) as f32;

        for c in text.chars() {
            let glyph = self.get_glyph(c, font_size_int);

            // Calculate glyph position
            let glyph_x = (x_pos + glyph.offset_x) as i32;
            let glyph_y = (baseline - glyph.height as f32 - glyph.offset_y) as i32;

            // Draw shadow
            if style.draw_shadow {
                self.draw_glyph_to_buffer(
                    &glyph.bitmap,
                    glyph.width,
                    glyph.height,
                    &mut buffer,
                    buffer_width,
                    buffer_height,
                    glyph_x + style.shadow_offset.0,
                    glyph_y + style.shadow_offset.1,
                    style.shadow_color,
                );
            }

            // Draw outline
            if style.draw_outline && style.outline_width > 0.0 {
                let outline_width = style.outline_width as i32;
                for dx in -outline_width..=outline_width {
                    for dy in -outline_width..=outline_width {
                        if dx != 0 || dy != 0 {
                            self.draw_glyph_to_buffer(
                                &glyph.bitmap,
                                glyph.width,
                                glyph.height,
                                &mut buffer,
                                buffer_width,
                                buffer_height,
                                glyph_x + dx,
                                glyph_y + dy,
                                style.outline,
                            );
                        }
                    }
                }
            }

            // Draw foreground
            self.draw_glyph_to_buffer(
                &glyph.bitmap,
                glyph.width,
                glyph.height,
                &mut buffer,
                buffer_width,
                buffer_height,
                glyph_x,
                glyph_y,
                style.foreground,
            );

            x_pos += glyph.advance;
        }

        (buffer, buffer_width, buffer_height)
    }

    /// Draw a glyph to an RGBA buffer.
    #[allow(clippy::too_many_arguments)]
    fn draw_glyph_to_buffer(
        &self,
        glyph_bitmap: &[u8],
        glyph_width: usize,
        glyph_height: usize,
        buffer: &mut [u8],
        buffer_width: u32,
        buffer_height: u32,
        x: i32,
        y: i32,
        color: Color,
    ) {
        for gy in 0..glyph_height {
            for gx in 0..glyph_width {
                let bx = x + gx as i32;
                let by = y + gy as i32;

                if bx < 0 || by < 0 || bx >= buffer_width as i32 || by >= buffer_height as i32 {
                    continue;
                }

                let glyph_alpha = glyph_bitmap[gy * glyph_width + gx];
                if glyph_alpha == 0 {
                    continue;
                }

                let buffer_idx = ((by as u32 * buffer_width + bx as u32) * 4) as usize;
                let alpha = (glyph_alpha as f32 / 255.0) * (color.a as f32 / 255.0);

                // Alpha blending
                let existing_alpha = buffer[buffer_idx + 3] as f32 / 255.0;
                let out_alpha = alpha + existing_alpha * (1.0 - alpha);

                if out_alpha > 0.0 {
                    buffer[buffer_idx] = ((color.r as f32 * alpha
                        + buffer[buffer_idx] as f32 * existing_alpha * (1.0 - alpha))
                        / out_alpha) as u8;
                    buffer[buffer_idx + 1] = ((color.g as f32 * alpha
                        + buffer[buffer_idx + 1] as f32 * existing_alpha * (1.0 - alpha))
                        / out_alpha) as u8;
                    buffer[buffer_idx + 2] = ((color.b as f32 * alpha
                        + buffer[buffer_idx + 2] as f32 * existing_alpha * (1.0 - alpha))
                        / out_alpha) as u8;
                    buffer[buffer_idx + 3] = (out_alpha * 255.0) as u8;
                }
            }
        }
    }

    /// Composite RGBA buffer onto a video frame.
    fn composite_rgba_to_frame(
        &self,
        frame: &mut VideoFrame,
        rgba_buffer: &[u8],
        buffer_width: u32,
        buffer_height: u32,
        x: i32,
        y: i32,
    ) {
        // Handle different pixel formats
        match frame.format {
            PixelFormat::Rgb24 | PixelFormat::Rgba32 => {
                self.composite_to_rgb(frame, rgba_buffer, buffer_width, buffer_height, x, y);
            }
            PixelFormat::Yuv420p | PixelFormat::Yuv422p | PixelFormat::Yuv444p => {
                self.composite_to_yuv(frame, rgba_buffer, buffer_width, buffer_height, x, y);
            }
            _ => {
                // Fallback: convert to YUV
                self.composite_to_yuv(frame, rgba_buffer, buffer_width, buffer_height, x, y);
            }
        }
    }

    /// Composite to RGB frame.
    fn composite_to_rgb(
        &self,
        frame: &mut VideoFrame,
        rgba_buffer: &[u8],
        buffer_width: u32,
        buffer_height: u32,
        x: i32,
        y: i32,
    ) {
        if frame.planes.is_empty() {
            return;
        }

        let plane = &frame.planes[0];
        let mut data = plane.data.to_vec();
        let bpp = if frame.format == PixelFormat::Rgba32 {
            4
        } else {
            3
        };

        for by in 0..buffer_height {
            for bx in 0..buffer_width {
                let fx = x + bx as i32;
                let fy = y + by as i32;

                if fx < 0 || fy < 0 || fx >= frame.width as i32 || fy >= frame.height as i32 {
                    continue;
                }

                let buffer_idx = ((by * buffer_width + bx) * 4) as usize;
                let alpha = rgba_buffer[buffer_idx + 3] as f32 / 255.0;

                if alpha < 0.01 {
                    continue;
                }

                let frame_idx = ((fy as u32 * frame.width + fx as u32) * bpp) as usize;

                // Alpha blending
                for c in 0..3 {
                    let bg = data[frame_idx + c] as f32;
                    let fg = rgba_buffer[buffer_idx + c] as f32;
                    data[frame_idx + c] = (bg * (1.0 - alpha) + fg * alpha) as u8;
                }

                if bpp == 4 {
                    let bg_alpha = data[frame_idx + 3] as f32 / 255.0;
                    let out_alpha = alpha + bg_alpha * (1.0 - alpha);
                    data[frame_idx + 3] = (out_alpha * 255.0) as u8;
                }
            }
        }

        frame.planes[0] = Plane::new(data, plane.stride);
    }

    /// Composite to YUV frame.
    fn composite_to_yuv(
        &self,
        frame: &mut VideoFrame,
        rgba_buffer: &[u8],
        buffer_width: u32,
        buffer_height: u32,
        x: i32,
        y: i32,
    ) {
        let (h_sub, v_sub) = frame.format.chroma_subsampling();

        // Process Y plane
        if !frame.planes.is_empty() {
            let plane = &frame.planes[0];
            let mut y_data = plane.data.to_vec();

            for by in 0..buffer_height {
                for bx in 0..buffer_width {
                    let fx = x + bx as i32;
                    let fy = y + by as i32;

                    if fx < 0 || fy < 0 || fx >= frame.width as i32 || fy >= frame.height as i32 {
                        continue;
                    }

                    let buffer_idx = ((by * buffer_width + bx) * 4) as usize;
                    let alpha = rgba_buffer[buffer_idx + 3] as f32 / 255.0;

                    if alpha < 0.01 {
                        continue;
                    }

                    let r = rgba_buffer[buffer_idx] as f32;
                    let g = rgba_buffer[buffer_idx + 1] as f32;
                    let b = rgba_buffer[buffer_idx + 2] as f32;

                    // RGB to Y (BT.709)
                    let y_val = 0.2126 * r + 0.7152 * g + 0.0722 * b;

                    let frame_idx = fy as usize * frame.width as usize + fx as usize;
                    let bg = y_data[frame_idx] as f32;
                    y_data[frame_idx] = (bg * (1.0 - alpha) + y_val * alpha) as u8;
                }
            }

            frame.planes[0] = Plane::new(y_data, plane.stride);
        }

        // Process U and V planes
        if frame.planes.len() >= 3 {
            for plane_idx in 1..3 {
                let plane = &frame.planes[plane_idx];
                let mut chroma_data = plane.data.to_vec();
                let chroma_width = frame.width / h_sub;
                let chroma_height = frame.height / v_sub;

                for by in 0..buffer_height {
                    for bx in 0..buffer_width {
                        let fx = x + bx as i32;
                        let fy = y + by as i32;

                        if fx < 0 || fy < 0 || fx >= frame.width as i32 || fy >= frame.height as i32
                        {
                            continue;
                        }

                        let buffer_idx = ((by * buffer_width + bx) * 4) as usize;
                        let alpha = rgba_buffer[buffer_idx + 3] as f32 / 255.0;

                        if alpha < 0.01 {
                            continue;
                        }

                        let cx = fx / h_sub as i32;
                        let cy = fy / v_sub as i32;

                        if cx < 0
                            || cy < 0
                            || cx >= chroma_width as i32
                            || cy >= chroma_height as i32
                        {
                            continue;
                        }

                        let r = rgba_buffer[buffer_idx] as f32;
                        let g = rgba_buffer[buffer_idx + 1] as f32;
                        let b = rgba_buffer[buffer_idx + 2] as f32;

                        // RGB to UV (BT.709)
                        let chroma_val = if plane_idx == 1 {
                            // U
                            -0.1146 * r - 0.3854 * g + 0.5 * b + 128.0
                        } else {
                            // V
                            0.5 * r - 0.4542 * g - 0.0458 * b + 128.0
                        };

                        let chroma_idx = cy as usize * chroma_width as usize + cx as usize;
                        let bg = chroma_data[chroma_idx] as f32;
                        chroma_data[chroma_idx] =
                            (bg * (1.0 - alpha) + chroma_val * alpha).clamp(0.0, 255.0) as u8;
                    }
                }

                frame.planes[plane_idx] = Plane::new(chroma_data, plane.stride);
            }
        }
    }

    /// Draw progress bar on frame.
    fn draw_progress_bar(&self, frame: &mut VideoFrame, current_frame: u64) {
        if !self.config.progress_bar.enabled || self.config.progress_bar.total_frames == 0 {
            return;
        }

        let bar = &self.config.progress_bar;
        let progress = (current_frame as f64 / bar.total_frames as f64).clamp(0.0, 1.0);
        let filled_width = (bar.width as f64 * progress) as u32;

        let (x, y) = bar.position.calculate(
            frame.width,
            frame.height,
            bar.width,
            bar.height,
            self.config.safe_margin,
        );

        // Create progress bar buffer
        let mut buffer = vec![0u8; (bar.width * bar.height * 4) as usize];

        // Fill background
        for pixel in buffer.chunks_exact_mut(4) {
            pixel[0] = bar.background.r;
            pixel[1] = bar.background.g;
            pixel[2] = bar.background.b;
            pixel[3] = bar.background.a;
        }

        // Fill progress
        for py in 0..bar.height {
            for px in 0..filled_width {
                let idx = ((py * bar.width + px) * 4) as usize;
                buffer[idx] = bar.foreground.r;
                buffer[idx + 1] = bar.foreground.g;
                buffer[idx + 2] = bar.foreground.b;
                buffer[idx + 3] = bar.foreground.a;
            }
        }

        self.composite_rgba_to_frame(frame, &buffer, bar.width, bar.height, x, y);
    }

    /// Process a frame and add overlays.
    fn process_frame(&mut self, mut frame: VideoFrame) -> VideoFrame {
        // Update frame context
        self.config.context.timecode = self
            .config
            .timecode_format
            .format_timecode(self.frame_count, &self.config.context.framerate);
        self.config.context.frame_number = self.frame_count;
        self.config.context.width = frame.width;
        self.config.context.height = frame.height;

        // Draw progress bar
        self.draw_progress_bar(&mut frame, self.frame_count);

        // Draw overlay elements
        for element in &self.config.elements.clone() {
            if !element.enabled {
                continue;
            }

            let text = element.field.value(&self.config.context);
            let (rgba_buffer, buffer_width, buffer_height) =
                self.render_text(&text, &element.style);

            let (x, y) = element.position.calculate(
                frame.width,
                frame.height,
                buffer_width,
                buffer_height,
                self.config.safe_margin,
            );

            self.composite_rgba_to_frame(
                &mut frame,
                &rgba_buffer,
                buffer_width,
                buffer_height,
                x,
                y,
            );
        }

        self.frame_count += 1;
        frame
    }
}

impl Node for TimecodeFilter {
    fn id(&self) -> NodeId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn node_type(&self) -> NodeType {
        NodeType::Filter
    }

    fn state(&self) -> NodeState {
        self.state
    }

    fn set_state(&mut self, state: NodeState) -> GraphResult<()> {
        if !self.state.can_transition_to(state) {
            return Err(GraphError::InvalidStateTransition {
                node: self.id,
                from: self.state.to_string(),
                to: state.to_string(),
            });
        }
        self.state = state;
        Ok(())
    }

    fn inputs(&self) -> &[InputPort] {
        &self.inputs
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn process(&mut self, input: Option<FilterFrame>) -> GraphResult<Option<FilterFrame>> {
        match input {
            Some(FilterFrame::Video(frame)) => {
                let processed = self.process_frame(frame);
                Ok(Some(FilterFrame::Video(processed)))
            }
            Some(_) => Err(GraphError::PortTypeMismatch {
                expected: "Video".to_string(),
                actual: "Audio".to_string(),
            }),
            None => Ok(None),
        }
    }

    fn reset(&mut self) -> GraphResult<()> {
        self.frame_count = 0;
        self.glyph_cache.clear();
        self.set_state(NodeState::Idle)
    }
}

/// Template for complex metadata layouts.
#[derive(Clone, Debug)]
pub struct MetadataTemplate {
    /// Template name.
    pub name: String,
    /// Template elements.
    pub elements: Vec<OverlayElement>,
    /// Safe area margin.
    pub safe_margin: u32,
    /// Enable progress bar.
    pub enable_progress: bool,
}

impl MetadataTemplate {
    /// Create a new template.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            elements: Vec::new(),
            safe_margin: 10,
            enable_progress: false,
        }
    }

    /// Add an element to the template.
    #[must_use]
    pub fn with_element(mut self, element: OverlayElement) -> Self {
        self.elements.push(element);
        self
    }

    /// Set safe margin.
    #[must_use]
    pub fn with_safe_margin(mut self, margin: u32) -> Self {
        self.safe_margin = margin;
        self
    }

    /// Enable progress bar.
    #[must_use]
    pub fn with_progress(mut self, enabled: bool) -> Self {
        self.enable_progress = enabled;
        self
    }

    /// Apply this template to a config.
    #[must_use]
    pub fn apply_to_config(&self, mut config: TimecodeConfig) -> TimecodeConfig {
        config.elements = self.elements.clone();
        config.safe_margin = self.safe_margin;
        config.progress_bar.enabled = self.enable_progress;
        config
    }
}

/// Multi-line text overlay for complex layouts.
#[derive(Clone, Debug)]
pub struct MultiLineText {
    /// Lines of text.
    pub lines: Vec<String>,
    /// Position on the frame.
    pub position: Position,
    /// Text styling.
    pub style: TextStyle,
    /// Line spacing in pixels.
    pub line_spacing: f32,
    /// Alignment of text within the box.
    pub alignment: TextAlignment,
}

/// Text alignment within a box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextAlignment {
    /// Left-aligned.
    Left,
    /// Center-aligned.
    Center,
    /// Right-aligned.
    Right,
}

impl Default for MultiLineText {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            position: Position::TopLeft,
            style: TextStyle::default(),
            line_spacing: 4.0,
            alignment: TextAlignment::Left,
        }
    }
}

impl MultiLineText {
    /// Create a new multi-line text overlay.
    #[must_use]
    pub fn new(lines: Vec<String>, position: Position) -> Self {
        Self {
            lines,
            position,
            ..Default::default()
        }
    }

    /// Set the text style.
    #[must_use]
    pub fn with_style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    /// Set line spacing.
    #[must_use]
    pub fn with_line_spacing(mut self, spacing: f32) -> Self {
        self.line_spacing = spacing;
        self
    }

    /// Set text alignment.
    #[must_use]
    pub fn with_alignment(mut self, alignment: TextAlignment) -> Self {
        self.alignment = alignment;
        self
    }
}

/// Safe area visualization (for broadcast).
#[derive(Clone, Debug)]
pub struct SafeAreaOverlay {
    /// Action safe area percentage (typically 90%).
    pub action_safe: f32,
    /// Title safe area percentage (typically 80%).
    pub title_safe: f32,
    /// Line color.
    pub color: Color,
    /// Line width.
    pub line_width: u32,
    /// Enabled.
    pub enabled: bool,
}

impl Default for SafeAreaOverlay {
    fn default() -> Self {
        Self {
            action_safe: 0.9,
            title_safe: 0.8,
            color: Color::new(255, 255, 0, 128),
            line_width: 2,
            enabled: false,
        }
    }
}

/// Preset timecode configurations.
pub mod presets {
    use super::*;

    /// Simple timecode in top-left corner.
    #[must_use]
    pub fn simple_timecode(format: TimecodeFormat) -> TimecodeConfig {
        TimecodeConfig {
            timecode_format: format,
            elements: vec![OverlayElement::new(
                MetadataField::Timecode,
                Position::TopLeft,
            )],
            safe_margin: 10,
            progress_bar: ProgressBar::default(),
            context: FrameContext::default(),
        }
    }

    /// Full metadata overlay.
    #[must_use]
    pub fn full_metadata(format: TimecodeFormat) -> TimecodeConfig {
        TimecodeConfig {
            timecode_format: format,
            elements: vec![
                OverlayElement::new(MetadataField::Timecode, Position::TopLeft),
                OverlayElement::new(MetadataField::FrameNumber, Position::TopRight),
                OverlayElement::new(MetadataField::Resolution, Position::BottomLeft),
                OverlayElement::new(MetadataField::Framerate, Position::BottomRight),
            ],
            safe_margin: 10,
            progress_bar: ProgressBar::default(),
            context: FrameContext::default(),
        }
    }

    /// Broadcast-style timecode.
    #[must_use]
    pub fn broadcast_timecode(format: TimecodeFormat) -> TimecodeConfig {
        let style = TextStyle {
            font_size: 32.0,
            foreground: Color::yellow(),
            background: Color::new(0, 0, 0, 224),
            padding: 8,
            ..TextStyle::default()
        };

        TimecodeConfig {
            timecode_format: format,
            elements: vec![
                OverlayElement::new(MetadataField::Timecode, Position::TopCenter).with_style(style),
            ],
            safe_margin: 20,
            progress_bar: ProgressBar::default(),
            context: FrameContext::default(),
        }
    }

    /// Production overlay with comprehensive metadata.
    #[must_use]
    pub fn production_overlay(format: TimecodeFormat) -> TimecodeConfig {
        let tc_style = TextStyle {
            font_size: 28.0,
            foreground: Color::yellow(),
            background: Color::new(0, 0, 0, 200),
            padding: 6,
            ..TextStyle::default()
        };

        let meta_style = TextStyle {
            font_size: 18.0,
            foreground: Color::white(),
            background: Color::new(0, 0, 0, 180),
            padding: 4,
            ..TextStyle::default()
        };

        TimecodeConfig {
            timecode_format: format,
            elements: vec![
                OverlayElement::new(MetadataField::Timecode, Position::TopLeft)
                    .with_style(tc_style.clone()),
                OverlayElement::new(MetadataField::FrameNumber, Position::TopRight)
                    .with_style(meta_style.clone()),
                OverlayElement::new(MetadataField::Resolution, Position::BottomLeft)
                    .with_style(meta_style.clone()),
                OverlayElement::new(MetadataField::Framerate, Position::BottomRight)
                    .with_style(meta_style.clone()),
                OverlayElement::new(MetadataField::Codec, Position::Custom(10, 40))
                    .with_style(meta_style.clone()),
                OverlayElement::new(MetadataField::Bitrate, Position::Custom(10, 65))
                    .with_style(meta_style),
            ],
            safe_margin: 10,
            progress_bar: ProgressBar {
                enabled: true,
                ..ProgressBar::default()
            },
            context: FrameContext::default(),
        }
    }

    /// Minimal corner timecode (small and unobtrusive).
    #[must_use]
    pub fn minimal_corner(format: TimecodeFormat) -> TimecodeConfig {
        let style = TextStyle {
            font_size: 14.0,
            foreground: Color::new(255, 255, 255, 200),
            background: Color::new(0, 0, 0, 100),
            padding: 2,
            draw_shadow: false,
            ..TextStyle::default()
        };

        TimecodeConfig {
            timecode_format: format,
            elements: vec![
                OverlayElement::new(MetadataField::Timecode, Position::BottomRight)
                    .with_style(style),
            ],
            safe_margin: 5,
            progress_bar: ProgressBar::default(),
            context: FrameContext::default(),
        }
    }

    /// QC (Quality Control) overlay for review.
    #[must_use]
    pub fn qc_overlay(format: TimecodeFormat) -> TimecodeConfig {
        let tc_style = TextStyle {
            font_size: 36.0,
            foreground: Color::new(0, 255, 0, 255),
            background: Color::new(0, 0, 0, 220),
            padding: 10,
            draw_outline: true,
            outline_width: 2.0,
            ..TextStyle::default()
        };

        let meta_style = TextStyle {
            font_size: 20.0,
            foreground: Color::white(),
            background: Color::new(0, 0, 0, 200),
            padding: 5,
            ..TextStyle::default()
        };

        TimecodeConfig {
            timecode_format: format,
            elements: vec![
                OverlayElement::new(MetadataField::Timecode, Position::TopCenter)
                    .with_style(tc_style),
                OverlayElement::new(MetadataField::FrameNumber, Position::TopLeft)
                    .with_style(meta_style.clone()),
                OverlayElement::new(MetadataField::Filename, Position::TopRight)
                    .with_style(meta_style.clone()),
                OverlayElement::new(MetadataField::Resolution, Position::BottomLeft)
                    .with_style(meta_style.clone()),
                OverlayElement::new(MetadataField::Date, Position::BottomRight)
                    .with_style(meta_style),
            ],
            safe_margin: 15,
            progress_bar: ProgressBar {
                enabled: true,
                width: 600,
                height: 10,
                show_percentage: true,
                ..ProgressBar::default()
            },
            context: FrameContext::default(),
        }
    }

    /// Streaming overlay optimized for live streams.
    #[must_use]
    pub fn streaming_overlay(format: TimecodeFormat) -> TimecodeConfig {
        let time_style = TextStyle {
            font_size: 24.0,
            foreground: Color::new(255, 100, 100, 255),
            background: Color::new(0, 0, 0, 200),
            padding: 6,
            draw_shadow: true,
            ..TextStyle::default()
        };

        let info_style = TextStyle {
            font_size: 16.0,
            foreground: Color::new(200, 200, 255, 255),
            background: Color::new(0, 0, 0, 180),
            padding: 4,
            ..TextStyle::default()
        };

        TimecodeConfig {
            timecode_format: format,
            elements: vec![
                OverlayElement::new(MetadataField::Time, Position::TopRight).with_style(time_style),
                OverlayElement::new(MetadataField::Bitrate, Position::BottomRight)
                    .with_style(info_style.clone()),
                OverlayElement::new(MetadataField::Framerate, Position::BottomLeft)
                    .with_style(info_style),
            ],
            safe_margin: 10,
            progress_bar: ProgressBar::default(),
            context: FrameContext::default(),
        }
    }
}

/// Template presets for common use cases.
pub mod templates {
    use super::*;

    /// Create a basic four-corner metadata template.
    #[must_use]
    pub fn four_corner_metadata() -> MetadataTemplate {
        let meta_style = TextStyle::default();

        MetadataTemplate::new("FourCorner")
            .with_element(
                OverlayElement::new(MetadataField::Timecode, Position::TopLeft)
                    .with_style(meta_style.clone()),
            )
            .with_element(
                OverlayElement::new(MetadataField::FrameNumber, Position::TopRight)
                    .with_style(meta_style.clone()),
            )
            .with_element(
                OverlayElement::new(MetadataField::Resolution, Position::BottomLeft)
                    .with_style(meta_style.clone()),
            )
            .with_element(
                OverlayElement::new(MetadataField::Framerate, Position::BottomRight)
                    .with_style(meta_style),
            )
            .with_safe_margin(10)
    }

    /// Create a center-focused template.
    #[must_use]
    pub fn center_focused() -> MetadataTemplate {
        let large_style = TextStyle {
            font_size: 48.0,
            foreground: Color::white(),
            background: Color::new(0, 0, 0, 200),
            padding: 12,
            ..TextStyle::default()
        };

        MetadataTemplate::new("CenterFocused")
            .with_element(
                OverlayElement::new(MetadataField::Timecode, Position::Center)
                    .with_style(large_style),
            )
            .with_safe_margin(20)
    }

    /// Create a top bar template with multiple fields.
    #[must_use]
    pub fn top_bar() -> MetadataTemplate {
        let bar_style = TextStyle {
            font_size: 20.0,
            foreground: Color::white(),
            background: Color::new(0, 0, 0, 220),
            padding: 6,
            ..TextStyle::default()
        };

        MetadataTemplate::new("TopBar")
            .with_element(
                OverlayElement::new(MetadataField::Timecode, Position::TopLeft)
                    .with_style(bar_style.clone()),
            )
            .with_element(
                OverlayElement::new(MetadataField::FrameNumber, Position::TopCenter)
                    .with_style(bar_style.clone()),
            )
            .with_element(
                OverlayElement::new(MetadataField::Resolution, Position::TopRight)
                    .with_style(bar_style),
            )
            .with_safe_margin(5)
    }
}

// Note: The default font is loaded from a separate file.
// Users can provide their own font using the with_font() constructor.

/// Check if a year is a leap year.
fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Get the number of days in a month for a given year.
fn days_in_month_gregorian(month: u64, year: u64) -> u64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30, // Invalid month, return default
    }
}
