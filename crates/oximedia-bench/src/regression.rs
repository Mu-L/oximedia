//! Benchmark regression tracking system for `OxiMedia`.
//!
//! This module provides tools for detecting performance regressions across benchmark runs,
//! storing historical results, computing statistical baselines, and performing trend analysis.
//!
//! # Overview
//!
//! - [`BenchmarkRecord`]: A single benchmark result snapshot
//! - [`BenchmarkHistory`]: Persistent store of all records with baseline computation
//! - [`RegressionDetector`]: Statistically-sound regression detection via z-score
//! - [`TrendAnalysis`]: Linear regression over time to detect slow degradation
//!
//! # Example
//!
//! ```
//! use oximedia_bench::regression::{
//!     BenchmarkRecord, BenchmarkHistory, RegressionDetector, RegressionKind,
//! };
//!
//! // Build some historical data
//! let mut history = BenchmarkHistory::new(100);
//! for i in 0..10u64 {
//!     history.add(BenchmarkRecord {
//!         name: "encode_av1".to_string(),
//!         timestamp: 1_700_000_000 + i * 3600,
//!         throughput_fps: 30.0 + (i as f64) * 0.1,
//!         latency_ms: 33.0,
//!         memory_bytes: 512_000_000,
//!         quality_score: 38.5,
//!         metadata: std::collections::HashMap::new(),
//!     });
//! }
//!
//! // Check current result for regression
//! let current = BenchmarkRecord {
//!     name: "encode_av1".to_string(),
//!     timestamp: 1_700_040_000,
//!     throughput_fps: 20.0,  // big drop!
//!     latency_ms: 55.0,
//!     memory_bytes: 512_000_000,
//!     quality_score: 38.4,
//!     metadata: std::collections::HashMap::new(),
//! };
//!
//! let detector = RegressionDetector::default();
//! // Pass the full history slice — detect filters by name internally.
//! let analysis = detector.detect(&current, &history.records);
//! println!("Regression kind: {:?}", analysis.fps_kind);
//! ```

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Core data types
// ─────────────────────────────────────────────────────────────────────────────

/// A single stored benchmark result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRecord {
    /// Benchmark identifier (e.g. `"encode_av1_1080p"`).
    pub name: String,

    /// Unix timestamp (seconds since epoch) of when this run was recorded.
    pub timestamp: u64,

    /// Processing throughput in frames per second.
    pub throughput_fps: f64,

    /// Per-frame latency in milliseconds.
    pub latency_ms: f64,

    /// Peak memory usage in bytes during the run.
    pub memory_bytes: u64,

    /// Quality score (e.g. PSNR in dB, or VMAF 0–100).
    pub quality_score: f64,

    /// Arbitrary key-value annotations (codec version, host, git SHA, …).
    pub metadata: HashMap<String, String>,
}

impl BenchmarkRecord {
    /// Construct a minimal record for quick testing.
    #[must_use]
    pub fn simple(name: impl Into<String>, timestamp: u64, fps: f64, latency_ms: f64) -> Self {
        Self {
            name: name.into(),
            timestamp,
            throughput_fps: fps,
            latency_ms,
            memory_bytes: 0,
            quality_score: 0.0,
            metadata: HashMap::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Severity / kind enums
// ─────────────────────────────────────────────────────────────────────────────

/// Regression severity classification.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Severity {
    /// 5–15 % degradation — worth watching.
    Minor,
    /// 15–30 % degradation — should be investigated.
    Moderate,
    /// 30–50 % degradation — likely a bug or configuration issue.
    Major,
    /// > 50 % degradation — critical production risk.
    Critical,
}

impl Severity {
    /// Classify a percent change into a severity level.
    ///
    /// `percent` is a **positive** value representing how much performance
    /// dropped compared to the baseline (e.g. `20.0` → 20 % worse).
    #[must_use]
    pub fn from_percent(percent: f64) -> Self {
        match percent {
            p if p >= 50.0 => Severity::Critical,
            p if p >= 30.0 => Severity::Major,
            p if p >= 15.0 => Severity::Moderate,
            _ => Severity::Minor,
        }
    }

    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Severity::Minor => "minor",
            Severity::Moderate => "moderate",
            Severity::Major => "major",
            Severity::Critical => "critical",
        }
    }
}

/// The outcome of comparing a single metric to its historical baseline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RegressionKind {
    /// Performance improved by `percent` %.
    Improvement {
        /// Percentage improvement (positive value).
        percent: f64,
    },
    /// Performance degraded beyond the noise threshold.
    Regression {
        /// Percentage degradation (positive value).
        percent: f64,
        /// How severe this regression is.
        severity: Severity,
    },
    /// Change is within the statistical noise threshold — no action needed.
    Stable,
}

impl RegressionKind {
    /// Return `true` if this is a regression of any severity.
    #[must_use]
    pub fn is_regression(&self) -> bool {
        matches!(self, RegressionKind::Regression { .. })
    }

    /// Return `true` if this is an improvement.
    #[must_use]
    pub fn is_improvement(&self) -> bool {
        matches!(self, RegressionKind::Improvement { .. })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Baseline
// ─────────────────────────────────────────────────────────────────────────────

/// Statistical baseline computed from recent historical records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkBaseline {
    /// Mean throughput in FPS across the baseline window.
    pub mean_fps: f64,
    /// Sample standard deviation of throughput.
    pub std_fps: f64,
    /// Mean latency in milliseconds.
    pub mean_latency_ms: f64,
    /// Sample standard deviation of latency.
    pub std_latency_ms: f64,
    /// Mean quality score.
    pub mean_quality: f64,
    /// Sample standard deviation of quality score.
    pub std_quality: f64,
    /// Number of samples used.
    pub sample_count: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Trend analysis
// ─────────────────────────────────────────────────────────────────────────────

/// Trend analysis computed via ordinary-least-squares linear regression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    /// Change in FPS per sequential run (slope of the regression line).
    pub slope_fps_per_run: f64,
    /// Whether throughput is trending downward over recent runs.
    pub is_trending_down: bool,
    /// Projected FPS for the *next* run based on the current trend.
    pub projected_fps_next: f64,
    /// R² coefficient of determination (0–1, higher = better linear fit).
    pub r_squared: f64,
    /// Number of data points used in the regression.
    pub sample_count: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Full analysis result
// ─────────────────────────────────────────────────────────────────────────────

/// Complete regression analysis for a single benchmark run compared to its baseline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionAnalysis {
    /// Benchmark name.
    pub name: String,
    /// Regression status for throughput (FPS).
    pub fps_kind: RegressionKind,
    /// Regression status for latency (lower is better, so inversion is applied).
    pub latency_kind: RegressionKind,
    /// Regression status for quality score (higher is better).
    pub quality_kind: RegressionKind,
    /// Z-score of the current FPS relative to the baseline distribution.
    pub fps_z_score: f64,
    /// Z-score of the current latency relative to the baseline distribution.
    pub latency_z_score: f64,
    /// Baseline statistics used for this analysis.
    pub baseline: BenchmarkBaseline,
    /// Whether at least one metric shows a statistically significant regression.
    pub has_regression: bool,
}

impl RegressionAnalysis {
    /// Return the worst severity across all regressing metrics, if any.
    #[must_use]
    pub fn worst_severity(&self) -> Option<Severity> {
        let kinds = [&self.fps_kind, &self.latency_kind, &self.quality_kind];
        let mut worst: Option<Severity> = None;
        for kind in &kinds {
            if let RegressionKind::Regression { severity, .. } = kind {
                worst = Some(match worst {
                    None => *severity,
                    Some(current) => Self::worse_severity(current, *severity),
                });
            }
        }
        worst
    }

