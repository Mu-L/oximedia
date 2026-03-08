//! Statistical sampling profiler for low-overhead CPU profiling.
//!
//! Records periodic stack snapshots and aggregates them into a call-frequency
//! histogram without requiring code instrumentation.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A single sample event captured by the sampling profiler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleEvent {
    /// Wall-clock timestamp of the sample (nanoseconds since epoch).
    pub timestamp_ns: u64,
    /// Symbolic call stack at sample time (outermost first).
    pub stack: Vec<String>,
    /// Thread identifier that was sampled.
    pub thread_id: u64,
    /// CPU time consumed by the thread at sample time (µs).
    pub cpu_time_us: u64,
}

impl SampleEvent {
    /// Creates a new sample event.
    #[must_use]
    pub fn new(timestamp_ns: u64, stack: Vec<String>, thread_id: u64, cpu_time_us: u64) -> Self {
        Self {
            timestamp_ns,
            stack,
            thread_id,
            cpu_time_us,
        }
    }

    /// Returns the top-of-stack function name, if any.
    #[must_use]
    pub fn top_frame(&self) -> Option<&str> {
        self.stack.last().map(String::as_str)
    }

    /// Returns the depth of the recorded stack.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}

/// Configuration for the sampling profiler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingConfig {
    /// Number of samples to collect per second.
    pub sample_rate_hz: u32,
    /// Maximum stack depth to record per sample.
    pub max_stack_depth: usize,
    /// Collect samples across all threads (`true`) or only the calling thread.
    pub all_threads: bool,
    /// Minimum CPU utilisation (0.0–1.0) to start sampling.
    pub min_cpu_threshold: f64,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            sample_rate_hz: 100,
            max_stack_depth: 64,
            all_threads: true,
            min_cpu_threshold: 0.0,
        }
    }
}

impl SamplingConfig {
    /// Creates a high-frequency sampling configuration (1 kHz).
    #[must_use]
    pub fn high_frequency() -> Self {
        Self {
            sample_rate_hz: 1_000,
            max_stack_depth: 32,
            all_threads: true,
            min_cpu_threshold: 0.0,
        }
    }

    /// Creates a low-overhead sampling configuration (10 Hz).
    #[must_use]
    pub fn low_overhead() -> Self {
        Self {
            sample_rate_hz: 10,
            max_stack_depth: 128,
            all_threads: true,
            min_cpu_threshold: 0.05,
        }
    }

    /// Returns the inter-sample interval as a `Duration`.
    #[must_use]
    pub fn sample_interval(&self) -> Duration {
        if self.sample_rate_hz == 0 {
            Duration::from_secs(1)
        } else {
            Duration::from_nanos(1_000_000_000 / u64::from(self.sample_rate_hz))
        }
    }
}

/// Statistical sampling profiler.
///
/// Collects `SampleEvent`s at a configurable rate and provides aggregated
/// call-frequency statistics without source-level instrumentation.
#[derive(Debug)]
pub struct SamplingProfiler {
    config: SamplingConfig,
    samples: Vec<SampleEvent>,
    running: bool,
    start_time: Option<Instant>,
    /// Per-function hit counts accumulated from all samples.
    hit_counts: HashMap<String, u64>,
}

impl SamplingProfiler {
    /// Creates a new `SamplingProfiler` with the provided configuration.
    #[must_use]
    pub fn new(config: SamplingConfig) -> Self {
        Self {
            config,
            samples: Vec::new(),
            running: false,
            start_time: None,
            hit_counts: HashMap::new(),
        }
    }

    /// Creates a `SamplingProfiler` with default configuration.
    #[must_use]
    pub fn default_config() -> Self {
        Self::new(SamplingConfig::default())
    }

    /// Returns the configured sample rate in Hz.
    #[must_use]
    pub fn sample_rate_hz(&self) -> u32 {
        self.config.sample_rate_hz
    }

    /// Returns the inter-sample interval.
    #[must_use]
    pub fn sample_interval(&self) -> Duration {
        self.config.sample_interval()
    }

    /// Starts the profiler session.
    pub fn start(&mut self) {
        self.running = true;
        self.start_time = Some(Instant::now());
        self.samples.clear();
        self.hit_counts.clear();
    }

    /// Stops the profiler session.
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Returns `true` if the profiler is currently running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Records a pre-built `SampleEvent`.
    ///
    /// Updates the internal hit-count histogram for every frame in the event's
    /// stack.  Truncates the stack to [`SamplingConfig::max_stack_depth`].
    pub fn record(&mut self, mut event: SampleEvent) {
        event.stack.truncate(self.config.max_stack_depth);
        for frame in &event.stack {
            *self.hit_counts.entry(frame.clone()).or_insert(0) += 1;
        }
        self.samples.push(event);
    }

    /// Returns a reference to all recorded samples.
    #[must_use]
    pub fn samples(&self) -> &[SampleEvent] {
        &self.samples
    }

    /// Returns the total number of samples collected.
    #[must_use]
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Returns the elapsed profiling duration, if started.
    #[must_use]
    pub fn elapsed(&self) -> Option<Duration> {
        self.start_time.map(|t| t.elapsed())
    }

