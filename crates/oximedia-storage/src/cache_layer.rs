//! Storage cache layer: LRU cache with policy tracking and statistics.

use std::collections::{HashMap, VecDeque};

// ---------------------------------------------------------------------------
// Cache policy
// ---------------------------------------------------------------------------

/// Eviction / replacement policy for a cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachePolicy {
    /// Least Recently Used.
    LRU,
    /// Least Frequently Used.
    LFU,
    /// First In, First Out.
    FIFO,
    /// Adaptive Replacement Cache.
    ARC,
}

impl CachePolicy {
    /// Returns a human-readable name for the policy.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::LRU => "LRU",
            Self::LFU => "LFU",
            Self::FIFO => "FIFO",
            Self::ARC => "ARC",
        }
    }
}

// ---------------------------------------------------------------------------
// Cache entry
// ---------------------------------------------------------------------------

/// Metadata for a single cache entry.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Unique identifier / key for the cached object.
    pub key: String,
    /// Size of the cached object in bytes.
    pub size_bytes: u64,
    /// Number of times this entry has been accessed.
    pub access_count: u64,
    /// Timestamp (in milliseconds) of the last access.
    pub last_access_ms: u64,
    /// Timestamp (in milliseconds) when the entry was created.
    pub created_ms: u64,
}

impl CacheEntry {
    /// Create a new cache entry.
    #[must_use]
    pub fn new(key: impl Into<String>, size_bytes: u64, now_ms: u64) -> Self {
        Self {
            key: key.into(),
            size_bytes,
            access_count: 0,
            last_access_ms: now_ms,
            created_ms: now_ms,
        }
    }

    /// Age of the entry in milliseconds relative to `now`.
    #[must_use]
    pub fn age_ms(&self, now: u64) -> u64 {
        now.saturating_sub(self.created_ms)
    }
}

// ---------------------------------------------------------------------------
// LRU cache
// ---------------------------------------------------------------------------

/// An LRU cache with a byte-capacity limit.
pub struct LruCache {
    /// Maximum total size in bytes.
    pub capacity_bytes: u64,
    /// Current total size in bytes.
    pub used_bytes: u64,
    /// Map from key to entry.
    pub entries: HashMap<String, CacheEntry>,
    /// LRU order: front = most recently used, back = least recently used.
    order: VecDeque<String>,
}

impl LruCache {
    /// Create a new LRU cache with the given byte capacity.
    #[must_use]
    pub fn new(capacity_bytes: u64) -> Self {
        Self {
            capacity_bytes,
            used_bytes: 0,
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Retrieve an entry by key, updating its access timestamp.
    ///
    /// Returns `None` if the key is not cached.
    pub fn get(&mut self, key: &str, now_ms: u64) -> Option<&CacheEntry> {
        if !self.entries.contains_key(key) {
            return None;
        }

        // Move to front of LRU order
        self.order.retain(|k| k != key);
        self.order.push_front(key.to_string());

        if let Some(entry) = self.entries.get_mut(key) {
            entry.access_count += 1;
            entry.last_access_ms = now_ms;
        }

        self.entries.get(key)
    }

    /// Insert (or refresh) a cache entry.
    ///
    /// Evicts entries as needed to satisfy the byte capacity.
    pub fn put(&mut self, key: impl Into<String>, size_bytes: u64, now_ms: u64) {
        let key = key.into();

        // Remove existing entry of the same key first
        if let Some(old) = self.entries.remove(&key) {
            self.used_bytes = self.used_bytes.saturating_sub(old.size_bytes);
            self.order.retain(|k| k != &key);
        }

        // Evict until there is room
        while self.used_bytes + size_bytes > self.capacity_bytes && !self.order.is_empty() {
            self.evict();
        }

        let entry = CacheEntry::new(key.clone(), size_bytes, now_ms);
        self.used_bytes += size_bytes;
        self.entries.insert(key.clone(), entry);
        self.order.push_front(key);
    }

    /// Evict the least recently used entry.
    ///
    /// Returns `true` if an entry was evicted.
    pub fn evict(&mut self) -> bool {
        if let Some(lru_key) = self.order.pop_back() {
            if let Some(entry) = self.entries.remove(&lru_key) {
                self.used_bytes = self.used_bytes.saturating_sub(entry.size_bytes);
                return true;
            }
        }
        false
    }

    /// Cache utilisation as a fraction (0.0–1.0).
    ///
    /// Returns `0.0` when capacity is zero.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn utilization(&self) -> f64 {
        if self.capacity_bytes == 0 {
            return 0.0;
        }
        self.used_bytes as f64 / self.capacity_bytes as f64
    }

    /// Number of entries currently in the cache.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the cache contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Cache statistics
// ---------------------------------------------------------------------------

/// Accumulates cache hit/miss/eviction counters.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Number of cache hits.
    pub hits: u64,
    /// Number of cache misses.
    pub misses: u64,
    /// Number of entries evicted.
    pub evictions: u64,
}

impl CacheStats {
    /// Fraction of lookups that were hits (0.0–1.0).
    ///
    /// Returns `0.0` when no lookups have been performed.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Record a cache hit.
    pub fn record_hit(&mut self) {
        self.hits += 1;
    }

