//! Quality gate evaluation for enforcing minimum quality thresholds.
//!
//! A [`QualityGate`] holds a set of [`GateThreshold`]s and evaluates a map of
//! named metric scores, returning a structured [`GateResult`] that identifies
//! which thresholds passed or failed.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A minimum (or maximum) threshold for a named quality metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateThreshold {
    /// Name of the metric this threshold applies to (e.g. `"vmaf"`, `"psnr"`).
    pub metric: String,
    /// Minimum acceptable score.  If `None`, no lower bound is applied.
    pub min_score: Option<f64>,
    /// Maximum acceptable score.  If `None`, no upper bound is applied.
    pub max_score: Option<f64>,
    /// Human-readable description of this threshold.
    pub description: String,
}

impl GateThreshold {
    /// Creates a lower-bound threshold: the metric must be ≥ `min_score`.
    #[must_use]
    pub fn at_least(metric: impl Into<String>, min_score: f64) -> Self {
        Self {
            metric: metric.into(),
            min_score: Some(min_score),
            max_score: None,
            description: String::new(),
        }
    }

    /// Creates an upper-bound threshold: the metric must be ≤ `max_score`.
    #[must_use]
    pub fn at_most(metric: impl Into<String>, max_score: f64) -> Self {
        Self {
            metric: metric.into(),
            min_score: None,
            max_score: Some(max_score),
            description: String::new(),
        }
    }

    /// Attaches a human-readable description, consuming and returning `self`.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Evaluates the threshold against a single numeric score.
    ///
    /// Returns `true` if the score satisfies all configured bounds.
    #[must_use]
    pub fn passes(&self, score: f64) -> bool {
        if let Some(min) = self.min_score {
            if score < min {
                return false;
            }
        }
        if let Some(max) = self.max_score {
            if score > max {
                return false;
            }
        }
        true
    }
}

/// Detailed outcome of evaluating a single threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdOutcome {
    /// The metric name.
    pub metric: String,
    /// Whether the threshold was satisfied.
    pub passed: bool,
    /// Actual score supplied to the gate.
    pub actual_score: f64,
    /// Threshold that was applied.
    pub threshold: GateThreshold,
}

/// Aggregate result of evaluating all thresholds in a [`QualityGate`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    /// `true` iff every threshold passed.
    pub passed: bool,
    /// Per-threshold outcomes (in evaluation order).
    pub outcomes: Vec<ThresholdOutcome>,
}

impl GateResult {
    /// Returns the outcomes for thresholds that failed.
    #[must_use]
    pub fn failures(&self) -> Vec<&ThresholdOutcome> {
        self.outcomes.iter().filter(|o| !o.passed).collect()
    }

    /// Returns the number of thresholds that failed.
    #[must_use]
    pub fn failure_count(&self) -> usize {
        self.outcomes.iter().filter(|o| !o.passed).count()
    }

    /// Returns the number of thresholds that passed.
    #[must_use]
    pub fn pass_count(&self) -> usize {
        self.outcomes.iter().filter(|o| o.passed).count()
    }
}

/// A configurable quality gate composed of one or more [`GateThreshold`]s.
///
/// Call [`QualityGate::evaluate`] with a map of metric scores to obtain a
/// [`GateResult`].
#[derive(Debug, Clone, Default)]
pub struct QualityGate {
    thresholds: Vec<GateThreshold>,
}

impl QualityGate {
    /// Creates an empty quality gate.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a threshold to this gate.
    pub fn add_threshold(&mut self, threshold: GateThreshold) {
        self.thresholds.push(threshold);
    }

    /// Builder-style threshold addition.
    #[must_use]
    pub fn with_threshold(mut self, threshold: GateThreshold) -> Self {
        self.add_threshold(threshold);
        self
    }

    /// Returns the number of configured thresholds.
    #[must_use]
    pub fn threshold_count(&self) -> usize {
        self.thresholds.len()
    }

    /// Evaluates the gate against the supplied metric scores.
    ///
    /// Metrics that appear in the gate but are absent from `scores` are treated
    /// as `0.0`.
    #[must_use]
    pub fn evaluate(&self, scores: &HashMap<String, f64>) -> GateResult {
        let mut outcomes = Vec::with_capacity(self.thresholds.len());
        let mut all_passed = true;

        for threshold in &self.thresholds {
            let actual_score = scores.get(&threshold.metric).copied().unwrap_or(0.0);
            let passed = threshold.passes(actual_score);
            if !passed {
                all_passed = false;
            }
            outcomes.push(ThresholdOutcome {
                metric: threshold.metric.clone(),
                passed,
                actual_score,
                threshold: threshold.clone(),
            });
        }

        GateResult {
            passed: all_passed,
            outcomes,
        }
    }

