//! Rate limiting primitives for batch processing pipelines.
//!
//! Provides a token-bucket implementation and a concurrency limiter that
//! can be composed to enforce both throughput and parallelism constraints.

#![allow(dead_code)]

/// Configuration for rate and concurrency limiting.
#[derive(Debug, Clone)]
pub struct ThrottleConfig {
    /// Maximum number of items to process concurrently.
    pub max_concurrent: usize,
    /// Sustained throughput limit (items per second).
    pub rate_per_second: f64,
    /// Additional tokens that may be consumed in a single burst.
    pub burst_allowance: usize,
}

impl ThrottleConfig {
    /// Default configuration (8 concurrent, 10 rps, burst 5).
    #[must_use]
    pub fn default() -> Self {
        Self {
            max_concurrent: 8,
            rate_per_second: 10.0,
            burst_allowance: 5,
        }
    }

    /// Conservative configuration (2 concurrent, 2 rps, no burst).
    #[must_use]
    pub fn conservative() -> Self {
        Self {
            max_concurrent: 2,
            rate_per_second: 2.0,
            burst_allowance: 0,
        }
    }
}

/// A token-bucket throttle driven by caller-supplied wall-clock timestamps.
///
/// The caller is responsible for providing monotonic `now_ms` values.
#[derive(Debug, Clone)]
pub struct TokenBucketThrottle {
    /// Current number of available tokens (fractional).
    pub tokens: f64,
    /// Maximum number of tokens (bucket capacity).
    pub max_tokens: f64,
    /// Tokens added per millisecond.
    pub refill_rate: f64,
    /// Timestamp of the last refill (milliseconds).
    pub last_refill_ms: u64,
}

impl TokenBucketThrottle {
    /// Create a new token bucket.
    ///
    /// * `max_tokens` — bucket capacity.
    /// * `rate_per_second` — sustained token replenishment rate.
    /// * `now_ms` — current timestamp used to initialise the refill clock.
    #[must_use]
    pub fn new(max_tokens: f64, rate_per_second: f64, now_ms: u64) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate: rate_per_second / 1_000.0,
            last_refill_ms: now_ms,
        }
    }

    /// Refill the bucket based on the elapsed time since the last refill.
    pub fn refill(&mut self, now_ms: u64) {
        let elapsed = now_ms.saturating_sub(self.last_refill_ms) as f64;
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.max_tokens);
        self.last_refill_ms = now_ms;
    }

    /// Try to consume one token.
    ///
    /// Returns `true` when a token was successfully consumed, `false` when the
    /// bucket is empty (the caller should back off).
    pub fn try_consume(&mut self, now_ms: u64) -> bool {
        self.refill(now_ms);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Current number of available tokens.
    #[must_use]
    pub fn available_tokens(&self) -> f64 {
        self.tokens
    }
}

/// Limits the number of simultaneously active items.
#[derive(Debug, Clone)]
pub struct ConcurrencyLimiter {
    /// Number of currently active (acquired) slots.
    pub active: usize,
    /// Maximum permitted concurrent slots.
    pub max_concurrent: usize,
}

impl ConcurrencyLimiter {
    /// Create a new limiter.
    #[must_use]
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            active: 0,
            max_concurrent,
        }
    }

    /// Attempt to acquire a concurrency slot.
    ///
    /// Returns `true` on success. The caller **must** call [`release`](Self::release)
    /// when the work item finishes.
    pub fn try_acquire(&mut self) -> bool {
        if self.active < self.max_concurrent {
            self.active += 1;
            true
        } else {
            false
        }
    }

    /// Release a previously acquired slot.
    pub fn release(&mut self) {
        self.active = self.active.saturating_sub(1);
    }

    /// Current utilisation as a fraction in `[0.0, 1.0]`.
    ///
    /// Returns `0.0` when `max_concurrent` is zero.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn utilization(&self) -> f64 {
        if self.max_concurrent == 0 {
            return 0.0;
        }
        self.active as f64 / self.max_concurrent as f64
    }
}

/// Accumulated statistics for a throttle session.
#[derive(Debug, Clone, Default)]
pub struct ThrottleStats {
    /// Number of requests that were allowed through.
    pub accepted: u64,
    /// Number of requests that were rejected (bucket empty).
    pub rejected: u64,
    /// Total milliseconds spent waiting across all accepted requests.
    pub total_wait_ms: u64,
}

