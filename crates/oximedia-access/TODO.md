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
- [ ] Add real-time caption synchronization adjustment in `caption/sync` (user-adjustable offset)
- [ ] Implement multi-speaker differentiation in `caption/generate` with speaker labels and color coding
- [ ] Add emoji and symbol descriptions in `audio_desc/generator` for screen reader contexts
- [ ] Implement adaptive font sizing in `caption/style` based on display resolution/distance
- [ ] Add caption region collision avoidance in `caption/position` (prevent overlap with burned-in text)
- [x] Implement progressive enhancement in `visual/contrast` (multiple enhancement levels)
- [ ] Add customizable color blind simulation modes in `color_blind` (protanopia, deuteranopia, tritanopia preview)
- [x] Implement SSML prosody control in `tts/prosody` for emphasis and pacing
- [ ] Add confidence scores to `stt/transcribe` output for quality assessment
- [ ] Improve `compliance/wcag` checker to cover WCAG 2.2 criteria (focus appearance, dragging alternatives)

## New Features
- [x] Add live captioning pipeline: STT -> caption generation -> rendering in real-time
- [ ] Implement automatic audio description from scene analysis (integrate with oximedia-analysis)
- [ ] Add haptic feedback description generation for touch-enabled devices
- [x] Implement reading level assessment in `transcript` for plain language compliance
- [ ] Add ASL/BSL/JSL-specific sign language grammar rules in `sign` module
- [ ] Implement audio spatializer for accessibility (directional cues for visually impaired users)
- [x] Add cognitive load assessment for media content complexity
- [ ] Implement auto-generated image alt-text from visual features in `media_alt_text`
- [ ] Add braille display output format support in `screen_reader`

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