    /// Record a cache miss.
    pub fn record_miss(&mut self) {
        self.misses += 1;
    }

    /// Record an eviction.
    pub fn record_eviction(&mut self) {
        self.evictions += 1;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_policy_names() {
        assert_eq!(CachePolicy::LRU.name(), "LRU");
        assert_eq!(CachePolicy::LFU.name(), "LFU");
        assert_eq!(CachePolicy::FIFO.name(), "FIFO");
        assert_eq!(CachePolicy::ARC.name(), "ARC");
    }

    #[test]
    fn test_cache_entry_age() {
        let entry = CacheEntry::new("key", 128, 1000);
        assert_eq!(entry.age_ms(1500), 500);
        assert_eq!(entry.age_ms(999), 0); // saturating sub
    }

    #[test]
    fn test_lru_cache_put_and_get() {
        let mut cache = LruCache::new(1024);
        cache.put("file.mp4", 100, 0);
        assert!(cache.get("file.mp4", 1).is_some());
    }

    #[test]
    fn test_lru_cache_miss() {
        let mut cache = LruCache::new(1024);
        assert!(cache.get("missing", 0).is_none());
    }

    #[test]
    fn test_lru_cache_eviction_on_overflow() {
        let mut cache = LruCache::new(200);
        cache.put("a", 100, 0);
        cache.put("b", 100, 1);
        // Both fit; now add one more that requires evicting 'a' (LRU)
        cache.put("c", 100, 2);
        assert_eq!(cache.len(), 2);
        // 'a' should have been evicted
        assert!(cache.get("a", 3).is_none());
        assert!(cache.get("b", 3).is_some());
        assert!(cache.get("c", 3).is_some());
    }

    #[test]
    fn test_lru_cache_access_updates_order() {
        let mut cache = LruCache::new(200);
        cache.put("a", 100, 0);
        cache.put("b", 100, 1);
        // Access 'a' to make it recently used
        cache.get("a", 2);
        // Now insert 'c'; 'b' is LRU and should be evicted
        cache.put("c", 100, 3);
        assert!(cache.get("a", 4).is_some());
        assert!(cache.get("b", 4).is_none());
        assert!(cache.get("c", 4).is_some());
    }

    #[test]
    fn test_lru_cache_utilization() {
        let mut cache = LruCache::new(1000);
        cache.put("x", 500, 0);
        assert!((cache.utilization() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_lru_cache_utilization_empty() {
        let cache = LruCache::new(0);
        assert_eq!(cache.utilization(), 0.0);
    }

    #[test]
    fn test_lru_cache_overwrite_same_key() {
        let mut cache = LruCache::new(1000);
        cache.put("k", 100, 0);
        cache.put("k", 200, 1); // overwrite
        assert_eq!(cache.used_bytes, 200);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_lru_cache_is_empty() {
        let cache = LruCache::new(1024);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_stats_hit_rate_no_lookups() {
        let stats = CacheStats::default();
        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_cache_stats_hit_rate_all_hits() {
        let mut stats = CacheStats::default();
        stats.record_hit();
        stats.record_hit();
        assert!((stats.hit_rate() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_cache_stats_hit_rate_mixed() {
        let mut stats = CacheStats::default();
        stats.record_hit();
        stats.record_miss();
        assert!((stats.hit_rate() - 0.5).abs() < 1e-9);
    }
}
