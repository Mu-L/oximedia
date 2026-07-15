//! `oximedia.analytics` submodule — Python bindings for `oximedia-analytics`.
//!
//! Wraps viewer-session playback tracking, session-metric analysis,
//! engagement scoring, and attention-heatmap generation behind PyO3 classes
//! with real delegation to [`oximedia_analytics`]. Unlike the WASM
//! `SessionTracker`/heatmap bindings — which reimplement their own ad-hoc
//! watch-time bookkeeping because they predate this crate's stabilised
//! session model — this binding drives the actual `oximedia_analytics`
//! session/engagement algorithms, so results match what a server-side batch
//! analytics job would compute for the same events.

use oximedia_analytics::{
    analyze_session as core_analyze_session, attention_heatmap as core_attention_heatmap,
    compute_engagement as core_compute_engagement, EngagementWeights, PlaybackEvent,
    SessionMetrics, ViewerSession,
};
use pyo3::prelude::*;

// ---------------------------------------------------------------------------
// ViewerSession
// ---------------------------------------------------------------------------

/// A single viewer's playback session — records timestamped playback events
/// (play / pause / seek / buffer / quality-change / end) for later analysis.
///
/// Real delegation to [`oximedia_analytics::ViewerSession`] /
/// [`oximedia_analytics::PlaybackEvent`].
#[pyclass(name = "ViewerSession")]
#[derive(Clone)]
pub struct PyViewerSession {
    inner: ViewerSession,
}

#[pymethods]
impl PyViewerSession {
    /// Create a new empty session.
    ///
    /// Args:
    ///     session_id: Unique identifier for this viewing session.
    ///     content_id: Identifier of the content being watched.
    ///     started_at_ms: Wall-clock session start time (Unix epoch ms).
    ///     user_id: Optional viewer identifier.
    #[new]
    #[pyo3(signature = (session_id, content_id, started_at_ms, user_id=None))]
    fn new(
        session_id: String,
        content_id: String,
        started_at_ms: i64,
        user_id: Option<String>,
    ) -> Self {
        Self {
            inner: ViewerSession::new(session_id, user_id, content_id, started_at_ms),
        }
    }

    /// Record a play event at `timestamp_ms` (wall-clock).
    fn track_play(&mut self, timestamp_ms: i64) {
        self.inner.push_event(PlaybackEvent::Play { timestamp_ms });
    }

    /// Record a pause event at `timestamp_ms` (wall-clock), content position
    /// `position_ms`.
    fn track_pause(&mut self, timestamp_ms: i64, position_ms: u64) {
        self.inner.push_event(PlaybackEvent::Pause {
            timestamp_ms,
            position_ms,
        });
    }

    /// Record a scrub from `from_ms` to `to_ms` (content positions).
    fn track_seek(&mut self, from_ms: u64, to_ms: u64) {
        self.inner
            .push_event(PlaybackEvent::Seek { from_ms, to_ms });
    }

    /// Record the start of a buffering stall at content position `position_ms`.
    fn track_buffer_start(&mut self, position_ms: u64) {
        self.inner
            .push_event(PlaybackEvent::BufferStart { position_ms });
    }

    /// Record the end of a buffering stall at `position_ms` that lasted
    /// `duration_ms`.
    fn track_buffer_end(&mut self, position_ms: u64, duration_ms: u32) {
        self.inner.push_event(PlaybackEvent::BufferEnd {
            position_ms,
            duration_ms,
        });
    }

    /// Record an adaptive-bitrate quality-level switch.
    fn track_quality_change(&mut self, from_height: u32, to_height: u32, bitrate: u32) {
        self.inner.push_event(PlaybackEvent::QualityChange {
            from_height,
            to_height,
            bitrate,
        });
    }

    /// Record the end of the session at `position_ms`, having watched
    /// `watch_duration_ms` in total.
    fn track_end(&mut self, position_ms: u64, watch_duration_ms: u64) {
        self.inner.push_event(PlaybackEvent::End {
            position_ms,
            watch_duration_ms,
        });
    }

