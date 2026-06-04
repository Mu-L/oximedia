# oximedia-proxy TODO

## Current Status
- 42 modules for proxy and offline editing workflow management
- Key types: ProxyGenerator, ProxyLinkManager, ConformEngine, OfflineWorkflow, ProxyRegistry, CacheManager, ProxySpec
- Modules: cache, conform, generate, generation, link, linking, media_link, metadata, offline_edit, offline_proxy, proxy_aging/bandwidth/cache/compare/fingerprint/format/index/manifest/pipeline/quality/registry_ext/scheduler/status/sync, registry, relink_proxy, render, resolution, sidecar, smart_proxy, spec, timecode, transcode_proxy, transcode_queue, utils, validation, workflow
- Dependencies: oximedia-core, oximedia-transcode, oximedia-edl, oximedia-timecode, oximedia-metadata, rayon, tokio

## Enhancements
- [x] Add incremental proxy generation in `generate` — only re-encode changed segments (verified 2026-05-16; src/generate/incremental.rs:42 SegmentManifest, SegmentHash:17, ChangeSet:87, 775 lines)
- [x] Implement proxy health monitoring in `proxy_status` — detect corrupted/incomplete proxies (verified 2026-05-16; src/proxy_status.rs:128 ProxyStatusTracker, ProxyJobStatus:48, transition:155, 430 lines)
- [x] Extend `conform` with AAF (Advanced Authoring Format) project file conforming support (verified 2026-05-16; src/conform/timeline.rs:219 TimelineFormat::Aaf variant, Aaf extension parsing:193)
- [x] Add multi-resolution proxy generation in `smart_proxy` — create 1/4, 1/2, full-res variants in one pass (verified 2026-05-16; src/smart_proxy.rs:445 ResolutionVariant, MultiResolutionProxySet:486 quarter/half/full, 682 lines)
- [x] Implement proxy migration in `proxy_sync` — transfer proxy databases between workstations (verified 2026-06-02: proxy_sync.rs `ProxyDbExport { entries, root_prefix, created_at }` + `import_with_rebase(new_root)` + `RebaseResult`; 4 tests pass: round-trip, rebase-rewrites-root, non-prefix-unchanged, reports-missing)
- [x] Extend `proxy_fingerprint` with perceptual hashing for content-based proxy-original matching (verified 2026-05-16; src/proxy_fingerprint.rs:452 PerceptualHash, DHashEngine:493, diff_hash:465)
- [x] Add batch conforming support in `ConformEngine` — conform multiple EDLs to a single timeline (verified 2026-06-02: conform/engine.rs `batch_conform(&[Edl], MergeStrategy) -> BatchConformResult`; `MergeStrategy { PreferEarlier, PreferLonger, LayerToTracks }`; `ConformedEvent`, `EventProvenance`; 5 tests pass: non-overlapping, overlap-prefer-earlier, overlap-prefer-longer, empty, provenance)
- [x] Implement proxy expiration policies in `proxy_aging` with configurable TTL per project (verified 2026-05-16; src/proxy_aging.rs 471 lines — TTL/expiration management)

## New Features
- [x] Add cloud proxy generation — offload proxy transcoding to remote workers (verified 2026-05-16; src/cloud_proxy.rs:66 CloudWorker, CloudRegion:19, 716 lines)
- [x] Implement proxy streaming — stream proxies over network without local download (verified 2026-05-16; src/proxy_streaming.rs:46 ByteRange, StreamingServer, chunked delivery, 663 lines)
- [x] Add DaVinci Resolve project file support in `conform` module (.drp format) (verified 2026-05-16; src/nle_format_select.rs:25 Nle::DaVinciResolve, .drp extension:57, DNxHR recommendation:262)
- [x] Implement proxy quality comparison dashboard data in `proxy_compare` (side-by-side metrics) (verified 2026-05-16; src/proxy_compare.rs 465 lines — quality metrics comparison)
- [x] Add automatic proxy format selection based on NLE detection (Premiere prefers ProRes, Resolve prefers DNx) (verified 2026-05-16; src/nle_format_select.rs:20 Nle enum, detect from path:67, recommend per NLE:121, 511 lines)
- [x] Implement proxy audit trail in `proxy_manifest` — track who generated/modified each proxy (verified 2026-05-16; src/proxy_audit.rs:60 AuditEntry, AuditLog:126, actor field:71, 385 lines)
- [x] Add proxy pool management for shared storage environments in `registry` (verified 2026-05-16; src/proxy_pool.rs:30 Worker, ProxyJob:95, ProxyPool with assign/drain, 376 lines)

## Performance
- [x] Implement parallel proxy generation in `transcode_queue` using rayon work-stealing (verified 2026-06-01; transcode_queue.rs:625 ThreadPoolBuilder::new().num_threads, :632 jobs.par_iter(), fallback to global rayon pool)
- [ ] Add proxy cache warming — pre-generate proxies for frequently accessed media
- [x] Optimize `proxy_index` lookups with B-tree index on file path and timecode (verified 2026-06-01; proxy_index.rs RangeProxyIndex BTreeMap<RangeKey,ProxyEntry> composite key (path,pts): find_by_original, find_in_timecode_range, find_by_path_prefix)
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
