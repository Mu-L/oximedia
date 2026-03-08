//! Health check framework for monitoring system components.
//!
//! Provides structured health checks with aggregation and reporting.

/// Status of a health check.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)]
pub enum HealthStatus {
    /// Component is healthy.
    Healthy,
    /// Component is degraded but still operational.
    Degraded,
    /// Component is unhealthy / not functioning.
    Unhealthy,
    /// Health could not be determined.
    Unknown,
}

impl HealthStatus {
    /// Returns `true` for `Healthy` only.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// Returns a numeric priority for aggregation (higher = worse).
    #[must_use]
    pub fn priority(&self) -> u32 {
        match self {
            Self::Healthy => 0,
            Self::Degraded => 1,
            Self::Unhealthy => 2,
            Self::Unknown => 3,
        }
    }
}

/// Result of a single health check.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CheckResult {
    /// Name of the check.
    pub name: String,
    /// Determined health status.
    pub status: HealthStatus,
    /// Human-readable message.
    pub message: String,
    /// How long the check took in milliseconds.
    pub elapsed_ms: u32,
}

impl CheckResult {
    /// Returns `true` if the result is `Healthy`.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.status.is_ok()
    }
}

/// Performs health checks for a named component.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HealthChecker {
    /// Component name.
    pub name: String,
    /// Maximum allowed latency before reporting `Unhealthy`.
    pub timeout_ms: u32,
}

impl HealthChecker {
    /// Create a new checker.
    pub fn new(name: impl Into<String>, timeout_ms: u32) -> Self {
        Self {
            name: name.into(),
            timeout_ms,
        }
    }

    /// Evaluate the health of a component based on observed latency.
    ///
    /// * `< 50 %` of timeout → `Healthy`
    /// * `< 100 %` of timeout → `Degraded`
    /// * `>= timeout` → `Unhealthy`
    #[must_use]
    pub fn check_latency(&self, latency_ms: u32) -> CheckResult {
        let half = self.timeout_ms / 2;
        let (status, message) = if latency_ms < half {
            (
                HealthStatus::Healthy,
                format!("Latency {latency_ms}ms is within normal range"),
            )
        } else if latency_ms < self.timeout_ms {
            (
                HealthStatus::Degraded,
                format!(
                    "Latency {}ms is elevated (timeout {}ms)",
                    latency_ms, self.timeout_ms
                ),
            )
        } else {
            (
                HealthStatus::Unhealthy,
                format!(
                    "Latency {}ms exceeds timeout {}ms",
                    latency_ms, self.timeout_ms
                ),
            )
        };

        CheckResult {
            name: self.name.clone(),
            status,
            message,
            elapsed_ms: latency_ms,
        }
    }

    /// Evaluate the health of a component based on its error rate.
    ///
    /// * `rate <= max_rate` → `Healthy`
    /// * `rate <= max_rate * 2` → `Degraded`
    /// * otherwise → `Unhealthy`
    #[must_use]
    pub fn check_error_rate(&self, rate: f32, max_rate: f32) -> CheckResult {
        let (status, message) = if rate <= max_rate {
            (
                HealthStatus::Healthy,
                format!("Error rate {rate:.2}% is within limit {max_rate:.2}%"),
            )
        } else if rate <= max_rate * 2.0 {
            (
                HealthStatus::Degraded,
                format!("Error rate {rate:.2}% exceeds limit {max_rate:.2}%"),
            )
        } else {
            (
                HealthStatus::Unhealthy,
                format!("Error rate {rate:.2}% far exceeds limit {max_rate:.2}%"),
            )
        };

        CheckResult {
            name: self.name.clone(),
            status,
            message,
            elapsed_ms: 0,
        }
    }
}

/// Aggregated health report from multiple checks.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HealthReport {
    /// Individual check results.
    pub checks: Vec<CheckResult>,
    /// Worst status across all checks.
    pub overall: HealthStatus,
}

impl HealthReport {
    /// Aggregate multiple check results into a report.
    /// The overall status is the worst (highest priority) status seen.
    #[must_use]
    pub fn aggregate(checks: Vec<CheckResult>) -> Self {
        let overall = checks
            .iter()
            .max_by_key(|c| c.status.priority())
            .map_or(HealthStatus::Unknown, |c| c.status.clone());

        Self { checks, overall }
    }

    /// Count of checks with `Healthy` status.
    #[must_use]
    pub fn healthy_count(&self) -> usize {
        self.checks.iter().filter(|c| c.is_healthy()).count()
    }

