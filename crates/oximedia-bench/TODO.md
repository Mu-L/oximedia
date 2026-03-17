# oximedia-bench TODO

## Current Status
- 28 modules providing codec benchmarking: runner, metrics (PSNR/SSIM/VMAF), comparison, statistics, reporting (JSON/CSV/HTML)
- Additional modules: `baseline`, `cpu_profile`, `gpu_bench`, `io_bench`, `latency`, `memory`, `pipeline_bench`, `regression`, `regression_bench`, `regression_detect`, `scalability_bench`, `throughput`, `warmup_strategy`
- Preset configurations: quick, standard, comprehensive, quality_focus, speed_focus
- `BenchmarkSuite` orchestrates multi-codec, multi-sequence benchmark runs

## Enhancements
- [x] Fix `format_timestamp()` to use proper calendar calculation instead of approximate year/month/day
- [x] Add BD-Rate (Bjontegaard Delta) calculation to `comparison` module for codec efficiency comparison
- [x] Extend `metrics::QualityMetrics` with MS-SSIM (Multi-Scale SSIM) support
- [ ] Add per-frame quality metric tracking in `metrics` for temporal quality analysis
- [ ] Improve `stats::compute_statistics` with outlier detection and removal (IQR method)
- [ ] Add confidence intervals to `Statistics` for mean encoding/decoding FPS
- [ ] Extend `BenchmarkFilter` with date range filtering for historical result comparison
- [ ] Add `CodecConfig` validation to reject invalid preset/bitrate/CQ combinations per codec

## New Features
- [x] Implement `audio_bench` module for audio codec benchmarking (Opus, Vorbis, FLAC)
- [ ] Add `container_bench` module benchmarking mux/demux overhead for WebM, MKV, OGG
- [ ] Implement `ab_comparison` module generating side-by-side visual quality comparisons
- [x] Add `rate_distortion` module for automated RD-curve generation across quality levels
- [ ] Implement `historical_db` module storing benchmark results in SQLite for trend analysis
- [ ] Add `system_fingerprint` module capturing exact hardware/OS/compiler info for reproducibility
- [ ] Implement `flamegraph_integration` module for automated CPU profiling during benchmarks
- [ ] Add markdown report export format alongside existing JSON/CSV/HTML

## Performance
- [ ] Parallelize multi-codec benchmarks in `BenchmarkSuite::run_all()` using rayon thread pool
- [ ] Implement incremental benchmarking: skip sequences whose results are cached and config unchanged
- [ ] Add streaming CSV/JSON export in `BenchmarkResults` to avoid holding all results in memory
- [ ] Use memory-mapped I/O for reading test sequences in `runner`
- [ ] Pre-allocate frame buffers in `runner` based on sequence resolution to avoid repeated allocation

## Testing
- [ ] Add test for `BenchmarkSuite::run_all()` with synthetic test sequences
- [ ] Test `comparison::CodecComparison::compare()` with known reference data
- [ ] Add regression test for `report::HtmlReport` output structure
- [ ] Test `BenchmarkFilter::apply()` correctly filters by all supported criteria combinations
- [ ] Add test verifying `BenchmarkPresets` configurations pass `BenchmarkConfigBuilder::build()` validation
- [ ] Test `duration_serde` round-trip for edge cases (zero, very large durations)

## Documentation
- [ ] Document how to create custom test sequences for `sequences` module
- [ ] Add guide for interpreting benchmark results and quality metrics
- [ ] Document `regression_detect` module usage for CI-based performance regression detection
