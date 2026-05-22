# oximedia-audiopost TODO

## Current Status
- 41 modules covering ADR, Foley, sound design, mixing, restoration, stems, loudness, automation, and delivery
- Key subsystems: `adr`/`adr_manager`, `foley`/`foley_manager`, `mixing`/`mix_session`, `effects`, `restoration`, `stems`/`stem_export`, `loudness`/`loudness_session`, `surround`, `pipeline`
- Dependencies on `rustfft`, `rubato`, `ndarray`, `rand` (policy violations pending cleanup)
- [x] Migrated `convert_sample_rate()` in `delivery.rs` from removed rubato 1.x `SincFixedIn` API to rubato 2.0 `Async::new_sinc` + `process_all_into_buffer` + `InterleavedOwned` API (2026-05-05)

## Enhancements
- [x] Replace `rustfft` dependency with OxiFFT per COOLJAPAN policy (already using `oxifft` in Cargo.toml)
- [x] Replace `ndarray` dependency with SciRS2-Core per SCIRS2 policy (ndarray not present; pure Vec<f32> used)
- [x] Replace `rand` dependency with SciRS2-Core RNG per SCIRS2 policy (rand not present; scirs2-core used)
- [x] Add surround sound upmixing algorithms (stereo-to-5.1, 5.1-to-7.1) in `surround` module (verified 2026-05-16; src/surround.rs:285 StereoTo51Upmixer:326)
- [x] Extend `loudness` module with ITU-R BS.1770-4 true-peak measurement (planned 2026-05-04)
  - **Goal:** FFT-based spectrum analysis, short-term LUFS, and true-peak detection
  - **Design:** FFT spectrum via OxiFFT (1024-pt). LUFS: wire to oximedia-audio::ebur128 (already exists). True-peak: 4× polyphase upsampling via Kaiser-windowed sinc (β=8.0, 33 taps), max of |upsampled|.
  - **Files:** `crates/oximedia-audiopost/src/metering.rs`
  - **Tests:** `tests/true_peak.rs` — 0 dBFS sine at 1 kHz with 0.5-sample phase offset → true peak ≥0 dBTP (within 0.1 dB)
  - **Risk:** FIR coefficients must be precomputed; runtime is 4× sample count
- [x] Add ARIB TR-B32 loudness standard support for Japanese broadcast in `loudness` (verified 2026-05-16; src/arib_loudness.rs:1 AribLoudnessAnalyzer, ARIB_TARGET_LKFS:38)
- [x] Implement convolution reverb engine using impulse response files in `reverb_profile` (planned 2026-05-04)
  - **Goal:** Wire oximedia-effects Reverb/Chorus/Distortion into a signal-flow chain
  - **Design:** Signal-flow builder: chain reverb→chorus→distortion with configurable parameters. Delegate to existing oximedia-effects implementations.
  - **Files:** `crates/oximedia-audiopost/src/sound_design.rs`
  - **Tests:** `tests/sound_design_chain.rs` — route signal through chain; output bounded, no NaN/Inf, energy plausible
  - **Risk:** Gain staging between effects; normalize output to avoid clipping
- [x] Add de-esser processor to `effects` module with adjustable frequency and threshold (`DeEsser::process` static method added)
- [x] Extend `restoration` with click/pop removal algorithm for vinyl-sourced audio (planned 2026-05-04)
  - **Goal:** Declick (transient detection + interpolation) and denoise (spectral subtraction + Wiener)
  - **Design:** Declick: detect samples where first-difference > N×MAD (N=8); replace click region with cubic-spline interpolation over ±50 surrounding samples. Denoise (Boll 1979): estimate noise PSD from quietest 5% of frames; subtract α·noisePSD (α=2.0), floor at β·noisePSD (β=0.05); Wiener gain = signalPSD/(signalPSD+noisePSD) post-processing. OxiFFT STFT: 1024-pt, 50% overlap, Hann window.
  - **Files:** `crates/oximedia-audiopost/src/restoration.rs`
  - **Tests:** `tests/declick.rs` — 1 kHz sine + 5 clicks → correlation >0.99 after declicker; `tests/spectral_subtraction.rs` — white noise + tone → SNR improves ≥6 dB
  - **Risk:** STFT edge effects — zero-pad and truncate to match-length; Wiener gain clamped to [0,1]
