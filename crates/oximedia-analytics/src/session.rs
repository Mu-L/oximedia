//! Viewer session analytics — playback events, session metrics, playback maps,
//! and attention heatmaps.

/// A discrete event emitted during media playback.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackEvent {
    /// The viewer pressed play.
    Play { timestamp_ms: i64 },
    /// The viewer paused at a given content position.
    Pause { timestamp_ms: i64, position_ms: u64 },
    /// The viewer scrubbed from one position to another.
    Seek { from_ms: u64, to_ms: u64 },
    /// Buffering started at a content position.
    BufferStart { position_ms: u64 },
    /// Buffering ended; `duration_ms` is how long the stall lasted.
    BufferEnd { position_ms: u64, duration_ms: u32 },
    /// The player switched quality levels.
    QualityChange {
        from_height: u32,
        to_height: u32,
        bitrate: u32,
    },
    /// Playback reached the end (or the user closed the player).
    End {
        position_ms: u64,
        watch_duration_ms: u64,
    },
}

/// A single viewing session for one piece of content.
#[derive(Debug, Clone)]
pub struct ViewerSession {
    pub session_id: String,
    pub user_id: Option<String>,
    pub content_id: String,
    /// Wall-clock start time of the session (Unix epoch ms).
    pub started_at_ms: i64,
    pub events: Vec<PlaybackEvent>,
}

impl ViewerSession {
    /// Create a new empty session.
    pub fn new(
        session_id: impl Into<String>,
        user_id: Option<String>,
        content_id: impl Into<String>,
        started_at_ms: i64,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            user_id,
            content_id: content_id.into(),
            started_at_ms,
            events: Vec::new(),
        }
    }

    /// Append a playback event.
    pub fn push_event(&mut self, event: PlaybackEvent) {
        self.events.push(event);
    }
}

/// Aggregate metrics derived from a single `ViewerSession`.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionMetrics {
    /// Total milliseconds of content actually watched (sum of watch chunks).
    pub total_watch_ms: u64,
    /// Number of unique 1-second positions watched (distinct content seconds).
    pub unique_positions_watched: u64,
    /// How many `Seek` events were recorded.
    pub seek_count: u32,
    /// How many buffering interruptions occurred.
    pub buffer_events: u32,
    /// Total stall time in milliseconds.
    pub buffer_time_ms: u64,
    /// How many quality-level switches happened.
    pub quality_changes: u32,
    /// What fraction of the content was completed (0.0 – 100.0).
    pub completion_pct: f32,
}

/// Analyse a session and return aggregate metrics.
///
/// `content_duration_ms` is required only to compute `completion_pct` and the
/// unique-position count; pass `0` if unknown (completion will be `0.0`).
pub fn analyze_session(session: &ViewerSession, content_duration_ms: u64) -> SessionMetrics {
    let mut seek_count: u32 = 0;
    let mut buffer_events: u32 = 0;
    let mut buffer_time_ms: u64 = 0;
    let mut quality_changes: u32 = 0;
    let mut max_position_ms: u64 = 0;
    let mut total_watch_ms: u64 = 0;

    // We reconstruct watch intervals by treating Play→Pause/Seek/End pairs.
    // A simple heuristic: use the `watch_duration_ms` field in `End` events,
    // and for sessions without an End event fall back to the last known position.
    let mut last_end_watch_ms: Option<u64> = None;

    for event in &session.events {
        match event {
            PlaybackEvent::Seek { from_ms, to_ms } => {
                seek_count += 1;
                max_position_ms = max_position_ms.max(*from_ms).max(*to_ms);
            }
            PlaybackEvent::BufferStart { position_ms } => {
                buffer_events += 1;
                max_position_ms = max_position_ms.max(*position_ms);
            }
            PlaybackEvent::BufferEnd {
                position_ms,
                duration_ms,
            } => {
                buffer_time_ms += u64::from(*duration_ms);
                max_position_ms = max_position_ms.max(*position_ms);
            }
            PlaybackEvent::QualityChange { .. } => {
                quality_changes += 1;
            }
            PlaybackEvent::End {
                position_ms,
                watch_duration_ms,
            } => {
                max_position_ms = max_position_ms.max(*position_ms);
                last_end_watch_ms = Some(*watch_duration_ms);
                total_watch_ms = total_watch_ms.max(*watch_duration_ms);
            }
            PlaybackEvent::Pause { position_ms, .. } => {
                max_position_ms = max_position_ms.max(*position_ms);
            }
            PlaybackEvent::Play { .. } => {}
        }
    }

    if let Some(w) = last_end_watch_ms {
        total_watch_ms = w;
    }

    // Build playback map for unique positions.
    let map = build_playback_map(session, content_duration_ms);
    let unique_positions_watched = if content_duration_ms > 0 {
        map.positions_watched.iter().filter(|&&b| b).count() as u64
    } else {
        0
    };

    let completion_pct = if content_duration_ms > 0 {
        (max_position_ms as f32 / content_duration_ms as f32 * 100.0).min(100.0)
    } else {
        0.0
    };

    SessionMetrics {
        total_watch_ms,
        unique_positions_watched,
        seek_count,
        buffer_events,
        buffer_time_ms,
        quality_changes,
        completion_pct,
    }
}

