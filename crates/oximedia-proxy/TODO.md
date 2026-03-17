# oximedia-proxy TODO

## Current Status
- 42 modules for proxy and offline editing workflow management
- Key types: ProxyGenerator, ProxyLinkManager, ConformEngine, OfflineWorkflow, ProxyRegistry, CacheManager, ProxySpec
- Modules: cache, conform, generate, generation, link, linking, media_link, metadata, offline_edit, offline_proxy, proxy_aging/bandwidth/cache/compare/fingerprint/format/index/manifest/pipeline/quality/registry_ext/scheduler/status/sync, registry, relink_proxy, render, resolution, sidecar, smart_proxy, spec, timecode, transcode_proxy, transcode_queue, utils, validation, workflow
- Dependencies: oximedia-core, oximedia-transcode, oximedia-edl, oximedia-timecode, oximedia-metadata, rayon, tokio

## Enhancements
- [ ] Add incremental proxy generation in `generate` — only re-encode changed segments
- [ ] Implement proxy health monitoring in `proxy_status` — detect corrupted/incomplete proxies
- [ ] Extend `conform` with AAF (Advanced Authoring Format) project file conforming support
- [ ] Add multi-resolution proxy generation in `smart_proxy` — create 1/4, 1/2, full-res variants in one pass
- [ ] Implement proxy migration in `proxy_sync` — transfer proxy databases between workstations
- [ ] Extend `proxy_fingerprint` with perceptual hashing for content-based proxy-original matching
- [ ] Add batch conforming support in `ConformEngine` — conform multiple EDLs to a single timeline
- [ ] Implement proxy expiration policies in `proxy_aging` with configurable TTL per project

## New Features
- [ ] Add cloud proxy generation — offload proxy transcoding to remote workers
- [ ] Implement proxy streaming — stream proxies over network without local download
- [ ] Add DaVinci Resolve project file support in `conform` module (.drp format)
- [ ] Implement proxy quality comparison dashboard data in `proxy_compare` (side-by-side metrics)
- [ ] Add automatic proxy format selection based on NLE detection (Premiere prefers ProRes, Resolve prefers DNx)
- [ ] Implement proxy audit trail in `proxy_manifest` — track who generated/modified each proxy
- [ ] Add proxy pool management for shared storage environments in `registry`

## Performance
- [ ] Implement parallel proxy generation in `transcode_queue` using rayon work-stealing
- [ ] Add proxy cache warming — pre-generate proxies for frequently accessed media
- [ ] Optimize `proxy_index` lookups with B-tree index on file path and timecode
- [ ] Use memory-mapped I/O in `proxy_fingerprint` for large file hashing
- [ ] Implement streaming proxy generation — start editing before proxy is fully generated

## Testing
- [ ] Add end-to-end test: ingest -> generate proxy -> edit (simulate) -> conform -> verify output
- [ ] Test `ConformEngine` with real-world EDL samples (CMX 3600, Final Cut Pro XML)
- [ ] Add stress test for `CacheManager` with rapid create/evict cycles (1000+ proxies)
- [ ] Test `proxy_sync` with simulated network interruptions and verify resume capability
- [ ] Verify `TimecodePreserver` accuracy across proxy generation (timecode drift < 1 frame)

## Documentation
- [ ] Add offline-to-online workflow tutorial with step-by-step proxy workflow example
- [ ] Document proxy format recommendations for different NLEs and storage constraints
- [ ] Add cache management guide with sizing recommendations based on project scale
