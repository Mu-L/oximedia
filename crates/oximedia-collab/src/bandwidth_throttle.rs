#![allow(dead_code)]
//! Adaptive sync bandwidth management for collaboration sessions.
//!
//! Provides token-bucket throttling, per-user rate limits, and adaptive
//! bandwidth allocation to prevent any single participant from saturating
//! the sync channel.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A token bucket for rate limiting.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    /// Maximum tokens the bucket can hold.
    pub capacity: u64,
    /// Current number of available tokens.
    pub tokens: u64,
    /// Tokens added per second.
    pub refill_rate: f64,
    /// Last time the bucket was refilled.
    pub last_refill: Instant,
}

impl TokenBucket {
    /// Create a new token bucket.
    pub fn new(capacity: u64, refill_rate: f64) -> Self {
        Self {
            capacity,
            tokens: capacity,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    /// Refill tokens based on elapsed time.
    #[allow(clippy::cast_precision_loss)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    pub fn refill(&mut self) {
        let elapsed = self.last_refill.elapsed().as_secs_f64();
        let new_tokens = (elapsed * self.refill_rate) as u64;
        if new_tokens > 0 {
            self.tokens = (self.tokens + new_tokens).min(self.capacity);
            self.last_refill = Instant::now();
        }
    }

    /// Try to consume `amount` tokens. Returns true if successful.
    pub fn try_consume(&mut self, amount: u64) -> bool {
        self.refill();
        if self.tokens >= amount {
            self.tokens -= amount;
            true
        } else {
            false
        }
    }

    /// Get how many tokens are currently available.
    pub fn available(&mut self) -> u64 {
        self.refill();
        self.tokens
    }

    /// Time until the requested amount of tokens becomes available.
    #[allow(clippy::cast_precision_loss)]
    pub fn time_until_available(&mut self, amount: u64) -> Duration {
        self.refill();
        if self.tokens >= amount {
            return Duration::ZERO;
        }
        let deficit = amount - self.tokens;
        if self.refill_rate <= 0.0 {
            return Duration::from_secs(u64::MAX);
        }
        let secs = deficit as f64 / self.refill_rate;
        Duration::from_secs_f64(secs)
    }
}

/// Throttle tier for adaptive bandwidth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThrottleTier {
    /// No throttling.
    None,
    /// Light throttling (75% bandwidth).
    Light,
    /// Moderate throttling (50% bandwidth).
    Moderate,
    /// Heavy throttling (25% bandwidth).
    Heavy,
    /// Paused (0% bandwidth, only control messages).
    Paused,
}

impl std::fmt::Display for ThrottleTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Light => write!(f, "Light"),
            Self::Moderate => write!(f, "Moderate"),
            Self::Heavy => write!(f, "Heavy"),
            Self::Paused => write!(f, "Paused"),
        }
    }
}

impl ThrottleTier {
    /// Get the bandwidth multiplier for this tier (0.0 to 1.0).
    pub fn multiplier(&self) -> f64 {
        match self {
            Self::None => 1.0,
            Self::Light => 0.75,
            Self::Moderate => 0.5,
            Self::Heavy => 0.25,
            Self::Paused => 0.0,
        }
    }
}

/// Per-user bandwidth state.
#[derive(Debug)]
pub struct UserBandwidth {
    /// User identifier.
    pub user_id: String,
    /// Token bucket for this user.
    pub bucket: TokenBucket,
    /// Current throttle tier.
    pub tier: ThrottleTier,
    /// Total bytes sent.
    pub total_bytes_sent: u64,
    /// Total messages sent.
    pub total_messages_sent: u64,
    /// When tracking started.
    pub tracking_start: Instant,
}

impl UserBandwidth {
    /// Create new user bandwidth tracking.
    pub fn new(user_id: &str, capacity: u64, refill_rate: f64) -> Self {
        Self {
            user_id: user_id.to_string(),
            bucket: TokenBucket::new(capacity, refill_rate),
            tier: ThrottleTier::None,
            total_bytes_sent: 0,
            total_messages_sent: 0,
            tracking_start: Instant::now(),
        }
    }

    /// Average bytes per second since tracking started.
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_bytes_per_sec(&self) -> f64 {
        let elapsed = self.tracking_start.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.total_bytes_sent as f64 / elapsed
        } else {
            0.0
        }
    }
}

