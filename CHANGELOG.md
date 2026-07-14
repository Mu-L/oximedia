# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.9] - 2026-07-14

This is a production-readiness release: the default build is now **100% Pure
Rust** end to end (verified — `aws-lc-sys`, `libsqlite3-sys`, `mlua-sys`,
`shaderc-sys`, `zstd-sys`, `openssl-sys`, `rustfft`, and `realfft` are all
absent from the default dependency graph), four real security vulnerabilities
(cryptographic, denial-of-service, and SQL injection) inherited from earlier
"demonstration" code were fixed, the algorithmic hardening from development
Waves 21–30 lands for general use, and
a new **`oximedia-web`** npm package brings four browser WebAssembly modules
(scopes, colour, scale, quality) downstream of WebCodecs for the first time.

### Security
- **SRT payload encryption** (`oximedia-net::srt::crypto`): replaced a
  homebrew XOR/byte-mixing "cipher" that provided no real confidentiality
  with genuine **AES-128/192/256-CTR** (NIST SP 800-38A) via the vetted
  RustCrypto `aes` + `ctr` crates. Key derivation replaced a toy
  hash-and-extend construction with real **PBKDF2-HMAC-SHA256** (RFC 8018),
  and salts now come from the process CSPRNG (`rand`) instead of a
  timestamp-seeded LCG.
- **HLS/DASH packager encryption key generation** (`oximedia-packager::encryption::KeyGenerator`):
  `generate_aes128_key()`/`generate_iv()` now draw from the OS CSPRNG
  (`rand::rngs::SysRng`) instead of deriving "random" bytes from
  `SystemTime::now()`; `from_passphrase()` now uses real
  PBKDF2-HMAC-SHA256 (100,000 rounds, salted) instead of a bare
  `DefaultHasher` (SipHash, not designed for password hashing). Both
  key-generation methods are now fallible (`PackagerResult<Vec<u8>>`) so a
  CSPRNG failure is surfaced instead of silently producing predictable keys.
- **MP4 sample-table parsing** (`oximedia-container::demux::mp4::boxes`):
  `stts`/`stsc`/`stsz`/`stco`/`co64`/`stss`/`ctts` box parsers now validate
  that the attacker-controlled 32-bit `entry_count`/`sample_count` field can
  actually fit within the bytes remaining in the box *before* calling
  `Vec::with_capacity`, closing a memory-exhaustion DoS where a ~20-byte
  crafted file could trigger a multi-gigabyte allocation attempt.
- **WebRTC DTLS-SRTP honestly demoted to Experimental** (`oximedia-net::webrtc::dtls`):
  the module previously returned a "successful" handshake with an all-zero
  SRTP master key/salt, silently transmitting media in plaintext under a
  DTLS-protected label. `DtlsEndpoint::handshake()` and
  `DtlsConnection::send`/`recv` now refuse (return an error) instead of
  fabricating a connection, since no real DTLS 1.2 handshake or RFC 5764
  SRTP key export is implemented yet. SDP fingerprint generation and
  signaling are unaffected. **Do not use WebRTC media transport for
  confidential media until this lands.**
- **SQL injection in `oximedia-mam` smart-collection queries**
  (`oximedia-mam::collection::CollectionManager::build_condition_sql`/
  `execute_smart_query`): user-authored smart-collection filter conditions
  (`QueryCondition::field`/`::value`, stored as JSON and replayed every time
  the collection is viewed) were spliced directly into SQL text — `field`
  was interpolated into the query with no validation at all (letting a
  stored collection inject an arbitrary column or expression), and `value`
  was embedded inside hand-rolled `'...'` string literals for the
  `Equals`/`NotEquals`/`Contains`/`StartsWith`/`EndsWith` operators with no
  escaping (a `'` in the value could break out of the literal). Fixed with a
  new `ALLOWED_ASSET_COLUMNS` allowlist validating every user-supplied
  `field`/`sort_by` identifier before interpolation (identifiers can never be
  SQL bind parameters) and a `BindValue` enum that routes every condition
  value through `sqlx`'s real parameter binding instead of string
  formatting; surfaced while auditing call sites for sqlx 0.9's
  `SqlSafeStr`/`AssertSqlSafe` gate.
