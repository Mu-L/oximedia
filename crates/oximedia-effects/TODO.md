# oximedia-effects TODO

## Current Status
- 73 source files spanning audio effects (reverb, delay, modulation, distortion, dynamics, filter, pitch, vocoder) and video effects (blend, chroma key, color grade, grain, lens flare, motion blur, vignette)
- Core `AudioEffect` trait with mono/stereo processing, reset, latency reporting
- Audio: Freeverb, plate reverb, convolution reverb, Schroeder reverb, room reverb, hall reverb
- Audio: Delay, multi-tap, ping-pong, tape echo; chorus, flanger, phaser, tremolo, vibrato, ring mod
- Audio: Overdrive, fuzz, bit crusher, waveshaper; gate, expander, compressor, de-esser, ducking
- Audio: Biquad, state variable, Moog ladder; pitch shifter, time stretch, harmonizer, auto-tune, vocoder
- Audio: Stereo widener, spatial audio, auto-pan, transient shaper, saturation
- Video: Blend modes, chroma key, luma key, barrel lens, chromatic aberration, composite, warp, glitch
- Dependencies: oximedia-core, oximedia-audio, oxifft, rubato, scirs2-core

## Enhancements
- [x] Replace `rustfft` with OxiFFT per COOLJAPAN policy in convolution reverb and pitch/vocoder
- [x] Add parameter smoothing to all effects to prevent zipper noise on real-time parameter changes
- [x] Implement true stereo processing in `reverb/freeverb.rs` with decorrelated L/R (prime offsets, per-channel diffusion)
- [x] Add sidechain input support to `compressor/mod.rs` (with SidechainFilter HPF/LPF/BPF) and `ducking.rs`
- [x] Enhance `pitch/autotune.rs` with chromatic scale, key-aware YIN pitch correction, 12-TET quantization
- [x] Add wet/dry mix control to all effects via the `AudioEffect` trait (verified 2026-05-16; src/wet_dry.rs)
- [x] Implement `eq/mod.rs` with parametric EQ bands (low shelf, high shelf, peaking, notch) (verified 2026-05-16; src/eq/parametric.rs:38 struct EqBand, low_shelf:70 high_shelf:82 notch:94)
- [x] Add feedback saturation modeling to `delay/delay.rs` for analog delay emulation (verified 2026-05-16; `FeedbackSaturationMode` {None,Tape,Tube,Diode} + `saturation_drive` fully implemented in `delay/delay.rs` feedback path; `delay_feedback_saturation_clips` + `delay_output_matches_reference_after_migration` tests added)
- [ ] Enhance `vocoder/channel.rs` with more analysis/synthesis filter bands (32+ bands) (verified-open 2026-05-16: not yet implemented)
- [x] Add oversampling option to `distortion/` effects to reduce aliasing artifacts

## New Features
- [x] Implement convolution-based cabinet simulator for guitar/bass processing (verified 2026-05-16; src/reverb/cabinet.rs:477 lines cabinet simulator)
- [x] Add multi-band compressor splitting signal into low/mid/high bands (verified 2026-05-16; src/dynamics/multiband.rs, Linkwitz-Riley crossovers)
- [x] Implement lookahead limiter for broadcast loudness compliance
- [x] Add spring reverb simulation using waveguide physical modeling (verified 2026-05-16; src/reverb/spring.rs:156 struct SpringReverb)
- [x] Implement stereo-to-surround upmixer (5.1/7.1 channel support) (verified 2026-05-16; src/stereo_upmix.rs)
- [x] Add LUFS loudness metering effect (EBU R128 / ITU-R BS.1770)
- [x] Implement granular synthesis time-stretcher as alternative to rubato (verified 2026-05-16; src/pitch/granular.rs:458 lines GranularSynthesizer)
- [x] Add video effect: motion vector-based optical flow slow motion (verified 2026-05-16; src/video/optical_flow.rs:107 OpticalFlowConfig, MotionVector)
- [x] Implement video effect: AI-free super resolution using edge-directed interpolation (verified 2026-05-16; src/video/super_resolution.rs:105 struct SuperResolution, NEDI edge-directed:245)

## Performance
- [x] Add SIMD-optimized biquad filter processing in `filter/mod.rs` (verified 2026-05-16; src/filter/simd_biquad.rs:556 SimdBiquad 4-sample unrolled vectorized kernel)
- [x] Implement block-based FFT processing in `pitch/shifter.rs` to reduce per-sample overhead (verified 2026-05-16; src/pitch/shifter.rs:120 shift_wsola WSOLA overlap-add)
- [x] Use pre-allocated ring buffers in all delay-based effects instead of `Vec<f32>` (verified 2026-05-16; `CombFilter`/`AllPass` in `reverb/freeverb.rs` + `CombFilter`/`AllpassFilter` in `reverb/schroeder.rs` migrated from `Vec<f32>` to `DelayLine`; predelay buffers in `Freeverb` also migrated; `delay_line_migration_output_bounded_and_decaying` + `delay_line_migration_schroeder_bounded_and_decaying` tests added; `reverb/plate.rs`, `reverb_hall.rs`, `room_reverb.rs`, `tape_echo.rs`, `analog_delay.rs` deferred to a subsequent pass)
- [ ] Add double-buffering in `reverb/convolution.rs` for overlap-add processing (verified-open 2026-05-16: no double-buffer overlap-add in reverb/convolution.rs)
- [ ] Optimize `modulation/chorus.rs` LFO computation with wavetable lookup (verified-open 2026-05-16: no wavetable LFO in modulation/chorus.rs)
- [ ] Profile `video/motion_blur.rs` and add frame accumulation caching (verified-open 2026-05-16: no frame accumulation cache in video/motion_blur.rs)

## Testing
- [ ] Add frequency response tests for all filter types in `filter/` (verify cutoff, Q, gain)
- [ ] Test `reverb/` effects for energy conservation (output energy <= input energy * wet+dry)
- [ ] Add latency compensation verification tests for all effects reporting non-zero latency
- [ ] Test `pitch/shifter.rs` pitch accuracy with sine wave inputs at known frequencies
- [ ] Add aliasing measurement tests for `distortion/` effects with oversampling on/off
- [ ] Test `video/chromakey.rs` with known green-screen test images

## Documentation
- [ ] Document the AudioEffect trait lifecycle (create, set_sample_rate, process, reset)
- [ ] Add signal flow diagrams for complex effects (reverb, vocoder, compressor)
- [ ] Document the video effects compositing pipeline and blend mode formulas
