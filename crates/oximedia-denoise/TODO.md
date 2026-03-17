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
- [ ] Improve `motion/estimation.rs` with hierarchical block matching for better motion vectors
- [ ] Add sub-pixel motion estimation accuracy in `motion/compensation.rs`
- [ ] Enhance `grain/analysis.rs` with frequency-domain grain characterization
- [x] Make `frame_buffer` use a `VecDeque` instead of `Vec` to avoid O(n) removal at index 0
- [ ] Add streaming/pipeline mode to `Denoiser` for integration with oximedia-graph processing chains
- [ ] Implement adaptive temporal window sizing based on motion level in `hybrid/adaptive.rs`

## New Features
- [x] Add BM3D (Block-Matching 3D) denoising algorithm as a new spatial method
- [ ] Implement video stabilization-aware denoising (joint stabilization + denoise pass)
- [ ] Add GPU-accelerated denoising path via oximedia-gpu for real-time 4K processing
- [ ] Implement perceptual noise shaping that keeps denoising artifacts below JND threshold
- [ ] Add HDR-aware denoising with PQ/HLG transfer function awareness
- [ ] Implement deep image prior-style iterative denoising without neural networks
- [ ] Add real-time noise level monitoring and auto-strength adjustment

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