/// Configuration for the bandwidth throttle.
#[derive(Debug, Clone)]
pub struct ThrottleConfig {
    /// Default token bucket capacity per user (bytes).
    pub default_capacity: u64,
    /// Default refill rate per user (bytes per second).
    pub default_refill_rate: f64,
    /// Threshold (bytes/sec) above which light throttling kicks in.
    pub light_threshold: f64,
    /// Threshold for moderate throttling.
    pub moderate_threshold: f64,
    /// Threshold for heavy throttling.
    pub heavy_threshold: f64,
    /// Global bandwidth limit (bytes per second).
    pub global_limit: f64,
}

impl Default for ThrottleConfig {
    fn default() -> Self {
        Self {
            default_capacity: 1_048_576,    // 1 MB
            default_refill_rate: 524_288.0, // 512 KB/s
            light_threshold: 262_144.0,     // 256 KB/s
            moderate_threshold: 524_288.0,  // 512 KB/s
            heavy_threshold: 1_048_576.0,   // 1 MB/s
            global_limit: 10_485_760.0,     // 10 MB/s
        }
    }
}

/// Bandwidth throttle manager.
#[derive(Debug)]
pub struct BandwidthThrottle {
    /// Configuration.
    config: ThrottleConfig,
    /// Per-user bandwidth state.
    users: HashMap<String, UserBandwidth>,
    /// Global token bucket.
    global_bucket: TokenBucket,
}

