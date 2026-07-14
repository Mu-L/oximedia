# OxiMedia WASM — Development Roadmap

**Version: 0.1.9**
**Status: 76 modules wired into `lib.rs` and exported; 3 dishonest decoder
classes removed from the public surface, plus a 4th dishonest decode path
found and fixed inside `media_player` (see "Removed" / "AV1 decode audit"
below).**

## Data-plane policy

Buffer-crossing `#[wasm_bindgen]` APIs use `Uint8Array`/`Uint8ClampedArray`
(8-bit SDR) or `Float32Array` (HDR/linear) exclusively — never
`Float64Array`. Per-packet/per-frame hot-path methods return typed structs
with getters, never JSON strings (JSON remains fine for one-shot
configuration/listing calls). All buffer-crossing APIs in this crate
conform to this policy as of 0.1.9.

## Implemented Modules

### Core decoders and muxers
- [x] `audio_decoder` — FLAC, Opus decoders (real; Vorbis removed, see "Removed")
- [x] `video_encoder` — VP8 encoder
- [x] `demuxer` — WebM/Matroska/Ogg/FLAC/WAV demuxer
- [x] `muxer` — WebM muxer
- [x] `streaming_demuxer` — streaming demux
- [x] `container` — container format helpers
- [x] `probe` — magic-byte format detection
- [x] `io` — I/O utilities
- [x] `media_player` — self-contained player (format probe/demux honest;
      `next_frame()` now errors on AV1 instead of the internal decode this
      line used to claim — see "AV1 decode audit" below)

### Analysis and quality
- [x] `analysis` — loudness (EBU R128), beat detection, spectral features
- [x] `quality_wasm` — PSNR, SSIM, frame quality
- [x] `scopes_wasm` — waveform, vectorscope, false color

### Color management
- [x] `colormgmt_wasm` — color space conversion, tone mapping, delta-E
      (`Float32Array` + `Uint8Array` `_u8` companions since 0.1.9; was `f64` before)
- [x] `hdr_wasm` — PQ/HLG transfer functions, HDR tone mapping — wired into
      `lib.rs` in 0.1.9 (previously implemented but orphaned: not declared as
      a module, so none of it was reachable or compiled into the crate).
      Buffer APIs converted `f64` → `f32` at the same time.
- [x] `lut_wasm` — 3D LUT application, photographic presets, .cube parser —
      wired into `lib.rs` in 0.1.9 (previously orphaned, same as `hdr_wasm`).
      Buffer APIs were already `f32`.
- [x] `dolbyvision_wasm` — Dolby Vision metadata
- [x] `calibrate_wasm` — color calibration

### Audio
- [x] `convert` — sample format and sample rate conversion
- [x] `convert_wasm` — format/codec conversion helpers
- [x] `mixer_wasm` — audio mixing, gain, pan
- [x] `mir_wasm` — beat/tempo/chord/key detection
- [x] `normalize_wasm` — loudness normalization with EBU R128 targets
- [x] `restore_wasm` — audio restoration, de-clip
- [x] `spatial_wasm` — Ambisonics (HOA), VBAP panning — wired into `lib.rs`
      in 0.1.9 (previously orphaned, same as `hdr_wasm`)
- [x] `audiopost_wasm` — stems, mix, delivery spec
- [x] `denoise_wasm` — audio/video denoising

### Graphics and compositing
- [x] `graphics_wasm` — broadcast graphics, templates
- [x] `vfx_wasm` — effects, chroma key, transitions
- [x] `image_wasm` — image ops, DPX/EXR, histograms
- [x] `multicam_wasm` — multi-camera compositing
- [x] `scaling_wasm` — video/image scaling
- [x] `filter_graph` — filter graph (DAG)

### Metadata and subtitles
- [x] `metadata_wasm` — ID3v2, Vorbis comments, EXIF, iTunes, Matroska tags
- [x] `subtitle_wasm` — SRT/VTT/ASS parsing and conversion
- [x] `captions_wasm` — captions processing
- [x] `timecode_wasm` — SMPTE timecode operations

### Production and workflow
- [x] `transcode_wasm` — transcoding presets and job management
- [x] `batch_wasm` — batch processing
- [x] `workflow_wasm` — workflow orchestration
- [x] `playout_wasm` — broadcast playout schedule
- [x] `timeline_wasm` — timeline editing
- [x] `scene_wasm` — scene detection
- [x] `shots_wasm` — shot cut/dissolve/fade detection

