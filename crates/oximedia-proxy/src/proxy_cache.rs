//! Proxy cache management: LRU eviction, TTL-based staleness, and utilisation tracking.
//!
//! Provides a simple in-memory cache that tracks proxy files by path, access
//! time, and hit count.  Eviction strategies include LRU (least-recently used),
//! TTL (time-to-live), and LFU (least-frequently used).

#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(clippy::cast_precision_loss)]

// ---------------------------------------------------------------------------
// CacheEntry
// ---------------------------------------------------------------------------

/// A single entry in the proxy cache.
#[derive(Debug, Clone, PartialEq)]
pub struct CacheEntry {
    /// File-system path of the cached proxy.
    pub path: String,
    /// Size of the proxy file in bytes.
    pub size_bytes: u64,
    /// Unix timestamp (milliseconds) of the most recent access.
    pub last_access_ms: u64,
    /// Number of times this entry has been accessed.
    pub hit_count: u32,
}

impl CacheEntry {
    /// Create a new cache entry.
    #[must_use]
    pub fn new(path: impl Into<String>, size_bytes: u64, now_ms: u64) -> Self {
        Self {
            path: path.into(),
            size_bytes,
            last_access_ms: now_ms,
            hit_count: 1,
        }
    }

    /// Returns `true` when the entry has not been accessed within `ttl_ms` milliseconds.
    ///
    /// # Arguments
    /// * `now_ms` – current time in Unix milliseconds.
    /// * `ttl_ms` – time-to-live threshold in milliseconds.
    #[must_use]
    pub fn is_stale(&self, now_ms: u64, ttl_ms: u64) -> bool {
        now_ms.saturating_sub(self.last_access_ms) > ttl_ms
    }
}

// ---------------------------------------------------------------------------
// CachePolicy
// ---------------------------------------------------------------------------

/// Eviction policy for the proxy cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CachePolicy {
    /// Evict the least-recently-used entry.
    Lru,
    /// Evict entries that have exceeded their time-to-live.
    Ttl,
    /// Evict the least-frequently-used entry.
    Lfu,
}

impl CachePolicy {
    /// Human-readable description of the policy.
    #[must_use]
    pub fn description(&self) -> &str {
        match self {
            Self::Lru => "Least Recently Used (LRU): evict the entry not accessed longest",
            Self::Ttl => "Time To Live (TTL): evict entries older than a configured threshold",
            Self::Lfu => "Least Frequently Used (LFU): evict the entry with the fewest accesses",
        }
    }
}

// ---------------------------------------------------------------------------
// ProxyCache
// ---------------------------------------------------------------------------

/// In-memory proxy cache with configurable capacity.
#[derive(Debug)]
pub struct ProxyCache {
    /// All cached entries.
    pub entries: Vec<CacheEntry>,
    /// Maximum allowed total size of the cache in bytes.
    pub max_size_bytes: u64,
    /// Current total size of all entries in bytes.
    pub used_bytes: u64,
}

impl ProxyCache {
    /// Create a new, empty cache with the given maximum size.
    #[must_use]
    pub fn new(max_size_bytes: u64) -> Self {
        Self {
            entries: Vec::new(),
            max_size_bytes,
            used_bytes: 0,
        }
    }

    /// Add a new entry to the cache.
    ///
    /// If an entry with the same path already exists it is replaced and the
    /// used-byte count is updated accordingly.
    pub fn add(&mut self, path: &str, size: u64, now_ms: u64) {
        // Remove existing entry with the same path to avoid duplicates.
        if let Some(pos) = self.entries.iter().position(|e| e.path == path) {
            let old_size = self.entries[pos].size_bytes;
            self.entries.remove(pos);
            self.used_bytes = self.used_bytes.saturating_sub(old_size);
        }
        self.entries.push(CacheEntry::new(path, size, now_ms));
        self.used_bytes += size;
    }

