# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.5] - 2026-04-21

### Added
- **Pure-Rust ONNX inference via OxiONNX** — new `oximedia-ml` crate wrapping `oxionnx` 0.1.2, `oxionnx-core`, `oxionnx-gpu`, and `oxionnx-directml` as optional deps. Typed pipelines with zero-cost defaults: no ONNX symbols are linked unless the `onnx` feature is explicitly enabled.
- **`oximedia-ml` core types** — `OnnxModel` (Session wrapper), `ModelCache` (concurrent `Arc<Mutex<_>>` map with optional LRU capacity), `TypedPipeline` trait (`Input`/`Output` associated types + `process()`), `DeviceType` with `DeviceType::auto()` runtime probe (`Cpu`/`Cuda`/`WebGpu`/`DirectMl`/`CoreMl`), `ImagePreprocessor` (ImageNet mean/std normalization, NCHW/NHWC, letterbox/resize-to-fit), postprocess helpers (`softmax`, `sigmoid`, `argmax`, `top_k`), and a `ModelZoo` registry scaffold.
- **`SceneClassifier` pipeline** — Places365/ImageNet-style typed pipeline on OxiONNX, configurable `top_k`, ImageNet-normalized 224×224 NCHW preprocessing, softmax → top-K postprocess. Constructors: `from_model`, `from_path`, `with_top_k`.
- **`ShotBoundaryDetector` pipeline** — TransNetV2-compatible I/O (48×27 NCHW rolling window of frames, many-hot output for hard/soft cuts) with configurable window length and threshold; returns `Vec<ShotBoundary { frame_index, confidence, kind: Hard | SoftCut }>`.
- **Facade integration** — new `oximedia::ml` module re-exporting `oximedia-ml` behind `features = ["ml"]`; sub-features `ml-scene-classifier`, `ml-shot-boundary`, and `ml-onnx` for selective inclusion. `full` feature now picks up `ml`, `ml-scene-classifier`, `ml-shot-boundary`.
- **Workspace deps** — added `oxionnx-ops`, `oxionnx-gpu`, `oxionnx-directml`, and `oxionnx-proto` at 0.1.2 to root `[workspace.dependencies]` so sub-crates can opt in via `workspace = true`.
- **Example** — `examples/ml_scene_classify.rs` demonstrates end-to-end scene classification via the typed pipeline (gated by `ml` + `ml-scene-classifier`).
- **Feature gates on `oximedia-ml`** — `onnx`, `cuda`, `webgpu`, `directml`, `scene-classifier`, `shot-boundary`, `all-pipelines` (default build remains symbol-free).
- **Tests** — 55+ tests across `oximedia-ml` covering model-cache concurrency, LRU eviction, preprocessing (ImageNet normalize, letterbox, layout), pipeline contracts, and synthetic tensor fixtures.
- **Comprehensive ML guide** (`docs/ml_guide.md`) + README `Sovereign ML Pipelines` section covering typed pipelines, feature matrix (crate + facade + downstream), device selection with GPU backend table, CLI reference, WASM support matrix, and roadmap — Wave 6 Slice C.
- **Python `oximedia.ml` submodule** (Wave 5 Slice B, 2026-04-21) — new PyO3 bindings for the typed ML pipeline stack, gated on the `oximedia-py/ml` feature. Exposes `MlDeviceType` (with `auto`/`cpu`/`cuda`/`webgpu`/`directml`/`coreml` constructors, `from_name`, `list_available`, `capabilities`), `MlDeviceCapabilities` (rich probe record), `OnnxModel` (`load`/`load_from_bytes` accepting bytes, per-model `info()`/`device()`), `MlModelInfo`/`MlTensorSpec`/`MlTensorDType`, `MlModelZoo` + `MlModelEntry` mirroring the zoo registry, and the full pipeline set: `SceneClassifier`, `ShotBoundaryDetector` (+ always-available `heuristic()` fallback), `AestheticScorer`, `ObjectDetector`, `FaceEmbedder`. Numpy `(H, W, 3) uint8` arrays for image pipelines and `(N, H, W, 3) uint8` for the shot-boundary sliding window. Result wrappers (`SceneClassification`, `ShotBoundary`, `AestheticScore`, `Detection`, `FaceEmbedding`) are Python-native dataclass-like objects; `FaceEmbedding` supports `cosine_similarity`, `to_list()`, and `to_numpy()`. 11 integration smoke tests in `crates/oximedia-py/tests/ml_smoke.rs` drive the submodule via an embedded Python interpreter. Depends on `oximedia-ml/all-pipelines`; not pulled in by default, so the default `pip install oximedia` build stays lean.

