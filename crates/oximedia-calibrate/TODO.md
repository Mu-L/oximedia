# oximedia-calibrate TODO

## Current Status
- 29 modules covering camera calibration, display calibration, color matching, ICC profiles, LUT generation, white balance, color temperature, gamut mapping, chromatic adaptation
- Sub-directories: `camera/`, `chromatic/`, `display/`, `gamut/`, `icc/`, `lut/`, `match/`, `temp/`, `white/`
- Supports ColorChecker Classic 24, Passport, Datacolor SpyderCheckr, and custom targets
- Dependencies on `ndarray` (policy violation pending)

## Enhancements
- [x] Replace `ndarray` dependency with SciRS2-Core per SCIRS2 policy (ndarray was not present; pure Rust confirmed)
- [x] Add CAT16 chromatic adaptation transform (from CIECAM16) in `chromatic` module (`chromatic/cat16.rs` + `cat16_adapt` free fn)
- [ ] Extend `icc` module with ICC v5 (iccMAX) profile support
- [x] Add DCI-P3 and Rec.2020 working color space support in `color_space` (`ColorSpace` enum with `DciP3`, `Rec2020`, `DciP3D65` variants)
- [x] Implement CIEDE2000 delta-E formula in `delta_e` (`delta_e_2000` + `test_delta_e2000_identical`)
- [ ] Extend `camera` profiling with dual-illuminant calibration (D50 + A) for DNG color matrices
- [ ] Add `display_verify` automated pass/fail report generation against target tolerances
- [ ] Improve `gamut_checker` with per-pixel gamut boundary visualization
- [x] Extend `lut` generation with tetrahedral interpolation for 3D LUTs (`trilinear_lookup_3d`, `tetrahedral_lookup_3d` free fns + `test_tetrahedral_identity_lut`)

## New Features
- [ ] Add `spectral_reconstruction` module to recover spectral data from RGB camera measurements
- [ ] Implement `aces_calibration` module for ACES (Academy Color Encoding System) IDT generation
- [ ] Add `hdr_calibration` module for PQ/HLG display calibration with peak luminance mapping
- [ ] Implement `printer_calibration` module for CMYK soft-proofing workflows
- [ ] Add `ambient_compensation` module adjusting display profile based on ambient light measurement
- [ ] Implement `batch_calibrate` module for calibrating multiple cameras from a single shooting session
- [ ] Add `calibration_schedule` module tracking when devices are due for re-calibration

## Performance
- [ ] Use SIMD for 3x3 matrix multiplication in `chromatic` adaptation transforms
- [ ] Parallelize ICC profile application across image tiles using rayon
- [ ] Cache frequently-used illuminant-to-illuminant adaptation matrices in `chromatic`
- [ ] Optimize `patch_extract` with early-exit when sufficient patches detected
- [ ] Pre-compute and cache LUT interpolation coefficients in `lut` module

## Testing
- [ ] Add test for `camera` profile generation against known ColorChecker reference values
- [ ] Test ICC profile round-trip: generate from measurements, write, read back, verify TRCs match
- [ ] Add delta-E verification test: calibrated output vs reference should be under 2.0 dE
- [ ] Test `chromatic` Bradford adaptation against published ASTM E308 reference data
- [ ] Add `gamut_checker` test with known out-of-gamut colors verifying correct mapping
- [ ] Test `white_balance` presets produce expected RGB multiplier ratios

## Documentation
- [ ] Document end-to-end camera calibration workflow with code examples
- [ ] Add color science reference appendix explaining CIE XYZ, Lab, illuminants, observers
- [ ] Document supported ColorChecker targets with patch layout diagrams
