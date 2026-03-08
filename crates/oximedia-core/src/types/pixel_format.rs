//! Pixel format definitions for video frames.
//!
//! This module provides the [`PixelFormat`] enum representing the various
//! ways pixel data can be stored in video frames.

/// Pixel format for video frames.
///
/// Defines how pixel data is organized in memory, including color space,
/// bit depth, and plane layout.
///
/// # Examples
///
/// ```
/// use oximedia_core::types::PixelFormat;
///
/// let format = PixelFormat::Yuv420p;
/// assert!(format.is_planar());
/// assert_eq!(format.plane_count(), 3);
/// assert_eq!(format.bits_per_pixel(), 12);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
#[derive(Default)]
pub enum PixelFormat {
    /// YUV 4:2:0 planar, 8-bit per component.
    /// Most common format for video compression.
    #[default]
    Yuv420p,

    /// YUV 4:2:2 planar, 8-bit per component.
    /// Higher chroma resolution than 4:2:0.
    Yuv422p,

    /// YUV 4:4:4 planar, 8-bit per component.
    /// Full chroma resolution.
    Yuv444p,

    /// YUV 4:2:0 planar, 10-bit little-endian per component.
    /// Used for HDR content.
    Yuv420p10le,

    /// YUV 4:2:0 planar, 12-bit little-endian per component.
    /// Professional and cinema formats.
    Yuv420p12le,

    /// RGB 24-bit packed (8 bits per channel).
    /// Common for display and image files.
    Rgb24,

    /// RGBA 32-bit packed (8 bits per channel with alpha).
    /// RGB with transparency.
    Rgba32,

    /// Grayscale 8-bit.
    /// Single luminance channel.
    Gray8,

    /// Grayscale 16-bit.
    /// High bit-depth luminance.
    Gray16,
}

impl PixelFormat {
    /// Returns the number of bits per pixel.
    ///
    /// For planar formats, this is the average bits per pixel
    /// considering all planes.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::PixelFormat;
    ///
    /// assert_eq!(PixelFormat::Yuv420p.bits_per_pixel(), 12);
    /// assert_eq!(PixelFormat::Rgb24.bits_per_pixel(), 24);
    /// assert_eq!(PixelFormat::Rgba32.bits_per_pixel(), 32);
    /// ```
    #[must_use]
    pub const fn bits_per_pixel(&self) -> u32 {
        match self {
            Self::Gray8 => 8,
            Self::Yuv420p => 12,
            Self::Yuv420p10le => 15,
            Self::Yuv422p | Self::Gray16 => 16,
            Self::Yuv420p12le => 18,
            Self::Yuv444p | Self::Rgb24 => 24,
            Self::Rgba32 => 32,
        }
    }

    /// Returns the number of planes in this format.
    ///
    /// Planar formats have separate planes for each component,
    /// while packed formats store all components interleaved.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::PixelFormat;
    ///
    /// assert_eq!(PixelFormat::Yuv420p.plane_count(), 3);
    /// assert_eq!(PixelFormat::Rgb24.plane_count(), 1);
    /// assert_eq!(PixelFormat::Gray8.plane_count(), 1);
    /// ```
    #[must_use]
    pub const fn plane_count(&self) -> u32 {
        match self {
            Self::Yuv420p
            | Self::Yuv422p
            | Self::Yuv444p
            | Self::Yuv420p10le
            | Self::Yuv420p12le => 3,
            Self::Rgb24 | Self::Rgba32 | Self::Gray8 | Self::Gray16 => 1,
        }
    }

    /// Returns whether this format uses planar storage.
    ///
    /// Planar formats store each color component in a separate
    /// contiguous memory region.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::PixelFormat;
    ///
    /// assert!(PixelFormat::Yuv420p.is_planar());
    /// assert!(!PixelFormat::Rgb24.is_planar());
    /// ```
    #[must_use]
    pub const fn is_planar(&self) -> bool {
        match self {
            Self::Yuv420p
            | Self::Yuv422p
            | Self::Yuv444p
            | Self::Yuv420p10le
            | Self::Yuv420p12le => true,
            Self::Rgb24 | Self::Rgba32 | Self::Gray8 | Self::Gray16 => false,
        }
    }

    /// Returns the bits per component for this format.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::PixelFormat;
    ///
    /// assert_eq!(PixelFormat::Yuv420p.bits_per_component(), 8);
    /// assert_eq!(PixelFormat::Yuv420p10le.bits_per_component(), 10);
    /// assert_eq!(PixelFormat::Gray16.bits_per_component(), 16);
    /// ```
    #[must_use]
    pub const fn bits_per_component(&self) -> u32 {
        match self {
            Self::Yuv420p
            | Self::Yuv422p
            | Self::Yuv444p
            | Self::Rgb24
            | Self::Rgba32
            | Self::Gray8 => 8,
            Self::Yuv420p10le => 10,
            Self::Yuv420p12le => 12,
            Self::Gray16 => 16,
        }
    }