    /// Number of events recorded so far.
    fn event_count(&self) -> usize {
        self.inner.events.len()
    }

    #[getter]
    fn session_id(&self) -> String {
        self.inner.session_id.clone()
    }

    #[getter]
    fn content_id(&self) -> String {
        self.inner.content_id.clone()
    }

    #[getter]
    fn user_id(&self) -> Option<String> {
        self.inner.user_id.clone()
    }

    /// Analyse this session and return aggregate [`SessionMetrics`].
    ///
    /// `content_duration_ms` is used to compute `completion_pct` and the
    /// unique-position count; pass `0` if unknown.
    fn analyze(&self, content_duration_ms: u64) -> PySessionMetrics {
        core_analyze_session(&self.inner, content_duration_ms).into()
    }

    fn __repr__(&self) -> String {
        format!(
            "ViewerSession(session_id={:?}, content_id={:?}, events={})",
            self.inner.session_id,
            self.inner.content_id,
            self.inner.events.len()
        )
    }
}

// ---------------------------------------------------------------------------
// SessionMetrics
// ---------------------------------------------------------------------------

/// Aggregate metrics derived from a single [`PyViewerSession::analyze`] call.
#[pyclass(name = "SessionMetrics")]
pub struct PySessionMetrics {
    /// Total milliseconds of content actually watched.
    #[pyo3(get)]
    pub total_watch_ms: u64,
    /// Number of unique 1-second positions watched.
    #[pyo3(get)]
    pub unique_positions_watched: u64,
    /// How many `Seek` events were recorded.
    #[pyo3(get)]
    pub seek_count: u32,
    /// How many buffering interruptions occurred.
    #[pyo3(get)]
    pub buffer_events: u32,
    /// Total stall time in milliseconds.
    #[pyo3(get)]
    pub buffer_time_ms: u64,
    /// How many quality-level switches happened.
    #[pyo3(get)]
    pub quality_changes: u32,
    /// Fraction of the content completed, `0.0-100.0`.
    #[pyo3(get)]
    pub completion_pct: f32,
}

impl From<SessionMetrics> for PySessionMetrics {
    fn from(m: SessionMetrics) -> Self {
        Self {
            total_watch_ms: m.total_watch_ms,
            unique_positions_watched: m.unique_positions_watched,
            seek_count: m.seek_count,
            buffer_events: m.buffer_events,
            buffer_time_ms: m.buffer_time_ms,
            quality_changes: m.quality_changes,
            completion_pct: m.completion_pct,
        }
    }
}

#[pymethods]
impl PySessionMetrics {
    fn __repr__(&self) -> String {
        format!(
            "SessionMetrics(watch_ms={}, completion_pct={:.1}, seeks={}, buffer_events={})",
            self.total_watch_ms, self.completion_pct, self.seek_count, self.buffer_events
        )
    }
}

// ---------------------------------------------------------------------------
// ContentEngagementScore
// ---------------------------------------------------------------------------

/// A per-content engagement score in `0.0-1.0`, decomposed into its
/// component signals. See [`compute_engagement`].
#[pyclass(name = "ContentEngagementScore")]
pub struct PyContentEngagementScore {
    /// The content ID the score was computed for (from the first session).
    #[pyo3(get)]
    pub content_id: String,
    /// Overall weighted engagement score, `0.0-1.0`.
    #[pyo3(get)]
    pub score: f32,
    /// Ratio of average watch time to content duration (capped at 1.0).
    #[pyo3(get)]
    pub watch_time_score: f32,
    /// Fraction of sessions that reached >= 95% completion.
    #[pyo3(get)]
    pub completion_score: f32,
    /// Fraction of sessions that rewatched any segment.
    #[pyo3(get)]
    pub rewatch_score: f32,
    /// Normalised social-interaction score; honestly `0.0` when no social
    /// data is available (this binding does not fabricate a placeholder).
    #[pyo3(get)]
    pub social_score: f32,
    /// Penalty term proportional to the forward-seek rate (lower is better).
    #[pyo3(get)]
    pub seek_forward_penalty: f32,
}