    /// Returns the hit count for a specific function name.
    #[must_use]
    pub fn hit_count(&self, function_name: &str) -> u64 {
        self.hit_counts.get(function_name).copied().unwrap_or(0)
    }

    /// Returns the top N hottest functions sorted by descending hit count.
    #[must_use]
    pub fn top_functions(&self, n: usize) -> Vec<(&str, u64)> {
        let mut entries: Vec<(&str, u64)> = self
            .hit_counts
            .iter()
            .map(|(k, &v)| (k.as_str(), v))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(n);
        entries
    }

    /// Returns the estimated sample rate actually achieved (samples/sec).
    ///
    /// Returns `None` if no samples have been recorded or the profiler has not
    /// been started yet.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn achieved_rate(&self) -> Option<f64> {
        let elapsed = self.elapsed()?;
        let secs = elapsed.as_secs_f64();
        if secs <= 0.0 || self.samples.is_empty() {
            return None;
        }
        Some(self.samples.len() as f64 / secs)
    }

    /// Returns the configuration used by this profiler.
    #[must_use]
    pub fn config(&self) -> &SamplingConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(stack: &[&str]) -> SampleEvent {
        SampleEvent::new(0, stack.iter().map(|s| s.to_string()).collect(), 1, 0)
    }

    #[test]
    fn test_default_sample_rate() {
        let p = SamplingProfiler::default_config();
        assert_eq!(p.sample_rate_hz(), 100);
    }

    #[test]
    fn test_high_frequency_config() {
        let cfg = SamplingConfig::high_frequency();
        assert_eq!(cfg.sample_rate_hz, 1_000);
    }

    #[test]
    fn test_low_overhead_config() {
        let cfg = SamplingConfig::low_overhead();
        assert_eq!(cfg.sample_rate_hz, 10);
    }

    #[test]
    fn test_sample_interval_100hz() {
        let cfg = SamplingConfig::default();
        assert_eq!(cfg.sample_interval(), Duration::from_nanos(10_000_000));
    }

    #[test]
    fn test_sample_interval_zero_rate() {
        let cfg = SamplingConfig {
            sample_rate_hz: 0,
            ..Default::default()
        };
        assert_eq!(cfg.sample_interval(), Duration::from_secs(1));
    }

    #[test]
    fn test_start_stop() {
        let mut p = SamplingProfiler::default_config();
        assert!(!p.is_running());
        p.start();
        assert!(p.is_running());
        p.stop();
        assert!(!p.is_running());
    }

    #[test]
    fn test_record_and_count() {
        let mut p = SamplingProfiler::default_config();
        p.start();
        p.record(make_event(&["main", "render", "encode"]));
        p.record(make_event(&["main", "render"]));
        p.stop();
        assert_eq!(p.sample_count(), 2);
        assert_eq!(p.hit_count("main"), 2);
        assert_eq!(p.hit_count("render"), 2);
        assert_eq!(p.hit_count("encode"), 1);
    }

    #[test]
    fn test_hit_count_missing_function() {
        let p = SamplingProfiler::default_config();
        assert_eq!(p.hit_count("nonexistent"), 0);
    }

    #[test]
    fn test_top_functions_ordering() {
        let mut p = SamplingProfiler::default_config();
        p.start();
        for _ in 0..5 {
            p.record(make_event(&["hot"]));
        }
        for _ in 0..2 {
            p.record(make_event(&["warm"]));
        }
        p.record(make_event(&["cold"]));
        p.stop();
        let top = p.top_functions(2);
        assert_eq!(top[0].0, "hot");
        assert_eq!(top[0].1, 5);
        assert_eq!(top[1].0, "warm");
    }

    #[test]
    fn test_stack_depth_truncation() {
        let cfg = SamplingConfig {
            max_stack_depth: 3,
            ..Default::default()
        };
        let mut p = SamplingProfiler::new(cfg);
        p.start();
        p.record(make_event(&["a", "b", "c", "d", "e"]));
        p.stop();
        assert_eq!(p.samples()[0].stack.len(), 3);
    }

    #[test]
    fn test_elapsed_none_before_start() {
        let p = SamplingProfiler::default_config();
        assert!(p.elapsed().is_none());
    }

    #[test]
    fn test_elapsed_some_after_start() {
        let mut p = SamplingProfiler::default_config();
        p.start();
        std::thread::sleep(Duration::from_millis(5));
        assert!(p.elapsed().is_some());
        p.stop();
    }

    #[test]
    fn test_achieved_rate_none_no_samples() {
        let mut p = SamplingProfiler::default_config();
        p.start();
        assert!(p.achieved_rate().is_none());
        p.stop();
    }

    #[test]
    fn test_sample_event_top_frame() {
        let e = make_event(&["main", "render", "encode"]);
        assert_eq!(e.top_frame(), Some("encode"));
    }

    #[test]
    fn test_sample_event_depth() {
        let e = make_event(&["a", "b", "c"]);
        assert_eq!(e.depth(), 3);
    }

    #[test]
    fn test_config_accessor() {
        let cfg = SamplingConfig::high_frequency();
        let p = SamplingProfiler::new(cfg.clone());
        assert_eq!(p.config().sample_rate_hz, cfg.sample_rate_hz);
    }
}