- `cargo-audit` advisories re-triaged for the new dependency set:
  `RUSTSEC-2026-0049` (rustls-rustcrypto → rustls-webpki 0.102.8, reachable
  only via oximedia-drm's non-default widevine/playready/fairplay features),
  `RUSTSEC-2026-0174` (azure_core → http-types notice, no user-controlled
  input reaches the affected constructors), and `RUSTSEC-2026-0192`
  (unmaintained `ttf-parser` via fontdue/usvg font-rasterization chains,
  application-supplied font assets only) documented in `audit.toml` with
  unreachability rationale; the stale `RUSTSEC-2026-0002` (tantivy/ratatui →
  `lru` unsound `IterMut`) ignore entry was removed now that it no longer
  applies.
- `cargo-audit`: `RUSTSEC-2026-0206` (`rustybuzz` unmaintained-crate notice,
  not a scored vulnerability) added to `.cargo/audit.toml`'s ignore list with
  documented unreachability rationale — reachable only via font-shaping/
  rendering paths operating on application-supplied font assets (subtitle
  rendering, SVG overlays), never attacker-controlled network input.

### Changed
- **Default build is now 100% Pure Rust.** All C/C++/Fortran dependencies
  that used to be compiled unconditionally are now behind non-default,
  opt-in Cargo features:
  - `oximedia-server`/`oximedia-rights`: SQLite storage migrated from `sqlx`
    (libsqlite3-sys, C) to **oxisql-sqlite-compat** (Pure Rust), via a
    small `sqlx`-API-shaped compat shim so existing call sites needed
    only an import-path change.
  - `oximedia-accel`: real Vulkan compute (`vulkano`/`vulkano-shaders`, which
    pull the shaderc/glslang C++ toolchain) gated behind the new
    `vulkan-backend` feature (`vulkan-detect` now implies it); Pure-Rust CPU
    fallback (and optional `webgpu`) is used by default.
  - `oximedia-automation`: Lua scripting (`mlua`, which vendors the Lua 5.4
    C interpreter) gated behind the new `lua-scripting` feature.
  - `oximedia-cloud`: the official AWS SDK (`aws-sdk-*`, whose smithy TLS
    stack only ships C-based crypto providers — `ring`/`aws-lc`/`s2n`) gated
    behind the new `aws-sdk` feature; S3-compatible endpoints remain
    available by default via `GenericStorage` (reqwest + rustls-rustcrypto).
  - `oximedia-videoip`: QUIC transport (`quinn`, which requires `ring` or
    `aws-lc-rs` since `rustls-rustcrypto` doesn't implement QUIC cipher
    suites) gated behind the new `quic-quinn` feature.
  - `tantivy` (oximedia-mam/search): default features trimmed to drop
    `zstd-sys`-backed columnar compression, keeping Pure-Rust
    `lz4-compression` only.
  - `actix-web` (oximedia-server): default features trimmed to drop
    `zstd`/`gzip`/`brotli` response compression (all C-backed).
  - `azure_core`/`azure_storage`/`azure_storage_blobs`: migrated to the
    unified `azure_storage_blob` 1.0 track with `default-features = false`
    and explicit `reqwest` + `hmac_rust` (Pure-Rust HMAC via sha2/hmac,
    replacing the openssl-backed default), closing the quick-xml
    RUSTSEC-2026-0194/0195 chain pinned by the deprecated 0.21 track.
  - `oximedia-audio`: the `rubato` resampler (which pulls `rustfft`/`realfft`)
    replaced with a hand-written 100% Pure-Rust band-limited
    windowed-sinc polyphase resampler (Blackman-Harris window, exact
    rational-position accumulator for drift-free long streams, chunk-size
    invariant output, explicit `flush()` for stream tails) — no FFT
    dependency of any kind.
- Added `[profile.release]` to the root `Cargo.toml`: `opt-level = 3`,
  `lto = "thin"`, `codegen-units = 1`, `strip = true` for smaller, faster
  release binaries across all ~109 crates.
- `sqlx` workspace dependency trimmed from `["runtime-tokio", "sqlite"]` to
  `["runtime-tokio", "postgres"]` (Postgres support is Pure Rust; SQLite use
  sites migrate to `oxisql-sqlite-compat` instead).
- `README.md`: added a "Live demos" section linking the OxiScope colour
  pipeline demo and the new peer-to-peer **OxiLink** video-call project —
  both running the same `oximedia-web` WebAssembly modules in production.
- Root `Cargo.toml`: fifteen dependencies that were hardcoded identically
  (or near-identically) across two or more member crates' `Cargo.toml`
  files — `approx`, `csv`, `dirs`, `encoding_rs`, `flume`, `futures-util`,
  `jsonwebtoken` (`rust_crypto` feature), `mockito`, `num-traits`,
  `pin-project`, `protox`, `reed-solomon-erasure`, `tonic-build`,
  `tonic-prost-build`, `unicode-segmentation` — centralized into
  `[workspace.dependencies]` for consistent version resolution across the
  workspace.

### Added
- **`oximedia-web`** (`web/`): a new nested Cargo workspace + npm package
  (`@cooljapan/oximedia-web`, unpublished — packaging is prepared, publish
  is pending explicit instruction) providing four independent,
  tree-shakeable WebAssembly modules downstream of the browser's own
  WebCodecs decoder — `scopes` (waveform/vectorscope/histogram/
  false-colour), `color` (exposure/contrast/saturation, tone-mapping,
  gamut mapping, 3D LUT), `scale` (Lanczos3/Catmull-Rom/Mitchell/bilinear
  resampling), and `quality` (PSNR/SSIM). Ported, dependency-free
  `f32`/`u8` kernels (no `rayon`/`scirs2`/`f64` data planes), each crate
  `#![forbid(unsafe_code)]`, no COOP/COEP requirement, all four modules
  comfortably under their gzip size budgets (150,072 B measured combined
  vs. a 512,000 B soft budget, 29% — re-measured after the kernel perf
  retune below; see `web/README.md`'s size table for the per-module
  breakdown). Ships with the OxiScope colorist demo
  (`web/demo/`, grading + four live scopes fed from the graded output) and
  a reproducible benchmark harness (`web/bench/`, headless-Chrome driven,
  zero committed/hard-coded numbers) plus four local CI-gate shell scripts
  (`build.sh`, `size-gate.sh`, `dep-gate.sh`, `serve.sh`) — no GitHub
  Actions workflow. See [`web/README.md`](web/README.md) and
  [`web/TODO.md`](web/TODO.md).
- `oximedia-web`'s public JS/TS surface (`web/js/`): hand-written ES-module
  wrappers (`_frame.js`, `color.js`, `quality.js`, `scale.js`, `scopes.js`,
  ~1,900 lines combined) plus matching hand-written `.d.ts` type
  declarations wrap each crate's raw `wasm-bindgen` glue in an idiomatic,
  tree-shakeable API — this, not the raw wasm-bindgen output, is what
  `@cooljapan/oximedia-web`'s four subpath exports (`./scopes`, `./color`,
  `./scale`, `./quality` in `web/package.json`) actually resolve to.
  `web/deny.toml` (a `cargo-deny` config scoped to the `web/` nested
  workspace — license allowlist plus the `wasm32-unknown-unknown`/
  `x86_64-unknown-linux-gnu`/`aarch64-apple-darwin` target graph) and
  `web/allowed-deps.txt` (the exact-name crate allowlist `dep-gate.sh`
  diffs against) enforce the "no `rayon`/`scirs2`/heavyweight dependency"
  constraint at CI-gate time, not just by convention.
