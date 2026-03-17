# oximedia-simd TODO

## Current Status
- 35 source files with scalar fallbacks, conditional x86 (AVX2/AVX-512) and aarch64 (NEON) assembly backends
- 29 public modules: accumulator, alpha_premul, audio_ops, avx512, bitwise_ops, blend, blend_simd, color_convert_simd, color_space, convolution, dispatch, filter, fixed_point, gather_scatter, histogram, interleave, lookup_table, math_ops, matrix, min_max, pack_unpack, pixel_ops, prefix_sum, reduce, saturate, threshold, transpose, vector_math, yuv_ops
- Core APIs: forward_dct/inverse_dct, interpolate, sad, CPU feature detection with OnceLock caching
- Features: native-asm (cc build dep), runtime-dispatch, force-avx2, avx512, force-avx512, force-neon

## Enhancements
- [ ] Add SATD (Sum of Absolute Transformed Differences) kernel alongside existing SAD for better motion estimation quality
- [ ] Extend `DctSize` enum to include Dct64x64 for AV1 large block transforms
- [ ] Add Hadamard transform support in `scalar` and platform-specific backends
- [ ] Implement `InterpolationFilter::Lanczos` variant for higher-quality resampling
- [ ] Add `BlockSize::Block8x8` and `Block4x4` to SAD operations for sub-block motion search
- [ ] Extend `color_convert_simd` to support BT.2020 and BT.2100 HDR color matrices
- [ ] Add runtime benchmark function that measures throughput for each detected SIMD tier

## New Features
- [ ] Implement SSIM (Structural Similarity Index) kernel with SIMD acceleration
- [ ] Add PSNR computation kernel operating on 16x16 and 32x32 blocks
- [ ] Implement variance/standard deviation computation in `reduce` module for adaptive quantization support
- [ ] Add `deblock_filter` module with SIMD-accelerated deblocking for codec post-processing
- [ ] Implement `motion_search` module with diamond/hexagonal search patterns using SAD kernels
- [ ] Add `resize` module with SIMD-accelerated bilinear and Lanczos image scaling
- [ ] Implement `entropy_coding` SIMD helpers for bit packing/unpacking in codec bitstreams
- [ ] Add Apple Silicon AMX (Apple Matrix Extension) detection in `CpuFeatures` for future acceleration paths

## Performance
- [ ] Implement AVX-512 VNNI (Vector Neural Network Instructions) path for 8-bit SAD accumulation
- [ ] Add prefetch hints in `interpolate` for sequential block access patterns
- [ ] Implement 4-way parallel SAD search in AVX-512 path (process 4 candidate blocks simultaneously)
- [ ] Use `_mm256_maddubs_epi16` intrinsic in AVX2 path for faster u8 multiplication in interpolation
- [ ] Add aligned allocation helper that guarantees 64-byte alignment for AVX-512 buffers
- [ ] Profile and optimize `scalar::forward_dct_scalar` with butterfly decomposition for Dct32x32

## Testing
- [ ] Add correctness tests comparing SIMD outputs against scalar reference for all DCT sizes
- [ ] Add cross-platform CI tests ensuring scalar fallback produces identical results to SIMD paths
- [ ] Test `CpuFeatures::detect` on actual x86_64 and aarch64 targets (not just compilation)
- [ ] Add fuzzing targets for `interpolate` and `sad` with random buffer sizes near boundary conditions
- [ ] Add criterion benchmarks for each kernel at each SIMD tier (scalar, SSE4.2, AVX2, AVX-512, NEON)

## Documentation
- [ ] Document SIMD tier selection logic and how `runtime-dispatch` feature interacts with `force-*` features
- [ ] Add performance comparison table (scalar vs. AVX2 vs. AVX-512 vs. NEON) for core kernels
- [ ] Document alignment requirements for each SIMD tier in module-level docs