### Changed
- Workspace version bumped to **0.1.5** (was 0.1.4).
- `oximedia` facade gains the `ml` feature (off by default); the `full` feature now pulls in `ml` plus the `ml-scene-classifier` and `ml-shot-boundary` sub-features.
- **Codec decoder honesty pass (documentation-only)**: introduced a four-tier decoder taxonomy (`Verified` / `Functional` / `Bitstream-parsing` / `Experimental`) in the top-level README and in `oximedia-codec/README.md`. Decoders that previously carried a "Stable" / "Complete" label but do not yet reconstruct pixel or sample data end-to-end (AV1, VP9, VP8, Theora, Vorbis, AVIF) are now accurately labelled `Bitstream-parsing`. No source behaviour changes — the decoders still parse the bitstream as before. See `docs/codec_status.md` for the full per-decoder status, what each stub is missing, and the effort estimate.
- `examples/decode_video.rs` rewritten to reflect the real decoder-status matrix instead of printing fake code samples that pretended to drive a full AV1/VP9 decode.

### Added
- **`docs/codec_status.md`** — per-decoder state, missing pieces, effort bucket (small / medium / large / specialist), and 0.1.5-vs-0.2.0+ target. Referenced from the top-level README, `oximedia-codec/README.md`, and `TODO.md`.
- **`crates/oximedia-codec/tests/av1_real_bitstream.rs`** — `#[ignore]`'d integration test harness for GitHub issue #9. Reads a real AV1 bitstream path from the `OXIMEDIA_AV1_FIXTURE` env var (skips cleanly when unset, so no binary fixture ships in the repo) and asserts that the Y plane of at least one decoded frame has non-zero variance. Will pass automatically once AV1 pixel reconstruction lands.
- **`TODO.md`** gains a "Codec Implementation Roadmap" section mirroring `docs/codec_status.md` effort buckets.
- Documentation round 3: `docs/rate_control.md`, `docs/simd_dispatch.md`, `docs/wave5_deltas.md`.

### Notes
- `oximedia-neural` continues to ship its pre-existing homegrown ONNX-style runtime alongside the new `oximedia-ml` OxiONNX-backed pipelines; consolidation onto a single ML stack is planned for a future milestone.
- CPU inference is fully pure-Rust via `oxionnx`. GPU backends (`cuda`, `webgpu`, `directml`) are additive feature gates wired in `oximedia-ml`; broader crate-by-crate integration (Waves 3–6 on the 0.1.5 TODO list) will land in subsequent cycles.

### Validated
- **Wave 6 Slice D — Full CI gate** (2026-04-21): `cargo check --workspace --all-features` clean; `cargo clippy --workspace --features onnx --all-targets -- -D warnings` clean (zero warnings); `cargo doc --workspace --features onnx --no-deps` clean after fixing 3 pre-existing unresolved intra-doc links to `MlError` in `oximedia-scene::ml` (fully-qualified to `oximedia_ml::MlError`); ML stack end-to-end tests all green — `oximedia-ml` 124 + 22 doctests, `oximedia-scene` 790, `oximedia-shots` 906, `oximedia-recommend` 991, `oximedia-mir` 800, `oximedia-caption-gen` 491 (4,124 tests); WASM gate clean for `oximedia-ml` (default, `onnx`, `onnx+webgpu`) and facade `oximedia --features ml` on `wasm32-unknown-unknown`; facade feature matrix validated (`no-default`, `ml`, `ml-onnx`, `full`); all `oximedia-ml` source files well under 2000-line refactor threshold (largest: `model.rs` at 500 lines).
- **Pre-existing non-ML surface noise surfaced (not blocking)**: `oximedia-container` emits an `unused import: TagMap` warning on `cargo check -p oximedia --target wasm32-unknown-unknown --features ml` (`crates/oximedia-container/src/metadata/editor.rs:8`) — exit code 0, unrelated to Wave 1-6 ML work, tracked separately for a future sweep.

## [0.1.4] - 2026-04-20

