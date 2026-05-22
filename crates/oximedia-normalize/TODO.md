# oximedia-normalize TODO

## Current Status
- 42 source files covering loudness normalization for broadcast and streaming
- Core: Normalizer (two-pass), RealtimeNormalizer (one-pass with lookahead), BatchProcessor (directory processing)
- Standards: EBU R128, ATSC A/85, Spotify, YouTube, Apple Music, Netflix, Amazon Prime, ReplayGain
- Processing: linear gain, DRC, true peak limiting, multiband normalization, multipass
- Analysis: LoudnessAnalyzer, compliance checking, loudness history tracking
- Advanced: AGC, adaptive normalization, dialogue normalization, sidechain, phase correction, DC offset removal
- Metadata: ReplayGain tags, R128 tags, iTunes Sound Check, loudness descriptors

## Enhancements
- [x] Integrate phase_correction into the main Normalizer pipeline as an optional pre-processing step (verified 2026-05-16; src/lib.rs:175 pub mod phase_correction, src/phase_correction.rs:348 lines)
- [x] Wire dc_offset removal into the processing chain as a pre-processing step (before analysis) (verified 2026-05-16; src/dc_offset.rs:301 lines, pub mod dc_offset in lib.rs)
- [x] Add crossfade_norm support for gapless album normalization (crossfade between tracks at target level) (verified 2026-05-16; src/crossfade_norm.rs:434 lines)
- [x] Improve surround_norm to handle 7.1.4 Atmos layouts with proper channel weighting (verified 2026-05-16; src/surround_norm.rs:25 7.1.4 Atmos bed, src/parallel_channels.rs 7.1.4)

## New Features
- [x] Add A/B comparison output -- generate both normalized and original for quality assessment — `ab_comparison.rs`
- [x] Implement automatic format detection and appropriate standard selection (broadcast file -> EBU R128, music file -> Spotify) — `format_detect.rs`
- [x] Add podcast loudness standard (-16 LUFS for Spotify, -14 LUFS for Apple Podcasts) (verified 2026-05-16; src/podcast_loudness.rs:969 lines)
- [x] Add cinema loudness normalization (Dolby Atmos -27 LUFS dialogue-gated measurement) (verified 2026-05-16; src/cinema_loudness.rs:25 Atmos -27 LUFS, src/dialogue_gate.rs:103 cinema preset)

## Performance
- [x] Process channels in parallel using rayon for surround content (>2 channels) — `parallel_channels.rs`
- [x] Use SIMD for gain application loop (multiply all samples by gain factor) (verified 2026-05-16; src/simd_gain.rs:324 lines)
- [x] Implement in-place processing mode to avoid the separate input/output buffer requirement (verified 2026-05-16; src/peak_limiter.rs:310 process_in_place fn)
- [ ] Add buffer recycling in RealtimeNormalizer to reduce allocation during streaming (verified-open 2026-05-16: not yet implemented)
- [x] Optimize true_peak_limiter lookahead buffer with circular buffer instead of shifting (verified 2026-05-16; src/peak_limiter.rs:153 circular lookahead buffer, zero-copy:4)

## Testing
- [ ] Add EBU R128 conformance test: -23 LUFS input should measure -23 LUFS after null normalization (gain=0)
- [ ] Test two-pass normalizer: -30 LUFS input normalized to -23 LUFS should gain +7 dB
- [ ] Verify true peak limiter: apply +20 dB gain to near-0 dBFS signal, verify output never exceeds -1 dBTP
- [ ] Test BatchProcessor with multiple files verifying consistent target loudness across all outputs
- [ ] Add test for DRC: high-LRA input should produce lower LRA output while maintaining target LUFS
- [ ] Test RealtimeNormalizer latency: verify output delay matches configured lookahead_ms

## Documentation
- [ ] Add normalization workflow guide: when to use two-pass vs one-pass vs batch mode
- [ ] Document the relationship between normalize modules and oximedia-metering (analysis reuse)
- [ ] Add standard selection guide with recommended settings per delivery target
