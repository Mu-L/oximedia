# oximedia-hdr TODO

## Current Status
- 8 modules covering HDR video processing
- Transfer functions: PQ (ST 2084), HLG, SDR gamma in `transfer_function.rs`
- Gamut handling: Rec.2020, color gamut conversion matrices in `gamut.rs`
- Tone mapping: configurable operators in `tone_mapping.rs`
- HDR10/HDR10+ metadata: CLL, mastering display parsing/encoding in `color_volume.rs`
- Dolby Vision: profile detection, RPU data, cross-version metadata in `dolby_vision_profile.rs`
- Dynamic metadata: per-frame HDR10+ dynamic metadata in `dynamic_metadata.rs`
- HLG advanced: HLG-to-HDR10 and HLG-to-SDR conversion in `hlg_advanced.rs`
- Dependencies: thiserror, serde, log

## Enhancements
- [ ] Add scene-referred tone mapping with per-frame luminance analysis in `tone_mapping.rs`
- [ ] Extend `gamut.rs` with soft-clip gamut mapping (perceptual desaturation instead of hard clamp)
- [ ] Add BT.2446 Method A and Method C tone mapping operators to `tone_mapping.rs`
- [ ] Implement Dolby Vision RPU generation (not just parsing) in `dolby_vision_profile.rs`
- [ ] Extend `hlg_advanced.rs` with system gamma adjustment based on display peak luminance
- [ ] Add MaxRGB and percentile luminance statistics computation for `color_volume.rs` CLL auto-detect
- [ ] Implement inverse tone mapping (SDR-to-HDR upconversion) in `tone_mapping.rs`
- [ ] Add validation for HDR10+ dynamic metadata JSON schema in `dynamic_metadata.rs`

## New Features
- [ ] Add a `hdr_histogram.rs` module for luminance histogram analysis of HDR frames
- [ ] Implement a `display_model.rs` module for target display characterization (peak nits, black level, gamut)
- [ ] Add a `color_volume_transform.rs` module for ICtCp color space conversions (BT.2100)
- [ ] Implement a `vivid_hdr.rs` module for Vivid HDR (SL-HDR1/SL-HDR2) metadata support
- [ ] Add a `hdr_scopes.rs` module for waveform/vectorscope analysis of HDR content
- [ ] Implement a `cuva_metadata.rs` module for CUVA HDR Vivid dynamic metadata (Chinese HDR standard)
- [ ] Add a `st2094.rs` module for SMPTE ST 2094 series dynamic metadata (beyond HDR10+)

## Performance
- [ ] Add SIMD-optimized PQ EOTF/OETF computation in `transfer_function.rs` (batch f32 processing)
- [ ] Implement LUT-based fast path for PQ and HLG transfer functions (precomputed 1D tables)
- [ ] Add parallel per-row tone mapping in `tone_mapping.rs` using rayon
- [ ] Cache gamut conversion matrices in `gamut.rs` to avoid recomputation per pixel

## Testing
- [ ] Add reference value tests for `pq_eotf`/`pq_oetf` against ITU-R BT.2100 Table values
- [ ] Test `hlg_oetf`/`hlg_eotf` round-trip accuracy across the full luminance range
- [ ] Add tests for `dolby_vision_profile.rs` with real-world RPU bitstream samples
- [ ] Test tone mapping operators against ITU-R BT.2446 reference implementations
- [ ] Add edge case tests for `color_volume.rs` with zero/negative luminance values

## Documentation
- [ ] Add a flowchart showing the HDR-to-SDR conversion pipeline (metadata -> gamut map -> tone map -> transfer)
- [ ] Document supported Dolby Vision profiles and their constraints
- [ ] Add reference links to SMPTE/ITU standards for each module
