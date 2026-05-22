# oximedia-imf TODO

## Current Status
- 37+ modules implementing SMPTE ST 2067 Interoperable Master Format
- Core: CPL (Composition Playlist), PKL (Packing List), AssetMap, OPL (Output Profile List)
- Parsing: cpl_parser, pkl_document, opl_document, xml_util (quick-xml based)
- Validation: validator, cpl_validator, package_validator with conformance levels
- Essence: MXF descriptors, essence constraints, track files, audio layout
- Advanced: CPL merge, supplemental packages, versioning, content versions
- Timeline: imf_timeline, composition_sequence, cpl_segment
- Metadata: marker_list, marker_resource, subtitle_resource, sidecar, IMSC1 subtitles
- Delivery: delivery, output_profile_list, application_profile, imf_report
- Hash: SHA-1 and MD5 verification via sha1 and md-5 crates
- Dependencies: oximedia-core, quick-xml, uuid, chrono, serde, sha1, md-5, hex

## Enhancements
- [x] Add SHA-256 and SHA-512 hash support to `essence_hash.rs` alongside existing SHA-1/MD5
- [x] Extend `cpl_validator.rs` with SMPTE ST 2067-2:2020 (latest revision) constraint checks (verified 2026-05-16; src/cpl_validator.rs:317 ST 2067-2:2020 §6.1 edit rates, §6.4 segment/UUID constraints)
- [x] Add incremental hash computation to `essence_hash.rs` for large MXF files (streaming digest)
- [x] Implement CPL diff in `cpl_merge.rs` to show what changed between two compositions
- [ ] Extend `application_profile.rs` with Netflix IMF App 2.1 and Disney DECE profiles (verified-open 2026-05-16: only App2/App2Extended/App4/App5Aces/App6/App7/Iabmm; no Netflix/DECE variants)
- [x] Add timeline gap detection and overlap reporting in `imf_timeline.rs` (verified 2026-05-16; src/imf_timeline.rs:52 overlaps fn, validate_timeline gaps/overlaps:198)
- [ ] Implement `subtitle_resource.rs` support for TTML and WebVTT subtitle formats (verified-open 2026-05-16: subtitle_resource.rs only handles IMSC1 reference, no TTML/WebVTT parse)
- [ ] Extend `versioning.rs` with automatic version increment and change log generation (verified-open 2026-05-16: no auto_increment or change_log found in versioning.rs)

## New Features
- [x] Add an `imf_builder.rs` high-level API for creating IMF packages from scratch with fluent interface
- [x] Implement an `imf_inspector.rs` module for detailed package inspection with human-readable report (verified 2026-05-16; src/imf_inspector.rs:711 lines)
- [x] Add an `essence_probe.rs` module for probing MXF essence files without full parse (quick metadata) (verified 2026-05-16; src/essence_probe.rs:784 lines)
- [x] Implement a `qc_report.rs` module for automated quality control reporting (EBU QC checks) (verified 2026-05-16; src/qc_report.rs:632 lines)
- [x] Add a `compliance_matrix.rs` module mapping application profiles to required constraints (verified 2026-05-16; src/compliance_matrix.rs:934 lines)
- [x] Implement an `imf_diff.rs` module for comparing two IMF packages (structural and content diff) (verified 2026-05-16; src/imf_diff.rs:570 lines)
- [x] Add a `partial_restore.rs` module for extracting specific segments/tracks from an IMP (verified 2026-05-16; src/partial_restore.rs:285 lines)
- [x] Implement a `metadata_extractor.rs` module for extracting all metadata to JSON/XML export (verified 2026-05-16; src/metadata_extractor.rs:249 lines)
- [x] Add an `imf_archive.rs` module for creating archival packages with long-term preservation metadata (verified 2026-05-16; src/imf_archive.rs:325 lines)

## Performance
- [x] Add parallel hash verification in `package_validator.rs` using rayon for multi-asset packages
- [ ] Implement lazy XML parsing in `cpl_parser.rs` (parse only requested sections) (verified-open 2026-05-16: no lazy/on-demand parsing in cpl_parser.rs)
- [ ] Cache parsed CPL/PKL structures to avoid re-parsing during repeated validation (verified-open 2026-05-16: not yet implemented)
- [ ] Add streaming XML writing in `xml_util.rs` for large OPL/CPL generation (verified-open 2026-05-16: not yet implemented)

## Testing
- [ ] Add conformance tests with reference IMF packages from the SMPTE IMF plugfest test suite
- [ ] Test `cpl_merge.rs` with conflicting edit rates and overlapping timeline segments
- [x] Add round-trip tests: create CPL -> serialize to XML -> parse XML -> compare structure
- [ ] Test `package_validator.rs` with intentionally corrupted hash values
- [ ] Add `supplemental_package.rs` tests with multi-level supplemental chain
- [ ] Test `imsc1.rs` subtitle parsing with multi-language and styled IMSC1 documents

## Documentation
- [ ] Add a reference table mapping each module to its SMPTE standard section
- [ ] Document the CPL/PKL/AssetMap relationship with a structural diagram
- [ ] Add examples for creating and validating common IMF delivery scenarios (broadcast, OTT, archive)
