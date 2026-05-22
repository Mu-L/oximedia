# oximedia-cache TODO

## Current Status
- 7 modules: `lru_cache`, `tiered_cache`, `cache_warming`, `bloom_filter`, `distributed_cache`, `eviction_policies`, `content_aware_cache`
- Zero external dependencies beyond `thiserror` (pure Rust, minimal footprint)
- LRU cache with O(1) operations, tiered multi-level cache with pluggable eviction (LRU/LFU/FIFO/Random/TinyLFU)
- Distributed cache with consistent-hash ring and quorum replication

## Enhancements
- [x] Add TTL (time-to-live) expiration support to `lru_cache::LruCache` entries
- [ ] Extend `tiered_cache` with async disk tier using file-backed storage (not just simulated) (verified-open 2026-05-16: tiered_cache has simulated disk tier, not actual file I/O)
- [x] Add cache entry pinning in `lru_cache` to prevent eviction of critical items
- [ ] Implement adaptive tier promotion thresholds in `tiered_cache` based on access frequency (verified-open 2026-05-16: promotion_threshold is a fixed config field, not dynamically adapted)
- [x] Extend `bloom_filter` with scalable Bloom filter variant that grows as elements are added
- [x] Add `distributed_cache` virtual node support for better hash ring distribution (verified 2026-05-16; src/distributed_cache.rs:50 virtual_nodes_per_node, ConsistentHash ring)
- [ ] Improve `content_aware_cache` scoring with configurable weight factors per media type (verified-open 2026-05-16: ContentCachePriority uses fixed per-type scoring, not user-configurable weights)
- [x] Add cache entry compression option for `tiered_cache` L2+ tiers to reduce memory footprint (verified 2026-05-16; src/tiered_cache.rs:65 TierConfig.compress, run-length encoding:133)

## New Features
- [x] Add `write_behind_cache` module with write-back to origin on eviction
- [x] Implement `cache_metrics` module with hit rate, miss rate, latency percentiles, and eviction counters (verified 2026-05-16; src/cache_metrics.rs)
- [x] Add `prefetch` module that pre-loads sequential media segments based on access pattern (verified 2026-05-16; src/prefetch.rs)
- [x] Implement `two_queue` (2Q) eviction policy for scan-resistant caching
- [x] Add `slab_allocator` module for cache entries to reduce fragmentation in long-running processes (verified 2026-05-16; src/slab_allocator.rs)
- [x] Implement `cache_partitioning` module for isolating cache space per tenant or stream (verified 2026-05-16; src/cache_partitioning.rs)
- [x] Add `cache_serialization` module for persisting cache state to disk on shutdown and restoring on startup (verified 2026-05-16; src/cache_serialization.rs)

## Performance
- [ ] Replace HashMap with a more cache-friendly data structure in `lru_cache` (e.g., Swiss table) (verified-open 2026-05-16: no hashbrown/Swiss table in lru_cache.rs)
- [ ] Add SIMD-accelerated hash computation for `bloom_filter` FNV-1a double hashing (verified-open 2026-05-16: no SIMD hash in bloom_filter.rs)
- [x] Implement sharded LRU cache for concurrent access without global lock contention (verified 2026-05-16; src/sharded_lru.rs)
- [ ] Use arena allocation for `tiered_cache` tier entries to reduce allocator pressure (verified-open 2026-05-16: no arena/bump alloc in tiered_cache.rs)
- [ ] Benchmark `eviction_policies::TinyLFU` admission gate overhead and optimize hot path (verified-open 2026-05-16: not yet profiled/optimized)

## Testing
- [ ] Add concurrent access stress test for `lru_cache` with multiple reader/writer threads
- [ ] Test `tiered_cache` promotion/demotion correctness under mixed workload patterns
- [ ] Add false positive rate validation test for `bloom_filter` against theoretical bounds
- [ ] Test `distributed_cache` rebalancing when nodes join/leave the consistent hash ring
- [ ] Add property-based test for `cache_warming` ensuring warmup plan respects capacity limits
- [ ] Test `content_aware_cache` eviction ordering with heterogeneous entry sizes and priorities

## Documentation
- [ ] Add capacity planning guide: how to size LRU/tiered caches for different media workloads
- [ ] Document eviction policy tradeoffs (LRU vs LFU vs TinyLFU vs ARC) with use-case recommendations
- [ ] Add examples showing `tiered_cache` configuration for L1 (memory) + L2 (disk) setup
