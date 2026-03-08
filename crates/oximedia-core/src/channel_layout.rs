//! Audio channel layout definitions and utilities.
//!
//! Provides channel layout descriptors for common speaker configurations
//! and utilities for interleaving / indexing individual channels.

#![allow(dead_code)]

/// An individual audio channel (speaker position).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AudioChannel {
    /// Front left.
    FrontLeft,
    /// Front right.
    FrontRight,
    /// Front centre.
    FrontCenter,
    /// Low-frequency effects (subwoofer).
    LowFrequency,
    /// Back / surround left.
    BackLeft,
    /// Back / surround right.
    BackRight,
    /// Side left.
    SideLeft,
    /// Side right.
    SideRight,
    /// Top front left.
    TopFrontLeft,
    /// Top front right.
    TopFrontRight,
}

impl AudioChannel {
    /// Returns a short label for the channel.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::FrontLeft => "FL",
            Self::FrontRight => "FR",
            Self::FrontCenter => "FC",
            Self::LowFrequency => "LFE",
            Self::BackLeft => "BL",
            Self::BackRight => "BR",
            Self::SideLeft => "SL",
            Self::SideRight => "SR",
            Self::TopFrontLeft => "TFL",
            Self::TopFrontRight => "TFR",
        }
    }

    /// Returns `true` if this channel is part of the surround field.
    #[must_use]
    pub fn is_surround(self) -> bool {
        matches!(
            self,
            Self::BackLeft
                | Self::BackRight
                | Self::SideLeft
                | Self::SideRight
                | Self::TopFrontLeft
                | Self::TopFrontRight
        )
    }
}

/// A named audio channel layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelLayoutKind {
    /// Mono (1 channel: FC).
    Mono,
    /// Stereo (2 channels: FL, FR).
    Stereo,
    /// 2.1 (3 channels: FL, FR, LFE).
    Surround21,
    /// 5.1 (6 channels: FL, FR, FC, LFE, BL, BR).
    Surround51,
    /// 7.1 (8 channels: FL, FR, FC, LFE, BL, BR, SL, SR).
    Surround71,
    /// Custom layout (channels described separately).
    Custom,
}

impl ChannelLayoutKind {
    /// Returns the number of channels for standard layouts.
    ///
    /// Returns `0` for `Custom`.
    #[must_use]
    pub fn channel_count(self) -> usize {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
            Self::Surround21 => 3,
            Self::Surround51 => 6,
            Self::Surround71 => 8,
            Self::Custom => 0,
        }
    }

    /// Returns the ordered channel list for standard layouts.
    ///
    /// Returns an empty slice for `Custom`.
    #[must_use]
    pub fn channels(self) -> &'static [AudioChannel] {
        match self {
            Self::Mono => &[AudioChannel::FrontCenter],
            Self::Stereo => &[AudioChannel::FrontLeft, AudioChannel::FrontRight],
            Self::Surround21 => &[
                AudioChannel::FrontLeft,
                AudioChannel::FrontRight,
                AudioChannel::LowFrequency,
            ],
            Self::Surround51 => &[
                AudioChannel::FrontLeft,
                AudioChannel::FrontRight,
                AudioChannel::FrontCenter,
                AudioChannel::LowFrequency,
                AudioChannel::BackLeft,
                AudioChannel::BackRight,
            ],
            Self::Surround71 => &[
                AudioChannel::FrontLeft,
                AudioChannel::FrontRight,
                AudioChannel::FrontCenter,
                AudioChannel::LowFrequency,
                AudioChannel::BackLeft,
                AudioChannel::BackRight,
                AudioChannel::SideLeft,
                AudioChannel::SideRight,
            ],
            Self::Custom => &[],
        }
    }

    /// Returns `true` if the layout has a low-frequency effects channel.
    #[must_use]
    pub fn has_lfe(self) -> bool {
        self.channels().contains(&AudioChannel::LowFrequency)
    }
}

/// A concrete channel layout (possibly custom).
#[derive(Debug, Clone)]
pub struct ChannelLayout {
    /// Kind of layout.
    pub kind: ChannelLayoutKind,
    /// Custom channel list (only used when `kind == Custom`).
    custom_channels: Vec<AudioChannel>,
}

impl ChannelLayout {
    /// Creates a standard layout.
    #[must_use]
    pub fn standard(kind: ChannelLayoutKind) -> Self {
        Self {
            kind,
            custom_channels: Vec::new(),
        }
    }

    /// Creates a custom layout from an explicit channel list.
    #[must_use]
    pub fn custom(channels: Vec<AudioChannel>) -> Self {
        Self {
            kind: ChannelLayoutKind::Custom,
            custom_channels: channels,
        }
    }

    /// Returns a slice of the channels in this layout.
    #[must_use]
    pub fn channels(&self) -> &[AudioChannel] {
        if self.kind == ChannelLayoutKind::Custom {
            &self.custom_channels
        } else {
            self.kind.channels()
        }
    }

    /// Returns the total number of channels.
    #[must_use]
    pub fn channel_count(&self) -> usize {
        self.channels().len()
    }

    /// Returns the index of `ch` in this layout, or `None` if not present.
    #[must_use]
    pub fn index_of(&self, ch: AudioChannel) -> Option<usize> {
        self.channels().iter().position(|&c| c == ch)
    }