#[pymethods]
impl PyContentEngagementScore {
    fn __repr__(&self) -> String {
        format!(
            "ContentEngagementScore(content_id={:?}, score={:.3})",
            self.content_id, self.score
        )
    }
}

// ---------------------------------------------------------------------------
// Standalone functions
// ---------------------------------------------------------------------------

/// Analyse a [`PyViewerSession`] and return its [`PySessionMetrics`].
///
/// Module-level convenience equivalent to `session.analyze(content_duration_ms)`.
#[pyfunction]
pub fn analyze_session(session: &PyViewerSession, content_duration_ms: u64) -> PySessionMetrics {
    session.analyze(content_duration_ms)
}

/// Compute an attention heatmap across multiple sessions, bucketed by
/// `bucket_ms`.
///
/// Returns a list of `(position_ms, intensity)` pairs; `intensity` is
/// normalised so the peak bucket is `1.0`. Returns an empty list if
/// `sessions` is empty or `content_duration_ms`/`bucket_ms` is `0`.
#[pyfunction]
pub fn attention_heatmap(
    sessions: Vec<PyRef<'_, PyViewerSession>>,
    content_duration_ms: u64,
    bucket_ms: u32,
) -> Vec<(u64, f32)> {
    let inner_sessions: Vec<ViewerSession> = sessions.iter().map(|s| s.inner.clone()).collect();
    core_attention_heatmap(&inner_sessions, content_duration_ms, bucket_ms)
        .into_iter()
        .map(|hp| (hp.position_ms, hp.intensity))
        .collect()
}

/// Compute an overall engagement score for a content item from its viewer
/// sessions, using equally-weighted default components (watch time,
/// completion, rewatch, social, forward-seek penalty).
///
/// `ViewerSession`/`PlaybackEvent` carry no social-interaction data, so the
/// social channel's weight is honestly redistributed across the measurable
/// channels (real crate behaviour) rather than fabricated. Use
/// `oximedia_analytics::engagement::compute_engagement_with_social` from
/// Rust for the social-aware variant (not yet bound — see module TODOs).
#[pyfunction]
pub fn compute_engagement(
    sessions: Vec<PyRef<'_, PyViewerSession>>,
    content_duration_ms: u64,
) -> PyContentEngagementScore {
    let inner_sessions: Vec<ViewerSession> = sessions.iter().map(|s| s.inner.clone()).collect();
    let weights = EngagementWeights::default();
    let score = core_compute_engagement(&inner_sessions, content_duration_ms, &weights);
    PyContentEngagementScore {
        content_id: score.content_id,
        score: score.score,
        watch_time_score: score.components.watch_time_score,
        completion_score: score.components.completion_score,
        rewatch_score: score.components.rewatch_score,
        social_score: score.components.social_score,
        seek_forward_penalty: score.components.seek_forward_penalty,
    }
}

