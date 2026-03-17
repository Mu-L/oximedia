# oximedia-accel TODO

## Current Status
- 37 source files across modules: `vulkan`, `cpu_fallback`, `device`, `buffer`, `cache`, `pool`, `dispatch`, `kernels`, `ops`, `shaders`, `traits`, `task_graph`, `task_scheduler`, `fence_timeline`, `memory_arena`, `memory_bandwidth`, `accel_profile`, `accel_stats`, `pipeline_accel`, `prefetch`, `workgroup`, `device_caps`
- `HardwareAccel` trait with GPU (Vulkan) and CPU backends
- Operations: image scaling, color conversion, motion estimation
- Shader modules for scale/color/motion compute shaders
- Dependencies: vulkano 0.35, oximedia-core, rayon, bytemuck

## Enhancements
- [x] Add `deinterlace` operation to `HardwareAccel` trait (bob, weave, motion-adaptive)
- [x] Implement Lanczos and bicubic resampling in `ops/scale` alongside bilinear
- [x] Add YUV<->RGB color space support to `ops/color` (currently RGB-centric)
- [x] Implement HDR tone mapping operations (PQ to SDR, HLG to SDR) in `ops/color`
- [ ] Add multi-GPU support: distribute work across multiple Vulkan devices in `dispatch`
- [ ] Implement async compute queue submission in `vulkan` for overlapped CPU/GPU work
- [ ] Add pipeline caching (VkPipelineCache) in `pipeline_accel` to speed up shader compilation
- [ ] Implement double/triple buffering strategy in `buffer` for streaming frame processing
- [x] Add GPU memory pressure monitoring in `memory_arena` with automatic eviction
- [x] Implement `workgroup` auto-tuning based on `device_caps` (optimal local size per device)

## New Features
- [ ] Add Metal backend for macOS/iOS acceleration (alongside Vulkan)
- [ ] Implement WebGPU backend for WASM target compatibility
- [ ] Add GPU-accelerated histogram computation for color grading pipelines
- [x] Implement GPU-based 2D convolution for filter kernels (blur, sharpen, edge detect)
- [x] Add GPU-accelerated alpha blending and compositing operations
- [ ] Implement GPU-based image rotation and affine transform
- [ ] Add compute shader for DCT/IDCT to accelerate codec operations
- [ ] Implement GPU-accelerated noise reduction (temporal and spatial NR)
- [x] Add profiling/timing overlay for measuring per-operation GPU times in `accel_stats`

## Performance
- [x] Implement descriptor set pooling in Vulkan backend to reduce allocation overhead
- [ ] Add staging buffer ring for efficient CPU-to-GPU transfers in `buffer`
- [ ] Profile and optimize `cpu_fallback` paths with SIMD intrinsics (SSE4.2/AVX2/NEON)
- [ ] Implement subgroup operations in compute shaders where supported (Vulkan 1.1+)
- [ ] Add shared memory tiling in scale/color shaders for reduced global memory bandwidth
- [ ] Benchmark and tune `prefetch` hints for different GPU architectures (AMD/NVIDIA/Intel)

## Testing
- [ ] Add visual regression tests: GPU vs CPU output comparison within tolerance
- [ ] Test device selection fallback when no Vulkan device is available
- [ ] Add stress test for concurrent `scale_image` + `convert_color` operations
- [ ] Test `memory_arena` allocation/deallocation patterns for leak detection
- [ ] Benchmark `task_graph` scheduling overhead with large dependency chains
- [ ] Test `fence_timeline` synchronization correctness under high contention

## Documentation
- [ ] Document device selection heuristics and how to override in `DeviceSelector`
- [ ] Add architecture diagram showing data flow: CPU buffer -> staging -> GPU -> staging -> CPU
- [ ] Document supported Vulkan extensions and minimum device requirements
