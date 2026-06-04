# oximedia-lut TODO

## Current Status
- Core modules: lut1d, lut3d, interpolation, identity_lut, lut_resample
- LUT ops: lut_chain, lut_combine, lut_analysis, lut_validate, lut_stats, lut_dither, lut_fingerprint
- LUT I/O: formats (cube), lut_io, cube_writer, export, lut_gradient, lut_metadata, lut_provenance, lut_version
- Color spaces: colorspace (Rec.709, Rec.2020, DCI-P3, Adobe RGB, sRGB, ProPhoto, ACES AP0/AP1), matrix operations
- Color science: chromatic adaptation (Bradford, Von Kries), temperature (Kelvin to RGB), gamut mapping
- HDR: hdr_lut, hdr_metadata, hdr_pipeline (Reinhard, ACES filmic, Drago, Hejl), tonemap
- Advanced: aces pipeline, baking, builder, color_cube, domain_clamp, gamut_compress_lut

## Enhancements
- [x] Add LUT size validation and automatic resampling when chaining LUTs of different sizes in `lut_chain.rs`
- [x] Implement `lut_validate.rs` monotonicity checks for 1D LUTs and smoothness checks for 3D LUTs
- [x] Add `lut_analysis.rs` gamut coverage analysis (percentage of target gamut covered by LUT transform)
- [x] Implement LUT inversion (analytical for 1D, iterative for 3D) in `lut_combine.rs` (verified 2026-05-16; src/invert.rs:246 lines)
- [x] Extend `hdr_pipeline.rs` with ACES RRT v1.2 reference rendering transform (verified 2026-05-16; src/aces.rs aces_rrt fn used in tests at line 300)

## New Features
- [x] Add a `clf.rs` module for Common LUT Format (CLF/DLP) read/write (Academy/ASC standard) (verified 2026-05-16; src/clf.rs:1141 lines)
- [x] Implement a `creative_grade.rs` module with named film emulation presets (Kodak Vision3, Fuji Eterna) (verified 2026-05-16; src/creative_grade.rs:929 lines)
- [x] Add a `lut_blend.rs` module for blending between two LUTs with a mix factor (crossfade grading) (verified 2026-05-16; src/lut_blend.rs:583 lines)
- [x] Implement a `display_calibration.rs` module for generating display calibration LUTs (verified 2026-05-16; src/display_calibration.rs:637 lines)
- [x] Add a `lut_compress.rs` module for lossy LUT compression (verified 2026-05-16; src/lut_compress.rs:632 lines)

## Testing
- [ ] Add round-trip tests for all LUT formats: .cube -> parse -> write -> parse -> compare
- [ ] Test `lut_chain.rs` with long chains (>10 LUTs) for numerical stability

## 0.1.8 Wave 6 — 2026-05-29
- [x] Register 22 orphan modules in lib.rs + hald_clut cfix (Slice D, 2026-05-29)
