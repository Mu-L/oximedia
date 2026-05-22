# oximedia-restore TODO

## Current Status
- 38 modules (12 subdirectory modules) covering both audio restoration (click/hum/noise/hiss/crackle/azimuth/wow/flutter/DC/phase) and video restoration (deband, deflicker, film grain, color restore, scan line, telecine detect, upscale)
- Core types: RestoreChain with RestorationStep enum, mono and stereo processing pipelines
- Presets for vinyl restoration, tape restoration, broadcast cleanup
- Dependencies: oximedia-core, oximedia-audio, oxifft, thiserror

## Enhancements
- [x] Add per-step bypass toggle to `RestoreChain` so individual steps can be disabled without removing them (verified 2026-05-16; src/lib.rs:234 WrappedStep with enabled:bool, bypass toggle, disabled:256)
- [x] Implement `RestoreChain::process_multichannel` for surround sound (5.1/7.1) with channel-aware processing (verified 2026-05-16; src/lib.rs:268 SurroundChannel, MultichannelLayout:292, process_surround:651)
- [x] Add step reordering validation in `RestoreChain` (e.g., DC removal should precede click detection) (verified 2026-05-16; src/lib.rs:352 OrderingViolation, validate_order:452, step ordering constants:126)
- [x] Extend `noise::SpectralSubtraction` with adaptive noise floor estimation from initial silence detection (verified 2026-05-16; src/noise/subtract.rs:248 adaptive noise profile update; src/noise/profile.rs:216 silence_threshold detection)
- [x] Add real-time preview mode to `RestoreChain` processing fixed-size blocks with overlap-add (verified 2026-05-16; src/overlap_add.rs:105 OverlapAddProcessor, OverlapAdd:246, real-time block processing:1)
- [ ] Improve `wow::WowFlutterCorrector` with pilot-tone detection for precise speed reference (verified-open 2026-05-16: flutter_repair.rs:127 SpeedVariationEstimator uses reference_freq but no pilot-tone detection; wow/ lacks pilot-tone)
- [x] Add severity/confidence output to `click::ClickDetector` for each detected event (verified 2026-05-16; src/click/detector.rs:68 Click.severity, .confidence:73, compute_severity_confidence:136)
- [x] Extend `hum::HumRemover` with automatic fundamental frequency detection (50Hz vs 60Hz) (verified 2026-05-16; src/hum/remover.rs:154 auto-detect 50/60 Hz hum, fallback to 50 Hz:158, FFT-based detection)

## New Features
- [x] Add a `breath_removal` module for podcast/voiceover restoration (detect and attenuate breaths) (verified 2026-05-16; src/breath_removal.rs 527 lines)
- [x] Implement `reverb_reduction` module using spectral dereverberation techniques (verified 2026-05-16; src/reverb_reduction.rs 315 lines)
- [x] Add `dynamic_eq` module for frequency-dependent compression/expansion
- [x] Implement `loudness_normalization` step (EBU R128 / ITU-R BS.1770) as a RestoreChain step (verified 2026-05-16; src/loudness_normalization.rs 511 lines)
- [x] Add `vinyl_surface_noise` module with adaptive surface noise profiling distinct from click/crackle
- [x] Implement `tape_dropout_repair` module for detecting and interpolating tape dropouts
- [x] Add `harmonic_reconstruct` module to rebuild missing harmonics in bandwidth-limited recordings (verified 2026-05-16; src/harmonic_reconstruct.rs 675 lines)
- [x] Implement `stereo_width` restoration step for collapsed or narrow stereo fields (verified 2026-05-16; src/stereo_width.rs 678 lines)
- [x] Add `restore_undo` with per-step rollback capability using stored intermediate buffers (verified 2026-05-16; src/restore_undo.rs 538 lines)

## Performance
- [ ] Add SIMD-optimized paths in `noise::WienerFilter` for batch FFT processing
- [ ] Implement block-based processing in `RestoreChain::process` to reduce peak memory for long files
- [ ] Use rayon parallel iterators in `batch` restoration of multiple files
- [ ] Cache FFT plans in `spectral_repair` across consecutive process calls with same block size
- [ ] Optimize `click::ClickRemover` interpolation to avoid full-buffer copies per click

## Testing
- [ ] Add tests for `RestoreChain` with all step types combined in a realistic vinyl restoration pipeline
- [ ] Test `process_stereo` with asymmetric corruption (clicks on left channel only)
- [ ] Add golden-file tests comparing restored output against known-good reference for each restoration type
- [ ] Test `AzimuthCorrection` and `PhaseCorrection` skip behavior in mono mode
- [ ] Add stress tests for `WienerFilter` with very short (<100 sample) and very long (>10M sample) inputs

## Documentation
- [ ] Document recommended step ordering for each preset type (vinyl, tape, broadcast)
- [ ] Add signal flow diagrams for the RestoreChain processing pipeline
- [ ] Document the difference between `noise` (broadband), `hiss` (high-frequency), and `crackle` (impulsive) removal
- [ ] Add parameter tuning guide for each restoration step with before/after spectrograms