    fn worse_severity(a: Severity, b: Severity) -> Severity {
        let rank = |s: Severity| match s {
            Severity::Minor => 0,
            Severity::Moderate => 1,
            Severity::Major => 2,
            Severity::Critical => 3,
        };
        if rank(a) >= rank(b) { a } else { b }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Regression detector
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration knobs for [`RegressionDetector`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorConfig {
    /// Z-score threshold above which a change is flagged as a regression.
    /// The default value (2.0) corresponds to roughly 95 % confidence.
    pub z_score_threshold: f64,

    /// Minimum percentage change required before triggering a regression,
    /// acting as an absolute noise floor below the z-score test.
    pub min_regression_percent: f64,

    /// Confidence level for the reported confidence intervals (0.90 / 0.95 / 0.99).
    pub confidence_level: f64,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            z_score_threshold: 2.0,
            min_regression_percent: 5.0,
            confidence_level: 0.95,
        }
    }
}

/// Detects performance regressions by comparing a current result to a historical baseline.
///
/// Uses z-score analysis to filter out noise: a change is only flagged if
/// `|z| > threshold` **and** the absolute percentage change exceeds
/// `min_regression_percent`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegressionDetector {
    /// Configuration for detection thresholds.
    pub config: DetectorConfig,
}

impl RegressionDetector {
    /// Create a detector with custom configuration.
    #[must_use]
    pub fn with_config(config: DetectorConfig) -> Self {
        Self { config }
    }

    /// Compare `current` against the distribution described by `history`.
    ///
    /// Returns a [`RegressionAnalysis`] with per-metric regression kinds and
    /// the baseline statistics used for the comparison.
    #[must_use]
    pub fn detect(&self, current: &BenchmarkRecord, history: &[BenchmarkRecord]) -> RegressionAnalysis {
        // Build baseline from all historical records for this benchmark name.
        let relevant: Vec<&BenchmarkRecord> = history
            .iter()
            .filter(|r| r.name == current.name)
            .collect();

        if relevant.is_empty() {
            // No history — treat as stable (first run).
            let baseline = BenchmarkBaseline {
                mean_fps: current.throughput_fps,
                std_fps: 0.0,
                mean_latency_ms: current.latency_ms,
                std_latency_ms: 0.0,
                mean_quality: current.quality_score,
                std_quality: 0.0,
                sample_count: 0,
            };
            return RegressionAnalysis {
                name: current.name.clone(),
                fps_kind: RegressionKind::Stable,
                latency_kind: RegressionKind::Stable,
                quality_kind: RegressionKind::Stable,
                fps_z_score: 0.0,
                latency_z_score: 0.0,
                baseline,
                has_regression: false,
            };
        }

        let fps_values: Vec<f64> = relevant.iter().map(|r| r.throughput_fps).collect();
        let lat_values: Vec<f64> = relevant.iter().map(|r| r.latency_ms).collect();
        let qual_values: Vec<f64> = relevant.iter().map(|r| r.quality_score).collect();

        let mean_fps = mean(&fps_values);
        let std_fps = sample_std_dev(&fps_values);
        let mean_lat = mean(&lat_values);
        let std_lat = sample_std_dev(&lat_values);
        let mean_qual = mean(&qual_values);
        let std_qual = sample_std_dev(&qual_values);

        let baseline = BenchmarkBaseline {
            mean_fps,
            std_fps,
            mean_latency_ms: mean_lat,
            std_latency_ms: std_lat,
            mean_quality: mean_qual,
            std_quality: std_qual,
            sample_count: relevant.len(),
        };

        // FPS: higher is better — regression when current < mean.
        let fps_z = z_score(current.throughput_fps, mean_fps, std_fps);
        let fps_kind = self.classify_fps_regression(current.throughput_fps, mean_fps, fps_z);

        // Latency: lower is better — regression when current > mean.
        let lat_z = z_score(current.latency_ms, mean_lat, std_lat);
        let latency_kind = self.classify_latency_regression(current.latency_ms, mean_lat, lat_z);

        // Quality score: higher is better.
        let qual_z = z_score(current.quality_score, mean_qual, std_qual);
        let quality_kind = self.classify_fps_regression(current.quality_score, mean_qual, qual_z);

        let has_regression = fps_kind.is_regression()
            || latency_kind.is_regression()
            || quality_kind.is_regression();

        RegressionAnalysis {
            name: current.name.clone(),
            fps_kind,
            latency_kind,
            quality_kind,
            fps_z_score: fps_z,
            latency_z_score: lat_z,
            baseline,
            has_regression,
        }
    }

    /// Classify a "higher-is-better" metric (FPS, quality score).
    ///
    /// When the baseline has zero variance (all samples identical) a change
    /// beyond `min_regression_percent` is flagged deterministically, since
    /// there is no noise to filter out.
    fn classify_fps_regression(&self, current: f64, baseline_mean: f64, z: f64) -> RegressionKind {
        if baseline_mean.abs() < f64::EPSILON {
            return RegressionKind::Stable;
        }
        let percent_change = (baseline_mean - current) / baseline_mean * 100.0;
        let zero_std = z == 0.0 && (current - baseline_mean).abs() > f64::EPSILON;

        let is_regression = percent_change > self.config.min_regression_percent
            && (zero_std || z < -self.config.z_score_threshold);
        let is_improvement = percent_change < -self.config.min_regression_percent
            && (zero_std || z > self.config.z_score_threshold);

        if is_regression {
            RegressionKind::Regression {
                percent: percent_change,
                severity: Severity::from_percent(percent_change),
            }
        } else if is_improvement {
            RegressionKind::Improvement {
                percent: percent_change.abs(),
            }
        } else {
            RegressionKind::Stable
        }
    }

    /// Classify a "lower-is-better" metric (latency).
    ///
    /// When the baseline has zero variance (all samples identical) a change
    /// beyond `min_regression_percent` is flagged deterministically.
    fn classify_latency_regression(&self, current: f64, baseline_mean: f64, z: f64) -> RegressionKind {
        if baseline_mean.abs() < f64::EPSILON {
            return RegressionKind::Stable;
        }
        // For latency, an *increase* is a regression.
        let percent_change = (current - baseline_mean) / baseline_mean * 100.0;
        let zero_std = z == 0.0 && (current - baseline_mean).abs() > f64::EPSILON;

        let is_regression = percent_change > self.config.min_regression_percent
            && (zero_std || z > self.config.z_score_threshold);
        let is_improvement = percent_change < -self.config.min_regression_percent
            && (zero_std || z < -self.config.z_score_threshold);

        if is_regression {
            RegressionKind::Regression {
                percent: percent_change,
                severity: Severity::from_percent(percent_change),
            }
        } else if is_improvement {
            RegressionKind::Improvement {
                percent: percent_change.abs(),
            }
        } else {
            RegressionKind::Stable
        }
    }

    /// Compute a two-sided confidence interval for `samples`.
    ///
    /// `confidence` should be one of `0.90`, `0.95`, or `0.99`.
    /// The interval is `(mean - margin, mean + margin)` using a normal
    /// approximation (appropriate for n ≥ 30; reasonable for smaller samples).
    #[must_use]
    pub fn confidence_interval(samples: &[f64], confidence: f64) -> (f64, f64) {
        if samples.is_empty() {
            return (0.0, 0.0);
        }
        if samples.len() == 1 {
            return (samples[0], samples[0]);
        }

        let m = mean(samples);
        let s = sample_std_dev(samples);
        let n = samples.len() as f64;

        let z = z_critical(confidence);
        let margin = z * s / n.sqrt();
        (m - margin, m + margin)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// History store
// ─────────────────────────────────────────────────────────────────────────────

/// Persistent in-memory store of benchmark records with rolling-window eviction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkHistory {
    /// All stored records (sorted by insertion order / timestamp).
    pub records: Vec<BenchmarkRecord>,
    /// Maximum number of records to retain (oldest are dropped first).
    pub max_records: usize,
}

impl BenchmarkHistory {
    /// Create a new history store that retains at most `max_records` entries.
    #[must_use]
    pub fn new(max_records: usize) -> Self {
        Self {
            records: Vec::new(),
            max_records: max_records.max(1),
        }
    }

    /// Add a new record, evicting the oldest if the capacity is exceeded.
    pub fn add(&mut self, record: BenchmarkRecord) {
        self.records.push(record);
        if self.records.len() > self.max_records {
            let excess = self.records.len() - self.max_records;
            self.records.drain(0..excess);
        }
    }

    /// Iterate over all records that match the given benchmark name.
    pub fn records_for<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a BenchmarkRecord> {
        self.records.iter().filter(move |r| r.name == name)
    }

