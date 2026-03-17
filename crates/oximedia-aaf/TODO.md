# oximedia-aaf TODO

## Current Status
- 33 modules covering full AAF object model, structured storage, composition mobs, effects, timeline, dictionary, essence, metadata
- Reader (`AafReader`) and writer (`AafWriter`) with SMPTE ST 377-1 compliance
- Export to EDL, XML, and OpenTimelineIO formats
- Dependencies: oximedia-core, oximedia-timecode, uuid, chrono, byteorder, serde, bitflags, bytes

## Enhancements
- [ ] Add streaming/incremental reading to `AafReader` for large AAF files (avoid loading entire file into memory)
- [ ] Implement lazy essence data loading in `read_essence_data` (load on demand, not upfront)
- [x] Add mob cloning/duplication support in `ContentStorage` with new UUID generation
- [x] Implement `find_composition_mob` by name (not just UUID) for ergonomic lookups
- [ ] Add track re-ordering and insertion APIs to `CompositionMob`
- [ ] Implement nested effect parameter keyframe interpolation in `parameter` module
- [x] Add validation pass after reading: verify all mob references resolve, all required properties present
- [ ] Support AAF Edit Protocol (read/modify/write without losing unknown properties)
- [x] Add `Display` implementations for `EditRate`, `Position`, and timeline types for debugging

## New Features
- [ ] Implement AAF low-level dump/inspection tool (hex + structure view) for debugging corrupt files
- [ ] Add merge capability: combine multiple AAF files into a single composition
- [ ] Implement Final Cut Pro XML (FCPXML) export in `convert` module
- [ ] Add DaVinci Resolve EDL dialect support in `edl_export`
- [ ] Implement AAF metadata search/query API (find mobs/clips matching criteria)
- [ ] Add Avid bin structure reading/writing for Avid Media Composer compatibility
- [ ] Implement essence relinking: update media file references when paths change
- [ ] Add timeline flattening: resolve nested compositions into a single flat sequence

## Performance
- [ ] Cache parsed dictionary entries to avoid re-parsing on repeated lookups
- [ ] Use memory-mapped I/O for large structured storage files in `StorageReader`
- [ ] Implement zero-copy byte slicing for essence data reading where possible
- [ ] Add parallel mob/track parsing for large compositions with many tracks

## Testing
- [x] Add round-trip test: create AAF -> write -> read -> verify all fields match
- [ ] Test `edl_export` output against reference EDL files from Avid/Premiere
- [ ] Add tests for edge cases: empty compositions, single-frame clips, nested effects
- [ ] Test `xml_bridge` XML serialization/deserialization round-trip
- [ ] Test handling of corrupted structured storage headers (graceful error, not panic)
- [ ] Add tests for `mob_traversal` with deep mob reference chains

## Documentation
- [ ] Document the AAF object model hierarchy with a diagram (Header -> ContentStorage -> Mobs -> Slots)
- [ ] Add examples for common workflows: read AAF, extract clip list, export EDL
- [ ] Document supported AAF versions and known limitations vs. Avid/Adobe implementations