/// A boolean map of 1-second content buckets indicating which positions were
/// watched during a session.
#[derive(Debug, Clone)]
pub struct PlaybackMap {
    /// One `bool` per second of content (`true` = watched).
    pub positions_watched: Vec<bool>,
}

impl PlaybackMap {
    /// Mark every second in `[start_ms, end_ms)` as watched.
    pub fn mark_range(&mut self, start_ms: u64, end_ms: u64) {
        if start_ms >= end_ms || self.positions_watched.is_empty() {
            return;
        }
        let start_sec = (start_ms / 1000) as usize;
        let end_sec = ((end_ms + 999) / 1000) as usize; // round up
        let cap = self.positions_watched.len();
        let end_sec = end_sec.min(cap);
        for i in start_sec..end_sec {
            self.positions_watched[i] = true;
        }
    }

    /// Fraction of the content that was watched (0.0 – 1.0).
    pub fn coverage_pct(&self, total_ms: u64) -> f32 {
        if total_ms == 0 || self.positions_watched.is_empty() {
            return 0.0;
        }
        let total_sec = ((total_ms + 999) / 1000) as usize;
        let total_sec = total_sec.min(self.positions_watched.len());
        if total_sec == 0 {
            return 0.0;
        }
        let watched = self.positions_watched[..total_sec]
            .iter()
            .filter(|&&b| b)
            .count();
        watched as f32 / total_sec as f32
    }
}

/// Analyze multiple sessions in batch, returning one `SessionMetrics` per session.
pub fn analyze_sessions_batch(
    sessions: &[ViewerSession],
    content_duration_ms: u64,
) -> Vec<SessionMetrics> {
    sessions
        .iter()
        .map(|s| analyze_session(s, content_duration_ms))
        .collect()
}

