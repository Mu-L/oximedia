//! Gaming clip and highlight management.
//!
//! Provides types for defining how clips are triggered, assembling highlight
//! reels, and choosing export formats.

#![allow(dead_code)]

// ---------------------------------------------------------------------------
// ClipTrigger
// ---------------------------------------------------------------------------

/// What caused a gaming clip to be saved.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipTrigger {
    /// Manually triggered by the user.
    Manual,
    /// An in-game achievement was unlocked.
    Achievement,
    /// A kill-streak of the given count was reached.
    KillStreak(u32),
    /// Player health dropped at or below the given threshold (0.0–1.0).
    HealthThreshold(f32),
    /// A custom/plugin-defined trigger.
    Custom(String),
}

impl ClipTrigger {
    /// Returns `true` for triggers that fire automatically without user input
    /// (`Achievement`, `KillStreak`, `HealthThreshold`).
    #[must_use]
    pub fn is_automatic(&self) -> bool {
        matches!(
            self,
            Self::Achievement | Self::KillStreak(_) | Self::HealthThreshold(_)
        )
    }
}

// ---------------------------------------------------------------------------
// GamingClip
// ---------------------------------------------------------------------------

/// A saved gaming clip with timing metadata.
#[derive(Debug, Clone)]
pub struct GamingClip {
    /// Unique clip identifier.
    pub id: u64,
    /// What caused this clip to be saved.
    pub triggered_by: ClipTrigger,
    /// Milliseconds of footage before the trigger point.
    pub pre_roll_ms: u32,
    /// Milliseconds of footage after the trigger point.
    pub post_roll_ms: u32,
    /// Clip start time in milliseconds since recording began.
    pub start_ms: u64,
    /// Clip end time in milliseconds since recording began.
    pub end_ms: u64,
}

impl GamingClip {
    /// Total clip duration in milliseconds.
    #[must_use]
    pub fn duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }

    /// Returns `true` when the clip has a non-zero pre-roll buffer.
    #[must_use]
    pub fn has_pre_roll(&self) -> bool {
        self.pre_roll_ms > 0
    }
}

// ---------------------------------------------------------------------------
// HighlightReel
// ---------------------------------------------------------------------------

/// An ordered collection of clips assembled into a highlight reel.
#[derive(Debug, Default)]
pub struct HighlightReel {
    /// All clips in the reel.
    pub clips: Vec<GamingClip>,
    /// Desired maximum reel duration in milliseconds.
    pub target_duration_ms: u64,
}

impl HighlightReel {
    /// Create a new highlight reel with a target maximum duration.
    #[must_use]
    pub fn new(target_duration_ms: u64) -> Self {
        Self {
            clips: Vec::new(),
            target_duration_ms,
        }
    }

    /// Append a clip to the reel.
    pub fn add_clip(&mut self, clip: GamingClip) {
        self.clips.push(clip);
    }

    /// Select the `max_clips` most-recently-added clips.
    ///
    /// The returned vector is in chronological order (oldest first).
    #[must_use]
    pub fn auto_select(&self, max_clips: usize) -> Vec<&GamingClip> {
        let skip = self.clips.len().saturating_sub(max_clips);
        self.clips.iter().skip(skip).collect()
    }

    /// Sum of all clip durations in milliseconds.
    #[must_use]
    pub fn total_duration_ms(&self) -> u64 {
        self.clips.iter().map(GamingClip::duration_ms).sum()
    }

    /// Returns `true` when the total duration exceeds the target.
    #[must_use]
    pub fn needs_trim(&self) -> bool {
        self.total_duration_ms() > self.target_duration_ms
    }
}

// ---------------------------------------------------------------------------
// ClipExportFormat
// ---------------------------------------------------------------------------

/// Container / codec combination used when exporting a clip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipExportFormat {
    /// MP4 container with H.264 video.
    Mp4H264,
    /// MP4 container with H.265 / HEVC video.
    Mp4Hevc,
    /// `WebM` container (VP8/VP9).
    WebM,
    /// Animated GIF (no audio, limited colour depth).
    Gif,
}

impl ClipExportFormat {
    /// Canonical file extension for this format (without leading dot).
    #[must_use]
    pub fn extension(&self) -> &str {
        match self {
            Self::Mp4H264 | Self::Mp4Hevc => "mp4",
            Self::WebM => "webm",
            Self::Gif => "gif",
        }
    }

