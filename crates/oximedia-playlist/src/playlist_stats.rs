#![allow(dead_code)]
//! Play statistics tracking for playlists and individual tracks.

use std::collections::HashMap;

/// Statistics for a single track across all play events.
#[derive(Debug, Clone, Default)]
pub struct PlayStats {
    /// Number of times the track was played to completion.
    pub complete_plays: u32,
    /// Number of times the track was skipped before completion.
    pub skips: u32,
    /// Cumulative seconds of playback time for this track.
    pub total_duration_secs: f64,
}

impl PlayStats {
    /// Creates a new zero-initialised `PlayStats`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the total number of play events (complete + skipped).
    pub fn total_plays(&self) -> u32 {
        self.complete_plays + self.skips
    }

    /// Returns the average duration per play event in seconds.
    ///
    /// Returns `0.0` if there have been no play events.
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_duration_secs(&self) -> f64 {
        let total = self.total_plays();
        if total == 0 {
            0.0
        } else {
            self.total_duration_secs / f64::from(total)
        }
    }

    /// Returns the completion rate as a value in `[0.0, 1.0]`.
    ///
    /// Returns `0.0` if there have been no play events.
    pub fn completion_rate(&self) -> f64 {
        let total = self.total_plays();
        if total == 0 {
            0.0
        } else {
            f64::from(self.complete_plays) / f64::from(total)
        }
    }
}

/// An individual play-event record.
#[derive(Debug, Clone)]
pub struct PlayEvent {
    /// Track URI or identifier.
    pub track_id: String,
    /// Duration actually played in seconds.
    pub played_secs: f64,
    /// Whether the track was played to completion.
    pub completed: bool,
}

impl PlayEvent {
    /// Creates a completed play event.
    pub fn completed(track_id: impl Into<String>, played_secs: f64) -> Self {
        Self {
            track_id: track_id.into(),
            played_secs,
            completed: true,
        }
    }

    /// Creates a skipped play event.
    pub fn skipped(track_id: impl Into<String>, played_secs: f64) -> Self {
        Self {
            track_id: track_id.into(),
            played_secs,
            completed: false,
        }
    }
}

/// Aggregated play statistics for an entire playlist.
#[derive(Debug, Default)]
pub struct PlaylistStats {
    /// Per-track statistics keyed by track ID.
    track_stats: HashMap<String, PlayStats>,
    /// Chronological history of play events.
    events: Vec<PlayEvent>,
}

impl PlaylistStats {
    /// Creates a new empty `PlaylistStats`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a play event and updates the underlying per-track statistics.
    pub fn record_play(&mut self, event: PlayEvent) {
        let stats = self.track_stats.entry(event.track_id.clone()).or_default();
        if event.completed {
            stats.complete_plays += 1;
        } else {
            stats.skips += 1;
        }
        stats.total_duration_secs += event.played_secs;
        self.events.push(event);
    }

    /// Returns the total number of play events recorded.
    pub fn total_plays(&self) -> usize {
        self.events.len()
    }

    /// Returns the number of unique tracks that have been played.
    pub fn unique_tracks(&self) -> usize {
        self.track_stats.len()
    }

    /// Returns the track ID with the most play events, or `None` if empty.
    pub fn most_played(&self) -> Option<&str> {
        self.track_stats
            .iter()
            .max_by_key(|(_, s)| s.total_plays())
            .map(|(id, _)| id.as_str())
    }

    /// Returns the `PlayStats` for a specific track, or `None` if not found.
    pub fn stats_for(&self, track_id: &str) -> Option<&PlayStats> {
        self.track_stats.get(track_id)
    }

