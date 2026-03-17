# oximedia-codec TODO

## Current Status
- 100+ source files; video codecs: AV1, VP9, VP8, Theora, H.263, FFV1; audio: Opus (SILK+CELT); image: PNG, GIF, WebP, JPEG-XL
- Feature-gated codecs: av1, vp9, vp8, theora, h263, opus, ffv1, jpegxl, image-io
- Subsystems: rate_control (CBR/VBR/CRF/CQP), multipass encoding, SIMD (x86 AVX2/AVX-512, ARM, scalar), intra prediction, motion estimation, tile encoding, reconstruction (CDEF, deblock, film grain, super-res)
- Re-exports: VideoFrame, AudioFrame, VideoDecoder/Encoder traits, rate control types, reconstruction pipeline, tile encoder

## Enhancements
- [ ] Complete VP9 encoder (currently only `Vp9Decoder` is re-exported; no `Vp9Encoder`)
- [ ] Complete VP8 encoder (currently only `Vp8Decoder` is re-exported; no `Vp8Encoder`)
- [ ] Improve AV1 film grain synthesis fidelity in `av1/film_grain.rs` with per-block grain parameters
- [ ] Add temporal scalability (SVC) support to AV1 encoder via `av1/sequence.rs`
- [ ] Extend `rate_control/lookahead.rs` with scene-adaptive bitrate allocation using content analysis
- [ ] Improve Opus encoder with voice activity detection (VAD) in `opus/silk.rs`
- [ ] Add adaptive quantization matrix selection in `av1/quantization.rs` based on content type
- [ ] Extend `motion/diamond.rs` with hexagonal and UMHex search patterns for faster estimation

## New Features
- [ ] Add AVIF still image encoding/decoding (AV1-based) as a codec variant
- [ ] Implement Vorbis audio encoder/decoder (currently missing; only Opus is present)
- [ ] Add FLAC audio encoder/decoder for lossless audio
- [ ] Implement PCM codec support (trivial encode/decode for raw audio)
- [ ] Add APNG (animated PNG) support in `png/` module
- [ ] Implement WebP animation encoding in `webp` module
- [ ] Add two-pass encoding support to the Theora encoder in `theora/rate_ctrl.rs`
- [ ] Implement constant-quality mode for GIF encoder in `gif` module

## Performance
- [ ] Expand SIMD coverage: add ARM NEON implementations in `simd/arm/` (currently only mod.rs)
- [ ] Add WASM SIMD128 backend in `simd/` for browser-based encoding
- [ ] Optimize AV1 CDEF filter with SIMD in `simd/av1/cdef.rs` for 10-bit depth
- [ ] Add parallel tile decoding in `tile.rs` using rayon work-stealing
- [ ] Optimize entropy coding in `entropy_coding.rs` with table-based arithmetic coding
- [ ] Profile and optimize `reconstruct/loop_filter.rs` hot paths with cache-friendly access patterns
- [ ] Add SIMD-accelerated pixel format conversion for YUV420/422/444

## Testing
- [ ] Add bitstream conformance tests for AV1 decoder against reference test vectors
- [ ] Add round-trip encode/decode quality tests for each codec (PSNR > threshold)
- [ ] Test rate control accuracy: verify CBR output stays within 5% of target bitrate
- [ ] Add fuzzing targets for `png/decoder.rs`, `gif` decoder, and `webp` decoder
- [ ] Test multipass encoding produces better quality than single-pass at same bitrate
- [ ] Add regression tests for `jpegxl` modular and ANS coding paths

## Documentation
- [ ] Document codec feature matrix (encode/decode, bitdepth, chroma support) in crate-level docs
- [ ] Add rate control tuning guide with examples for each mode (CBR/VBR/CRF/CQP)
- [ ] Document SIMD dispatch mechanism in `simd/mod.rs`
