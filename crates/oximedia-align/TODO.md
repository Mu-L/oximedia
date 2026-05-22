# oximedia-align TODO

## Current Status
- 36 source files across modules: `temporal`, `spatial`, `features`, `distortion`, `color`, `markers`, `rolling_shutter`, `optical_flow`, `phase_correlate`, `affine`, `transform`, `warp`, `icp`, `lip_sync`, `drift_correct`, `drift_correction`, `gradient_flow`, `sync_score`, `stereo_rectify`, `beat_align`, `elastic_align`, `frame_matcher`, `frequency_align`, `multicam_sync`, `multitrack_align`, `motion_compensate`, `multi_stream`, `audio_align`, `subframe_interp`, `tempo_align`, `temporal_align`, `align_report`
- Audio cross-correlation sync, timecode sync, visual marker detection
- Homography estimation with RANSAC, perspective correction, feature matching (FAST, BRIEF, ORB)
- Brown-Conrady and fisheye lens distortion correction
- Color transfer, histogram matching, white balance
- Rolling shutter correction
- Dependencies: oximedia-core, oximedia-audio, nalgebra, rayon, serde

## Enhancements
- [x] Add KLT (Kanade-Lucas-Tomasi) tracker to `optical_flow` for sparse flow tracking
  — `KltTracker::track_features(prev, curr, width, height, &[(f32,f32)]) -> Vec<Option<(f32,f32)>>` added to `klt_tracker`
- [x] Implement multi-scale feature matching in `features` (pyramid ORB for scale invariance) (verified 2026-05-16; src/features.rs:1014 FeaturePyramid, build_pyramid:1043, detect_features_at_all_levels:1109, num_levels:1516, 1757 lines)
- [x] Add PROSAC as alternative to RANSAC in `spatial` for faster convergence with sorted matches
  — `ProsacEstimator` in `prosac` module (Affine + Homography models)
- [x] Implement weighted least squares in `HomographyEstimator` for refined estimates after RANSAC
- [x] Add sub-pixel refinement for FAST corner detection in `features`
- [x] Implement exposure-invariant feature descriptors in `features` for mixed-lighting scenarios (verified 2026-05-16; src/illumination_invariant.rs:25 IlluminationInvariantConfig, IlluminationInvariantDescriptor:48, 334 lines)
- [x] Add temporal smoothing to `rolling_shutter` correction to prevent frame-to-frame jitter
- [x] Implement spectral domain audio alignment in `audio_align` using phase correlation
- [x] Add genlock drift estimation in `drift_correct` for long-duration multi-camera recordings
- [x] Implement bundle adjustment in `multicam_sync` for globally consistent multi-camera calibration (verified 2026-05-16; src/bundle_adjust.rs:155 BundleAdjuster, BundleAdjustConfig:32, adjust:192, BundleAdjustResult:141, 799 lines)

## New Features
- [x] Add dense optical flow (Farneback method) to `optical_flow` module
  — `compute_dense_flow(prev: &[f32], curr: &[f32], width: u32, height: u32) -> Vec<(f32,f32)>`
  — Full Farneback implementation in `farneback_flow`; convenience bridge in `optical_flow`
- [x] Implement image stitching pipeline: detect features -> match -> estimate homography -> blend (verified 2026-05-16; src/image_stitch.rs:125 ImageStitcher, StitchConfig:36, chain_homographies:564, bilinear_sample:550, 595 lines)
- [x] Add automatic lens calibration from checkerboard/ArUco marker detection (verified 2026-05-16; src/lens_calibration.rs:37 CheckerboardDetector, ArUco-style:8, CameraCalibrator:349, 955 lines)
- [x] Implement depth-from-stereo using `stereo_rectify` + block matching (implemented 2026-05-15: src/stereo_depth.rs — StereoDepthEstimator::compute_depth_map using SAD block matching + depth=f*B/d formula; tests: test_depth_map_uniform_disparity, test_depth_map_zero_disparity_infinite_depth, test_depth_map_known_shift; 650/650 tests pass)
- [x] Add motion-compensated temporal interpolation for frame rate conversion (verified 2026-05-16; src/motion_interp.rs:36 MotionInterpolator, 202 lines)
- [x] Implement video stabilization pipeline combining `optical_flow` + `affine` + `warp` (verified 2026-05-16; src/stabilize.rs:286 stabilize_pipeline, StabilizeConfig:36, compute_corrections:202, 799 lines)
- [x] Add network-based time sync (PTP/NTP) for distributed camera systems in `temporal` (implemented 2026-05-15: NtpConfig/TimeDelta/NtpClient::query_offset added to temporal.rs — SNTP RFC 4330 single-packet UDP exchange; tests: test_ntp_packet_parse_known_bytes, test_ntp_offset_computation_formula, test_ntp_unix_roundtrip; 650/650 tests pass)
- [ ] Implement scene-based alignment: detect scene changes and align segments independently (verified-open 2026-05-16: no SceneAlign/scene_align/AlignSegment in align sources; scene change detection not integrated into alignment pipeline)

## Performance
- [x] Add SIMD-accelerated Hamming distance for BRIEF descriptor matching in `features`
  — `hamming_distance_simd(a: &[u8], b: &[u8]) -> u32` using u64 popcount batching
- [ ] Implement parallel RANSAC with early termination across rayon threads
- [ ] Use integral images for fast feature detection in `features` (box filter acceleration)
- [ ] Optimize `phase_correlate` FFT computation with real-valued FFT (half the complex ops)
- [ ] Add GPU acceleration path for dense optical flow in `optical_flow`
- [ ] Cache feature descriptors across frames for temporal feature tracking

## Testing
- [x] Add synthetic test: generate known homography, project points, recover and compare
  — `test_homography_roundtrip` in `spatial`: forward-inverse roundtrip + RANSAC recovery
  — Also fixed latent bug: `estimate_from_4_points` now uses A^T*A normal equations
    (9×9 SVD) instead of the broken thin-SVD path that panicked for 8×9 A matrices
- [ ] Test `audio_align` with known offsets using time-shifted synthetic audio
- [ ] Add accuracy tests for `distortion` using synthetic radially distorted checkerboard images
- [ ] Test `rolling_shutter` correction with synthetic rolling shutter simulation
- [ ] Benchmark `features` (FAST+ORB) against varying image sizes (VGA to 4K)
- [ ] Test `color` transfer with known color chart images across different illuminants

## Documentation
- [ ] Add coordinate system convention diagram (image vs camera vs world)
- [ ] Document multi-camera workflow: calibrate -> sync -> align -> composite
- [ ] Add accuracy/performance comparison table for different feature detectors
