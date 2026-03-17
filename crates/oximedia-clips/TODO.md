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
- [ ] Extend `search` with fuzzy matching for keyword and clip name queries
- [x] Add `clip_metadata` support for camera metadata (lens, ISO, aperture, shutter speed) — `CameraMetadata` struct with full builder API; `camera: Option<CameraMetadata>` field added to `Clip`; `CameraMetadataStore` for batch access
- [ ] Implement `smart_collection` auto-refresh with configurable polling interval
- [ ] Extend `export` with Final Cut Pro XML (FCPXML) export format
- [x] Add `clip_relations` bidirectional linking (A relates to B implies B relates to A) — `clip_relations_bidirectional` module with `BidirectionalRelationGraph`
- [x] Improve `take::TakeSelector` with multi-criteria ranking (rating + recency + duration match) — `MultiCriteriaTakeSelector`, `TakeScoreWeights`/`TakeWeights`, `rank_takes_multi_criteria` free function

## New Features
- [x] Add `clip_waveform` module generating audio waveform thumbnails for timeline display — `WaveformThumbnail` with `generate(samples, width) -> Vec<f32>` RMS downsampler; `ClipWaveformGenerator` for min/max peak data
- [ ] Implement `clip_ai_tag` module for automatic keyword suggestion based on visual content
- [ ] Add `clip_collaboration` module for multi-user clip annotation with conflict resolution
- [ ] Implement `clip_versioning` module tracking edit history with undo/redo support
- [ ] Add `clip_transcode_status` module tracking proxy generation and transcode progress per clip
- [ ] Implement `clip_face_index` module for face-based clip search and grouping
- [ ] Add `clip_scene_detect` module automatically creating subclips at scene boundaries
- [ ] Implement `clip_favorites` module with quick-access collections and recent clips list

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
