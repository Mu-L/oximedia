# oximedia-cloud TODO

## Current Status
- 46 source files across AWS, Azure, GCP providers
- Key features: S3/Blob/GCS storage, CDN, transcoding pipelines, cost management, transfer management, security/encryption, auto-scaling, replication, lifecycle management
- Modules: aws (s3, media), azure (blob, media), gcp (gcs, media), cdn, cost, transfer, security, cloud_backup, bandwidth_throttle, cloud_lifecycle, event_bridge, multicloud, multiregion, etc.

## Enhancements
- [x] Add retry logic with exponential backoff in `transfer.rs` for transient network failures
- [x] Implement resumable multipart uploads in `upload_manager.rs` with checkpoint persistence
- [ ] Add connection pooling and keep-alive management in `storage_provider.rs`
- [ ] Extend `cdn_edge.rs` with edge cache invalidation patterns (wildcard, tag-based)
- [ ] Add bandwidth measurement and adaptive throttling in `bandwidth_throttle.rs`
- [ ] Implement cross-region transfer optimization in `multiregion.rs` with latency-based routing
- [x] Add signed URL generation with custom expiry for all three providers in `security.rs`
- [x] Extend `cost_monitor.rs` with budget alerts and cost anomaly detection

## New Features
- [ ] Add Oracle Cloud Infrastructure (OCI) Object Storage provider
- [ ] Add Backblaze B2 as a low-cost storage provider
- [ ] Implement server-side copy between buckets/containers within the same provider
- [ ] Add cloud-native video thumbnail generation via provider-specific services
- [ ] Implement storage tiering automation in `storage_class.rs` (intelligent tiering rules)
- [ ] Add webhook/event notification support for object lifecycle events in `event_bridge.rs`
- [ ] Implement cloud-to-cloud migration tool using `multicloud` module
- [ ] Add pre-signed POST policy generation for browser-based direct uploads

## Performance
- [x] Implement parallel multipart upload with configurable part size in `upload_manager.rs`
- [ ] Add streaming download with zero-copy I/O in `transfer.rs` using `bytes::Bytes`
- [ ] Cache cloud provider credentials with automatic refresh in `cloud_credentials.rs`
- [ ] Add connection multiplexing for concurrent operations in `generic.rs`

## Testing
- [ ] Add integration test suite using `wiremock` for all three provider APIs
- [ ] Test `cloud_backup.rs` incremental backup/restore round-trip
- [ ] Add stress tests for concurrent upload/download in `transfer.rs`
- [ ] Test `replication_policy.rs` failover scenarios with simulated provider outages
- [ ] Add tests for `cost_model.rs` estimation accuracy across different storage classes

## Documentation
- [ ] Add architecture diagram showing provider abstraction layers
- [ ] Document authentication flow for each provider in `cloud_auth.rs`
- [ ] Add usage examples for CDN configuration in `cdn_config.rs`
