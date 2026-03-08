//! Automated highlight clip generation from playout.
//!
//! Monitors playout and captures highlight clips based on configurable
//! triggers such as sports scores, keyword detection, bookmarks, or manual
//! operator marking.

#![allow(dead_code)]

/// What caused a highlight clip to be created.
#[derive(Debug, Clone, PartialEq)]
pub enum HighlightTrigger {
    /// Operator manually marked the highlight
    Manual,
    /// Triggered by a sports score event
    SportsScore,
    /// Triggered by keyword detection in commentary or captions
    KeywordDetect,
    /// Triggered by wall-clock time schedule
    TimeBased,
    /// Operator set a bookmark during playout
    BookmarkSet,
}

impl HighlightTrigger {
    /// Returns true if this trigger is generated automatically (without
    /// operator action).
    pub fn is_automatic(&self) -> bool {
        matches!(
            self,
            Self::SportsScore | Self::KeywordDetect | Self::TimeBased
        )
    }
}

/// A highlight clip defined by in/out timecodes.
#[derive(Debug, Clone)]
pub struct HighlightClip {
    /// Unique clip identifier
    pub id: u64,
    /// In-point timecode (frame number)
    pub in_tc: u64,
    /// Out-point timecode (frame number)
    pub out_tc: u64,
    /// What triggered this highlight
    pub trigger: HighlightTrigger,
    /// Human-readable label
    pub label: String,
}

impl HighlightClip {
    /// Return the duration of this clip in frames.
    ///
    /// Returns 0 if out_tc <= in_tc.
    pub fn duration_frames(&self) -> u64 {
        self.out_tc.saturating_sub(self.in_tc)
    }

    /// Returns true if this clip is shorter than `min_duration` frames.
    pub fn is_short(&self, min_duration: u64) -> bool {
        self.duration_frames() < min_duration
    }
}

/// Engine that records and queries highlight clips.
#[derive(Debug, Clone)]
pub struct HighlightEngine {
    /// All captured highlight clips in chronological order
    pub clips: Vec<HighlightClip>,
    /// Counter used to assign unique IDs
    pub next_id: u64,
    /// Clips shorter than this (in frames) are rejected
    pub min_duration_frames: u64,
}

impl HighlightEngine {
    /// Create a new engine that rejects clips shorter than `min_duration` frames.
    pub fn new(min_duration: u64) -> Self {
        Self {
            clips: Vec::new(),
            next_id: 1,
            min_duration_frames: min_duration,
        }
    }

