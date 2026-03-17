# oximedia-dolbyvision TODO

## Current Status
- 32 source files covering RPU parsing, writing, tone mapping, metadata blocks, and profile management
- Metadata-only implementation respecting Dolby intellectual property
- Supports Profiles 5, 7, 8, 8.1, 8.4 with full level metadata (L1-L11)
- Features: RPU parse/write (NAL and bitstream), tone mapping, XML metadata, shot boundary detection
- Dependencies: oximedia-core, bitstream-io, bitflags, bytes; optional serde

## Enhancements
- [x] Add Level 4 (global dimming) and Level 7 (source display color volume) metadata support
- [x] Implement RPU data validation for all metadata levels in `validation.rs` (currently only checks header)
- [x] Add round-trip fidelity verification in `parser.rs` / `writer.rs` (parse -> write -> parse should be identical)
- [x] Enhance `profile_convert.rs` with Profile 8 -> Profile 8.4 (HDR10 to HLG) conversion
- [ ] Add automatic profile detection from RPU header fields
- [ ] Improve `scene_trim.rs` with scene-change detection heuristics from L1 metadata
- [ ] Add `cm_analysis.rs` content mapping analysis with histogram-based scene statistics
- [ ] Implement RPU merging for combining metadata from multiple sources in `shot_metadata_ext.rs`
- [ ] Add streaming RPU parser that processes NAL units incrementally without buffering full stream

## New Features
- [ ] Implement Dolby Vision to HDR10+ metadata conversion bridge
- [ ] Add RPU metadata visualization/plotting utilities for debugging
- [ ] Implement `ambient_metadata.rs` ambient light adaptation metadata generation
- [ ] Add batch RPU extraction from HEVC/AVC bitstreams
- [ ] Implement RPU metadata timeline editing (insert, delete, retime RPU entries)
- [ ] Add Profile 10 (AV1-based Dolby Vision) metadata structure support
- [ ] Implement `dv_xml_metadata.rs` full Dolby Vision XML round-trip (parse + generate)
- [ ] Add RPU statistics reporting (min/max/avg luminance per scene)

## Performance
- [ ] Optimize `tonemap.rs` PQ/HLG transfer functions with lookup tables
- [ ] Add SIMD-accelerated `ipt_pq.rs` color space conversion
- [ ] Cache parsed RPU structures in `parser.rs` to avoid re-parsing identical NAL units
- [x] Optimize `mapping_curve.rs` polynomial evaluation with Horner's method
- [ ] Pre-compute `BilateralGrid` and `ColorVolumeLut` for repeated tone mapping operations

## Testing
- [ ] Add conformance tests with known-good RPU bitstreams from reference tools
- [ ] Test all profile conversions in `profile_convert.rs` with real-world metadata samples
- [ ] Add fuzz testing for `parser.rs` to verify robustness against malformed RPU data
- [ ] Test `tone_mapping.rs` output against reference DV display management pipeline
- [ ] Add round-trip tests for all Level metadata types (L1 through L11)
- [ ] Test `xml_metadata.rs` parsing against Dolby Vision XML specification samples

## Documentation
- [ ] Document the RPU binary format structure in `parser.rs` and `writer.rs`
- [ ] Add metadata level reference table documenting each level's purpose and fields
- [ ] Document tone mapping pipeline flow from RPU metadata to display output
