//! Health check and readiness monitoring for the OxiMedia server.
//!
//! Provides liveness/readiness probes, dependency health checks,
//! and structured JSON response formatting for orchestrators such as
//! Kubernetes, Consul, and load balancers.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Outcome of a single health check
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    /// The component is healthy
    Healthy,
    /// The component is degraded but still functional
    Degraded,
    /// The component is unhealthy
    Unhealthy,
    /// The check timed out
    Timeout,
    /// The check was skipped (e.g., optional dependency not configured)
    Skipped,
}

impl CheckStatus {
    /// Returns the HTTP status code appropriate for this check outcome
    pub fn http_status(&self) -> u16 {
        match self {
            CheckStatus::Healthy | CheckStatus::Skipped | CheckStatus::Degraded => 200,
            CheckStatus::Unhealthy | CheckStatus::Timeout => 503,
        }
    }

    /// Returns the display label for this status
    pub fn label(&self) -> &'static str {
        match self {
            CheckStatus::Healthy => "healthy",
            CheckStatus::Degraded => "degraded",
            CheckStatus::Unhealthy => "unhealthy",
            CheckStatus::Timeout => "timeout",
            CheckStatus::Skipped => "skipped",
        }
    }

    /// Returns true if this status is considered a passing check
    pub fn is_passing(&self) -> bool {
        matches!(
            self,
            CheckStatus::Healthy | CheckStatus::Degraded | CheckStatus::Skipped
        )
    }
}

/// Result of an individual dependency check
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// Name of the component or dependency
    pub name: String,
    /// Health status
    pub status: CheckStatus,
    /// Optional detail message
    pub message: Option<String>,
    /// Latency of the check
    pub latency: Duration,
    /// Arbitrary metadata (e.g., version, connection count)
    pub metadata: HashMap<String, String>,
}

impl CheckResult {
    /// Creates a healthy result with no message
    pub fn healthy(name: impl Into<String>, latency: Duration) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Healthy,
            message: None,
            latency,
            metadata: HashMap::new(),
        }
    }

    /// Creates an unhealthy result with an error message
    pub fn unhealthy(
        name: impl Into<String>,
        latency: Duration,
        message: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Unhealthy,
            message: Some(message.into()),
            latency,
            metadata: HashMap::new(),
        }
    }

    /// Creates a degraded result
    pub fn degraded(
        name: impl Into<String>,
        latency: Duration,
        message: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Degraded,
            message: Some(message.into()),
            latency,
            metadata: HashMap::new(),
        }
    }

    /// Creates a skipped result
    pub fn skipped(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Skipped,
            message: None,
            latency: Duration::ZERO,
            metadata: HashMap::new(),
        }
    }

    /// Attaches metadata to the result
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// Aggregated health response
#[derive(Debug, Clone)]
pub struct HealthResponse {
    /// Overall status derived from all checks
    pub status: CheckStatus,
    /// Server version string
    pub version: String,
    /// Individual check results
    pub checks: Vec<CheckResult>,
    /// Time taken to run all checks
    pub total_latency: Duration,
    /// When the response was generated (uptime reference)
    pub generated_at: Instant,
}

impl HealthResponse {
    /// Creates a new health response from a list of check results
    pub fn new(
        version: impl Into<String>,
        checks: Vec<CheckResult>,
        total_latency: Duration,
    ) -> Self {
        let status = Self::aggregate_status(&checks);
        Self {
            status,
            version: version.into(),
            checks,
            total_latency,
            generated_at: Instant::now(),
        }
    }

    /// Computes the aggregate status from individual results:
    /// - Any `Unhealthy` or `Timeout` → `Unhealthy`
    /// - Any `Degraded` → `Degraded`
    /// - All `Healthy` / `Skipped` → `Healthy`
    fn aggregate_status(checks: &[CheckResult]) -> CheckStatus {
        let has_unhealthy = checks
            .iter()
            .any(|c| matches!(c.status, CheckStatus::Unhealthy | CheckStatus::Timeout));
        if has_unhealthy {
            return CheckStatus::Unhealthy;
        }
        let has_degraded = checks.iter().any(|c| c.status == CheckStatus::Degraded);
        if has_degraded {
            return CheckStatus::Degraded;
        }
        CheckStatus::Healthy
    }

    /// Returns the appropriate HTTP status code
    pub fn http_status(&self) -> u16 {
        self.status.http_status()
    }

