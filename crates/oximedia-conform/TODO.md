# oximedia-conform TODO

## Current Status
- 60 source files; professional media conforming system
- Import formats: EDL (CMX 3600/3400), XML (FCP/Premiere/DaVinci), AAF (Avid)
- Matching strategies: filename, timecode, content hash, duration
- Features: SQLite media catalog, QC validation, timeline reconstruction (multi-track), batch processing, export (MP4, MKV, EDL, XML, AAF, frame sequences)
- Modules: importers (edl, xml, aaf), matching (filename, timecode, content, strategies), exporters (report, project, sequence), media (catalog, scanner, fingerprint), timeline (clip, track, transition), qc (checker, validator), session, batch, database, reconstruction, etc.

## Enhancements
- [ ] Extend `importers/edl.rs` with support for CMX 340 and File128 EDL variants (verified-open 2026-05-16: importers/edl.rs handles CMX 3600/3400; CMX 340 and File128 not present)
- [x] Add OTIO (OpenTimelineIO) import/export support in `importers/` and `exporters/` (verified 2026-05-16; src/importers/otio.rs:656 lines; src/exporters/otio.rs:428 OTIO JSON exporter)
- [x] Improve `matching/timecode.rs` with sub-frame accuracy matching for high frame rate content (verified 2026-05-16; src/matching/timecode.rs:11 SubFrameTimecode, sub_frame:20, 80 sub-frames per frame)
- [x] Extend `matching/content.rs` with perceptual hash-based fuzzy matching for re-encoded sources
- [x] Add confidence scoring to `matching/strategies.rs` with weighted multi-strategy combination
- [x] Improve `media_relink.rs` with recursive directory search and fuzzy path matching (verified 2026-05-16; src/media_relink.rs:126 RelinkCandidate, RelinkStrategy, confidence:141, recursive search support)
- [ ] Extend `qc/validator.rs` with codec-specific validation rules (AV1 levels, Opus bitrate ranges) (verified-open 2026-05-16: qc/validator.rs has general rules; no AV1 level/Opus bitrate-specific codec rules)
- [x] Add `conform_diff.rs` comparison between two conform sessions for change tracking (verified 2026-05-16; src/conform_diff.rs 596 lines)

## New Features
- [x] Implement watch folder mode in `session.rs` for automatic re-conform on source changes
- [ ] Add partial conform (selected clips only) support in `batch.rs` (verified-open 2026-05-16: batch.rs processes all clips; no partial/selected-clip-only conform)
- [ ] Implement proxy/offline-to-online conform workflow with resolution scaling (verified-open 2026-05-16: no proxy/offline-to-online conform with resolution scaling in conform sources)
- [x] Add color space conforming rules in `format_conform.rs` (ensure consistent color space across clips) (verified 2026-05-16; src/format_conform.rs 435 lines — color space conforming rules)
- [x] Implement audio loudness normalization during conform in `loudness_conform.rs` (EBU R128) (verified 2026-05-16; src/loudness_conform.rs 464 lines)
- [x] Add `delivery_map.rs` deliverable generation from a single conform session (multiple output specs) (verified 2026-05-16; src/delivery_map.rs 336 lines)
- [x] Implement frame rate conversion during conform in `frame_rate_convert.rs` with pulldown detection (verified 2026-05-16; src/frame_rate_convert.rs 345 lines)
- [x] Add `test_card.rs` offline placeholder generation for missing source media (verified 2026-05-16; src/test_card.rs 363 lines)

## Performance
- [x] Parallelize `media/scanner.rs` directory scanning using rayon
- [ ] Add incremental database updates in `database.rs` (skip unchanged files on re-scan)
- [ ] Cache fingerprint computation results in `media/fingerprint.rs` with file modification time checks
- [ ] Optimize `matching/` strategies to use bloom filters for initial candidate filtering
- [ ] Profile and optimize `reconstruction.rs` for timelines with 1000+ clips

## Testing
- [ ] Add end-to-end conform test with sample EDL, source media, and expected output verification
- [ ] Test `importers/xml.rs` with real FCP X, Premiere Pro, and DaVinci Resolve XML exports
- [ ] Test `importers/aaf.rs` with Avid Media Composer AAF exports
- [x] Add round-trip test: import EDL -> conform -> export EDL -> verify identical timeline
- [ ] Test `batch.rs` with 100+ clip conform jobs for throughput and correctness
- [ ] Test `timecode_conform.rs` with drop-frame and non-drop-frame timecode edge cases

## Documentation
- [ ] Document supported EDL/XML/AAF format variants and their limitations
- [ ] Add conform workflow tutorial from import to export
- [ ] Document matching strategy selection guidelines for different source material types