### Infrastructure
- [x] `worker_helpers` — transfer header, plane splitting, transferable frames
- [x] `webcodecs_bridge` — WebCodecs API bridge; `get_video_decoder_config`
      and `oximedia_packet_to_encoded_chunk` return typed
      `WasmVideoDecoderConfig` / `WasmEncodedChunkInfo` structs (0.1.9;
      previously hand-formatted JSON strings on a per-packet hot path)
- [x] `types` — shared types (WasmPacket, WasmStreamInfo, etc.)
- [x] `utils` — error helpers
- [x] `plugin_wasm` — plugin system info
- [x] `cache_wasm` — media cache management
- [x] `analytics_wasm` — session tracking, A/B testing
- [x] `neural_wasm` — in-browser ML inference (pending WASM SIMD performance)
- [x] `stream_wasm` — ABR streaming manifest builder

### Professional tools
- [x] `drm_wasm` — DRM encrypt/decrypt
- [x] `forensics_wasm` — image forensics (ELA, noise, compression)
- [x] `watermark_wasm` — audio/image watermarking
- [x] `dedup_wasm` — media deduplication
- [x] `rights_wasm` — digital rights checking
- [x] `qc_wasm` — quality control (no file-system paths; in-memory only)
- [x] `review_wasm` — review and approval workflows
- [x] `collab_wasm` — collaborative editing
- [x] `monitor_wasm` — system monitoring
- [x] `profiler_wasm` — performance profiling

### Other
- [x] `aaf_wasm` — AAF-shaped structure info for the browser (hand-rolled;
      does not link against `oximedia-aaf` — see "Dependency hygiene" below)
- [x] `access_wasm` — access control
- [x] `align_wasm` — media alignment
- [x] `archivepro_wasm` — professional archiving
- [x] `auto_wasm` — automated editing
- [x] `clips_wasm` — clip management
- [x] `conform_wasm` — delivery conformance
- [x] `gaming_wasm` — game capture/streaming
- [x] `imf_wasm` — IMF-shaped structure info for the browser (hand-rolled;
      does not link against `oximedia-imf` — see "Dependency hygiene" below)
- [x] `presets_wasm` — encoding presets
- [x] `proxy_wasm` — proxy media
- [x] `recommend_wasm` — content recommendation
- [x] `renderfarm_wasm` — render farm
- [x] `routing_wasm` — audio/video routing
- [x] `stabilize_wasm` — hand-rolled video stabilization (does not link
      against `oximedia-stabilize` — see "Dependency hygiene" below)
- [x] `switcher_wasm` — live production switching
- [x] `timesync_wasm` — time synchronization
- [x] `virtual_wasm` — virtual production

## Removed (0.1.9): dishonest decoders

Three decoder classes were removed entirely from the WASM surface because
their implementations did not do what their names promised:

- [x] ~~`video_decoder` — `WasmVp8Decoder`~~ removed: the underlying VP8
  decoder returned `Err` on every `decode_frame()` call.
- [x] ~~`av1_decoder` — `WasmAv1Decoder`~~ removed: returned `Ok` with
  YUV buffers that were never actually populated with decoded pixels.
- [x] `audio_decoder::WasmVorbisDecoder` removed: only round-trips its own
  synthetic test format, not real Ogg Vorbis bitstreams.

Native VP8/AV1 decoders in `crates/oximedia-codec` and Vorbis in
`crates/oximedia-audio` are untouched (this was a WASM-surface-only change);
the corresponding Cargo features (`vp8` on `oximedia-codec`, `vorbis` on
`oximedia-audio`) were dropped from this crate's `Cargo.toml` since nothing
here uses them anymore.

## AV1 decode audit (0.1.9, later pass): `media_player` had the same bug

This "Removed" section previously claimed `WasmMediaPlayer` "still performs
real AV1 decode internally," as a contrast with the removed `WasmAv1Decoder`.
That claim was false — verified by reading `crates/oximedia-codec`'s
`Av1Decoder` end to end:

- `Av1Decoder::send_packet` → `decode_temporal_unit` parses OBU/frame headers
  and pushes a `VideoFrame::allocate()`d buffer (all-zero planes) onto its
  output queue. The `TileGroup` OBU case is a no-op (see the comment at the
  call site: "Tile group data would be processed here. For now, we've
  already created the frame in FrameHeader handling").
- `decode_frame_with_pipeline` / `decode_tiles` / `decode_superblocks` /
  `decode_block` / `decode_block_coefficients` — the real tile/coefficient/
  reconstruction pipeline — exist in the same file but are **never called**
  from `send_packet`'s path (dead code; `#![allow(dead_code)]` at the top of
  the module is why this compiled warning-free).
