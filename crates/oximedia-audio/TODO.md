# oximedia-audio TODO

## Current Status
- 100+ source files across codecs (Opus, Vorbis, FLAC, MP3, PCM), DSP (biquad, compressor, delay, EQ, limiter, reverb), effects (chorus, flanger, phaser, LFO), spectrum (FFT, analyzer, spectrogram, waveform, features), fingerprint (constellation, hash, matching, database), loudness (EBU R128, ATSC A/85, K-weighting, gating, true peak), meters (VU, PPM, peak, RMS, correlation, goniometer, Dolby, ITU), spatial (ambisonics, binaural, panning, reverb), description (ducking, mixing, synthesis, timing, metadata)
- `AudioDecoder`/`AudioEncoder` traits, `AudioFrame`, `ChannelLayout`, `Resampler`
- Feature-gated codecs: opus, vorbis, flac, mp3, pcm
- Dependencies: oximedia-core, rubato, audioadapter, oxifft, bytes

## Enhancements
- [x] Add gapless playback support with proper encoder delay/padding handling in codec traits
- [x] Implement true peak limiter in `loudness/peak` with 4x oversampled detection
- [x] Add multi-band compressor in `compressor` (crossover network + per-band compression)
- [x] Implement look-ahead delay in `compressor` and `gate` for attack anticipation
- [x] Add wet/dry mix parameter to all `effects` (chorus, flanger, phaser)
- [x] Implement sidechain input for `compressor` and `gate` (external key signal)
- [x] Add auto-gain in `loudness/normalize` to maintain consistent output level after processing (verified 2026-05-16; src/auto_gain.rs:560 lines AutoGain)
- [x] Implement `Resampler` quality presets (draft/good/best) mapping to rubato configurations
- [x] Add `AudioFrame` format conversion utilities (interleaved <-> planar, bit depth conversion)
- [x] Implement FLAC encoder compression level parameter (0-8) in `flac/encoder` (verified 2026-05-16; src/flac/encoder.rs:24 CompressionLevel(u8) 0-8, with_compression_level:150)

## New Features
- [x] Add AAC decoder (patent-free since 2023) as feature-gated module (verified 2026-05-16; src/aac.rs:734 lines AacDecoder)
- [x] Implement ALAC (Apple Lossless) decoder for Apple ecosystem compatibility (verified 2026-05-16; src/alac.rs:612 lines AlacDecoder)
- [x] Add WAV file reader/writer with full RIFF chunk handling (verified 2026-05-16; src/wav.rs:790 lines WavReader/WavWriter)
- [ ] Implement audio watermarking module (embed/detect inaudible watermarks) (verified-open 2026-05-16: oximedia-watermark has this but not in oximedia-audio crate itself)
- [x] Add noise reduction module (spectral subtraction, Wiener filter) (verified 2026-05-16; src/noise_reduce.rs:638 lines NoiseReducer)
- [x] Implement click/pop removal for vinyl restoration workflows (verified 2026-05-16; src/click_remove.rs:506 lines ClickRemover)
- [x] Add convolution reverb using impulse response loading (verified 2026-05-16; src/convolution_reverb.rs:567 lines ConvolutionReverb)
- [x] Implement graphic equalizer (31-band ISO standard) using `biquad` banks (verified 2026-05-16; src/graphic_eq.rs:581 lines GraphicEq)
- [x] Add audio ducking module (auto-duck music under voiceover) (verified 2026-05-16; src/ducking.rs:557 lines AudioDucker)
- [x] Implement Dolby Atmos object metadata parsing for spatial audio rendering (verified 2026-05-16; src/dolby_atmos.rs:962 lines DolbyAtmosParser)

## Performance
- [x] Replace `rustfft` with OxiFFT per COOLJAPAN Policy
- [x] Add SIMD-optimized sample format conversion in `format_convert` (verified 2026-05-16; src/format_convert.rs:168 SIMD-optimised batch conversion, auto-vectorised chunks of 8)
- [x] Implement lock-free ring buffer for real-time audio threading in `stream_buffer` (verified 2026-05-16; src/stream_buffer.rs:136 struct StreamBuffer)
- [x] Optimize `biquad` filter with direct form II transposed for better numerical behavior (implemented 2026-05-15; src/dsp/biquad.rs BiquadDf2t struct, 2 delay elements, IR matches DF1 to 1e-12)
- [x] Add batch processing mode to `meters` (process multiple channels simultaneously) (verified 2026-05-16; src/meters/batch.rs:57 BatchMeterConfig, BatchMeterProcessor)
- [x] Implement FFT plan caching in `spectrum/fft` to avoid repeated planner allocation (verified 2026-05-16; src/spectrum/fft_cache.rs:52 struct FftPlanCache, hit/miss counters)
- [x] Optimize Vorbis MDCT with split-radix algorithm in `vorbis/mdct` (implemented 2026-05-15; src/vorbis/mdct.rs MdctFast struct, FFT-based O(N log N) forward+inverse via oxifft)

## Testing
- [x] Add FLAC round-trip test: encode -> decode -> bit-exact comparison (8 tests in tests/conformance_tests.rs)
- [ ] Test Opus encoder/decoder with ITU-T P.862 PESQ-like quality metric (requires external PESQ library; deferred)
- [x] Add `loudness` EBU R128 conformance test with EBU test signals (8 tests in tests/conformance_tests.rs)
- [x] Test `meters/vu` ballistics against IEC 60268-10 specified rise/fall times (8 tests in tests/conformance_tests.rs)
- [x] Test `spatial/ambisonics` encoding/decoding round-trip for 1st order (4 tests in tests/conformance_tests.rs)
- [x] Add `fingerprint` matching accuracy test with time-stretched and pitch-shifted audio (4 tests in tests/conformance_tests.rs)
- [x] Test `effects/chorus` with known LFO parameters and verify modulation depth (4 tests in tests/conformance_tests.rs)

## Documentation
- [x] Document codec feature gates and their compile-time implications (implemented 2026-05-15; lib.rs feature gate table)
- [x] Add DSP signal flow diagrams for compressor, reverb, and EQ chains (implemented 2026-05-15; dsp/compressor.rs, dsp/reverb.rs, dsp/eq.rs signal flow ASCII art)
- [x] Document `AudioFrame` memory layout and channel ordering conventions (implemented 2026-05-15; frame.rs module + struct doc)
