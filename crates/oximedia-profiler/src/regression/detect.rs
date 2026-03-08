//! Performance regression detection.

use crate::benchmark::runner::BenchmarkResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Regression information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionInfo {
    /// Benchmark name.
    pub name: String,

    /// Baseline time.
    pub baseline: Duration,

    /// Current time.
    pub current: Duration,

    /// Regression percentage.
    pub regression_percent: f64,

    /// Number of standard deviations from baseline.
    pub std_deviations: f64,

    /// Whether this is statistically significant.
    pub is_significant: bool,
}

/// Regression detector.
#[derive(Debug)]
pub struct RegressionDetector {
    baselines: HashMap<String, BenchmarkResult>,
    threshold_percent: f64,
    std_dev_threshold: f64,
}

impl RegressionDetector {
    /// Create a new regression detector.
    pub fn new(threshold_percent: f64, std_dev_threshold: f64) -> Self {
        Self {
            baselines: HashMap::new(),
            threshold_percent,
            std_dev_threshold,
        }
    }

    /// Set baseline for a benchmark.
    pub fn set_baseline(&mut self, name: String, result: BenchmarkResult) {
        self.baselines.insert(name, result);
    }

    /// Detect regression compared to baseline.
    pub fn detect(&self, current: &BenchmarkResult) -> Option<RegressionInfo> {
        let baseline = self.baselines.get(&current.name)?;

        let baseline_secs = baseline.mean.as_secs_f64();
        let current_secs = current.mean.as_secs_f64();

        let regression_percent = ((current_secs - baseline_secs) / baseline_secs) * 100.0;

        if regression_percent < self.threshold_percent {
            return None; // No regression
        }

        let std_dev_secs = baseline.std_dev.as_secs_f64();
        let std_deviations = if std_dev_secs > 0.0 {
            (current_secs - baseline_secs) / std_dev_secs
        } else {
            0.0
        };

        let is_significant = std_deviations.abs() >= self.std_dev_threshold;

        Some(RegressionInfo {
            name: current.name.clone(),
            baseline: baseline.mean,
            current: current.mean,
            regression_percent,
            std_deviations,
            is_significant,
        })
    }

    /// Detect all regressions in a set of results.
    pub fn detect_all(&self, results: &[BenchmarkResult]) -> Vec<RegressionInfo> {
        results
            .iter()
            .filter_map(|result| self.detect(result))
            .collect()
    }

    /// Get baseline count.
    pub fn baseline_count(&self) -> usize {
        self.baselines.len()
    }

    /// Generate a report.
    pub fn report(&self, regressions: &[RegressionInfo]) -> String {
        let mut report = String::new();

        if regressions.is_empty() {
            report.push_str("No performance regressions detected.\n");
        } else {
            report.push_str(&format!(
                "Performance Regressions Detected: {}\n\n",
                regressions.len()
            ));

            for regression in regressions {
                let significance = if regression.is_significant {
                    "SIGNIFICANT"
                } else {
                    "MINOR"
                };

                report.push_str(&format!("[{}] {}\n", significance, regression.name));
                report.push_str(&format!("  Baseline: {:?}\n", regression.baseline));
                report.push_str(&format!("  Current:  {:?}\n", regression.current));
                report.push_str(&format!(
                    "  Regression: {:.2}%\n",
                    regression.regression_percent
                ));
                report.push_str(&format!(
                    "  Std Deviations: {:.2}\n\n",
                    regression.std_deviations
                ));
            }
        }

        report
    }
}

impl Default for RegressionDetector {
    fn default() -> Self {
        Self::new(5.0, 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(name: &str, mean_ms: u64, std_dev_ms: u64) -> BenchmarkResult {
        BenchmarkResult {
            name: name.to_string(),
            iterations: 100,
            mean: Duration::from_millis(mean_ms),
            median: Duration::from_millis(mean_ms),
            std_dev: Duration::from_millis(std_dev_ms),
            min: Duration::from_millis(mean_ms),
            max: Duration::from_millis(mean_ms),
            throughput: 1000.0 / mean_ms as f64,
        }
    }

    #[test]
    fn test_regression_detector() {
        let mut detector = RegressionDetector::new(5.0, 2.0);
        let baseline = make_result("test", 100, 5);
        detector.set_baseline("test".to_string(), baseline);

        let current = make_result("test", 120, 5);
        let regression = detector.detect(&current);

        assert!(regression.is_some());
        let reg = regression.expect("should succeed in test");
        assert!((reg.regression_percent - 20.0).abs() < 0.0001);
    }

    #[test]
    fn test_no_regression() {
        let mut detector = RegressionDetector::new(10.0, 2.0);
        let baseline = make_result("test", 100, 5);
        detector.set_baseline("test".to_string(), baseline);

        let current = make_result("test", 105, 5);
        let regression = detector.detect(&current);

        assert!(regression.is_none());
    }

    #[test]
    fn test_significance() {
        let mut detector = RegressionDetector::new(5.0, 2.0);
        let baseline = make_result("test", 100, 5);
        detector.set_baseline("test".to_string(), baseline);

        let current = make_result("test", 120, 5);
        let regression = detector.detect(&current).expect("should succeed in test");

        assert!(regression.is_significant);
    }
}
