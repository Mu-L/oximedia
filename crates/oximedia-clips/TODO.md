# oximedia-clips TODO

## Current Status
- 33 modules providing clip management: database, subclips, grouping (bins/folders/collections), logging (keywords/markers/ratings/notes), takes, proxy association, smart collections, search, import/export
- Sub-directories: `clip/`, `database/`, `export/`, `group/`, `import/`, `logging/`, `marker/`, `note/`, `proxy/`, `rating/`, `search/`, `subclip/`, `take/`, `trim/`
- Platform-conditional: `database`, `import`, `manager` excluded on `wasm32`
- Uses `sqlx` for async database, `tokio` for async I/O

## Enhancements
- [x] Add `clip_fingerprint` module to lib.rs exports (file exists at `src/clip_fingerprint.rs` but not declared)
- [x] Add `clip_merge` module to lib.rs exports (file exists at `src/clip_merge.rs` but not declared)
- [x] Add `clip_playlist` module to lib.rs exports (file exists at `src/clip_playlist.rs` but not declared)
- [x] Extend `search` with fuzzy matching for keyword and clip name queries (verified 2026-05-16; src/search/fuzzy.rs)
- [x] Add `clip_metadata` support for camera metadata (lens, ISO, aperture, shutter speed) — `CameraMetadata` struct with full builder API; `camera: Option<CameraMetadata>` field added to `Clip`; `CameraMetadataStore` for batch access
- [x] Implement `smart_collection` auto-refresh with configurable polling interval (verified 2026-05-16; src/group/smart.rs:28 poll_interval_secs, set_poll_interval:161)
- [x] Extend `export` with Final Cut Pro XML (FCPXML) export format (verified 2026-05-16; src/export/fcpxml.rs:294 lines)
- [x] Add `clip_relations` bidirectional linking (A relates to B implies B relates to A) — `clip_relations_bidirectional` module with `BidirectionalRelationGraph`
- [x] Improve `take::TakeSelector` with multi-criteria ranking (rating + recency + duration match) — `MultiCriteriaTakeSelector`, `TakeScoreWeights`/`TakeWeights`, `rank_takes_multi_criteria` free function

## New Features
- [x] Add `clip_waveform` module generating audio waveform thumbnails for timeline display — `WaveformThumbnail` with `generate(samples, width) -> Vec<f32>` RMS downsampler; `ClipWaveformGenerator` for min/max peak data
- [x] Implement `clip_ai_tag` module for automatic keyword suggestion based on visual content (verified 2026-05-16; src/clip_ai_tag.rs:442 lines AiTagger)
- [x] Add `clip_collaboration` module for multi-user clip annotation with conflict resolution (verified 2026-05-16; src/clip_collaboration.rs:578 lines)
- [x] Implement `clip_versioning` module tracking edit history with undo/redo support (verified 2026-05-16; src/clip_versioning.rs:556 lines ClipVersionHistory)
- [x] Add `clip_transcode_status` module tracking proxy generation and transcode progress per clip (verified 2026-05-16; src/clip_transcode_status.rs:482 lines TranscodeStatusStore)
- [x] Implement `clip_face_index` module for face-based clip search and grouping (verified 2026-05-16; src/clip_face_index.rs:470 lines ClipFaceIndex)
- [x] Add `clip_scene_detect` module automatically creating subclips at scene boundaries (verified 2026-05-16; src/clip_scene_detect.rs:434 lines ThresholdSceneDetector)
- [x] Implement `clip_favorites` module with quick-access collections and recent clips list (verified 2026-05-16; src/clip_favorites.rs:546 lines FavoriteCollection, RecentClipList)

## Performance
- [ ] Add database index on `keywords` and `rating` columns for faster filtered queries
- [ ] Implement batch `add_clips()` method in `ClipManager` with single transaction for bulk import
- [ ] Cache `SmartCollection` query results with invalidation on clip metadata changes
- [ ] Use prepared statement caching in `database` module to avoid repeated SQL parsing
- [ ] Add pagination support to `search` and `ClipManager::list_clips()` for large clip libraries

## Testing
- [ ] Add integration test for `ClipManager` full lifecycle: create, search, update, delete
- [ ] Test `smart_collection` auto-updating when new clips matching criteria are added
- [ ] Add `export` round-trip test: export EDL, reimport, verify clip in/out points preserved
- [x] Test `subclip` creation with boundary conditions (start=0, end=clip duration) — `test_subclip_boundary_conditions` in `clip/subclip.rs`
- [ ] Add concurrent access test for `ClipManager` with multiple simultaneous writers
- [ ] Test `clip_compare` produces correct similarity scores for known duplicate/different clips
- [ ] Test `bin_organizer` automatic bin assignment based on metadata rules

## Documentation
- [ ] Document database schema and migration strategy for `database` module
- [ ] Add guide for setting up proxy workflows with `proxy_link` module
- [ ] Document supported import/export formats with feature requirements
