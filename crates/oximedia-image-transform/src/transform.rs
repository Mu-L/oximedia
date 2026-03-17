// SPDX-License-Identifier: Apache-2.0
// Copyright (c) COOLJAPAN OU (Team Kitasan)

//! Cloudflare Images-compatible transformation options.
//!
//! This module defines the complete set of image transformation parameters
//! that mirror Cloudflare Images' URL-based API.  Every parameter supported by
//! Cloudflare's `/cdn-cgi/image/` endpoint is represented here, including
//! resize, crop, quality, format, effects (blur, sharpen, brightness, contrast,
//! gamma), rotation, trim, DPR, metadata, animation, background, border,
//! padding, compression strategy, and error handling.
//!
//! # Example
//!
//! ```
//! use oximedia_image_transform::transform::{TransformParams, FitMode, OutputFormat, Rotation};
//!
//! let params = TransformParams {
//!     width: Some(800),
//!     height: Some(600),
//!     quality: 85,
//!     format: OutputFormat::Auto,
//!     fit: FitMode::Cover,
//!     rotate: Rotation::Auto,
//!     ..TransformParams::default()
//! };
//!
//! assert_eq!(params.effective_width(), Some(800));
//! assert!(params.validate().is_ok());
//! ```

use std::fmt;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum allowed dimension (width or height) in pixels.
pub const MAX_DIMENSION: u32 = 12000;

/// Default JPEG/WebP quality (1-100).
pub const DEFAULT_QUALITY: u8 = 85;

/// Maximum blur radius in pixels.
pub const MAX_BLUR_RADIUS: f64 = 250.0;

/// Maximum sharpen amount.
pub const MAX_SHARPEN: f64 = 10.0;

/// Maximum gamma value.
pub const MAX_GAMMA: f64 = 10.0;

/// Maximum device pixel ratio.
pub const MAX_DPR: f64 = 4.0;

/// Minimum device pixel ratio.
pub const MIN_DPR: f64 = 1.0;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when parsing or validating image transformation parameters.
#[derive(Debug, Error)]
pub enum TransformParseError {
    /// A parameter has an invalid value.
    #[error("invalid parameter: {name}={value}")]
    InvalidParameter {
        /// Parameter name.
        name: String,
        /// Invalid value that was provided.
        value: String,
    },

    /// A dimension exceeds the allowed maximum.
    #[error("invalid dimension: {0} exceeds maximum {1}")]
    DimensionTooLarge(u32, u32),

    /// Quality value is out of the 1-100 range.
    #[error("invalid quality: {0}, must be 1-100")]
    InvalidQuality(u8),

    /// A color string could not be parsed.
    #[error("invalid color: {0}")]
    InvalidColor(String),

    /// An unrecognised parameter name was encountered.
    #[error("unknown parameter: {0}")]
    UnknownParameter(String),

    /// An invalid output format string was provided.
    #[error("invalid format: {0}")]
    InvalidFormat(String),

    /// An invalid gravity/anchor string was provided.
    #[error("invalid gravity: {0}")]
    InvalidGravity(String),

    /// DPR value is outside the 1.0-4.0 range.
    #[error("invalid DPR: {0}, must be 1.0-4.0")]
    InvalidDpr(f64),

    /// The source image path is missing from the URL.
    #[error("missing required source path")]
    MissingSourcePath,

    /// Generic parse error with a descriptive message.
    #[error("parse error: {0}")]
    ParseError(String),

    /// A path-traversal or other security violation was detected.
    #[error("security violation: {0}")]
    SecurityViolation(String),
}

// ---------------------------------------------------------------------------
// FitMode
// ---------------------------------------------------------------------------

/// Fit mode for image resizing (Cloudflare Images compatible).
///
/// Controls how the source image is fitted into the requested width/height.
///
/// ```
/// use oximedia_image_transform::transform::FitMode;
///
/// let fit = FitMode::default();
/// assert_eq!(fit, FitMode::ScaleDown);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FitMode {
    /// Resize to fit within dimensions, preserving aspect ratio.
    /// Never enlarges — only shrinks. This is the Cloudflare default.
    ScaleDown,
    /// Resize to fit within dimensions, preserving aspect ratio.
    /// May letterbox if aspect ratios differ.
    Contain,
    /// Resize and crop to fill dimensions exactly, preserving aspect ratio.
    Cover,
    /// Same as [`Cover`](Self::Cover) but respects [`Gravity`] for crop position.
    Crop,
    /// Fit within dimensions and pad with [`background`](TransformParams::background) color.
    Pad,
    /// Stretch to exact dimensions, ignoring aspect ratio.
    Fill,
}

impl Default for FitMode {
    fn default() -> Self {
        Self::ScaleDown
    }
}

impl FitMode {
    /// Parse a fit mode string (case-insensitive).
    ///
    /// Accepts `"scale-down"`, `"scale_down"`, `"scaledown"`, `"contain"`,
    /// `"cover"`, `"crop"`, `"pad"`, and `"fill"`.
    ///
    /// ```
    /// use oximedia_image_transform::transform::FitMode;
    ///
    /// let fit = FitMode::from_str_loose("scale-down").expect("valid");
    /// assert_eq!(fit, FitMode::ScaleDown);
    ///
    /// let fit = FitMode::from_str_loose("Cover").expect("case insensitive");
    /// assert_eq!(fit, FitMode::Cover);
    /// ```
    pub fn from_str_loose(s: &str) -> Result<Self, TransformParseError> {
        match s.to_ascii_lowercase().as_str() {
            "scale-down" | "scale_down" | "scaledown" => Ok(Self::ScaleDown),
            "contain" => Ok(Self::Contain),
            "cover" => Ok(Self::Cover),
            "crop" => Ok(Self::Crop),
            "pad" => Ok(Self::Pad),
            "fill" => Ok(Self::Fill),
            _ => Err(TransformParseError::InvalidParameter {
                name: "fit".to_string(),
                value: s.to_string(),
            }),
        }
    }

    /// Canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ScaleDown => "scale-down",
            Self::Contain => "contain",
            Self::Cover => "cover",
            Self::Crop => "crop",
            Self::Pad => "pad",
            Self::Fill => "fill",
        }
    }
}

impl fmt::Display for FitMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Gravity
// ---------------------------------------------------------------------------