// TODO(0.2.x): expose oximedia_analytics::ab_testing (Experiment, assign_variant,
// bayesian_winner) for A/B test allocation and analysis.
// TODO(0.2.x): expose oximedia_analytics::bandit (MultiArmedBandit, RegretTracker).
// TODO(0.2.x): expose oximedia_analytics::cohort (CohortAnalyzer, build_cohort_matrix).
// TODO(0.2.x): expose oximedia_analytics::funnel (FunnelAnalyzer, predict_churn,
// compute_loyalty).
// TODO(0.2.x): expose oximedia_analytics::retention (compute_retention,
// drop_off_points, compare_to_benchmark, re_watch_segments).
// TODO(0.2.x): expose oximedia_analytics::geo_device (BreakdownAnalyzer,
// SliceComparison).
// TODO(0.2.x): expose oximedia_analytics::quantile (TDigest, percentiles) and
// oximedia_analytics::percentile / uniformity.
// TODO(0.2.x): expose oximedia_analytics::realtime (SlidingWindowAggregator).
// TODO(0.2.x): expose oximedia_analytics::replay / anomaly / attribution /
// fingerprint / heatmap (grid-based `Heatmap`) / multivariate /
// weighted_retention / segment_retention.
// TODO(0.2.x): expose `compute_engagement_with_social` (explicit SocialSignals +
// custom EngagementWeights) and `reservoir_sampled_heatmap` (memory-bounded
// sampling for very large session sets).

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the `oximedia.analytics` submodule.
pub fn register_submodule(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "analytics")?;

    m.add_class::<PyViewerSession>()?;
    m.add_class::<PySessionMetrics>()?;
    m.add_class::<PyContentEngagementScore>()?;
    m.add_function(wrap_pyfunction!(analyze_session, &m)?)?;
    m.add_function(wrap_pyfunction!(attention_heatmap, &m)?)?;
    m.add_function(wrap_pyfunction!(compute_engagement, &m)?)?;

    parent.add_submodule(&m)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn full_watch_session(id: &str) -> PyViewerSession {
        let mut s = PyViewerSession::new(id.to_string(), "content-1".to_string(), 0, None);
        s.track_play(0);
        s.track_end(10_000, 10_000);
        s
    }

    #[test]
    fn viewer_session_new_has_no_events() {
        let s = PyViewerSession::new(
            "s1".to_string(),
            "c1".to_string(),
            0,
            Some("u1".to_string()),
        );
        assert_eq!(s.event_count(), 0);
        assert_eq!(s.session_id(), "s1");
        assert_eq!(s.content_id(), "c1");
        assert_eq!(s.user_id(), Some("u1".to_string()));
    }

    #[test]
    fn track_events_increments_count() {
        let mut s = PyViewerSession::new("s2".to_string(), "c1".to_string(), 0, None);
        s.track_play(0);
        s.track_pause(1000, 5000);
        s.track_seek(5000, 8000);
        s.track_buffer_start(8000);
        s.track_buffer_end(8000, 500);
        s.track_quality_change(720, 1080, 5_000_000);
        s.track_end(10_000, 9500);
        assert_eq!(s.event_count(), 7);
    }

    #[test]
    fn analyze_full_watch_reports_full_completion() {
        let s = full_watch_session("s3");
        let metrics = s.analyze(10_000);
        assert!((metrics.completion_pct - 100.0).abs() < 1e-3);
        assert_eq!(metrics.total_watch_ms, 10_000);
    }

    #[test]
    fn analyze_no_events_is_zero_metrics() {
        let s = PyViewerSession::new("s4".to_string(), "c1".to_string(), 0, None);
        let metrics = s.analyze(10_000);
        assert_eq!(metrics.total_watch_ms, 0);
        assert_eq!(metrics.seek_count, 0);
    }

    #[test]
    fn module_level_analyze_session_matches_method() {
        let s = full_watch_session("s5");
        let via_method = s.analyze(10_000);
        let via_function = analyze_session(&s, 10_000);
        assert_eq!(via_method.total_watch_ms, via_function.total_watch_ms);
        assert_eq!(via_method.completion_pct, via_function.completion_pct);
        assert_eq!(via_method.seek_count, via_function.seek_count);
    }

    #[test]
    fn session_repr_contains_ids() {
        let s = full_watch_session("s6");
        let repr = s.__repr__();
        assert!(repr.contains("s6"));
        assert!(repr.contains("content-1"));
    }

    // `attention_heatmap` and `compute_engagement` take `Vec<PyRef<PyViewerSession>>`,
    // which requires a live Python object (GIL) to construct a `PyRef` from — those
    // are covered end-to-end via the embedded interpreter in
    // `tests/analytics_smoke.rs` instead of here.
}
