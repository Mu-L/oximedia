//! Bottleneck detection.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A detected performance bottleneck.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bottleneck {
    /// Description of the bottleneck.
    pub description: String,

    /// Location (function/module).
    pub location: String,

    /// Time impact.
    pub time_impact: Duration,

    /// Impact percentage.
    pub impact_percentage: f64,

    /// Severity (0.0-1.0).
    pub severity: f64,

    /// Suggested optimization.
    pub suggestion: Option<String>,
}

impl Bottleneck {
    /// Create a new bottleneck.
    pub fn new(description: String, location: String, time_impact: Duration) -> Self {
        Self {
            description,
            location,
            time_impact,
            impact_percentage: 0.0,
            severity: 0.0,
            suggestion: None,
        }
    }

    /// Set the impact percentage and severity.
    pub fn with_impact(mut self, percentage: f64) -> Self {
        self.impact_percentage = percentage;
        self.severity = (percentage / 100.0).min(1.0);
        self
    }

    /// Add an optimization suggestion.
    pub fn with_suggestion(mut self, suggestion: String) -> Self {
        self.suggestion = Some(suggestion);
        self
    }

    /// Check if this is a critical bottleneck.
    pub fn is_critical(&self) -> bool {
        self.severity > 0.7
    }

    /// Check if this is a significant bottleneck.
    pub fn is_significant(&self) -> bool {
        self.severity > 0.4
    }
}

/// Bottleneck detector.
#[derive(Debug)]
pub struct BottleneckDetector {
    threshold: f64,
    bottlenecks: Vec<Bottleneck>,
}

impl BottleneckDetector {
    /// Create a new bottleneck detector.
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold,
            bottlenecks: Vec::new(),
        }
    }

    /// Add a potential bottleneck.
    pub fn add_bottleneck(&mut self, bottleneck: Bottleneck) {
        if bottleneck.impact_percentage >= self.threshold {
            self.bottlenecks.push(bottleneck);
        }
    }

    /// Detect bottlenecks from timing data.
    pub fn detect(&mut self, timings: &[(String, Duration)], total_time: Duration) {
        self.bottlenecks.clear();

        for (location, time) in timings {
            let percentage = if total_time.as_secs_f64() > 0.0 {
                (time.as_secs_f64() / total_time.as_secs_f64()) * 100.0
            } else {
                0.0
            };

            if percentage >= self.threshold {
                let bottleneck = Bottleneck::new(
                    format!("High execution time in {}", location),
                    location.clone(),
                    *time,
                )
                .with_impact(percentage);

                self.bottlenecks.push(bottleneck);
            }
        }

        self.bottlenecks
            .sort_by(|a, b| b.severity.total_cmp(&a.severity));
    }

    /// Get detected bottlenecks.
    pub fn bottlenecks(&self) -> &[Bottleneck] {
        &self.bottlenecks
    }

    /// Get critical bottlenecks.
    pub fn critical_bottlenecks(&self) -> Vec<&Bottleneck> {
        self.bottlenecks
            .iter()
            .filter(|b| b.is_critical())
            .collect()
    }

    /// Clear all bottlenecks.
    pub fn clear(&mut self) {
        self.bottlenecks.clear();
    }
}

impl Default for BottleneckDetector {
    fn default() -> Self {
        Self::new(5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bottleneck() {
        let bottleneck = Bottleneck::new(
            "Test bottleneck".to_string(),
            "test_function".to_string(),
            Duration::from_secs(1),
        )
        .with_impact(75.0);

        assert!(bottleneck.is_critical());
        assert!(bottleneck.is_significant());
        assert_eq!(bottleneck.impact_percentage, 75.0);
    }

    #[test]
    fn test_bottleneck_detector() {
        let mut detector = BottleneckDetector::new(10.0);

        let timings = vec![
            ("func1".to_string(), Duration::from_millis(500)),
            ("func2".to_string(), Duration::from_millis(300)),
            ("func3".to_string(), Duration::from_millis(200)),
        ];

        detector.detect(&timings, Duration::from_secs(1));

        assert!(!detector.bottlenecks().is_empty());
    }

    #[test]
    fn test_critical_bottlenecks() {
        let mut detector = BottleneckDetector::new(1.0);

        let timings = vec![
            ("func1".to_string(), Duration::from_millis(800)),
            ("func2".to_string(), Duration::from_millis(100)),
        ];

        detector.detect(&timings, Duration::from_secs(1));

        let critical = detector.critical_bottlenecks();
        assert_eq!(critical.len(), 1);
        assert_eq!(critical[0].location, "func1");
    }
}