    /// Returns the track with the highest completion rate, or `None` if empty.
    pub fn best_completion_rate_track(&self) -> Option<&str> {
        self.track_stats
            .iter()
            .filter(|(_, s)| s.total_plays() > 0)
            .max_by(|(_, a), (_, b)| {
                a.completion_rate()
                    .partial_cmp(&b.completion_rate())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(id, _)| id.as_str())
    }

    /// Returns the total number of skips across all tracks.
    pub fn total_skips(&self) -> u32 {
        self.track_stats.values().map(|s| s.skips).sum()
    }

    /// Clears all recorded statistics.
    pub fn reset(&mut self) {
        self.track_stats.clear();
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_play_stats_default_zero() {
        let s = PlayStats::new();
        assert_eq!(s.total_plays(), 0);
        assert_eq!(s.avg_duration_secs(), 0.0);
        assert_eq!(s.completion_rate(), 0.0);
    }

    #[test]
    fn test_play_stats_total_plays() {
        let s = PlayStats {
            complete_plays: 3,
            skips: 2,
            total_duration_secs: 0.0,
        };
        assert_eq!(s.total_plays(), 5);
    }

    #[test]
    fn test_avg_duration_secs() {
        let s = PlayStats {
            complete_plays: 2,
            skips: 0,
            total_duration_secs: 300.0,
        };
        assert!((s.avg_duration_secs() - 150.0).abs() < 1e-9);
    }

    #[test]
    fn test_completion_rate_full() {
        let s = PlayStats {
            complete_plays: 4,
            skips: 0,
            total_duration_secs: 0.0,
        };
        assert!((s.completion_rate() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_completion_rate_partial() {
        let s = PlayStats {
            complete_plays: 1,
            skips: 3,
            total_duration_secs: 0.0,
        };
        assert!((s.completion_rate() - 0.25).abs() < 1e-9);
    }

    #[test]
    fn test_record_play_completed() {
        let mut ps = PlaylistStats::new();
        ps.record_play(PlayEvent::completed("track_a", 180.0));
        assert_eq!(ps.total_plays(), 1);
        assert_eq!(ps.unique_tracks(), 1);
        let s = ps.stats_for("track_a").expect("should succeed in test");
        assert_eq!(s.complete_plays, 1);
        assert_eq!(s.skips, 0);
    }

    #[test]
    fn test_record_play_skipped() {
        let mut ps = PlaylistStats::new();
        ps.record_play(PlayEvent::skipped("track_b", 30.0));
        let s = ps.stats_for("track_b").expect("should succeed in test");
        assert_eq!(s.skips, 1);
        assert_eq!(s.complete_plays, 0);
    }

    #[test]
    fn test_unique_tracks() {
        let mut ps = PlaylistStats::new();
        ps.record_play(PlayEvent::completed("t1", 100.0));
        ps.record_play(PlayEvent::completed("t2", 200.0));
        ps.record_play(PlayEvent::skipped("t1", 50.0));
        assert_eq!(ps.unique_tracks(), 2);
    }

    #[test]
    fn test_most_played() {
        let mut ps = PlaylistStats::new();
        ps.record_play(PlayEvent::completed("popular", 100.0));
        ps.record_play(PlayEvent::completed("popular", 100.0));
        ps.record_play(PlayEvent::completed("rare", 100.0));
        assert_eq!(ps.most_played(), Some("popular"));
    }

    #[test]
    fn test_most_played_empty() {
        let ps = PlaylistStats::new();
        assert!(ps.most_played().is_none());
    }

    #[test]
    fn test_total_skips() {
        let mut ps = PlaylistStats::new();
        ps.record_play(PlayEvent::skipped("t1", 10.0));
        ps.record_play(PlayEvent::skipped("t1", 5.0));
        ps.record_play(PlayEvent::completed("t2", 60.0));
        assert_eq!(ps.total_skips(), 2);
    }

    #[test]
    fn test_reset() {
        let mut ps = PlaylistStats::new();
        ps.record_play(PlayEvent::completed("t1", 60.0));
        ps.reset();
        assert_eq!(ps.total_plays(), 0);
        assert_eq!(ps.unique_tracks(), 0);
    }

    #[test]
    fn test_stats_for_unknown_track() {
        let ps = PlaylistStats::new();
        assert!(ps.stats_for("nonexistent").is_none());
    }

    #[test]
    fn test_best_completion_rate_track() {
        let mut ps = PlaylistStats::new();
        // t1: 2 complete, 2 skip → 50%
        ps.record_play(PlayEvent::completed("t1", 60.0));
        ps.record_play(PlayEvent::completed("t1", 60.0));
        ps.record_play(PlayEvent::skipped("t1", 10.0));
        ps.record_play(PlayEvent::skipped("t1", 10.0));
        // t2: 3 complete, 0 skip → 100%
        ps.record_play(PlayEvent::completed("t2", 90.0));
        ps.record_play(PlayEvent::completed("t2", 90.0));
        ps.record_play(PlayEvent::completed("t2", 90.0));
        assert_eq!(ps.best_completion_rate_track(), Some("t2"));
    }
}
