# oximedia-360 TODO

## Current Status
- 4 modules: `projection`, `stereo`, `fisheye`, `spatial_metadata`
- 5 source files total
- Equirectangular/cubemap projections, stereo 3D frame splitting/merging, fisheye lens models, Google Spatial Media v2 metadata
- Minimal dependencies (only `thiserror`)

## Enhancements
- [ ] Add bicubic and Lanczos sampling to `projection` alongside existing `bilinear_sample_u8`
- [ ] Support 16-bit and f32 pixel formats in `bilinear_sample_u8` (rename or add typed variants)
- [ ] Add configurable blend width parameter to `DualFisheyeStitcher` for fine-tuning seam quality
- [ ] Implement exposure compensation in `DualFisheyeStitcher` for brightness matching between fisheye images
- [ ] Add multi-resolution blending (Laplacian pyramid) to `DualFisheyeStitcher` for seamless stitching
- [ ] Support rectilinear sub-region extraction from equirectangular frames (virtual camera)
- [ ] Add rotation/orientation transforms to `SphericalCoord` (yaw/pitch/roll)
- [ ] Implement `sphere_to_equirect` round-trip accuracy validation utilities
- [ ] Add RGBA support to stereo frame splitting/merging in `stereo` module

## New Features
- [ ] Add `EAC` (Equi-Angular Cubemap) projection used by YouTube/Google for more uniform sampling
- [ ] Implement octahedral projection mapping for efficient VR video compression
- [ ] Add viewport rendering: extract a perspective view from equirectangular at given FOV/orientation
- [ ] Implement mesh-based projection for custom lens models (lookup table warping)
- [ ] Add Apple Spatial Video metadata support (MV-HEVC stereo pairs) in `spatial_metadata`
- [ ] Implement V3D box parsing for VR180 format metadata
- [ ] Add stabilization for 360 video using gyroscope/IMU metadata integration
- [ ] Implement FOV-dependent quality allocation (higher quality where user looks)

## Performance
- [ ] Add SIMD-accelerated bilinear sampling using packed f32 operations
- [ ] Implement tiled cubemap conversion for better cache locality in `equirect_to_cube`
- [ ] Add parallel scanline processing with rayon for projection conversions
- [ ] Pre-compute lookup tables for fisheye-to-equirect mapping to amortize trig cost
- [ ] Use `f32` fast-math approximations for `sin`/`cos`/`atan2` in hot projection loops

## Testing
- [ ] Add round-trip tests: equirect -> cubemap -> equirect with PSNR threshold
- [ ] Add property-based tests for `sphere_to_equirect` / `equirect_to_sphere` inverse relationship
- [ ] Test `DualFisheyeStitcher` with synthetic checkerboard fisheye images
- [ ] Add edge-case tests for polar singularities in equirectangular projection
- [ ] Benchmark cubemap face extraction at 4K and 8K resolutions

## Documentation
- [ ] Add visual diagrams showing coordinate system conventions (theta/phi orientation)
- [ ] Document supported stereo layouts (SBS, TB, frame-sequential) with pixel layout examples
- [ ] Add end-to-end example: load equirectangular image, extract cubemap faces, write output
