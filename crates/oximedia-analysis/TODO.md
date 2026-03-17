# oximedia-analysis TODO

## Current Status
- 43 source files across modules: `scene`, `black`, `quality`, `content`, `thumbnail`, `motion`, `color`, `audio`, `temporal`, `report`, `utils`
- Advanced modules: `action_detect`, `brand_detection`, `logo_detect`, `facial_analysis`, `crowd_analysis`, `text_detection`, `object_tracking`, `flicker_detect`, `frame_cadence`, `saliency_map`, `visual_attention`, `speech`, `audio_spectrum`, `audio_video_sync`, `noise_profile`, `edge_density`, `histogram_analysis`, `color_analysis`, `content_complexity`, `content_rating`, `motion_analysis`, `scene_stats`, `shot_list`, `temporal_analysis`, `temporal_stats`, `quality_score`
- Single-pass modular `Analyzer` with config builder pattern
- JSON and HTML report generation
- Dependencies: oximedia-core, oximedia-audio, oximedia-metering, oxifft, rayon, serde (ndarray removed)

## Enhancements
- [x] Add adaptive scene detection threshold in `scene` based on content type (high-motion vs static)
- [ ] Implement dissolve and wipe detection in `scene` (currently focused on hard cuts)
- [x] Add per-scene quality scoring in `quality` (aggregate blockiness/blur/noise per scene segment)
- [ ] Implement VMAF-like perceptual quality metric in `quality_score` module
- [ ] Add face blur/obscurity detection in `facial_analysis` for privacy compliance
- [x] Implement letterbox/pillarbox detection in `black` module (partial black frame patterns)
- [x] Add interlacing detection in `frame_cadence` (detect combing artifacts)
- [x] Implement cadence pattern recognition in `frame_cadence` (3:2 pulldown, 2:2 pulldown)
- [ ] Add configurable saliency model weights in `saliency_map` for different content types
- [ ] Implement multi-pass analysis mode for higher accuracy (second pass with refined parameters)

## New Features
- [x] Add BRISQUE blind image quality assessment in `quality` module
- [ ] Implement commercial/advertisement break detection using `brand_detection` + `audio` cues
- [ ] Add subtitle/burned-in text region detection in `text_detection` with OCR readiness score
- [x] Implement camera motion classification in `motion_analysis` (pan, tilt, zoom, static, handheld)
- [ ] Add duplicate frame detection (frozen video) as separate detector
- [ ] Implement bandwidth estimation module for adaptive streaming bitrate recommendations
- [ ] Add color gamut analysis: detect out-of-gamut pixels for SDR/HDR content
- [ ] Implement shot composition analysis (rule of thirds, symmetry, leading lines)
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
