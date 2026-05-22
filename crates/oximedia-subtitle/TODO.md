# oximedia-subtitle TODO

## Current Status
- 47 source files across 33+ public modules covering subtitle parsing, rendering, and processing
- Parsers: SRT, WebVTT, SSA/ASS, CEA-608/708, TTML, PGS, DVB (7 format parsers)
- Rendering: font loading (fontdue + ab_glyph), glyph caching, burn-in, overlay composition
- Text: bidirectional text (unicode-bidi), Unicode segmentation, line breaking
- Processing: timing adjustment, format conversion, merge, diff, search, stats, sanitize, validation, overlap detection, reading speed analysis, segmentation, spell check, cue parsing/timing, position calculation, accessibility, translation
- Export: subtitle_export with multi-format output; subtitle_index for timestamp-indexed lookup
- Dependencies: oximedia-core, oximedia-codec, nom, fontdue, ab_glyph, unicode-bidi, unicode-segmentation, quick-xml

## Enhancements
- [x] Add HTML tag stripping and formatting preservation in `parser::srt` for complex SRT files with `<b>`, `<i>`, `<font color>` tags
- [x] Extend `parser::webvtt` to support region definitions and vertical text cue settings (verified 2026-05-16; src/parser/webvtt.rs:60 VttRegion struct, vertical text cue settings:94)
- [ ] Improve `parser::ssa` to handle all ASS override tags including `\clip`, `\iclip`, `\org`, `\fad`, `\move` (verified-open 2026-05-16: not yet fully implemented)
- [ ] Add fallback font chain support in `font::Font` for missing glyphs (CJK, Arabic, Devanagari) (verified-open 2026-05-16: not yet implemented)
- [ ] Extend `burn_in` to support soft-shadow rendering with configurable blur radius (verified-open 2026-05-16: not yet implemented)
- [x] Improve `reading_speed` module to account for word complexity (not just character count per second)
- [x] Add `timing_adjuster` support for non-linear time remapping (speed ramp, variable frame rate)
- [x] Extend `subtitle_validator` to check for WCAG 2.1 AA contrast ratio compliance (verified 2026-05-16; src/wcag_validator.rs:71 WcagValidator, WcagLevel:50)

## New Features
- [x] Implement `parser::eia608_realtime` module for live CEA-608 caption decoding from a byte stream (verified 2026-05-16; src/eia608_realtime.rs:441 lines, src/parser/eia608_realtime.rs)
- [x] Add `karaoke_engine` module implementing full ASS karaoke timing with syllable-level highlighting (verified 2026-05-16; src/karaoke_engine.rs:513 lines)
- [x] Implement `teletext` parser module for DVB Teletext subtitle extraction (ETS 300 706) (verified 2026-05-16; src/teletext.rs:601 lines, src/teletext_subtitle.rs)
- [x] Add `subtitle_alignment` module for automatic timing alignment between two subtitle tracks (e.g., sync fansubs to official audio) (verified 2026-05-16; src/subtitle_alignment.rs:598 lines)
- [x] Implement `forced_subtitle` detection and flagging for narrative foreign-language subtitles
- [x] Add `sign_language` module for sign language overlay region management and picture-in-picture positioning (verified 2026-05-16; src/sign_language.rs:817 lines)
- [x] Implement `subtitle_ocr` module for extracting text from bitmap-based subtitle formats (PGS, VobSub) using pattern matching (verified 2026-05-16; src/subtitle_ocr.rs:795 lines)
- [x] Add `live_caption` module for real-time caption display with roll-up and paint-on modes (verified 2026-05-16; src/live_caption.rs:549 lines)

## Performance
- [x] Implement glyph atlas caching in `font::GlyphCache` to batch render frequently used characters into a texture atlas (verified 2026-05-16; src/glyph_atlas.rs:32 GlyphAtlas config, AtlasConfig, pixel_slice_mut:173)
- [x] Use binary search in `subtitle_index` for O(log n) timestamp lookup instead of linear scan
- [ ] Add parallel subtitle parsing in `format_converter` for batch conversion of large subtitle collections
- [ ] Cache parsed ASS style lookups in `parser::ssa` to avoid re-parsing style definitions per dialogue line
- [ ] Implement incremental rendering in `renderer::SubtitleRenderer` that only redraws changed regions

## Testing
- [ ] Add conformance tests for CEA-608 decoder with known test streams (FCC captioning test patterns)
- [ ] Test `format_convert` round-trip: SRT -> WebVTT -> SRT and verify timing preservation within 1ms
- [ ] Add edge case tests for `parser::srt` with malformed files (missing blank lines, BOM markers, CR/LF variations)
- [ ] Test `overlap_detect` with intentionally overlapping subtitle tracks and verify all collisions are reported
- [ ] Add bidirectional text rendering test with mixed Arabic/Hebrew and Latin text in `text::TextLayoutEngine`
- [ ] Test `subtitle_merge` with two tracks having partially overlapping time ranges

## Documentation
- [ ] Add format comparison table showing feature support across SRT, WebVTT, ASS, TTML, CEA-608/708
- [ ] Document font loading requirements and supported font formats (TTF, OTF, TTC)
- [ ] Add example showing subtitle burn-in pipeline: parse -> style -> render -> composite onto video frame
