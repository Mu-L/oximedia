# oximedia-cdn TODO

## Current Status
- 5 modules: `edge_manager`, `cache_invalidation`, `origin_failover`, `geo_routing`, `cdn_metrics`
- `CdnManager` orchestrates all subsystems with `RwLock`/`Mutex` guarded state
- Zero external dependencies beyond `thiserror` and `uuid` (pure Rust)
- Features: consistent-hash ring, EWMA latency smoothing, Haversine geo-routing, Prometheus metrics export

## Enhancements
- [x] Replace `unwrap_or_else(|e| e.into_inner())` mutex recovery in `CdnManager` with proper error propagation
- [x] Add health check probing with configurable HTTP/TCP check in `origin_failover` (verified 2026-05-16; src/origin_failover.rs:369 HealthCheckProtocol, HealthChecker, HealthCheckConfig:404)
- [x] Extend `cache_invalidation` with tag-based invalidation (purge by content tag, not just path/glob)
- [ ] Add `edge_manager` node weight support for heterogeneous capacity (some PoPs larger than others) (verified-open 2026-05-16: not yet implemented)
- [ ] Implement soft-purge in `cache_invalidation` that marks stale but serves while revalidating (verified-open 2026-05-16: not yet implemented)
- [ ] Extend `geo_routing` with anycast simulation support for DNS-based routing (verified-open 2026-05-16: not yet implemented)
- [ ] Add `cdn_metrics` per-content-type breakdown (video, audio, image, manifest) (verified-open 2026-05-16: not yet implemented)
- [x] Improve `origin_failover` with circuit breaker pattern (half-open state for recovery detection)

## New Features
- [x] Add `token_auth` module for signed URL generation and validation (HMAC-based)
- [x] Implement `bandwidth_throttle` module for per-client or per-origin rate limiting (verified 2026-05-16; src/bandwidth_throttle.rs:829 lines)
- [x] Add `manifest_rewrite` module for HLS/DASH manifest URL rewriting to edge-local paths (verified 2026-05-16; src/manifest_rewrite.rs:912 lines)
- [x] Implement `ssl_cert_manager` module tracking TLS certificate expiration per edge node (verified 2026-05-16; src/ssl_cert_manager.rs:790 lines)
- [x] Add `cdn_log_analysis` module parsing access logs for traffic patterns and anomaly detection (verified 2026-05-16; src/cdn_log_analysis.rs:860 lines)
- [x] Implement `multi_cdn` module for coordinating across multiple CDN providers (failover, cost optimization) (verified 2026-05-16; src/multi_cdn.rs:661 lines, parking_lot::Mutex)
- [x] Add `prefetch_scheduler` module that pushes popular content to edges before demand spikes (verified 2026-05-16; src/prefetch_scheduler.rs:661 lines)
- [x] Implement `request_coalescing` module to deduplicate concurrent origin fetches for the same resource (verified 2026-05-16; src/request_coalescing.rs:626 lines)

## Performance
- [x] Replace `std::sync::RwLock` with `parking_lot::RwLock` in `CdnManager` for better performance (verified 2026-05-16; src/multi_cdn.rs:25 parking_lot::Mutex used for EWMA latency)
- [x] Use lock-free atomic counters in `cdn_metrics` instead of mutex-protected fields
- [ ] Implement connection pooling simulation in `origin_failover` for origin keep-alive reuse (verified-open 2026-05-16: not yet implemented)
- [ ] Cache Haversine distance calculations in `geo_routing` for repeated client-to-edge lookups (verified-open 2026-05-16: no Haversine cache in geo_routing.rs)
- [ ] Use a spatial index (R-tree) in `geo_routing` for O(log n) nearest-edge lookup instead of linear scan (verified-open 2026-05-16: not yet implemented)

## Testing
- [ ] Add concurrent invalidation stress test with 1000+ requests and multiple processing nodes
- [ ] Test `origin_failover` recovery: mark origin failed, simulate health check success, verify re-selection
- [ ] Add `geo_routing` test with edge cases: equidistant PoPs, antipodal points, same-location client/edge
- [ ] Test `cdn_metrics` Prometheus output format against Prometheus parser specification
- [ ] Add `CdnManager` integration test covering full lifecycle: add edges, configure origins, route, invalidate
- [ ] Test `cache_invalidation` rate limiting enforcement under burst conditions

## Documentation
- [ ] Document CDN architecture with data flow diagram showing edge-origin-client interactions
- [ ] Add deployment guide for configuring `CdnManager` in a multi-region setup
- [ ] Document Prometheus metrics names, types, and labels for monitoring dashboard setup
