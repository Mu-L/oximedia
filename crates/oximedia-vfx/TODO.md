# oximedia-vfx TODO

## Current Status
- 103 source files across 17 top-level modules and numerous submodules
- Key features: VideoEffect/TransitionEffect traits, keyframe animation system, Frame RGBA processing, Color/Rect/Vec2 primitives, EasingFunction interpolation, ParameterTrack system
- Effect categories: transitions (dissolve, wipe, push, slide, zoom, 3D), generators (bars, gradient, noise, pattern, solid), keying (advanced, spill, edge), time effects (remap, speed, freeze, reverse), distortion (barrel, lens, wave, ripple), style (cartoon, halftone, mosaic, paint, sketch), light (bloom, flare, glow, rays, anamorphic), particles (snow, rain, sparks, dust), compositing (blend modes, alpha, matte, layers), tracking (point, planar, stabilize, mask), rotoscoping (bezier, assisted, keyframe), text (render, animate), shape (draw, animate, mask)
- Additional modules: blur_kernel, chroma_key, chromatic_aberration, color_grade, color_grading (curves/matching/wheels), color_lut, deform_mesh, depth_of_field, film_effect, fog, heat_distort, lens_aberration, lens_flare, motion_blur, vector_blur, noise_field, particle_fx, particle_sim, render_pass, ripple, trail_effect, vfx_preset

## Enhancements
- [ ] Add GPU compute shader backend to `VideoEffect::apply()` for effects that set `supports_gpu() = true` (verified-open 2026-05-16: no GPU/wgpu compute backend wired to VideoEffect::apply in vfx sources)
- [ ] Implement multi-channel ParameterTrack (Vec2/Vec3/Color tracks) instead of single f32 value only (verified-open 2026-05-16: no Vec2Track/Vec3Track/ColorTrack in keyframe/ submodules; only f32 ParameterTrack)
- [x] Add spring/bounce/elastic easing functions to `EasingFunction` enum beyond current 5 types
- [x] Extend `compositing::blend_modes` with all Photoshop-compatible blend modes (vivid light, pin light, hard mix)
- [x] Add feathered edge support to `keying::edge` refinement with configurable falloff curve
- [ ] Implement motion-vector-based `motion_blur` that reads actual frame motion rather than uniform blur (verified-open 2026-05-16: motion_blur.rs:50 MotionBlurKernel uses linear/radial kernels; no actual motion vector input)
- [x] Add color management (OCIO-style) to `color_grading` pipeline for proper working-space transforms (verified 2026-05-16; src/color_grading/color_space.rs:1 OCIO-style color space transforms)
- [x] Extend `rotoscoping::bezier` with cubic B-spline and Catmull-Rom curve types for smoother masks (verified 2026-05-16; src/rotoscoping/spline_curves.rs:99 CatmullRomCurve, CubicBSplineCurve:32)
- [ ] Add sub-pixel precision to `tracking::point` tracker using bilinear interpolation on correlation (verified-open 2026-05-16: no sub-pixel/bilinear correlation in tracking/point.rs)

## New Features
- [x] Implement `MorphTransition` effect (mesh-based morph between two frames for organic transitions) (verified 2026-05-16; src/transition/morph.rs 472 lines)
- [x] Add `Glitch` effect module (digital glitch, datamosh, RGB shift, scan line artifacts)
- [x] Implement `Tilt-Shift` miniature effect using configurable gradient mask with depth_of_field (verified 2026-05-16; src/tilt_shift.rs 588 lines)
- [x] Add `Vignette` effect with customizable shape (circular, elliptical, rectangular) and falloff
- [x] Implement `ChromaticAberration` as proper RGB channel offset with lens distortion model (verified 2026-05-16; src/chromatic_aberration.rs:56 ChromaticAberration, Brown-Conrady radial distortion model:14, 785 lines)
- [x] Add `RetroFilm` generator combining film_effect grain, color_grade desaturation, and vignette (verified 2026-05-16; src/retro_film.rs 665 lines)
- [x] Implement `Parallax` effect for 2.5D camera motion on layered images using depth maps (verified 2026-05-16; src/parallax.rs 427 lines)
- [x] Add `AudioReactive` effect modifier that drives effect parameters from audio amplitude/frequency data (verified 2026-05-16; src/audio_reactive.rs 567 lines)

## Performance
- [ ] Use SIMD (std::simd when stable, or manual intrinsics) for pixel-level operations in `Frame::clear()`, `Color::blend()`, and `Color::lerp()`
- [ ] Implement tile-based parallel processing in `VideoEffect::apply()` using rayon for large frames
- [ ] Add frame caching/memoization in `ParameterTrack::evaluate()` for repeated same-time queries
- [ ] Optimize `particle::system` with spatial partitioning (grid or quadtree) for particle-particle interactions
- [ ] Implement downsampled preview path in `QualityMode::Draft` that processes at 1/4 resolution then upscales
- [ ] Pool Frame allocations to avoid repeated large heap allocations in effect chains

## Testing
- [ ] Add visual regression tests comparing effect output against golden reference frames (pixel diff threshold)
- [ ] Test all TransitionEffect implementations at progress 0.0, 0.5, and 1.0 for correct blending behavior
- [ ] Add boundary tests for Frame with maximum resolution (8K) and minimum (1x1)
- [ ] Test ParameterTrack interpolation with overlapping keyframes, NaN times, and single-keyframe tracks
- [ ] Add performance benchmarks for each effect category to track regression (using criterion)
- [ ] Test compositing::layer_manager with deep layer stacks (50+ layers) for correctness

## Documentation
- [ ] Add visual examples (ASCII art or image references) for each transition type in `transition` module docs
- [ ] Document particle system parameter tuning guide in `particle` module
- [ ] Add effect chain cookbook showing common post-production workflows (color grade -> blur -> composite)
