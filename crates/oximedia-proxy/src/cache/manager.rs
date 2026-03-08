//! Proxy cache manager for intelligent caching.

use super::strategy::CacheStrategy;
use std::collections::HashMap;
use std::path::PathBuf;

/// Proxy cache manager.
pub struct CacheManager {
    /// Cache directory.
    #[allow(dead_code)]
    cache_dir: PathBuf,

    /// Cache strategy.
    strategy: CacheStrategy,

    /// Cached items.
    cache: HashMap<PathBuf, CacheEntry>,

    /// Maximum cache size in bytes.
    max_size: u64,

    /// Current cache size in bytes.
    current_size: u64,
}

impl CacheManager {
    /// Create a new cache manager.
    #[must_use]
    pub fn new(cache_dir: PathBuf, max_size: u64) -> Self {
        Self {
            cache_dir,
            strategy: CacheStrategy::Lru,
            cache: HashMap::new(),
            max_size,
            current_size: 0,
        }
    }

    /// Set the cache strategy.
    pub fn set_strategy(&mut self, strategy: CacheStrategy) {
        self.strategy = strategy;
    }

    /// Add a proxy to the cache.
    pub fn add(&mut self, path: PathBuf, size: u64) {
        if self.current_size + size > self.max_size {
            self.evict(size);
        }

        let entry = CacheEntry {
            path: path.clone(),
            size,
            access_count: 0,
            last_access: current_timestamp(),
        };

        self.cache.insert(path, entry);
        self.current_size += size;
    }

    /// Check if a proxy is in the cache.
    #[must_use]
    pub fn contains(&self, path: &PathBuf) -> bool {
        self.cache.contains_key(path)
    }

    /// Mark a proxy as accessed.
    pub fn access(&mut self, path: &PathBuf) {
        if let Some(entry) = self.cache.get_mut(path) {
            entry.access_count += 1;
            entry.last_access = current_timestamp();
        }
    }

    /// Evict items to make room for new size.
    fn evict(&mut self, needed_size: u64) {
        let mut freed = 0u64;
        let mut to_remove = Vec::new();

        // Sort entries by strategy
        let mut entries: Vec<_> = self.cache.values().collect();
        entries.sort_by(|a, b| match self.strategy {
            CacheStrategy::Lru => a.last_access.cmp(&b.last_access),
            CacheStrategy::Lfu => a.access_count.cmp(&b.access_count),
            CacheStrategy::Fifo => a.last_access.cmp(&b.last_access),
        });

        for entry in entries {
            if freed >= needed_size {
                break;
            }
            to_remove.push(entry.path.clone());
            freed += entry.size;
        }

        for path in to_remove {
            if let Some(entry) = self.cache.remove(&path) {
                self.current_size -= entry.size;
            }
        }
    }

    /// Get current cache size.
    #[must_use]
    pub const fn current_size(&self) -> u64 {
        self.current_size
    }

    /// Get cache utilization percentage.
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.max_size == 0 {
            0.0
        } else {
            (self.current_size as f64 / self.max_size as f64) * 100.0
        }
    }
}

/// Cache entry.
#[derive(Debug, Clone)]
struct CacheEntry {
    path: PathBuf,
    size: u64,
    access_count: u64,
    last_access: i64,
}

fn current_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("infallible: system clock is always after UNIX_EPOCH")
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_manager() {
        let temp_dir = std::env::temp_dir();
        let mut manager = CacheManager::new(temp_dir, 1000);

        manager.add(PathBuf::from("proxy1.mp4"), 100);
        assert_eq!(manager.current_size(), 100);

        manager.add(PathBuf::from("proxy2.mp4"), 200);
        assert_eq!(manager.current_size(), 300);
    }

    #[test]
    fn test_cache_eviction() {
        let temp_dir = std::env::temp_dir();
        let mut manager = CacheManager::new(temp_dir, 500);

        manager.add(PathBuf::from("proxy1.mp4"), 200);
        manager.add(PathBuf::from("proxy2.mp4"), 200);
        manager.add(PathBuf::from("proxy3.mp4"), 200);

        // Should have evicted oldest entries
        assert!(manager.current_size() <= 500);
    }
}
