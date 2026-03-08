#![allow(dead_code)]
//! Per-job metrics collection — outcomes, efficiency, and aggregate statistics.

use std::collections::HashMap;
use std::time::Duration;

/// The outcome of a completed job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobOutcome {
    /// Job completed successfully.
    Success,
    /// Job failed with an error.
    Failure,
    /// Job was cancelled before completion.
    Cancelled,
    /// Job timed out.
    TimedOut,
    /// Job was skipped (e.g. dependency failed).
    Skipped,
}

impl JobOutcome {
    /// Returns true for outcomes that represent successful completion.
    pub fn is_success(&self) -> bool {
        matches!(self, JobOutcome::Success)
    }

    /// Returns true for outcomes that represent a terminal failure.
    pub fn is_terminal_failure(&self) -> bool {
        matches!(self, JobOutcome::Failure | JobOutcome::TimedOut)
    }

    /// Short string label.
    pub fn label(&self) -> &'static str {
        match self {
            JobOutcome::Success => "success",
            JobOutcome::Failure => "failure",
            JobOutcome::Cancelled => "cancelled",
            JobOutcome::TimedOut => "timed_out",
            JobOutcome::Skipped => "skipped",
        }
    }
}

/// Metrics captured for a single job execution.
#[derive(Debug, Clone)]
pub struct JobMetric {
    /// Identifier for the job.
    pub job_id: String,
    /// Outcome of the job.
    pub outcome: JobOutcome,
    /// Wall-clock duration from start to finish.
    pub duration: Duration,
    /// CPU time consumed (approximate).
    pub cpu_time: Duration,
    /// Number of items processed (e.g. frames transcoded).
    pub items_processed: u64,
    /// Number of items in the total workload.
    pub items_total: u64,
    /// Worker that executed the job.
    pub worker_id: String,
    /// Number of retry attempts (0 = first try succeeded or failed).
    pub retry_count: u32,
}

impl JobMetric {
    /// Create a new metric record.
    pub fn new(
        job_id: impl Into<String>,
        outcome: JobOutcome,
        duration: Duration,
        cpu_time: Duration,
        items_processed: u64,
        items_total: u64,
        worker_id: impl Into<String>,
    ) -> Self {
        Self {
            job_id: job_id.into(),
            outcome,
            duration,
            cpu_time,
            items_processed,
            items_total,
            worker_id: worker_id.into(),
            retry_count: 0,
        }
    }

    /// Set retry count.
    pub fn with_retries(mut self, count: u32) -> Self {
        self.retry_count = count;
        self
    }

    /// Efficiency as a percentage: (items_processed / items_total) * 100.
    /// Returns 0.0 if items_total is 0.
    #[allow(clippy::cast_precision_loss)]
    pub fn efficiency_pct(&self) -> f64 {
        if self.items_total == 0 {
            return 0.0;
        }
        (self.items_processed as f64 / self.items_total as f64) * 100.0
    }

    /// CPU utilisation as a fraction of wall-clock time [0.0, ∞).
    /// Values > 1.0 indicate multi-core utilisation.
    pub fn cpu_utilization(&self) -> f64 {
        let wall = self.duration.as_secs_f64();
        if wall <= 0.0 {
            return 0.0;
        }
        self.cpu_time.as_secs_f64() / wall
    }

    /// Processing rate in items per second. 0.0 if duration is zero.
    #[allow(clippy::cast_precision_loss)]
    pub fn items_per_second(&self) -> f64 {
        let secs = self.duration.as_secs_f64();
        if secs <= 0.0 {
            return 0.0;
        }
        self.items_processed as f64 / secs
    }
}

/// Collector for multiple job metrics with aggregate query methods.
#[derive(Debug, Default, Clone)]
pub struct JobMetricsCollector {
    records: Vec<JobMetric>,
    /// Total number of successful outcomes counted.
    success_total: u64,
    /// Total number of failure outcomes counted.
    failure_total: u64,
}

impl JobMetricsCollector {
    /// Create a new empty collector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a job metric.
    pub fn record(&mut self, metric: JobMetric) {
        if metric.outcome.is_success() {
            self.success_total += 1;
        } else if metric.outcome.is_terminal_failure() {
            self.failure_total += 1;
        }
        self.records.push(metric);
    }

    /// Total number of recorded jobs.
    pub fn total_recorded(&self) -> usize {
        self.records.len()
    }

    /// Number of successful jobs.
    pub fn success_count(&self) -> usize {
        self.records
            .iter()
            .filter(|m| m.outcome.is_success())
            .count()
    }

    /// Success rate as a fraction [0.0, 1.0]. Returns 0.0 if no records.
    #[allow(clippy::cast_precision_loss)]
    pub fn success_rate(&self) -> f64 {
        if self.records.is_empty() {
            return 0.0;
        }
        self.success_count() as f64 / self.records.len() as f64
    }

    /// Average duration in milliseconds across all records. 0.0 if empty.
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_duration_ms(&self) -> f64 {
        if self.records.is_empty() {
            return 0.0;
        }
        let total_ms: f64 = self
            .records
            .iter()
            .map(|m| m.duration.as_secs_f64() * 1000.0)
            .sum();
        total_ms / self.records.len() as f64
    }

    /// Maximum duration across all records.
    pub fn max_duration(&self) -> Option<Duration> {
        self.records.iter().map(|m| m.duration).max()
    }

    /// Minimum duration across all records.
    pub fn min_duration(&self) -> Option<Duration> {
        self.records.iter().map(|m| m.duration).min()
    }

    /// Average efficiency percentage across all records.
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_efficiency_pct(&self) -> f64 {
        if self.records.is_empty() {
            return 0.0;
        }
        let total: f64 = self.records.iter().map(|m| m.efficiency_pct()).sum();
        total / self.records.len() as f64
    }

