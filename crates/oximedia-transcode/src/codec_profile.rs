//! Codec profile definitions and selector utilities.
//!
//! Provides `CodecLevel`, `CodecProfile`, and `CodecProfileSelector`
//! for choosing the best encoding profile for a given resolution.

#![allow(dead_code)]

/// H.264/HEVC codec conformance level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CodecLevel {
    /// Level 3.0 – up to 720p30.
    Level30,
    /// Level 3.1 – up to 720p60 / 1080p30.
    Level31,
    /// Level 4.0 – up to 1080p30 high-quality.
    Level40,
    /// Level 4.1 – up to 1080p60.
    Level41,
    /// Level 5.0 – up to 2160p30.
    Level50,
    /// Level 5.1 – up to 2160p60.
    Level51,
    /// Level 5.2 – 2160p120 / 8K30.
    Level52,
}

impl CodecLevel {
    /// Returns the level as a floating-point number (e.g. 4.1).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn as_f32(self) -> f32 {
        match self {
            CodecLevel::Level30 => 3.0,
            CodecLevel::Level31 => 3.1,
            CodecLevel::Level40 => 4.0,
            CodecLevel::Level41 => 4.1,
            CodecLevel::Level50 => 5.0,
            CodecLevel::Level51 => 5.1,
            CodecLevel::Level52 => 5.2,
        }
    }

    /// Returns the maximum pixel count this level supports per frame.
    #[must_use]
    pub fn max_pixels(self) -> u64 {
        match self {
            CodecLevel::Level30 => 1_280 * 720,
            CodecLevel::Level31 => 1_280 * 720,
            CodecLevel::Level40 => 1_920 * 1_080,
            CodecLevel::Level41 => 1_920 * 1_080,
            CodecLevel::Level50 => 3_840 * 2_160,
            CodecLevel::Level51 => 3_840 * 2_160,
            CodecLevel::Level52 => 7_680 * 4_320,
        }
    }
}

/// An encoding profile combining codec name, level, and capability flags.
#[derive(Debug, Clone)]
pub struct CodecProfile {
    /// Codec identifier (e.g. "h264", "hevc", "av1").
    pub codec: String,
    /// Conformance level.
    pub level: CodecLevel,
    /// Maximum bitrate in Megabits per second.
    max_bitrate_mbps_val: f64,
    /// Whether hardware encoders are typically available for this profile.
    hw_encodable: bool,
    /// Whether this profile supports 10-bit colour.
    supports_10bit: bool,
}

impl CodecProfile {
    /// Creates a new codec profile.
    pub fn new(
        codec: impl Into<String>,
        level: CodecLevel,
        max_bitrate_mbps: f64,
        hw_encodable: bool,
    ) -> Self {
        Self {
            codec: codec.into(),
            level,
            max_bitrate_mbps_val: max_bitrate_mbps,
            hw_encodable,
            supports_10bit: false,
        }
    }

    /// Enables 10-bit colour support for this profile.
    #[must_use]
    pub fn with_10bit(mut self) -> Self {
        self.supports_10bit = true;
        self
    }

    /// Returns the maximum bitrate in Megabits per second.
    #[must_use]
    pub fn max_bitrate_mbps(&self) -> f64 {
        self.max_bitrate_mbps_val
    }

    /// Returns `true` if this profile is typically hardware-encodable.
    #[must_use]
    pub fn is_hardware_encodable(&self) -> bool {
        self.hw_encodable
    }

    /// Returns `true` if this profile supports 10-bit depth.
    #[must_use]
    pub fn supports_10bit(&self) -> bool {
        self.supports_10bit
    }

    /// Returns the maximum pixel count for this profile's level.
    #[must_use]
    pub fn max_pixels(&self) -> u64 {
        self.level.max_pixels()
    }

    /// Returns `true` if this profile can encode the given resolution.
    #[must_use]
    pub fn supports_resolution(&self, width: u32, height: u32) -> bool {
        let pixels = u64::from(width) * u64::from(height);
        pixels <= self.level.max_pixels()
    }
}

/// Selects an appropriate codec profile based on resolution requirements.
#[derive(Debug, Default)]
pub struct CodecProfileSelector {
    profiles: Vec<CodecProfile>,
}

impl CodecProfileSelector {
    /// Creates an empty selector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a selector pre-loaded with common H.264 profiles.
    #[must_use]
    pub fn with_h264_defaults() -> Self {
        let mut s = Self::new();
        s.add(CodecProfile::new("h264", CodecLevel::Level31, 10.0, true));
        s.add(CodecProfile::new("h264", CodecLevel::Level41, 50.0, true));
        s.add(CodecProfile::new("h264", CodecLevel::Level51, 240.0, true));
        s
    }

