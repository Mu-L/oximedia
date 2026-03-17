# oximedia-metering TODO

## Current Status
- 47 source files covering audio loudness, peak metering, video quality, spectrum analysis, and meter rendering
- Standards: ITU-R BS.1770-4, EBU R128, ATSC A/85, streaming platforms (Spotify, YouTube, Apple Music, Netflix, Amazon Prime)
- Key components: LoudnessMeter, PeakMeter, KSystemMeter, PhaseCorrelationMeter, SpectrumAnalyzer, LuminanceMeter, GamutMeter, QualityAnalyzer
- Wave 12 modules: crest_factor, k_weighted, meter_bridge
- Wave 15 modules: loudness_trend, noise_floor, stereo_balance

## Enhancements
- [x] Unify `true_peak.rs` and `truepeak.rs` -- resolved via `pub use truepeak as true_peak` re-export in lib.rs
- [x] Unify `correlation.rs` and `correlation_meter.rs` -- resolved via `pub use correlation as correlation_meter` re-export in lib.rs
- [x] Unify `phase.rs` and `phase_analysis.rs` -- resolved via `pub use phase as phase_analysis` re-export in lib.rs
- [x] Unify `peak.rs` and `peak_meter.rs` -- resolved via `pub use peak as peak_meter` re-export in lib.rs
- [x] Unify `dynamics.rs` and `dynamic_range_meter.rs` and `dr_meter.rs` -- resolved via `pub use dynamics as dynamic_range_meter` re-export in lib.rs
- [ ] Add 8x oversampling option to TruePeakDetector for mastering-grade precision
- [ ] Implement BS.2051 channel weights for NHK 22.2 immersive audio layout
- [x] Add Tidal HiFi and Amazon Music HD loudness targets to the Standard enum
- [ ] Extend LuminanceMeter to support HLG (Hybrid Log-Gamma) transfer function in addition to PQ/HDR10
- [ ] Add temporal noise measurement (inter-frame noise) to video_quality module

## New Features
- [ ] Add VMAF (Video Multi-Method Assessment Fusion) estimator using pure Rust feature extraction
- [ ] Implement MS-SSIM (Multi-Scale SSIM) in video_quality for better perceptual quality scoring
- [ ] Add Leq (equivalent continuous sound level) meter for environmental/broadcast compliance
- [ ] Implement ITU-R BS.2132 loudness measurement for short-form content (<30s)
- [ ] Add real-time waveform display data generation (oscilloscope-style) to render module
- [ ] Implement vectorscope data generation for video color analysis in render module

## Performance
- [x] Replace rustfft with OxiFFT in spectrum analysis (COOLJAPAN policy) -- Cargo.toml already uses `oxifft.workspace = true`, no rustfft present
- [x] Remove ndarray dependency -- Cargo.toml has no ndarray; video frames already use flat Vec<f64>
- [ ] Add SIMD-accelerated K-weighting filter processing using portable_simd (when stable)
- [ ] Cache FFT plan in SpectrumAnalyzer to avoid repeated allocation per process() call
- [ ] Use rayon parallel iterators for per-channel true peak detection in multi-channel (>4ch) audio

## Testing
- [x] Add reference signal tests: 997 Hz sine at -23 LUFS should measure exactly -23.0 LUFS (EBU R128 test signal) -- `test_ebu_r128_reference_signal` in lib.rs, ±0.5 LUFS tolerance
- [ ] Add gating algorithm conformance tests per ITU-R BS.1770-4 Section 3
- [ ] Test LRA calculation against EBU PLOUD reference values
- [ ] Add round-trip test: normalize to target then re-meter should report target LUFS
- [ ] Test all streaming platform targets (Spotify/YouTube/Apple/Netflix/Amazon) with known-loudness signals

## Documentation
- [ ] Add architecture diagram showing LoudnessMeter internal pipeline (filter -> LKFS -> gating -> LRA)
- [ ] Document the relationship between clip_counter.rs, rms_envelope.rs, silence_detect.rs and the main metering pipeline
- [ ] Add compliance testing guide with step-by-step EBU R128 verification procedure