    /// Converts the response to a JSON-like string (no external deps)
    pub fn to_json(&self) -> String {
        let checks_json: Vec<String> = self
            .checks
            .iter()
            .map(|c| {
                let msg = c
                    .message
                    .as_deref()
                    .map(|m| format!(r#","message":"{m}""#))
                    .unwrap_or_default();
                let meta: Vec<String> = c
                    .metadata
                    .iter()
                    .map(|(k, v)| format!(r#""{k}":"{v}""#))
                    .collect();
                let meta_str = if meta.is_empty() {
                    String::new()
                } else {
                    format!(r#","metadata":{{{}}}"#, meta.join(","))
                };
                format!(
                    r#"{{"name":"{}","status":"{}","latency_ms":{:.2}{msg}{meta_str}}}"#,
                    c.name,
                    c.status.label(),
                    c.latency.as_secs_f64() * 1000.0,
                )
            })
            .collect();

        format!(
            r#"{{"status":"{}","version":"{}","total_latency_ms":{:.2},"checks":[{}]}}"#,
            self.status.label(),
            self.version,
            self.total_latency.as_secs_f64() * 1000.0,
            checks_json.join(","),
        )
    }
}

/// Configuration for a dependency health check
#[derive(Debug, Clone)]
pub struct CheckConfig {
    /// Name of the dependency
    pub name: String,
    /// Timeout for the check
    pub timeout: Duration,
    /// Whether the dependency is required (affects aggregate status)
    pub required: bool,
}

impl CheckConfig {
    /// Creates a required check config
    pub fn required(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            timeout: Duration::from_secs(5),
            required: true,
        }
    }

    /// Creates an optional check config
    pub fn optional(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            timeout: Duration::from_secs(5),
            required: false,
        }
    }

    /// Sets a custom timeout
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// Mock dependency simulator for testing checks
#[derive(Debug, Clone)]
pub struct MockDependency {
    /// Dependency name
    pub name: String,
    /// Whether the dependency should report healthy
    pub healthy: bool,
    /// Simulated latency
    pub latency: Duration,
    /// Optional detail message
    pub message: Option<String>,
}

impl MockDependency {
    /// Creates a healthy mock dependency
    pub fn healthy(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            healthy: true,
            latency: Duration::from_millis(1),
            message: None,
        }
    }

    /// Creates an unhealthy mock dependency
    pub fn unhealthy(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            healthy: false,
            latency: Duration::from_millis(10),
            message: Some(message.into()),
        }
    }

    /// Runs the mock check and returns a CheckResult
    pub fn check(&self) -> CheckResult {
        if self.healthy {
            CheckResult::healthy(&self.name, self.latency)
        } else {
            CheckResult::unhealthy(
                &self.name,
                self.latency,
                self.message
                    .clone()
                    .unwrap_or_else(|| "unhealthy".to_string()),
            )
        }
    }
}

/// Liveness probe — reports whether the process itself is alive
#[derive(Debug, Clone)]
pub struct LivenessProbe {
    /// Server version
    pub version: String,
}

impl LivenessProbe {
    /// Creates a new liveness probe
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
        }
    }

    /// Runs the liveness check (always returns healthy for a live process)
    pub fn check(&self) -> HealthResponse {
        let result = CheckResult::healthy("process", Duration::from_nanos(1));
        HealthResponse::new(&self.version, vec![result], Duration::from_nanos(1))
    }
}

/// Readiness probe — reports whether the server is ready to accept traffic
pub struct ReadinessProbe {
    /// Server version
    pub version: String,
    /// List of dependency checks to run
    pub dependencies: Vec<MockDependency>,
}