impl BandwidthThrottle {
    /// Create a new bandwidth throttle.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    pub fn new(config: ThrottleConfig) -> Self {
        let global_cap = config.global_limit as u64;
        let global_rate = config.global_limit;
        Self {
            config,
            users: HashMap::new(),
            global_bucket: TokenBucket::new(global_cap, global_rate),
        }
    }

    /// Register a user.
    pub fn register_user(&mut self, user_id: &str) {
        let bw = UserBandwidth::new(
            user_id,
            self.config.default_capacity,
            self.config.default_refill_rate,
        );
        self.users.insert(user_id.to_string(), bw);
    }

    /// Remove a user.
    pub fn unregister_user(&mut self, user_id: &str) {
        self.users.remove(user_id);
    }

    /// Try to send `bytes` for a given user. Returns true if allowed.
    pub fn try_send(&mut self, user_id: &str, bytes: u64) -> bool {
        // Check global bucket first
        if !self.global_bucket.try_consume(bytes) {
            return false;
        }
        if let Some(user) = self.users.get_mut(user_id) {
            if user.tier == ThrottleTier::Paused {
                return false;
            }
            if user.bucket.try_consume(bytes) {
                user.total_bytes_sent += bytes;
                user.total_messages_sent += 1;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Evaluate and update throttle tier for a user.
    pub fn evaluate_tier(&mut self, user_id: &str) {
        if let Some(user) = self.users.get_mut(user_id) {
            let rate = user.avg_bytes_per_sec();
            user.tier = if rate >= self.config.heavy_threshold {
                ThrottleTier::Heavy
            } else if rate >= self.config.moderate_threshold {
                ThrottleTier::Moderate
            } else if rate >= self.config.light_threshold {
                ThrottleTier::Light
            } else {
                ThrottleTier::None
            };
        }
    }

    /// Get the current tier for a user.
    pub fn get_tier(&self, user_id: &str) -> Option<ThrottleTier> {
        self.users.get(user_id).map(|u| u.tier)
    }

    /// Get total bytes sent by a user.
    pub fn user_bytes_sent(&self, user_id: &str) -> Option<u64> {
        self.users.get(user_id).map(|u| u.total_bytes_sent)
    }

    /// Get the number of tracked users.
    pub fn user_count(&self) -> usize {
        self.users.len()
    }

    /// Manually set a user's tier.
    pub fn set_tier(&mut self, user_id: &str, tier: ThrottleTier) {
        if let Some(user) = self.users.get_mut(user_id) {
            user.tier = tier;
        }
    }

    /// Reset a user's statistics.
    pub fn reset_user_stats(&mut self, user_id: &str) {
        if let Some(user) = self.users.get_mut(user_id) {
            user.total_bytes_sent = 0;
            user.total_messages_sent = 0;
            user.tracking_start = Instant::now();
            user.tier = ThrottleTier::None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_creation() {
        let bucket = TokenBucket::new(100, 10.0);
        assert_eq!(bucket.capacity, 100);
        assert_eq!(bucket.tokens, 100);
    }

    #[test]
    fn test_token_bucket_consume() {
        let mut bucket = TokenBucket::new(100, 10.0);
        assert!(bucket.try_consume(50));
        assert_eq!(bucket.tokens, 50);
        assert!(bucket.try_consume(50));
        assert!(!bucket.try_consume(1));
    }

    #[test]
    fn test_token_bucket_available() {
        let mut bucket = TokenBucket::new(100, 10.0);
        bucket.try_consume(30);
        assert_eq!(bucket.available(), 70);
    }

    #[test]
    fn test_throttle_tier_multiplier() {
        assert!((ThrottleTier::None.multiplier() - 1.0).abs() < f64::EPSILON);
        assert!((ThrottleTier::Light.multiplier() - 0.75).abs() < f64::EPSILON);
        assert!((ThrottleTier::Moderate.multiplier() - 0.5).abs() < f64::EPSILON);
        assert!((ThrottleTier::Heavy.multiplier() - 0.25).abs() < f64::EPSILON);
        assert!((ThrottleTier::Paused.multiplier() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_throttle_tier_display() {
        assert_eq!(ThrottleTier::None.to_string(), "None");
        assert_eq!(ThrottleTier::Light.to_string(), "Light");
        assert_eq!(ThrottleTier::Moderate.to_string(), "Moderate");
        assert_eq!(ThrottleTier::Heavy.to_string(), "Heavy");
        assert_eq!(ThrottleTier::Paused.to_string(), "Paused");
    }

    #[test]
    fn test_register_and_send() {
        let mut throttle = BandwidthThrottle::new(ThrottleConfig::default());
        throttle.register_user("alice");
        assert!(throttle.try_send("alice", 1024));
        assert_eq!(throttle.user_bytes_sent("alice"), Some(1024));
    }

    #[test]
    fn test_unregistered_user_rejected() {
        let mut throttle = BandwidthThrottle::new(ThrottleConfig::default());
        assert!(!throttle.try_send("unknown", 100));
    }

    #[test]
    fn test_paused_user_blocked() {
        let mut throttle = BandwidthThrottle::new(ThrottleConfig::default());
        throttle.register_user("bob");
        throttle.set_tier("bob", ThrottleTier::Paused);
        assert!(!throttle.try_send("bob", 1));
    }

    #[test]
    fn test_user_count() {
        let mut throttle = BandwidthThrottle::new(ThrottleConfig::default());
        throttle.register_user("a");
        throttle.register_user("b");
        assert_eq!(throttle.user_count(), 2);
        throttle.unregister_user("a");
        assert_eq!(throttle.user_count(), 1);
    }

    #[test]
    fn test_get_tier_default() {
        let mut throttle = BandwidthThrottle::new(ThrottleConfig::default());
        throttle.register_user("x");
        assert_eq!(throttle.get_tier("x"), Some(ThrottleTier::None));
    }

    #[test]
    fn test_set_tier() {
        let mut throttle = BandwidthThrottle::new(ThrottleConfig::default());
        throttle.register_user("x");
        throttle.set_tier("x", ThrottleTier::Heavy);
        assert_eq!(throttle.get_tier("x"), Some(ThrottleTier::Heavy));
    }

    #[test]
    fn test_reset_user_stats() {
        let mut throttle = BandwidthThrottle::new(ThrottleConfig::default());
        throttle.register_user("y");
        throttle.try_send("y", 500);
        throttle.set_tier("y", ThrottleTier::Moderate);
        throttle.reset_user_stats("y");
        assert_eq!(throttle.user_bytes_sent("y"), Some(0));
        assert_eq!(throttle.get_tier("y"), Some(ThrottleTier::None));
    }

    #[test]
    fn test_time_until_available_immediate() {
        let mut bucket = TokenBucket::new(100, 10.0);
        let dur = bucket.time_until_available(50);
        assert_eq!(dur, Duration::ZERO);
    }

    #[test]
    fn test_throttle_tier_ordering() {
        assert!(ThrottleTier::None < ThrottleTier::Light);
        assert!(ThrottleTier::Light < ThrottleTier::Moderate);
        assert!(ThrottleTier::Moderate < ThrottleTier::Heavy);
        assert!(ThrottleTier::Heavy < ThrottleTier::Paused);
    }
}
