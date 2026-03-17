# oximedia-repair TODO

## Current Status
- 35 modules (15 subdirectory modules) covering corruption detection, header repair, index rebuilding, timestamp correction, packet recovery, sync fixes, truncation recovery, metadata repair, frame reordering, and error concealment
- Core types: RepairEngine, RepairOptions, RepairMode (Safe/Balanced/Aggressive/Extract), Issue/IssueType/Severity
- Batch repair support via `repair_batch`
- Dependencies: oximedia-core, oximedia-container, oximedia-codec, thiserror

## Enhancements
- [ ] Implement actual repair logic in `RepairEngine::fix_issue` dispatcher (currently returns stub Ok(true)/Ok(false))
- [ ] Add progress callback to `repair_file` and `repair_batch` for UI integration
- [ ] Extend `corruption_map` to visualize byte-level corruption regions with exportable heat maps
- [ ] Add confidence scores to `detect::corruption` results indicating certainty of corruption detection
- [ ] Implement incremental `verify::integrity` that can resume verification from a checkpoint
- [ ] Extend `repair_log` with structured JSON output for machine-parseable repair history
- [ ] Add `RepairMode::Custom` variant allowing per-issue-type aggressiveness configuration
- [ ] Improve `frame_concealment` with motion-compensated interpolation from adjacent good frames

## New Features
- [ ] Add a `codec_probe` module that identifies codec parameters from raw bitstream when headers are damaged
- [ ] Implement `container_migrate` for re-muxing repaired streams into a clean container without re-encoding
- [ ] Add `repair_profile` for saving and loading repair configuration presets per container format
- [ ] Implement `parallel_repair` using rayon for batch repair of independent files
- [ ] Add `corruption_simulator` for testing: inject controlled corruption (bit-flip, truncation, header wipe)
- [ ] Implement `stream_splice` for extracting playable segments from partially corrupt files
- [ ] Add `repair_diff` module to generate before/after comparison reports with frame-by-frame quality metrics
- [ ] Implement `progressive_repair` that yields partially repaired output as each issue is fixed

## Performance
- [ ] Use memory-mapped I/O in `detect::scan::deep_scan` for large files (>1GB) to avoid full reads
- [ ] Add sector-aligned reads in `bitstream_repair` for SSD-friendly I/O patterns
- [ ] Cache detected issues in `RepairEngine` to avoid re-analysis when repairing the same file
- [ ] Parallelize `detect::analyze::analyze_file` across independent analysis passes

## Testing
- [ ] Add tests for `repair_file` with actual corrupted test fixtures (truncated MP4, damaged WebM headers)
- [ ] Test `repair_batch` error handling when some files in the batch are unrecoverable
- [ ] Add roundtrip tests: corrupt a valid file, repair it, verify playback
- [ ] Test `backup` creation and skip-backup threshold logic with files of varying sizes
- [ ] Add fuzz tests for `header` repair modules with random byte sequences

## Documentation
- [ ] Document the detection pipeline: corruption -> analyze -> deep_scan flow
- [ ] Add repair strategy guide explaining when to use Safe vs Balanced vs Aggressive modes
- [ ] Document each IssueType with example symptoms and expected repair outcomes
- [ ] Add troubleshooting guide for files that fail the TooCorrupted check