- `receive_frame()` therefore returns `Ok(Some(frame))` with unpopulated
  (solid-black) pixel data — bit-for-bit the same defect `WasmAv1Decoder` was
  removed for.

`oximedia-wasm/src/media_player.rs` wrapped this same `Av1Decoder` and
returned its fake frames to JS as `Ok(Some(yuv_buffer))`, unlike the direct
`WasmAv1Decoder` binding which had already been removed. Fixed to match:
the AV1 branch (the `av1_decoder` field, its construction, and the
`decode_frame_with_pipeline`-adjacent copy path) was removed from
`WasmMediaPlayer`; `next_frame()` now returns
`Err("AV1 decode is not supported in the browser build; use WebCodecs
VideoDecoder")` for an AV1-coded video track instead of a frame. Format
probing/`media_info()` is unaffected (still assumes AV1 for Matroska/MP4 per
the existing "common profile" heuristic — that's a probing simplification,
not a decode claim). The now-unreachable `PlayerState::Playing` state (only
ever set by the fake AV1 decode success path) was removed from the state
machine along with it. A regression test
(`test_next_frame_av1_track_errors`) asserts `next_frame()` errors rather
than returning a frame for an AV1 track.

## Dependency hygiene (0.1.9)

- [x] Removed unused `oximedia-stabilize`, `oximedia-imf`, `oximedia-aaf`,
  `oximedia-analytics` dependencies from `Cargo.toml`. Their corresponding
  `*_wasm.rs` modules (`stabilize_wasm`, `imf_wasm`, `aaf_wasm`,
  `analytics_wasm`) are hand-rolled browser-side implementations that never
  called into those crates (verified via `oximedia_stabilize::` /
  `oximedia_imf::` / `oximedia_aaf::` / `oximedia_analytics::` grep — zero
  hits in `src/`). The modules themselves are unchanged and still exported.
- [x] Removed a vestigial `format!("/tmp/oximedia_qc_wasm_{}.bin", ...)` dead
  path construction in `qc_wasm.rs` (WASM has no file system; the value was
  never used for I/O).
- [x] `[package.metadata.wasm-pack.profile.release] wasm-opt` changed from
  `false` to `["-Oz"]` (was shipping unoptimized release `.wasm`).

## npm packaging honesty (0.1.9)

- [x] `@cooljapan/oximedia` (bundler target of *this* crate) is the only npm
  package this crate publishes. `build.sh`/`build-dev.sh`/`npm-publish.yml`
  no longer rename `pkg-web`/`pkg-node` to `@cooljapan/oximedia` — they keep
  wasm-pack's default scoped name (`@cooljapan/oximedia-wasm`) and are
  documented as unpublished build artifacts.
- [x] README no longer advertises `@cooljapan/oximedia-web` /
  `@cooljapan/oximedia-node` as installable packages of this crate.
  `@cooljapan/oximedia-web` now belongs to the separate `/web` package
  (four WebCodecs-adjacent modules, independently versioned/published);
  README points browser users there.

## Pending Modules (future work)

- [ ] `hdr_wasm` extensions — HDR scene analysis, CUVA/VIVID metadata
- [ ] `lut_wasm` extensions — ACES pipeline, Hald CLUT round-trip
- [ ] `spatial_wasm` extensions — HRTF binaural rendering, room simulation
- [ ] Real VP8/AV1/Vorbis WASM decoders, if/when the underlying
  `crates/oximedia-codec` (VP8/AV1) and `crates/oximedia-audio` (Vorbis)
  decoders are made to actually decode real bitstreams end-to-end. Do not
  re-add `WasmVp8Decoder`/`WasmAv1Decoder`/`WasmVorbisDecoder` until then.

## 0.1.2 Changes (historical)

| Item | Status |
|------|--------|
| `hdr_wasm`: PQ/HLG OETF/EOTF, batch frame conversion, tone mapping, `WasmHdrConverter` implemented | ✅ Done (module itself was orphaned/unreachable until 0.1.9 — see above) |
| `lut_wasm`: photographic presets, identity LUT, `WasmLut3d`, `.cube` parser implemented | ✅ Done (module itself was orphaned/unreachable until 0.1.9 — see above) |
| `spatial_wasm`: Ambisonics encode/decode (1st–5th order), VBAP panning, `WasmAmbisonicsEncoder` implemented | ✅ Done (module itself was orphaned/unreachable until 0.1.9 — see above) |
| All three modules: 8+ tests each, 0 clippy warnings | ✅ Done |
