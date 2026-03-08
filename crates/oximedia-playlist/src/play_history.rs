//! Play history and recently-played tracking.
//!
//! Records play events per track and provides queries for play counts,
//! recently played tracks, and most-played tracks.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

const SECONDS_PER_DAY: u64 = 86_400;

/// A single play event recorded in the history.
#[derive(Debug, Clone)]
pub struct PlayEvent {
    /// The track that was played.
    pub track_id: u64,
    /// Unix epoch timestamp of when playback started (seconds).
    pub timestamp_epoch: u64,
    /// Whether the track was played to completion.
    pub completed: bool,
    /// Cumulative play count for this track at the time of recording.
    pub play_count: u32,
}

impl PlayEvent {
    /// Returns `true` when the track was played to completion.
    pub fn is_full_play(&self) -> bool {
        self.completed
    }

    /// Returns how many days ago this event occurred relative to `now` (epoch seconds).
    pub fn age_days(&self, now: u64) -> f32 {
        let elapsed_secs = now.saturating_sub(self.timestamp_epoch);
        elapsed_secs as f32 / SECONDS_PER_DAY as f32
    }
}

/// In-memory play history with a capped number of entries.
#[derive(Debug, Clone)]
pub struct PlayHistory {
    /// Recorded events, ordered from oldest to newest.
    pub events: Vec<PlayEvent>,
    /// Maximum number of events retained.
    pub max_entries: usize,
}

impl PlayHistory {
    /// Creates an empty history with the given capacity cap.
    pub fn new(max_entries: usize) -> Self {
        Self {
            events: Vec::new(),
            max_entries,
        }
    }

    /// Records a play event for `track_id`.
    ///
    /// When `max_entries` is exceeded the oldest entry is evicted.
    pub fn record_play(&mut self, track_id: u64, epoch: u64, completed: bool) {
        let count = self.play_count(track_id) + 1;
        self.events.push(PlayEvent {
            track_id,
            timestamp_epoch: epoch,
            completed,
            play_count: count,
        });
        if self.max_entries > 0 && self.events.len() > self.max_entries {
            self.events.remove(0);
        }
    }

    /// Returns the total number of times `track_id` appears in the history.
    pub fn play_count(&self, track_id: u64) -> u32 {
        self.events
            .iter()
            .filter(|e| e.track_id == track_id)
            .count() as u32
    }

    /// Returns up to `limit` track ids from the most recently played events
    /// (newest first, de-duplicated).
    pub fn recently_played(&self, limit: usize) -> Vec<u64> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for event in self.events.iter().rev() {
            if seen.insert(event.track_id) {
                result.push(event.track_id);
                if result.len() >= limit {
                    break;
                }
            }
        }
        result
    }

    /// Returns up to `limit` `(track_id, play_count)` pairs sorted by play
    /// count descending.
    pub fn most_played(&self, limit: usize) -> Vec<(u64, u32)> {
        let mut counts: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();
        for event in &self.events {
            *counts.entry(event.track_id).or_insert(0) += 1;
        }
        let mut sorted: Vec<(u64, u32)> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        sorted.truncate(limit);
        sorted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- PlayEvent ---

    fn make_event(track_id: u64, completed: bool, epoch: u64) -> PlayEvent {
        PlayEvent {
            track_id,
            timestamp_epoch: epoch,
            completed,
            play_count: 1,
        }
    }

    #[test]
    fn test_is_full_play_true() {
        let e = make_event(1, true, 1_000_000);
        assert!(e.is_full_play());
    }

    #[test]
    fn test_is_full_play_false() {
        let e = make_event(1, false, 1_000_000);
        assert!(!e.is_full_play());
    }

    #[test]
    fn test_age_days_one_day() {
        let e = make_event(1, true, 0);
        let now = SECONDS_PER_DAY;
        let age = e.age_days(now);
        assert!((age - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_age_days_zero() {
        let e = make_event(1, true, 1_000);
        let age = e.age_days(1_000);
        assert_eq!(age, 0.0);
    }

    #[test]
    fn test_age_days_future_now_saturates() {
        // now < timestamp should not panic
        let e = make_event(1, true, 5_000);
        let age = e.age_days(1_000);
        assert_eq!(age, 0.0);
    }

    // --- PlayHistory ---

    fn make_history() -> PlayHistory {
        let mut h = PlayHistory::new(100);
        h.record_play(10, 1_000, true);
        h.record_play(20, 2_000, false);
        h.record_play(10, 3_000, true);
        h.record_play(30, 4_000, true);
        h.record_play(20, 5_000, true);
        h.record_play(10, 6_000, false);
        h
    }

    #[test]
    fn test_play_count_track() {
        let h = make_history();
        assert_eq!(h.play_count(10), 3);
    }

    #[test]
    fn test_play_count_other_track() {
        let h = make_history();
        assert_eq!(h.play_count(20), 2);
    }

    #[test]
    fn test_play_count_unknown_track() {
        let h = make_history();
        assert_eq!(h.play_count(99), 0);
    }

    #[test]
    fn test_recently_played_order() {
        let h = make_history();
        // Last played order: 10 (epoch 6000), 20 (5000), 30 (4000)
        let recent = h.recently_played(3);
        assert_eq!(recent[0], 10);
        assert_eq!(recent[1], 20);
        assert_eq!(recent[2], 30);
    }

    #[test]
    fn test_recently_played_deduplication() {
        let h = make_history();
        // track 10 played 3 times but should appear once
        let recent = h.recently_played(10);
        let count_10 = recent.iter().filter(|&&id| id == 10).count();
        assert_eq!(count_10, 1);
    }

    #[test]
    fn test_recently_played_limit() {
        let h = make_history();
        let recent = h.recently_played(1);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn test_most_played_order() {
        let h = make_history();
        // track 10 → 3 plays, 20 → 2 plays, 30 → 1 play
        let top = h.most_played(3);
        assert_eq!(top[0].0, 10);
        assert_eq!(top[0].1, 3);
        assert_eq!(top[1].0, 20);
        assert_eq!(top[1].1, 2);
    }

    #[test]
    fn test_most_played_limit() {
        let h = make_history();
        let top = h.most_played(2);
        assert_eq!(top.len(), 2);
    }

    #[test]
    fn test_max_entries_eviction() {
        let mut h = PlayHistory::new(3);
        h.record_play(1, 100, true);
        h.record_play(2, 200, true);
        h.record_play(3, 300, true);
        h.record_play(4, 400, true);
        // oldest (track 1 at epoch 100) should be evicted
        assert_eq!(h.events.len(), 3);
        assert!(h.events.iter().all(|e| e.track_id != 1));
    }

    #[test]
    fn test_record_play_increments_count() {
        let mut h = PlayHistory::new(100);
        h.record_play(42, 1_000, true);
        h.record_play(42, 2_000, true);
        assert_eq!(h.play_count(42), 2);
    }
}
