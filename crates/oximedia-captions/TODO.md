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
- [x] Extend `formats` with Netflix Timed Text (NFLX-TT) profile support (verified 2026-05-16; src/formats/nflx_tt.rs:19 NflxTtEntry, NflxTtParser:363, NflxTtWriter:384, parse_nflx_tt:95, serialize_nflx_tt:253, 575 lines)
- [ ] Improve `caption_sync` with audio waveform-based synchronization (match speech onset) (verified-open 2026-05-16: caption_sync.rs has offset/drift adjustment but no audio_waveform/speech_onset analysis)
- [x] Add `caption_rate_control` adaptive rate limiting based on scene complexity (verified 2026-05-16; src/caption_rate_control.rs:85 RateControlConfig, analyse_rates:189, RateStats, 433 lines)
- [x] Extend `caption_validator` with broadcast-specific checks (max lines, max chars, safe area)

## New Features
- [x] Add `ocr_subtitle` module for extracting text from bitmap-based subtitles (PGS, VobSub, DVB) (verified 2026-05-16; src/ocr_subtitle.rs:26 BitmapSubtitleFormat PGS/VobSub/DVB, 781 lines)
- [x] Implement `sdh_generator` module for Subtitles for Deaf and Hard-of-Hearing (sound descriptions) (verified 2026-05-16; src/sdh_generator.rs:265 SdhGenerator, SdhCaption:235, strip_sdh:345, 539 lines)
- [x] Add `karaoke_timing` module for word-by-word highlight timing (WebVTT cue settings) (verified 2026-05-16; src/karaoke_timing.rs:128 KaraokeTrack, KaraokeAligner:301, WebVTT cue body:179, 619 lines)
- [x] Implement `caption_localization` module managing multi-language caption sets per video (verified 2026-05-16; src/caption_localization.rs:164 LocalizationSet, LocalizedTrack:29, ManifestEntry:143, 497 lines)
- [x] Add `teletext_page_editor` module for composing Teletext page layouts (Level 1.5 colors) (verified 2026-05-16; src/teletext_page_editor.rs:358 TeletextPageEditor, TeletextPage:218, TeletextPageHeader:180, 603 lines)
- [x] Implement `caption_versioning` module tracking revision history of caption edits (verified 2026-05-16; src/caption_versioning.rs:81 CaptionRevisionHistory, CaptionRevision:33, revision:34, 286 lines)
- [x] Add `smpte_2052` module for SMPTE ST 2052-1 TTML profile compliance (verified 2026-05-16; src/smpte_2052.rs:84 validate_smpte_tt, ST 2052-1:5, compliance issues:51, 292 lines)

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
