# oximedia-cli — Command-Line Interface TODO

**Version: 0.1.3**
**Status as of: 2026-04-15**

The `oximedia-cli` crate is the primary user-facing entry point to the OxiMedia
Sovereign Media Framework. It ships **two binaries** from a single crate — the
main `oximedia` multitool with ~85 domain subcommands covering the entire FFmpeg
+ OpenCV feature surface in patent-free, pure-Rust form, and the `oximedia-ff`
drop-in that accepts raw FFmpeg-style argv and executes each translated job
through the native transcode engine. Every subcommand is wired through clap
derive, returns `anyhow::Result<()>`, respects a global `--json` / `--no-color`
/ `-v/-vv/-vvv` / `-q` flag set, and delegates real work to the ~101 workspace
crates. The whole crate weighs in at ~50,900 lines of Rust across 89 source
files (counted on 2026-04-15 via `wc -l src/*.rs src/bin/*.rs`).

---

## Current Status

### Binary targets (`Cargo.toml`)

| Binary         | Entry point                | Purpose                                                  |
|----------------|----------------------------|----------------------------------------------------------|
| `oximedia`     | `src/main.rs` (775 lines)  | Primary multitool; dispatches all domain subcommands.    |
| `oximedia-ff`  | `src/bin/oximedia-ff.rs`   | FFmpeg drop-in — argv → `oximedia-compat-ffmpeg` → exec. |

Both share code via the thin `src/lib.rs` (33 lines) which publicly re-exports
`presets`, `progress`, and `transcode` so the `oximedia-ff` binary does not
duplicate transcode plumbing.

### Source layout (89 files, ~50,900 SLOC)

- **Entry**: `main.rs` (775), `lib.rs` (33), `bin/oximedia-ff.rs` (348).
- **Command surface**: `commands.rs` (1442) — defines the top-level `Commands`
  enum (~85 variants) and shared `MonitorCommand`, `RestoreCommand`,
  `CaptionsCommand`, `PresetCommand` sub-enums; `handlers.rs` (829) — shared
  handler functions (`probe_file`, `show_info`, `show_version`, logging init,
  monitor/restore/captions/preset dispatch).
- **Domain modules (`*_cmd.rs`)**: 77 files, one per top-level subcommand group
  (aaf, access, align, archive, archivepro, audio, audiopost, auto, batch,
  calibrate, captions, clips, cloud, collab, color, conform, dedup, denoise,
  distributed, dolbyvision, drm, edl, farm, ffcompat, filter, forensics, gaming,
  graphics, image, imf, loudness, lut, mam, mir, mixer, monitor, multicam, ndi,
  normalize, optimize, package, playlist, playout, plugin, profiler, proxy, qc,
  quality, recommend, renderfarm, repair, restore, review, rights, routing,
  scaling, scopes, search, stabilize, stream, subtitle, switcher, timecode,
  timeline, timesync, tui, vfx, videoip, virtual, watermark, workflow).
- **Shared helpers**: `analyze.rs`, `batch.rs`, `benchmark.rs`, `concat.rs`,
  `extract.rs`, `metadata.rs`, `progress.rs`, `scene.rs`, `thumbnail.rs`,
  `transcode.rs`, `validate.rs`, plus `presets/` (builtin, custom, device,
  streaming, validate, web) and `sprite/` (generate, output, timestamps, utils).

### Top-level subcommand taxonomy (from `commands::Commands`)

- **Probing & inspection** — `probe`, `info`, `version`, `validate`, `analyze`,
  `benchmark`, `metadata`, `forensics`.
- **Transcoding & muxing** — `transcode` (alias `convert`), `extract`, `batch`,
  `concat`, `thumbnail`, `sprite`, `package` (HLS/DASH), `optimize`,
  `batch-engine` (SQLite-backed persistent queue).
- **Audio** — `audio`, `loudness`, `normalize`, `mixer`, `audiopost`,
  `mir` (music-info retrieval), `align`.
- **Video processing** — `scene`, `scopes`, `denoise`, `stabilize`, `scaling`,
  `filter`, `lut`, `color`, `dolby-vision`, `multicam`, `vfx`, `image`,
  `graphics`.
- **Subtitles & captions** — `subtitle`, `captions`, `timecode`.
- **Broadcast & production** — `playout`, `switcher`, `ndi`, `videoip` (RTP/
  SRT/RIST), `calibrate`, `virtual`, `routing`, `timeline`, `timesync`, `edl`,
  `playlist`, `conform`, `qc`.
- **Archival & asset management** — `archive`, `archive-pro`, `mam`, `clips`,
  `proxy`, `dedup`, `search`, `watermark`, `drm`, `rights`, `access`,
  `repair`, `restore`.
- **Collaboration & workflow** — `collab`, `review`, `workflow`, `auto`,
  `recommend`, `imf`, `aaf`.
- **Infrastructure** — `distributed`, `farm`, `renderfarm`, `cloud`, `monitor`,
  `profiler`, `stream`, `plugin`, `quality`, `gaming`.
- **Interop & UX** — `ffcompat` (alias `ff`), `tui`, `preset`.

### Global flags (`Cli` in `main.rs`)

- `-v / --verbose` (repeatable, `ArgAction::Count` → `-v`, `-vv`, `-vvv`).
- `-q / --quiet` — suppress everything except errors.
- `--no-color` — disables `colored::control` overrides.
- `--json` — propagates into every subcommand that has a structured-output path.

### FFmpeg compatibility story

Two complementary layers both route through `oximedia-compat-ffmpeg`:

1. **In-tool**: `oximedia ff <ffmpeg-args>` (`Commands::Ffcompat`, alias `ff`)
   handled by `src/ffcompat_cmd.rs` (373 lines). Supports `--dry-run` / `--plan`
   to print translated jobs without executing.
