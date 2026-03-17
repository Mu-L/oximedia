# oximedia-lut TODO

## Current Status
- 42+ modules implementing professional LUT and color science
- LUT core: lut1d, lut3d, interpolation (trilinear, tetrahedral), identity_lut, lut_resample
- LUT ops: lut_chain, lut_chain_ops, lut_combine, lut_analysis, lut_validate, lut_stats, lut_dither, lut_fingerprint
- LUT I/O: formats (cube, 3dl, csp), lut_io, cube_writer, export, lut_gradient, lut_metadata, lut_provenance, lut_version
- Color spaces: colorspace (Rec.709, Rec.2020, DCI-P3, Adobe RGB, sRGB, ProPhoto, ACES AP0/AP1), matrix operations
- Color science: chromatic adaptation (Bradford, Von Kries), temperature (Kelvin to RGB), gamut mapping
- HDR: hdr_lut, hdr_metadata, hdr_pipeline (Reinhard, ACES filmic, Drago, Hejl), tonemap
- Advanced: aces pipeline, baking, builder, color_cube, domain_clamp, gamut_compress_lut, hald_clut
- Creative: photographic_luts, preview
- Dependencies: oximedia-core, nom, serde

## Enhancements
- [ ] Add CTL (Color Transformation Language) script interpretation for ACES transforms in `aces.rs`
- [ ] Extend `tetrahedral.rs` with pyramidal interpolation as an alternative to tetrahedral
- [ ] Add LUT size validation and automatic resampling when chaining LUTs of different sizes in `lut_chain.rs`
- [ ] Implement LUT inversion (analytical for 1D, iterative for 3D) in `lut_combine.rs`
- [ ] Extend `formats/cube.rs` parser to handle malformed .cube files with graceful error recovery
- [ ] Add `lut_analysis.rs` gamut coverage analysis (percentage of target gamut covered by LUT transform)
- [ ] Implement `lut_validate.rs` monotonicity checks for 1D LUTs and smoothness checks for 3D LUTs
- [ ] Extend `hdr_pipeline.rs` with ACES RRT v1.2 reference rendering transform

## New Features
- [ ] Add a `clf.rs` module for Common LUT Format (CLF/DLP) read/write (Academy/ASC standard)
- [ ] Implement a `icc_profile.rs` module for ICC profile to 3D LUT conversion
- [ ] Add a `lut_preview_html.rs` module for generating interactive HTML LUT preview (color wheel, before/after)
- [ ] Implement a `creative_grade.rs` module with named film emulation presets (Kodak Vision3, Fuji Eterna)
- [ ] Add a `display_calibration.rs` module for generating display calibration LUTs from measurement data
- [ ] Implement a `lut_compress.rs` module for lossy LUT compression (reduce 65^3 to smaller with quality metric)
- [ ] Add a `color_chart.rs` module for Macbeth/ColorChecker chart analysis and LUT generation from chart photos
- [ ] Implement a `lut_blend.rs` module for blending between two LUTs with a mix factor (crossfade grading)

## Performance
- [ ] Add SIMD-accelerated tetrahedral interpolation in `tetrahedral.rs` (process 4 pixels per iteration)
- [ ] Implement batch pixel processing in `lut3d.rs` (apply LUT to entire scanline/image buffer at once)
- [ ] Add multi-threaded LUT application in `lut3d.rs` using rayon for image-wide processing
- [ ] Cache interpolation coefficients in `interpolation.rs` for repeated lookups at nearby coordinates
- [ ] Optimize `lut1d.rs` with branchless binary search for curve lookups
- [ ] Add GPU compute shader for 3D LUT application (wgpu-based, feature-gated)

## Testing
- [ ] Add round-trip tests for all LUT formats: .cube -> parse -> write -> parse -> compare
- [ ] Test `tetrahedral.rs` interpolation accuracy against reference values from DaVinci Resolve
- [ ] Add `chromatic.rs` adaptation tests with D50/D65/A illuminant pairs against CIE reference data
- [ ] Test `temperature.rs` Kelvin-to-RGB conversion against CIE daylight locus values
- [ ] Add `hdr_pipeline.rs` tone mapping tests comparing output against reference HDR test patterns
- [ ] Test `lut_chain.rs` with long chains (>10 LUTs) for numerical stability

## Documentation
- [ ] Add a color space conversion matrix reference table in `colorspace.rs` docs
- [ ] Document the ACES pipeline stages with a flowchart (IDT -> ACEScg -> RRT -> ODT)
- [ ] Add a LUT format comparison table (.cube vs .3dl vs .csp capabilities)
