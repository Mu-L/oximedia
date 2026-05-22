# oximedia-analysis TODO

## Current Status
- 43 source files across modules: `scene`, `black`, `quality`, `content`, `thumbnail`, `motion`, `color`, `audio`, `temporal`, `report`, `utils`
- Advanced modules: `action_detect`, `brand_detection`, `logo_detect`, `facial_analysis`, `crowd_analysis`, `text_detection`, `object_tracking`, `flicker_detect`, `frame_cadence`, `saliency_map`, `visual_attention`, `speech`, `audio_spectrum`, `audio_video_sync`, `noise_profile`, `edge_density`, `histogram_analysis`, `color_analysis`, `content_complexity`, `content_rating`, `motion_analysis`, `scene_stats`, `shot_list`, `temporal_analysis`, `temporal_stats`, `quality_score`
- Single-pass modular `Analyzer` with config builder pattern
- JSON and HTML report generation
- Dependencies: oximedia-core, oximedia-audio, oximedia-metering, oxifft, rayon, serde (ndarray removed)

## Enhancements
- [x] Add adaptive scene detection threshold in `scene` based on content type (high-motion vs static)
- [x] Implement dissolve and wipe detection in `scene` (currently focused on hard cuts) (verified 2026-05-16; src/scene.rs:66 dissolve_window, wipe_gradients:68, compute_wipe_score:295, detect_scene_change:183 dissolve/wipe branches)
- [x] Add per-scene quality scoring in `quality` (aggregate blockiness/blur/noise per scene segment)
- [x] Implement VMAF-like perceptual quality metric in `quality_score` module (verified 2026-05-16; src/vmaf_estimator.rs:18 VmafEstimator, estimate combining PSNR+SSIM:8, 222 lines)
- [ ] Add face blur/obscurity detection in `facial_analysis` for privacy compliance (verified-open 2026-05-16: facial_analysis.rs has FacePrivacyAssessment:489, HaarCascadeDetector:248, GazeEstimator:410 but no blur/obscurity detection fn)
- [x] Implement letterbox/pillarbox detection in `black` module (partial black frame patterns)
- [x] Add interlacing detection in `frame_cadence` (detect combing artifacts)
- [x] Implement cadence pattern recognition in `frame_cadence` (3:2 pulldown, 2:2 pulldown)
- [x] Add configurable saliency model weights in `saliency_map` for different content types (verified 2026-05-16; src/saliency_map.rs:225 SaliencyModelWeights, ContentTypeHint:195 General/Face/Sports/Documentary/ScreenCapture/Nature, weights_for:309, 574 lines)
- [x] Implement multi-pass analysis mode for higher accuracy (second pass with refined parameters) (verified 2026-05-16; src/multi_pass.rs:98 MultiPassAnalyzer, analyze_first_pass:255, analyze_second_pass:255, 406 lines)

## New Features
- [x] Add BRISQUE blind image quality assessment in `quality` module
- [x] Implement commercial/advertisement break detection using `brand_detection` + `audio` cues (verified 2026-05-16; src/commercial_detect.rs:70 CommercialDetector, detect:102, CommercialBreak:35, 249 lines)
- [x] Add subtitle/burned-in text region detection in `text_detection` with OCR readiness score (verified 2026-05-16; src/text_detection.rs:11 TextRegion, OCR confidence score:20, TextFrame:68, TextTimeline:123, 302 lines)
- [x] Implement camera motion classification in `motion_analysis` (pan, tilt, zoom, static, handheld)
- [x] Add duplicate frame detection (frozen video) as separate detector (verified 2026-05-16; src/frozen_frame.rs:73 FrozenFrameDetector, FrozenRange:49, detect:103, 260 lines)
- [x] Implement bandwidth estimation module for adaptive streaming bitrate recommendations (verified 2026-05-16; src/bitrate_recommender.rs:41 BitrateRecommender, recommend:77, BitrateRecommendation:11, 186 lines)
- [x] Add color gamut analysis: detect out-of-gamut pixels for SDR/HDR content (verified 2026-05-16; src/gamut_analyzer.rs:1 GamutAnalyzer, GamutDescriptor Rec709/DCI-P3/Rec2020:6, out_of_gamut_ratio:69, 275 lines)
- [x] Implement shot composition analysis (rule of thirds, symmetry, leading lines) (verified 2026-05-16; src/shot_composition.rs:128 rule_of_thirds, compute_rule_of_thirds:238, RuleOfThirdsResult, 704 lines)
- [x] Add audio-video sync drift detection over time in `audio_video_sync`

## Performance
- [x] Replace ndarray with pure-Rust matrix operations to satisfy COOLJAPAN Pure Rust Policy
- [ ] Parallelize `Analyzer::process_video_frame` sub-analyzers with rayon (currently sequential)
- [ ] Add downscaling option: analyze at half/quarter resolution for faster processing
- [ ] Implement ring buffer for temporal analysis to bound memory usage for long videos
- [ ] Cache histogram computations across analyzers that share the same frame data
- [ ] Use integer histograms instead of float arrays in `color` and `scene` for speed

## Testing
- [x] Add test with synthetic gradient image for `scene` detector (known scene boundary)
- [ ] Test `black` detector with various near-black thresholds and letterbox patterns
- [ ] Add `quality` assessor accuracy test with reference VQEG test sequences
- [ ] Test `thumbnail` selector diversity: verify selected frames span different scenes
- [ ] Test `report` HTML output renders valid HTML (basic tag matching validation)
- [x] Add integration test: process 30fps 10-second synthetic video through full analyzer pipeline

## Documentation
- [ ] Document each analyzer's algorithm and computational complexity
- [ ] Add example showing custom `AnalysisConfig` for broadcast QC workflow
- [ ] Document `report` output schema for JSON format (field descriptions and value ranges)