2. **Drop-in replacement**: `oximedia-ff` binary (348 lines) — a standalone
   argv→translate→execute pipeline that can be symlinked as `ffmpeg` so legacy
   scripts transparently retarget onto OxiMedia. Honours `-h/--help`,
   `-version/--version`, `--dry-run`, `--plan` and prints FFmpeg-style
   diagnostics (`warning:`, `info:`, `error:` prefixes with hints).

Both rely on `oximedia-compat-ffmpeg`'s `parse_and_translate()` which emits
`DiagnosticKind::{PatentCodecSubstituted, UnknownOptionIgnored, FilterNotSupported,
UnsupportedFeature, Info, Warning, Error}` so H.264/H.265/AAC invocations are
auto-rewritten into AV1/VP9/Opus and reported back transparently.

### Interactive TUI (`tui_cmd.rs`, 521 lines)

Three-tab ratatui interface (`Files`, `Commands`, `About`) with crossterm input,
panic-safe terminal restoration, and a working file browser that lists cwd
entries by size. Keyboard: `q`/`Ctrl+C` quit, `Tab`/`→`/`←`/`Shift+Tab` change
tabs, `↑`/`↓` navigate, `Enter` show details.

---

## Completed `[x]`

- [x] Two-binary crate layout — `oximedia` (primary) and `oximedia-ff` (FFmpeg
  drop-in) both shipping from the same `oximedia-cli` crate.
- [x] Core inspection suite — `probe` (text/json/csv, chapters, per-stream,
  metadata dump, content hash, quality snapshot), `info`, `version`.
- [x] Full transcode pipeline — `transcode` with FFmpeg-compatible aliases
  (`-i`, `-c:v`, `-c:a`, `-b:v`, `-b:a`, `-vf`, `-af`, `-ss`, `-t`, `-r`, `-y`),
  two-pass, CRF, preset names, resume, stream mapping, loudness normalize hook.
- [x] Frame / thumbnail / sprite generation — `extract`, `thumbnail` (single /
  multiple / grid / auto), `sprite` with WebVTT + JSON manifest, configurable
  sampling strategy (uniform / scene-based / keyframe-only / smart) and layout
  modes.
- [x] Batch processing — both the ad-hoc `batch` subcommand (TOML config, `-j`
  jobs, `--continue-on-error`, `--dry-run`) and the persistent SQLite-backed
  `batch-engine submit/status/list/cancel/report` command.
- [x] 0.1.2 additions: `loudness` (EBU R128 analyze / check / standards / info),
  `quality` (PSNR/SSIM/BRISQUE/NIQE etc. compare/analyze/list/explain),
  `dedup` (scan/report/clean/hash/compare), `timecode` (convert/calculate/
  validate/burn-in), `normalize` (analyze/process/check/targets),
  `batch-engine`, `scopes`, `workflow`, `version`.
- [x] ~85 domain subcommand modules wired end-to-end through `commands.rs` →
  `main.rs` match arms → `*_cmd.rs::handle_*_command` handlers, every one
  honouring the global `--json` flag.
- [x] Broadcast & live production coverage — `playout`, `switcher`, `ndi`,
  `videoip`, `multicam`, `routing`, `virtual`, `calibrate`, `timesync`.
- [x] MAM / archival coverage — `mam`, `search`, `dedup`, `archive`, `archive-pro`,
  `proxy`, `clips`, `review`, `drm`, `rights`, `access`, `watermark`, `repair`,
  `restore`.
- [x] Production post coverage — `timeline`, `edl`, `conform`, `qc`, `vfx`,
  `graphics`, `image`, `audiopost`, `mixer`, `mir`, `subtitle`, `captions`,
  `color`, `lut`, `dolby-vision`.
- [x] Infrastructure — `distributed`, `farm`, `renderfarm`, `cloud`, `monitor`,
  `profiler`, `plugin`, `stream`, `quality`, `recommend`, `scaling`, `optimize`.
- [x] FFmpeg compatibility layer — both `oximedia ff <args>` and the standalone
  `oximedia-ff` binary, both wired to `oximedia-compat-ffmpeg::parse_and_translate`
  with patent-codec auto-substitution, diagnostic forwarding, and `--dry-run`.
- [x] Interactive TUI — `oximedia tui` launches a three-tab ratatui UI with
  cwd file browser, command reference with descriptions, and about panel.
- [x] Shared `handlers.rs` — `init_logging` respects `-v` count and `-q`,
  `probe_file`, `show_info`, `show_version` with feature-gated build info.
- [x] Coloured error output — every failure is reported via `colored` as
  `Error: …` in red + `Caused by: …` chain from `anyhow::Error::source()`.
- [x] Progress reporting harness — `progress.rs` (397 lines) built on
  `indicatif` for transcode / batch / analyze long-running operations.
- [x] Preset system — `presets/` module (builtin, custom, device, streaming,
  validate, web) plus `preset list/show/create/template/import/export/remove`
  subcommand; preset doctest defects fixed during 0.1.2.
- [x] Bug fixes shipped during 0.1.2 — archive_cmd compile errors, farm_cmd
  async issues, search_cmd missing types, and the preset-module doctest.
- [x] `build.rs` minimised to `fn main() {}` after workspace-level linker
  script (`.cargo/config.toml`) took over glibc 2.38+ `__isoc23_*` symbol
  compat (keeps crate-specific build config available for the future).
- [x] WASM / non-CLI surface explicitly NOT linked — the crate only compiles
  for native targets by design; `oximedia-wasm` handles the browser story.

---

## Enhancements `[ ]`

### Shell integration & packaging