/// Build a `PlaybackMap` from a session's events.
///
/// The function reconstructs watch intervals by pairing `Play` events with
/// subsequent `Pause`, `Seek`, or `End` events.  A running `current_position`
/// advances with each event so the heuristic is robust to sessions that only
/// contain a single `End` event.
pub fn build_playback_map(session: &ViewerSession, content_duration_ms: u64) -> PlaybackMap {
    let num_seconds = if content_duration_ms > 0 {
        ((content_duration_ms + 999) / 1000) as usize
    } else {
        // Derive a reasonable size from the maximum event position.
        let max_pos = session.events.iter().fold(0u64, |acc, e| match e {
            PlaybackEvent::Pause { position_ms, .. } => acc.max(*position_ms),
            PlaybackEvent::Seek { from_ms, to_ms } => acc.max(*from_ms).max(*to_ms),
            PlaybackEvent::BufferStart { position_ms } => acc.max(*position_ms),
            PlaybackEvent::BufferEnd { position_ms, .. } => acc.max(*position_ms),
            PlaybackEvent::End { position_ms, .. } => acc.max(*position_ms),
            _ => acc,
        });
        if max_pos == 0 {
            return PlaybackMap {
                positions_watched: Vec::new(),
            };
        }
        ((max_pos + 999) / 1000) as usize + 1
    };

    let mut map = PlaybackMap {
        positions_watched: vec![false; num_seconds],
    };

    // State machine: track whether we are currently "playing" and from where.
    let mut playing = false;
    let mut play_start_pos: u64 = 0;
    let mut current_pos: u64 = 0;

    for event in &session.events {
        match event {
            PlaybackEvent::Play { .. } => {
                playing = true;
                play_start_pos = current_pos;
            }
            PlaybackEvent::Pause { position_ms, .. } => {
                if playing {
                    map.mark_range(play_start_pos, *position_ms);
                    playing = false;
                }
                current_pos = *position_ms;
            }
            PlaybackEvent::Seek { from_ms, to_ms } => {
                if playing {
                    map.mark_range(play_start_pos, *from_ms);
                }
                current_pos = *to_ms;
                if playing {
                    play_start_pos = *to_ms;
                }
            }
            PlaybackEvent::BufferStart { position_ms } => {
                if playing {
                    map.mark_range(play_start_pos, *position_ms);
                    // Pause tracking during buffer stall.
                    play_start_pos = *position_ms;
                }
                current_pos = *position_ms;
            }
            PlaybackEvent::BufferEnd { position_ms, .. } => {
                current_pos = *position_ms;
                if playing {
                    play_start_pos = *position_ms;
                }
            }
            PlaybackEvent::End { position_ms, .. } => {
                if playing {
                    map.mark_range(play_start_pos, *position_ms);
                    playing = false;
                }
                current_pos = *position_ms;
            }
            PlaybackEvent::QualityChange { .. } => {}
        }
    }

    // If still "playing" at end of event list (session cut off), mark up to last position.
    if playing && current_pos > play_start_pos {
        map.mark_range(play_start_pos, current_pos);
    }

    map
}

/// A single point on an attention heatmap.
#[derive(Debug, Clone, PartialEq)]
pub struct HeatPoint {
    /// Content position in milliseconds (start of bucket).
    pub position_ms: u64,
    /// Normalised viewer attention intensity (0.0 – 1.0).
    pub intensity: f32,
}