    /// Update the access timestamp and hit count for an entry.
    ///
    /// Returns `true` if the entry was found and updated.
    pub fn touch(&mut self, path: &str, now_ms: u64) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.path == path) {
            entry.last_access_ms = now_ms;
            entry.hit_count = entry.hit_count.saturating_add(1);
            true
        } else {
            false
        }
    }

    /// Remove the least-recently-used entry and return its path.
    ///
    /// Returns `None` if the cache is empty.
    pub fn evict_lru(&mut self) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        // Find the index of the entry with the smallest `last_access_ms`.
        let idx = self
            .entries
            .iter()
            .enumerate()
            .min_by_key(|(_, e)| e.last_access_ms)
            .map(|(i, _)| i)?;

        let removed = self.entries.remove(idx);
        self.used_bytes = self.used_bytes.saturating_sub(removed.size_bytes);
        Some(removed.path)
    }

    /// Remove all entries that are stale (last access older than `ttl_ms`).
    ///
    /// Returns the paths of all evicted entries.
    pub fn evict_stale(&mut self, now_ms: u64, ttl_ms: u64) -> Vec<String> {
        let mut evicted = Vec::new();
        self.entries.retain(|e| {
            if e.is_stale(now_ms, ttl_ms) {
                evicted.push(e.path.clone());
                false
            } else {
                true
            }
        });
        // Update used_bytes: subtract sizes of evicted entries.
        // We already removed them, so recalculate from remaining entries.
        self.used_bytes = self.entries.iter().map(|e| e.size_bytes).sum();
        evicted
    }

    /// Cache utilisation as a fraction in `[0.0, 1.0]`.
    ///
    /// Returns `0.0` when `max_size_bytes` is 0.
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.max_size_bytes == 0 {
            return 0.0;
        }
        (self.used_bytes as f64 / self.max_size_bytes as f64).min(1.0)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_entry_is_stale_fresh() {
        let entry = CacheEntry::new("p.mp4", 1024, 1_000);
        // TTL 5 000 ms, now 2 000 ms → only 1 000 ms old → not stale
        assert!(!entry.is_stale(2_000, 5_000));
    }

    #[test]
    fn test_cache_entry_is_stale_expired() {
        let entry = CacheEntry::new("p.mp4", 1024, 0);
        // TTL 500 ms, now 1 000 ms → 1 000 ms old → stale
        assert!(entry.is_stale(1_000, 500));
    }

    #[test]
    fn test_cache_entry_is_stale_exactly_at_ttl() {
        let entry = CacheEntry::new("p.mp4", 1024, 0);
        // Exactly at TTL boundary → not stale (> rather than >=)
        assert!(!entry.is_stale(500, 500));
    }

    #[test]
    fn test_cache_policy_descriptions_non_empty() {
        assert!(!CachePolicy::Lru.description().is_empty());
        assert!(!CachePolicy::Ttl.description().is_empty());
        assert!(!CachePolicy::Lfu.description().is_empty());
    }

    #[test]
    fn test_proxy_cache_add_single() {
        let mut cache = ProxyCache::new(1_000_000);
        cache.add("a.mp4", 100, 1_000);
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(cache.used_bytes, 100);
    }

    #[test]
    fn test_proxy_cache_add_replaces_existing() {
        let mut cache = ProxyCache::new(1_000_000);
        cache.add("a.mp4", 100, 1_000);
        cache.add("a.mp4", 200, 2_000);
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(cache.used_bytes, 200);
    }

    #[test]
    fn test_proxy_cache_touch_updates_access() {
        let mut cache = ProxyCache::new(1_000_000);
        cache.add("a.mp4", 100, 1_000);
        let updated = cache.touch("a.mp4", 5_000);
        assert!(updated);
        assert_eq!(cache.entries[0].last_access_ms, 5_000);
        assert_eq!(cache.entries[0].hit_count, 2);
    }

    #[test]
    fn test_proxy_cache_touch_missing_returns_false() {
        let mut cache = ProxyCache::new(1_000_000);
        assert!(!cache.touch("missing.mp4", 1_000));
    }

    #[test]
    fn test_proxy_cache_evict_lru_empty() {
        let mut cache = ProxyCache::new(1_000_000);
        assert!(cache.evict_lru().is_none());
    }

    #[test]
    fn test_proxy_cache_evict_lru_removes_oldest() {
        let mut cache = ProxyCache::new(1_000_000);
        cache.add("old.mp4", 100, 1_000);
        cache.add("new.mp4", 100, 9_000);
        let evicted = cache.evict_lru();
        assert_eq!(evicted, Some("old.mp4".to_string()));
        assert_eq!(cache.entries.len(), 1);
    }

    #[test]
    fn test_proxy_cache_evict_stale() {
        let mut cache = ProxyCache::new(1_000_000);
        cache.add("stale.mp4", 100, 0);
        cache.add("fresh.mp4", 200, 9_000);
        let evicted = cache.evict_stale(10_000, 5_000);
        assert_eq!(evicted.len(), 1);
        assert_eq!(evicted[0], "stale.mp4");
        assert_eq!(cache.used_bytes, 200);
    }

    #[test]
    fn test_proxy_cache_utilization() {
        let mut cache = ProxyCache::new(1_000);
        cache.add("a.mp4", 500, 0);
        let u = cache.utilization();
        assert!((u - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_proxy_cache_utilization_zero_max() {
        let cache = ProxyCache::new(0);
        assert_eq!(cache.utilization(), 0.0);
    }
}