- [x] Add shell completions for bash/zsh/fish/powershell/elvish
  - **Goal:** `oximedia completions <shell>` emits completion script to stdout
  - **Design:** `clap_complete::generate(shell, &mut cmd, "oximedia", &mut io::stdout())` with Generator impls for Bash/Zsh/Fish/PowerShell/Elvish
  - **Files:** `src/completions_cmd.rs` (new), `src/commands.rs`, `src/main.rs`, `Cargo.toml`
  - **Tests:** `tests/completions_smoke.rs` — each shell variant produces non-empty output
- [x] Generate Unix man page (oximedia.1)
  - **Goal:** `oximedia man-page` emits roff to stdout
  - **Design:** `clap_mangen::Man::new(cmd).render(&mut buf)?`
  - **Files:** `src/man_cmd.rs` (new), `src/commands.rs`, `src/main.rs`, `Cargo.toml`
  - **Tests:** `tests/man_smoke.rs` — output starts with `.TH OXIMEDIA`
- [x] Implement `oximedia doctor` environment diagnostics command
  - **Goal:** Reports Rust version, GPU adapters, temp dir space, OXIMEDIA_TEMP writability
  - **Design:** `DoctorReport { rust_version, gpu_adapters, temp_dir_space, temp_writability }` — enumerate_adapters (sync) + statvfs + write-delete probe; `--json` flag
  - **Files:** `src/doctor_cmd.rs` (new), `src/commands.rs`, `src/main.rs`
  - **Tests:** `tests/doctor_smoke.rs` — exits 0, `--json` produces valid JSON
- [ ] `cargo-dist` / GitHub Actions pipeline producing signed pre-built binaries
  (x86_64-linux-gnu, x86_64-linux-musl, aarch64-linux, x86_64-darwin,
  aarch64-darwin, x86_64-windows-msvc) plus shasums and SBOMs.
- [ ] Platform packaging — Homebrew formula, Windows `winget`/Scoop manifest,
  Debian/Ubuntu `.deb`, RPM for Fedora/RHEL, Arch Linux AUR `PKGBUILD`.

### Output & piping

- [x] Audit every *_cmd.rs accepting --json for strict JSON output (no mixed colored text)
  - **Goal:** When --json is set, stdout is pure parseable JSON; colored text goes to stderr only
  - **Design:** For each of scopes_cmd, graphics_cmd, timeline_cmd, workflow_cmd, vfx_cmd, review_cmd, collab_cmd — ensure colored output is behind `!json_output` guards; `colored::control::set_override(false)` when json_output
  - **Files:** `src/output.rs` (new), 7 cmd files, `src/main.rs`
  - **Tests:** `tests/json_strict.rs` — each of 7 commands with --json produces parseable JSON stdout
- [x] Add --ndjson streaming output for probe/quality/loudness/monitor commands
  - **Goal:** One NDJSON record per file/frame/window, flushed immediately
  - **Design:** `NdjsonWriter<W: Write>` with `emit<T: Serialize>` + `flush`; global `--ndjson` flag `conflicts_with("json")`
  - **Files:** `src/output.rs` (new), `src/main.rs`, `src/probe_cmd.rs`, `src/quality_cmd.rs`, `src/loudness_cmd.rs`, `src/monitor_cmd.rs`
  - **Tests:** `tests/ndjson_probe.rs`, `tests/json_ndjson_conflict.rs`
- [x] Standardise exit codes (0=Ok, 1=GenericError, 2=UsageError, 3=IoError, 4=ValidationError)
  - **Goal:** All CLI errors emit the correct numeric exit code
  - **Design:** `pub enum ExitCode { Ok=0, GenericError=1, UsageError=2, IoError=3, ValidationError=4 }` + `From<&anyhow::Error>` classifier
  - **Files:** `src/exit_codes.rs` (new), `src/main.rs`
  - **Tests:** `tests/exit_codes.rs` — nonexistent file → code 3; unknown subcommand → code 2
- [x] Add `--log-format json` global flag via tracing-subscriber JSON layer
  - **Goal:** Structured JSON log output for pipeline/automation consumers
  - **Design:** `LogFormat { Plain, Json }` enum; `init_logging` accepts `log_format`; Json path uses `tracing_subscriber::fmt::layer().json()`
  - **Files:** `src/handlers.rs`, `src/main.rs`
  - **Tests:** inline — `--log-format json` produces valid JSON log lines

### `oximedia-ff` / `ffcompat` coverage

- [x] Expand the FFmpeg flag surface handled by `oximedia-compat-ffmpeg`
  - **Goal:** `-filter_complex`, `-map_metadata`, `-hwaccel`, `-threads`, `-g`/`-keyint_min`, `-hide_banner`, `-stats`, `-progress`, `-nostats`, `-loglevel` all reach the translator; wire existing dead-code `metadata_compat.rs` and `hwaccel_compat.rs` modules
  - **Design:** Add `pub mod metadata_compat; pub mod hwaccel_compat;` to `lib.rs`; wire into `translator.rs`; add `-map_metadata` arm to `arg_parser.rs`; deepen `filter_complex.rs` with quoted-string and escape lexer
  - **Files:** `crates/oximedia-compat-ffmpeg/src/lib.rs`, `translator.rs`, `arg_parser.rs`, `filter_complex.rs`
  - **Tests:** `crates/oximedia-compat-ffmpeg/tests/hwaccel.rs`, `map_metadata.rs`; inline filter_complex parser tests for quoted strings and escapes
