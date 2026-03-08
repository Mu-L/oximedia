#![allow(dead_code)]
//! Retry policy engine for workflow task execution.
//!
//! Provides configurable retry strategies for failed workflow tasks including
//! fixed delay, exponential back-off, linear back-off, and custom schedules.
//! Includes jitter support, max attempt limits, and retry budgets.

use std::collections::HashMap;
use std::time::Duration;

/// Strategy used to calculate delays between retries.
#[derive(Debug, Clone, PartialEq)]
pub enum RetryStrategy {
    /// Fixed delay between retries.
    Fixed {
        /// The constant delay.
        delay: Duration,
    },
    /// Exponential back-off: `base_delay * multiplier^attempt`.
    Exponential {
        /// Initial delay.
        base_delay: Duration,
        /// Multiplier applied per attempt.
        multiplier: f64,
        /// Maximum delay cap.
        max_delay: Duration,
    },
    /// Linear back-off: `base_delay + increment * attempt`.
    Linear {
        /// Initial delay.
        base_delay: Duration,
        /// Increment per attempt.
        increment: Duration,
        /// Maximum delay cap.
        max_delay: Duration,
    },
    /// A predefined schedule of delays.
    Custom {
        /// List of delays for each retry attempt.
        delays: Vec<Duration>,
    },
    /// No retries at all.
    NoRetry,
}

/// Jitter mode applied to computed delays.
#[derive(Debug, Clone, PartialEq)]
pub enum JitterMode {
    /// No jitter.
    None,
    /// Full jitter: random value in `[0, delay]`.
    Full,
    /// Equal jitter: `delay/2 + random(0, delay/2)`.
    Equal,
    /// Decorrelated jitter based on previous delay.
    Decorrelated,
}

/// Configuration for a retry policy.
#[derive(Debug, Clone)]
pub struct RetryPolicyConfig {
    /// Name of this policy.
    pub name: String,
    /// The retry strategy.
    pub strategy: RetryStrategy,
    /// Maximum number of retry attempts (0 = no retries).
    pub max_attempts: u32,
    /// Jitter mode.
    pub jitter: JitterMode,
    /// Set of error codes that should be retried. Empty means all errors.
    pub retryable_errors: Vec<String>,
    /// Set of error codes that should never be retried.
    pub non_retryable_errors: Vec<String>,
    /// Maximum total time spent retrying.
    pub total_timeout: Option<Duration>,
    /// Metadata.
    pub metadata: HashMap<String, String>,
}

impl RetryPolicyConfig {
    /// Create a new retry policy configuration.
    pub fn new(name: impl Into<String>, strategy: RetryStrategy, max_attempts: u32) -> Self {
        Self {
            name: name.into(),
            strategy,
            max_attempts,
            jitter: JitterMode::None,
            retryable_errors: Vec::new(),
            non_retryable_errors: Vec::new(),
            total_timeout: None,
            metadata: HashMap::new(),
        }
    }

    /// Set the jitter mode.
    #[must_use]
    pub fn with_jitter(mut self, jitter: JitterMode) -> Self {
        self.jitter = jitter;
        self
    }

    /// Add a retryable error code.
    pub fn add_retryable_error(mut self, code: impl Into<String>) -> Self {
        self.retryable_errors.push(code.into());
        self
    }

    /// Add a non-retryable error code.
    pub fn add_non_retryable_error(mut self, code: impl Into<String>) -> Self {
        self.non_retryable_errors.push(code.into());
        self
    }

    /// Set the total timeout.
    #[must_use]
    pub fn with_total_timeout(mut self, timeout: Duration) -> Self {
        self.total_timeout = Some(timeout);
        self
    }

    /// Set metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Create a default fixed-delay policy.
    pub fn fixed(name: impl Into<String>, delay: Duration, max_attempts: u32) -> Self {
        Self::new(name, RetryStrategy::Fixed { delay }, max_attempts)
    }

    /// Create a default exponential back-off policy.
    pub fn exponential(
        name: impl Into<String>,
        base_delay: Duration,
        multiplier: f64,
        max_delay: Duration,
        max_attempts: u32,
    ) -> Self {
        Self::new(
            name,
            RetryStrategy::Exponential {
                base_delay,
                multiplier,
                max_delay,
            },
            max_attempts,
        )
    }
}

/// Tracks the state of retry attempts for a single task.
#[derive(Debug, Clone)]
pub struct RetryState {
    /// The retry policy configuration.
    pub config: RetryPolicyConfig,
    /// Current attempt number (starts at 0).
    pub current_attempt: u32,
    /// History of delays that were applied.
    pub delay_history: Vec<Duration>,
    /// Total time spent waiting so far.
    pub total_wait_time: Duration,
    /// Error messages from each failed attempt.
    pub error_history: Vec<String>,
    /// Whether retries are exhausted.
    pub exhausted: bool,
}

