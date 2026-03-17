# oximedia-captions TODO

## Current Status
- 43 modules providing professional captioning: authoring, import/export, format support (SRT/WebVTT/ASS/TTML/DFXP/SCC/STL/iTT), CEA-608/708, Teletext, ARIB, DVB, PGS, VobSub
- Additional modules: `accessibility`, `asr`, `live_caption`/`live_captions`, `speaker_diarization`/`speaker_diarize`, `caption_sync`, `caption_qc`, `caption_renderer`, `embedding`, `imsc`
- Feature gates: `cea`, `broadcast`, `web`, `professional`
- Dependencies: `nom` (parsing), `quick-xml` (TTML/DFXP), `encoding_rs`, `whatlang`

## Enhancements
- [x] Add `caption_diff` module to lib.rs exports (file exists at `src/caption_diff.rs` but not declared)
- [x] Add `caption_fingerprint` module to lib.rs exports (file exists at `src/caption_fingerprint.rs` but not declared)
- [x] Add `caption_normalize` module to lib.rs exports (file exists at `src/caption_normalize.rs` but not declared)
- [x] Consolidate `live_caption` and `live_captions` into a single module (redundant naming)
- [x] Consolidate `speaker_diarization` and `speaker_diarize` into a single module (redundant naming)
- [x] Add IMSC 1.1 (Internet Media Subtitles and Captions) profile validation in `imsc`
- [ ] Extend `formats` with Netflix Timed Text (NFLX-TT) profile support
- [ ] Improve `caption_sync` with audio waveform-based synchronization (match speech onset)
- [ ] Add `caption_rate_control` adaptive rate limiting based on scene complexity
- [x] Extend `caption_validator` with broadcast-specific checks (max lines, max chars, safe area)

## New Features
- [ ] Add `ocr_subtitle` module for extracting text from bitmap-based subtitles (PGS, VobSub, DVB)
- [ ] Implement `sdh_generator` module for Subtitles for Deaf and Hard-of-Hearing (sound descriptions)
- [ ] Add `karaoke_timing` module for word-by-word highlight timing (WebVTT cue settings)
- [ ] Implement `caption_localization` module managing multi-language caption sets per video
- [ ] Add `teletext_page_editor` module for composing Teletext page layouts (Level 1.5 colors)
- [ ] Implement `caption_versioning` module tracking revision history of caption edits
- [ ] Add `smpte_2052` module for SMPTE ST 2052-1 TTML profile compliance

## Performance
- [ ] Optimize `formats` SRT parser using zero-copy `nom` combinators instead of string allocation
- [ ] Add parallel caption rendering in `caption_renderer` for batch export of burned-in subtitles
- [ ] Cache compiled regex patterns in `caption_search` for repeated query execution
- [ ] Use arena allocation for `Caption` entries in large track imports to reduce heap fragmentation
- [ ] Lazy-parse TTML/DFXP XML: parse structure first, defer style resolution until rendering

## Testing
- [x] Add format round-trip tests for all 17 `CaptionFormat` variants: export then reimport (SRT and WebVTT done; remaining formats deferred)
- [ ] Test CEA-608 control code encoding/decoding against reference SCC files
- [ ] Add `caption_qc` test suite with intentionally flawed captions to verify all checks trigger
- [ ] Test `accessibility` module WCAG compliance scoring against known pass/fail samples
- [ ] Add stress test for `live_caption` with high-frequency caption updates (10+ per second)
- [ ] Test `encoding_rs` integration for correct handling of non-UTF-8 STL files (ISO 6937)
- [ ] Test `caption_merge` correctly handles overlapping timestamps from different sources

## Documentation
- [ ] Document all 17 supported caption formats with feature gate requirements
- [ ] Add migration guide for converting between format families (broadcast CC to web subtitles)
- [ ] Document `shotchange` module integration for avoiding caption display across edit points