- [x] Document and test the 75+ codec / 30 format mappings
  - **Goal:** Rustdoc in `oximedia-compat-ffmpeg/src/lib.rs` lists all codec/format mappings with FFmpeg-side names → OxiMedia codec IDs
  - **Design:** Pure doc expansion; no runtime changes; "test against real ffmpeg" deferred to a follow-up pass (requires real ffmpeg in CI)
  - **Files:** `crates/oximedia-compat-ffmpeg/src/lib.rs`
  - **Tests:** `cargo doc --no-deps -p oximedia-compat-ffmpeg` must succeed with zero warnings
- [x] Add an `oximedia-ff --explain <args>` mode
  - **Goal:** Prints translation table (input flag → OxiMedia option) without executing transcode
  - **Design:** `--explain` flag on top-level Cli; when present, run translation but skip `oximedia_transcode::run`; emit rows like `Input → -i in.mp4 → input_path = "in.mp4"`
  - **Files:** `oximedia-cli/src/ffcompat_cmd.rs`, `oximedia-cli/src/bin/oximedia-ff.rs`
  - **Tests:** `oximedia-cli/tests/explain.rs` — exits 0, contains "Translation table", no file written
- [x] Ship an opt-in `oximedia-cv2` companion binary (planned 2026-05-05, completed 2026-05-05)
  - **Goal:** Companion binary `oximedia-cv2` using the new `oximedia-compat-cv2` crate; 14 named-arg subcommands + introspection flags (`--list-functions`, `--list-constants`, `--explain`)
  - **Design:** clap-derive; subcommands: `Imread`, `CvtColor`, `Resize`, `GaussianBlur`, `Canny`, `Threshold`, `Sobel`, `Erode`, `Dilate`, `FindContours`, `HoughLines`, `EqualizeHist`, `LkFlow`, `Probe`; global `--list-functions` (≥30 entries table), `--list-constants` (≥130 entries), `--explain` (dispatch table, no execution)
  - **Files:** `oximedia-cli/src/bin/oximedia-cv2.rs`, `oximedia-cli/Cargo.toml` (add `[[bin]]` + dep on `oximedia-compat-cv2`), `oximedia-cli/tests/cv2_smoke.rs`, `cv2_list.rs`, `cv2_imread_imwrite.rs`, `cv2_cvt_color.rs`, `cv2_explain.rs`
  - **Tests:** smoke (`--version`/`--help` exit 0), list (functions ≥30 / constants ≥130), imread+imwrite PNG round-trip via assert_cmd, cvt-color BGR2GRAY output is 1-ch, explain does NOT write output file
  - **Risk:** binary size increase; acceptable — users can opt out at build time with `--bins oximedia`
  - **Prerequisite:** `oximedia-compat-cv2` crate (Slices A-E)

### TUI polish (`tui_cmd.rs`)

- [x] Replace the placeholder file-info string with a real mini-probe render
  - **Goal:** `App::on_enter()` triggers real probe; right pane shows codec, resolution, duration, bitrate
  - **Design:** Spawn background thread with sync `std::fs::File::read` → `MultiFormatProber::probe()`; send via `std::sync::mpsc::sync_channel`; drain in event loop tick via `poll_probe_result()`; render via `ratatui::widgets::Paragraph`
  - **Files:** `oximedia-cli/src/tui_cmd.rs`
  - **Tests:** `format_probe_info_full`, `format_probe_info_audio_only` inline unit tests
- [x] Add an actionable "run command" tab
  - **Goal:** Commands tab lets user prefill arguments and dispatch into real subcommand; result shown in scrollable pane
  - **Design:** `CommandTabState::Browsing/InputArgs` enum; Enter enters input mode; second Enter spawns `std::process::Command::new(current_exe())` capturing stdout+stderr; shown in scrollable Paragraph
  - **Files:** `oximedia-cli/src/tui_cmd.rs`
  - **Tests:** `command_tab_enters_input_mode_on_enter`, `command_tab_esc_in_input_mode_goes_back`, `command_input_accumulates_chars`
- [x] Mouse support, PgUp/PgDn, `/` incremental search
  - **Goal:** Mouse scroll changes selection; PgUp/PgDn jumps 10; `/` enters search mode filtering file_list case-insensitively; Esc exits search
  - **Design:** `EnableMouseCapture`/`DisableMouseCapture` in setup/teardown and panic hook; `Event::Mouse(ScrollUp/Down)` → `select_previous/next()`; `PageUp/Down` → 10× loop; `search_mode: bool` + `search_query: String`; key routing checks search mode first to prevent 'q'/Tab from leaking
  - **Files:** `oximedia-cli/src/tui_cmd.rs`
  - **Tests:** `search_filter_case_insensitive`, `search_filter_uppercase_query`, `search_esc_clears_filter`, `search_empty_query_shows_all`, `pgup_moves_selection_by_10`, `pgdn_moves_selection_by_10`
- [x] Persist cwd navigation (Enter on directory descends)
  - **Goal:** Enter on a directory entry descends into it; Backspace pops the stack
  - **Design:** `cwd_stack: Vec<PathBuf>` pushed on descend, popped on `ascend_dir()`; `filtered_file_list` refreshed via `refresh_file_list()` → `load_dir_files()`; persisted to `$XDG_STATE_HOME/oximedia/tui.json` (best-effort via `persist_cwd`)
  - **Files:** `oximedia-cli/src/tui_cmd.rs`
  - **Tests:** `descend_pushes_cwd_stack`, `ascend_pops_cwd_stack`

### Subcommand-level gaps (direct `grep` findings)

- [x] `captions_cmd.rs:114` — `generate` currently writes a placeholder caption
  track; wire to the real ASR pipeline (OxiMedia has speech alignment in
  `oximedia-caption-gen`). Done: feature-gated `caption-gen` implementation
  wiring `CaptionEncoder` → `greedy_decode` → `build_caption_blocks` →
  `optimal_break` → `CaptionTrack`. Default build returns a clear error.