impl ThrottleStats {
    /// Fraction of requests that were rejected (`[0.0, 1.0]`).
    ///
    /// Returns `0.0` when no requests have been made.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn rejection_rate(&self) -> f64 {
        let total = self.accepted + self.rejected;
        if total == 0 {
            return 0.0;
        }
        self.rejected as f64 / total as f64
    }

    /// Average wait time per accepted request in milliseconds.
    ///
    /// Returns `0.0` when no requests have been accepted.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_wait_ms(&self) -> f64 {
        if self.accepted == 0 {
            return 0.0;
        }
        self.total_wait_ms as f64 / self.accepted as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- ThrottleConfig ----------

    #[test]
    fn test_throttle_config_default_fields() {
        let cfg = ThrottleConfig::default();
        assert_eq!(cfg.max_concurrent, 8);
        assert!(cfg.rate_per_second > 0.0);
        assert_eq!(cfg.burst_allowance, 5);
    }

    #[test]
    fn test_throttle_config_conservative_more_restrictive() {
        let c = ThrottleConfig::conservative();
        let d = ThrottleConfig::default();
        assert!(c.max_concurrent < d.max_concurrent);
        assert!(c.rate_per_second < d.rate_per_second);
    }

    // ---------- TokenBucketThrottle ----------

    #[test]
    fn test_token_bucket_starts_full() {
        let tb = TokenBucketThrottle::new(10.0, 1.0, 0);
        assert!((tb.available_tokens() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_token_bucket_consume_decrements() {
        let mut tb = TokenBucketThrottle::new(10.0, 0.0, 0);
        assert!(tb.try_consume(0));
        assert!((tb.available_tokens() - 9.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_token_bucket_empty_rejects() {
        let mut tb = TokenBucketThrottle::new(1.0, 0.0, 0);
        assert!(tb.try_consume(0));
        assert!(!tb.try_consume(0)); // bucket now empty
    }

    #[test]
    fn test_token_bucket_refill_over_time() {
        // 1 token/second = 1 token/1000 ms
        let mut tb = TokenBucketThrottle::new(10.0, 1.0, 0);
        // Drain the bucket
        for _ in 0..10 {
            tb.try_consume(0);
        }
        assert!(tb.available_tokens() < 1.0);
        // Advance 2000 ms → 2 new tokens
        tb.refill(2_000);
        assert!(tb.available_tokens() >= 2.0);
    }

    #[test]
    fn test_token_bucket_does_not_exceed_max() {
        let mut tb = TokenBucketThrottle::new(5.0, 100.0, 0);
        tb.refill(10_000); // would give 1_000_000 tokens without the cap
        assert!((tb.available_tokens() - 5.0).abs() < f64::EPSILON);
    }

    // ---------- ConcurrencyLimiter ----------

    #[test]
    fn test_concurrency_limiter_acquire_success() {
        let mut cl = ConcurrencyLimiter::new(3);
        assert!(cl.try_acquire());
        assert_eq!(cl.active, 1);
    }

    #[test]
    fn test_concurrency_limiter_blocks_at_max() {
        let mut cl = ConcurrencyLimiter::new(2);
        cl.try_acquire();
        cl.try_acquire();
        assert!(!cl.try_acquire());
    }

    #[test]
    fn test_concurrency_limiter_release() {
        let mut cl = ConcurrencyLimiter::new(1);
        cl.try_acquire();
        cl.release();
        assert!(cl.try_acquire()); // slot is free again
    }

    #[test]
    fn test_concurrency_limiter_utilization() {
        let mut cl = ConcurrencyLimiter::new(4);
        cl.try_acquire();
        cl.try_acquire();
        let u = cl.utilization();
        assert!((u - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_concurrency_limiter_zero_max_utilization() {
        let cl = ConcurrencyLimiter::new(0);
        assert!((cl.utilization() - 0.0).abs() < f64::EPSILON);
    }

    // ---------- ThrottleStats ----------

    #[test]
    fn test_throttle_stats_rejection_rate_no_requests() {
        let s = ThrottleStats::default();
        assert!((s.rejection_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_throttle_stats_rejection_rate_half() {
        let s = ThrottleStats {
            accepted: 5,
            rejected: 5,
            total_wait_ms: 0,
        };
        assert!((s.rejection_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_throttle_stats_avg_wait_no_accepted() {
        let s = ThrottleStats {
            accepted: 0,
            rejected: 3,
            total_wait_ms: 100,
        };
        assert!((s.avg_wait_ms() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_throttle_stats_avg_wait() {
        let s = ThrottleStats {
            accepted: 4,
            rejected: 0,
            total_wait_ms: 200,
        };
        assert!((s.avg_wait_ms() - 50.0).abs() < f64::EPSILON);
    }
}