    /// Returns `true` when the format can carry HDR metadata.
    ///
    /// Only `Mp4Hevc` supports HDR in this implementation.
    #[must_use]
    pub fn supports_hdr(&self) -> bool {
        *self == Self::Mp4Hevc
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_clip(id: u64, start: u64, end: u64) -> GamingClip {
        GamingClip {
            id,
            triggered_by: ClipTrigger::Manual,
            pre_roll_ms: 5000,
            post_roll_ms: 3000,
            start_ms: start,
            end_ms: end,
        }
    }

    // ClipTrigger

    #[test]
    fn test_manual_not_automatic() {
        assert!(!ClipTrigger::Manual.is_automatic());
    }

    #[test]
    fn test_achievement_is_automatic() {
        assert!(ClipTrigger::Achievement.is_automatic());
    }

    #[test]
    fn test_kill_streak_is_automatic() {
        assert!(ClipTrigger::KillStreak(5).is_automatic());
    }

    #[test]
    fn test_health_threshold_is_automatic() {
        assert!(ClipTrigger::HealthThreshold(0.2).is_automatic());
    }

    #[test]
    fn test_custom_not_automatic() {
        assert!(!ClipTrigger::Custom("plugin".to_string()).is_automatic());
    }

    // GamingClip

    #[test]
    fn test_clip_duration_ms() {
        let c = make_clip(1, 10_000, 40_000);
        assert_eq!(c.duration_ms(), 30_000);
    }

    #[test]
    fn test_clip_duration_zero_end_before_start() {
        let c = make_clip(1, 50_000, 30_000);
        assert_eq!(c.duration_ms(), 0);
    }

    #[test]
    fn test_clip_has_pre_roll_true() {
        let c = make_clip(1, 0, 10_000);
        assert!(c.has_pre_roll());
    }

    #[test]
    fn test_clip_has_pre_roll_false() {
        let mut c = make_clip(1, 0, 10_000);
        c.pre_roll_ms = 0;
        assert!(!c.has_pre_roll());
    }

    // HighlightReel

    #[test]
    fn test_reel_total_duration() {
        let mut reel = HighlightReel::new(60_000);
        reel.add_clip(make_clip(1, 0, 10_000));
        reel.add_clip(make_clip(2, 10_000, 30_000));
        assert_eq!(reel.total_duration_ms(), 30_000);
    }

    #[test]
    fn test_reel_needs_trim_true() {
        let mut reel = HighlightReel::new(15_000);
        reel.add_clip(make_clip(1, 0, 10_000));
        reel.add_clip(make_clip(2, 10_000, 20_000));
        assert!(reel.needs_trim());
    }

    #[test]
    fn test_reel_needs_trim_false() {
        let mut reel = HighlightReel::new(60_000);
        reel.add_clip(make_clip(1, 0, 10_000));
        assert!(!reel.needs_trim());
    }

    #[test]
    fn test_reel_auto_select_count() {
        let mut reel = HighlightReel::new(120_000);
        for i in 0..5u64 {
            reel.add_clip(make_clip(i, i * 10_000, (i + 1) * 10_000));
        }
        let selected = reel.auto_select(3);
        assert_eq!(selected.len(), 3);
    }

    #[test]
    fn test_reel_auto_select_most_recent() {
        let mut reel = HighlightReel::new(120_000);
        reel.add_clip(make_clip(1, 0, 10_000));
        reel.add_clip(make_clip(2, 10_000, 20_000));
        reel.add_clip(make_clip(3, 20_000, 30_000));
        let selected = reel.auto_select(2);
        assert_eq!(selected[0].id, 2);
        assert_eq!(selected[1].id, 3);
    }

    #[test]
    fn test_reel_auto_select_more_than_available() {
        let mut reel = HighlightReel::new(120_000);
        reel.add_clip(make_clip(1, 0, 5_000));
        let selected = reel.auto_select(10);
        assert_eq!(selected.len(), 1);
    }

    // ClipExportFormat

    #[test]
    fn test_mp4_h264_extension() {
        assert_eq!(ClipExportFormat::Mp4H264.extension(), "mp4");
    }

    #[test]
    fn test_mp4_hevc_extension() {
        assert_eq!(ClipExportFormat::Mp4Hevc.extension(), "mp4");
    }

    #[test]
    fn test_webm_extension() {
        assert_eq!(ClipExportFormat::WebM.extension(), "webm");
    }

    #[test]
    fn test_gif_extension() {
        assert_eq!(ClipExportFormat::Gif.extension(), "gif");
    }

    #[test]
    fn test_hevc_supports_hdr() {
        assert!(ClipExportFormat::Mp4Hevc.supports_hdr());
    }

    #[test]
    fn test_h264_no_hdr() {
        assert!(!ClipExportFormat::Mp4H264.supports_hdr());
    }
}
