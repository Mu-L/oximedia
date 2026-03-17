# oximedia-normalize TODO

## Current Status
- 43 source files covering loudness normalization for broadcast and streaming
- Core: Normalizer (two-pass), RealtimeNormalizer (one-pass with lookahead), BatchProcessor (directory processing)
- Standards: EBU R128, ATSC A/85, Spotify, YouTube, Apple Music, Netflix, Amazon Prime, ReplayGain
- Processing: linear gain, DRC, true peak limiting, multiband normalization, multipass
- Analysis: LoudnessAnalyzer, compliance checking, loudness history tracking
- Advanced: AGC, adaptive normalization, dialogue normalization, sidechain, phase correction, DC offset removal
- Metadata: ReplayGain tags, R128 tags, iTunes Sound Check, loudness descriptors

## Enhancements
- [ ] Integrate phase_correction into the main Normalizer pipeline as an optional pre-processing step
- [ ] Implement actual spectral_balance processing -- equalize frequency balance before normalization
- [ ] Wire dc_offset removal into the processing chain as a pre-processing step (before analysis)
- [ ] Add crossfade_norm support for gapless album normalization (crossfade between tracks at target level)
- [ ] Improve surround_norm to handle 7.1.4 Atmos layouts with proper channel weighting
- [ ] Add stem_loudness support for normalizing individual stems (vocals, drums, bass) independently
- [ ] Implement voice_activity detection to apply dialogue normalization only to speech segments
- [ ] Add loudness gate to agc module to prevent AGC from boosting silence/noise between speech

## New Features
- [ ] Implement Podcast loudness standard (-16 LUFS for Spotify, -14 LUFS for Apple Podcasts)
- [ ] Add cinema loudness normalization (Dolby Atmos -27 LUFS dialogue-gated measurement)
- [ ] Implement album/compilation normalization mode -- normalize all tracks relative to the loudest track
- [ ] Add live streaming normalization with adaptive target (adjust target based on content type detection)
- [ ] Implement loudness normalization with dynamics preservation (multiband approach preserving LRA)
- [ ] Add A/B comparison output -- generate both normalized and original for quality assessment
- [ ] Implement automatic format detection and appropriate standard selection (broadcast file -> EBU R128, music file -> Spotify)

## Performance
- [ ] Process channels in parallel using rayon for surround content (>2 channels)
- [ ] Use SIMD for gain application loop (multiply all samples by gain factor)
- [ ] Implement in-place processing mode to avoid the separate input/output buffer requirement
- [ ] Add buffer recycling in RealtimeNormalizer to reduce allocation during streaming
- [ ] Optimize true_peak_limiter lookahead buffer with circular buffer instead of shifting

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
