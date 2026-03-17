# oximedia-cdn TODO

## Current Status
- 5 modules: `edge_manager`, `cache_invalidation`, `origin_failover`, `geo_routing`, `cdn_metrics`
- `CdnManager` orchestrates all subsystems with `RwLock`/`Mutex` guarded state
- Zero external dependencies beyond `thiserror` and `uuid` (pure Rust)
- Features: consistent-hash ring, EWMA latency smoothing, Haversine geo-routing, Prometheus metrics export

## Enhancements
- [x] Replace `unwrap_or_else(|e| e.into_inner())` mutex recovery in `CdnManager` with proper error propagation
- [ ] Add health check probing with configurable HTTP/TCP check in `origin_failover`
- [x] Extend `cache_invalidation` with tag-based invalidation (purge by content tag, not just path/glob)
- [ ] Add `edge_manager` node weight support for heterogeneous capacity (some PoPs larger than others)
- [ ] Implement soft-purge in `cache_invalidation` that marks stale but serves while revalidating
- [ ] Extend `geo_routing` with anycast simulation support for DNS-based routing
- [ ] Add `cdn_metrics` per-content-type breakdown (video, audio, image, manifest)
- [x] Improve `origin_failover` with circuit breaker pattern (half-open state for recovery detection)

## New Features
- [x] Add `token_auth` module for signed URL generation and validation (HMAC-based)
- [ ] Implement `bandwidth_throttle` module for per-client or per-origin rate limiting
- [ ] Add `manifest_rewrite` module for HLS/DASH manifest URL rewriting to edge-local paths
- [ ] Implement `ssl_cert_manager` module tracking TLS certificate expiration per edge node
- [ ] Add `cdn_log_analysis` module parsing access logs for traffic patterns and anomaly detection
- [ ] Implement `multi_cdn` module for coordinating across multiple CDN providers (failover, cost optimization)
- [ ] Add `prefetch_scheduler` module that pushes popular content to edges before demand spikes
- [ ] Implement `request_coalescing` module to deduplicate concurrent origin fetches for the same resource

## Performance
- [ ] Replace `std::sync::RwLock` with `parking_lot::RwLock` in `CdnManager` for better performance
- [x] Use lock-free atomic counters in `cdn_metrics` instead of mutex-protected fields
- [ ] Implement connection pooling simulation in `origin_failover` for origin keep-alive reuse
- [ ] Cache Haversine distance calculations in `geo_routing` for repeated client-to-edge lookups
- [ ] Use a spatial index (R-tree) in `geo_routing` for O(log n) nearest-edge lookup instead of linear scan

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