### Added
- **MJPEG codec end-to-end wiring**: encoder, decoder, MP4/MOV sample entry (`jpeg` fourcc), Matroska `V_MJPEG` codec ID, proxy codec integration in `oximedia-multicam`, transcode dispatch in `oximedia-transcode`
- **APV codec end-to-end wiring**: encoder, decoder, MP4 sample entry (`apv1` fourcc), Matroska `V_MS/VFW/FOURCC` with BITMAPINFOHEADER CodecPrivate, compat-ffmpeg pass-through, transcode dispatch
- **AVI container (Wave 3)**: AVI v3 OpenDML support for files >1 GB; PCM audio muxing; H264/RGB24 codec arms in RIFF-AVI muxer (`mux/avi/writer.rs`) and demuxer (`demux/avi/reader.rs`); hdrl + movi + idx1 index
- **AJXL ISOBMFF animated container**: `AnimatedJxlEncoder::finish_isobmff()` emits spec-conformant `ftyp` + `jxll` + `jxlp*` box chain (ISO/IEC 18181-2); shared ISOBMFF helper module (`make_box`, `make_full_box`, `BoxIter<R: Read>`)
- **AJXL streaming decoder**: `JxlStreamingDecoder<R: Read>: Iterator<Item = CodecResult<JxlFrame>>` with auto-detection of ISOBMFF vs OxiMedia native format; lazy `jxlp` box parsing; memory-bounded (one frame in-flight)
- **`CodecId::FromStr` + `FourCc`**: 24-alias `FromStr` implementation and `canonical_name()` for all codec IDs; `FourCc` struct with 31 predefined constants in `oximedia-core` (`types/fourcc.rs`)
- **CLI MJPEG/APV support**: `VideoCodec::{Mjpeg, Apv}` variants with `is_intra_only()`, `default_crf()`, `validate_crf()`; intra-codec fast path in `TranscodePipeline`
- **WASM32 platform gating**: `oximedia-batch` and `oximedia-convert` `mio` dependency cfg-gated for WASM; `oximedia-gpu` and `oximedia-graphics` `GpuAccelerator` Send+Sync WASM cfg-gate; `oximedia-colormgmt`, `oximedia-workflow`, `oximedia-farm` also pass `cargo check --target wasm32-unknown-unknown` cleanly; tokio/tonic/rusqlite deps target-gated in `oximedia-farm`
- **MP4 muxer fragment modes (Wave 3)**: `Mp4FragmentMode` enum (Progressive/Fragmented); AV1 `av1C` config box emission; MJPEG/APV codec arms in MP4 sample entry
- **Matroska enhancements (Wave 3 + Wave 4)**: `seek_sample_accurate()` in Matroska demuxer; `preroll_samples`/`padding_samples` fields in MP4 elst box; `BlockAdditionMapping` support in MKV muxer
- **DASH/CMAF streaming (Wave 3 + Wave 4)**: DASH MPD manifest emitter (`dash/manifest.rs`); CMAF-LL chunked DASH MPD emitter for low-latency delivery; cross-format `seek_sample_accurate()` trait
- **FFmpeg compat extensions (Wave 3)**: `filter_complex.rs` — FilterGraph parser for `-filter_complex` arguments; `stream_spec.rs` — `StreamSelector` for FFmpeg stream specifiers; `seek.rs` — `parse_duration` for FFmpeg duration strings; `ffprobe_output.rs` — `FfprobeOutputFormat` output struct
- **FFmpeg compat quality flags (Wave 4)**: `OnceLock`-cached codec-map for zero-cost repeated lookups; `-crf`/`-b:v`/`-maxrate`/`-bufsize` arguments translated to `EncoderQuality`; `-vf`/`-af` filter chain parsing; two-pass encoding support (`-pass 1`/`-pass 2`)
- **APV codec aliases (Wave 3)**: APV codec aliases added to `codec_map.rs` and `codec_mapping.rs` in `oximedia-compat-ffmpeg`; 4 pre-existing failing tests fixed
- **Dolby Atmos channel layouts (Wave 4)**: `oximedia-core` gains 7.1.2, 5.1.4, 7.1.4, 9.1.6, and binaural Dolby Atmos channel layout variants
- **Color metadata types (Wave 4)**: `ColorPrimaries`, `TransferCharacteristics`, and `MatrixCoefficients` enums plus `ColorMetadata` struct added to `oximedia-core`
- **Timestamp arithmetic (Wave 4)**: arithmetic operator impls (`Add`, `Sub`, `Mul`, `Div`) on `Timestamp` in `oximedia-core`

### Fixed
- **JPEG encoder spec-compliance**: DQT table now serialized in zigzag order per JPEG spec; EOB marker emitted only when trailing-zero AC run exists; dequantization ordering corrected. MJPEG round-trip PSNR at Q85: 6.16 dB → 32.53 dB
- **Matroska MJPEG/APV codec IDs**: `codec_id_string` now returns `V_MJPEG` / `V_MS/VFW/FOURCC` instead of falling through to `V_UNCOMPRESSED`
- **MP4 muxer APV/MJPEG validation**: `validate_codec` now accepts royalty-free codecs APV and MJPEG; `codec_to_fourcc` maps them to `apv1`/`jpeg`

