//! I/O rate limiting for bandwidth control.
//!
//! Provides a token-bucket rate limiter, a per-stream rate limiting manager,
//! and a sliding-window bandwidth tracker.

#![allow(dead_code)]

use std::collections::HashMap;

// ──────────────────────────────────────────────────────────────────────────────
// RateLimit
// ──────────────────────────────────────────────────────────────────────────────

/// Describes the rate limit for a single I/O stream
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateLimit {
    /// Sustained rate in bytes per second
    pub bytes_per_sec: u64,
    /// Maximum burst size in bytes (tokens that can accumulate)
    pub burst_bytes: u64,
}

impl RateLimit {
    /// Create a simple limit with no extra burst (burst == `bytes_per_sec`)
    #[must_use]
    pub fn new(bytes_per_sec: u64) -> Self {
        Self {
            bytes_per_sec,
            burst_bytes: bytes_per_sec,
        }
    }

    /// Create a limit with an explicit burst allowance
    #[must_use]
    pub fn with_burst(bytes_per_sec: u64, burst_bytes: u64) -> Self {
        Self {
            bytes_per_sec,
            burst_bytes,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// TokenBucket
// ──────────────────────────────────────────────────────────────────────────────

/// Token-bucket rate limiter
///
/// Tokens accumulate at `refill_rate` tokens/ms up to `capacity`.
/// Each byte consumed removes one token.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    /// Maximum number of tokens (== `burst_bytes`)
    pub capacity: f64,
    /// Current token count
    pub tokens: f64,
    /// Token refill rate in tokens per millisecond
    pub refill_rate: f64,
    /// Timestamp (ms) of the last refill
    pub last_refill_ms: u64,
}

impl TokenBucket {
    /// Create a new bucket from a `RateLimit`.
    ///
    /// `now_ms` should be the current time in milliseconds (e.g. from a
    /// monotonic clock or test fixture).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn new(limit: RateLimit, now_ms: u64) -> Self {
        let refill_rate = limit.bytes_per_sec as f64 / 1000.0; // per ms
        Self {
            capacity: limit.burst_bytes as f64,
            tokens: limit.burst_bytes as f64, // start full
            refill_rate,
            last_refill_ms: now_ms,
        }
    }

    /// Attempt to consume `bytes` tokens at time `now_ms`.
    ///
    /// Refills the bucket first, then checks availability.
    /// Returns `true` if the tokens were consumed, `false` if insufficient.
    #[allow(clippy::cast_precision_loss)]
    pub fn try_consume(&mut self, bytes: u64, now_ms: u64) -> bool {
        // Refill
        if now_ms > self.last_refill_ms {
            let elapsed_ms = (now_ms - self.last_refill_ms) as f64;
            self.tokens = (self.tokens + elapsed_ms * self.refill_rate).min(self.capacity);
            self.last_refill_ms = now_ms;
        }

        if self.tokens >= bytes as f64 {
            self.tokens -= bytes as f64;
            true
        } else {
            false
        }
    }

    /// Estimate how many milliseconds until `bytes` tokens are available
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn wait_ms_for(&self, bytes: u64) -> u64 {
        let needed = bytes as f64 - self.tokens;
        if needed <= 0.0 || self.refill_rate <= 0.0 {
            return 0;
        }
        (needed / self.refill_rate).ceil() as u64
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// RateLimitResult
// ──────────────────────────────────────────────────────────────────────────────

/// Result returned by `RateLimiter::check_and_consume`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitResult {
    /// The bytes were consumed; the caller may proceed immediately
    Allowed,
    /// The bucket is insufficient; caller should wait `wait_ms` milliseconds
    Throttled(u64),
}

impl RateLimitResult {
    /// Returns `true` if the operation is allowed
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// RateLimiter
// ──────────────────────────────────────────────────────────────────────────────

/// Per-stream rate limiter managing multiple `TokenBucket`s
pub struct RateLimiter {
    buckets: HashMap<String, TokenBucket>,
}

impl RateLimiter {
    /// Create a new, empty rate limiter
    #[must_use]
    pub fn new() -> Self {
        Self {
            buckets: HashMap::new(),
        }
    }

    /// Add or replace a rate limit for the stream identified by `id`
    pub fn add_stream(&mut self, id: &str, limit: RateLimit, now_ms: u64) {
        let bucket = TokenBucket::new(limit, now_ms);
        self.buckets.insert(id.to_string(), bucket);
    }

    /// Attempt to consume `bytes` from the bucket for stream `id`.
    ///
    /// Returns `Throttled(wait_ms)` if the bucket is insufficient, or
    /// `Allowed` if the bytes were successfully consumed.
    ///
    /// If no bucket is registered for `id`, this always returns `Allowed`.
    pub fn check_and_consume(&mut self, id: &str, bytes: u64, now_ms: u64) -> RateLimitResult {
        let Some(bucket) = self.buckets.get_mut(id) else {
            return RateLimitResult::Allowed;
        };

        if bucket.try_consume(bytes, now_ms) {
            RateLimitResult::Allowed
        } else {
            let wait_ms = bucket.wait_ms_for(bytes);
            RateLimitResult::Throttled(wait_ms)
        }
    }

    /// Remove the rate limit for the stream identified by `id`
    pub fn remove_stream(&mut self, id: &str) {
        self.buckets.remove(id);
    }

    /// Returns the number of registered streams
    #[must_use]
    pub fn stream_count(&self) -> usize {
        self.buckets.len()
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// BandwidthTracker — sliding window
// ──────────────────────────────────────────────────────────────────────────────

/// Tracks bandwidth with a sliding time window
///
/// Observations older than `window_ms` milliseconds are discarded when
/// querying `current_bps`.
pub struct BandwidthTracker {
    /// Sliding window duration in milliseconds
    window_ms: u64,
    /// Observations stored as (`timestamp_ms`, bytes)
    observations: Vec<(u64, u64)>,
}

impl BandwidthTracker {
    /// Create a tracker with the given sliding window duration
    #[must_use]
    pub fn new(window_ms: u64) -> Self {
        Self {
            window_ms,
            observations: Vec::new(),
        }
    }

    /// Record that `bytes` were transferred at time `now_ms`
    pub fn record(&mut self, bytes: u64, now_ms: u64) {
        self.observations.push((now_ms, bytes));
        // Prune old observations eagerly to avoid unbounded growth
        self.prune(now_ms);
    }

    /// Compute the current bandwidth in bytes per second.
    ///
    /// Returns 0 if no observations are within the window.
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn current_bps(&self, now_ms: u64) -> u64 {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        let total_bytes: u64 = self
            .observations
            .iter()
            .filter(|(ts, _)| *ts >= cutoff)
            .map(|(_, bytes)| bytes)
            .sum();

        if self.window_ms == 0 {
            return 0;
        }

        // Convert window from ms to seconds for bytes/sec calculation
        let window_sec = self.window_ms as f64 / 1000.0;
        (total_bytes as f64 / window_sec) as u64
    }

    /// Total bytes recorded in the current window
    #[must_use]
    pub fn total_bytes_in_window(&self, now_ms: u64) -> u64 {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.observations
            .iter()
            .filter(|(ts, _)| *ts >= cutoff)
            .map(|(_, bytes)| bytes)
            .sum()
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.observations.retain(|(ts, _)| *ts >= cutoff);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── TokenBucket ────────────────────────────────────────────────────────────

    #[test]
    fn test_token_bucket_starts_full() {
        let limit = RateLimit::with_burst(1000, 5000);
        let bucket = TokenBucket::new(limit, 0);
        assert!((bucket.tokens - 5000.0).abs() < f64::EPSILON);
        assert!((bucket.capacity - 5000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_token_bucket_consume_success() {
        let limit = RateLimit::new(1_000_000);
        let mut bucket = TokenBucket::new(limit, 0);
        assert!(bucket.try_consume(500_000, 0));
        assert!((bucket.tokens - 500_000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_token_bucket_consume_fail_insufficient() {
        let limit = RateLimit::new(1000);
        let mut bucket = TokenBucket::new(limit, 0);
        // Immediately consume everything first
        bucket.try_consume(1000, 0);
        // No tokens left, should fail
        assert!(!bucket.try_consume(1, 0));
    }

    #[test]
    fn test_token_bucket_refill_over_time() {
        let limit = RateLimit::new(1000); // 1 token/ms
        let mut bucket = TokenBucket::new(limit, 0);
        bucket.try_consume(1000, 0); // drain
                                     // After 500 ms, 500 tokens should have refilled
        assert!(bucket.try_consume(500, 500));
    }

    #[test]
    fn test_token_bucket_capped_at_capacity() {
        let limit = RateLimit::new(100);
        let mut bucket = TokenBucket::new(limit, 0);
        // Pass 100 seconds — tokens should NOT exceed capacity
        bucket.try_consume(0, 100_000);
        assert!(bucket.tokens <= bucket.capacity + f64::EPSILON);
    }

    #[test]
    fn test_token_bucket_wait_ms() {
        let limit = RateLimit::new(1000); // 1 token/ms
        let mut bucket = TokenBucket::new(limit, 0);
        bucket.try_consume(1000, 0); // drain
        let wait = bucket.wait_ms_for(500);
        assert!(wait >= 500);
    }

    // ── RateLimitResult ────────────────────────────────────────────────────────

    #[test]
    fn test_rate_limit_result_is_allowed() {
        assert!(RateLimitResult::Allowed.is_allowed());
        assert!(!RateLimitResult::Throttled(100).is_allowed());
    }

    // ── RateLimiter ────────────────────────────────────────────────────────────

    #[test]
    fn test_rate_limiter_no_stream_always_allowed() {
        let mut rl = RateLimiter::new();
        assert_eq!(
            rl.check_and_consume("unknown", 1_000_000, 0),
            RateLimitResult::Allowed
        );
    }

    #[test]
    fn test_rate_limiter_stream_allowed() {
        let mut rl = RateLimiter::new();
        rl.add_stream("s1", RateLimit::new(10_000), 0);
        assert_eq!(
            rl.check_and_consume("s1", 5_000, 0),
            RateLimitResult::Allowed
        );
    }

    #[test]
    fn test_rate_limiter_stream_throttled() {
        let mut rl = RateLimiter::new();
        rl.add_stream("s2", RateLimit::new(100), 0);
        rl.check_and_consume("s2", 100, 0); // drain
        let result = rl.check_and_consume("s2", 50, 0);
        assert!(matches!(result, RateLimitResult::Throttled(_)));
    }

    #[test]
    fn test_rate_limiter_remove_stream() {
        let mut rl = RateLimiter::new();
        rl.add_stream("s3", RateLimit::new(100), 0);
        rl.remove_stream("s3");
        assert_eq!(rl.stream_count(), 0);
        // Now the stream is gone; should be allowed
        assert_eq!(
            rl.check_and_consume("s3", 99999, 0),
            RateLimitResult::Allowed
        );
    }

    // ── BandwidthTracker ───────────────────────────────────────────────────────

    #[test]
    fn test_bandwidth_tracker_empty() {
        let tracker = BandwidthTracker::new(1000);
        assert_eq!(tracker.current_bps(0), 0);
    }

    #[test]
    fn test_bandwidth_tracker_single_observation() {
        let mut tracker = BandwidthTracker::new(1000); // 1-second window
        tracker.record(500_000, 500); // 500 KB at t=500ms
                                      // 500 KB / 1 s = 500_000 bps
        assert_eq!(tracker.current_bps(1000), 500_000);
    }

    #[test]
    fn test_bandwidth_tracker_old_observations_pruned() {
        let mut tracker = BandwidthTracker::new(1000);
        tracker.record(1_000_000, 0); // at t=0
                                      // At t=2000, the window covers [1000, 2000]; observation at 0 is outside
        assert_eq!(tracker.current_bps(2000), 0);
    }

    #[test]
    fn test_bandwidth_tracker_total_bytes_in_window() {
        let mut tracker = BandwidthTracker::new(2000);
        tracker.record(100, 0);
        tracker.record(200, 1000);
        tracker.record(400, 2000);
        // At now=2000 window covers [0, 2000]; all three are included
        assert_eq!(tracker.total_bytes_in_window(2000), 700);
    }
}