- [x] `restore_cmd.rs:80,210` — audio path assumes raw PCM f32 LE; video path
  writes the input through unchanged. Wire to `oximedia-restore` decoders. (Wave 2 Slices 4+5)
  - **Goal:** Format-aware WAV/FLAC/MP3 decode for audio restore; frame-level deinterlace/upscale/color-correct for video restore via FramePipelineConfig.
  - **Files:** `restore_cmd.rs`, `commands.rs`, `crates/oximedia-transcode/src/frame_pipeline.rs`
- [x] Wire conform_cmd to single-file QC path (replace byte-copy stub)
  - **Goal:** `conform check/report` use `oximedia-qc::QualityControl` + `oximedia-transcode::TranscodePipeline` for single-file QC. `ConformSession` is reserved for the EDL-timeline matching path (different use case).
  - **Design:** `ConformSession::new(timeline_path)?.match_assets(paths).await?` for check; `compute_statistics()` for report
  - **Files:** `src/conform_cmd.rs`, `Cargo.toml`
  - **Tests:** `tests/conform_check.rs`, inline unit tests for fallback path
- [x] Wire cloud_cmd to real oximedia-storage backends (replace simulation strings)
  - **Goal:** `cloud upload/download/list` use real S3/GCS/Azure/R2/B2 backends; error when creds missing
  - **Design:** `build_backend_for_provider(p)` dispatches to S3Backend/GcsBackend/AzureBackend constructed from env vars
  - **Files:** `src/cloud_cmd.rs`, `Cargo.toml`
  - **Tests:** `tests/cloud_no_creds.rs` — no-creds error names AWS_ACCESS_KEY_ID; inline provider dispatch tests
- [x] Extend scopes_cmd frame extraction beyond Y4M (use oximedia-container/codec)
  - **Goal:** Any supported container/codec works as scopes input, not only Y4M
  - **Design:** `async fn decode_via_demuxer<D: Demuxer>` backed by Mp4/Matroska/MpegTs demuxers + Av1/Vp9/Vp8 decoders; Y4M stays as fast path; stride-aware `repack_plane` handles padded decoder output
  - **Files:** `src/frame_extract.rs`, `src/scopes_cmd.rs`, `src/thumbnail.rs`
  - **Tests:** `tests/scopes_smoke.rs`, inline async unit tests in `frame_extract.rs`
- [x] Replace synthetic silence in normalize_cmd with real audio decode
  - **Goal:** LUFS analysis runs on actual audio content, not zeros
  - **Design:** `AudioSampleIterator::open(input)?` loop feeding `normalizer.analyze_f32(chunk?)`
  - **Files:** `src/decode_helper.rs` (new), `src/normalize_cmd.rs`
  - **Tests:** `tests/normalize_e2e.rs` — 1kHz@0dBFS WAV → LUFS ∈ [-3.5, -2.5]
- [x] `proxy_cmd.rs:302` — proxy generation writes a placeholder file; invoke
  the real low-bitrate transcode path. (Resolved: `generate_proxy_via_proxy_crate`
  now calls `oximedia_proxy::ProxyGenerator` which uses `TranscodePipeline`
  with per-codec settings; placeholder fallback removed.)
- [x] Wire metadata.rs to real oximedia-metadata readers; replace sidecar with format-specific metadata
  - **Goal:** id3v2/exif/quicktime/matroska readers; format_timestamp returns RFC 3339
  - **Design:** `detect_format(path)` → match to id3v2/vorbis/opus_tags/quicktime/matroska/exif readers; `chrono::DateTime::<Utc>::from_timestamp(secs,0).to_rfc3339()`
  - **Files:** `src/metadata.rs`, `Cargo.toml`
  - **Tests:** `tests/metadata_id3v2.rs`, `tests/metadata_exif.rs`, `tests/metadata_sidecar_fallback.rs`, inline timestamp tests
- [x] `timeline_cmd.rs:763,966` — `generate_otio_placeholder` writes minimal JSON missing RationalTime rationals, TimeRange schema, ExternalReference. (Wave 2 Slice 2)
  - **Goal:** Full OTIO 0.17-compatible JSON (Timeline/Stack/Track/Clip/Gap/RationalTime/TimeRange/ExternalReference schemas).
  - **Files:** `oximedia-cli/src/timeline_cmd.rs`
- [x] `extract.rs:296` — all frame extracts currently fall back to PPM as a
  lossless stand-in; finish the PNG/JPEG output encoders.
  (Done: real container probe + demux + codec decode pipeline; PNG via PngEncoder,
  JPEG via JpegEncoder; synthetic frame generator removed; log updated to show actual input path.)

### Testing

- [x] Build out tests/ directory with assert_cmd integration tests and E2E smoke suite
  - **Goal:** Non-empty tests/ with --help smoke, JSON validation, E2E decode, snapshot tests
  - **Design:** `assert_cmd::Command::cargo_bin("oximedia")` + `tempfile::TempDir`; common/mod.rs with WAV/fixture helpers
  - **Files:** `tests/cli_help.rs`, `tests/probe_json_snapshot.rs`, `tests/normalize_e2e.rs`, `tests/scopes_decode.rs`, `tests/common/mod.rs`
  - **Tests:** ~15 tests covering help smoke, JSON strict, LUFS E2E, decode smoke