    /// Convenience method: returns `true` iff all thresholds pass for `scores`.
    #[must_use]
    pub fn passes(&self, scores: &HashMap<String, f64>) -> bool {
        self.evaluate(scores).passed
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn scores(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn test_threshold_at_least_passes() {
        let t = GateThreshold::at_least("vmaf", 80.0);
        assert!(t.passes(90.0));
        assert!(t.passes(80.0));
    }

    #[test]
    fn test_threshold_at_least_fails() {
        let t = GateThreshold::at_least("vmaf", 80.0);
        assert!(!t.passes(79.9));
    }

    #[test]
    fn test_threshold_at_most_passes() {
        let t = GateThreshold::at_most("noise", 5.0);
        assert!(t.passes(3.0));
        assert!(t.passes(5.0));
    }

    #[test]
    fn test_threshold_at_most_fails() {
        let t = GateThreshold::at_most("noise", 5.0);
        assert!(!t.passes(5.1));
    }

    #[test]
    fn test_threshold_with_description() {
        let t = GateThreshold::at_least("vmaf", 90.0).with_description("Broadcast quality");
        assert_eq!(t.description, "Broadcast quality");
    }

    #[test]
    fn test_gate_all_pass() {
        let gate = QualityGate::new()
            .with_threshold(GateThreshold::at_least("vmaf", 80.0))
            .with_threshold(GateThreshold::at_least("psnr", 35.0));
        let result = gate.evaluate(&scores(&[("vmaf", 92.0), ("psnr", 40.0)]));
        assert!(result.passed);
        assert_eq!(result.failure_count(), 0);
        assert_eq!(result.pass_count(), 2);
    }

    #[test]
    fn test_gate_one_fail() {
        let gate = QualityGate::new()
            .with_threshold(GateThreshold::at_least("vmaf", 80.0))
            .with_threshold(GateThreshold::at_least("psnr", 35.0));
        let result = gate.evaluate(&scores(&[("vmaf", 92.0), ("psnr", 30.0)]));
        assert!(!result.passed);
        assert_eq!(result.failure_count(), 1);
        assert_eq!(result.failures()[0].metric, "psnr");
    }

    #[test]
    fn test_gate_missing_metric_treated_as_zero() {
        let gate = QualityGate::new().with_threshold(GateThreshold::at_least("vmaf", 80.0));
        let result = gate.evaluate(&scores(&[]));
        assert!(!result.passed);
        assert_eq!(result.outcomes[0].actual_score, 0.0);
    }

    #[test]
    fn test_gate_passes_convenience() {
        let gate = QualityGate::new().with_threshold(GateThreshold::at_least("ssim", 0.9));
        assert!(gate.passes(&scores(&[("ssim", 0.95)])));
        assert!(!gate.passes(&scores(&[("ssim", 0.85)])));
    }

    #[test]
    fn test_gate_threshold_count() {
        let gate = QualityGate::new()
            .with_threshold(GateThreshold::at_least("a", 0.0))
            .with_threshold(GateThreshold::at_least("b", 0.0));
        assert_eq!(gate.threshold_count(), 2);
    }

    #[test]
    fn test_empty_gate_passes_anything() {
        let gate = QualityGate::new();
        assert!(gate.passes(&scores(&[])));
    }

    #[test]
    fn test_gate_result_failures_list() {
        let gate = QualityGate::new()
            .with_threshold(GateThreshold::at_least("vmaf", 80.0))
            .with_threshold(GateThreshold::at_least("psnr", 40.0));
        let result = gate.evaluate(&scores(&[("vmaf", 70.0), ("psnr", 38.0)]));
        assert_eq!(result.failures().len(), 2);
    }

    #[test]
    fn test_threshold_both_bounds() {
        let t = GateThreshold {
            metric: "brightness".to_string(),
            min_score: Some(0.1),
            max_score: Some(0.9),
            description: String::new(),
        };
        assert!(t.passes(0.5));
        assert!(!t.passes(0.05));
        assert!(!t.passes(0.95));
    }

    #[test]
    fn test_gate_result_pass_count() {
        let gate = QualityGate::new()
            .with_threshold(GateThreshold::at_least("a", 50.0))
            .with_threshold(GateThreshold::at_least("b", 50.0))
            .with_threshold(GateThreshold::at_least("c", 50.0));
        let result = gate.evaluate(&scores(&[("a", 60.0), ("b", 40.0), ("c", 70.0)]));
        assert_eq!(result.pass_count(), 2);
        assert_eq!(result.failure_count(), 1);
    }
}
