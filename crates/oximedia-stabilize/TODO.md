# oximedia-stabilize TODO

## Current Status
- 54 source files across 29 public modules organized in submodule trees: motion (tracker, estimate, model, trajectory), smooth (filter, adaptive, temporal), transform (calculate, interpolate, optimize), warp (apply, boundary, interpolation), rolling (correct, detect), three_d (stabilize, estimate), horizon (level, detect), zoom (calculate, dynamic), blur (motion), multipass (analyze, optimize)
- Full stabilization pipeline: feature tracking -> motion estimation -> trajectory smoothing -> transform calculation -> frame warping
- Modes: Translation, Affine, Perspective, 3D; Quality presets: Fast, Balanced, Maximum
- Advanced features: rolling shutter correction, horizon leveling, zoom optimization, motion blur synthesis, gyro integration, mesh warp, parallax compensation, vibration isolation, jitter detection
- Dependencies: oximedia-core, oximedia-cv, oximedia-graph, scirs2-core (ndarray), rayon, serde

## Enhancements
- [x] Add optical flow-based motion estimation in `optical_flow` module — Horn-Schunck dense flow with coarse-to-fine pyramid, affine/translation fit from flow field
- [x] Implement L1 optimal camera path in `l1_optimal_path` module — IRLS approximation minimizing acceleration with crop constraints
- [x] Extend rolling shutter correction with per-row motion model in `per_row_rolling_shutter` — linear velocity interpolation per scanline with bilinear warp
- [x] Add content-aware saliency-based cropping in `saliency_crop` module — center-surround saliency detection, salient region protection, temporal smoothing
- [x] Improve gyro integration with EKF in `gyro_ekf` module — 6-state Extended Kalman Filter fusing gyroscope rates with visual motion estimates
- [ ] Add `stabilize_report` generation with per-frame motion magnitude, crop area, and quality metrics
- [ ] Extend `path_planner` to support user-defined motion constraints (e.g., lock pan but allow tilt stabilization)

## New Features
- [ ] Implement real-time single-pass stabilization mode with bounded look-ahead buffer for live streaming use cases
- [ ] Add `lens_distortion` module for pre-correcting barrel/pincushion distortion before stabilization
- [ ] Implement `tripod_mode` that detects static camera and applies stronger smoothing with minimal crop
- [ ] Add `roi_stabilization` module for stabilizing a specific region of interest independently from global motion
- [ ] Implement `stabilize_preview` that generates low-resolution stabilized preview before full-resolution processing
- [ ] Add `motion_mask` module to exclude moving foreground objects from motion estimation (e.g., actors, vehicles)
- [ ] Implement `stitching_stabilize` module for stabilizing 360-degree video with wrap-around boundary handling
- [ ] Add export of stabilization data as motion vectors for use in compositing applications (Nuke, Fusion)

## Performance
- [ ] Parallelize feature tracking across frame pairs using rayon in `motion::tracker::track`
- [ ] Implement GPU-accelerated frame warping in `warp::apply` using compute shaders (feature-gated)
- [ ] Use tile-based processing in `mesh_warp` to improve cache locality for large frames
- [ ] Add SIMD-accelerated bilinear interpolation in `warp::interpolation` for the hot path
- [ ] Cache feature descriptors between consecutive frames in `motion::tracker` to reduce redundant computation
- [ ] Implement progressive processing in `multipass::analyze` that refines estimates incrementally

## Testing
- [ ] Add synthetic test sequences with known translation/rotation to verify stabilization accuracy within 0.5px
- [ ] Test rolling shutter correction with synthetic rolling-shutter-distorted frames and verify line straightening
- [ ] Add test for `horizon::level` with frames containing tilted horizon lines at known angles
- [ ] Test all StabilizationMode variants with high-motion synthetic sequences (>50px displacement between frames)
- [ ] Add regression test ensuring `zoom::calculate` never produces transforms with negative crop area
- [ ] Test `vibration_isolate` with high-frequency sinusoidal motion patterns at known frequencies

## Documentation
- [ ] Add pipeline architecture diagram showing data flow through all 11 stabilization steps
- [ ] Document recommended QualityPreset selection guidelines based on content type and resolution
- [ ] Add tuning guide for `smoothing_strength` and `crop_ratio` parameters with visual examples