/// Gravity/anchor point for cropping.
///
/// When a [`FitMode::Cover`] or [`FitMode::Crop`] resize causes the image to be
/// cropped, the gravity determines which region of the source is preserved.
///
/// ```
/// use oximedia_image_transform::transform::Gravity;
///
/// let g = Gravity::from_str_loose("0.3x0.7").expect("focal point");
/// assert!(matches!(g, Gravity::FocalPoint(x, y) if (x - 0.3).abs() < 0.001));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Gravity {
    /// Content-aware automatic cropping (saliency-based).
    Auto,
    /// Center of the image (default).
    Center,
    /// Top edge center.
    Top,
    /// Bottom edge center.
    Bottom,
    /// Left edge center.
    Left,
    /// Right edge center.
    Right,
    /// Top-left corner.
    TopLeft,
    /// Top-right corner.
    TopRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom-right corner.
    BottomRight,
    /// Face-detection-based cropping.
    Face,
    /// Focal point as (x, y) normalised to 0.0..1.0.
    FocalPoint(f64, f64),
}

impl Default for Gravity {
    fn default() -> Self {
        Self::Center
    }
}

impl Gravity {
    /// Parse a gravity string.  Supports named values and `0.5x0.5` focal-point syntax.
    ///
    /// ```
    /// use oximedia_image_transform::transform::Gravity;
    ///
    /// assert_eq!(Gravity::from_str_loose("center").expect("valid"), Gravity::Center);
    /// assert_eq!(Gravity::from_str_loose("auto").expect("valid"), Gravity::Auto);
    /// assert_eq!(Gravity::from_str_loose("top-left").expect("valid"), Gravity::TopLeft);
    /// ```
    pub fn from_str_loose(s: &str) -> Result<Self, TransformParseError> {
        // Focal point syntax: "0.5x0.5"
        if let Some((lhs, rhs)) = s.split_once('x') {
            if let (Ok(x), Ok(y)) = (lhs.parse::<f64>(), rhs.parse::<f64>()) {
                if (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y) {
                    return Ok(Self::FocalPoint(x, y));
                }
                return Err(TransformParseError::InvalidGravity(s.to_string()));
            }
        }
        match s.to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "center" | "centre" => Ok(Self::Center),
            "top" => Ok(Self::Top),
            "bottom" => Ok(Self::Bottom),
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            "top-left" | "topleft" | "top_left" => Ok(Self::TopLeft),
            "top-right" | "topright" | "top_right" => Ok(Self::TopRight),
            "bottom-left" | "bottomleft" | "bottom_left" => Ok(Self::BottomLeft),
            "bottom-right" | "bottomright" | "bottom_right" => Ok(Self::BottomRight),
            "face" => Ok(Self::Face),
            _ => Err(TransformParseError::InvalidGravity(s.to_string())),
        }
    }

    /// Canonical string representation.
    pub fn as_str(&self) -> String {
        match self {
            Self::Auto => "auto".to_string(),
            Self::Center => "center".to_string(),
            Self::Top => "top".to_string(),
            Self::Bottom => "bottom".to_string(),
            Self::Left => "left".to_string(),
            Self::Right => "right".to_string(),
            Self::TopLeft => "top-left".to_string(),
            Self::TopRight => "top-right".to_string(),
            Self::BottomLeft => "bottom-left".to_string(),
            Self::BottomRight => "bottom-right".to_string(),
            Self::Face => "face".to_string(),
            Self::FocalPoint(x, y) => format!("{x}x{y}"),
        }
    }
}

impl fmt::Display for Gravity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ---------------------------------------------------------------------------
// OutputFormat
// ---------------------------------------------------------------------------

/// Output format for the transformed image.
///
/// ```
/// use oximedia_image_transform::transform::OutputFormat;
///
/// let fmt = OutputFormat::from_str_loose("webp").expect("valid format");
/// assert_eq!(fmt.mime_type(), "image/webp");
/// assert_eq!(fmt.file_extension(), "webp");
/// assert!(fmt.supports_transparency());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputFormat {
    /// Automatic format negotiation based on Accept header.
    Auto,
    /// AV1 Image File Format — best compression, modern browsers.
    Avif,
    /// WebP — good compression, wide browser support.
    WebP,
    /// JPEG — universal support, lossy.
    Jpeg,
    /// PNG — lossless, supports transparency.
    Png,
    /// GIF — animation support, limited colours.
    Gif,
    /// Baseline JPEG (non-progressive).
    Baseline,
    /// JSON metadata output (image dimensions, format info only).
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Auto
    }
}

impl OutputFormat {
    /// Returns the MIME type for this format.
    ///
    /// ```
    /// use oximedia_image_transform::transform::OutputFormat;
    /// assert_eq!(OutputFormat::Avif.mime_type(), "image/avif");
    /// assert_eq!(OutputFormat::Json.mime_type(), "application/json");
    /// ```
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Auto => "image/jpeg", // fallback
            Self::Avif => "image/avif",
            Self::WebP => "image/webp",
            Self::Jpeg | Self::Baseline => "image/jpeg",
            Self::Png => "image/png",
            Self::Gif => "image/gif",
            Self::Json => "application/json",
        }
    }

    /// Returns the file extension for this format.
    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Auto => "jpg",
            Self::Avif => "avif",
            Self::WebP => "webp",
            Self::Jpeg | Self::Baseline => "jpg",
            Self::Png => "png",
            Self::Gif => "gif",
            Self::Json => "json",
        }
    }

    /// Whether this format supports animation frames.
    pub fn supports_animation(&self) -> bool {
        matches!(self, Self::Gif | Self::WebP | Self::Avif)
    }

    /// Whether this format supports alpha transparency.
    pub fn supports_transparency(&self) -> bool {
        matches!(self, Self::Png | Self::WebP | Self::Avif | Self::Gif)
    }

    /// Parse a format string (case-insensitive).
    ///
    /// ```
    /// use oximedia_image_transform::transform::OutputFormat;
    ///
    /// assert_eq!(OutputFormat::from_str_loose("avif").expect("valid"), OutputFormat::Avif);
    /// assert_eq!(OutputFormat::from_str_loose("WEBP").expect("valid"), OutputFormat::WebP);
    /// assert_eq!(OutputFormat::from_str_loose("jpg").expect("valid"), OutputFormat::Jpeg);
    /// assert_eq!(OutputFormat::from_str_loose("json").expect("valid"), OutputFormat::Json);
    /// assert!(OutputFormat::from_str_loose("bmp").is_err());
    /// ```
    pub fn from_str_loose(s: &str) -> Result<Self, TransformParseError> {
        match s.to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "avif" => Ok(Self::Avif),
            "webp" => Ok(Self::WebP),
            "jpeg" | "jpg" => Ok(Self::Jpeg),
            "png" => Ok(Self::Png),
            "gif" => Ok(Self::Gif),
            "baseline" => Ok(Self::Baseline),
            "json" => Ok(Self::Json),
            _ => Err(TransformParseError::InvalidFormat(s.to_string())),
        }
    }

    /// Canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Avif => "avif",
            Self::WebP => "webp",
            Self::Jpeg => "jpeg",
            Self::Png => "png",
            Self::Gif => "gif",
            Self::Baseline => "baseline",
            Self::Json => "json",
        }
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// MetadataMode
// ---------------------------------------------------------------------------

