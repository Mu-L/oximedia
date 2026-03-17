# oximedia-scopes TODO

## Current Status
- 33 modules covering waveform monitors (luma, RGB parade, RGB overlay, YCbCr), vectorscope (circular/rectangular), histogram (RGB/luma), parade displays, false color exposure, CIE 1931 chromaticity, focus assist, HDR waveform (PQ/HLG/nits), audio scopes, bit depth analysis, clipping detection, gamut scope, Lissajous, loudness scope, motion vector scope, overlay rendering, peaking, zebra patterns, signal stats
- Core types: VideoScopes, ScopeConfig, ScopeType (14 variants), ScopeData, WaveformMode, VectorscopeMode, HistogramMode, GamutColorspace
- Dependencies: oximedia-core, rayon, thiserror

## Enhancements
- [x] Add 10-bit and 12-bit precision paths in `waveform` (currently implied but not explicit in ScopeConfig)
- [x] Extend `vectorscope` with zoom/pan controls and IQ vs UV display mode selection
- [x] Add `histogram_stats` percentile markers (1%, 5%, 95%, 99%) overlay on histogram display
- [x] Implement `clipping_detector` with configurable head/foot room thresholds per broadcast standard
- [x] Extend `false_color_mapping` with custom user-defined color LUT support
- [ ] Add `gamut_scope` with triangle outline overlay for Rec.709/P3/Rec.2020 boundaries on CIE diagram
- [ ] Implement `audio_scope` stereo phase correlation meter alongside existing Lissajous display
- [ ] Add `scope_layout` multi-scope grid arrangement with configurable scope combinations

## New Features
- [ ] Add `timecode_overlay` module for burning timecode and frame number onto scope displays
- [ ] Implement `scope_comparison` for side-by-side before/after scope display of color-graded footage
- [ ] Add `safe_area_overlay` module showing title-safe and action-safe boundaries per broadcast standard
- [ ] Implement `scope_recording` for capturing scope data over time as time-series for analysis
- [ ] Add `exposure_histogram` module with zone system (Ansel Adams) overlay for cinematography
- [ ] Implement `chroma_level_scope` dedicated Cb/Cr level display with broadcast-legal limits
- [ ] Add `scope_snapshot` for exporting scope displays as PNG images with metadata annotations
- [ ] Implement `noise_floor_scope` for measuring and displaying signal-to-noise ratio per channel
- [ ] Add `color_checker_scope` for analyzing captured color checker charts against reference values

## Performance
- [ ] Add SIMD-optimized RGB-to-YCbCr conversion in `waveform` and `vectorscope` generation
- [ ] Implement downsampled analysis in `analyze` for real-time preview at reduced accuracy
- [ ] Add incremental scope update in `VideoScopes` (update from changed regions only)
- [ ] Use rayon parallel column processing in `parade::generate_rgb_parade`
- [ ] Optimize `histogram` bin accumulation with thread-local bins merged at end
- [ ] Add configurable scope resolution independent of output resolution for faster rendering

## Testing
- [ ] Add tests for `waveform` with known test patterns (color bars, ramp, flat field)
- [ ] Test `vectorscope` with SMPTE color bar input and verify target dot positions
- [ ] Add tests for `false_color` mapping consistency across Rec.709 and HLG inputs
- [ ] Test `histogram` with uniform-distribution input and verify flat histogram output
- [ ] Add roundtrip tests for `ScopeConfig` serialization/deserialization

## Documentation
- [ ] Add visual reference guide showing each scope type with example output images
- [ ] Document broadcast compliance checking workflow using `compliance` module
- [ ] Add guide for interpreting vectorscope displays (skin tone line, gamut boundaries, color balance)
- [ ] Document HDR scope usage with PQ, HLG, and nits scale interpretation