- [x] End-to-end test for `oximedia-ff` against a golden set of real FFmpeg
  command lines (both passing and patent-codec-substituted).
  (completed 2026-05-06 — 71 JSON fixtures in `tests/ff_golden/*.json` driven by
  `tests/ff_golden.rs` via the new `oximedia-ff --json` structured-output flag.
  Coverage: 12 patent substitutions, 4 direct codecs + copy, 8 quality flags,
  15 single-chain video/audio filters, 3 filter_complex graphs, 5 stream
  selectors (-map / -vn / -an), 4 seek/duration flags, 3 metadata/movflags,
  2 two-pass phases, 3 hardware acceleration backends, 4 loglevel/banner
  flags, 2 format aliases, 1 overwrite, 2 combo invocations, 2 hard-error
  cases. Partial-match expectations keep the suite resilient to translator
  field additions.)
- [x] Snapshot tests for `oximedia probe --format json` on fixtures shipped
  by `oximedia-io`. (completed 2026-05-06: `tests/probe_format_json_snapshot.rs`
  builds tiny in-process magic-byte fixtures (MP4/WebM/Ogg/FLAC) into per-test
  `tempfile::TempDir`, runs `oximedia --quiet probe -i ... --format json`, and
  compares against `tests/probe_snapshots/*.json` after path/size/f32-confidence
  normalization. 8 snapshots cover 4 containers + the `--streams` / `--chapters`
  / `--metadata` / all-flags schemas. Re-baseline with `OXIMEDIA_UPDATE_SNAPSHOTS=1`.)
- [x] `assert_cmd` + `predicates` based CLI test harness that exercises
  every top-level variant of `Commands` at least once.
  *(completed 2026-05-06: `tests/cli_help_per_command.rs` covers all 90
  variants + `convert`/`ff` aliases via `oximedia <subcmd> --help` smoke
  (92 tests). `tests/cli_smoke_listings.rs` adds 10 listing-style smokes
  (`preset list`, `plugin list`, `loudness standards`, `quality list`,
  `info`, `version`, `doctor`, `doctor --json`, `man-page`,
  `completions bash`). `tests/cli_help.rs` rewritten as the root-level
  `--help` / `--version` / no-args smoke (5 tests). 107 new tests total,
  zero clippy warnings introduced.)
- [x] Add an `examples/` directory demonstrating shell-script workflows that
  chain subcommands (transcode → loudness check → package → upload via cloud).
  *(completed 2026-05-06: 8 reference scripts + README under
  `oximedia-cli/examples/` — `dailies-ingest`, `quality-check`,
  `abr-package`, `loudness-normalize`, `forensics-investigate`,
  `restore-degraded`, `live-broadcast`, `cv2-pipeline`. All scripts pass
  `bash -n` and use only verified-wired flags.)*

### Performance & UX

- [ ] Lazy subcommand loading — the single `main.rs` match is 500+ arms; as
  the CLI grows, consider switching to `clap_derive` with `#[command(flatten)]`
  into per-domain enums so `--help` per-domain is faster to render.
  (Deferred with the commands.rs 1442-line split refactor — they belong together.)
- [x] Colour-aware `--no-color` auto-detection
  - **Goal:** `NO_COLOR` (any value), `CLICOLOR=0`, `TERM=dumb`, explicit `--no-color` flag all disable coloured output; flag wins over env
  - **Design:** `pub fn init_color(force_no_color: bool)` in `handlers.rs`; called early in `main` before any `colored::Colorize` use; compatible with existing `--json`/`--ndjson` disable-color logic from Run 1
  - **Files:** `oximedia-cli/src/handlers.rs`, `oximedia-cli/src/main.rs`
  - **Tests:** `tests/no_color.rs` — `NO_COLOR=1 oximedia probe nonexistent` has zero ANSI codes in stderr; `--no-color` flag same; inline handler unit tests
- [x] Parallelise `batch` using rayon work-stealing
  - **Goal:** `oximedia batch -j N` uses rayon ThreadPool; `-j 0` means num_cpus; result order preserved
  - **Design:** `tokio::task::spawn_blocking(move || pool.install(|| jobs.par_iter().map(run_one_job).collect()))` to avoid blocking the async runtime; `rayon.workspace = true` + `num_cpus.workspace = true`
  - **Files:** `oximedia-cli/src/batch.rs`, `oximedia-cli/Cargo.toml`, root `Cargo.toml` [workspace.dependencies]
  - **Tests:** `tests/batch_parallel.rs` — multi-job batch with `-j 0` dry-run; no panic; result order preserved
- [x] Streaming `--progress json` output
  - **Goal:** Each progress tick emits NDJSON to stderr; compatible with `2>&1 | jq` consumers
  - **Design:** `ProgressFormat { Plain, Json }` enum; `set_format()` / `new_with_format()` on `TranscodeProgress` and `BatchProgress`; `--progress` global flag in `Cli`; Json path hides indicatif bar and emits JSON to stderr
  - **Files:** `oximedia-cli/src/progress.rs`, `oximedia-cli/src/main.rs`, `oximedia-cli/src/batch.rs`, `oximedia-cli/src/transcode.rs`
  - **Tests:** `tests/progress_json.rs` — flag accepted; appears in --help; inline progress.rs unit tests

### Documentation

- [x] Expand the crate-level doc comment in `main.rs`
  - **Goal:** 85-subcommand taxonomy grouped by domain in `//!` doc comment; each subcommand gets a one-line description and "see also" link to its `*_cmd.rs` rustdoc
  - **Design:** Groups: Inspection / Transcode+Convert / Audio / Video / Broadcast+Live / MAM / Post / Infrastructure / Compat / Tooling; pure doc change, no runtime effect
  - **Files:** `oximedia-cli/src/main.rs`
  - **Tests:** `cargo doc --no-deps -p oximedia-cli` succeeds with zero warnings
- [ ] Generate a static HTML reference from clap `--help` output and publish
  to `docs.oximedia.rs` / GitHub Pages.
  (Deferred — CI-blocked: would require a new `.github/workflows/docs.yml` which is outside CI policy.)