/// Metadata preservation mode.
///
/// Controls which EXIF/XMP metadata is kept in the output image.
///
/// ```
/// use oximedia_image_transform::transform::MetadataMode;
/// let m = MetadataMode::default();
/// assert_eq!(m, MetadataMode::None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetadataMode {
    /// Keep all EXIF/XMP metadata.
    Keep,
    /// Strip all metadata (smallest output).
    None,
    /// Keep only copyright-related metadata.
    Copyright,
}

impl Default for MetadataMode {
    fn default() -> Self {
        Self::None
    }
}

impl MetadataMode {
    /// Parse a metadata mode string.
    ///
    /// ```
    /// use oximedia_image_transform::transform::MetadataMode;
    /// assert_eq!(MetadataMode::from_str_loose("keep").expect("valid"), MetadataMode::Keep);
    /// assert_eq!(MetadataMode::from_str_loose("strip").expect("valid"), MetadataMode::None);
    /// ```
    pub fn from_str_loose(s: &str) -> Result<Self, TransformParseError> {
        match s.to_ascii_lowercase().as_str() {
            "keep" | "all" => Ok(Self::Keep),
            "copyright" => Ok(Self::Copyright),
            "none" | "strip" => Ok(Self::None),
            _ => Err(TransformParseError::InvalidParameter {
                name: "metadata".to_string(),
                value: s.to_string(),
            }),
        }
    }

    /// Canonical string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Copyright => "copyright",
            Self::None => "none",
        }
    }
}

impl fmt::Display for MetadataMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Color (RGBA)
// ---------------------------------------------------------------------------

/// RGBA colour for background/border/padding fill.
///
/// ```
/// use oximedia_image_transform::transform::Color;
///
/// let c = Color::from_css("#ff8800").expect("valid color");
/// assert_eq!(c.r, 255);
/// assert_eq!(c.g, 136);
/// assert_eq!(c.b, 0);
/// assert_eq!(c.a, 255);
///
/// let c = Color::from_css("#ff880080").expect("valid color with alpha");
/// assert_eq!(c.a, 128);
///
/// let t = Color::transparent();
/// assert_eq!(t.a, 0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red channel (0-255).
    pub r: u8,
    /// Green channel (0-255).
    pub g: u8,
    /// Blue channel (0-255).
    pub b: u8,
    /// Alpha channel (0-255, 0 = fully transparent, 255 = fully opaque).
    pub a: u8,
}