    /// Returns true if this is a YUV format.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::PixelFormat;
    ///
    /// assert!(PixelFormat::Yuv420p.is_yuv());
    /// assert!(!PixelFormat::Rgb24.is_yuv());
    /// ```
    #[must_use]
    pub const fn is_yuv(&self) -> bool {
        matches!(
            self,
            Self::Yuv420p | Self::Yuv422p | Self::Yuv444p | Self::Yuv420p10le | Self::Yuv420p12le
        )
    }

    /// Returns true if this is an RGB format.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::PixelFormat;
    ///
    /// assert!(PixelFormat::Rgb24.is_rgb());
    /// assert!(PixelFormat::Rgba32.is_rgb());
    /// assert!(!PixelFormat::Yuv420p.is_rgb());
    /// ```
    #[must_use]
    pub const fn is_rgb(&self) -> bool {
        matches!(self, Self::Rgb24 | Self::Rgba32)
    }

    /// Returns true if this format has an alpha channel.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::PixelFormat;
    ///
    /// assert!(PixelFormat::Rgba32.has_alpha());
    /// assert!(!PixelFormat::Rgb24.has_alpha());
    /// ```
    #[must_use]
    pub const fn has_alpha(&self) -> bool {
        matches!(self, Self::Rgba32)
    }

    /// Returns the chroma subsampling ratio as (horizontal, vertical).
    ///
    /// For non-YUV formats, returns (1, 1) indicating no subsampling.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::PixelFormat;
    ///
    /// assert_eq!(PixelFormat::Yuv420p.chroma_subsampling(), (2, 2));
    /// assert_eq!(PixelFormat::Yuv422p.chroma_subsampling(), (2, 1));
    /// assert_eq!(PixelFormat::Yuv444p.chroma_subsampling(), (1, 1));
    /// ```
    #[must_use]
    pub const fn chroma_subsampling(&self) -> (u32, u32) {
        match self {
            Self::Yuv420p | Self::Yuv420p10le | Self::Yuv420p12le => (2, 2),
            Self::Yuv422p => (2, 1),
            Self::Yuv444p | Self::Rgb24 | Self::Rgba32 | Self::Gray8 | Self::Gray16 => (1, 1),
        }
    }
}

impl std::fmt::Display for PixelFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Yuv420p => "yuv420p",
            Self::Yuv422p => "yuv422p",
            Self::Yuv444p => "yuv444p",
            Self::Yuv420p10le => "yuv420p10le",
            Self::Yuv420p12le => "yuv420p12le",
            Self::Rgb24 => "rgb24",
            Self::Rgba32 => "rgba32",
            Self::Gray8 => "gray8",
            Self::Gray16 => "gray16",
        };
        write!(f, "{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bits_per_pixel() {
        assert_eq!(PixelFormat::Yuv420p.bits_per_pixel(), 12);
        assert_eq!(PixelFormat::Yuv422p.bits_per_pixel(), 16);
        assert_eq!(PixelFormat::Yuv444p.bits_per_pixel(), 24);
        assert_eq!(PixelFormat::Rgb24.bits_per_pixel(), 24);
        assert_eq!(PixelFormat::Rgba32.bits_per_pixel(), 32);
        assert_eq!(PixelFormat::Gray8.bits_per_pixel(), 8);
        assert_eq!(PixelFormat::Gray16.bits_per_pixel(), 16);
    }

    #[test]
    fn test_plane_count() {
        assert_eq!(PixelFormat::Yuv420p.plane_count(), 3);
        assert_eq!(PixelFormat::Yuv422p.plane_count(), 3);
        assert_eq!(PixelFormat::Rgb24.plane_count(), 1);
        assert_eq!(PixelFormat::Gray8.plane_count(), 1);
    }

    #[test]
    fn test_is_planar() {
        assert!(PixelFormat::Yuv420p.is_planar());
        assert!(PixelFormat::Yuv422p.is_planar());
        assert!(!PixelFormat::Rgb24.is_planar());
        assert!(!PixelFormat::Rgba32.is_planar());
    }

    #[test]
    fn test_is_yuv() {
        assert!(PixelFormat::Yuv420p.is_yuv());
        assert!(PixelFormat::Yuv420p10le.is_yuv());
        assert!(!PixelFormat::Rgb24.is_yuv());
    }

    #[test]
    fn test_chroma_subsampling() {
        assert_eq!(PixelFormat::Yuv420p.chroma_subsampling(), (2, 2));
        assert_eq!(PixelFormat::Yuv422p.chroma_subsampling(), (2, 1));
        assert_eq!(PixelFormat::Yuv444p.chroma_subsampling(), (1, 1));
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", PixelFormat::Yuv420p), "yuv420p");
        assert_eq!(format!("{}", PixelFormat::Rgb24), "rgb24");
    }
}
