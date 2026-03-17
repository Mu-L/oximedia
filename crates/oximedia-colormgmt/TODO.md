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
- [ ] Add ICC v5 profile support in `icc/parser.rs` (iccMAX)
- [ ] Extend `hdr/tonemapping.rs` with Reinhard, Filmic, and ACES-fitted tone mapping operators
- [x] Add Jzazbz and JzCzhz perceptual color space support alongside CIECAM02
- [ ] Extend `gamut_mapping.rs` with cusp-based gamut mapping (ACES gamut mapping algorithm)
- [ ] Add multi-illuminant support in `chromatic_adaptation.rs` (beyond Bradford/Von Kries)
- [ ] Implement automatic color space detection from ICC profile in `display_profile.rs`
- [ ] Extend `color_quantize.rs` with octree and k-means quantization algorithms

## New Features
- [ ] Implement OCIO (OpenColorIO) configuration file parser for studio pipeline integration
- [ ] Add CTL (Color Transformation Language) interpreter for custom ACES transforms
- [ ] Implement spectral rendering via `spectral_data.rs` for physically accurate color processing
- [ ] Add display characterization and profiling utilities in `display_profile.rs`
- [ ] Implement color grading LUT generation (3D LUT export) in `grading/`
- [ ] Add ACES Output Transform 2.0 (candidate) implementation in `aces_pipeline.rs`
- [x] Implement ICtCp color space for HDR content perceptual analysis
- [x] Add Oklab/Oklch perceptual color space support for uniform perceptual blending

## Performance
- [ ] Add SIMD-accelerated 3x3 matrix multiply for bulk pixel transforms in `transforms/`
- [ ] Implement LUT interpolation with tetrahedral interpolation in `transforms/lut.rs`
- [ ] Cache linearization/delinearization lookup tables in `transfer_function.rs`
- [ ] Parallelize batch pixel conversion using rayon in `pipeline/mod.rs`
- [ ] Optimize `delta_e_2000` with precomputed trigonometric tables for batch color comparisons

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