impl Color {
    /// Create a new RGBA colour.
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Fully transparent black.
    pub fn transparent() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }
    }

    /// Opaque white.
    pub fn white() -> Self {
        Self {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        }
    }

    /// Opaque black.
    pub fn black() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }

    /// Parse a CSS-like colour string.
    ///
    /// Supported formats:
    /// - `#RRGGBB` / `RRGGBB` (alpha defaults to 255)
    /// - `#RRGGBBAA` / `RRGGBBAA`
    /// - `rgb(R,G,B)` where R, G, B are 0-255
    /// - `rgba(R,G,B,A)` where R, G, B are 0-255 and A is 0.0-1.0
    ///
    /// ```
    /// use oximedia_image_transform::transform::Color;
    ///
    /// let c = Color::from_css("#ff0000").expect("parse red");
    /// assert_eq!(c, Color::new(255, 0, 0, 255));
    ///
    /// let c = Color::from_css("00ff00").expect("parse green");
    /// assert_eq!(c, Color::new(0, 255, 0, 255));
    ///
    /// let c = Color::from_css("ff000080").expect("parse red alpha");
    /// assert_eq!(c, Color::new(255, 0, 0, 128));
    ///
    /// let c = Color::from_css("rgb(128,64,32)").expect("parse rgb");
    /// assert_eq!(c, Color::new(128, 64, 32, 255));
    ///
    /// let c = Color::from_css("rgba(128,64,32,0.5)").expect("parse rgba");
    /// assert_eq!(c.a, 128);
    /// ```
    pub fn from_css(s: &str) -> Result<Self, TransformParseError> {
        let s = s.trim();

        // Try rgb(...) / rgba(...)
        if let Some(inner) = s
            .strip_prefix("rgba(")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            return Self::parse_rgba_func(inner);
        }
        if let Some(inner) = s
            .strip_prefix("rgb(")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            return Self::parse_rgb_func(inner);
        }

        // Hex formats
        Self::from_hex(s)
    }

    /// Parse a hex colour string: `#RRGGBB`, `RRGGBB`, `#RRGGBBAA`, `RRGGBBAA`.
    pub fn from_hex(s: &str) -> Result<Self, TransformParseError> {
        let hex = s.strip_prefix('#').unwrap_or(s);
        match hex.len() {
            6 => {
                let r = parse_hex_pair(&hex[0..2], s)?;
                let g = parse_hex_pair(&hex[2..4], s)?;
                let b = parse_hex_pair(&hex[4..6], s)?;
                Ok(Self { r, g, b, a: 255 })
            }
            8 => {
                let r = parse_hex_pair(&hex[0..2], s)?;
                let g = parse_hex_pair(&hex[2..4], s)?;
                let b = parse_hex_pair(&hex[4..6], s)?;
                let a = parse_hex_pair(&hex[6..8], s)?;
                Ok(Self { r, g, b, a })
            }
            _ => Err(TransformParseError::InvalidColor(s.to_string())),
        }
    }

    /// Convert to hex string without `#` prefix (6 hex digits if fully opaque, 8 otherwise).
    pub fn to_hex(&self) -> String {
        if self.a == 255 {
            format!("{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    // -- private helpers --

    fn parse_rgb_func(inner: &str) -> Result<Self, TransformParseError> {
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() != 3 {
            return Err(TransformParseError::InvalidColor(format!("rgb({inner})")));
        }
        let r = parts[0]
            .parse::<u8>()
            .map_err(|_| TransformParseError::InvalidColor(format!("rgb({inner})")))?;
        let g = parts[1]
            .parse::<u8>()
            .map_err(|_| TransformParseError::InvalidColor(format!("rgb({inner})")))?;
        let b = parts[2]
            .parse::<u8>()
            .map_err(|_| TransformParseError::InvalidColor(format!("rgb({inner})")))?;
        Ok(Self { r, g, b, a: 255 })
    }

    fn parse_rgba_func(inner: &str) -> Result<Self, TransformParseError> {
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() != 4 {
            return Err(TransformParseError::InvalidColor(format!("rgba({inner})")));
        }
        let r = parts[0]
            .parse::<u8>()
            .map_err(|_| TransformParseError::InvalidColor(format!("rgba({inner})")))?;
        let g = parts[1]
            .parse::<u8>()
            .map_err(|_| TransformParseError::InvalidColor(format!("rgba({inner})")))?;
        let b = parts[2]
            .parse::<u8>()
            .map_err(|_| TransformParseError::InvalidColor(format!("rgba({inner})")))?;
        let alpha_f = parts[3]
            .parse::<f64>()
            .map_err(|_| TransformParseError::InvalidColor(format!("rgba({inner})")))?;
        let a = (alpha_f.clamp(0.0, 1.0) * 255.0).round() as u8;
        Ok(Self { r, g, b, a })
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.to_hex())
    }
}

/// Parse a two-character hex pair into a `u8`.
fn parse_hex_pair(pair: &str, original: &str) -> Result<u8, TransformParseError> {
    u8::from_str_radix(pair, 16)
        .map_err(|_| TransformParseError::InvalidColor(original.to_string()))
}

// Re-exported from `transform_types` module.
pub use crate::transform_types::{Border, Compression, OutputOptions, Padding, Rotation, Trim};

// ---------------------------------------------------------------------------
// TransformParams
// ---------------------------------------------------------------------------

/// Complete image transformation parameters (Cloudflare Images-compatible).
///
/// This struct carries every option that the Cloudflare Images URL API supports.
/// Sensible defaults are provided via [`Default`]; individual fields can be
/// overridden as needed.
///
/// # Example
///
/// ```
/// use oximedia_image_transform::transform::TransformParams;
///
/// let p = TransformParams::default();
/// assert_eq!(p.quality, 85);
/// assert!(p.effective_width().is_none()); // no width set
///
/// let mut p = TransformParams::default();
/// p.width = Some(800);
/// p.dpr = 2.0;
/// assert_eq!(p.effective_width(), Some(1600));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TransformParams {
    /// Desired output width in pixels (before DPR multiplication).
    pub width: Option<u32>,
    /// Desired output height in pixels (before DPR multiplication).
    pub height: Option<u32>,
    /// Output quality, 1-100 (default 85).
    pub quality: u8,
    /// Output image format.
    pub format: OutputFormat,
    /// How the image should fit into the target dimensions.
    pub fit: FitMode,
    /// Crop anchor point.
    pub gravity: Gravity,
    /// Sharpen amount (0.0-10.0, 0 = no sharpening).
    pub sharpen: f64,
    /// Gaussian blur radius in pixels (0.0-250.0, 0 = no blur).
    pub blur: f64,
    /// Brightness adjustment (-1.0 to 1.0, 0 = no change).
    pub brightness: f64,
    /// Contrast adjustment (-1.0 to 1.0, 0 = no change).
    pub contrast: f64,
    /// Gamma correction (>0.0, default 1.0 = no change).
    pub gamma: f64,
    /// Rotation.
    pub rotate: Rotation,
    /// Auto-trim edges specification.
    pub trim: Option<Trim>,
    /// Device pixel ratio multiplier (default 1.0, max 4.0).
    pub dpr: f64,
    /// Metadata preservation mode.
    pub metadata: MetadataMode,
    /// Whether to preserve animation frames (default `true`).
    pub anim: bool,
    /// Background colour for transparent images or padding.
    pub background: Color,
    /// Optional border around the image.
    pub border: Option<Border>,
    /// Optional padding (fractional).
    pub pad: Option<Padding>,
    /// Compression strategy hint.
    pub compression: Option<String>,
    /// Error-handling fallback behaviour (e.g. `"redirect"`).
    pub onerror: Option<String>,
    /// Optional per-request output encoding options (progressive JPEG, quality
    /// override, etc.). When absent, sensible defaults are used.
    pub output_options: Option<OutputOptions>,
}

impl Default for TransformParams {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            quality: DEFAULT_QUALITY,
            format: OutputFormat::Auto,
            fit: FitMode::ScaleDown,
            gravity: Gravity::Center,
            sharpen: 0.0,
            blur: 0.0,
            brightness: 0.0,
            contrast: 0.0,
            gamma: 1.0,
            rotate: Rotation::Deg0,
            trim: None,
            dpr: 1.0,
            metadata: MetadataMode::None,
            anim: true,
            background: Color::transparent(),
            border: None,
            pad: None,
            compression: None,
            onerror: None,
            output_options: None,
        }
    }
}

impl TransformParams {
    /// Get effective width after DPR multiplication.
    ///
    /// ```
    /// use oximedia_image_transform::transform::TransformParams;
    ///
    /// let mut p = TransformParams::default();
    /// p.width = Some(400);
    /// p.dpr = 2.0;
    /// assert_eq!(p.effective_width(), Some(800));
    /// ```
    pub fn effective_width(&self) -> Option<u32> {
        self.width.map(|w| {
            let scaled = (f64::from(w) * self.dpr).round() as u32;
            scaled.min(MAX_DIMENSION).max(1)
        })
    }

    /// Get effective height after DPR multiplication.
    ///
    /// ```
    /// use oximedia_image_transform::transform::TransformParams;
    ///
    /// let mut p = TransformParams::default();
    /// p.height = Some(300);
    /// p.dpr = 3.0;
    /// assert_eq!(p.effective_height(), Some(900));
    /// ```
    pub fn effective_height(&self) -> Option<u32> {
        self.height.map(|h| {
            let scaled = (f64::from(h) * self.dpr).round() as u32;
            scaled.min(MAX_DIMENSION).max(1)
        })
    }

    /// Returns `true` if the output should be encoded as a progressive JPEG.
    ///
    /// Progressive JPEG mode is enabled when:
    /// - `output_options` is set **and**
    /// - `output_options.progressive_jpeg` is `true` **and**
    /// - the output format is JPEG-compatible (`Jpeg`, `Baseline`, or `Auto`).
    ///
    /// For non-JPEG formats (WebP, AVIF, PNG, …) this always returns `false`
    /// because progressive encoding is JPEG-specific.
    ///
    /// # Example
    ///
    /// ```
    /// use oximedia_image_transform::transform::{OutputFormat, OutputOptions, TransformParams};
    ///
    /// let mut p = TransformParams::default();
    /// assert!(!p.is_progressive_output());
    ///
    /// p.output_options = Some(OutputOptions { progressive_jpeg: true, quality: 85 });
    /// // Default format is Auto, which is JPEG-compatible.
    /// assert!(p.is_progressive_output());
    ///
    /// p.format = OutputFormat::WebP;
    /// assert!(!p.is_progressive_output(), "progressive does not apply to WebP");
    /// ```
    pub fn is_progressive_output(&self) -> bool {
        let opts = match &self.output_options {
            Some(o) => o,
            None => return false,
        };
        if !opts.progressive_jpeg {
            return false;
        }
        // Progressive encoding is only meaningful for JPEG streams.
        matches!(
            self.format,
            OutputFormat::Jpeg | OutputFormat::Baseline | OutputFormat::Auto
        )
    }