/// Compute an attention heatmap across a collection of sessions.
///
/// The content is divided into `bucket_ms`-wide buckets.  For each bucket the
/// intensity is the fraction of sessions that watched any part of that bucket,
/// normalised to the maximum bucket count so the peak bucket always has
/// intensity 1.0.
pub fn attention_heatmap(
    sessions: &[ViewerSession],
    content_duration_ms: u64,
    bucket_ms: u32,
) -> Vec<HeatPoint> {
    if sessions.is_empty() || content_duration_ms == 0 || bucket_ms == 0 {
        return Vec::new();
    }

    let bucket_ms_u64 = u64::from(bucket_ms);
    let num_buckets = ((content_duration_ms + bucket_ms_u64 - 1) / bucket_ms_u64) as usize;
    let mut counts = vec![0u32; num_buckets];

    for session in sessions {
        let map = build_playback_map(session, content_duration_ms);
        // Aggregate per-bucket: bucket is "watched" if any second inside it was watched.
        for (bucket_idx, count) in counts.iter_mut().enumerate() {
            let bucket_start_ms = bucket_idx as u64 * bucket_ms_u64;
            let bucket_end_ms = (bucket_start_ms + bucket_ms_u64).min(content_duration_ms);
            let start_sec = (bucket_start_ms / 1000) as usize;
            let end_sec = ((bucket_end_ms + 999) / 1000) as usize;
            let end_sec = end_sec.min(map.positions_watched.len());
            let watched = (start_sec..end_sec)
                .any(|s| map.positions_watched.get(s).copied().unwrap_or(false));
            if watched {
                *count += 1;
            }
        }
    }

    let max_count = counts.iter().copied().max().unwrap_or(0);
    if max_count == 0 {
        return Vec::new();
    }

    counts
        .into_iter()
        .enumerate()
        .map(|(idx, c)| HeatPoint {
            position_ms: idx as u64 * bucket_ms_u64,
            intensity: c as f32 / max_count as f32,
        })
        .collect()
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_session(id: &str, content_id: &str, events: Vec<PlaybackEvent>) -> ViewerSession {
        ViewerSession {
            session_id: id.to_string(),
            user_id: None,
            content_id: content_id.to_string(),
            started_at_ms: 0,
            events,
        }
    }

    // ── PlaybackMap ──────────────────────────────────────────────────────────

    #[test]
    fn playback_map_mark_range_basic() {
        let mut map = PlaybackMap {
            positions_watched: vec![false; 10],
        };
        map.mark_range(0, 3000);
        assert!(map.positions_watched[0]);
        assert!(map.positions_watched[1]);
        assert!(map.positions_watched[2]);
        assert!(!map.positions_watched[3]);
    }

    #[test]
    fn playback_map_mark_range_clamps_to_capacity() {
        let mut map = PlaybackMap {
            positions_watched: vec![false; 5],
        };
        map.mark_range(0, 100_000);
        assert!(map.positions_watched.iter().all(|&b| b));
    }

    #[test]
    fn playback_map_empty_range_is_noop() {
        let mut map = PlaybackMap {
            positions_watched: vec![false; 5],
        };
        map.mark_range(3000, 3000);
        assert!(map.positions_watched.iter().all(|&b| !b));
    }

    #[test]
    fn playback_map_coverage_full() {
        let map = PlaybackMap {
            positions_watched: vec![true; 10],
        };
        let pct = map.coverage_pct(10_000);
        assert!((pct - 1.0).abs() < 1e-6);
    }

    #[test]
    fn playback_map_coverage_half() {
        let mut positions = vec![false; 10];
        for i in 0..5 {
            positions[i] = true;
        }
        let map = PlaybackMap {
            positions_watched: positions,
        };
        let pct = map.coverage_pct(10_000);
        assert!((pct - 0.5).abs() < 1e-6);
    }

    #[test]
    fn playback_map_coverage_zero_duration() {
        let map = PlaybackMap {
            positions_watched: vec![true; 10],
        };
        assert_eq!(map.coverage_pct(0), 0.0);
    }

    // ── build_playback_map ───────────────────────────────────────────────────

    #[test]
    fn build_map_play_then_end() {
        let session = simple_session(
            "s1",
            "c1",
            vec![
                PlaybackEvent::Play { timestamp_ms: 0 },
                PlaybackEvent::End {
                    position_ms: 5000,
                    watch_duration_ms: 5000,
                },
            ],
        );
        let map = build_playback_map(&session, 10_000);
        // Seconds 0-4 should be watched.
        assert!(map.positions_watched[0]);
        assert!(map.positions_watched[4]);
        assert!(!map.positions_watched[5]);
    }

    #[test]
    fn build_map_play_pause_play_end() {
        let session = simple_session(
            "s2",
            "c1",
            vec![
                PlaybackEvent::Play { timestamp_ms: 0 },
                PlaybackEvent::Pause {
                    timestamp_ms: 1000,
                    position_ms: 3000,
                },
                PlaybackEvent::Play { timestamp_ms: 2000 },
                PlaybackEvent::End {
                    position_ms: 7000,
                    watch_duration_ms: 7000,
                },
            ],
        );
        let map = build_playback_map(&session, 10_000);
        // 0-2 watched, 3-6 watched.
        assert!(map.positions_watched[0]);
        assert!(map.positions_watched[3]);
        assert!(map.positions_watched[6]);
        assert!(!map.positions_watched[7]);
    }

    #[test]
    fn build_map_seek_forward() {
        let session = simple_session(
            "s3",
            "c1",
            vec![
                PlaybackEvent::Play { timestamp_ms: 0 },
                PlaybackEvent::Seek {
                    from_ms: 2000,
                    to_ms: 8000,
                },
                PlaybackEvent::End {
                    position_ms: 10_000,
                    watch_duration_ms: 4000,
                },
            ],
        );
        let map = build_playback_map(&session, 12_000);
        // 0-1 watched, 2-7 NOT (skipped), 8-9 watched.
        assert!(map.positions_watched[0]);
        assert!(!map.positions_watched[5]);
        assert!(map.positions_watched[8]);
    }

    #[test]
    fn build_map_no_events_returns_all_false() {
        let session = simple_session("s4", "c1", vec![]);
        let map = build_playback_map(&session, 5_000);
        assert!(map.positions_watched.iter().all(|&b| !b));
    }

    // ── analyze_session ──────────────────────────────────────────────────────

    #[test]
    fn analyze_session_basic() {
        let session = simple_session(
            "s5",
            "c1",
            vec![
                PlaybackEvent::Play { timestamp_ms: 0 },
                PlaybackEvent::BufferStart { position_ms: 2000 },
                PlaybackEvent::BufferEnd {
                    position_ms: 2000,
                    duration_ms: 500,
                },
                PlaybackEvent::Seek {
                    from_ms: 4000,
                    to_ms: 1000,
                },
                PlaybackEvent::QualityChange {
                    from_height: 720,
                    to_height: 1080,
                    bitrate: 5_000_000,
                },
                PlaybackEvent::End {
                    position_ms: 10_000,
                    watch_duration_ms: 9000,
                },
            ],
        );
        let metrics = analyze_session(&session, 10_000);
        assert_eq!(metrics.buffer_events, 1);
        assert_eq!(metrics.buffer_time_ms, 500);
        assert_eq!(metrics.seek_count, 1);
        assert_eq!(metrics.quality_changes, 1);
        assert_eq!(metrics.total_watch_ms, 9000);
        assert!((metrics.completion_pct - 100.0).abs() < 1e-3);
    }

    #[test]
    fn analyze_session_no_end_event() {
        let session = simple_session(
            "s6",
            "c1",
            vec![
                PlaybackEvent::Play { timestamp_ms: 0 },
                PlaybackEvent::Pause {
                    timestamp_ms: 5000,
                    position_ms: 5000,
                },
            ],
        );
        let metrics = analyze_session(&session, 20_000);
        assert_eq!(metrics.seek_count, 0);
        assert!((metrics.completion_pct - 25.0).abs() < 1e-3);
    }

    #[test]
    fn analyze_session_zero_duration() {
        let session = simple_session(
            "s7",
            "c1",
            vec![PlaybackEvent::End {
                position_ms: 5000,
                watch_duration_ms: 5000,
            }],
        );
        let metrics = analyze_session(&session, 0);
        assert_eq!(metrics.completion_pct, 0.0);
    }

    // ── attention_heatmap ────────────────────────────────────────────────────

    #[test]
    fn heatmap_empty_sessions() {
        let result = attention_heatmap(&[], 60_000, 1000);
        assert!(result.is_empty());
    }

    #[test]
    fn heatmap_single_session_full_watch() {
        let session = simple_session(
            "h1",
            "c1",
            vec![
                PlaybackEvent::Play { timestamp_ms: 0 },
                PlaybackEvent::End {
                    position_ms: 10_000,
                    watch_duration_ms: 10_000,
                },
            ],
        );
        let heat = attention_heatmap(&[session], 10_000, 2000);
        // Peak intensity = 1.0 for all buckets that were watched.
        assert!(!heat.is_empty());
        let peak = heat.iter().map(|h| h.intensity).fold(0.0f32, f32::max);
        assert!((peak - 1.0).abs() < 1e-6);
    }

    #[test]
    fn heatmap_bucket_positions_correct() {
        let session = simple_session(
            "h2",
            "c1",
            vec![
                PlaybackEvent::Play { timestamp_ms: 0 },
                PlaybackEvent::End {
                    position_ms: 5000,
                    watch_duration_ms: 5000,
                },
            ],
        );
        let heat = attention_heatmap(&[session], 10_000, 5000);
        assert_eq!(heat.len(), 2);
        assert_eq!(heat[0].position_ms, 0);
        assert_eq!(heat[1].position_ms, 5000);
    }

    #[test]
    fn heatmap_zero_bucket_ms_returns_empty() {
        let session = simple_session("h3", "c1", vec![]);
        let heat = attention_heatmap(&[session], 10_000, 0);
        assert!(heat.is_empty());
    }

    #[test]
    fn viewer_session_new() {
        let s = ViewerSession::new("id1", Some("u1".to_string()), "vid1", 12345);
        assert_eq!(s.session_id, "id1");
        assert_eq!(s.user_id, Some("u1".to_string()));
        assert!(s.events.is_empty());
    }

    #[test]
    fn viewer_session_push_event() {
        let mut s = ViewerSession::new("id2", None, "vid2", 0);
        s.push_event(PlaybackEvent::Play { timestamp_ms: 100 });
        assert_eq!(s.events.len(), 1);
    }
}