- [x] Document the plugin search paths honoured by `plugin_cmd`
  - **Goal:** `$OXIMEDIA_PLUGIN_PATH`, loading order, feature-gate requirement documented in rustdoc
  - **Design:** Expand module-level doc comment in `plugin_cmd.rs`; cross-link from `presets/mod.rs`; pure doc change
  - **Files:** `oximedia-cli/src/plugin_cmd.rs`, `oximedia-cli/src/presets/mod.rs`
  - **Tests:** `cargo doc --no-deps -p oximedia-cli` with zero warnings

---

## Known Issues / Gaps

- The `monitor_cmd.rs` module (430 lines) exists on disk but the `Monitor`
  variant is dispatched via `handlers::handle_monitor_command`. Audit whether
  the handler fully delegates into `monitor_cmd` or still carries duplicate
  logic in `handlers.rs` (which is 829 lines — at the 2000-line refactor
  boundary but worth splitting sooner).
- `commands.rs` was split 2026-05-06 into `commands/mod.rs` (1118 lines holding
  the single 90-variant `Commands` enum) plus four per-domain submodules
  (`infrastructure.rs`/`video.rs`/`subtitle.rs`/`compat.rs`) for the nested
  sub-enums. The top-level enum cannot be further split — clap-derive requires
  one `Subcommand` enum to live in one definition.
- Placeholder/stub behaviours enumerated above under "Subcommand-level gaps"
  — each one is a working code path but short of the ecosystem's
  production-grade capability.
- No dedicated `tests/` directory; test coverage lives inside each
  `*_cmd.rs` via `#[cfg(test)]` modules. At least `loudness_cmd`, `batch_cmd`,
  `quality_cmd`, `normalize_cmd` have inline tests writing scratch files
  under `std::env::temp_dir()` per policy.

---

## Future (Post-0.2.0)

| Item                                | Rationale                                                  |
|-------------------------------------|------------------------------------------------------------|
| Shell completions (bash/zsh/fish)   | Tab-complete on 85 subcommands is a huge discoverability win. |
| Man pages via `clap_mangen`         | Offline reference; required by most Linux distro policies. |
| Signed pre-built binaries           | `cargo-dist` + GitHub Actions for all Tier-1 targets.      |
| Homebrew / winget / apt / AUR       | Distribution-channel parity with FFmpeg / MPV.             |
| Full interactive TUI mode           | Run subcommands from inside the TUI, not just browse them. |
| Expanded `oximedia-ff` flag surface | Cover `-filter_complex`, `-progress`, `-hwaccel`, metadata. |
| `oximedia-cv2` companion binary     | Opt-in OpenCV drop-in layered on `oximedia-compat-cv2`.    |
| `oximedia doctor` diagnostic        | Single-command environment audit for support channels.    |
| NDJSON streaming output             | First-class machine-readable progress everywhere.          |
| `assert_cmd`-driven integration CI  | Guarantees every subcommand stays wired as features grow.  |
| Docs site (docs.oximedia.rs)        | Auto-generated from clap metadata on every release.        |
| Crate split under `commands.rs`     | Keep files < 2000 SLOC as the subcommand count climbs.     |

---

*Last updated: 2026-05-06 — v0.1.7, oximedia-cli summary (Run 4 of `/ultra oximedia-cli` LANDED 2026-05-06: handlers.rs split into per-domain submodule, doctor `--full` Phase 1 with codec matrix + plugin path validation + OXICUDA probe, oximedia-compat-cv2 `dnn` module wrapping oxionnx + ORB pipeline followups (BFMatcher / knn_match / mask), 10 new oximedia-cv2 subcommands; bookkeeping flips Refinement 8 done and closes Refinement 5 as WONT-FIX)*

---

## Refinement Proposals (added 2026-05-05 by /ultra)

### Refinement 1 — TUI polish (deferred)
Four items in `interactive_cmd.rs`: real mini-probe render, "run command" tab, mouse + PgUp/PgDn + `/` search, persist cwd nav. Needs interactive terminal capture. Defer to `/ultra tui`.

### Refinement 2 — ffcompat coverage expansion (deferred)
Four items: `-filter_complex` depth, `-map_metadata`, `-hwaccel`, `--explain` mode, `oximedia-cv2` binary. Depends on `oximedia-compat-ffmpeg` API extension. Defer to `/ultra ffcompat`.

### Refinement 3 — Platform packaging (deferred)
`cargo-dist`, Homebrew formula, winget, .deb, RPM, AUR. Outside CI policy (only pypi-publish.yml/npm-publish.yml allowed). Defer to dedicated release-engineering pass.

### Refinement 4 — `commands.rs` 1463-line split
- [x] Done 2026-05-06 — `commands.rs` (1480 lines) replaced by `commands/` module:
  `mod.rs` (1118 lines, the 90-variant `Commands` enum + `parse_key_val`),
  `infrastructure.rs` (102 lines, `MonitorCommand`),
  `video.rs` (118 lines, `RestoreCommand`),
  `subtitle.rs` (137 lines, `CaptionsCommand`),
  `compat.rs` (75 lines, `PresetCommand`).
  All 647 tests pass; clippy clean; `cargo doc` clean.
  The `Commands` enum stays in `mod.rs` as a single 90-variant unit because
  clap-derive `#[derive(Subcommand)]` requires all variants in one definition;
  only the four nested sub-enums were movable. Re-exports keep the
  `crate::commands::Foo` import path stable for `handlers.rs`.