    /// Validate all parameter ranges.
    ///
    /// Checks:
    /// - width/height in 1..=12000
    /// - quality in 1..=100
    /// - dpr in 1.0..=4.0
    /// - sharpen in 0.0..=10.0
    /// - blur in 0.0..=250.0
    /// - brightness in -1.0..=1.0
    /// - contrast in -1.0..=1.0
    /// - gamma in 0.0..=10.0
    /// - focal point coordinates in 0.0..=1.0
    ///
    /// ```
    /// use oximedia_image_transform::transform::TransformParams;
    ///
    /// let p = TransformParams::default();
    /// assert!(p.validate().is_ok());
    ///
    /// let mut p = TransformParams::default();
    /// p.width = Some(20000);
    /// assert!(p.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), TransformParseError> {
        // Dimensions
        if let Some(w) = self.width {
            if w == 0 || w > MAX_DIMENSION {
                return Err(TransformParseError::DimensionTooLarge(w, MAX_DIMENSION));
            }
        }
        if let Some(h) = self.height {
            if h == 0 || h > MAX_DIMENSION {
                return Err(TransformParseError::DimensionTooLarge(h, MAX_DIMENSION));
            }
        }

        // Quality
        if self.quality == 0 || self.quality > 100 {
            return Err(TransformParseError::InvalidQuality(self.quality));
        }

        // DPR
        if self.dpr < MIN_DPR || self.dpr > MAX_DPR {
            return Err(TransformParseError::InvalidDpr(self.dpr));
        }

        // Sharpen
        if self.sharpen < 0.0 || self.sharpen > MAX_SHARPEN {
            return Err(TransformParseError::InvalidParameter {
                name: "sharpen".to_string(),
                value: self.sharpen.to_string(),
            });
        }

        // Blur
        if self.blur < 0.0 || self.blur > MAX_BLUR_RADIUS {
            return Err(TransformParseError::InvalidParameter {
                name: "blur".to_string(),
                value: self.blur.to_string(),
            });
        }

        // Brightness
        if self.brightness < -1.0 || self.brightness > 1.0 {
            return Err(TransformParseError::InvalidParameter {
                name: "brightness".to_string(),
                value: self.brightness.to_string(),
            });
        }

        // Contrast
        if self.contrast < -1.0 || self.contrast > 1.0 {
            return Err(TransformParseError::InvalidParameter {
                name: "contrast".to_string(),
                value: self.contrast.to_string(),
            });
        }

        // Gamma
        if self.gamma < 0.0 || self.gamma > MAX_GAMMA {
            return Err(TransformParseError::InvalidParameter {
                name: "gamma".to_string(),
                value: self.gamma.to_string(),
            });
        }

        // Focal point
        if let Gravity::FocalPoint(x, y) = &self.gravity {
            if *x < 0.0 || *x > 1.0 || *y < 0.0 || *y > 1.0 {
                return Err(TransformParseError::InvalidGravity(format!("{x}x{y}")));
            }
        }

        Ok(())
    }

    /// Generate a deterministic cache key string from these transform params.
    ///
    /// Only non-default values are included. Keys are sorted alphabetically
    /// for determinism.
    ///
    /// ```
    /// use oximedia_image_transform::transform::TransformParams;
    ///
    /// let mut p = TransformParams::default();
    /// p.width = Some(800);
    /// let key = p.cache_key();
    /// assert!(key.contains("width=800"));
    /// ```
    pub fn cache_key(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        let defaults = TransformParams::default();

        if self.anim != defaults.anim {
            parts.push(format!("anim={}", self.anim));
        }
        if self.background != defaults.background {
            parts.push(format!("background={}", self.background.to_hex()));
        }
        if (self.blur - defaults.blur).abs() > f64::EPSILON {
            parts.push(format!("blur={}", self.blur));
        }
        if let Some(ref border) = self.border {
            parts.push(format!(
                "border={},{},{},{}:{}",
                border.top,
                border.right,
                border.bottom,
                border.left,
                border.color.to_hex()
            ));
        }
        if (self.brightness - defaults.brightness).abs() > f64::EPSILON {
            parts.push(format!("brightness={}", self.brightness));
        }
        if let Some(ref comp) = self.compression {
            parts.push(format!("compression={comp}"));
        }
        if (self.contrast - defaults.contrast).abs() > f64::EPSILON {
            parts.push(format!("contrast={}", self.contrast));
        }
        if (self.dpr - defaults.dpr).abs() > f64::EPSILON {
            parts.push(format!("dpr={}", self.dpr));
        }
        if self.fit != defaults.fit {
            parts.push(format!("fit={}", self.fit));
        }
        if self.format != defaults.format {
            parts.push(format!("format={}", self.format));
        }
        if (self.gamma - defaults.gamma).abs() > f64::EPSILON {
            parts.push(format!("gamma={}", self.gamma));
        }
        if self.gravity != defaults.gravity {
            parts.push(format!("gravity={}", self.gravity));
        }
        if let Some(h) = self.height {
            parts.push(format!("height={h}"));
        }
        if self.metadata != defaults.metadata {
            parts.push(format!("metadata={}", self.metadata));
        }
        if let Some(ref pad) = self.pad {
            parts.push(format!(
                "pad={},{},{},{}",
                pad.top, pad.right, pad.bottom, pad.left
            ));
        }
        if self.quality != defaults.quality {
            parts.push(format!("quality={}", self.quality));
        }
        if self.rotate != defaults.rotate {
            parts.push(format!("rotate={}", self.rotate));
        }
        if (self.sharpen - defaults.sharpen).abs() > f64::EPSILON {
            parts.push(format!("sharpen={}", self.sharpen));
        }
        if let Some(ref trim) = self.trim {
            parts.push(format!(
                "trim={},{},{},{}",
                trim.top, trim.right, trim.bottom, trim.left
            ));
        }
        if let Some(w) = self.width {
            parts.push(format!("width={w}"));
        }
        // onerror is intentionally excluded (affects error handling, not output)
        if let Some(ref opts) = self.output_options {
            if opts.progressive_jpeg {
                parts.push("progressive=true".to_string());
            }
            if opts.quality != DEFAULT_QUALITY {
                parts.push(format!("output_quality={}", opts.quality));
            }
        }

        parts.join(",")
    }

    /// Returns `true` if this transform is effectively a no-op.
    pub fn is_identity(&self) -> bool {
        let defaults = TransformParams::default();
        self.width.is_none()
            && self.height.is_none()
            && self.format == defaults.format
            && (self.sharpen - defaults.sharpen).abs() < f64::EPSILON
            && (self.blur - defaults.blur).abs() < f64::EPSILON
            && (self.brightness - defaults.brightness).abs() < f64::EPSILON
            && (self.contrast - defaults.contrast).abs() < f64::EPSILON
            && (self.gamma - defaults.gamma).abs() < f64::EPSILON
            && self.rotate == defaults.rotate
            && self.trim.is_none()
            && self.border.is_none()
            && self.pad.is_none()
    }
}