- [x] Add phase correlation meter to `metering` module for stereo/surround monitoring (planned 2026-05-04)
  - **Goal:** FFT-based spectrum analysis, short-term LUFS, and true-peak detection
  - **Design:** FFT spectrum via OxiFFT (1024-pt). LUFS: wire to oximedia-audio::ebur128 (already exists). True-peak: 4× polyphase upsampling via Kaiser-windowed sinc (β=8.0, 33 taps), max of |upsampled|.
  - **Files:** `crates/oximedia-audiopost/src/metering.rs`
  - **Tests:** `tests/true_peak.rs` — 0 dBFS sine at 1 kHz with 0.5-sample phase offset → true peak ≥0 dBTP (within 0.1 dB)
  - **Risk:** FIR coefficients must be precomputed; runtime is 4× sample count

## New Features
- [x] Add `spectral_editor` module to lib.rs exports (declared at lib.rs line 95)
- [x] Add `clip_gain` module to lib.rs exports (declared at lib.rs line 68)
- [x] Add `phase_alignment` module to lib.rs exports (declared at lib.rs line 86)
- [x] Implement Dolby Atmos object-based audio layout support in `surround` (verified 2026-05-16; src/surround.rs:641 AtmosObject:649, AtmosBedLayout:729, Atmos session layout:759)
- [x] Add broadcast limiter with true-peak limiting in `pipeline` (planned 2026-05-04)
  - **Goal:** Pipeline orchestrator: declick→denoise→EQ→compressor→loudness→metering tap
  - **Design:** Sequential DSP chain with metering tap at each stage. Configurable per-stage bypass. Output normalized at -23 LUFS ±0.5 LU.
  - **Files:** `crates/oximedia-audiopost/src/workflow.rs`
  - **Tests:** `tests/workflow.rs` — pipeline yields normalized output at -23 LUFS within ±0.5 LU
  - **Risk:** Stage ordering matters; EQ before compressor, compressor before loudness
- [x] Implement sample-accurate crossfade engine for seamless take splicing in `take_manager` (verified 2026-05-16; src/crossfade_engine.rs:476 lines CrossfadeEngine, sample-accurate splicing)
- [x] Add M/S (Mid-Side) encoding/decoding processor in `effects` (verified 2026-05-16; src/effects.rs:676 MidSideProcessor)

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
- [x] Test `restoration` noise reduction with synthetic noise profiles (planned 2026-05-04)
  - **Goal:** Declick (transient detection + interpolation) and denoise (spectral subtraction + Wiener)
  - **Design:** Declick: detect samples where first-difference > N×MAD (N=8); replace click region with cubic-spline interpolation over ±50 surrounding samples. Denoise (Boll 1979): estimate noise PSD from quietest 5% of frames; subtract α·noisePSD (α=2.0), floor at β·noisePSD (β=0.05); Wiener gain = signalPSD/(signalPSD+noisePSD) post-processing. OxiFFT STFT: 1024-pt, 50% overlap, Hann window.
  - **Files:** `crates/oximedia-audiopost/src/restoration.rs`
  - **Tests:** `tests/declick.rs` — 1 kHz sine + 5 clicks → correlation >0.99 after declicker; `tests/spectral_subtraction.rs` — white noise + tone → SNR improves ≥6 dB
  - **Risk:** STFT edge effects — zero-pad and truncate to match-length; Wiener gain clamped to [0,1]

## Documentation
- [ ] Add architecture diagram showing signal flow through `pipeline` module
- [ ] Document supported loudness standards and compliance levels in `loudness` module
- [ ] Add examples for `broadcast_delivery` showing typical delivery spec configurations