### Improved
- **87,387 tests passing** (up from 80,901 in Wave 3; 80,901 up from ~80,500 pre-Wave 3); zero clippy warnings
- **Docs sweep (Wave 3 + Wave 4)**: rustdoc updated for 10 crates (gpu, storage, routing, collab, presets, switcher, automation, core, codec, compat-ffmpeg) plus codec, io, and bitstream crates; 20 TODO markers resolved

## [0.1.3] - 2026-04-15

### Added
- `JobProgress` tracking in `oximedia-farm` job queue
- `bit_depth()` method on `SampleFormat` in `oximedia-core`
- `output_validator`, `worker_health`, `auto_scaler`, `cloud_storage` modules now public in `oximedia-farm`

### Fixed
- VU meter ballistics -Inf poisoning when processing zero-amplitude samples (`oximedia-audio`)
- Subtitle chain comma replacement corrupting subtitle text (`oximedia-convert`)
- ABR rate control overflow in lookahead multiplier calculation (`oximedia-codec`)
- Scene cut detection depth limit missing spikes beyond index 4 (`oximedia-codec`)
- EWA resampling weight table returning non-empty on zero source dimensions (`oximedia-scaling`)
- Audio codec validation rejecting patent-free codecs only (`oximedia-cli`)
- Module conflict between `processor.rs` and `processor/mod.rs` (`oximedia-image-transform`)
- Broken intra-doc links in `oximedia-routing`, `oximedia-server`, `oximedia-container`, `oximedia-neural`, `oximedia-review`, `oximedia-effects`
- Multiple clippy warnings across workspace

### Changed
- Replaced banned `lz4` dependency with `lz4_flex` in `oximedia-collab` and `oximedia-renderfarm`
- Replaced `zstd` with `lz4_flex` compression in `oximedia-renderfarm` storage
- Updated workspace metadata: authors, homepage fields standardized across all crates

### Improved
- 80,393 tests passing (up from 70,800+ in v0.1.2)
- Zero clippy warnings with `-D warnings`
- Clean rustdoc build with strict flags
- 2.65M SLOC across 106 crates

## [0.1.2] - 2026-03-16

### Added

#### New Crates (11)
- **oximedia-hdr** — HDR processing with PQ/HLG transfer functions, tone mapping, gamut mapping, HDR10+ SEI metadata, HLG advanced modes, color volume analysis, and Dolby Vision profile support.
- **oximedia-spatial** — Spatial audio engine with Higher-Order Ambisonics (HOA), HRTF binaural rendering, room simulation, VBAP panning, head tracking, Wave Field Synthesis, and object-based audio.
- **oximedia-cache** — Intelligent media caching with LRU eviction, tiered storage, predictive warming, Bloom filter membership, consistent hashing, ARC adaptive replacement, and content-aware policies.
- **oximedia-stream** — Adaptive streaming with BOLA ABR algorithm, segment lifecycle management, SCTE-35 ad signaling, multi-CDN failover, manifest builder, and stream packager.
- **oximedia-video** — Video processing toolkit with motion estimation, deinterlacing, frame interpolation, scene detection, pulldown removal, video fingerprinting, and temporal denoising.
- **oximedia-cdn** — Content delivery network management with edge node orchestration, cache invalidation, origin failover, geographic routing, and CDN performance metrics.
- **oximedia-neural** — Neural network inference for media with tensor operations, Conv2D layers, batch normalization, activation functions, and media-specific models (scene classifier).
- **oximedia-360** — 360-degree video processing with equirectangular-to-cubemap projection, fisheye correction, stereo 3D layout, and Google Spatial Media XMP metadata.
- **oximedia-analytics** — Media analytics with session tracking, retention curve analysis, A/B testing framework, and engagement scoring models.
- **oximedia-caption-gen** — Automatic caption generation with speech-to-text alignment, Knuth-Plass line breaking, WCAG 2.1 accessibility compliance, and speaker diarization.
- **oximedia-pipeline** — Declarative media processing DSL with typed filter graph construction, execution planning, and optimization passes.

#### Plugin System
- **oximedia-plugin** — SemVer dependency resolver, u32 bitmask capability sandbox, FNV-1a hash-based hot-reload for dynamic codec plugins at runtime.

#### Broadcast and Routing
- **NMOS IS-04/05/07/08/09/11 REST APIs** in `oximedia-routing` with full device discovery, connection management, event and tally, audio channel mapping, stream compatibility, and system API support (656 tests).
- **NMOS mDNS/DNS-SD discovery** for automatic service registration and browsing (605 tests).