    /// Creates a selector pre-loaded with common HEVC profiles.
    #[must_use]
    pub fn with_hevc_defaults() -> Self {
        let mut s = Self::new();
        s.add(CodecProfile::new("hevc", CodecLevel::Level41, 40.0, true).with_10bit());
        s.add(CodecProfile::new("hevc", CodecLevel::Level51, 160.0, true).with_10bit());
        s.add(CodecProfile::new("hevc", CodecLevel::Level52, 640.0, false).with_10bit());
        s
    }

    /// Adds a profile to the selector.
    pub fn add(&mut self, profile: CodecProfile) {
        self.profiles.push(profile);
    }

    /// Selects the lowest-level (most compatible) profile that supports the
    /// given resolution. Returns `None` if no profile is sufficient.
    #[must_use]
    pub fn select_for_resolution(&self, width: u32, height: u32) -> Option<&CodecProfile> {
        self.profiles
            .iter()
            .filter(|p| p.supports_resolution(width, height))
            .min_by(|a, b| a.level.cmp(&b.level))
    }

    /// Returns all profiles that support the given resolution.
    #[must_use]
    pub fn all_for_resolution(&self, width: u32, height: u32) -> Vec<&CodecProfile> {
        self.profiles
            .iter()
            .filter(|p| p.supports_resolution(width, height))
            .collect()
    }

    /// Returns the number of profiles registered.
    #[must_use]
    pub fn profile_count(&self) -> usize {
        self.profiles.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codec_level_ordering() {
        assert!(CodecLevel::Level30 < CodecLevel::Level31);
        assert!(CodecLevel::Level41 < CodecLevel::Level50);
        assert!(CodecLevel::Level51 < CodecLevel::Level52);
    }

    #[test]
    fn test_codec_level_as_f32() {
        let v = CodecLevel::Level41.as_f32();
        assert!((v - 4.1_f32).abs() < 0.001);
    }

    #[test]
    fn test_codec_level_max_pixels_1080p() {
        assert_eq!(CodecLevel::Level41.max_pixels(), 1_920 * 1_080);
    }

    #[test]
    fn test_codec_level_max_pixels_4k() {
        assert_eq!(CodecLevel::Level51.max_pixels(), 3_840 * 2_160);
    }

    #[test]
    fn test_profile_max_bitrate() {
        let p = CodecProfile::new("h264", CodecLevel::Level41, 50.0, true);
        assert!((p.max_bitrate_mbps() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_profile_hw_encodable() {
        let p = CodecProfile::new("h264", CodecLevel::Level41, 50.0, true);
        assert!(p.is_hardware_encodable());
        let p2 = CodecProfile::new("av1", CodecLevel::Level51, 100.0, false);
        assert!(!p2.is_hardware_encodable());
    }

    #[test]
    fn test_profile_10bit_default_false() {
        let p = CodecProfile::new("h264", CodecLevel::Level41, 50.0, true);
        assert!(!p.supports_10bit());
    }

    #[test]
    fn test_profile_with_10bit() {
        let p = CodecProfile::new("hevc", CodecLevel::Level51, 160.0, true).with_10bit();
        assert!(p.supports_10bit());
    }

    #[test]
    fn test_profile_supports_resolution_1080p() {
        let p = CodecProfile::new("h264", CodecLevel::Level41, 50.0, true);
        assert!(p.supports_resolution(1920, 1080));
        assert!(!p.supports_resolution(3840, 2160));
    }

    #[test]
    fn test_selector_h264_defaults_count() {
        let sel = CodecProfileSelector::with_h264_defaults();
        assert_eq!(sel.profile_count(), 3);
    }

    #[test]
    fn test_selector_select_for_1080p_returns_lowest_level() {
        let sel = CodecProfileSelector::with_h264_defaults();
        let p = sel
            .select_for_resolution(1920, 1080)
            .expect("should succeed in test");
        // Level31 max_pixels is 1280*720, so Level41 should be selected
        assert_eq!(p.level, CodecLevel::Level41);
    }

    #[test]
    fn test_selector_select_for_720p() {
        let sel = CodecProfileSelector::with_h264_defaults();
        let p = sel
            .select_for_resolution(1280, 720)
            .expect("should succeed in test");
        assert_eq!(p.level, CodecLevel::Level31);
    }

    #[test]
    fn test_selector_select_for_8k_returns_none_h264() {
        let sel = CodecProfileSelector::with_h264_defaults();
        // 8K exceeds all H.264 profiles in the default set
        assert!(sel.select_for_resolution(7680, 4320).is_none());
    }

    #[test]
    fn test_selector_hevc_supports_4k() {
        let sel = CodecProfileSelector::with_hevc_defaults();
        let p = sel
            .select_for_resolution(3840, 2160)
            .expect("should succeed in test");
        assert_eq!(p.codec, "hevc");
        assert!(p.supports_10bit());
    }
}
