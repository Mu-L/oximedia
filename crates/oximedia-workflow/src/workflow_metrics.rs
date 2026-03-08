//! Workflow runtime metrics collection for `oximedia-workflow`.
//!
//! [`WorkflowMetricsCollector`] accumulates [`MetricSample`]s tagged with a
//! [`WorkflowMetric`] variant and exposes summary statistics (count, sum,
//! average, min, max) without requiring an external dependency.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Metric kinds ──────────────────────────────────────────────────────────────

/// Discriminates the type of measurement stored in a [`MetricSample`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkflowMetric {
    /// Wall-clock time a task spent in the queue before execution (seconds).
    QueueWaitSeconds,
    /// Wall-clock execution time of a single task (seconds).
    TaskDurationSeconds,
    /// Peak resident memory used by a task (bytes).
    TaskMemoryBytes,
    /// CPU utilisation percentage sampled during task execution (0–100).
    CpuPercent,
    /// Number of retry attempts consumed by a task before success or failure.
    RetryCount,
    /// Total workflow wall-clock time from submission to completion (seconds).
    WorkflowDurationSeconds,
}

impl WorkflowMetric {
    /// Returns the SI unit label for this metric.
    #[must_use]
    pub fn unit(self) -> &'static str {
        match self {
            Self::QueueWaitSeconds | Self::TaskDurationSeconds | Self::WorkflowDurationSeconds => {
                "s"
            }
            Self::TaskMemoryBytes => "bytes",
            Self::CpuPercent => "%",
            Self::RetryCount => "count",
        }
    }

    /// Returns `true` when lower values indicate better performance.
    #[must_use]
    pub fn lower_is_better(self) -> bool {
        !matches!(self, Self::CpuPercent)
    }
}

impl std::fmt::Display for WorkflowMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::QueueWaitSeconds => "queue_wait_seconds",
            Self::TaskDurationSeconds => "task_duration_seconds",
            Self::TaskMemoryBytes => "task_memory_bytes",
            Self::CpuPercent => "cpu_percent",
            Self::RetryCount => "retry_count",
            Self::WorkflowDurationSeconds => "workflow_duration_seconds",
        };
        write!(f, "{s}")
    }
}

// ── Sample ────────────────────────────────────────────────────────────────────

/// A single metric observation tied to a workflow or task identifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricSample {
    /// The kind of metric being recorded.
    pub metric: WorkflowMetric,
    /// Identifier of the workflow or task that generated this sample.
    pub source_id: String,
    /// The measured value.
    pub value: f64,
}

impl MetricSample {
    /// Creates a new metric sample.
    #[must_use]
    pub fn new(metric: WorkflowMetric, source_id: impl Into<String>, value: f64) -> Self {
        Self {
            metric,
            source_id: source_id.into(),
            value,
        }
    }
}

// ── Summary ───────────────────────────────────────────────────────────────────

/// Aggregated statistics for a single [`WorkflowMetric`] type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricSummary {
    /// The metric these statistics describe.
    pub metric: WorkflowMetric,
    /// Number of samples included in the summary.
    pub count: usize,
    /// Sum of all sample values.
    pub sum: f64,
    /// Minimum observed value.
    pub min: f64,
    /// Maximum observed value.
    pub max: f64,
}

impl MetricSummary {
    /// Computes the arithmetic mean, or `0.0` when `count == 0`.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn mean(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }
}

// ── Collector ─────────────────────────────────────────────────────────────────

/// Accumulates [`MetricSample`]s and produces per-metric [`MetricSummary`]s.
#[derive(Debug, Default, Clone)]
pub struct WorkflowMetricsCollector {
    samples: Vec<MetricSample>,
}

impl WorkflowMetricsCollector {
    /// Creates a new, empty collector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a single metric observation.
    pub fn record(&mut self, sample: MetricSample) {
        self.samples.push(sample);
    }

    /// Convenience method to record a value directly.
    pub fn record_value(
        &mut self,
        metric: WorkflowMetric,
        source_id: impl Into<String>,
        value: f64,
    ) {
        self.record(MetricSample::new(metric, source_id, value));
    }

    /// Total number of samples collected so far.
    #[must_use]
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Returns all samples for the given metric kind.
    #[must_use]
    pub fn samples_for(&self, metric: WorkflowMetric) -> Vec<&MetricSample> {
        self.samples.iter().filter(|s| s.metric == metric).collect()
    }

    /// Builds a [`MetricSummary`] for the given metric, or `None` if no
    /// samples of that kind have been collected.
    #[must_use]
    pub fn summarize(&self, metric: WorkflowMetric) -> Option<MetricSummary> {
        let relevant: Vec<f64> = self
            .samples
            .iter()
            .filter(|s| s.metric == metric)
            .map(|s| s.value)
            .collect();

        if relevant.is_empty() {
            return None;
        }

        let sum: f64 = relevant.iter().sum();
        let min = relevant.iter().copied().fold(f64::INFINITY, f64::min);
        let max = relevant.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        Some(MetricSummary {
            metric,
            count: relevant.len(),
            sum,
            min,
            max,
        })
    }

