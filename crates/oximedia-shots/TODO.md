# oximedia-shots TODO

## Current Status
- 38 public modules + lib.rs (~70 source files total) covering shot detection, classification, camera movement, composition analysis, continuity checking, pattern analysis, and export
- Key features: cut/dissolve/fade/wipe detection, 7 shot types (ECU to ELS), camera angle/movement classification, scene detection, coverage analysis, EDL/CSV/JSON export
- Dependencies: oximedia-core, oximedia-cv, oximedia-scene, oximedia-edl, oximedia-timecode

## Enhancements
- [x] Add adaptive threshold tuning in `detect::CutDetector` based on content complexity (e.g., action vs. dialogue scenes)
- [x] Extend `classify::ShotTypeClassifier` to support insert shots and cutaway classification
- [x] Add confidence calibration to `classify::AngleClassifier` using histogram-based features (verified 2026-05-16; src/confidence_calibration.rs:359 ConfidenceCalibrator with histogram similarity)
- [x] Improve `composition::CompositionAnalyzer` with golden ratio and phi grid analysis in addition to rule of thirds
- [x] Add multi-threaded frame processing in `ShotDetector::detect_shots` using rayon parallel iterators (verified 2026-05-16; src/lib.rs:226 rayon par_iter boundary detection, src/detector.rs:46)
- [x] Extend `export::shotlist` to support XML-based shot list formats (FCP XML, Resolve markers) (verified 2026-05-16; src/export/shotlist.rs:137 FCP XML, :213 DaVinci Resolve markers XML)
- [x] Add configurable wipe direction patterns in `detect::WipeDetector` (radial, iris, clock) (already implemented at wipe_patterns.rs:48 WipePatternKind::Radial/Iris/Clock + WipePatternDetector:145, Wave 14 verified)
- [x] Improve `continuity::ContinuityChecker` to detect 180-degree rule violations using optical flow direction (verified 2026-05-16; src/continuity/continuity_errors.rs:168 AxisViolation struct, 180-degree rule check:166)

## New Features
- [x] Add ML-based shot boundary detection module using lightweight convolutional features (no external ML runtime) (verified 2026-05-16; src/ml_boundary.rs:1 MlBoundaryDetector using hand-crafted conv features)
- [x] Implement `shot_similarity` module for finding visually similar shots across a project using perceptual hashing (verified 2026-05-16; src/shot_similarity.rs:91 ShotSimilarity, 849 lines)
- [x] Add `color_continuity` module to detect color temperature/grading inconsistencies between consecutive shots (verified 2026-05-16; src/color_continuity.rs:747 lines)
- [x] Implement `depth_of_field` estimation module to classify shallow vs. deep focus shots (verified 2026-05-16; src/depth_of_field.rs:57 DofEstimate, DofEstimator:113, 691 lines)
- [x] Add `audio_scene_boundary` integration with audio energy envelope for scene boundary refinement (verified 2026-05-16; src/audio_scene_boundary.rs:493 lines)
- [x] Implement real-time shot detection mode in `ShotDetector` with streaming frame input (already implemented at realtime.rs:107 RealtimeShotDetector + push_frame:160, Wave 14 verified)
- [x] Add `shot_annotation` module for attaching free-form metadata and tags to detected shots (verified 2026-05-16; src/shot_annotation.rs:922 lines)

## Performance
- [x] Cache histogram computations in `detect::CutDetector` to avoid recomputing for overlapping frame pairs (already implemented at detect/cut.rs:63 histogram_cache Mutex<HashMap<u64,...>>, Wave 14 verified)
- [x] Use downscaled proxy frames (e.g., 160x90) in `camera::MovementDetector` for faster optical flow estimation (implemented at camera/movement.rs: box_downsample_to_proxy + GrayImageProxy, PROXY_WIDTH=160, Wave 14 Slice G)
- [x] Implement block-based SAD comparison in `detect::DissolveDetector` instead of full-frame pixel difference (implemented at detect/dissolve.rs: detect_dissolve_block_sad, DISSOLVE_BLOCK_SIZE=16, DISSOLVE_SAD_THRESHOLD=2048, Wave 14 Slice G)
- [x] Add early termination in `classify::ShotTypeClassifier` when confidence exceeds 0.95 threshold (implemented at classify/shottype.rs: HIGH_CONFIDENCE_THRESHOLD=0.95, Wave 14 Slice G)
- [x] Pool `FrameBuffer` allocations in `detect_shots` to reduce allocation pressure on long sequences (implemented at frame_buffer.rs: FrameBufferPool + acquire/release, used in lib.rs detect_shots, Wave 14 Slice G)

## Testing
- [ ] Add integration tests for `detect_shots` with synthetic frame sequences containing known cuts and dissolves
- [ ] Test `continuity::ContinuityChecker` with crafted jump-cut and axis-crossing scenarios
- [ ] Add round-trip tests for `export::edl` and `export::shotlist` (export then re-import)
- [ ] Test `scene::SceneDetector` with multi-scene sequences having distinct visual characteristics
- [ ] Add property-based tests for `pattern::RhythmAnalyzer` with varying shot duration distributions

## Documentation
- [ ] Add architecture diagram showing the full detection pipeline (tracking -> estimation -> classification -> export)
- [ ] Document threshold tuning guidelines for different content types (documentary, action, interview)
- [ ] Add inline examples to `ShotDetector::detect_shots` showing how to iterate results