impl ReadinessProbe {
    /// Creates a new readiness probe
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            dependencies: Vec::new(),
        }
    }

    /// Adds a dependency check
    #[must_use]
    pub fn add_dependency(mut self, dep: MockDependency) -> Self {
        self.dependencies.push(dep);
        self
    }

    /// Runs all dependency checks and returns an aggregated health response
    pub fn check(&self) -> HealthResponse {
        let start = Instant::now();
        let checks: Vec<CheckResult> = self.dependencies.iter().map(|d| d.check()).collect();
        let total = start.elapsed();
        HealthResponse::new(&self.version, checks, total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_status_http_status() {
        assert_eq!(CheckStatus::Healthy.http_status(), 200);
        assert_eq!(CheckStatus::Degraded.http_status(), 200);
        assert_eq!(CheckStatus::Unhealthy.http_status(), 503);
        assert_eq!(CheckStatus::Timeout.http_status(), 503);
        assert_eq!(CheckStatus::Skipped.http_status(), 200);
    }

    #[test]
    fn test_check_status_label() {
        assert_eq!(CheckStatus::Healthy.label(), "healthy");
        assert_eq!(CheckStatus::Degraded.label(), "degraded");
        assert_eq!(CheckStatus::Unhealthy.label(), "unhealthy");
        assert_eq!(CheckStatus::Timeout.label(), "timeout");
        assert_eq!(CheckStatus::Skipped.label(), "skipped");
    }

    #[test]
    fn test_check_status_is_passing() {
        assert!(CheckStatus::Healthy.is_passing());
        assert!(CheckStatus::Degraded.is_passing());
        assert!(CheckStatus::Skipped.is_passing());
        assert!(!CheckStatus::Unhealthy.is_passing());
        assert!(!CheckStatus::Timeout.is_passing());
    }

    #[test]
    fn test_check_result_healthy() {
        let r = CheckResult::healthy("database", Duration::from_millis(5));
        assert_eq!(r.status, CheckStatus::Healthy);
        assert!(r.message.is_none());
    }

    #[test]
    fn test_check_result_unhealthy() {
        let r = CheckResult::unhealthy("redis", Duration::from_millis(3000), "connection refused");
        assert_eq!(r.status, CheckStatus::Unhealthy);
        assert_eq!(r.message.as_deref(), Some("connection refused"));
    }

    #[test]
    fn test_check_result_degraded() {
        let r = CheckResult::degraded("cache", Duration::from_millis(200), "high latency");
        assert_eq!(r.status, CheckStatus::Degraded);
    }

    #[test]
    fn test_check_result_skipped() {
        let r = CheckResult::skipped("optional-service");
        assert_eq!(r.status, CheckStatus::Skipped);
        assert_eq!(r.latency, Duration::ZERO);
    }

    #[test]
    fn test_check_result_with_metadata() {
        let r = CheckResult::healthy("db", Duration::from_millis(2))
            .with_metadata("version", "14.1")
            .with_metadata("connections", "42");
        assert_eq!(r.metadata.get("version").map(String::as_str), Some("14.1"));
        assert_eq!(
            r.metadata.get("connections").map(String::as_str),
            Some("42")
        );
    }

    #[test]
    fn test_health_response_all_healthy() {
        let checks = vec![
            CheckResult::healthy("db", Duration::from_millis(2)),
            CheckResult::healthy("cache", Duration::from_millis(1)),
        ];
        let resp = HealthResponse::new("1.0.0", checks, Duration::from_millis(3));
        assert_eq!(resp.status, CheckStatus::Healthy);
        assert_eq!(resp.http_status(), 200);
    }

    #[test]
    fn test_health_response_degraded() {
        let checks = vec![
            CheckResult::healthy("db", Duration::from_millis(2)),
            CheckResult::degraded("cache", Duration::from_millis(100), "slow"),
        ];
        let resp = HealthResponse::new("1.0.0", checks, Duration::from_millis(102));
        assert_eq!(resp.status, CheckStatus::Degraded);
        assert_eq!(resp.http_status(), 200);
    }

    #[test]
    fn test_health_response_unhealthy() {
        let checks = vec![
            CheckResult::healthy("db", Duration::from_millis(2)),
            CheckResult::unhealthy("queue", Duration::from_millis(5000), "timeout"),
        ];
        let resp = HealthResponse::new("1.0.0", checks, Duration::from_millis(5002));
        assert_eq!(resp.status, CheckStatus::Unhealthy);
        assert_eq!(resp.http_status(), 503);
    }

    #[test]
    fn test_health_response_to_json_contains_status() {
        let checks = vec![CheckResult::healthy("db", Duration::from_millis(1))];
        let resp = HealthResponse::new("2.0.0", checks, Duration::from_millis(1));
        let json = resp.to_json();
        assert!(json.contains(r#""status":"healthy""#));
        assert!(json.contains(r#""version":"2.0.0""#));
        assert!(json.contains("db"));
    }

    #[test]
    fn test_liveness_probe_always_healthy() {
        let probe = LivenessProbe::new("1.2.3");
        let resp = probe.check();
        assert_eq!(resp.status, CheckStatus::Healthy);
        assert_eq!(resp.version, "1.2.3");
    }

    #[test]
    fn test_readiness_probe_all_healthy() {
        let probe = ReadinessProbe::new("1.0.0")
            .add_dependency(MockDependency::healthy("db"))
            .add_dependency(MockDependency::healthy("cache"));
        let resp = probe.check();
        assert_eq!(resp.status, CheckStatus::Healthy);
        assert_eq!(resp.checks.len(), 2);
    }

    #[test]
    fn test_readiness_probe_one_unhealthy() {
        let probe = ReadinessProbe::new("1.0.0")
            .add_dependency(MockDependency::healthy("db"))
            .add_dependency(MockDependency::unhealthy("redis", "ECONNREFUSED"));
        let resp = probe.check();
        assert_eq!(resp.status, CheckStatus::Unhealthy);
        assert_eq!(resp.http_status(), 503);
    }

    #[test]
    fn test_check_config_required() {
        let cfg = CheckConfig::required("database");
        assert!(cfg.required);
        assert_eq!(cfg.timeout, Duration::from_secs(5));
    }

    #[test]
    fn test_check_config_optional_with_timeout() {
        let cfg = CheckConfig::optional("analytics").with_timeout(Duration::from_secs(2));
        assert!(!cfg.required);
        assert_eq!(cfg.timeout, Duration::from_secs(2));
    }
}