    /// Attempt to add a new highlight clip.
    ///
    /// Returns `Some(id)` if the clip was accepted (duration >= minimum), or
    /// `None` if the clip was too short or the out-point is before the
    /// in-point.
    pub fn add_highlight(
        &mut self,
        in_tc: u64,
        out_tc: u64,
        trigger: HighlightTrigger,
        label: impl Into<String>,
    ) -> Option<u64> {
        let duration = out_tc.saturating_sub(in_tc);
        if duration < self.min_duration_frames {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.clips.push(HighlightClip {
            id,
            in_tc,
            out_tc,
            trigger,
            label: label.into(),
        });
        Some(id)
    }

    /// Return references to the most recent `count` clips (most-recent last).
    ///
    /// If `count` exceeds the total number of clips, all clips are returned.
    pub fn recent_clips(&self, count: usize) -> Vec<&HighlightClip> {
        let start = self.clips.len().saturating_sub(count);
        self.clips[start..].iter().collect()
    }

    /// Return references to all clips with the given trigger type.
    pub fn clips_by_trigger(&self, t: &HighlightTrigger) -> Vec<&HighlightClip> {
        self.clips.iter().filter(|c| &c.trigger == t).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- HighlightTrigger tests ---

    #[test]
    fn test_sports_score_is_automatic() {
        assert!(HighlightTrigger::SportsScore.is_automatic());
    }

    #[test]
    fn test_keyword_detect_is_automatic() {
        assert!(HighlightTrigger::KeywordDetect.is_automatic());
    }

    #[test]
    fn test_time_based_is_automatic() {
        assert!(HighlightTrigger::TimeBased.is_automatic());
    }

    #[test]
    fn test_manual_is_not_automatic() {
        assert!(!HighlightTrigger::Manual.is_automatic());
    }

    #[test]
    fn test_bookmark_is_not_automatic() {
        assert!(!HighlightTrigger::BookmarkSet.is_automatic());
    }

    // --- HighlightClip tests ---

    #[test]
    fn test_clip_duration_frames() {
        let clip = HighlightClip {
            id: 1,
            in_tc: 100,
            out_tc: 150,
            trigger: HighlightTrigger::Manual,
            label: "Goal".into(),
        };
        assert_eq!(clip.duration_frames(), 50);
    }

    #[test]
    fn test_clip_duration_frames_zero_when_out_before_in() {
        let clip = HighlightClip {
            id: 2,
            in_tc: 200,
            out_tc: 100,
            trigger: HighlightTrigger::Manual,
            label: "Invalid".into(),
        };
        assert_eq!(clip.duration_frames(), 0);
    }

    #[test]
    fn test_clip_is_short_true() {
        let clip = HighlightClip {
            id: 3,
            in_tc: 0,
            out_tc: 10,
            trigger: HighlightTrigger::Manual,
            label: "Short".into(),
        };
        assert!(clip.is_short(25));
    }

    #[test]
    fn test_clip_is_short_false() {
        let clip = HighlightClip {
            id: 4,
            in_tc: 0,
            out_tc: 50,
            trigger: HighlightTrigger::Manual,
            label: "Long".into(),
        };
        assert!(!clip.is_short(25));
    }

    // --- HighlightEngine tests ---

    #[test]
    fn test_engine_add_highlight_returns_id() {
        let mut eng = HighlightEngine::new(25);
        let id = eng.add_highlight(0, 50, HighlightTrigger::Manual, "Test");
        assert_eq!(id, Some(1));
    }

    #[test]
    fn test_engine_add_highlight_too_short_returns_none() {
        let mut eng = HighlightEngine::new(25);
        let id = eng.add_highlight(0, 10, HighlightTrigger::Manual, "Too short");
        assert!(id.is_none());
    }

    #[test]
    fn test_engine_ids_increment() {
        let mut eng = HighlightEngine::new(10);
        let id1 = eng.add_highlight(0, 50, HighlightTrigger::Manual, "A");
        let id2 = eng.add_highlight(100, 200, HighlightTrigger::BookmarkSet, "B");
        assert_eq!(id1, Some(1));
        assert_eq!(id2, Some(2));
    }

    #[test]
    fn test_engine_recent_clips_count() {
        let mut eng = HighlightEngine::new(10);
        for i in 0..5u64 {
            eng.add_highlight(i * 100, i * 100 + 50, HighlightTrigger::Manual, "x");
        }
        let recent = eng.recent_clips(3);
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn test_engine_recent_clips_exceeds_total() {
        let mut eng = HighlightEngine::new(10);
        eng.add_highlight(0, 50, HighlightTrigger::Manual, "only");
        let recent = eng.recent_clips(10);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn test_engine_clips_by_trigger() {
        let mut eng = HighlightEngine::new(10);
        eng.add_highlight(0, 50, HighlightTrigger::Manual, "A");
        eng.add_highlight(100, 200, HighlightTrigger::SportsScore, "B");
        eng.add_highlight(300, 400, HighlightTrigger::SportsScore, "C");
        let sports = eng.clips_by_trigger(&HighlightTrigger::SportsScore);
        assert_eq!(sports.len(), 2);
    }

    #[test]
    fn test_engine_clips_by_trigger_empty() {
        let eng = HighlightEngine::new(10);
        let found = eng.clips_by_trigger(&HighlightTrigger::KeywordDetect);
        assert!(found.is_empty());
    }
}
