# oximedia-access TODO

## Current Status
- 29 top-level modules, ~68 source files total
- Audio description (generator, mixer, scripting, timing, templates), closed captions (generate, style, position, sync), sign language overlay, TTS/STT, transcripts, translation
- Visual enhancements (contrast, color blindness, high contrast, sizing)
- Audio enhancements (clarity, noise reduction, normalization)
- Speed control with pitch preservation
- Compliance checking (WCAG, Section 508, EBU)
- RBAC, session management, token handling, keyboard navigation, screen reader support
- Dependencies: oximedia-core, oximedia-audio, oximedia-subtitle, oximedia-graph

## Enhancements
- [x] Add real-time caption synchronization adjustment in `caption/sync` (user-adjustable offset) (verified 2026-05-16; src/caption/sync.rs:21 offset_ms, SyncAdjustment:18, with_offset:45)
- [x] Implement multi-speaker differentiation in `caption/generate` with speaker labels and color coding (verified 2026-05-16; src/caption/generate.rs:28 identify_speakers, SpeakerColor:35, SpeakerLabel:41)
- [x] Add emoji and symbol descriptions in `audio_desc/generator` for screen reader contexts (verified 2026-05-16; src/audio_desc/generator.rs:105 expand_emoji_and_symbols, EMOJI_TABLE:103, 188 fn body)
- [x] Implement adaptive font sizing in `caption/style` based on display resolution/distance (verified 2026-05-16; src/adaptive_font.rs:45 FontSizePolicy, measure_line:102, font size from resolution)
- [x] Add caption region collision avoidance in `caption/position` (prevent overlap with burned-in text) (verified 2026-05-16; src/caption_collision.rs:46 spatial_overlap_area, temporal_overlap_ms:57, collision detection)
- [x] Implement progressive enhancement in `visual/contrast` (multiple enhancement levels)
- [x] Add customizable color blind simulation modes in `color_blind` (protanopia, deuteranopia, tritanopia preview) (verified 2026-05-16; src/color_blind_sim.rs:169 protanopia/deuteranopia/tritanopia matrices, SimulationConfig:227)
- [x] Implement SSML prosody control in `tts/prosody` for emphasis and pacing
- [x] Add confidence scores to `stt/transcribe` output for quality assessment (verified 2026-05-16; src/stt/transcribe.rs:13 WordConfidence.confidence:21, quality assessment:46, is_uncertain:35)
- [x] Improve `compliance/wcag` checker to cover WCAG 2.2 criteria (focus appearance, dragging alternatives) (verified 2026-05-16; src/wcag_checker.rs:200 WCAG 2.1/2.2 checker, includes 2.2 criteria per module doc)

## New Features
- [x] Add live captioning pipeline: STT -> caption generation -> rendering in real-time
- [ ] Implement automatic audio description from scene analysis (integrate with oximedia-analysis) (verified-open 2026-05-16: audio_desc/generator.rs has scripting/template but no scene analysis integration with oximedia-analysis)
- [x] Add haptic feedback description generation for touch-enabled devices (verified 2026-05-16; src/haptic.rs 553 lines)
- [x] Implement reading level assessment in `transcript` for plain language compliance
- [x] Add ASL/BSL/JSL-specific sign language grammar rules in `sign` module (verified 2026-05-16; src/sign/grammar.rs:1 ASL/BSL/JSL grammar rules, 497 lines)
- [x] Implement audio spatializer for accessibility (directional cues for visually impaired users) (verified 2026-05-16; src/spatial_accessibility.rs 637 lines)
- [x] Add cognitive load assessment for media content complexity
- [x] Implement auto-generated image alt-text from visual features in `media_alt_text` (verified 2026-05-16; src/auto_alt_text.rs 748 lines)
- [ ] Add braille display output format support in `screen_reader` (verified-open 2026-05-16: no braille*.rs module in access sources; screen_reader has no braille output)

## Performance
- [ ] Cache TTS synthesis results in `tts/synthesize` for repeated text segments
- [ ] Implement incremental STT processing in `stt` (process audio chunks, not entire files)
- [ ] Add parallel compliance checking across multiple media files in `compliance`
- [ ] Optimize `audio/clarity` enhancement pipeline with pre-computed filter coefficients
- [ ] Use SIMD for contrast enhancement calculations in `visual/contrast`

## Testing
- [ ] Add end-to-end test: generate captions -> style -> render -> verify positioning
- [ ] Test `compliance/section508` checker against known-compliant and non-compliant media samples
- [ ] Add WCAG color contrast ratio validation tests for all `caption/style` presets
- [ ] Test `audio_desc/timing` gap detection with overlapping dialogue scenarios
- [ ] Test `translate/quality` scoring against human-evaluated reference translations
- [ ] Add integration test: STT -> transcript -> translate -> caption pipeline

## Documentation
- [ ] Add compliance checklist mapping WCAG 2.1 success criteria to module functions
- [ ] Document supported languages for `translate` and `stt` modules
- [ ] Add example workflow: ingest media -> check compliance -> generate accessibility assets -> re-check
