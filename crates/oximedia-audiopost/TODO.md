# oximedia-audiopost TODO

## Current Status
- 41 modules covering ADR, Foley, sound design, mixing, restoration, stems, loudness, automation, and delivery
- Key subsystems: `adr`/`adr_manager`, `foley`/`foley_manager`, `mixing`/`mix_session`, `effects`, `restoration`, `stems`/`stem_export`, `loudness`/`loudness_session`, `surround`, `pipeline`
- Dependencies on `rustfft`, `rubato`, `ndarray`, `rand` (policy violations pending cleanup)

## Enhancements
- [x] Replace `rustfft` dependency with OxiFFT per COOLJAPAN policy (already using `oxifft` in Cargo.toml)
- [x] Replace `ndarray` dependency with SciRS2-Core per SCIRS2 policy (ndarray not present; pure Vec<f32> used)
- [x] Replace `rand` dependency with SciRS2-Core RNG per SCIRS2 policy (rand not present; scirs2-core used)
- [ ] Add surround sound upmixing algorithms (stereo-to-5.1, 5.1-to-7.1) in `surround` module
- [ ] Extend `loudness` module with ITU-R BS.1770-4 true-peak measurement
- [ ] Add ARIB TR-B32 loudness standard support for Japanese broadcast in `loudness`
- [ ] Implement convolution reverb engine using impulse response files in `reverb_profile`
- [x] Add de-esser processor to `effects` module with adjustable frequency and threshold (`DeEsser::process` static method added)
- [ ] Extend `restoration` with click/pop removal algorithm for vinyl-sourced audio
- [ ] Add phase correlation meter to `metering` module for stereo/surround monitoring

## New Features
- [x] Add `spectral_editor` module to lib.rs exports (declared at lib.rs line 95)
- [x] Add `clip_gain` module to lib.rs exports (declared at lib.rs line 68)
- [x] Add `phase_alignment` module to lib.rs exports (declared at lib.rs line 86)
- [ ] Implement Dolby Atmos object-based audio layout support in `surround`
- [ ] Add broadcast limiter with true-peak limiting in `pipeline`
- [ ] Implement sample-accurate crossfade engine for seamless take splicing in `take_manager`
- [ ] Add M/S (Mid-Side) encoding/decoding processor in `effects`

## Performance
- [ ] Add SIMD-accelerated sample processing for `mixing::ChannelStrip` gain/pan operations
- [ ] Implement lock-free ring buffer for real-time audio routing in `bus_routing`
- [ ] Add block-based FFT processing with overlap-add in `effects` to reduce per-sample overhead
- [ ] Use SIMD for loudness gating calculation in `loudness` (K-weighted filter + gate)
- [ ] Pre-compute and cache reverb impulse response FFTs in `reverb_profile`

## Testing
- [ ] Add integration test for complete ADR workflow: session create, cue add, record, sync
- [ ] Add property-based tests for `loudness` module against known EBU R128 reference signals
- [ ] Test `stem_export` round-trip: create stems, export, re-import, verify sample accuracy
- [ ] Add stress test for `mixing::MixingConsole` with 128+ channels
- [ ] Test `restoration` noise reduction with synthetic noise profiles

## Documentation
- [ ] Add architecture diagram showing signal flow through `pipeline` module
- [ ] Document supported loudness standards and compliance levels in `loudness` module
- [ ] Add examples for `broadcast_delivery` showing typical delivery spec configurations
