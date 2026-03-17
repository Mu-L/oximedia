//! # oximedia-analytics
//!
//! Media engagement analytics for the OxiMedia Sovereign Media Framework.
//!
//! This crate provides viewer session tracking, audience retention curve
//! computation, A/B testing with statistical significance analysis (both
//! frequentist and Bayesian), multi-armed bandit adaptive experiments,
//! cohort analysis, funnel analysis, real-time sliding-window aggregation,
//! time-series decomposition, and engagement scoring — all in pure Rust.
//!
//! ## Modules
//!
//! - [`session`] — Playback event modelling, session metrics, playback maps,
//!   attention heatmaps, funnel tracking, and batch session analysis.
//! - [`retention`] — Audience retention curves, segment-level retention,
//!   incremental retention computation, drop-off detection, and benchmarking.
//! - [`ab_testing`] — Deterministic variant assignment (FNV-1a), per-variant
//!   metric collection, two-proportion z-test, Bayesian A/B testing,
//!   and winner selection with configurable significance levels.
//! - [`engagement`] — Weighted engagement scoring, linear-regression trend
//!   analysis, time-series decomposition, and content ranking.
//! - [`bandit`] — Multi-armed bandit (epsilon-greedy and Thompson sampling)
//!   for adaptive media experiments.
//! - [`cohort`] — Cohort analysis: group viewers by first-view date and track
//!   retention over time.
//! - [`realtime`] — Sliding-window real-time aggregation for concurrent
//!   viewers and bitrate statistics.
//! - [`funnel`] — Viewer progression funnel analysis, churn prediction,
//!   and viewer loyalty scoring.
//! - [`quantile`] — Approximate quantile estimation via t-digest.
//! - [`attribution`] — Watch-time attribution across content segments.

pub mod ab_testing;
pub mod attribution;
pub mod bandit;
pub mod cohort;
pub mod engagement;
pub mod error;
pub mod funnel;
pub mod quantile;
pub mod realtime;
pub mod retention;
pub mod session;

// ── Re-exports of key public types ──────────────────────────────────────────

pub use ab_testing::{
    assign_variant, bayesian_winner, winning_variant, winning_variant_with_alpha, AssignmentMethod,
    BayesianAbResult, Experiment, ExperimentResults, OptimisationMetric, Variant, VariantMetrics,
};
pub use bandit::{BanditArm, BanditStrategy, MultiArmedBandit, RegretTracker};
pub use cohort::{
    build_cohort_matrix, Cohort, CohortAnalyzer, CohortDefinition, CohortMatrix,
    CohortRetentionCell, CohortWindow, UserEvent, ViewerEvent,
};
pub use engagement::{
    compute_engagement, decompose_time_series, linear_regression_slope, ContentEngagementScore,
    ContentRanker, DecomposedSeries, EngagementComponents, EngagementTrend, EngagementWeights,
    SeasonalPeriod,
};
pub use error::AnalyticsError;
pub use funnel::{
    compute_funnel, compute_loyalty, predict_churn, ChurnAssessment, ChurnConfig, ChurnRisk,
    FunnelAnalyzer, FunnelDefinition, FunnelMilestone, FunnelReport, FunnelResult, FunnelStep,
    FunnelStepDef, LoyaltyComponents, LoyaltyScore, LoyaltyWeights, SessionEvent,
};
pub use quantile::{percentiles, TDigest};
pub use realtime::{BucketMetrics, RealtimeEvent, SlidingWindowAggregator};
pub use retention::{
    average_view_duration, compare_to_benchmark, compute_retention, compute_retention_incremental,
    compute_segment_retention, drop_off_points, re_watch_segments, ContentSegment,
    IncrementalRetentionState, RetentionBenchmark, RetentionBucket, RetentionCurve,
    SegmentRetentionResult,
};
pub use session::{
    analyze_session, analyze_sessions_batch, attention_heatmap, build_playback_map, HeatPoint,
    PlaybackEvent, PlaybackMap, SessionMetrics, ViewerSession,
};