    /// Compute a [`BenchmarkBaseline`] from the most recent `window` records for `name`.
    ///
    /// Returns `None` if there are no records for `name`.
    #[must_use]
    pub fn baseline(&self, name: &str, window: usize) -> Option<BenchmarkBaseline> {
        let mut matching: Vec<&BenchmarkRecord> = self.records_for(name).collect();
        if matching.is_empty() {
            return None;
        }

        // Use only the most recent `window` records.
        let take = window.min(matching.len());
        let start = matching.len() - take;
        matching = matching[start..].to_vec();

        let fps_vals: Vec<f64> = matching.iter().map(|r| r.throughput_fps).collect();
        let lat_vals: Vec<f64> = matching.iter().map(|r| r.latency_ms).collect();
        let qual_vals: Vec<f64> = matching.iter().map(|r| r.quality_score).collect();

        Some(BenchmarkBaseline {
            mean_fps: mean(&fps_vals),
            std_fps: sample_std_dev(&fps_vals),
            mean_latency_ms: mean(&lat_vals),
            std_latency_ms: sample_std_dev(&lat_vals),
            mean_quality: mean(&qual_vals),
            std_quality: sample_std_dev(&qual_vals),
            sample_count: matching.len(),
        })
    }

    /// Compute a [`TrendAnalysis`] for `name` using all available records.
    ///
    /// Returns `None` if fewer than two records exist (cannot fit a line).
    #[must_use]
    pub fn trend(&self, name: &str) -> Option<TrendAnalysis> {
        let matching: Vec<&BenchmarkRecord> = self.records_for(name).collect();
        if matching.len() < 2 {
            return None;
        }

        // Use sequential run index (0, 1, 2, …) as the x-axis so that
        // unevenly-spaced timestamps don't distort the slope.
        let xs: Vec<f64> = (0..matching.len()).map(|i| i as f64).collect();
        let ys: Vec<f64> = matching.iter().map(|r| r.throughput_fps).collect();

        let (slope, intercept, r_squared) = linear_regression(&xs, &ys);

        let n = matching.len() as f64;
        let projected = slope * n + intercept;

        Some(TrendAnalysis {
            slope_fps_per_run: slope,
            is_trending_down: slope < 0.0,
            projected_fps_next: projected,
            r_squared,
            sample_count: matching.len(),
        })
    }

    /// Serialize the history to a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error string if serialization fails.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
    }

