# oximedia-forensics TODO

## Current Status
- 37 source files covering image/video tampering detection across multiple forensic domains
- Analysis types: ELA, compression artifacts, noise patterns, PRNU, copy-move, lighting inconsistency
- Additional: steganalysis, splicing detection, shadow analysis, frequency forensics, blocking artifacts
- Chain of custody, file integrity, hash registry, provenance tracking
- ForensicsAnalyzer orchestrates all tests and produces TamperingReport with confidence scoring
- Dependencies: ndarray, image, rayon, serde, serde_json; optional oximedia-cv

## Enhancements
- [x] Improve `ela.rs` with multi-quality ELA (compare at multiple JPEG quality levels)
- [x] Enhance `noise_analysis.rs` with per-region noise variance mapping for localized tampering
- [x] Add `source_camera.rs` PRNU (Photo Response Non-Uniformity) fingerprint database support
- [x] Improve `geometric.rs` copy-move detection with SIFT/ORB-like keypoint matching
- [x] Enhance `lighting.rs` with 3D light source estimation from shadow directions
- [x] Add weighted confidence in `TamperingReport::calculate_overall_confidence()` (some tests more reliable) — `TestWeight` config + reliability-weighted averaging with detection boost; `test_reliability_weight()` function
- [x] Improve `compression_history.rs` with double JPEG compression detection via DCT coefficient analysis — `detect_double_jpeg(dct_coefficients: &[f64]) -> f64` in `compression.rs` using histogram valley depth at multiples of 8
- [x] Add `metadata_forensics.rs` EXIF thumbnail vs main image comparison for editing detection — `MetadataForensics::analyze_exif_thumbnail()` and `MetadataReport`
- [ ] Enhance `splicing.rs` with boundary artifact detection at splice regions
- [x] Add timestamp consistency analysis in `time_forensics.rs` (creation vs modification vs EXIF dates) — `MetadataForensics::check_timestamp_consistency(created, modified, exif_datetime)` + `TimestampReport`

## New Features
- [ ] Implement video forensics: temporal splice detection across frame sequences
- [ ] Add deep fake detection using facial landmark consistency analysis (no neural network required)
- [ ] Implement audio forensics module for detecting spliced/edited audio recordings
- [ ] Add `quantization_table.rs` JPEG quantization table matching against known camera databases
- [ ] Implement image phylogeny (trace image editing history from multiple versions)
- [x] Add batch forensic analysis with CSV/JSON report generation — `ForensicsAnalyzer::analyze_batch(paths: &[PathBuf])` using rayon `par_iter()`; `TamperingReport::to_json()` for JSON export
- [ ] Implement `chromatic_forensics.rs` chromatic aberration pattern analysis for lens identification
- [ ] Add video compression artifact inconsistency detection across GOP boundaries

## Performance
- [x] Parallelize `ForensicsAnalyzer::analyze()` to run independent tests concurrently with rayon — pixel-level tests dispatched via `par_iter()` on a task closure vec
- [ ] Add tile-based ELA processing in `ela.rs` for memory-efficient analysis of large images
- [ ] Optimize `copy_detect.rs` with spatial hashing for O(n) instead of O(n^2) block comparison
- [ ] Use `ndarray` parallel iterators for matrix operations in `noise.rs` and `frequency_forensics.rs`
- [ ] Add progressive analysis mode that stops early if confidence threshold is already met
- [ ] Cache DCT coefficients in `compression.rs` to avoid recomputation across tests

## Testing
- [ ] Add test suite with known tampered images (spliced, cloned, retouched) and ground truth masks
- [ ] Test `ela.rs` detection accuracy with synthetic noise addition at various levels
- [ ] Test `chain_of_custody.rs` with multi-step custody transfer scenarios
- [ ] Add `hash_registry.rs` tests with collision detection and lookup performance
- [ ] Test `steganalysis.rs` with LSB steganography embedded test images
- [ ] Test `watermark_detect.rs` with various watermark embedding strengths
- [ ] Add false positive rate measurement tests for each forensic test type

## Documentation
- [ ] Document each forensic test methodology with academic references
- [ ] Add confidence score interpretation guide for end users
- [ ] Document the chain of custody data model and verification process
