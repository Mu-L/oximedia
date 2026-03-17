# oximedia-edl TODO

## Current Status
- 34 source files covering CMX 3600/3400, GVG, Sony BVE-9000 format parsing and generation
- Full timecode support: drop-frame, non-drop-frame at 24/25/30/60 fps
- Event types: Cut, Dissolve, Wipe, Key with motion effects
- Audio channel mapping and routing in `audio.rs`
- Validation, comparison, merging, filtering, optimization, and batch export
- Dependencies: oximedia-core, oximedia-timecode, nom, serde, bitflags

## Enhancements
- [x] Add ALE (Avid Log Exchange) format parsing in addition to CMX formats
- [ ] Implement AAF (Advanced Authoring Format) round-trip support
- [x] Enhance `edl_compare.rs` with diff visualization showing added/removed/modified events
- [x] Add `edl_merge.rs` conflict resolution strategies (prefer source A, prefer source B, manual)
- [x] Implement `optimizer.rs` event consolidation (merge adjacent cuts from same reel)
- [x] Enhance `validator.rs` with format-specific validation rules per EDL format
- [ ] Add `edl_filter.rs` filtering by reel name patterns and timecode ranges
- [x] Improve `timecode.rs` with 23.976 fps and 59.94 fps drop-frame timecode support (`Fps23_976`, `Fps59_94`)
- [ ] Add `conform_report.rs` report generation comparing EDL against actual media files
- [ ] Implement `roundtrip.rs` format conversion between CMX 3600 and GVG/Sony formats

## New Features
- [ ] Add XML-based EDL format support (Final Cut Pro XML, DaVinci Resolve XML)
- [ ] Implement OTIO (OpenTimelineIO) format import/export
- [ ] Add EDL-to-timeline conversion producing oximedia-edit Timeline structures
- [ ] Implement multi-camera EDL support with camera angle switching events
- [x] Add EDL visualization as ASCII timeline for debugging
- [x] Implement `batch_export.rs` parallel EDL generation via `BatchEdlExporter::export_parallel` (rayon)
- [ ] Add EDL change tracking with version history
- [ ] Implement sub-frame timecode precision for high frame rate content (120fps+)

## Performance
- [ ] Optimize `parser.rs` nom combinators to reduce allocations during parsing
- [ ] Add lazy parsing mode that only parses event headers until details are accessed
- [ ] Cache timecode-to-frames conversions in `timecode.rs` for repeated lookups
- [ ] Optimize `edl_statistics.rs` to compute all statistics in a single pass
- [ ] Use `SmallVec` for event comments to avoid heap allocation for common cases

## Testing
- [ ] Add conformance tests with real-world EDL files from major NLE systems (Avid, Premiere, Resolve)
- [ ] Test `cmx3600.rs` parser with EDLs containing complex wipe patterns and key events
- [ ] Add fuzz testing for `parser.rs` with malformed EDL inputs
- [ ] Test `motion.rs` speed effect parsing with reverse playback and freeze frames
- [ ] Test `edl_timeline.rs` conversion accuracy with overlapping events
- [x] Add round-trip tests for all supported formats — `test_cmx3600_roundtrip` (3-event EDL), `test_ale_parse_basic`
- [ ] Test `reel_registry.rs` with duplicate reel name handling

## Documentation
- [ ] Document CMX 3600 format specification with field-by-field breakdown
- [ ] Add format comparison table (CMX 3600 vs GVG vs Sony BVE-9000 capabilities)
- [ ] Document the event numbering and timecode conventions used across formats
