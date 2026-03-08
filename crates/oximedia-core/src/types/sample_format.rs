//! Audio sample format definitions.
//!
//! This module provides the [`SampleFormat`] enum representing the various
//! ways audio sample data can be stored in memory.

/// Audio sample format.
///
/// Defines how audio samples are stored in memory, including bit depth,
/// signedness, and whether samples are interleaved or planar.
///
/// Formats ending with 'p' are planar (one plane per channel).
///
/// # Examples
///
/// ```
/// use oximedia_core::types::SampleFormat;
///
/// let format = SampleFormat::F32;
/// assert!(!format.is_planar());
/// assert_eq!(format.bytes_per_sample(), 4);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[derive(Default)]
pub enum SampleFormat {
    /// Unsigned 8-bit integer, interleaved.
    U8,

    /// Signed 16-bit integer, interleaved.
    S16,

    /// Signed 32-bit integer, interleaved.
    S32,

    /// 32-bit floating point, interleaved.
    #[default]
    F32,

    /// 64-bit floating point, interleaved.
    F64,

    /// Signed 16-bit integer, planar.
    S16p,

    /// Signed 32-bit integer, planar.
    S32p,

    /// 32-bit floating point, planar.
    F32p,

    /// 64-bit floating point, planar.
    F64p,
}

impl SampleFormat {
    /// Returns the number of bytes per sample.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::SampleFormat;
    ///
    /// assert_eq!(SampleFormat::U8.bytes_per_sample(), 1);
    /// assert_eq!(SampleFormat::S16.bytes_per_sample(), 2);
    /// assert_eq!(SampleFormat::F32.bytes_per_sample(), 4);
    /// assert_eq!(SampleFormat::F64.bytes_per_sample(), 8);
    /// ```
    #[must_use]
    pub const fn bytes_per_sample(&self) -> usize {
        match self {
            Self::U8 => 1,
            Self::S16 | Self::S16p => 2,
            Self::S32 | Self::S32p | Self::F32 | Self::F32p => 4,
            Self::F64 | Self::F64p => 8,
        }
    }

    /// Returns whether this format uses planar storage.
    ///
    /// Planar formats store each channel in a separate contiguous
    /// memory region, as opposed to interleaved formats where
    /// samples alternate between channels.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::SampleFormat;
    ///
    /// assert!(!SampleFormat::F32.is_planar());
    /// assert!(SampleFormat::F32p.is_planar());
    /// ```
    #[must_use]
    pub const fn is_planar(&self) -> bool {
        matches!(self, Self::S16p | Self::S32p | Self::F32p | Self::F64p)
    }

    /// Returns the number of bits per sample.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::SampleFormat;
    ///
    /// assert_eq!(SampleFormat::U8.bits_per_sample(), 8);
    /// assert_eq!(SampleFormat::S16.bits_per_sample(), 16);
    /// assert_eq!(SampleFormat::F32.bits_per_sample(), 32);
    /// ```
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn bits_per_sample(&self) -> u32 {
        (self.bytes_per_sample() * 8) as u32
    }

    /// Returns whether this format uses floating point samples.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::SampleFormat;
    ///
    /// assert!(SampleFormat::F32.is_float());
    /// assert!(SampleFormat::F64p.is_float());
    /// assert!(!SampleFormat::S16.is_float());
    /// ```
    #[must_use]
    pub const fn is_float(&self) -> bool {
        matches!(self, Self::F32 | Self::F64 | Self::F32p | Self::F64p)
    }

    /// Returns whether this format uses signed integer samples.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::SampleFormat;
    ///
    /// assert!(SampleFormat::S16.is_signed());
    /// assert!(SampleFormat::S32p.is_signed());
    /// assert!(!SampleFormat::U8.is_signed());
    /// ```
    #[must_use]
    pub const fn is_signed(&self) -> bool {
        !matches!(self, Self::U8)
    }

    /// Returns the packed (interleaved) equivalent of this format.
    ///
    /// If the format is already packed, returns self.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::SampleFormat;
    ///
    /// assert_eq!(SampleFormat::F32p.to_packed(), SampleFormat::F32);
    /// assert_eq!(SampleFormat::S16.to_packed(), SampleFormat::S16);
    /// ```
    #[must_use]
    pub const fn to_packed(&self) -> Self {
        match self {
            Self::S16p => Self::S16,
            Self::S32p => Self::S32,
            Self::F32p => Self::F32,
            Self::F64p => Self::F64,
            other => *other,
        }
    }

    /// Returns the planar equivalent of this format.
    ///
    /// If the format is already planar, returns self.
    /// Note: U8 has no planar equivalent and returns U8.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_core::types::SampleFormat;
    ///
    /// assert_eq!(SampleFormat::F32.to_planar(), SampleFormat::F32p);
    /// assert_eq!(SampleFormat::S16p.to_planar(), SampleFormat::S16p);
    /// ```
    #[must_use]
    pub const fn to_planar(&self) -> Self {
        match self {
            Self::S16 => Self::S16p,
            Self::S32 => Self::S32p,
            Self::F32 => Self::F32p,
            Self::F64 => Self::F64p,
            other => *other,
        }
    }
}

impl std::fmt::Display for SampleFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::U8 => "u8",
            Self::S16 => "s16",
            Self::S32 => "s32",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::S16p => "s16p",
            Self::S32p => "s32p",
            Self::F32p => "f32p",
            Self::F64p => "f64p",
        };
        write!(f, "{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_per_sample() {
        assert_eq!(SampleFormat::U8.bytes_per_sample(), 1);
        assert_eq!(SampleFormat::S16.bytes_per_sample(), 2);
        assert_eq!(SampleFormat::S16p.bytes_per_sample(), 2);
        assert_eq!(SampleFormat::S32.bytes_per_sample(), 4);
        assert_eq!(SampleFormat::F32.bytes_per_sample(), 4);
        assert_eq!(SampleFormat::F64.bytes_per_sample(), 8);
    }

    #[test]
    fn test_is_planar() {
        assert!(!SampleFormat::U8.is_planar());
        assert!(!SampleFormat::S16.is_planar());
        assert!(!SampleFormat::F32.is_planar());
        assert!(SampleFormat::S16p.is_planar());
        assert!(SampleFormat::F32p.is_planar());
        assert!(SampleFormat::F64p.is_planar());
    }

    #[test]
    fn test_is_float() {
        assert!(!SampleFormat::U8.is_float());
        assert!(!SampleFormat::S16.is_float());
        assert!(SampleFormat::F32.is_float());
        assert!(SampleFormat::F64.is_float());
        assert!(SampleFormat::F32p.is_float());
    }

    #[test]
    fn test_is_signed() {
        assert!(!SampleFormat::U8.is_signed());
        assert!(SampleFormat::S16.is_signed());
        assert!(SampleFormat::S32.is_signed());
        assert!(SampleFormat::F32.is_signed());
    }

    #[test]
    fn test_to_packed() {
        assert_eq!(SampleFormat::F32p.to_packed(), SampleFormat::F32);
        assert_eq!(SampleFormat::S16p.to_packed(), SampleFormat::S16);
        assert_eq!(SampleFormat::F32.to_packed(), SampleFormat::F32);
    }

    #[test]
    fn test_to_planar() {
        assert_eq!(SampleFormat::F32.to_planar(), SampleFormat::F32p);
        assert_eq!(SampleFormat::S16.to_planar(), SampleFormat::S16p);
        assert_eq!(SampleFormat::F32p.to_planar(), SampleFormat::F32p);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", SampleFormat::F32), "f32");
        assert_eq!(format!("{}", SampleFormat::S16p), "s16p");
    }
}