impl RetryState {
    /// Create a new retry state from a config.
    #[must_use]
    pub fn new(config: RetryPolicyConfig) -> Self {
        Self {
            config,
            current_attempt: 0,
            delay_history: Vec::new(),
            total_wait_time: Duration::ZERO,
            error_history: Vec::new(),
            exhausted: false,
        }
    }

    /// Check whether the given error code is retryable under this policy.
    #[must_use]
    pub fn is_retryable(&self, error_code: &str) -> bool {
        if self
            .config
            .non_retryable_errors
            .contains(&error_code.to_string())
        {
            return false;
        }
        if self.config.retryable_errors.is_empty() {
            return true;
        }
        self.config
            .retryable_errors
            .contains(&error_code.to_string())
    }

    /// Check whether another retry attempt is possible.
    #[must_use]
    pub fn can_retry(&self) -> bool {
        if self.exhausted {
            return false;
        }
        if self.current_attempt >= self.config.max_attempts {
            return false;
        }
        if let Some(timeout) = self.config.total_timeout {
            if self.total_wait_time >= timeout {
                return false;
            }
        }
        true
    }

    /// Calculate the delay for the next retry attempt.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn next_delay(&self) -> Duration {
        let attempt = self.current_attempt;
        match &self.config.strategy {
            RetryStrategy::Fixed { delay } => *delay,
            RetryStrategy::Exponential {
                base_delay,
                multiplier,
                max_delay,
            } => {
                let base_ms = base_delay.as_millis() as f64;
                let computed = base_ms * multiplier.powi(attempt as i32);
                let capped = computed.min(max_delay.as_millis() as f64);
                Duration::from_millis(capped as u64)
            }
            RetryStrategy::Linear {
                base_delay,
                increment,
                max_delay,
            } => {
                let base_ms = base_delay.as_millis() as u64;
                let inc_ms = increment.as_millis() as u64;
                let computed = base_ms.saturating_add(inc_ms.saturating_mul(u64::from(attempt)));
                let capped = computed.min(max_delay.as_millis() as u64);
                Duration::from_millis(capped)
            }
            RetryStrategy::Custom { delays } => {
                if (attempt as usize) < delays.len() {
                    delays[attempt as usize]
                } else {
                    delays.last().copied().unwrap_or(Duration::ZERO)
                }
            }
            RetryStrategy::NoRetry => Duration::ZERO,
        }
    }

    /// Apply jitter to a computed delay.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn apply_jitter(&self, delay: Duration, random_factor: f64) -> Duration {
        let ms = delay.as_millis() as f64;
        let factor = random_factor.clamp(0.0, 1.0);
        let jittered = match &self.config.jitter {
            JitterMode::None => ms,
            JitterMode::Full => ms * factor,
            JitterMode::Equal => (ms / 2.0) + (ms / 2.0) * factor,
            JitterMode::Decorrelated => {
                let prev = self
                    .delay_history
                    .last()
                    .map_or(ms, |d| d.as_millis() as f64);
                let upper = prev * 3.0;
                ms.max(upper * factor)
            }
        };
        Duration::from_millis(jittered as u64)
    }

    /// Record a failed attempt and advance the state.
    pub fn record_attempt(&mut self, error_message: impl Into<String>) {
        let delay = self.next_delay();
        self.delay_history.push(delay);
        self.total_wait_time += delay;
        self.error_history.push(error_message.into());
        self.current_attempt += 1;
        if !self.can_retry() {
            self.exhausted = true;
        }
    }

    /// Return the number of attempts made so far.
    #[must_use]
    pub fn attempts_made(&self) -> u32 {
        self.current_attempt
    }

    /// Return remaining attempts.
    #[must_use]
    pub fn remaining_attempts(&self) -> u32 {
        self.config
            .max_attempts
            .saturating_sub(self.current_attempt)
    }

    /// Reset the retry state for reuse.
    pub fn reset(&mut self) {
        self.current_attempt = 0;
        self.delay_history.clear();
        self.total_wait_time = Duration::ZERO;
        self.error_history.clear();
        self.exhausted = false;
    }
}

/// Registry of named retry policies.
#[derive(Debug)]
pub struct RetryPolicyRegistry {
    /// Policies indexed by name.
    policies: HashMap<String, RetryPolicyConfig>,
}

