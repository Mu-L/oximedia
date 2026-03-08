#![allow(dead_code)]
//! Rate limiting — token bucket and fixed-window algorithms for controlling job throughput.

use std::time::{Duration, Instant};

/// Algorithm used for rate limiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitAlgorithm {
    /// Token bucket: tokens accumulate up to a capacity and are consumed per request.
    TokenBucket,
    /// Fixed window: allows N requests per window period, resets at each boundary.
    FixedWindow,
    /// Sliding window: smooth rate limiting based on a rolling time window.
    SlidingWindow,
    /// Leaky bucket: requests drain at a fixed rate regardless of burst.
    LeakyBucket,
}

impl RateLimitAlgorithm {
    /// Display name.
    pub fn name(&self) -> &'static str {
        match self {
            RateLimitAlgorithm::TokenBucket => "token_bucket",
            RateLimitAlgorithm::FixedWindow => "fixed_window",
            RateLimitAlgorithm::SlidingWindow => "sliding_window",
            RateLimitAlgorithm::LeakyBucket => "leaky_bucket",
        }
    }

    /// Whether this algorithm allows short bursts above the sustained rate.
    pub fn supports_bursting(&self) -> bool {
        matches!(
            self,
            RateLimitAlgorithm::TokenBucket | RateLimitAlgorithm::LeakyBucket
        )
    }
}

/// Configuration for a rate limiter.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests (or tokens) per window.
    pub limit: u64,
    /// The window duration over which `limit` requests are allowed.
    pub window: Duration,
    /// The algorithm to use.
    pub algorithm: RateLimitAlgorithm,
    /// Whether to strictly enforce the limit (deny vs queue excess).
    pub strict: bool,
    /// Optional name/label for this limiter.
    pub name: String,
}

impl RateLimitConfig {
    /// Create a new config.
    pub fn new(
        name: impl Into<String>,
        limit: u64,
        window: Duration,
        algorithm: RateLimitAlgorithm,
    ) -> Self {
        Self {
            limit,
            window,
            algorithm,
            strict: true,
            name: name.into(),
        }
    }

    /// Set strict mode (deny excess requests rather than queuing them).
    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    /// Returns true if this config uses strict enforcement.
    pub fn is_strict(&self) -> bool {
        self.strict
    }

    /// Sustained rate in requests per second.
    #[allow(clippy::cast_precision_loss)]
    pub fn rate_per_second(&self) -> f64 {
        let secs = self.window.as_secs_f64();
        if secs <= 0.0 {
            return 0.0;
        }
        self.limit as f64 / secs
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self::new(
            "default",
            100,
            Duration::from_secs(1),
            RateLimitAlgorithm::TokenBucket,
        )
    }
}

/// A token-bucket rate limiter.
///
/// Tokens refill continuously at `limit / window` per second up to `limit` capacity.
#[derive(Debug)]
pub struct RateLimiter {
    config: RateLimitConfig,
    /// Current number of available tokens (fractional for precision).
    tokens: f64,
    /// When the token count was last updated.
    last_refill: Instant,
}

impl RateLimiter {
    /// Create a rate limiter from the given config.
    pub fn new(config: RateLimitConfig) -> Self {
        let tokens = config.limit as f64;
        Self {
            config,
            tokens,
            last_refill: Instant::now(),
        }
    }

    /// Refill tokens based on elapsed time.
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let rate = self.config.rate_per_second();
        self.tokens = (self.tokens + elapsed * rate).min(self.config.limit as f64);
        self.last_refill = now;
    }

    /// Attempt to acquire `count` tokens. Returns true if acquired, false if denied.
    pub fn try_acquire(&mut self, count: u64) -> bool {
        self.refill();
        let needed = count as f64;
        if self.tokens >= needed {
            self.tokens -= needed;
            true
        } else {
            false
        }
    }

    /// Attempt to acquire one token.
    pub fn try_acquire_one(&mut self) -> bool {
        self.try_acquire(1)
    }

    /// Available tokens (floored to nearest integer for display).
    pub fn available_tokens(&self) -> u64 {
        self.tokens.floor() as u64
    }

    /// The current fractional token level (for tests/diagnostics).
    pub fn tokens_f64(&self) -> f64 {
        self.tokens
    }

    /// The configured limit.
    pub fn limit(&self) -> u64 {
        self.config.limit
    }

    /// Returns a reference to the config.
    pub fn config(&self) -> &RateLimitConfig {
        &self.config
    }
}

/// A record of a single rate-limited request.
#[derive(Debug, Clone)]
pub struct RequestRecord {
    /// Timestamp of the request.
    pub at: Instant,
    /// Whether the request was allowed.
    pub allowed: bool,
    /// Number of tokens requested.
    pub tokens_requested: u64,
}

/// Tracks rate-limit decisions over time for analysis.
#[derive(Debug, Default)]
pub struct RateLimitTracker {
    records: Vec<RequestRecord>,
}