    /// Count of checks whose status is not `Healthy`.
    #[must_use]
    pub fn unhealthy_count(&self) -> usize {
        self.checks.len() - self.healthy_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------ //
    // HealthStatus
    // ------------------------------------------------------------------ //

    #[test]
    fn test_status_is_ok_healthy_only() {
        assert!(HealthStatus::Healthy.is_ok());
        assert!(!HealthStatus::Degraded.is_ok());
        assert!(!HealthStatus::Unhealthy.is_ok());
        assert!(!HealthStatus::Unknown.is_ok());
    }

    #[test]
    fn test_status_priority_ordering() {
        assert!(HealthStatus::Healthy.priority() < HealthStatus::Degraded.priority());
        assert!(HealthStatus::Degraded.priority() < HealthStatus::Unhealthy.priority());
        assert!(HealthStatus::Unhealthy.priority() < HealthStatus::Unknown.priority());
    }

    // ------------------------------------------------------------------ //
    // HealthChecker – latency
    // ------------------------------------------------------------------ //

    #[test]
    fn test_latency_check_healthy() {
        let checker = HealthChecker::new("db", 200);
        let result = checker.check_latency(50);
        assert_eq!(result.status, HealthStatus::Healthy);
    }

    #[test]
    fn test_latency_check_degraded() {
        let checker = HealthChecker::new("db", 200);
        // half = 100, 150 is between half and timeout
        let result = checker.check_latency(150);
        assert_eq!(result.status, HealthStatus::Degraded);
    }

    #[test]
    fn test_latency_check_unhealthy() {
        let checker = HealthChecker::new("db", 200);
        let result = checker.check_latency(250);
        assert_eq!(result.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_latency_check_elapsed_ms_preserved() {
        let checker = HealthChecker::new("db", 1000);
        let result = checker.check_latency(42);
        assert_eq!(result.elapsed_ms, 42);
    }

    // ------------------------------------------------------------------ //
    // HealthChecker – error rate
    // ------------------------------------------------------------------ //

    #[test]
    fn test_error_rate_healthy() {
        let checker = HealthChecker::new("api", 1000);
        let result = checker.check_error_rate(0.5, 1.0);
        assert_eq!(result.status, HealthStatus::Healthy);
    }

    #[test]
    fn test_error_rate_degraded() {
        let checker = HealthChecker::new("api", 1000);
        // rate=1.5 is between max_rate=1.0 and max_rate*2=2.0
        let result = checker.check_error_rate(1.5, 1.0);
        assert_eq!(result.status, HealthStatus::Degraded);
    }

    #[test]
    fn test_error_rate_unhealthy() {
        let checker = HealthChecker::new("api", 1000);
        let result = checker.check_error_rate(5.0, 1.0);
        assert_eq!(result.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_check_result_name_matches_checker() {
        let checker = HealthChecker::new("my-service", 500);
        let result = checker.check_latency(10);
        assert_eq!(result.name, "my-service");
    }

    // ------------------------------------------------------------------ //
    // HealthReport
    // ------------------------------------------------------------------ //

    #[test]
    fn test_report_aggregate_worst_wins() {
        let checks = vec![
            CheckResult {
                name: "a".into(),
                status: HealthStatus::Healthy,
                message: String::new(),
                elapsed_ms: 0,
            },
            CheckResult {
                name: "b".into(),
                status: HealthStatus::Unhealthy,
                message: String::new(),
                elapsed_ms: 0,
            },
        ];
        let report = HealthReport::aggregate(checks);
        assert_eq!(report.overall, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_report_healthy_count() {
        let checker = HealthChecker::new("svc", 200);
        let checks = vec![checker.check_latency(10), checker.check_latency(250)];
        let report = HealthReport::aggregate(checks);
        assert_eq!(report.healthy_count(), 1);
        assert_eq!(report.unhealthy_count(), 1);
    }

    #[test]
    fn test_report_all_healthy() {
        let checker = HealthChecker::new("svc", 1000);
        let checks = vec![
            checker.check_latency(1),
            checker.check_latency(2),
            checker.check_latency(3),
        ];
        let report = HealthReport::aggregate(checks);
        assert_eq!(report.overall, HealthStatus::Healthy);
        assert_eq!(report.unhealthy_count(), 0);
    }

    #[test]
    fn test_report_empty_checks_is_unknown() {
        let report = HealthReport::aggregate(vec![]);
        assert_eq!(report.overall, HealthStatus::Unknown);
    }
}
