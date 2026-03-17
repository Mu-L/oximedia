# oximedia-gpu TODO

## Current Status
- 60+ source files across core GPU ops, compute pipeline, memory management, kernels, and synchronization
- WGPU-based cross-platform GPU acceleration (Vulkan, Metal, DX12, WebGPU)
- Operations: RGB/YUV color conversion (BT.601/709/2020), bilinear/bicubic scaling, Gaussian blur, sharpen, edge detect, DCT/IDCT
- Kernel modules: color conversion, convolution, filter, resize, reduce, transform
- Memory: allocator, pool, managed buffers, buffer copy, upload queue
- Pipeline: compute pipeline manager, dispatch, shader compiler, pipeline cache
- Sync: fences, semaphores, barriers, events, fence pool, GPU timer
- Profiling: gpu_profiler, gpu_stats, occupancy, workgroup sizing
- Video: histogram, motion detection, video frame processor, tone mapping, denoising
- CPU fallback backend available
- Dependencies: wgpu, bytemuck, rayon, pollster, parking_lot

## Enhancements
- [ ] Replace `expect("Failed to create GPU context")` in `Default for GpuContext` with a non-panicking alternative
- [ ] Add BT.2020 and BT.2100 color space support to `ops/colorspace.rs` for HDR content
- [x] Implement Lanczos scaling filter in `ops/scale.rs` alongside bilinear and bicubic
- [ ] Enhance `ops/denoise.rs` with GPU-accelerated bilateral filter and NLM denoising
- [x] Add configurable workgroup size auto-tuning in `workgroup.rs` based on device limits
- [ ] Improve `shader_cache.rs` with disk-persistent shader cache (currently memory-only)
- [x] Enhance `memory_pool.rs` with defragmentation and compaction for long-running sessions
- [ ] Add proper error propagation in `ops/` functions instead of internal panics
- [ ] Implement `compute_dispatch.rs` indirect dispatch for data-dependent workload sizes
- [ ] Add async compute queue support in `queue.rs` for overlapping compute and transfer

## New Features
- [ ] Implement GPU-accelerated AV1/VP9 motion estimation using compute shaders
- [x] Add GPU-based SSIM/PSNR quality metric computation in `compute_kernels.rs`
- [ ] Implement GPU histogram equalization and tone curve application
- [ ] Add GPU-accelerated optical flow computation for motion interpolation
- [x] Implement GPU-based chroma subsampling (4:2:0, 4:2:2 conversion)
- [ ] Add multi-GPU load balancing with automatic frame distribution
- [ ] Implement GPU-based film grain synthesis matching `oximedia-denoise` grain model
- [ ] Add GPU-accelerated image compositing with alpha blending and blend modes
- [ ] Implement GPU-based perspective transform and lens distortion correction

## Performance
- [ ] Profile and optimize `ops/filter.rs` Gaussian blur with separable two-pass implementation
- [ ] Add shared memory (workgroup local) optimization to convolution kernels
- [ ] Implement double-buffered GPU command submission to overlap CPU/GPU work
- [ ] Optimize `texture.rs` TexturePool with LRU eviction for bounded memory usage
- [ ] Add pipeline barrier optimization in `pipeline.rs` to minimize GPU stalls
- [ ] Implement sub-allocation within large buffers in `buffer_pool.rs` to reduce bind overhead
- [ ] Profile `compute_pass.rs` dispatch overhead and batch small dispatches together

## Testing
- [ ] Add GPU vs CPU output comparison tests for all operations (verify correctness within tolerance)
- [ ] Test `ops/colorspace.rs` with known color conversion test vectors (BT.601, BT.709)
- [ ] Test `ops/scale.rs` with known downscale/upscale reference images
- [ ] Add memory leak tests for `memory_pool.rs` and `buffer_pool.rs` (allocate/free cycles)
- [ ] Test multi-GPU device selection and fallback behavior
- [ ] Add performance regression benchmarks for core operations (color convert, scale, blur)
- [ ] Test `shader_cache.rs` invalidation when shader source changes

## Documentation
- [ ] Document supported GPU backends and their feature differences
- [ ] Add performance benchmarks table comparing GPU vs CPU for each operation
- [ ] Document the shader compilation and caching pipeline
