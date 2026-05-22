# oximedia-denoise TODO

## Current Status
- 22 modules across spatial, temporal, hybrid, motion, grain, multiscale, audio, and video domains
- 45 source files total including submodules (spatial/bilateral, spatial/nlmeans, spatial/wiener, spatial/wavelet, temporal/kalman, temporal/median, temporal/motioncomp, grain/analysis, grain/preserve, etc.)
- Core Denoiser struct with 5 modes: Fast, Balanced, Quality, GrainAware, Custom
- Dependencies: oximedia-core, oximedia-codec, oximedia-graph, rayon, ndarray

## Enhancements
- [x] Replace `expect("Invalid configuration")` in `Denoiser::new()` with proper error return (no unwrap policy)
- [x] Add configurable noise model selection to `DenoiseConfig` (currently hardcoded in estimator)
- [x] Implement actual algorithm selection in `process_custom()` mode (currently just delegates to `process_balanced`)
- [x] Add per-channel denoising strength in `chroma_denoise` (luma vs chroma independent control)
- [x] Improve `motion/estimation.rs` with hierarchical block matching for better motion vectors (verified 2026-05-16; src/motion/estimation.rs:212 hierarchical_motion_estimation, multi-level pyramid:226)
- [x] Add sub-pixel motion estimation accuracy in `motion/compensation.rs` (verified 2026-05-16; src/motion/compensation.rs:153 SubPixelMv struct)
- [x] Enhance `grain/analysis.rs` with frequency-domain grain characterization (verified 2026-05-16; src/grain/analysis.rs:44 FrequencyGrainProfile, characterize_grain_frequency:126)
- [x] Make `frame_buffer` use a `VecDeque` instead of `Vec` to avoid O(n) removal at index 0
- [x] Add streaming/pipeline mode to `Denoiser` for integration with oximedia-graph processing chains (verified 2026-05-16; src/streaming.rs:1 streaming pipeline-mode denoiser, 426 lines)
- [x] Implement adaptive temporal window sizing based on motion level in `hybrid/adaptive.rs` (verified 2026-05-16; src/hybrid/spatiotemporal.rs:119 adaptive_spatio_temporal fn)

## New Features
- [x] Add BM3D (Block-Matching 3D) denoising algorithm as a new spatial method
- [ ] Implement video stabilization-aware denoising (joint stabilization + denoise pass) (verified-open 2026-05-16: no joint stab+denoise module found)
- [ ] Add GPU-accelerated denoising path via oximedia-gpu for real-time 4K processing (verified-open 2026-05-16: no GPU path in denoise sources)
- [ ] Implement perceptual noise shaping that keeps denoising artifacts below JND threshold (verified-open 2026-05-16: no JND/perceptual_noise_shaping module found)
- [x] Add HDR-aware denoising with PQ/HLG transfer function awareness (verified 2026-05-16; src/hdr_denoise.rs:868 lines)
- [ ] Implement deep image prior-style iterative denoising without neural networks (verified-open 2026-05-16: no deep_prior/iterative denoising module found)
- [ ] Add real-time noise level monitoring and auto-strength adjustment (verified-open 2026-05-16: no noise_monitor/real-time auto-strength module found)

## Performance
- [x] Add SIMD-accelerated bilateral filter kernel in `spatial/bilateral.rs`
- [ ] Implement tiled processing in NLM to improve cache locality in `spatial/nlmeans.rs`
- [ ] Use rayon parallel iterators for row-parallel wavelet transform in `multiscale/wavelet.rs`
- [ ] Add look-up table acceleration for Gaussian weight computation in bilateral filter
- [ ] Profile and optimize `noise_estimate.rs` variance computation for large frames

## Testing
- [ ] Add PSNR/SSIM regression tests comparing denoised output against known-good references
- [ ] Add temporal consistency tests verifying no flickering across frame sequences
- [ ] Test edge preservation metrics for bilateral and NLM filters
- [ ] Add stress tests with various pixel formats beyond Yuv420p (Yuv422p, Yuv444p, RGB)
- [ ] Test `spectral_gate.rs` with known audio noise patterns

## Documentation
- [ ] Document the noise model mathematical formulations in `noise_model.rs`
- [ ] Add architecture diagram showing the relationship between spatial, temporal, and hybrid modules
- [ ] Document the grain preservation pipeline in `grain/` submodules