### Refinement 5 — End-to-end ffmpeg golden tests (closed 2026-05-06 — WONT-FIX)
Closed: depending on real `ffmpeg` execution violates the project's Pure Rust Policy (`.cargo/config.toml` and CLAUDE.md). The structured-`TranslateResult` golden suite (71 fixtures in `tests/ff_golden/*.json`, Run 3 Slice B) provides translator-level correctness coverage; pixel-byte comparison against real ffmpeg would require system ffmpeg in CI which is out of scope. If a future user requests interop validation, consider a local-only `tests/ffmpeg_interop.rs` gated by an `OXIMEDIA_FFMPEG_INTEROP=1` env var, but do not auto-run.

### Refinement 6 — Other 6 prior-UU files (verification residue)
- [x] Done 2026-05-06 — verified via `git ls-files --unmerged` (zero output); no remaining unmerged files anywhere in the workspace.
A prior `/ultra` workspace-wide run listed 7 merge-conflict files. Only `image_cmd.rs` was verified clean. The other 6 (`cpu_fallback`, `worker_health_check`, `text`, `aces`, `dash/client`, `context_manager`) were not re-verified. Run `git ls-files --unmerged` to confirm or list any remaining UU files.

### Refinement 7 — oximedia-compat-cv2: `oximedia-py` delegation refactor
After `oximedia-compat-cv2` is stable, a `/ultra cv2-py` pass refactors `crates/oximedia-py/src/cv2_compat` (7,079 LoC across 18 PyO3 modules) to delegate into the new crate. High PyPI compatibility risk. Estimated 800-1,200 LoC glue + parity tests. Defer.

### Refinement 8 — oximedia-compat-cv2: `dnn` module wiring
- [x] (Done 2026-05-06 in Run 4) — `oxionnx = "0.1.2"` already in workspace deps; feature gate `dnn = ["dep:oxionnx"]` already declared in oximedia-compat-cv2/Cargo.toml. Implementation: new `src/dnn.rs` (~480 lines) with `Net` (wrapping `oxionnx::session::Session`), `read_net_from_onnx`, `Net::forward`, `blob_from_image` (BGR→NCHW preprocessing with optional R↔B swap, mean subtract, scale), `nms_boxes` (greedy NMS by score-desc + IoU). 5 tests in `tests/dnn_smoke.rs` covering load-error path, `blob_from_image` correctness, swap_rb correctness, nms IoU correctness, nms empty/no-overlap edge cases. Wire `oximedia-cv2 dnn-forward` CLI subcommand gated `#[cfg(feature = "dnn")]`.

### Refinement 9 — oximedia-compat-cv2: `VideoCapture`/`VideoWriter` for video files
Bridge `oximedia-container::Demuxer` (async) to sync `next_frame()` via `tokio::Handle::current().block_on()`. v1 supports image sequences only. Estimated 300 LoC. Defer.

### Refinement 10 — oximedia-compat-cv2: Real OpenCV interop tests
Pixel-identical comparison against OpenCV reference output. Requires OpenCV in CI — current policy doesn't allow it. Could live in `benches/` as local-only. Defer.

### Refinement 11 — oximedia-compat-cv2: `imshow`/`waitKey` windowing
Feature-gate `windowing = ["winit", "softbuffer"]`. Not core to cv2 API; users typically use external display libs. Estimated 400 LoC. Defer.

### Refinement 12 — oximedia-compat-cv2: Compile-time constants reflection
[x] Done 2026-05-06 — build.rs syn-parses src/constants.rs and emits LIST_CONSTANTS table; binary now iterates the auto-generated table.

### Refinement 13 — oximedia-compat-cv2: ORB BRIEF descriptor
- [x] Done 2026-05-06 — full ORB BRIEF descriptor (256-bit, rotated, Gaussian-smoothed) + Hamming brute-force matcher implemented; orb_create() now returns Ok.
If Slice D's ORB lift has only keypoint detection without BRIEF descriptor extraction + matching, add BRIEF for full ORB feature-parity with cv2. Estimated 200 LoC.

## Run 4 (planned 2026-05-06)

Slice plan from `/Users/kitasan/.claude/plans/parallel-scribbling-nebula.md` (approved
2026-05-06). Six slices, all pure-Rust, all in-repo. No CI yaml, no real ffmpeg.

- [x] **Slice A** — `handlers.rs` (927 lines) → `handlers/` submodule split via splitrs
  - Output: `handlers/{mod,logging,inspect,dispatch,preset_ui,reference}.rs`; old
    `handlers.rs` deleted. Re-exports preserve `crate::handlers::Foo` import paths.
- [x] **Slice B** — `oximedia doctor` Phase 1 expansion (`--full` flag)
  - Adds codec availability matrix, `OXIMEDIA_PLUGIN_PATH` validity check, `OXICUDA_HOME`
    probe. Default output unchanged. ~6 new tests.
- [x] **Slice C** — `oximedia-compat-cv2` `dnn` module + `oximedia-cv2 dnn-forward` (Refinement 8)
  - New `crates/oximedia-compat-cv2/src/dnn.rs`; wraps `oxionnx::Session` with cv2-style
    API (`Net`, `read_net_from_onnx`, `blob_from_image`, `nms_boxes`). ~7 tests.
- [x] **Slice E** — ORB pipeline follow-ups + `oximedia-cv2 orb-detect`
  - `BFMatcher` struct (cv2-idiomatic), `knn_match(k)` for Lowe's ratio test, `mask`
    parameter respected in `Orb::detect_and_compute`. ~6 tests.
- [x] **Slice F** — `oximedia-cv2` binary: 8 already-impl'd cv2 ops wired as subcommands
  - flip, rotate, sobel, laplacian, adaptive-threshold, morphology-ex, fast-corners,
    harris-corners. ~10 tests.
- [x] **Slice G** — Bookkeeping (Run 4 marker flips, Refinement 5 close, two CHANGELOGs)