#### CLI Extensions
- Loudness analysis and normalization commands.
- Quality assessment (VMAF/SSIM/PSNR) commands.
- Deduplication detection commands.
- Timecode conversion and arithmetic commands.
- Batch engine commands for job scheduling.
- Scopes rendering (waveform/vectorscope/histogram) commands.
- Workflow template execution commands.
- Version info command (333 tests across CLI).

#### Benchmarks and Testing
- 4 criterion benchmark suites in `benches/` crate for codec, filter, I/O, and pipeline performance regression testing.
- 9 new examples demonstrating common workflows.
- 51 integration tests in `oximedia/tests/integration.rs`.
- 70,800+ tests passing across the entire workspace.

#### WASM and Python
- WASM target `wasm32-unknown-unknown` now builds cleanly with all feature gates (505 tests pass).
- PyPI publish workflow fixed (maturin 1.8.4, corrected protoc URL, macOS Intel runner).

### Changed

#### Major Crate Enhancements (40+)

- **oximedia-normalize** — DisneyPlus, PrimeVideo, Apple Spatial Audio, and Dolby Atmos loudness standards; adaptive scene-based normalization; multiband IIR filtering.
- **oximedia-server** — Admin API endpoints, Prometheus `/metrics` endpoint with AtomicU64 counters, HMAC webhook signing, batch delete and batch transcode operations.
- **oximedia-playout** — Transitions (dissolve, wipe, dip-to-color), CEA-608/708 subtitle insertion into playout streams, pre-flight validation checks, MultiChannelScheduler for parallel channel playout.
- **oximedia-net** — Low-Latency HLS (RFC 8216bis) with partial segments and preload hints, XOR FEC (RFC 5109) for packet recovery, QUIC transport abstraction layer.
- **oximedia-mam** — Pub/sub EventBus for asset lifecycle events, rule-based AI auto-tagger, BM25+Jaccard smart search with relevance ranking.
- **oximedia-batch** — Priority-heap job queue, conditional DAG execution (OnSuccess/OnFailure/Threshold branches), timeout enforcer with graceful cancellation.
- **oximedia-graphics** — HDR compositor with 16 blend modes, 1D/3D LUT application with Adobe .cube parser, ASC CDL color grading with slope/offset/power/saturation.
- **oximedia-workflow** — 8 pipeline templates with DOT graph export, StepCondition evaluator for conditional branching, p95 latency metrics tracking.
- **oximedia-monitor** — Alerting rules engine (Threshold, RateOfChange, Absence detection), LTTB downsampling with EWMA time-series smoothing, health registry with dependency checks.
- **oximedia-archive** — LZ77+LZ4 streaming compressor, pure-Rust SHA-256 digest verification, split/reassemble OARC format for large media archives.
- **oximedia-farm** — 6 load-balancing strategies (round-robin, least-connections, weighted, random, hash, power-of-two), locality-aware job distribution, heartbeat-based worker pool management.
- **oximedia-scopes** — False color overlay (7 exposure zones), 3D RGB histogram visualization, 5-mode exposure metering (spot, center-weighted, matrix, highlight, shadow).
- **oximedia-subtitle** — SRT/VTT/ASS/TTML parsers and serializers, 8x12 bitmap burn-in renderer, timing adjuster with offset and stretch.
- **oximedia-effects** — Freeverb and convolution reverb, multi-voice chorus and flanger, 7 distortion algorithms (overdrive, fuzz, bitcrush, wavefold, tube, tape, digital clip).
- **oximedia-mixer** — Topology-sorted mixing bus graph, 8-band parametric EQ with biquad filters, DAW-style automation lanes with interpolation.
- **oximedia-drm** — AES-128/256 implementation from scratch (NIST FIPS 197 verified), content key lifecycle management, license server with region-based gating.
- **oximedia-gpu** — RGBA-to-YUV420 and YUV420-to-RGBA conversion kernels, Gaussian/Sobel/Otsu image processing, buffer pool allocator, pipeline stage chaining.
- **oximedia-rights** — Royalty calculation engine (6 revenue bases), clearance workflow with counter-offer/region/time constraints, ISRC/ISWC/ISAN identifier validation.
- **oximedia-virtual** — LED volume stage simulation with moire pattern checker, FreeD D1 camera tracking protocol, frustum culling with 6-plane extraction.
- **oximedia-io** — 42-variant magic-byte content detector, Boyer-Moore-Horspool optimized reader, MP4/FLAC/WAV/MKV probe implementations.
- **oximedia-mir** — Beat tracking with dynamic programming, mood detection on Russell circumplex model, Camelot harmonic mixing codes (607+ tests).
- **oximedia-colormgmt** — Rec.709/Rec.2020/DCI-P3 gamut mapping, Bradford chromatic adaptation, CIECAM02 full forward/inverse transform, CIEDE2000 with RT rotation term, median-cut/k-means/octree palette quantization.
- **oximedia-cv** — SORT multi-object tracker, pyramidal Lucas-Kanade optical flow (831+302 tests).
- **oximedia-shots** — Audio scene boundary detection via spectral flux analysis, flash detection and Harding PSE compliance checker.
- **oximedia-recommend** — ALS and SVD++ collaborative filtering for encoding parameter recommendation.
- **oximedia-quality** — Temporal quality analyzer for frame-over-frame drift, pipeline quality gate with broadcast/streaming/preview threshold presets.
- **oximedia-codec** — VBV-aware rate control, AV1 level constraint table, PacketReorderer for B-frame output ordering.
- **oximedia-audio** — YIN pitch detection (4 algorithm variants), Kaiser-windowed sinc resampler, EBU R128 K-weighted loudness gating.
- **oximedia-image** — 2D DFT with Butterworth frequency-domain filters, 7 morphological operations with union-find connected components, Non-Local Means denoising.
- **oximedia-simd** — AVX-512 SIMD kernels with runtime CPU feature detection (`CpuFeatures` dispatcher).
- **oximedia-transcode** — 9 platform presets (YouTube, Netflix, Twitch, Vimeo, Instagram, TikTok, Broadcast, Archive, Web), VP9 CRF encoding, FFV1 lossless archive mode, TranscodeEstimator for time/size prediction, per-scene CRF adaptation, 6-rung quality ladder, HW acceleration config, Prometheus metrics export.
- **oximedia-dedup** — Perceptual hash (pHash), SSIM structural similarity, histogram comparison, feature-based matching, audio fingerprint dedup, metadata-based dedup (404 tests).
- **oximedia-search** — Real facet aggregation across 7 dimensions (codec, resolution, duration, format, date, tags, status) with 444 tests.
- **oximedia-core** — RationalTime with GCD/LCM arithmetic, PtsMediaTime 128-bit rebase for sub-sample precision, RingBuffer and MediaFrameQueue lock-free structures.
- **oximedia-lut** — Hald CLUT (identity generation + trilinear interpolation), 12 photographic presets (portra, velvia, tri-x, etc.), LutChainOps bake-to-33-cubed optimization.
- **oximedia-compat-ffmpeg** — 19-node FilterGraph parser, 75 codec and 30 format mappings, FfmpegArgumentBuilder for programmatic CLI construction.
- **oximedia-scaling** — EWA Lanczos elliptical weighted average resampling, FidelityFX CAS sharpening, half-pixel correction for chroma, per-title encoding ladder generator.
- **oximedia-auto** — Narrative arc detection (3-Act, Hero's Journey, Kishotenketsu), beat-synced automatic cuts, saliency-based reframing for aspect ratio adaptation.
- **oximedia-dolbyvision** — IPT-PQ color space transforms, CM v4.0 trim metadata with sloped curves, quickselect-based shot statistics, Dolby Vision XML import/export.
- **oximedia-collab** — Three-way merge for concurrent edits, Operational Transform primitives, presence and cursor tracking, snapshot-based branching.
- **oximedia-plugin** — SemVer dependency resolver, u32 bitmask capability sandbox, FNV-1a hash-based hot-reload detection.

### Fixed

- Facade crate (`oximedia`) now correctly re-exports all 108 crates with proper feature gating.
- WASM build target resolves all feature-gate incompatibilities for browser environments.
- PyPI publish workflow corrected for maturin 1.8.4, protoc binary URL, and macOS Intel runner matrix.

## [0.1.1] - 2026-03-10

### Added

- **FFmpeg CLI compatibility layer** — `oximedia-compat-ffmpeg` crate and `oximedia-ff` binary providing drop-in argument compatibility with FFmpeg CLI for common transcoding, streaming, and filter workflows.
- **OpenCV Python API compatibility** — `oximedia.cv2` submodule in `oximedia-py` exposing 18 modules aligned to the OpenCV Python API surface (imread, imwrite, resize, cvtColor, VideoCapture, VideoWriter, etc.).
- **MP4 demuxer complete implementation** — `probe` and `read_packet` fully implemented in `oximedia-container`, enabling reliable MP4/MOV source reading in transcode pipelines.
- **Transcode pipeline implementation** — end-to-end demux→filter→encode→mux pipeline in `oximedia-transcode`, connecting all processing stages with backpressure and async task scheduling.
- **Archive checksum real hash verification** — `oximedia-archive` now performs actual MD5, SHA-1, SHA-256, and xxHash digest verification (replacing placeholder stubs).
- **QR code watermarking** — ISO 18004 compliant QR code generation and embedding in `oximedia-watermark`, supporting data capacity modes 1–4 with Reed-Solomon error correction.
- **DCT-domain forensic watermarking** — Quantization Index Modulation (QIM) embedding and blind detection in `oximedia-watermark`, providing robust invisible watermarks surviving re-encoding.
- **Video deinterlacing** — Edge-Directed Interpolation (EDI) deinterlacer added to `oximedia-cv`, including bob, weave, and blend fallback modes.
- **Smart crop** — content-aware crop detection using saliency maps and face-priority weighting in `oximedia-cv`.
- **Super-resolution (EDI)** — single-frame and multi-frame SR upscaling in `oximedia-cv` via learned edge-directed interpolation.
- **GCS storage enhancements** — ACL management, signed URL generation (V4), CMEK encryption key association, and storage class transitions in `oximedia-cloud`.
- **NMF source separation** — Non-negative Matrix Factorisation based audio source separation in `oximedia-audio-analysis`.
- **CEA-608 subtitle parser** — Line 21 closed caption byte-pair decoding in `oximedia-subtitle`.
- **DVB subtitle parser** — ETSI EN 300 743 PES/segment parsing in `oximedia-subtitle`.
- **Plugin system** — `oximedia-plugin` crate providing `CodecPlugin` trait, `PluginRegistry`, `StaticPlugin` builder, `declare_plugin!` macro, JSON manifests, and `dynamic-loading` feature gate for shared library support.
- **FFV1 codec** — Lossless video codec (decoder + encoder) in `oximedia-codec` with range coder, Golomb-Rice coding, and multi-plane support.
- **Y4M container** — Raw YUV sequence format (demuxer + muxer) in `oximedia-container` for uncompressed video interchange.
- **JPEG-XL codec** — Next-generation image codec (decoder + encoder) in `oximedia-codec` with modular transform, entropy coding, and progressive decoding.
- **DNG image format** — Digital Negative RAW image support (reader + writer) in `oximedia-image` with TIFF/IFD parsing, CFA demosaicing, and color calibration.

### Changed

- Refactored 6 over-limit source files (super_resolution, denoise, grading, lut, delogo, ivtc) — each split below the 2000-line policy boundary using splitrs.
- Promoted 22 Alpha crates and 10 Partial crates to fuller implementation status.

## [0.1.0] - 2026-03-07

### Added

- Initial release of the oximedia workspace — a comprehensive professional media processing platform in pure Rust.

#### Core Infrastructure
- `oximedia-core` — foundational types, error handling, and shared abstractions for the entire workspace
- `oximedia-io` — unified I/O layer with async file and stream support
- `oximedia-codec` — audio/video codec abstractions and implementations
- `oximedia-container` — media container format support (MXF, MP4, MOV, MPEG-TS, MKV, etc.)
- `oximedia-simd` — SIMD-accelerated media processing primitives
- `oximedia-accel` — hardware acceleration abstractions (GPU, FPGA, DSP)
- `oximedia-gpu` — GPU compute pipelines for media processing

#### Audio Processing
- `oximedia-audio` — core audio processing primitives and pipelines
- `oximedia-audio-analysis` — audio analysis including rhythm, tempo, and spectral features
- `oximedia-audiopost` — post-production audio tools (mixing, mastering, restoration)
- `oximedia-effects` — audio effects processing (chorus, reverb, EQ, dynamics)
- `oximedia-metering` — broadcast-grade audio metering (LUFS, LRA, peak, PPM)
- `oximedia-mixer` — multi-channel audio mixing and routing
- `oximedia-normalize` — audio normalization to broadcast standards
- `oximedia-mir` — music information retrieval and audio fingerprinting (AcoustID)

#### Video Processing
- `oximedia-cv` — computer vision and image analysis with super-resolution support
- `oximedia-vfx` — visual effects compositing and processing
- `oximedia-image` — image processing and format conversion
- `oximedia-lut` — LUT (Look-Up Table) processing for color grading
- `oximedia-colormgmt` — ICC color management and color space conversion
- `oximedia-dolbyvision` — Dolby Vision HDR metadata processing
- `oximedia-scopes` — broadcast video scopes (waveform, vectorscope, histogram)
- `oximedia-denoise` — video and audio denoising algorithms
- `oximedia-stabilize` — video stabilization
- `oximedia-scaling` — high-quality video scaling and resizing
- `oximedia-watermark` — digital watermarking

#### Graph and Pipeline
- `oximedia-graph` — media processing graph/pipeline engine
- `oximedia-edit` — non-linear editing operations
- `oximedia-timeline` — timeline management and sequencing
- `oximedia-timecode` — SMPTE timecode parsing, generation, and arithmetic
- `oximedia-timesync` — clock synchronization and PTP/NTP support
- `oximedia-clips` — clip management and media bin
- `oximedia-shots` — shot detection and scene segmentation
- `oximedia-scene` — scene analysis and classification

#### Transcoding and Conversion
- `oximedia-transcode` — multi-format transcoding pipeline
- `oximedia-convert` — universal media format conversion
- `oximedia-packager` — DASH/HLS adaptive streaming packaging
- `oximedia-proxy` — proxy media generation and management
- `oximedia-optimize` — media optimization for delivery targets
- `oximedia-batch` — batch processing job management
- `oximedia-renderfarm` — distributed render farm coordination

#### Distributed and Cloud
- `oximedia-distributed` — distributed encoding coordinator with consensus, leader election, and work stealing
- `oximedia-farm` — production-grade encoding farm job management and worker coordination
- `oximedia-jobs` — job scheduling and queue management
- `oximedia-cloud` — cloud storage and processing integration
- `oximedia-storage` — cloud storage abstraction (S3, Azure Blob, Google Cloud Storage)
- `oximedia-workflow` — media workflow automation and orchestration
- `oximedia-automation` — event-driven automation and rules engine

#### Networking
- `oximedia-net` — network transport protocols for media (RTP, RTMP, SRT, RIST)
- `oximedia-ndi` — NDI (Network Device Interface) protocol support
- `oximedia-server` — media server with WebSocket and HTTP APIs
- `oximedia-videoip` — video-over-IP transport (ST 2110, ST 2022)
- `oximedia-routing` — software-defined media routing and signal routing
- `oximedia-switcher` — live production switcher functionality
- `oximedia-playout` — broadcast playout automation

#### Quality and Analysis
- `oximedia-qc` — automated quality control and validation
- `oximedia-quality` — perceptual quality metrics (VMAF, SSIM, PSNR)
- `oximedia-analysis` — comprehensive media analysis and reporting
- `oximedia-monitor` — real-time media monitoring and alerting
- `oximedia-forensics` — media forensics and chain-of-custody tools
- `oximedia-dedup` — media deduplication and similarity detection
- `oximedia-profiler` — GPU and CPU profiling for media workloads

#### Metadata and Rights
- `oximedia-metadata` — media metadata extraction, editing, and standards (XMP, ID3, etc.)
- `oximedia-rights` — digital rights management metadata
- `oximedia-drm` — DRM encryption and key management
- `oximedia-access` — accessibility features (audio description generation)
- `oximedia-captions` — caption and subtitle processing
- `oximedia-subtitle` — subtitle format parsing and conversion

#### Format-Specific
- `oximedia-aaf` — AAF (Advanced Authoring Format) support
- `oximedia-edl` — EDL (Edit Decision List) parsing and generation
- `oximedia-imf` — IMF (Interoperable Master Format) support
- `oximedia-lut` — LUT format support (cube, 3dl, etc.)

#### Advanced Features
- `oximedia-align` — audio/video alignment and synchronization
- `oximedia-calibrate` — camera and display calibration tools
- `oximedia-collab` — collaborative editing and review workflows
- `oximedia-conform` — media conform and EDL-to-media matching
- `oximedia-gaming` — game capture and streaming integration
- `oximedia-graphics` — graphics overlay and titling
- `oximedia-mam` — Media Asset Management integration
- `oximedia-multicam` — multi-camera editing and synchronization
- `oximedia-playlist` — playlist management and scheduling
- `oximedia-presets` — encoding and processing preset management
- `oximedia-recommend` — AI-powered encoding parameter recommendation
- `oximedia-repair` — media repair and error concealment
- `oximedia-restore` — media restoration and archival tools
- `oximedia-review` — collaborative review and approval workflows
- `oximedia-search` — full-text and semantic media search
- `oximedia-virtual` — virtual production tools
- `oximedia-archive` — media archiving and long-term preservation
- `oximedia-archive-pro` — advanced archival formats and migration

#### Tooling
- `oximedia-bench` — benchmarking harnesses for media processing
- `oximedia-py` — Python bindings via PyO3
- `oximedia-wasm` — WebAssembly bindings
- `oximedia-cli` — command-line interface

[Unreleased]: https://github.com/cool-japan/oximedia/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/cool-japan/oximedia/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/cool-japan/oximedia/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/cool-japan/oximedia/releases/tag/v0.1.0