    /// Count records by outcome.
    pub fn outcome_counts(&self) -> HashMap<JobOutcome, usize> {
        let mut map = HashMap::new();
        for m in &self.records {
            *map.entry(m.outcome).or_insert(0) += 1;
        }
        map
    }

    /// Metrics for jobs that ran on a specific worker.
    pub fn records_for_worker(&self, worker_id: &str) -> Vec<&JobMetric> {
        self.records
            .iter()
            .filter(|m| m.worker_id == worker_id)
            .collect()
    }

    /// Clear all records.
    pub fn clear(&mut self) {
        self.records.clear();
        self.success_total = 0;
        self.failure_total = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn metric(id: &str, outcome: JobOutcome, dur_ms: u64, processed: u64, total: u64) -> JobMetric {
        JobMetric::new(
            id,
            outcome,
            Duration::from_millis(dur_ms),
            Duration::from_millis(dur_ms / 2),
            processed,
            total,
            "worker-1",
        )
    }

    #[test]
    fn test_job_outcome_is_success() {
        assert!(JobOutcome::Success.is_success());
        assert!(!JobOutcome::Failure.is_success());
        assert!(!JobOutcome::Cancelled.is_success());
    }

    #[test]
    fn test_job_outcome_is_terminal_failure() {
        assert!(JobOutcome::Failure.is_terminal_failure());
        assert!(JobOutcome::TimedOut.is_terminal_failure());
        assert!(!JobOutcome::Success.is_terminal_failure());
        assert!(!JobOutcome::Cancelled.is_terminal_failure());
    }

    #[test]
    fn test_job_outcome_label() {
        assert_eq!(JobOutcome::Success.label(), "success");
        assert_eq!(JobOutcome::TimedOut.label(), "timed_out");
    }

    #[test]
    fn test_efficiency_pct_full() {
        let m = metric("j1", JobOutcome::Success, 1000, 100, 100);
        assert!((m.efficiency_pct() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_efficiency_pct_partial() {
        let m = metric("j2", JobOutcome::Failure, 500, 50, 100);
        assert!((m.efficiency_pct() - 50.0).abs() < 1e-9);
    }

    #[test]
    fn test_efficiency_pct_zero_total() {
        let m = metric("j3", JobOutcome::Success, 100, 0, 0);
        assert_eq!(m.efficiency_pct(), 0.0);
    }

    #[test]
    fn test_items_per_second() {
        // 1000 items in 1 second
        let m = metric("j4", JobOutcome::Success, 1000, 1000, 1000);
        let ips = m.items_per_second();
        assert!((ips - 1000.0).abs() < 1.0, "expected ~1000 ips, got {ips}");
    }

    #[test]
    fn test_collector_empty() {
        let c = JobMetricsCollector::new();
        assert_eq!(c.total_recorded(), 0);
        assert_eq!(c.success_rate(), 0.0);
        assert_eq!(c.avg_duration_ms(), 0.0);
    }

    #[test]
    fn test_collector_record_and_success_rate() {
        let mut c = JobMetricsCollector::new();
        c.record(metric("a", JobOutcome::Success, 100, 10, 10));
        c.record(metric("b", JobOutcome::Success, 200, 10, 10));
        c.record(metric("c", JobOutcome::Failure, 150, 5, 10));
        assert_eq!(c.total_recorded(), 3);
        assert_eq!(c.success_count(), 2);
        let rate = c.success_rate();
        assert!((rate - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn test_collector_avg_duration_ms() {
        let mut c = JobMetricsCollector::new();
        c.record(metric("a", JobOutcome::Success, 100, 1, 1));
        c.record(metric("b", JobOutcome::Success, 300, 1, 1));
        let avg = c.avg_duration_ms();
        assert!((avg - 200.0).abs() < 1e-6, "expected 200ms avg, got {avg}");
    }

    #[test]
    fn test_collector_max_min_duration() {
        let mut c = JobMetricsCollector::new();
        c.record(metric("a", JobOutcome::Success, 100, 1, 1));
        c.record(metric("b", JobOutcome::Success, 500, 1, 1));
        assert_eq!(c.max_duration(), Some(Duration::from_millis(500)));
        assert_eq!(c.min_duration(), Some(Duration::from_millis(100)));
    }

    #[test]
    fn test_collector_outcome_counts() {
        let mut c = JobMetricsCollector::new();
        c.record(metric("a", JobOutcome::Success, 100, 1, 1));
        c.record(metric("b", JobOutcome::Failure, 100, 0, 1));
        c.record(metric("c", JobOutcome::Cancelled, 10, 0, 1));
        let counts = c.outcome_counts();
        assert_eq!(counts[&JobOutcome::Success], 1);
        assert_eq!(counts[&JobOutcome::Failure], 1);
        assert_eq!(counts[&JobOutcome::Cancelled], 1);
    }

    #[test]
    fn test_records_for_worker() {
        let mut c = JobMetricsCollector::new();
        let mut m2 = metric("b", JobOutcome::Success, 200, 10, 10);
        m2.worker_id = "worker-2".to_string();
        c.record(metric("a", JobOutcome::Success, 100, 10, 10)); // worker-1
        c.record(m2); // worker-2
        assert_eq!(c.records_for_worker("worker-1").len(), 1);
        assert_eq!(c.records_for_worker("worker-2").len(), 1);
        assert_eq!(c.records_for_worker("worker-3").len(), 0);
    }

    #[test]
    fn test_collector_clear() {
        let mut c = JobMetricsCollector::new();
        c.record(metric("a", JobOutcome::Success, 100, 1, 1));
        c.clear();
        assert_eq!(c.total_recorded(), 0);
        assert_eq!(c.success_rate(), 0.0);
    }
}