impl RateLimitTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a rate-limit decision.
    pub fn record_request(&mut self, allowed: bool, tokens_requested: u64) {
        self.records.push(RequestRecord {
            at: Instant::now(),
            allowed,
            tokens_requested,
        });
    }

    /// Total requests recorded.
    pub fn total(&self) -> usize {
        self.records.len()
    }

    /// Number of allowed requests.
    pub fn allowed_count(&self) -> usize {
        self.records.iter().filter(|r| r.allowed).count()
    }

    /// Number of denied requests.
    pub fn denied_count(&self) -> usize {
        self.records.iter().filter(|r| !r.allowed).count()
    }

    /// Allow rate [0.0, 1.0]. Returns 0.0 if no records.
    #[allow(clippy::cast_precision_loss)]
    pub fn allow_rate(&self) -> f64 {
        if self.records.is_empty() {
            return 0.0;
        }
        self.allowed_count() as f64 / self.records.len() as f64
    }

    /// Total tokens that were successfully acquired.
    pub fn total_tokens_acquired(&self) -> u64 {
        self.records
            .iter()
            .filter(|r| r.allowed)
            .map(|r| r.tokens_requested)
            .sum()
    }

    /// Clear all records.
    pub fn clear(&mut self) {
        self.records.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_algorithm_name() {
        assert_eq!(RateLimitAlgorithm::TokenBucket.name(), "token_bucket");
        assert_eq!(RateLimitAlgorithm::FixedWindow.name(), "fixed_window");
        assert_eq!(RateLimitAlgorithm::SlidingWindow.name(), "sliding_window");
        assert_eq!(RateLimitAlgorithm::LeakyBucket.name(), "leaky_bucket");
    }

    #[test]
    fn test_algorithm_supports_bursting() {
        assert!(RateLimitAlgorithm::TokenBucket.supports_bursting());
        assert!(RateLimitAlgorithm::LeakyBucket.supports_bursting());
        assert!(!RateLimitAlgorithm::FixedWindow.supports_bursting());
        assert!(!RateLimitAlgorithm::SlidingWindow.supports_bursting());
    }

    #[test]
    fn test_config_is_strict() {
        let cfg = RateLimitConfig::new(
            "test",
            100,
            Duration::from_secs(1),
            RateLimitAlgorithm::TokenBucket,
        );
        assert!(cfg.is_strict()); // default strict
        let non_strict = cfg.with_strict(false);
        assert!(!non_strict.is_strict());
    }

    #[test]
    fn test_config_rate_per_second() {
        let cfg = RateLimitConfig::new(
            "rps",
            60,
            Duration::from_secs(60),
            RateLimitAlgorithm::TokenBucket,
        );
        assert!((cfg.rate_per_second() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_config_default() {
        let cfg = RateLimitConfig::default();
        assert_eq!(cfg.limit, 100);
        assert!(cfg.is_strict());
    }

    #[test]
    fn test_rate_limiter_initial_full() {
        let cfg = RateLimitConfig::new(
            "l1",
            10,
            Duration::from_secs(1),
            RateLimitAlgorithm::TokenBucket,
        );
        let limiter = RateLimiter::new(cfg);
        assert_eq!(limiter.available_tokens(), 10);
    }

    #[test]
    fn test_rate_limiter_try_acquire() {
        let cfg = RateLimitConfig::new(
            "l2",
            5,
            Duration::from_secs(1),
            RateLimitAlgorithm::TokenBucket,
        );
        let mut limiter = RateLimiter::new(cfg);
        assert!(limiter.try_acquire(3));
        assert!(limiter.try_acquire(2));
        // Tokens exhausted
        assert!(!limiter.try_acquire(1));
    }

    #[test]
    fn test_rate_limiter_try_acquire_one() {
        let cfg = RateLimitConfig::new(
            "l3",
            2,
            Duration::from_secs(1),
            RateLimitAlgorithm::TokenBucket,
        );
        let mut limiter = RateLimiter::new(cfg);
        assert!(limiter.try_acquire_one());
        assert!(limiter.try_acquire_one());
        assert!(!limiter.try_acquire_one());
    }

    #[test]
    fn test_rate_limiter_limit() {
        let cfg = RateLimitConfig::new(
            "l4",
            42,
            Duration::from_secs(1),
            RateLimitAlgorithm::TokenBucket,
        );
        let limiter = RateLimiter::new(cfg);
        assert_eq!(limiter.limit(), 42);
    }

    #[test]
    fn test_tracker_empty() {
        let t = RateLimitTracker::new();
        assert_eq!(t.total(), 0);
        assert_eq!(t.allow_rate(), 0.0);
        assert_eq!(t.total_tokens_acquired(), 0);
    }

    #[test]
    fn test_tracker_record_request() {
        let mut t = RateLimitTracker::new();
        t.record_request(true, 1);
        t.record_request(true, 2);
        t.record_request(false, 1);
        assert_eq!(t.total(), 3);
        assert_eq!(t.allowed_count(), 2);
        assert_eq!(t.denied_count(), 1);
    }

    #[test]
    fn test_tracker_allow_rate() {
        let mut t = RateLimitTracker::new();
        t.record_request(true, 1);
        t.record_request(false, 1);
        let rate = t.allow_rate();
        assert!((rate - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_tracker_total_tokens_acquired() {
        let mut t = RateLimitTracker::new();
        t.record_request(true, 5);
        t.record_request(true, 3);
        t.record_request(false, 10); // denied — not counted
        assert_eq!(t.total_tokens_acquired(), 8);
    }

    #[test]
    fn test_tracker_clear() {
        let mut t = RateLimitTracker::new();
        t.record_request(true, 1);
        t.clear();
        assert_eq!(t.total(), 0);
    }
}