    /// Deserialize a [`BenchmarkHistory`] from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error string if deserialization fails.
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    /// Return the total number of stored records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Return `true` if the history contains no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Return all distinct benchmark names present in the history.
    #[must_use]
    pub fn benchmark_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .records
            .iter()
            .map(|r| r.name.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        names.sort();
        names
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal statistical helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Arithmetic mean of `values`. Returns `0.0` for an empty slice.
fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Sample standard deviation (divides by n-1). Returns `0.0` for ≤1 element.
fn sample_std_dev(values: &[f64]) -> f64 {
    if values.len() <= 1 {
        return 0.0;
    }
    let m = mean(values);
    let variance = values.iter().map(|v| (v - m).powi(2)).sum::<f64>()
        / (values.len() - 1) as f64;
    variance.sqrt()
}

/// Z-score: `(value - mean) / std_dev`.
/// Returns `0.0` if `std_dev` is effectively zero (avoids division by zero).
fn z_score(value: f64, mean_val: f64, std_dev: f64) -> f64 {
    if std_dev < f64::EPSILON {
        return 0.0;
    }
    (value - mean_val) / std_dev
}

/// Critical z-value for two-sided normal confidence intervals.
fn z_critical(confidence: f64) -> f64 {
    match confidence {
        c if (c - 0.99).abs() < 0.001 => 2.576,
        c if (c - 0.95).abs() < 0.001 => 1.960,
        c if (c - 0.90).abs() < 0.001 => 1.645,
        _ => 1.960, // default to 95 %
    }
}

/// Ordinary least-squares linear regression: returns `(slope, intercept, r²)`.
fn linear_regression(xs: &[f64], ys: &[f64]) -> (f64, f64, f64) {
    let n = xs.len() as f64;
    if n < 2.0 {
        return (0.0, ys.first().copied().unwrap_or(0.0), 0.0);
    }

    let mean_x = mean(xs);
    let mean_y = mean(ys);

    let ss_xx: f64 = xs.iter().map(|x| (x - mean_x).powi(2)).sum();
    let ss_xy: f64 = xs
        .iter()
        .zip(ys.iter())
        .map(|(x, y)| (x - mean_x) * (y - mean_y))
        .sum();
    let ss_yy: f64 = ys.iter().map(|y| (y - mean_y).powi(2)).sum();

    if ss_xx.abs() < f64::EPSILON {
        return (0.0, mean_y, 0.0);
    }

    let slope = ss_xy / ss_xx;
    let intercept = mean_y - slope * mean_x;

    // R² = (SS_xy)² / (SS_xx * SS_yy)
    let r_squared = if ss_yy.abs() < f64::EPSILON {
        1.0 // all y values identical — perfect horizontal fit
    } else {
        (ss_xy * ss_xy) / (ss_xx * ss_yy)
    };

    (slope, intercept, r_squared.clamp(0.0, 1.0))
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn make_record(name: &str, ts: u64, fps: f64, latency: f64, quality: f64) -> BenchmarkRecord {
        BenchmarkRecord {
            name: name.to_string(),
            timestamp: ts,
            throughput_fps: fps,
            latency_ms: latency,
            memory_bytes: 512_000_000,
            quality_score: quality,
            metadata: HashMap::new(),
        }
    }

    fn stable_history(n: usize) -> Vec<BenchmarkRecord> {
        (0..n)
            .map(|i| make_record("bench_a", 1_700_000_000 + i as u64 * 3600, 30.0, 33.0, 38.5))
            .collect()
    }

    // ── mean / std_dev ───────────────────────────────────────────────────────

    #[test]
    fn test_mean_basic() {
        assert_eq!(mean(&[1.0, 2.0, 3.0, 4.0, 5.0]), 3.0);
    }

    #[test]
    fn test_mean_empty() {
        assert_eq!(mean(&[]), 0.0);
    }

    #[test]
    fn test_sample_std_dev_known() {
        // Sample std-dev of [2,4,4,4,5,5,7,9] ≈ 2.138 (divides by n-1 = 7)
        // Population std-dev would be ~2.0 (divides by n = 8)
        let values = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let sd = sample_std_dev(&values);
        assert!((sd - 2.138).abs() < 0.001, "expected ~2.138, got {sd}");
    }

    #[test]
    fn test_sample_std_dev_single() {
        assert_eq!(sample_std_dev(&[42.0]), 0.0);
    }

    #[test]
    fn test_sample_std_dev_empty() {
        assert_eq!(sample_std_dev(&[]), 0.0);
    }

    // ── z-score ──────────────────────────────────────────────────────────────

    #[test]
    fn test_z_score_zero_std() {
        assert_eq!(z_score(10.0, 10.0, 0.0), 0.0);
    }

    #[test]
    fn test_z_score_two_sigma_below() {
        let z = z_score(6.0, 10.0, 2.0);
        assert!((z - (-2.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_z_score_positive() {
        let z = z_score(14.0, 10.0, 2.0);
        assert!((z - 2.0).abs() < f64::EPSILON);
    }

    // ── linear regression ────────────────────────────────────────────────────

    #[test]
    fn test_linear_regression_perfect() {
        let xs = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let ys = vec![1.0, 3.0, 5.0, 7.0, 9.0]; // y = 2x + 1
        let (slope, intercept, r2) = linear_regression(&xs, &ys);
        assert!((slope - 2.0).abs() < 1e-9, "slope: {slope}");
        assert!((intercept - 1.0).abs() < 1e-9, "intercept: {intercept}");
        assert!((r2 - 1.0).abs() < 1e-9, "r²: {r2}");
    }

    #[test]
    fn test_linear_regression_flat() {
        let xs = vec![0.0, 1.0, 2.0, 3.0];
        let ys = vec![5.0, 5.0, 5.0, 5.0];
        let (slope, _intercept, r2) = linear_regression(&xs, &ys);
        assert!(slope.abs() < f64::EPSILON);
        assert!((r2 - 1.0).abs() < f64::EPSILON); // perfect horizontal line
    }

    #[test]
    fn test_linear_regression_negative_slope() {
        let xs = vec![0.0, 1.0, 2.0, 3.0];
        let ys = vec![10.0, 8.0, 6.0, 4.0]; // y = -2x + 10
        let (slope, intercept, r2) = linear_regression(&xs, &ys);
        assert!((slope - (-2.0)).abs() < 1e-9);
        assert!((intercept - 10.0).abs() < 1e-9);
        assert!((r2 - 1.0).abs() < 1e-9);
    }

    // ── confidence interval ───────────────────────────────────────────────────

    #[test]
    fn test_confidence_interval_95() {
        let samples = vec![10.0, 12.0, 11.0, 13.0, 10.5, 11.5, 12.5, 11.0, 10.0, 13.5];
        let (lo, hi) = RegressionDetector::confidence_interval(&samples, 0.95);
        assert!(lo < hi);
        let m = mean(&samples);
        assert!(lo < m && m < hi);
    }

    #[test]
    fn test_confidence_interval_single() {
        let (lo, hi) = RegressionDetector::confidence_interval(&[42.0], 0.95);
        assert_eq!(lo, 42.0);
        assert_eq!(hi, 42.0);
    }

    #[test]
    fn test_confidence_interval_empty() {
        let (lo, hi) = RegressionDetector::confidence_interval(&[], 0.95);
        assert_eq!(lo, 0.0);
        assert_eq!(hi, 0.0);
    }

    // ── severity ─────────────────────────────────────────────────────────────

    #[test]
    fn test_severity_thresholds() {
        assert_eq!(Severity::from_percent(3.0), Severity::Minor);
        assert_eq!(Severity::from_percent(10.0), Severity::Minor);
        assert_eq!(Severity::from_percent(20.0), Severity::Moderate);
        assert_eq!(Severity::from_percent(35.0), Severity::Major);
        assert_eq!(Severity::from_percent(55.0), Severity::Critical);
    }

    #[test]
    fn test_severity_labels() {
        assert_eq!(Severity::Minor.label(), "minor");
        assert_eq!(Severity::Moderate.label(), "moderate");
        assert_eq!(Severity::Major.label(), "major");
        assert_eq!(Severity::Critical.label(), "critical");
    }

    // ── RegressionKind ───────────────────────────────────────────────────────

    #[test]
    fn test_regression_kind_predicates() {
        let reg = RegressionKind::Regression {
            percent: 20.0,
            severity: Severity::Moderate,
        };
        assert!(reg.is_regression());
        assert!(!reg.is_improvement());

        let imp = RegressionKind::Improvement { percent: 10.0 };
        assert!(imp.is_improvement());
        assert!(!imp.is_regression());

        assert!(!RegressionKind::Stable.is_regression());
        assert!(!RegressionKind::Stable.is_improvement());
    }

    // ── RegressionDetector – no history ──────────────────────────────────────

    #[test]
    fn test_detect_no_history() {
        let detector = RegressionDetector::default();
        let current = make_record("bench_a", 1_700_000_000, 30.0, 33.0, 38.5);
        let analysis = detector.detect(&current, &[]);
        assert_eq!(analysis.fps_kind, RegressionKind::Stable);
        assert!(!analysis.has_regression);
        assert_eq!(analysis.baseline.sample_count, 0);
    }

    // ── RegressionDetector – stable history ──────────────────────────────────

    #[test]
    fn test_detect_stable_within_noise() {
        let detector = RegressionDetector::default();
        let history = stable_history(10);
        // Tiny deviation — within noise
        let current = make_record("bench_a", 1_700_040_000, 30.1, 33.0, 38.5);
        let analysis = detector.detect(&current, &history);
        assert_eq!(analysis.fps_kind, RegressionKind::Stable);
        assert!(!analysis.has_regression);
    }

    // ── RegressionDetector – clear FPS regression ────────────────────────────

    #[test]
    fn test_detect_fps_regression() {
        let detector = RegressionDetector::default();
        // History of stable 30 FPS with very small variance.
        let mut history: Vec<BenchmarkRecord> = (0..20)
            .map(|i| {
                make_record(
                    "bench_a",
                    1_700_000_000 + i as u64 * 3600,
                    30.0 + (i % 2) as f64 * 0.1, // ±0.1 fps jitter
                    33.0,
                    38.5,
                )
            })
            .collect();

        // Simulate a 40 % throughput drop.
        let current = make_record("bench_a", 1_700_080_000, 18.0, 33.0, 38.5);
        let analysis = detector.detect(&current, &history);

        // Consume the borrow of `history` after calling detect.
        history.clear();
        let _ = history; // suppress unused warning

        assert!(
            analysis.fps_kind.is_regression(),
            "expected FPS regression, got {:?}",
            analysis.fps_kind
        );
        assert!(analysis.has_regression);

        if let RegressionKind::Regression { severity, .. } = analysis.fps_kind {
            assert!(
                matches!(severity, Severity::Major | Severity::Critical),
                "expected major/critical severity, got {severity:?}"
            );
        }
    }

    // ── RegressionDetector – FPS improvement ─────────────────────────────────

    #[test]
    fn test_detect_fps_improvement() {
        let detector = RegressionDetector::default();
        let history: Vec<BenchmarkRecord> = (0..15)
            .map(|i| {
                make_record(
                    "bench_b",
                    1_700_000_000 + i as u64 * 3600,
                    30.0,
                    33.0,
                    38.5,
                )
            })
            .collect();

        // 40 % throughput gain.
        let current = make_record("bench_b", 1_700_060_000, 42.0, 33.0, 38.5);
        let analysis = detector.detect(&current, &history);
        assert!(
            analysis.fps_kind.is_improvement(),
            "expected improvement, got {:?}",
            analysis.fps_kind
        );
        assert!(!analysis.has_regression);
    }

    // ── RegressionDetector – latency regression ───────────────────────────────

    #[test]
    fn test_detect_latency_regression() {
        let detector = RegressionDetector::default();
        let history: Vec<BenchmarkRecord> = (0..15)
            .map(|i| make_record("bench_c", 1_700_000_000 + i as u64 * 3600, 30.0, 33.0, 38.5))
            .collect();

        // Latency doubles — clear regression.
        let current = make_record("bench_c", 1_700_060_000, 30.0, 70.0, 38.5);
        let analysis = detector.detect(&current, &history);
        assert!(
            analysis.latency_kind.is_regression(),
            "expected latency regression, got {:?}",
            analysis.latency_kind
        );
    }

    // ── RegressionDetector – quality regression ───────────────────────────────

    #[test]
    fn test_detect_quality_regression() {
        let detector = RegressionDetector::default();
        let history: Vec<BenchmarkRecord> = (0..15)
            .map(|i| make_record("bench_d", 1_700_000_000 + i as u64 * 3600, 30.0, 33.0, 40.0))
            .collect();

        // Quality drops from 40 dB to 20 dB.
        let current = make_record("bench_d", 1_700_060_000, 30.0, 33.0, 20.0);
        let analysis = detector.detect(&current, &history);
        assert!(
            analysis.quality_kind.is_regression(),
            "expected quality regression, got {:?}",
            analysis.quality_kind
        );
    }

    // ── worst_severity ────────────────────────────────────────────────────────

    #[test]
    fn test_worst_severity_none() {
        let analysis = RegressionAnalysis {
            name: "x".to_string(),
            fps_kind: RegressionKind::Stable,
            latency_kind: RegressionKind::Stable,
            quality_kind: RegressionKind::Stable,
            fps_z_score: 0.0,
            latency_z_score: 0.0,
            baseline: BenchmarkBaseline {
                mean_fps: 30.0,
                std_fps: 1.0,
                mean_latency_ms: 33.0,
                std_latency_ms: 1.0,
                mean_quality: 38.5,
                std_quality: 0.5,
                sample_count: 10,
            },
            has_regression: false,
        };
        assert!(analysis.worst_severity().is_none());
    }

    #[test]
    fn test_worst_severity_picks_highest() {
        let analysis = RegressionAnalysis {
            name: "x".to_string(),
            fps_kind: RegressionKind::Regression {
                percent: 10.0,
                severity: Severity::Minor,
            },
            latency_kind: RegressionKind::Regression {
                percent: 35.0,
                severity: Severity::Major,
            },
            quality_kind: RegressionKind::Stable,
            fps_z_score: -3.0,
            latency_z_score: 4.0,
            baseline: BenchmarkBaseline {
                mean_fps: 30.0,
                std_fps: 1.0,
                mean_latency_ms: 33.0,
                std_latency_ms: 1.0,
                mean_quality: 38.5,
                std_quality: 0.5,
                sample_count: 10,
            },
            has_regression: true,
        };
        assert_eq!(analysis.worst_severity(), Some(Severity::Major));
    }

    // ── BenchmarkHistory ──────────────────────────────────────────────────────

    #[test]
    fn test_history_add_and_len() {
        let mut h = BenchmarkHistory::new(5);
        assert!(h.is_empty());
        h.add(make_record("a", 1, 30.0, 33.0, 38.5));
        h.add(make_record("b", 2, 25.0, 40.0, 35.0));
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn test_history_eviction() {
        let mut h = BenchmarkHistory::new(3);
        for i in 0..6u64 {
            h.add(make_record("a", i, 30.0 + i as f64, 33.0, 38.5));
        }
        assert_eq!(h.len(), 3);
        // Oldest records should have been dropped; remaining FPS are 33, 34, 35.
        let fps: Vec<f64> = h.records.iter().map(|r| r.throughput_fps).collect();
        assert_eq!(fps, vec![33.0, 34.0, 35.0]);
    }

    #[test]
    fn test_history_baseline() {
        let mut h = BenchmarkHistory::new(100);
        for i in 0..10u64 {
            h.add(make_record("x", i, 30.0, 33.0, 38.5));
        }
        let b = h.baseline("x", 5).unwrap();
        assert_eq!(b.sample_count, 5);
        assert!((b.mean_fps - 30.0).abs() < f64::EPSILON);
        assert!((b.mean_latency_ms - 33.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_history_baseline_missing_name() {
        let h = BenchmarkHistory::new(100);
        assert!(h.baseline("nonexistent", 10).is_none());
    }

    #[test]
    fn test_history_trend_increasing() {
        let mut h = BenchmarkHistory::new(100);
        for i in 0..10u64 {
            h.add(make_record("y", i, 20.0 + i as f64 * 2.0, 33.0, 38.5));
        }
        let t = h.trend("y").unwrap();
        assert!(t.slope_fps_per_run > 0.0, "slope should be positive");
        assert!(!t.is_trending_down);
        assert!(t.r_squared > 0.99);
    }

    #[test]
    fn test_history_trend_decreasing() {
        let mut h = BenchmarkHistory::new(100);
        for i in 0..8u64 {
            h.add(make_record("z", i, 40.0 - i as f64 * 1.5, 33.0, 38.5));
        }
        let t = h.trend("z").unwrap();
        assert!(t.slope_fps_per_run < 0.0, "slope should be negative");
        assert!(t.is_trending_down);
    }

    #[test]
    fn test_history_trend_too_few_records() {
        let mut h = BenchmarkHistory::new(100);
        h.add(make_record("w", 1, 30.0, 33.0, 38.5));
        assert!(h.trend("w").is_none());
    }

    #[test]
    fn test_history_benchmark_names() {
        let mut h = BenchmarkHistory::new(100);
        h.add(make_record("encode_av1", 1, 30.0, 33.0, 38.5));
        h.add(make_record("encode_vp9", 2, 25.0, 40.0, 35.0));
        h.add(make_record("encode_av1", 3, 31.0, 33.0, 38.6));
        let names = h.benchmark_names();
        assert_eq!(names, vec!["encode_av1", "encode_vp9"]);
    }

    // ── JSON round-trip ───────────────────────────────────────────────────────

    #[test]
    fn test_history_json_roundtrip() {
        let mut h = BenchmarkHistory::new(50);
        let mut meta = HashMap::new();
        meta.insert("git_sha".to_string(), "abc123".to_string());
        h.add(BenchmarkRecord {
            name: "bench_json".to_string(),
            timestamp: 1_700_000_000,
            throughput_fps: 29.97,
            latency_ms: 33.37,
            memory_bytes: 1_024_000,
            quality_score: 42.1,
            metadata: meta,
        });

        let json = h.to_json();
        let restored = BenchmarkHistory::from_json(&json).unwrap();
        assert_eq!(restored.len(), 1);
        let r = &restored.records[0];
        assert_eq!(r.name, "bench_json");
        assert!((r.throughput_fps - 29.97).abs() < 1e-9);
        assert_eq!(r.metadata.get("git_sha"), Some(&"abc123".to_string()));
    }

    #[test]
    fn test_history_from_json_invalid() {
        assert!(BenchmarkHistory::from_json("not valid json {{").is_err());
    }

    // ── BenchmarkRecord::simple ───────────────────────────────────────────────

    #[test]
    fn test_record_simple_constructor() {
        let r = BenchmarkRecord::simple("my_bench", 12345, 60.0, 16.7);
        assert_eq!(r.name, "my_bench");
        assert_eq!(r.timestamp, 12345);
        assert!((r.throughput_fps - 60.0).abs() < f64::EPSILON);
        assert!((r.latency_ms - 16.7).abs() < f64::EPSILON);
        assert!(r.metadata.is_empty());
    }

    // ── DetectorConfig ────────────────────────────────────────────────────────

    #[test]
    fn test_detector_with_strict_config() {
        let config = DetectorConfig {
            z_score_threshold: 1.0, // very sensitive
            min_regression_percent: 1.0,
            confidence_level: 0.90,
        };
        let detector = RegressionDetector::with_config(config);

        let history: Vec<BenchmarkRecord> = (0..20)
            .map(|i| make_record("strict", 1_700_000_000 + i as u64 * 3600, 30.0, 33.0, 38.5))
            .collect();

        // Even a small dip triggers a regression with the strict config.
        let current = make_record("strict", 1_700_080_000, 27.0, 33.0, 38.5);
        let analysis = detector.detect(&current, &history);
        assert!(
            analysis.fps_kind.is_regression(),
            "strict detector should flag 10% drop: {:?}",
            analysis.fps_kind
        );
    }

    #[test]
    fn test_detector_with_lenient_config() {
        let config = DetectorConfig {
            z_score_threshold: 4.0, // very lenient
            min_regression_percent: 40.0,
            confidence_level: 0.99,
        };
        let detector = RegressionDetector::with_config(config);

        let history: Vec<BenchmarkRecord> = (0..20)
            .map(|i| make_record("lenient", 1_700_000_000 + i as u64 * 3600, 30.0, 33.0, 38.5))
            .collect();

        // A moderate 15% drop should not trigger with very lenient thresholds.
        let current = make_record("lenient", 1_700_080_000, 25.5, 33.0, 38.5);
        let analysis = detector.detect(&current, &history);
        assert!(
            !analysis.fps_kind.is_regression(),
            "lenient detector should not flag 15% drop: {:?}",
            analysis.fps_kind
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// detect_with_confidence — confidence-interval-aware regression analysis
// ─────────────────────────────────────────────────────────────────────────────

/// A regression analysis result that also carries confidence intervals for
/// each metric, enabling callers to reason about statistical uncertainty.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceRegressionAnalysis {
    /// Core regression analysis (same as [`RegressionAnalysis`]).
    pub core: RegressionAnalysis,
    /// 95 % confidence interval for the FPS baseline mean: (lower, upper).
    pub fps_ci: (f64, f64),
    /// 95 % confidence interval for the latency baseline mean: (lower, upper).
    pub latency_ci: (f64, f64),
    /// 95 % confidence interval for the quality baseline mean: (lower, upper).
    pub quality_ci: (f64, f64),
    /// Whether the current FPS value lies *outside* the FPS confidence interval.
    pub fps_outside_ci: bool,
    /// Whether the current latency value lies *outside* the latency confidence interval.
    pub latency_outside_ci: bool,
    /// Whether the current quality value lies *outside* the quality confidence interval.
    pub quality_outside_ci: bool,
    /// Confidence level used (e.g. 0.95).
    pub confidence_level: f64,
}

impl RegressionDetector {
    /// Like [`RegressionDetector::detect`] but additionally computes
    /// confidence intervals for each metric and reports whether the
    /// observed value falls outside those intervals.
    ///
    /// The confidence level is taken from [`DetectorConfig::confidence_level`].
    #[must_use]
    pub fn detect_with_confidence(
        &self,
        current: &BenchmarkRecord,
        history: &[BenchmarkRecord],
    ) -> ConfidenceRegressionAnalysis {
        let core = self.detect(current, history);

        let relevant: Vec<&BenchmarkRecord> = history
            .iter()
            .filter(|r| r.name == current.name)
            .collect();

        let conf = self.config.confidence_level;

        if relevant.is_empty() {
            return ConfidenceRegressionAnalysis {
                fps_ci: (current.throughput_fps, current.throughput_fps),
                latency_ci: (current.latency_ms, current.latency_ms),
                quality_ci: (current.quality_score, current.quality_score),
                fps_outside_ci: false,
                latency_outside_ci: false,
                quality_outside_ci: false,
                confidence_level: conf,
                core,
            };
        }

        let fps_vals: Vec<f64> = relevant.iter().map(|r| r.throughput_fps).collect();
        let lat_vals: Vec<f64> = relevant.iter().map(|r| r.latency_ms).collect();
        let qual_vals: Vec<f64> = relevant.iter().map(|r| r.quality_score).collect();

        let fps_ci = Self::confidence_interval(&fps_vals, conf);
        let latency_ci = Self::confidence_interval(&lat_vals, conf);
        let quality_ci = Self::confidence_interval(&qual_vals, conf);

        let fps_outside_ci =
            current.throughput_fps < fps_ci.0 || current.throughput_fps > fps_ci.1;
        let latency_outside_ci =
            current.latency_ms < latency_ci.0 || current.latency_ms > latency_ci.1;
        let quality_outside_ci =
            current.quality_score < quality_ci.0 || current.quality_score > quality_ci.1;

        ConfidenceRegressionAnalysis {
            core,
            fps_ci,
            latency_ci,
            quality_ci,
            fps_outside_ci,
            quality_outside_ci,
            latency_outside_ci,
            confidence_level: conf,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TrendAnalyzer — advanced trend analysis including Mann-Kendall test
// ─────────────────────────────────────────────────────────────────────────────

/// Result of the Mann-Kendall monotonic trend test.
///
/// The Mann-Kendall test is a non-parametric test for monotonic trends in a
/// time series.  It does not assume normality and is robust against outliers.
///
/// Reference: Mann (1945), Kendall (1975).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MannKendallResult {
    /// Kendall's S statistic (sum of sign differences).
    pub s_statistic: f64,
    /// Variance of S (accounts for ties).
    pub var_s: f64,
    /// Standardised Z score.
    pub z_score: f64,
    /// Approximate two-sided p-value (normal approximation, valid for n ≥ 8).
    pub p_value: f64,
    /// Detected trend direction.
    pub trend: MannKendallTrend,
    /// Whether the trend is statistically significant at α = 0.05.
    pub significant: bool,
    /// Number of data points used.
    pub n: usize,
    /// Sen's slope estimator (median of pair-wise slopes).
    pub sens_slope: f64,
}

/// Trend direction from the Mann-Kendall test.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MannKendallTrend {
    /// Statistically significant increasing trend.
    Increasing,
    /// Statistically significant decreasing trend.
    Decreasing,
    /// No statistically significant monotonic trend.
    NoTrend,
}

/// Advanced trend analysis on time series of benchmark records.
#[derive(Debug, Clone, Default)]
pub struct TrendAnalyzer;

impl TrendAnalyzer {
    /// Creates a new `TrendAnalyzer`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Performs the Mann-Kendall monotonic trend test on the FPS values of
    /// the records for the given benchmark name.
    ///
    /// Returns `None` if fewer than 4 records are available (test is
    /// unreliable with very small samples).
    #[must_use]
    pub fn mann_kendall_test(
        &self,
        name: &str,
        history: &[BenchmarkRecord],
    ) -> Option<MannKendallResult> {
        let values: Vec<f64> = history
            .iter()
            .filter(|r| r.name == name)
            .map(|r| r.throughput_fps)
            .collect();

        if values.len() < 4 {
            return None;
        }

        Some(mann_kendall_inner(&values))
    }

    /// Performs Mann-Kendall on an arbitrary slice of f64 values.
    ///
    /// Returns `None` if fewer than 4 values.
    #[must_use]
    pub fn mann_kendall_values(&self, values: &[f64]) -> Option<MannKendallResult> {
        if values.len() < 4 {
            return None;
        }
        Some(mann_kendall_inner(values))
    }
}

/// Core Mann-Kendall computation.
fn mann_kendall_inner(values: &[f64]) -> MannKendallResult {
    let n = values.len();

    // Compute S statistic: sum of sign(values[j] - values[i]) for all i < j.
    let mut s = 0.0_f64;
    for i in 0..n {
        for j in (i + 1)..n {
            let diff = values[j] - values[i];
            if diff > 0.0 {
                s += 1.0;
            } else if diff < 0.0 {
                s -= 1.0;
            }
            // diff == 0 contributes 0 (tie)
        }
    }

    // Compute variance of S accounting for ties.
    // Var(S) = [n(n-1)(2n+5) - Σ_g t_g(t_g-1)(2t_g+5)] / 18
    // where t_g is the count of the g-th tied group.
    let n_f = n as f64;
    let base_var = n_f * (n_f - 1.0) * (2.0 * n_f + 5.0) / 18.0;

    // Count ties
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut tie_correction = 0.0_f64;
    let mut i = 0;
    while i < sorted.len() {
        let mut j = i + 1;
        while j < sorted.len() && (sorted[j] - sorted[i]).abs() < f64::EPSILON {
            j += 1;
        }
        let t = (j - i) as f64;
        if t > 1.0 {
            tie_correction += t * (t - 1.0) * (2.0 * t + 5.0) / 18.0;
        }
        i = j;
    }

    let var_s = (base_var - tie_correction).max(0.0);

    // Compute Z score with continuity correction
    let z = if s > 0.0 {
        (s - 1.0) / var_s.sqrt().max(f64::EPSILON)
    } else if s < 0.0 {
        (s + 1.0) / var_s.sqrt().max(f64::EPSILON)
    } else {
        0.0
    };

    // Two-sided p-value using normal approximation (Φ is the standard normal CDF)
    let p_value = 2.0 * (1.0 - standard_normal_cdf(z.abs()));

    // Significance at α = 0.05 (|z| > 1.96)
    let significant = z.abs() > 1.96;

    // Trend direction
    let trend = if significant {
        if s > 0.0 {
            MannKendallTrend::Increasing
        } else {
            MannKendallTrend::Decreasing
        }
    } else {
        MannKendallTrend::NoTrend
    };

    // Sen's slope estimator: median of all pair-wise slopes (x_j - x_i)/(j - i)
    let mut slopes: Vec<f64> = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            slopes.push((values[j] - values[i]) / (j - i) as f64);
        }
    }
    slopes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let sens_slope = if slopes.is_empty() {
        0.0
    } else {
        let mid = slopes.len() / 2;
        if slopes.len() % 2 == 0 {
            (slopes[mid - 1] + slopes[mid]) / 2.0
        } else {
            slopes[mid]
        }
    };

    MannKendallResult {
        s_statistic: s,
        var_s,
        z_score: z,
        p_value,
        trend,
        significant,
        n,
        sens_slope,
    }
}

/// Standard normal CDF approximation using the rational approximation by
/// Abramowitz and Stegun §26.2.17 (maximum error 7.5e-8).
fn standard_normal_cdf(x: f64) -> f64 {
    if x < 0.0 {
        return 1.0 - standard_normal_cdf(-x);
    }
    let t = 1.0 / (1.0 + 0.2316419 * x);
    let poly = t * (0.319_381_53
        + t * (-0.356_563_782
            + t * (1.781_477_937 + t * (-1.821_255_978 + t * 1.330_274_429))));
    1.0 - ((-x * x / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt()) * poly
}

// ─────────────────────────────────────────────────────────────────────────────
// OutlierDetector — IQR-based and Z-score-based outlier detection
// ─────────────────────────────────────────────────────────────────────────────

/// Outlier detection result for a single value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlierInfo {
    /// The index of this value in the original slice.
    pub index: usize,
    /// The outlier value.
    pub value: f64,
    /// The lower fence used (Q1 - 1.5*IQR for standard; Q1 - 3*IQR for extreme).
    pub lower_fence: f64,
    /// The upper fence used.
    pub upper_fence: f64,
    /// Whether this is a mild outlier (outside 1.5*IQR) vs extreme (outside 3*IQR).
    pub is_extreme: bool,
}

/// Result of IQR outlier detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IqrOutlierResult {
    /// Q1 (25th percentile).
    pub q1: f64,
    /// Median (50th percentile).
    pub median: f64,
    /// Q3 (75th percentile).
    pub q3: f64,
    /// Inter-quartile range: Q3 - Q1.
    pub iqr: f64,
    /// Lower mild fence: Q1 - 1.5 * IQR.
    pub lower_mild_fence: f64,
    /// Upper mild fence: Q3 + 1.5 * IQR.
    pub upper_mild_fence: f64,
    /// Lower extreme fence: Q1 - 3.0 * IQR.
    pub lower_extreme_fence: f64,
    /// Upper extreme fence: Q3 + 3.0 * IQR.
    pub upper_extreme_fence: f64,
    /// All detected outliers.
    pub outliers: Vec<OutlierInfo>,
    /// Cleaned data — the input with outliers removed.
    pub cleaned: Vec<f64>,
}

impl IqrOutlierResult {
    /// Returns the number of detected outliers.
    #[must_use]
    pub fn outlier_count(&self) -> usize {
        self.outliers.len()
    }

    /// Returns `true` if any outliers were detected.
    #[must_use]
    pub fn has_outliers(&self) -> bool {
        !self.outliers.is_empty()
    }
}

/// Detects statistical outliers in benchmark data.
#[derive(Debug, Clone, Default)]
pub struct OutlierDetector;

impl OutlierDetector {
    /// Creates a new `OutlierDetector`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Detects outliers in `values` using the IQR (Tukey fences) method.
    ///
    /// A value `x` is considered a *mild* outlier if:
    ///   x < Q1 − 1.5·IQR  or  x > Q3 + 1.5·IQR
    ///
    /// A value is an *extreme* outlier if:
    ///   x < Q1 − 3·IQR    or  x > Q3 + 3·IQR
    ///
    /// Returns `None` if `values` has fewer than 4 elements (quartiles are
    /// unreliable with very small samples).
    #[must_use]
    pub fn iqr_method(&self, values: &[f64]) -> Option<IqrOutlierResult> {
        if values.len() < 4 {
            return None;
        }

        let mut sorted: Vec<(usize, f64)> = values.iter().copied().enumerate().collect();
        sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let sorted_vals: Vec<f64> = sorted.iter().map(|(_, v)| *v).collect();

        let q1 = percentile(&sorted_vals, 0.25);
        let median = percentile(&sorted_vals, 0.50);
        let q3 = percentile(&sorted_vals, 0.75);
        let iqr = q3 - q1;

        let lower_mild = q1 - 1.5 * iqr;
        let upper_mild = q3 + 1.5 * iqr;
        let lower_extreme = q1 - 3.0 * iqr;
        let upper_extreme = q3 + 3.0 * iqr;

        let mut outliers: Vec<OutlierInfo> = Vec::new();
        let mut cleaned: Vec<f64> = Vec::new();

        for &(original_idx, val) in &sorted {
            let is_mild_outlier = val < lower_mild || val > upper_mild;
            let is_extreme_outlier = val < lower_extreme || val > upper_extreme;

            if is_mild_outlier {
                let (lower_fence, upper_fence) = if is_extreme_outlier {
                    (lower_extreme, upper_extreme)
                } else {
                    (lower_mild, upper_mild)
                };
                outliers.push(OutlierInfo {
                    index: original_idx,
                    value: val,
                    lower_fence,
                    upper_fence,
                    is_extreme: is_extreme_outlier,
                });
            } else {
                cleaned.push(val);
            }
        }

        // Restore cleaned data to original index order
        let mut cleaned_indexed: Vec<(usize, f64)> = cleaned
            .iter()
            .copied()
            .zip(
                sorted
                    .iter()
                    .filter(|(orig_i, v)| {
                        let is_mild = *v < lower_mild || *v > upper_mild;
                        let _ = orig_i;
                        !is_mild
                    })
                    .map(|(i, _)| *i),
            )
            .map(|(v, i)| (i, v))
            .collect();
        cleaned_indexed.sort_by_key(|(i, _)| *i);
        let cleaned: Vec<f64> = cleaned_indexed.iter().map(|(_, v)| *v).collect();

        Some(IqrOutlierResult {
            q1,
            median,
            q3,
            iqr,
            lower_mild_fence: lower_mild,
            upper_mild_fence: upper_mild,
            lower_extreme_fence: lower_extreme,
            upper_extreme_fence: upper_extreme,
            outliers,
            cleaned,
        })
    }

    /// Detects outlier benchmark *records* for a given benchmark name using
    /// the IQR method on throughput (FPS) values.
    ///
    /// Returns `None` if fewer than 4 records are found.
    #[must_use]
    pub fn detect_fps_outliers<'a>(
        &self,
        name: &str,
        history: &'a [BenchmarkRecord],
    ) -> Option<(IqrOutlierResult, Vec<&'a BenchmarkRecord>)> {
        let records: Vec<&BenchmarkRecord> =
            history.iter().filter(|r| r.name == name).collect();

        if records.len() < 4 {
            return None;
        }

        let fps: Vec<f64> = records.iter().map(|r| r.throughput_fps).collect();
        let iqr_result = self.iqr_method(&fps)?;

        // Map outlier indices back to the original records
        let outlier_records: Vec<&BenchmarkRecord> = iqr_result
            .outliers
            .iter()
            .map(|o| records[o.index])
            .collect();

        Some((iqr_result, outlier_records))
    }
}

/// Compute the p-th percentile of an already-sorted slice using linear interpolation.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }

    let h = p * (sorted.len() - 1) as f64;
    let lo = h.floor() as usize;
    let hi = (lo + 1).min(sorted.len() - 1);
    let frac = h - lo as f64;

    sorted[lo] + frac * (sorted[hi] - sorted[lo])
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests for new features
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod enhanced_tests {
    use super::*;

    fn make_record(name: &str, ts: u64, fps: f64, latency: f64, quality: f64) -> BenchmarkRecord {
        BenchmarkRecord {
            name: name.to_string(),
            timestamp: ts,
            throughput_fps: fps,
            latency_ms: latency,
            memory_bytes: 512_000_000,
            quality_score: quality,
            metadata: HashMap::new(),
        }
    }

    // ── detect_with_confidence ────────────────────────────────────────────────

    #[test]
    fn test_detect_with_confidence_stable() {
        let detector = RegressionDetector::default();
        let history: Vec<BenchmarkRecord> = (0..20)
            .map(|i| make_record("bench_ci", 1_700_000_000 + i as u64 * 3600, 30.0, 33.0, 38.5))
            .collect();
        let current = make_record("bench_ci", 1_700_080_000, 29.8, 33.1, 38.4);
        let result = detector.detect_with_confidence(&current, &history);
        // Small deviation — within CI
        assert_eq!(result.confidence_level, 0.95);
        let (lo, hi) = result.fps_ci;
        assert!(lo < hi, "CI lower should be < upper");
    }

    #[test]
    fn test_detect_with_confidence_regression_outside_ci() {
        let detector = RegressionDetector::default();
        // Tight distribution: all exactly 30.0 FPS
        let history: Vec<BenchmarkRecord> = (0..20)
            .map(|i| make_record("bench_ci2", 1_700_000_000 + i as u64 * 3600, 30.0, 33.0, 38.5))
            .collect();
        // Big drop — should be outside CI
        let current = make_record("bench_ci2", 1_700_080_000, 10.0, 33.0, 38.5);
        let result = detector.detect_with_confidence(&current, &history);
        assert!(
            result.fps_outside_ci || result.core.fps_kind.is_regression(),
            "Expected CI breach or regression flag"
        );
    }

    #[test]
    fn test_detect_with_confidence_no_history() {
        let detector = RegressionDetector::default();
        let current = make_record("new_bench", 1_700_000_000, 30.0, 33.0, 38.5);
        let result = detector.detect_with_confidence(&current, &[]);
        assert_eq!(result.fps_ci.0, 30.0);
        assert_eq!(result.fps_ci.1, 30.0);
        assert!(!result.fps_outside_ci);
    }

    #[test]
    fn test_detect_with_confidence_uses_config_level() {
        let config = DetectorConfig {
            z_score_threshold: 2.0,
            min_regression_percent: 5.0,
            confidence_level: 0.99,
        };
        let detector = RegressionDetector::with_config(config);
        let history: Vec<BenchmarkRecord> = (0..20)
            .map(|i| make_record("bench_99", 1_700_000_000 + i as u64 * 3600, 30.0 + i as f64 * 0.1, 33.0, 38.5))
            .collect();
        let current = make_record("bench_99", 1_700_080_000, 32.0, 33.0, 38.5);
        let result = detector.detect_with_confidence(&current, &history);
        assert_eq!(result.confidence_level, 0.99);
        // CI for 0.99 is wider than for 0.95
        let (lo, hi) = result.fps_ci;
        assert!(hi - lo > 0.0);
    }

    // ── Mann-Kendall test ──────────────────────────────────────────────────────

    #[test]
    fn test_mann_kendall_increasing_trend() {
        let analyzer = TrendAnalyzer::new();
        let history: Vec<BenchmarkRecord> = (0..20)
            .map(|i| make_record("mk_inc", 1_700_000_000 + i as u64 * 3600, 10.0 + i as f64, 33.0, 38.5))
            .collect();
        let result = analyzer.mann_kendall_test("mk_inc", &history).unwrap();
        assert_eq!(result.trend, MannKendallTrend::Increasing);
        assert!(result.significant);
        assert!(result.s_statistic > 0.0);
        assert!(result.sens_slope > 0.0);
    }

    #[test]
    fn test_mann_kendall_decreasing_trend() {
        let analyzer = TrendAnalyzer::new();
        let history: Vec<BenchmarkRecord> = (0..20)
            .map(|i| make_record("mk_dec", 1_700_000_000 + i as u64 * 3600, 50.0 - i as f64 * 1.5, 33.0, 38.5))
            .collect();
        let result = analyzer.mann_kendall_test("mk_dec", &history).unwrap();
        assert_eq!(result.trend, MannKendallTrend::Decreasing);
        assert!(result.significant);
        assert!(result.s_statistic < 0.0);
        assert!(result.sens_slope < 0.0);
    }

    #[test]
    fn test_mann_kendall_no_trend_constant() {
        let analyzer = TrendAnalyzer::new();
        let history: Vec<BenchmarkRecord> = (0..15)
            .map(|i| make_record("mk_flat", 1_700_000_000 + i as u64 * 3600, 30.0, 33.0, 38.5))
            .collect();
        let result = analyzer.mann_kendall_test("mk_flat", &history).unwrap();
        assert_eq!(result.trend, MannKendallTrend::NoTrend);
        assert!(!result.significant);
    }

    #[test]
    fn test_mann_kendall_not_enough_data() {
        let analyzer = TrendAnalyzer::new();
        let history: Vec<BenchmarkRecord> = (0..3)
            .map(|i| make_record("mk_few", 1_700_000_000 + i as u64, 30.0, 33.0, 38.5))
            .collect();
        assert!(analyzer.mann_kendall_test("mk_few", &history).is_none());
    }

    #[test]
    fn test_mann_kendall_values_directly() {
        let analyzer = TrendAnalyzer::new();
        let values: Vec<f64> = (0..15).map(|i| i as f64 * 2.0).collect();
        let result = analyzer.mann_kendall_values(&values).unwrap();
        assert_eq!(result.trend, MannKendallTrend::Increasing);
        assert!(result.n == 15);
    }

    #[test]
    fn test_mann_kendall_p_value_range() {
        let analyzer = TrendAnalyzer::new();
        let values: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let result = analyzer.mann_kendall_values(&values).unwrap();
        assert!(result.p_value >= 0.0 && result.p_value <= 1.0);
    }

    #[test]
    fn test_mann_kendall_sens_slope_constant() {
        let analyzer = TrendAnalyzer::new();
        let values = vec![5.0; 10];
        let result = analyzer.mann_kendall_values(&values).unwrap();
        assert!(result.sens_slope.abs() < f64::EPSILON);
    }

    // ── IQR outlier detection ─────────────────────────────────────────────────

    #[test]
    fn test_iqr_no_outliers() {
        let detector = OutlierDetector::new();
        let values: Vec<f64> = (0..20).map(|i| 30.0 + i as f64 * 0.1).collect();
        let result = detector.iqr_method(&values).unwrap();
        assert!(!result.has_outliers(), "Uniformly distributed data should have no outliers");
        assert!(result.iqr > 0.0);
    }

    #[test]
    fn test_iqr_detects_high_outlier() {
        let detector = OutlierDetector::new();
        let mut values: Vec<f64> = vec![30.0; 20];
        values[5] = 1000.0; // Massive outlier
        let result = detector.iqr_method(&values).unwrap();
        assert!(result.has_outliers(), "Should detect the 1000.0 outlier");
        assert!(result.outliers.iter().any(|o| (o.value - 1000.0).abs() < 1.0));
    }

    #[test]
    fn test_iqr_detects_low_outlier() {
        let detector = OutlierDetector::new();
        let mut values: Vec<f64> = vec![30.0; 20];
        values[10] = -500.0; // Very low outlier
        let result = detector.iqr_method(&values).unwrap();
        assert!(result.has_outliers(), "Should detect the -500.0 outlier");
        assert!(result.outliers.iter().any(|o| (o.value - (-500.0)).abs() < 1.0));
    }

    #[test]
    fn test_iqr_extreme_vs_mild() {
        let detector = OutlierDetector::new();
        let mut values: Vec<f64> = (0..20).map(|_| 30.0).collect();
        values[0] = 200.0; // Extreme outlier (well beyond 3*IQR)
        let result = detector.iqr_method(&values).unwrap();
        let extreme: Vec<_> = result.outliers.iter().filter(|o| o.is_extreme).collect();
        assert!(!extreme.is_empty(), "Expected at least one extreme outlier");
    }

    #[test]
    fn test_iqr_too_few_values() {
        let detector = OutlierDetector::new();
        let values = vec![1.0, 2.0, 3.0]; // Only 3 values
        assert!(detector.iqr_method(&values).is_none());
    }

    #[test]
    fn test_iqr_cleaned_data_excludes_outliers() {
        let detector = OutlierDetector::new();
        let mut values: Vec<f64> = vec![10.0; 20];
        values[7] = 9999.0;
        let result = detector.iqr_method(&values).unwrap();
        assert!(!result.cleaned.contains(&9999.0), "Outlier should be removed from cleaned data");
    }

    #[test]
    fn test_iqr_quartiles_ordered() {
        let detector = OutlierDetector::new();
        let values: Vec<f64> = (0..30).map(|i| i as f64).collect();
        let result = detector.iqr_method(&values).unwrap();
        assert!(result.q1 <= result.median, "Q1 ≤ median");
        assert!(result.median <= result.q3, "median ≤ Q3");
        assert!(result.iqr >= 0.0, "IQR ≥ 0");
    }

    #[test]
    fn test_iqr_fps_outliers_on_records() {
        let detector = OutlierDetector::new();
        let mut history: Vec<BenchmarkRecord> = (0..20)
            .map(|i| make_record("fps_out", 1_700_000_000 + i as u64 * 3600, 30.0, 33.0, 38.5))
            .collect();
        history[3].throughput_fps = 999.9; // Outlier record
        let (iqr_result, outlier_records) = detector
            .detect_fps_outliers("fps_out", &history)
            .unwrap();
        assert!(iqr_result.has_outliers());
        assert!(!outlier_records.is_empty());
        assert!((outlier_records[0].throughput_fps - 999.9).abs() < 1.0);
    }

    #[test]
    fn test_percentile_basic() {
        let sorted = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((percentile(&sorted, 0.0) - 1.0).abs() < 1e-9);
        assert!((percentile(&sorted, 1.0) - 5.0).abs() < 1e-9);
        assert!((percentile(&sorted, 0.5) - 3.0).abs() < 1e-9);
    }
}
