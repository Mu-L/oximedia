# oximedia-watermark TODO

## Current Status
- 40 source files implementing professional audio watermarking and steganography
- Key features: 6 watermarking algorithms (Spread Spectrum DSSS, Echo Hiding, Phase Coding, LSB Steganography, Patchwork, QIM), unified WatermarkEmbedder/WatermarkDetector API, psychoacoustic masking, Reed-Solomon error correction, blind detection, robustness testing, quality metrics (SNR, ODG)
- Modules: attacks, audio_watermark, batch_embed, bit_packing, capacity_calc, chain_of_custody, dct_watermark, detection_map, detector (BlindDetector, NonBlindDetector), echo, forensic, forensic_watermark, fragile, invisible_wm, key_schedule, lsb, media_watermark, metrics, patchwork, payload/payload_encoder, perceptual_hash, phase, psychoacoustic, qim, qr_watermark, robust, robustness, spatial_watermark, spread_spectrum, ss_audio_wm, steganography, visible/visible_watermark, watermark_database, watermark_robustness, wm_detect, wm_strength
- Dependencies: oximedia-core, oximedia-audio, rustfft, rand, reed-solomon-erasure

## Enhancements
- [x] Replace `rustfft` with OxiFFT per COOLJAPAN ecosystem policy
- [x] Replace `rand` usage with scirs2-core random facilities per SCIRS2 policy
- [x] Improve `psychoacoustic` masking model with Bark scale critical band analysis for more accurate human hearing modeling (`bark_masking.rs`)
- [x] Add multi-channel watermarking support to `WatermarkEmbedder` (stereo/5.1/7.1 with Independent/Distributed/MidOnly/Complementary/Selective strategies) (`multichannel.rs`)
- [x] Extend `spread_spectrum` with Gold code sequences for better cross-correlation properties (`gold_code.rs`)
- [ ] Add configurable Reed-Solomon parameters in `payload_encoder` (currently fixed 16,8 -- allow user-specified redundancy level)
- [ ] Improve `dct_watermark` with adaptive coefficient selection based on local signal energy
- [ ] Add watermark strength auto-tuning in `wm_strength` that maximizes robustness while staying below perceptual threshold

## New Features
- [x] Implement `video_watermark` module for spatial-domain video frame watermarking (DCT-based per-frame embedding with spatial and frequency modes) (`video_watermark.rs`)
- [ ] Add `fingerprint_watermark` module combining `perceptual_hash` with watermark for dual content identification
- [ ] Implement `realtime_embedder` for streaming/live audio watermarking with frame-by-frame processing and state persistence
- [ ] Add `watermark_comparator` module for comparing extracted watermarks against database with fuzzy matching
- [ ] Implement `multi_layer_watermark` for embedding multiple independent watermarks (owner + distributor + session) in same audio
- [ ] Add `temporal_watermark` module that encodes data across time (frame sequence) rather than within single frames
- [ ] Implement `watermark_analyzer` CLI-style module that reports embedded watermark metadata, strength, and degradation level
- [ ] Add `image_watermark` module extending spatial_watermark with DWT-based robust image watermarking

## Performance
- [ ] Optimize `spread_spectrum` FFT-based embedding with in-place transforms to halve memory allocation
- [ ] Add batch FFT processing in `phase` embedder to amortize FFT setup across multiple frames
- [ ] Implement SIMD-optimized correlation computation in `spread_spectrum` detector for faster extraction
- [ ] Cache PN sequence generation in `spread_spectrum` (currently regenerated per embed/detect call)
- [ ] Optimize `echo` embedder overlap-add convolution with FFT-based fast convolution for long kernels
- [ ] Profile `qim` quantizer and eliminate unnecessary f32<->i32 conversions in inner loop

## Testing
- [ ] Add round-trip embed/detect tests for all 6 algorithms with various payload sizes (1 byte, 32 bytes, 256 bytes)
- [x] Test robustness of each algorithm against MP3 compression, resampling, low-pass filtering, and time stretching (`robustness_suite.rs`)
- [ ] Add capacity limit tests verifying that embedding beyond capacity returns proper error
- [ ] Test `chain_of_custody` with multi-hop watermark tracking (embed A, embed B, detect both)
- [ ] Add perceptual quality tests verifying SNR > 30dB and ODG > -1.0 for all algorithms at default strength
- [ ] Test `forensic_watermark` with simulated collusion attack (averaging multiple watermarked copies)
- [ ] Benchmark embed/detect throughput for each algorithm at 44.1kHz and 96kHz sample rates

## Documentation
- [ ] Document algorithm selection guide (robustness vs capacity vs imperceptibility tradeoffs)
- [ ] Add attack resistance matrix showing which algorithms survive which attacks
- [ ] Document `chain_of_custody` workflow for forensic leak tracing use case
