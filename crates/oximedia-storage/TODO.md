# oximedia-storage TODO

## Current Status
- 29 source files covering cloud storage abstraction for S3, Azure Blob Storage, and Google Cloud Storage
- Core modules: s3, azure, gcs (all feature-gated), local, cache, quota, replication, transfer, tiering
- Advanced modules: cache_layer (LRU/LFU/FIFO/ARC), compression_store, dedup_store, integrity_checker, lifecycle, namespace, object_store, path_resolver, replication_policy, retention_manager, storage_events, storage_metrics, storage_policy, transfer_stats, write_ahead_log, access_log
- Async CloudStorage trait with unified API: upload/download streams, multipart, presigned URLs, pagination
- Dependencies: tokio, async-trait, bytes, futures, chrono, sha2, lru, and provider SDKs behind feature gates

## Enhancements
- [ ] Add automatic content-type detection in `UploadOptions` based on file extension when content_type is None
- [ ] Implement transparent compression in `compression_store` that auto-selects between zstd and lz4 based on object size
- [ ] Extend `cache_layer` ARC cache with size-aware eviction (evict by total bytes, not just entry count)
- [ ] Add connection pooling configuration to `UnifiedConfig` with idle timeout and max lifetime settings
- [ ] Implement batch metadata update in `CloudStorage` trait for bulk tagging operations
- [ ] Add bandwidth throttling integration in `transfer` module using token bucket algorithm
- [ ] Extend `replication_policy` to support cross-provider replication (e.g., S3 primary -> GCS replica)
- [ ] Add retry configuration (max retries, backoff multiplier, jitter) as part of `UnifiedConfig`

## New Features
- [ ] Implement `minio` backend module for self-hosted S3-compatible object storage (feature-gated)
- [ ] Add `object_versioning` module with version listing, restore, and delete-marker management
- [ ] Implement `batch_operations` module for parallel multi-object upload/download with configurable concurrency
- [ ] Add `object_lock` module for compliance-mode and governance-mode object locking (WORM storage)
- [ ] Implement `storage_migration` module for cross-provider migration with progress tracking and verification
- [ ] Add `server_side_copy` optimization that uses provider-native copy for same-provider transfers
- [ ] Implement `inventory_report` module for generating storage inventory (object count, total size, class distribution)
- [ ] Add `presigned_post` support for browser-based direct uploads with policy conditions
- [ ] Implement `multipart_resumable` module for resumable uploads that survive process restarts

## Performance
- [ ] Add parallel chunk upload in multipart operations with configurable part size and concurrency
- [ ] Implement predictive prefetching in `cache` based on access pattern analysis (sequential vs. random)
- [ ] Use memory-mapped I/O in `local` backend for large file reads to reduce copy overhead
- [ ] Add connection keep-alive and HTTP/2 multiplexing support in provider clients
- [ ] Implement lazy metadata loading in `list_objects` that defers per-object HEAD requests until accessed

## Testing
- [ ] Add integration tests with MinIO container for S3 operations without AWS credentials
- [ ] Test `cache_layer` eviction policies with deterministic access sequences and verify hit/miss ratios
- [ ] Add concurrent access tests for `write_ahead_log` with multiple simultaneous writers
- [ ] Test `dedup_store` reference counting correctness under concurrent put/delete operations
- [ ] Add round-trip tests for `compression_store` verifying data integrity after compress/decompress
- [ ] Test `lifecycle` policy engine with objects at various ages and verify correct tier transitions

## Documentation
- [ ] Add provider setup guide with minimal IAM permissions required for each cloud provider
- [ ] Document cache layer selection guidelines (when to use LRU vs. ARC vs. LFU)
- [ ] Add architecture diagram showing the storage abstraction layers (trait -> provider -> cache -> local)