- `rust-toolchain.toml` pinning `channel = "stable"` with `rustfmt`/`clippy`
  components, for reproducible CI and local builds.
- `CONTRIBUTING.md`, `SECURITY.md`, and `CODE_OF_CONDUCT.md` at the repo root.
- Per-crate opt-in Cargo features documented above (`vulkan-backend`,
  `lua-scripting`, `aws-sdk`, `quic-quinn`) so downstream users can restore
  the C-backed fast paths deliberately, without them leaking into the
  default build.
- Ten new `cargo-fuzz` targets in `fuzz/fuzz_targets/` — `aaf_parser`,
  `ass_parser`, `exr_parser`, `ffv1_decoder`, `jpegxl_decoder`, `srt_parser`,
  `tiff_parser`, `ttml_parser`, `webvtt_parser`, `y4m_parser` — extending
  malformed-input fuzz coverage to the AAF/ASS/OpenEXR/FFV1/JPEG XL/SubRip/
  TIFF/TTML/WebVTT/Y4M parsers, alongside the pre-existing DASH/FLAC/Opus/
  RTMP/Vorbis targets.
- `oximedia` facade crate: `oximedia/src/lib.rs` gained three real,
  `cargo test --doc`-executed doctests (probing + `dedup`, `transcode`
  config + `quality` PSNR assessment, and a `prelude` quick-start), a
  per-example "Cookbook" table cross-referencing each `examples/*.rs` file
  to the Cargo feature(s) it needs, and a doc-link to the underlying
  `oximedia_*` crate on every feature-gated re-export module;
  `oximedia/examples/ffmpeg_translate_demo.rs` demonstrates translating an
  FFmpeg command line into an OxiMedia transcode job via the
  `compat-ffmpeg` feature; `oximedia/tests/feature_matrix.rs` is a
  compile-only harness proving every Cargo feature flag builds
  independently (`cargo check --no-default-features --features <flag>` per
  flag, parsed straight out of the crate's own `Cargo.toml`);
  `oximedia/tests/prelude_smoke.rs` verifies `prelude::*` is usable with
  zero optional features enabled; and `oximedia/tests/integration.rs` adds
  cross-feature subsystem tests (always-on `OxiError`/`OxiResult`/
  `probe_format` coverage for Matroska and MP4 headers, plus feature-gated
  `quality`/`timecode`/`metering`/`archive` and combined `search`+`quality`
  suites).
- `oximedia-cli/tests/cli.rs`: a 267-line core end-to-end CLI smoke suite —
  `--version`/`version`/`version --json` reporting, top-level `--help`,
  invalid-subcommand error handling, and a real `probe` run against a
  synthetic WAV file written to `std::env::temp_dir()` — alongside the
  pre-existing, more granular `cli_help.rs`/`cli_help_per_command.rs`/
  `probe_json_snapshot.rs`/`exit_code_smoke.rs` suites.
- `oximedia` facade crate: `[package.metadata.docs.rs]` added to
  `oximedia/Cargo.toml` (`all-features = true`, `rustdoc-args = ["--cfg",
  "docsrs"]`) so the published docs.rs build renders every optional
  feature, with `doc_cfg` feature-availability badges, instead of only the
  defaults.

### Fixed
- Repo hygiene: removed tracked build-artifact droppings that should never
  have been committed (`crates/oximedia-core/src/hdr/mod.rs.orig`/`.rej`
  patch-reject files, `crates/oximedia-proxy/Cargo.toml.bak`,
  `crates/oximedia-simd/Cargo.toml.bak`, `crates/oximedia-playout/IMPLEMENTATION_SUMMARY.md`,
  a generated `doc/` rustdoc output tree, and an orphaned
  `linker-scripts/glibc_compat.lds` — a `PROVIDE()`-based shim aliasing ISO
  C23 `strtol`/`strtoll`/`strtoull` symbols for a pre-compiled ONNX Runtime
  object on glibc <2.38, unreferenced by any current build script).
- **Root workspace tokio feature-unification** (`Cargo.toml`): pinning
  `tokio = { features = ["full"] }` at the workspace root was silently
  unioning `"full"` (via Cargo's workspace feature-unification) into every
  member that declares a `tokio` dependency, including `oximedia-graph` —
  the sole `tokio` consumer reachable from the `oximedia-wasm` wasm32
  build graph — pulling in `mio` (`"full"` → `"net"` → `mio`) and breaking
  `cargo check -p oximedia-wasm --target wasm32-unknown-unknown`. Root pin
  lowered to `default-features = false` with an explicit feature list on
  every tokio-declaring member (`oximedia-graph` gets the minimal
  `net`-free set; every other member keeps an exact superset of its prior
  union-derived features, zero behavioural change); the wasm32 `mio`
  blocker is resolved.
- **`oximedia-wasm` data-plane and decoder honesty** (`oximedia-wasm/`):
  `Float64Array`-crossing `#[wasm_bindgen]` APIs (colour-management
  buffers, HDR EOTF/OETF buffers, `audiopost_wasm::wasm_mix_audio`)
  converted to `f32`/`u8`; `webcodecs_bridge.rs`'s JSON-string hot-path
  methods (`get_video_decoder_config`, `oximedia_packet_to_encoded_chunk`)
  replaced with typed getter classes; the standalone `WasmVp8Decoder`,
  `WasmAv1Decoder`, and `WasmVorbisDecoder` classes removed — each wrapped
  a decoder that produced no real output (error, unpopulated buffers, or a
  synthetic-format-only round-trip), so shipping them was dishonest;
  `oximedia-stabilize`/`oximedia-imf`/`oximedia-aaf`/`oximedia-analytics`
  (unused) dependencies and a dead `/tmp`-path line pruned; the orphaned
  `hdr_wasm`/`lut_wasm`/`spatial_wasm` modules wired into `lib.rs` (with
  their own f64→f32 conversion); `wasm-opt` re-enabled (`-Oz`); npm
  packaging (`build.sh`/`build-dev.sh`/`npm-publish.yml`) and `README.md`
  corrected so only `pkg-bundler`'s `@cooljapan/oximedia` is presented as
  published/installable — `pkg-web`/`pkg-node` are documented as
  unpublished local build artifacts. Native check/clippy(`-D
  warnings`)/test are clean, and the wasm32 target now compiles: the two
  `#[cfg(target_arch = "wasm32")]` `RequestAdapterOptions` initializers in
  `crates/oximedia-gpu/src/device.rs` were missing the `apply_limit_buckets`
  field required by `wgpu` 30.0 (the native path already set it), so
  `cargo check -p oximedia-wasm --target wasm32-unknown-unknown` failed;
  both browser paths now set `apply_limit_buckets: true` (limit-bucketing is
  a browser GPU-fingerprinting mitigation), restoring the `oximedia-gpu`
  wasm32 build reached transitively via `oximedia-colormgmt`'s default
  `gpu-accel` feature.
- **wasm32 warnings-zero sweep**: `cargo check -p oximedia-wasm --target
  wasm32-unknown-unknown` and `cargo clippy --target wasm32-unknown-unknown
  -- -D warnings` across `oximedia-gpu`, `oximedia-convert`,
  `oximedia-dolbyvision`, `oximedia-container`, `oximedia-monitor`, and
  `oximedia-batch` are now clean (0 warnings, native and wasm32 alike):
  unreachable-expression fixes in `oximedia-convert`, cfg-gated unused
  constants in `oximedia-dolbyvision`, a genuine `Shared<T>` type-alias
  split (`Rc` on `wasm32`, `Arc` elsewhere) resolving `arc_with_non_send_sync`
  in `oximedia-gpu` plus matching `apply_limit_buckets: false` wasm32
  semantics, and matching consumer-tied cfg-gates in `oximedia-container` /
  `oximedia-monitor` / `oximedia-batch`. `docs/simd_dispatch.md`'s
  "SSE4.2 fallback" section corrected: it no longer claims WASM SIMD128
  "falls to SSE4.2 paths in `x86.rs`" (that module is
  `core::arch::x86_64`-gated and never compiles on `wasm32`); it now
  forward-references the doc's own WASM SIMD128 section instead.
- `oximedia-server`: 13 production `.expect()`/`.unwrap()` call sites removed
  across `admin.rs`, `rtmp/server.rs`, `webhooks.rs`, and `db.rs` — replaced
  with proper error propagation or infallible-by-construction rewrites (e.g.
  indexing the element just pushed instead of `.last().expect(...)`,
  constructing default socket addresses instead of `"...".parse().expect(...)`).
- **`oximedia-mam` list-filter bind-parameter mismatch**
  (`AssetManager::list` in `asset.rs`, `AuditLogger::query_logs` in
  `audit.rs`): every optional filter (`mime_type`/`min_duration`/
  `max_duration`/`status`/`created_by` for assets; all 7 `AuditLogFilter`
  fields for audit logs) appended its `$N` placeholder to the query text
  but was never actually passed to `.bind()` — `audit.rs`'s `bindings` Vec
  only compiled because `user_id`/`resource_id` happen to share a type, and
  even that Vec was never bound. Any call supplying so much as one filter
  would have failed at execution time with a bind-parameter count
  mismatch. Replaced with a `bind_filters!` macro binding each present
  field in the exact order its placeholder was appended; a pre-existing bug
  unrelated to any dependency version, surfaced while auditing these call
  sites for sqlx 0.9's `SqlSafeStr` gate (`sqlx::AssertSqlSafe` now wraps
  both dynamically-built query strings, whose only dynamic content is the
  bound `$N` placeholders).
- Version metadata (`workspace.package.version`, all 108 crate path-dependency
  version pins) bumped `0.1.8` → `0.1.9`.
- **`oxiarc-*` dependency family realigned at `0.3.6`** (`Cargo.toml`):
  `oxiarc-archive`, `oxiarc-deflate`, `oxiarc-lz4`, and `oxiarc-zstd` bumped
  `0.3.5` → `0.3.6`. `oxiarc-archive` 0.3.6 calls APIs added in its own
  transitive siblings (`oxiarc-brotli`/`oxiarc-bzip2`/`oxiarc-lzma`/
  `oxiarc-snappy`/`oxiarc-core`/`oxiarc-lzhuf`) at the same `0.3.6` release;
  crates.io confirms all of them published `0.3.6` within the same few
  minutes on 2026-07-13 (siblings first, `oxiarc-archive` last, respecting
  publish dependency order), and `Cargo.lock` now resolves the whole family
  at a uniform `0.3.6`, unblocking `cargo build` for `oximedia-archive-pro`,
  `oximedia-batch`, `oximedia-convert`, `oximedia-cli`, `oximedia-py`, and
  `oximedia-wasm`.
- `oximedia-transcode`: the `TranscodeCache` module-doc example passed a
  `PathBuf` (`std::env::temp_dir().join(...)`) directly to
  `TranscodeCache::insert`, which takes `output_path: String` — the doctest
  did not compile. Fixed with `.to_string_lossy().into_owned()`.

### Improved
- **`oximedia-web` WASM kernel perf retune**: `scopes`/`color`/`scale`
  per-frame kernels retuned — `scale`'s Lanczos3 h-pass monomorphised over
  a const tap-count span with a 2-bank FMA accumulator (the prior
  runtime-tap-count loop broke the FMA latency chain) plus a 4-tap-fused
  v-pass and a bit-exact opaque-frame premultiply skip; `color`'s 3D LUT
  path repacked to u64 lattice points (1 load/corner), a branchless
  Sakamoto tetrahedron select, and a bit-identical last-pixel memo;
  `scopes` killed per-pixel function-pointer YCbCr dispatch, added
  vectorised row-buffer conversion and run-collapsed scatter accumulation
  for the vectorscope/histogram, and gained a `Scopes.load_frame` +
  `*_current()` API so a caller pays one frame-boundary copy per frame
  instead of four. wasm SIMD128 confirmed real via `wasm-dis` v128-op
  counts (hundreds per module, not just the codegen flag). Measured via
  `web/bench/run.sh` (headless Chrome 150, median of 60, macOS, this
  machine): `scopes` all-four-combined ~13.0–13.25 ms (≤16 ms budget
  target: **met**; ≤8 ms stretch goal: not met), `color`
  exposure+ACES+LUT33 ~24.9–25.1 ms (≤12 ms target: **not met**, ~3.7x
  faster than the pre-retune baseline), `scale` Lanczos3 4K→1080p
  ~51.8–52.25 ms (≤40 ms target: **not met**, ~5.4x faster). Total gzip
  across all four modules + glue held at 150,072 B / 512,000 B soft budget
  (29%) despite the kernel work. See `web/README.md`'s "Measured
  performance" section for the full table and `web/TODO.md` for the
  itemized remaining gaps (`color`/`scale` targets, both attributed to
  `VideoFrame` copy/acquisition cost rather than the wasm kernel itself).

### Development waves 21–30 (algorithmic hardening)
- **oximedia-calibrate**: flagship fix — the ICC/display color-matrix solver
  now performs a real least-squares 3×3 color matrix fit (`B·A⁻¹` via a 3×3
  adjugate inverse with a condition-number/rank-deficiency guard),
  replacing an identity-matrix stub; verified against known-answer fixtures
  at ΔE2000 < 2.0.
- Six pre-existing bugs fixed across the audio/video pipeline: `oximedia-restore`
  Wiener-filter gain (~14 dB error) and wow/flutter destructive processing,
  `oximedia-audio-analysis` Hann-window and 2048× synthesis attenuation,
  `oximedia-graph` node-collision on merge, and related SILK NSQ 440 Hz SNR
  threshold correction (now matches the actual round-trip floor, ~5.2 dB).
- `oximedia-proxy`: frequency/recency-aware cache warming (`ProxyCacheWarmer`);
  `oximedia-align`: bit-exact cross-frame descriptor cache for feature
  matching; `oximedia-search`: reusable P@k/R@k/AP/MAP evaluation harness.
  `oximedia-forensics`: perceptual-hash nearest-neighbor search wired in.
- Extensive test hardening across `gpu`, `neural`, `stream`, `video`,
  `multicam`, `normalize`, `container`, `captions`, and other crates —
  combined test gate green with 0 clippy warnings at each wave checkpoint.

## [0.1.8] - 2026-06-02

### Added
- `oximedia-repair`: mmap-backed `deep_scan` (memmap2, ≥4 MiB threshold with streaming fallback for smaller files), mtime-aware `detection_cache` (parking_lot `RwLock` short-circuit), and full `fix_issue` dispatcher wired to `conceal`, `partial`, `container_migrate`, and `codec_probe` submodules.
- `oximedia-neural`: `onnx` Cargo feature gate; new `OnnxBackend` struct (`load`, `run` with `HashMap<String, Tensor>` API) backed by `oxionnx`.
- `oximedia-audio`: `compute_log_mel_spectrogram` (STFT → Hann window → MelScale filterbank → log) added to the `spectrum` module.
- `oximedia-ml`: `AutoCaptionPipeline` — Whisper-compatible encoder+decoder ONNX inference pipeline with greedy decode; `AutoCaptionConfig`, `encode_audio`, `step_decode`, and `caption` entry points; gated behind the `auto-caption` Cargo feature.
- `oxionnx` (companion crate): `SessionBuilder::with_provider_kinds()` for typed runtime EP selection; `ProviderKind::DirectMl` variant (behind `directml` feature); EP dispatch chain consults the provider priority list at runtime.
- `oximedia-hdr`: process-wide `GamutConversionMatrix` cache (`OnceLock<RwLock<HashMap<(ColorGamut, ColorGamut), [[f32;3];3]>>>`) eliminates redundant Bradford CAT + matrix-inverse computation per call pair.
- `oximedia-stream`: six `SpliceInfoSection` encode→parse→re-encode roundtrip tests; `CmafChunk.data` migrated from `Vec<u8>` to `bytes::Bytes`; `write_cmaf_segment` returns `Vec<Bytes>` for zero-copy scatter-gather segment output.
- `oximedia-colormgmt`: `ToneCurve` enum with `ReinhardSimple`, `ReinhardExtended { l_white }`, `FilmicHable` (Hable/Uncharted2), and `AcesFitted` (Narkowicz rational) operators.
- `oximedia-dedup`: `MergeExecutor`, `AppliedAction`, and `MergeReport` — real filesystem duplicate resolution with symlink, hardlink, delete, and dry-run modes, including safety precondition checks.

### Changed
- `oxionnx`: version bumped `0.1.2 → 0.1.3` to reflect the new typed EP selection API.

### Fixed
- `oxionnx`: 18 clippy warnings in CoreML example files (`coreml_arcface_smoke.rs`, `coreml_scrfd_smoke.rs`, `coreml_inswapper_smoke.rs`) resolved; examples now compile cleanly under `-D warnings`.
- `oximedia-repair`: orphaned stub `repair_engine.rs` (all branches logged no-ops with no real implementation) deleted.

## [0.1.7] - 2026-05-21

### Fixed
- **Issue #9** — Theora decoded frame plane data now correctly written into `VideoFrame` planes via `copy_from_slice` instead of writing to a dropped temporary clone.
- **Issue #13** — `oximedia-timesync` IPC socket module now gated on `#[cfg(all(unix, not(target_arch = "wasm32")))]` to prevent Windows/WASM compilation failures.
- **Issue #14** — `TempFileManager` filenames are now globally unique across concurrent instances using a static `AtomicU64` manager-ID counter combined with process ID and creation nanoseconds.
- **Issue #15** — AVC SPS constraint byte doctest corrected to use 8-bit read (6 flags + 2 reserved) per H.264 §7.3.2.1.1.
- **Issue #16** — Scope command temp input files now use unique per-call names (PID + thread ID + AtomicU64 + nanos) to prevent parallel invocation collisions; frame-extract hardcoded `/tmp/out.y4m` replaced with `std::env::temp_dir()`.

### Added
- Regression tests for all fixed issues (#9, #13, #14, #15, #16).
- Build prerequisites documentation for `protoc` (tonic-build), `cmake`, and `shaderc` toolchain in root `README.md` and `crates/oximedia-accel/README.md`.

## [0.1.6] - 2026-04-25

### Added
- **Stub implementations across 10+ crates** — accel color-space conversion helpers (RGB↔YCbCr, HSV, linear↔sRGB), Vorbis codebook VQ decode scaffolding, ACES Output Device Transform (ODT) variants (P3-D65, Rec.709, Rec.2020, D60-sim, sRGB), DASH segment HTTP fetch skeleton, and system font directory scanning (`/System/Library/Fonts`, `~/.local/share/fonts`, Windows `C:\Windows\Fonts`). All stubs compile cleanly, are documented with `#[allow(dead_code)]` guards, and carry TODO markers pinned to specific crate milestones.
- **Wave 3 stub resolution** — 13 previously-placeholder functions across `oximedia-codec`, `oximedia-audio`, `oximedia-image`, `oximedia-lut`, and `oximedia-caption-gen` replaced with functional implementations; total test count rose to **81,582** (up from ~80,900 at Wave 2 baseline).
- **`oxifft` upgraded to 0.3.0** — workspace dependency bumped from 0.2.0 to 0.3.0; all 13 dependent crates (`oximedia-audio`, `oximedia-audio-analysis`, `oximedia-audiopost`, `oximedia-mir`, `oximedia-effects`, `oximedia-dedup`, `oximedia-watermark`, `oximedia-multicam`, `oximedia-cv`, `oximedia-metering`, `oximedia-restore`, `oximedia-analysis`, `oximedia-watermark`) pass `cargo check` cleanly. OxiFFT 0.3.0 delivers Makhoul-reduction DCT-II (~4× faster vs 0.2.0), plan caching for R2r/R2c solvers, and hand-optimized AVX-512 codelets for sizes 16/32/64; the `fft`/`ifft`/`Complex` surface used by OxiMedia is API-stable.

### Changed
- **`exr.rs` refactored into 9 modules** via `splitrs` — the monolithic `oximedia-image/src/exr.rs` (previously over 2000 lines) was split into: `exr/core.rs`, `exr/compression.rs`, `exr/channels.rs`, `exr/metadata.rs`, `exr/scan_lines.rs`, `exr/tiles.rs`, `exr/deep.rs`, `exr/multipart.rs`, and `exr/mod.rs`. All files are under 2000 lines; public API is unchanged.
- **AWS SDK sub-crate version constraints updated** to match Cargo.lock actuals: `aws-sdk-s3 1.131`, `aws-sdk-mediaconvert 1.126`, `aws-sdk-medialive 1.134`, `aws-sdk-mediapackage 1.98`, `aws-sdk-cloudwatch 1.110`, `aws-sdk-sts 1.103`, `aws-sdk-kms 1.105` (cosmetic alignment; Cargo.lock was already current).
- Workspace version bumped to **0.1.6** (was 0.1.5).

### Security
- **RUSTSEC-2026-0104 documented and ignored** (`audit.toml` + `.cargo/audit.toml`) — reachable panic in `rustls-webpki 0.101.7` CRL parsing, transitive via `aws-sdk-* → aws-smithy-runtime/tls-rustls → legacy-rustls-ring → rustls 0.21.12`. OxiMedia S3/cloud calls never perform CRL checks (standard DNS hostnames, no revocation list usage); the affected code path is unreachable at runtime. Upgrading to the patched `rustls-webpki 0.103.13` path requires `aws-lc-sys` (C dependency excluded by COOLJAPAN Pure Rust Policy). Entry mirrors the existing rationale for RUSTSEC-2026-0098 and RUSTSEC-2026-0099; `cargo audit` exits 0. Will resolve when AWS SDK migrates `aws-smithy-runtime` to rustls 0.23+.

### Validated
- `cargo check` clean for all 13 `oxifft`-dependent crates after upgrade to 0.3.0.
- `cargo audit --no-fetch` exits 0 (no unignored vulnerabilities).
- `splitrs`-generated `exr/` modules all under 2000 lines; no public API regressions.

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

[Unreleased]: https://github.com/cool-japan/oximedia/compare/v0.1.9...HEAD
[0.1.9]: https://github.com/cool-japan/oximedia/compare/v0.1.8...v0.1.9
[0.1.2]: https://github.com/cool-japan/oximedia/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/cool-japan/oximedia/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/cool-japan/oximedia/releases/tag/v0.1.0