impl Default for RetryPolicyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RetryPolicyRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            policies: HashMap::new(),
        }
    }

    /// Register a policy.
    pub fn register(&mut self, config: RetryPolicyConfig) {
        self.policies.insert(config.name.clone(), config);
    }

    /// Get a policy by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&RetryPolicyConfig> {
        self.policies.get(name)
    }

    /// Create a new retry state for a named policy.
    #[must_use]
    pub fn create_state(&self, policy_name: &str) -> Option<RetryState> {
        self.policies
            .get(policy_name)
            .map(|c| RetryState::new(c.clone()))
    }

    /// Return the number of registered policies.
    #[must_use]
    pub fn count(&self) -> usize {
        self.policies.len()
    }

    /// List all registered policy names.
    #[must_use]
    pub fn policy_names(&self) -> Vec<&str> {
        self.policies
            .keys()
            .map(std::string::String::as_str)
            .collect()
    }

    /// Remove a policy.
    pub fn remove(&mut self, name: &str) -> Option<RetryPolicyConfig> {
        self.policies.remove(name)
    }

    /// Create a registry pre-loaded with common default policies.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(RetryPolicyConfig::fixed(
            "quick-retry",
            Duration::from_secs(1),
            3,
        ));
        registry.register(RetryPolicyConfig::exponential(
            "exponential-backoff",
            Duration::from_secs(1),
            2.0,
            Duration::from_secs(60),
            5,
        ));
        registry.register(RetryPolicyConfig::new(
            "no-retry",
            RetryStrategy::NoRetry,
            0,
        ));
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_delay() {
        let config = RetryPolicyConfig::fixed("test", Duration::from_secs(5), 3);
        let state = RetryState::new(config);
        assert_eq!(state.next_delay(), Duration::from_secs(5));
    }

    #[test]
    fn test_exponential_delay() {
        let config = RetryPolicyConfig::exponential(
            "exp",
            Duration::from_millis(100),
            2.0,
            Duration::from_secs(10),
            5,
        );
        let mut state = RetryState::new(config);
        assert_eq!(state.next_delay(), Duration::from_millis(100)); // 100 * 2^0
        state.current_attempt = 1;
        assert_eq!(state.next_delay(), Duration::from_millis(200)); // 100 * 2^1
        state.current_attempt = 2;
        assert_eq!(state.next_delay(), Duration::from_millis(400)); // 100 * 2^2
    }

    #[test]
    fn test_exponential_capped() {
        let config = RetryPolicyConfig::exponential(
            "exp-cap",
            Duration::from_secs(1),
            10.0,
            Duration::from_secs(5),
            10,
        );
        let mut state = RetryState::new(config);
        state.current_attempt = 5;
        assert_eq!(state.next_delay(), Duration::from_secs(5)); // capped
    }

    #[test]
    fn test_linear_delay() {
        let config = RetryPolicyConfig::new(
            "linear",
            RetryStrategy::Linear {
                base_delay: Duration::from_millis(100),
                increment: Duration::from_millis(50),
                max_delay: Duration::from_secs(1),
            },
            5,
        );
        let mut state = RetryState::new(config);
        assert_eq!(state.next_delay(), Duration::from_millis(100));
        state.current_attempt = 2;
        assert_eq!(state.next_delay(), Duration::from_millis(200));
    }

    #[test]
    fn test_custom_delays() {
        let config = RetryPolicyConfig::new(
            "custom",
            RetryStrategy::Custom {
                delays: vec![
                    Duration::from_secs(1),
                    Duration::from_secs(5),
                    Duration::from_secs(30),
                ],
            },
            3,
        );
        let mut state = RetryState::new(config);
        assert_eq!(state.next_delay(), Duration::from_secs(1));
        state.current_attempt = 1;
        assert_eq!(state.next_delay(), Duration::from_secs(5));
        state.current_attempt = 2;
        assert_eq!(state.next_delay(), Duration::from_secs(30));
    }

    #[test]
    fn test_no_retry() {
        let config = RetryPolicyConfig::new("none", RetryStrategy::NoRetry, 0);
        let state = RetryState::new(config);
        assert!(!state.can_retry());
    }

    #[test]
    fn test_can_retry_limits() {
        let config = RetryPolicyConfig::fixed("test", Duration::from_secs(1), 2);
        let mut state = RetryState::new(config);
        assert!(state.can_retry());
        state.record_attempt("error 1");
        assert!(state.can_retry());
        state.record_attempt("error 2");
        assert!(!state.can_retry());
        assert!(state.exhausted);
    }

    #[test]
    fn test_total_timeout_limit() {
        let config = RetryPolicyConfig::fixed("test", Duration::from_secs(10), 100)
            .with_total_timeout(Duration::from_secs(5));
        let mut state = RetryState::new(config);
        state.record_attempt("fail");
        // total_wait_time is now 10s which exceeds the 5s timeout
        assert!(!state.can_retry());
    }

    #[test]
    fn test_retryable_errors() {
        let config = RetryPolicyConfig::fixed("test", Duration::from_secs(1), 3)
            .add_retryable_error("TIMEOUT")
            .add_retryable_error("CONN_RESET");
        let state = RetryState::new(config);
        assert!(state.is_retryable("TIMEOUT"));
        assert!(state.is_retryable("CONN_RESET"));
        assert!(!state.is_retryable("PERMISSION_DENIED"));
    }

    #[test]
    fn test_non_retryable_errors() {
        let config = RetryPolicyConfig::fixed("test", Duration::from_secs(1), 3)
            .add_non_retryable_error("FATAL");
        let state = RetryState::new(config);
        assert!(!state.is_retryable("FATAL"));
        assert!(state.is_retryable("ANYTHING_ELSE"));
    }

    #[test]
    fn test_record_attempt_tracks_history() {
        let config = RetryPolicyConfig::fixed("test", Duration::from_secs(1), 5);
        let mut state = RetryState::new(config);
        state.record_attempt("error A");
        state.record_attempt("error B");
        assert_eq!(state.attempts_made(), 2);
        assert_eq!(state.remaining_attempts(), 3);
        assert_eq!(state.error_history.len(), 2);
        assert_eq!(state.delay_history.len(), 2);
    }

    #[test]
    fn test_reset_state() {
        let config = RetryPolicyConfig::fixed("test", Duration::from_secs(1), 3);
        let mut state = RetryState::new(config);
        state.record_attempt("err");
        state.record_attempt("err");
        state.reset();
        assert_eq!(state.attempts_made(), 0);
        assert!(state.can_retry());
        assert!(!state.exhausted);
    }

    #[test]
    fn test_jitter_none() {
        let config = RetryPolicyConfig::fixed("test", Duration::from_secs(2), 3);
        let state = RetryState::new(config);
        let delay = state.apply_jitter(Duration::from_secs(2), 0.5);
        assert_eq!(delay, Duration::from_secs(2));
    }

    #[test]
    fn test_jitter_full() {
        let config = RetryPolicyConfig::fixed("test", Duration::from_millis(1000), 3)
            .with_jitter(JitterMode::Full);
        let state = RetryState::new(config);
        let delay = state.apply_jitter(Duration::from_millis(1000), 0.5);
        assert_eq!(delay, Duration::from_millis(500));
    }

    #[test]
    fn test_jitter_equal() {
        let config = RetryPolicyConfig::fixed("test", Duration::from_millis(1000), 3)
            .with_jitter(JitterMode::Equal);
        let state = RetryState::new(config);
        let delay = state.apply_jitter(Duration::from_millis(1000), 0.5);
        // 500 + 500*0.5 = 750
        assert_eq!(delay, Duration::from_millis(750));
    }

    #[test]
    fn test_registry_basic() {
        let mut registry = RetryPolicyRegistry::new();
        let config = RetryPolicyConfig::fixed("my-policy", Duration::from_secs(1), 3);
        registry.register(config);
        assert_eq!(registry.count(), 1);
        assert!(registry.get("my-policy").is_some());
        assert!(registry.get("unknown").is_none());
    }

    #[test]
    fn test_registry_create_state() {
        let mut registry = RetryPolicyRegistry::new();
        registry.register(RetryPolicyConfig::fixed("p", Duration::from_secs(1), 2));
        let state = registry.create_state("p");
        assert!(state.is_some());
        let s = state.expect("should succeed in test");
        assert_eq!(s.config.max_attempts, 2);
    }

    #[test]
    fn test_registry_with_defaults() {
        let registry = RetryPolicyRegistry::with_defaults();
        assert!(registry.get("quick-retry").is_some());
        assert!(registry.get("exponential-backoff").is_some());
        assert!(registry.get("no-retry").is_some());
        assert_eq!(registry.count(), 3);
    }

    #[test]
    fn test_registry_remove() {
        let mut registry = RetryPolicyRegistry::with_defaults();
        let removed = registry.remove("quick-retry");
        assert!(removed.is_some());
        assert!(registry.get("quick-retry").is_none());
    }

    #[test]
    fn test_config_metadata() {
        let config =
            RetryPolicyConfig::fixed("m", Duration::from_secs(1), 1).with_metadata("team", "media");
        assert_eq!(
            config.metadata.get("team").map(|s| s.as_str()),
            Some("media")
        );
    }

    #[test]
    fn test_default_registry() {
        let registry = RetryPolicyRegistry::default();
        assert_eq!(registry.count(), 0);
    }
}