    /// Returns `true` if this layout contains the given channel.
    #[must_use]
    pub fn contains(&self, ch: AudioChannel) -> bool {
        self.channels().contains(&ch)
    }

    /// Returns a human-readable description of the layout.
    #[must_use]
    pub fn description(&self) -> String {
        self.channels()
            .iter()
            .map(|c| c.label())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // 1. AudioChannel::label
    #[test]
    fn test_channel_labels() {
        assert_eq!(AudioChannel::FrontLeft.label(), "FL");
        assert_eq!(AudioChannel::LowFrequency.label(), "LFE");
        assert_eq!(AudioChannel::SideRight.label(), "SR");
    }

    // 2. AudioChannel::is_surround
    #[test]
    fn test_is_surround_true() {
        assert!(AudioChannel::BackLeft.is_surround());
        assert!(AudioChannel::SideRight.is_surround());
        assert!(AudioChannel::TopFrontLeft.is_surround());
    }

    #[test]
    fn test_is_surround_false() {
        assert!(!AudioChannel::FrontLeft.is_surround());
        assert!(!AudioChannel::LowFrequency.is_surround());
    }

    // 3. ChannelLayoutKind::channel_count
    #[test]
    fn test_layout_kind_channel_count() {
        assert_eq!(ChannelLayoutKind::Mono.channel_count(), 1);
        assert_eq!(ChannelLayoutKind::Stereo.channel_count(), 2);
        assert_eq!(ChannelLayoutKind::Surround51.channel_count(), 6);
        assert_eq!(ChannelLayoutKind::Surround71.channel_count(), 8);
        assert_eq!(ChannelLayoutKind::Custom.channel_count(), 0);
    }

    // 4. ChannelLayoutKind::has_lfe
    #[test]
    fn test_has_lfe() {
        assert!(ChannelLayoutKind::Surround51.has_lfe());
        assert!(ChannelLayoutKind::Surround71.has_lfe());
        assert!(!ChannelLayoutKind::Stereo.has_lfe());
        assert!(!ChannelLayoutKind::Mono.has_lfe());
    }

    // 5. ChannelLayout::standard – channel_count matches kind
    #[test]
    fn test_standard_layout_count() {
        let layout = ChannelLayout::standard(ChannelLayoutKind::Surround51);
        assert_eq!(layout.channel_count(), 6);
    }

    // 6. ChannelLayout::standard – contains check
    #[test]
    fn test_standard_layout_contains() {
        let layout = ChannelLayout::standard(ChannelLayoutKind::Stereo);
        assert!(layout.contains(AudioChannel::FrontLeft));
        assert!(layout.contains(AudioChannel::FrontRight));
        assert!(!layout.contains(AudioChannel::FrontCenter));
    }

    // 7. ChannelLayout::index_of – found
    #[test]
    fn test_index_of_found() {
        let layout = ChannelLayout::standard(ChannelLayoutKind::Surround51);
        // 5.1 order: FL FR FC LFE BL BR
        assert_eq!(layout.index_of(AudioChannel::FrontLeft), Some(0));
        assert_eq!(layout.index_of(AudioChannel::LowFrequency), Some(3));
        assert_eq!(layout.index_of(AudioChannel::BackRight), Some(5));
    }

    // 8. ChannelLayout::index_of – not found
    #[test]
    fn test_index_of_not_found() {
        let layout = ChannelLayout::standard(ChannelLayoutKind::Stereo);
        assert_eq!(layout.index_of(AudioChannel::FrontCenter), None);
    }

    // 9. ChannelLayout::custom
    #[test]
    fn test_custom_layout() {
        let layout = ChannelLayout::custom(vec![
            AudioChannel::FrontLeft,
            AudioChannel::FrontRight,
            AudioChannel::TopFrontLeft,
            AudioChannel::TopFrontRight,
        ]);
        assert_eq!(layout.channel_count(), 4);
        assert_eq!(layout.kind, ChannelLayoutKind::Custom);
        assert!(layout.contains(AudioChannel::TopFrontLeft));
    }

    // 10. ChannelLayout::description
    #[test]
    fn test_description_stereo() {
        let layout = ChannelLayout::standard(ChannelLayoutKind::Stereo);
        assert_eq!(layout.description(), "FL, FR");
    }

    // 11. ChannelLayout::description – mono
    #[test]
    fn test_description_mono() {
        let layout = ChannelLayout::standard(ChannelLayoutKind::Mono);
        assert_eq!(layout.description(), "FC");
    }

    // 12. 7.1 has side channels
    #[test]
    fn test_surround71_side_channels() {
        let layout = ChannelLayout::standard(ChannelLayoutKind::Surround71);
        assert!(layout.contains(AudioChannel::SideLeft));
        assert!(layout.contains(AudioChannel::SideRight));
    }

    // 13. 2.1 has LFE
    #[test]
    fn test_surround21_has_lfe() {
        let layout = ChannelLayout::standard(ChannelLayoutKind::Surround21);
        assert!(layout.contains(AudioChannel::LowFrequency));
        assert_eq!(layout.channel_count(), 3);
    }

    // 14. Custom layout index_of
    #[test]
    fn test_custom_index_of() {
        let layout = ChannelLayout::custom(vec![AudioChannel::SideLeft, AudioChannel::SideRight]);
        assert_eq!(layout.index_of(AudioChannel::SideRight), Some(1));
        assert_eq!(layout.index_of(AudioChannel::FrontLeft), None);
    }
}
