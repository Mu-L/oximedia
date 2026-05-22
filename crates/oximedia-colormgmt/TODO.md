# oximedia-colormgmt TODO

## Current Status
- 60 source files; professional color management with ICC profiles, ACES workflows, HDR processing
- Color spaces: sRGB, Adobe RGB, ProPhoto RGB, Display P3, Rec.709, Rec.2020, DCI-P3
- ACES: full pipeline (IDT, RRT, ODT, LMT) with AP0 and AP1 primaries
- ICC: v2/v4 profile parsing and validation
- HDR: PQ/HLG transfer functions, tone mapping, gamut mapping
- Modules: colorspaces, aces/aces_config/aces_pipeline, icc/icc_profile, hdr (gamut, tonemapping, transfer, metadata), gamut/gamut_clip/gamut_mapping/gamut_ops, transforms (LUT, parametric), pipeline, delta_e (1976, 2000), ciecam02, color_appearance, spectral_data/spectral_locus, chromatic_adaptation, grading, curves

## Enhancements
- [x] Implement CIEDE2000 weighting parameters (k_L, k_C, k_H) as configurable in `delta_e` module
- [x] Add ICC v5 profile support in `icc/parser.rs` (iccMAX) (verified 2026-05-16; src/icc/parser.rs:259 IccVersion::V5, struct IccMaxSpectralTag:308)
- [x] Extend `hdr/tonemapping.rs` with Reinhard, Filmic, and ACES-fitted tone mapping operators (verified 2026-05-16; src/hdr/tonemapping.rs:ToneCurve)
  - **Implemented:** `ToneCurve` enum — `ReinhardSimple` (L/(1+L)), `ReinhardExtended { l_white }` (L*(1+L/lw²)/(1+L)), `FilmicHable` (Hable/Uncharted2 7-param curve, A=0.15 B=0.50 C=0.10 D=0.20 E=0.02 F=0.30 W=11.2), `AcesFitted` (Narkowicz rational, a=2.51 b=0.03 c=2.43 d=0.59 e=0.14).
  - **Tests:** 7 tests — Reinhard simple (1.0→0.5), extended (4.0→1.0 at l_white=4), Filmic (monotone on [0,W]), ACES (1.0→~0.83), monotone property test on [0,16].
- [x] Add Jzazbz and JzCzhz perceptual color space support alongside CIECAM02
- [ ] Extend `gamut_mapping.rs` with cusp-based gamut mapping (ACES gamut mapping algorithm) (verified-open 2026-05-16: no cusp/ACES gamut mapping in gamut_mapping.rs)
- [x] Add multi-illuminant support in `chromatic_adaptation.rs` (beyond Bradford/Von Kries) (verified 2026-05-16; src/chromatic_adaptation.rs:296 compute_multi_illuminant_adaptation, MultiIlluminantAdapter:347)
- [x] Implement automatic color space detection from ICC profile in `display_profile.rs` (verified 2026-05-16; src/display_profile.rs:399 detect_color_space_from_primaries, detected_color_space:499)
- [ ] Extend `color_quantize.rs` with octree and k-means quantization algorithms (verified-open 2026-05-16: no octree/k-means in color_quantize.rs)

## New Features
- [x] Implement OCIO (OpenColorIO) configuration file parser for studio pipeline integration (verified 2026-05-16; src/ocio_parser.rs:1 OcioConfig, OcioColorSpace, transforms)
- [x] Add CTL (Color Transformation Language) interpreter for custom ACES transforms (verified 2026-05-16; src/ctl_interpreter.rs:1269 lines, ctl_lexer.rs, ctl_parser.rs)
- [x] Implement spectral rendering via `spectral_data.rs` for physically accurate color processing (verified 2026-05-16; src/spectral_data.rs:410, src/spectral_upsampling.rs:1514 lines)
- [x] Add display characterization and profiling utilities in `display_profile.rs` (verified 2026-05-16; src/display_profile.rs:729 lines display characterization)
- [x] Implement color grading LUT generation (3D LUT export) in `grading/` (verified 2026-05-16; src/grading/lut_export.rs:498 lines GradingLut3D)
- [x] Add ACES Output Transform 2.0 (candidate) implementation in `aces_pipeline.rs` (verified 2026-05-16; src/aces_output_transform.rs:1 ACES OT 2.0, AcesOutputTransform2:67)
- [x] Implement ICtCp color space for HDR content perceptual analysis
- [x] Add Oklab/Oklch perceptual color space support for uniform perceptual blending

## Performance
- [x] Add SIMD-accelerated 3x3 matrix multiply for bulk pixel transforms in `transforms/` (verified 2026-05-16; src/transforms/batch_matrix.rs:164 invert_matrix_3x3, src/transforms/matrix_bulk.rs:35 apply_matrix_bulk_f64)
- [x] Implement LUT interpolation with tetrahedral interpolation in `transforms/lut.rs` (verified 2026-05-16; src/transforms/lut.rs:91 tetrahedral_interpolate from math::interpolation)
- [ ] Cache linearization/delinearization lookup tables in `transfer_function.rs` (verified-open 2026-05-16: no cache/OnceLock in transfer_function.rs)
- [x] Parallelize batch pixel conversion using rayon in `pipeline/mod.rs` (verified 2026-05-16; src/pipeline/mod.rs:172 rayon par_iter under #[cfg(feature = "rayon")])
- [ ] Optimize `delta_e_2000` with precomputed trigonometric tables for batch color comparisons (verified-open 2026-05-16: no precomputed trig tables in utils/delta_e.rs)

## Testing
- [ ] Add CIE standard illuminant round-trip tests (D50, D65, A, F2) in `white_point.rs`
- [ ] Test ICC profile parsing against real-world profiles (sRGB IEC61966, Adobe RGB, ProPhoto)
- [ ] Add accuracy benchmarks: verify sRGB-to-Rec.2020 conversion achieves dE < 1.0
- [ ] Test ACES pipeline end-to-end: IDT -> ACEScg -> RRT -> sRGB ODT
- [ ] Add edge-case tests for out-of-gamut colors in `gamut_clip.rs`
- [ ] Test `color_blindness.rs` simulation accuracy against Brettel/Vienot references

## Documentation
- [ ] Add color pipeline architecture diagram showing transform chain
- [ ] Document supported transfer functions with mathematical definitions
- [ ] Add ACES workflow tutorial with code examples in crate-level docs