    /// Returns summaries for every metric kind that has at least one sample.
    #[must_use]
    pub fn all_summaries(&self) -> HashMap<WorkflowMetric, MetricSummary> {
        let mut map = HashMap::new();
        for metric in [
            WorkflowMetric::QueueWaitSeconds,
            WorkflowMetric::TaskDurationSeconds,
            WorkflowMetric::TaskMemoryBytes,
            WorkflowMetric::CpuPercent,
            WorkflowMetric::RetryCount,
            WorkflowMetric::WorkflowDurationSeconds,
        ] {
            if let Some(summary) = self.summarize(metric) {
                map.insert(metric, summary);
            }
        }
        map
    }

    /// Clears all collected samples.
    pub fn reset(&mut self) {
        self.samples.clear();
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn collector_with_samples() -> WorkflowMetricsCollector {
        let mut c = WorkflowMetricsCollector::new();
        c.record_value(WorkflowMetric::TaskDurationSeconds, "task-1", 10.0);
        c.record_value(WorkflowMetric::TaskDurationSeconds, "task-2", 20.0);
        c.record_value(WorkflowMetric::TaskDurationSeconds, "task-3", 30.0);
        c.record_value(WorkflowMetric::CpuPercent, "task-1", 55.0);
        c
    }

    #[test]
    fn test_new_collector_empty() {
        let c = WorkflowMetricsCollector::new();
        assert_eq!(c.sample_count(), 0);
    }

    #[test]
    fn test_record_value_increments_count() {
        let mut c = WorkflowMetricsCollector::new();
        c.record_value(WorkflowMetric::RetryCount, "wf-1", 2.0);
        assert_eq!(c.sample_count(), 1);
    }

    #[test]
    fn test_samples_for_filters_correctly() {
        let c = collector_with_samples();
        let dur_samples = c.samples_for(WorkflowMetric::TaskDurationSeconds);
        assert_eq!(dur_samples.len(), 3);
    }

    #[test]
    fn test_summarize_mean() {
        let c = collector_with_samples();
        let summary = c
            .summarize(WorkflowMetric::TaskDurationSeconds)
            .expect("should succeed in test");
        // (10 + 20 + 30) / 3 = 20.0
        assert!((summary.mean() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn test_summarize_min_max() {
        let c = collector_with_samples();
        let summary = c
            .summarize(WorkflowMetric::TaskDurationSeconds)
            .expect("should succeed in test");
        assert!((summary.min - 10.0).abs() < 1e-9);
        assert!((summary.max - 30.0).abs() < 1e-9);
    }

    #[test]
    fn test_summarize_count() {
        let c = collector_with_samples();
        let summary = c
            .summarize(WorkflowMetric::TaskDurationSeconds)
            .expect("should succeed in test");
        assert_eq!(summary.count, 3);
    }

    #[test]
    fn test_summarize_none_for_missing_metric() {
        let c = collector_with_samples();
        assert!(c.summarize(WorkflowMetric::QueueWaitSeconds).is_none());
    }

    #[test]
    fn test_all_summaries_keys() {
        let c = collector_with_samples();
        let summaries = c.all_summaries();
        assert!(summaries.contains_key(&WorkflowMetric::TaskDurationSeconds));
        assert!(summaries.contains_key(&WorkflowMetric::CpuPercent));
        assert!(!summaries.contains_key(&WorkflowMetric::QueueWaitSeconds));
    }

    #[test]
    fn test_reset_clears_samples() {
        let mut c = collector_with_samples();
        c.reset();
        assert_eq!(c.sample_count(), 0);
        assert!(c.summarize(WorkflowMetric::TaskDurationSeconds).is_none());
    }

    #[test]
    fn test_metric_unit() {
        assert_eq!(WorkflowMetric::TaskDurationSeconds.unit(), "s");
        assert_eq!(WorkflowMetric::TaskMemoryBytes.unit(), "bytes");
        assert_eq!(WorkflowMetric::CpuPercent.unit(), "%");
        assert_eq!(WorkflowMetric::RetryCount.unit(), "count");
    }

    #[test]
    fn test_metric_lower_is_better() {
        assert!(WorkflowMetric::TaskDurationSeconds.lower_is_better());
        assert!(WorkflowMetric::QueueWaitSeconds.lower_is_better());
        assert!(!WorkflowMetric::CpuPercent.lower_is_better());
    }

    #[test]
    fn test_metric_display() {
        assert_eq!(
            format!("{}", WorkflowMetric::TaskDurationSeconds),
            "task_duration_seconds"
        );
    }

    #[test]
    fn test_metric_summary_mean_empty() {
        let s = MetricSummary {
            metric: WorkflowMetric::RetryCount,
            count: 0,
            sum: 0.0,
            min: 0.0,
            max: 0.0,
        };
        assert!((s.mean() - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_single_sample_summary() {
        let mut c = WorkflowMetricsCollector::new();
        c.record_value(WorkflowMetric::WorkflowDurationSeconds, "wf-a", 42.0);
        let s = c
            .summarize(WorkflowMetric::WorkflowDurationSeconds)
            .expect("should succeed in test");
        assert_eq!(s.count, 1);
        assert!((s.min - 42.0).abs() < 1e-9);
        assert!((s.max - 42.0).abs() < 1e-9);
        assert!((s.mean() - 42.0).abs() < 1e-9);
    }
}
