# oximedia-cache TODO

## Current Status
- 7 modules: `lru_cache`, `tiered_cache`, `cache_warming`, `bloom_filter`, `distributed_cache`, `eviction_policies`, `content_aware_cache`
- Zero external dependencies beyond `thiserror` (pure Rust, minimal footprint)
- LRU cache with O(1) operations, tiered multi-level cache with pluggable eviction (LRU/LFU/FIFO/Random/TinyLFU)
- Distributed cache with consistent-hash ring and quorum replication

## Enhancements
- [x] Add TTL (time-to-live) expiration support to `lru_cache::LruCache` entries
- [ ] Extend `tiered_cache` with async disk tier using file-backed storage (not just simulated)
- [x] Add cache entry pinning in `lru_cache` to prevent eviction of critical items
- [ ] Implement adaptive tier promotion thresholds in `tiered_cache` based on access frequency
- [x] Extend `bloom_filter` with scalable Bloom filter variant that grows as elements are added
- [ ] Add `distributed_cache` virtual node support for better hash ring distribution
- [ ] Improve `content_aware_cache` scoring with configurable weight factors per media type
- [ ] Add cache entry compression option for `tiered_cache` L2+ tiers to reduce memory footprint

## New Features
- [x] Add `write_behind_cache` module with write-back to origin on eviction
- [ ] Implement `cache_metrics` module with hit rate, miss rate, latency percentiles, and eviction counters
- [ ] Add `prefetch` module that pre-loads sequential media segments based on access pattern
- [x] Implement `two_queue` (2Q) eviction policy for scan-resistant caching
- [ ] Add `slab_allocator` module for cache entries to reduce fragmentation in long-running processes
- [ ] Implement `cache_partitioning` module for isolating cache space per tenant or stream
- [ ] Add `cache_serialization` module for persisting cache state to disk on shutdown and restoring on startup

## Performance
- [ ] Replace HashMap with a more cache-friendly data structure in `lru_cache` (e.g., Swiss table)
- [ ] Add SIMD-accelerated hash computation for `bloom_filter` FNV-1a double hashing
- [ ] Implement sharded LRU cache for concurrent access without global lock contention
- [ ] Use arena allocation for `tiered_cache` tier entries to reduce allocator pressure
- [ ] Benchmark `eviction_policies::TinyLFU` admission gate overhead and optimize hot path

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