impl fmt::Display for TransformParams {
    /// Format back to Cloudflare-style comma-separated string:
    /// `width=800,height=600,quality=85,format=auto`.
    ///
    /// Only includes fields that differ from the defaults.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.cache_key())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Default / identity ──

    #[test]
    fn test_default_params() {
        let p = TransformParams::default();
        assert_eq!(p.quality, 85);
        assert_eq!(p.fit, FitMode::ScaleDown);
        assert_eq!(p.metadata, MetadataMode::None);
        assert!(p.anim);
        assert_eq!(p.gravity, Gravity::Center);
        assert!((p.dpr - 1.0).abs() < f64::EPSILON);
        assert!((p.gamma - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_identity() {
        let p = TransformParams::default();
        assert!(p.is_identity());

        let mut p2 = TransformParams::default();
        p2.width = Some(800);
        assert!(!p2.is_identity());
    }

    // ── Effective dimensions ──

    #[test]
    fn test_effective_width_no_dpr() {
        let mut p = TransformParams::default();
        p.width = Some(800);
        assert_eq!(p.effective_width(), Some(800));
    }

    #[test]
    fn test_effective_height_no_dpr() {
        let mut p = TransformParams::default();
        p.height = Some(600);
        assert_eq!(p.effective_height(), Some(600));
    }

    #[test]
    fn test_effective_width_with_dpr() {
        let mut p = TransformParams::default();
        p.width = Some(400);
        p.dpr = 2.0;
        assert_eq!(p.effective_width(), Some(800));
    }

    #[test]
    fn test_effective_height_with_dpr() {
        let mut p = TransformParams::default();
        p.height = Some(300);
        p.dpr = 2.0;
        assert_eq!(p.effective_height(), Some(600));
    }

    #[test]
    fn test_effective_width_clamped() {
        let mut p = TransformParams::default();
        p.width = Some(10000);
        p.dpr = 3.0;
        // 10000 * 3 = 30000 > MAX_DIMENSION, clamped
        assert_eq!(p.effective_width(), Some(MAX_DIMENSION));
    }

    #[test]
    fn test_effective_none() {
        let p = TransformParams::default();
        assert!(p.effective_width().is_none());
        assert!(p.effective_height().is_none());
    }

    // ── Validation ──

    #[test]
    fn test_validate_valid() {
        let mut p = TransformParams::default();
        p.width = Some(800);
        p.height = Some(600);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn test_validate_zero_width() {
        let mut p = TransformParams::default();
        p.width = Some(0);
        assert!(p.validate().is_err());
    }

    #[test]
    fn test_validate_exceed_dimension() {
        let mut p = TransformParams::default();
        p.width = Some(20000);
        assert!(p.validate().is_err());
    }

    #[test]
    fn test_validate_quality_zero() {
        let mut p = TransformParams::default();
        p.quality = 0;
        assert!(p.validate().is_err());
    }

    #[test]
    fn test_validate_quality_101() {
        let mut p = TransformParams::default();
        p.quality = 101;
        assert!(p.validate().is_err());
    }

    #[test]
    fn test_validate_quality_100() {
        let mut p = TransformParams::default();
        p.quality = 100;
        assert!(p.validate().is_ok());
    }

    #[test]
    fn test_validate_dpr_low() {
        let mut p = TransformParams::default();
        p.dpr = 0.5;
        assert!(p.validate().is_err());
    }

    #[test]
    fn test_validate_dpr_high() {
        let mut p = TransformParams::default();
        p.dpr = 5.0;
        assert!(p.validate().is_err());
    }

    #[test]
    fn test_validate_dpr_ok() {
        let mut p = TransformParams::default();
        p.dpr = 2.0;
        assert!(p.validate().is_ok());
    }

    #[test]
    fn test_validate_sharpen_ok() {
        let mut p = TransformParams::default();
        p.sharpen = 5.0;
        assert!(p.validate().is_ok());
    }

    #[test]
    fn test_validate_sharpen_too_high() {
        let mut p = TransformParams::default();
        p.sharpen = 11.0;
        assert!(p.validate().is_err());
    }

    #[test]
    fn test_validate_sharpen_negative() {
        let mut p = TransformParams::default();
        p.sharpen = -0.1;
        assert!(p.validate().is_err());
    }

    #[test]
    fn test_validate_blur_ok() {
        let mut p = TransformParams::default();
        p.blur = 100.0;
        assert!(p.validate().is_ok());
    }

    #[test]
    fn test_validate_blur_too_high() {
        let mut p = TransformParams::default();
        p.blur = 251.0;
        assert!(p.validate().is_err());
    }

    #[test]
    fn test_validate_brightness_ok() {
        let mut p = TransformParams::default();
        p.brightness = 0.5;
        assert!(p.validate().is_ok());
    }

    #[test]
    fn test_validate_brightness_too_low() {
        let mut p = TransformParams::default();
        p.brightness = -1.1;
        assert!(p.validate().is_err());
    }

    #[test]
    fn test_validate_brightness_too_high() {
        let mut p = TransformParams::default();
        p.brightness = 1.1;
        assert!(p.validate().is_err());
    }

    #[test]
    fn test_validate_contrast_ok() {
        let mut p = TransformParams::default();
        p.contrast = -1.0;
        assert!(p.validate().is_ok());
    }

    #[test]
    fn test_validate_contrast_too_high() {
        let mut p = TransformParams::default();
        p.contrast = 1.5;
        assert!(p.validate().is_err());
    }

    #[test]
    fn test_validate_gamma_ok() {
        let mut p = TransformParams::default();
        p.gamma = 2.2;
        assert!(p.validate().is_ok());
    }

    #[test]
    fn test_validate_gamma_too_high() {
        let mut p = TransformParams::default();
        p.gamma = 11.0;
        assert!(p.validate().is_err());
    }

    #[test]
    fn test_validate_focal_point_ok() {
        let mut p = TransformParams::default();
        p.gravity = Gravity::FocalPoint(0.5, 0.5);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn test_validate_focal_point_out_of_range() {
        let mut p = TransformParams::default();
        p.gravity = Gravity::FocalPoint(1.1, 0.5);
        assert!(p.validate().is_err());
    }

    // ── FitMode ──

    #[test]
    fn test_fit_parse() {
        assert_eq!(
            FitMode::from_str_loose("scale-down").ok(),
            Some(FitMode::ScaleDown)
        );
        assert_eq!(
            FitMode::from_str_loose("contain").ok(),
            Some(FitMode::Contain)
        );
        assert_eq!(FitMode::from_str_loose("cover").ok(), Some(FitMode::Cover));
        assert_eq!(FitMode::from_str_loose("crop").ok(), Some(FitMode::Crop));
        assert_eq!(FitMode::from_str_loose("pad").ok(), Some(FitMode::Pad));
        assert_eq!(FitMode::from_str_loose("fill").ok(), Some(FitMode::Fill));
        assert!(FitMode::from_str_loose("stretch").is_err());
    }

    #[test]
    fn test_fit_as_str() {
        assert_eq!(FitMode::ScaleDown.as_str(), "scale-down");
        assert_eq!(FitMode::Contain.as_str(), "contain");
        assert_eq!(FitMode::Cover.as_str(), "cover");
        assert_eq!(FitMode::Crop.as_str(), "crop");
        assert_eq!(FitMode::Pad.as_str(), "pad");
        assert_eq!(FitMode::Fill.as_str(), "fill");
    }

    #[test]
    fn test_fit_display() {
        assert_eq!(format!("{}", FitMode::Cover), "cover");
    }

    // ── Gravity ──

    #[test]
    fn test_gravity_parse_named() {
        assert_eq!(Gravity::from_str_loose("auto").ok(), Some(Gravity::Auto));
        assert_eq!(
            Gravity::from_str_loose("center").ok(),
            Some(Gravity::Center)
        );
        assert_eq!(Gravity::from_str_loose("top").ok(), Some(Gravity::Top));
        assert_eq!(Gravity::from_str_loose("face").ok(), Some(Gravity::Face));
        assert_eq!(
            Gravity::from_str_loose("bottom-right").ok(),
            Some(Gravity::BottomRight)
        );
    }

    #[test]
    fn test_gravity_focal_point_parse() {
        let g = Gravity::from_str_loose("0.3x0.7");
        assert!(g.is_ok());
        if let Ok(Gravity::FocalPoint(x, y)) = g {
            assert!((x - 0.3).abs() < 0.001);
            assert!((y - 0.7).abs() < 0.001);
        }
    }

    #[test]
    fn test_gravity_focal_point_out_of_range() {
        assert!(Gravity::from_str_loose("1.5x0.5").is_err());
        assert!(Gravity::from_str_loose("0.5x-0.1").is_err());
    }

    #[test]
    fn test_gravity_as_str() {
        assert_eq!(Gravity::Center.as_str(), "center");
        assert_eq!(Gravity::FocalPoint(0.5, 0.5).as_str(), "0.5x0.5");
    }

    #[test]
    fn test_gravity_display() {
        assert_eq!(format!("{}", Gravity::TopLeft), "top-left");
    }

    // ── OutputFormat ──

    #[test]
    fn test_format_parse() {
        assert_eq!(
            OutputFormat::from_str_loose("auto").ok(),
            Some(OutputFormat::Auto)
        );
        assert_eq!(
            OutputFormat::from_str_loose("AVIF").ok(),
            Some(OutputFormat::Avif)
        );
        assert_eq!(
            OutputFormat::from_str_loose("webp").ok(),
            Some(OutputFormat::WebP)
        );
        assert_eq!(
            OutputFormat::from_str_loose("JPEG").ok(),
            Some(OutputFormat::Jpeg)
        );
        assert_eq!(
            OutputFormat::from_str_loose("jpg").ok(),
            Some(OutputFormat::Jpeg)
        );
        assert_eq!(
            OutputFormat::from_str_loose("png").ok(),
            Some(OutputFormat::Png)
        );
        assert_eq!(
            OutputFormat::from_str_loose("gif").ok(),
            Some(OutputFormat::Gif)
        );
        assert_eq!(
            OutputFormat::from_str_loose("baseline").ok(),
            Some(OutputFormat::Baseline)
        );
        assert_eq!(
            OutputFormat::from_str_loose("json").ok(),
            Some(OutputFormat::Json)
        );
        assert!(OutputFormat::from_str_loose("bmp").is_err());
    }

    #[test]
    fn test_format_mime() {
        assert_eq!(OutputFormat::Avif.mime_type(), "image/avif");
        assert_eq!(OutputFormat::WebP.mime_type(), "image/webp");
        assert_eq!(OutputFormat::Jpeg.mime_type(), "image/jpeg");
        assert_eq!(OutputFormat::Png.mime_type(), "image/png");
        assert_eq!(OutputFormat::Gif.mime_type(), "image/gif");
        assert_eq!(OutputFormat::Json.mime_type(), "application/json");
    }

    #[test]
    fn test_format_extension() {
        assert_eq!(OutputFormat::Avif.file_extension(), "avif");
        assert_eq!(OutputFormat::WebP.file_extension(), "webp");
        assert_eq!(OutputFormat::Jpeg.file_extension(), "jpg");
        assert_eq!(OutputFormat::Json.file_extension(), "json");
    }

    #[test]
    fn test_format_animation() {
        assert!(OutputFormat::Gif.supports_animation());
        assert!(OutputFormat::WebP.supports_animation());
        assert!(OutputFormat::Avif.supports_animation());
        assert!(!OutputFormat::Jpeg.supports_animation());
        assert!(!OutputFormat::Png.supports_animation());
        assert!(!OutputFormat::Json.supports_animation());
    }

    #[test]
    fn test_format_transparency() {
        assert!(OutputFormat::Png.supports_transparency());
        assert!(OutputFormat::WebP.supports_transparency());
        assert!(!OutputFormat::Jpeg.supports_transparency());
        assert!(!OutputFormat::Baseline.supports_transparency());
    }

    #[test]
    fn test_format_as_str() {
        assert_eq!(OutputFormat::Auto.as_str(), "auto");
        assert_eq!(OutputFormat::Avif.as_str(), "avif");
        assert_eq!(OutputFormat::WebP.as_str(), "webp");
        assert_eq!(OutputFormat::Json.as_str(), "json");
    }

    #[test]
    fn test_format_display() {
        assert_eq!(format!("{}", OutputFormat::Avif), "avif");
    }

    // ── MetadataMode ──

    #[test]
    fn test_metadata_parse() {
        assert_eq!(
            MetadataMode::from_str_loose("keep").ok(),
            Some(MetadataMode::Keep)
        );
        assert_eq!(
            MetadataMode::from_str_loose("copyright").ok(),
            Some(MetadataMode::Copyright)
        );
        assert_eq!(
            MetadataMode::from_str_loose("none").ok(),
            Some(MetadataMode::None)
        );
        assert_eq!(
            MetadataMode::from_str_loose("strip").ok(),
            Some(MetadataMode::None)
        );
    }

    // ── Color ──

    #[test]
    fn test_color_from_hex_6() {
        let c = Color::from_hex("#ff8800").expect("valid hex");
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 136);
        assert_eq!(c.b, 0);
        assert_eq!(c.a, 255);
    }

    #[test]
    fn test_color_from_hex_8() {
        let c = Color::from_hex("#ff880080").expect("valid hex");
        assert_eq!(c.r, 255);
        assert_eq!(c.g, 136);
        assert_eq!(c.b, 0);
        assert_eq!(c.a, 128);
    }

    #[test]
    fn test_color_from_hex_no_hash() {
        let c = Color::from_hex("00ff00").expect("valid hex");
        assert_eq!(c, Color::new(0, 255, 0, 255));
    }

    #[test]
    fn test_color_from_css_rgb() {
        let c = Color::from_css("rgb(128,64,32)").expect("valid rgb");
        assert_eq!(c, Color::new(128, 64, 32, 255));
    }

    #[test]
    fn test_color_from_css_rgba() {
        let c = Color::from_css("rgba(128,64,32,0.5)").expect("valid rgba");
        assert_eq!(c.r, 128);
        assert_eq!(c.g, 64);
        assert_eq!(c.b, 32);
        assert_eq!(c.a, 128); // 0.5 * 255 = 127.5 -> 128 (rounded)
    }

    #[test]
    fn test_color_invalid() {
        assert!(Color::from_hex("xyz").is_err());
        assert!(Color::from_hex("#gg0000").is_err());
        assert!(Color::from_hex("#ff00").is_err());
    }

    #[test]
    fn test_color_to_hex_opaque() {
        let c = Color::new(255, 0, 128, 255);
        assert_eq!(c.to_hex(), "ff0080");
    }

    #[test]
    fn test_color_to_hex_transparent() {
        let c = Color::new(255, 0, 128, 128);
        assert_eq!(c.to_hex(), "ff008080");
    }

    #[test]
    fn test_color_display() {
        let c = Color::new(255, 0, 0, 255);
        assert_eq!(format!("{c}"), "#ff0000");
    }

    #[test]
    fn test_color_presets() {
        assert_eq!(Color::transparent().a, 0);
        assert_eq!(Color::white(), Color::new(255, 255, 255, 255));
        assert_eq!(Color::black(), Color::new(0, 0, 0, 255));
    }

    // ── Rotation ──

    #[test]
    fn test_rotation_from_degrees() {
        assert_eq!(Rotation::from_degrees(0).ok(), Some(Rotation::Deg0));
        assert_eq!(Rotation::from_degrees(90).ok(), Some(Rotation::Deg90));
        assert_eq!(Rotation::from_degrees(180).ok(), Some(Rotation::Deg180));
        assert_eq!(Rotation::from_degrees(270).ok(), Some(Rotation::Deg270));
        assert!(Rotation::from_degrees(45).is_err());
    }

    #[test]
    fn test_rotation_from_str() {
        assert_eq!(Rotation::from_str_loose("auto").ok(), Some(Rotation::Auto));
        assert_eq!(Rotation::from_str_loose("90").ok(), Some(Rotation::Deg90));
    }

    #[test]
    fn test_rotation_to_degrees() {
        assert_eq!(Rotation::Deg90.to_degrees(), Some(90));
        assert_eq!(Rotation::Auto.to_degrees(), None);
    }

    #[test]
    fn test_rotation_display() {
        assert_eq!(format!("{}", Rotation::Deg90), "90");
        assert_eq!(format!("{}", Rotation::Auto), "auto");
    }

    // ── Compression ──

    #[test]
    fn test_compression_parse() {
        assert_eq!(
            Compression::from_str_loose("fast").ok(),
            Some(Compression::Fast)
        );
        assert_eq!(
            Compression::from_str_loose("default").ok(),
            Some(Compression::Default)
        );
        assert_eq!(
            Compression::from_str_loose("best").ok(),
            Some(Compression::Best)
        );
        assert!(Compression::from_str_loose("invalid").is_err());
    }

    // ── Border ──

    #[test]
    fn test_border_uniform() {
        let b = Border::uniform(5, Color::black());
        assert_eq!(b.top, 5);
        assert_eq!(b.right, 5);
        assert_eq!(b.bottom, 5);
        assert_eq!(b.left, 5);
    }

    // ── Padding ──

    #[test]
    fn test_padding_uniform() {
        let p = Padding::uniform(0.05);
        assert!((p.top - 0.05).abs() < 1e-9);
        assert!((p.right - 0.05).abs() < 1e-9);
        assert!((p.bottom - 0.05).abs() < 1e-9);
        assert!((p.left - 0.05).abs() < 1e-9);
    }

    // ── Trim ──

    #[test]
    fn test_trim_uniform() {
        let t = Trim::uniform(10);
        assert_eq!(t.top, 10);
        assert_eq!(t.right, 10);
        assert_eq!(t.bottom, 10);
        assert_eq!(t.left, 10);
    }

    // ── Display / cache key ──

    #[test]
    fn test_display_default_is_empty() {
        let p = TransformParams::default();
        assert_eq!(format!("{p}"), "");
    }

    #[test]
    fn test_display_with_params() {
        let mut p = TransformParams::default();
        p.width = Some(800);
        p.height = Some(600);
        p.quality = 90;
        let s = format!("{p}");
        assert!(s.contains("width=800"));
        assert!(s.contains("height=600"));
        assert!(s.contains("quality=90"));
    }

    #[test]
    fn test_cache_key_deterministic() {
        let mut p = TransformParams::default();
        p.width = Some(800);
        p.quality = 90;
        let k1 = p.cache_key();
        let k2 = p.cache_key();
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_cache_key_excludes_onerror() {
        let mut p = TransformParams::default();
        p.width = Some(800);
        p.onerror = Some("redirect".to_string());
        let key = p.cache_key();
        assert!(!key.contains("onerror"));
    }
}
