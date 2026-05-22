# oximedia-qc TODO

## Current Status
- 38 modules for comprehensive quality control and validation
- Key types: QualityControl, QcPreset, QcRule trait, CheckResult, QcReport
- Modules: audio, audio_qc, batch, bitrate_qc, black_silence, broadcast_safe, caption_qc_checker, closed_caption_qc, codec_validation, color_qc, compliance, compliance_report, container, database, detectors, dolby_vision_qc, examples, file_qc, format, format_qc, hdr_qc, profiles, qc_profile, qc_report, qc_scheduler, qc_template, report, rules, standards, sync_qc, temporal_qc, temporal, tests, utils, video, video_measure, video_quality_metrics
- Feature gates: json (serde), xml (quick-xml), database (rusqlite), pdf
- Dependencies: oximedia-core, oximedia-io, oximedia-container, oximedia-codec, oximedia-timecode, oximedia-audio-analysis, rayon, chrono, bitflags

## Enhancements
- [x] Add auto-fix capability for common QC failures (loudness normalization, bitrate adjustment)
- [x] Implement severity levels in `rules::CheckResult` (error, warning, info) with configurable thresholds (verified 2026-05-16; src/rules.rs:10 Severity enum Info/Warning/Error:16-20, configurable thresholds)
- [x] Extend `broadcast_safe` with region-specific broadcast standards (NTSC/PAL/SECAM color space checks)
- [x] Add `batch` module parallel processing with per-file progress reporting callbacks (verified 2026-05-16; src/batch.rs:60 BatchProcessor, with_parallel_jobs:86, process_files:96, progress tracking:4)
- [ ] Implement `qc_template` inheritance — derive custom templates from built-in presets (verified-open 2026-05-16: qc_template.rs:93 QcTemplate has add_check but no parent/inherit/derive_from mechanism)
- [ ] Extend `dolby_vision_qc` with RPU metadata validation (profile, level, compatibility) (verified-open 2026-05-16: dolby_vision_qc.rs checks light levels and plausibility but no RPU profile/level/compatibility validation)
- [x] Add `bitrate_qc` VBR quality analysis — detect quality dips during high-motion scenes (verified 2026-05-16; src/bitrate_qc.rs:507 check_vbr fn, BitrateMode::Vbr:25, extreme ratio detection:516, 720 lines)
- [x] Implement `sync_qc` lip-sync offset detection with sub-frame accuracy (verified 2026-05-16; src/sync_qc.rs:34 offset_ms:f64 (sub-frame), SyncPoint:32, abs_offset_ms:61, 612 lines)

## New Features
- [x] Add IMF (Interoperable Master Format) compliance checking (verified 2026-05-16; src/imf_compliance.rs:46 CplStructure, ST 2067-2 compliance, 771 lines)
- [x] Implement automated QC report delivery (email, webhook, Slack notification) (verified 2026-05-16; src/qc_delivery.rs:102 DeliveryPayload, WebhookTarget:189, email/slack targets, 869 lines)
- [x] Add SMPTE ST 2067 (IMF) and ST 2084 (PQ) compliance rules (verified 2026-05-16; src/imf_compliance.rs:8 ST 2067-2 and ST 2084 PQ compliance rules)
- [x] Implement QC watch folder — auto-validate files on arrival in monitored directories (verified 2026-05-16; src/qc_watch_folder.rs:27 WatchFolderConfig, QcJobResult:70, 431 lines)
- [x] Add QC comparison mode — diff two files and highlight quality differences (verified 2026-05-16; src/qc_compare.rs:69 RuleCompareResult, QcComparator, is_regression:52, 589 lines)
- [x] Implement `caption_qc_checker` with timing gap/overlap detection and reading speed validation
- [x] Add network stream QC — validate live RTMP/SRT/HLS streams in real-time (verified 2026-05-16; src/stream_qc.rs:58 StreamSample, RTMP/SRT/HLS:29-44, 703 lines)
- [ ] Implement PDF report generation in `report` module (feature-gated) (verified-open 2026-05-16: report.rs exists; pdf feature gate listed in Cargo.toml but no PDF generation implemented)

## Performance
- [ ] Use SIMD for pixel-level analysis in `video_quality_metrics` (black frame, freeze frame detection)
- [ ] Implement early termination in `batch` — skip remaining checks after critical failure (configurable)
- [ ] Cache decoded frames across multiple video checks to avoid redundant decoding
- [ ] Parallelize independent QC rules using rayon task parallelism within single-file validation

## Testing
- [ ] Add QC validation tests with known-good and known-bad reference media files
- [ ] Test `compliance` module against all supported broadcast standards (ATSC, DVB, ISDB)
- [ ] Add round-trip tests for `qc_report` JSON/XML serialization
- [ ] Test `qc_scheduler` with concurrent QC jobs and verify no resource contention
- [ ] Verify `black_silence` detection thresholds match industry-standard definitions

## Documentation
- [ ] Add QC rule writing guide for custom rule implementation via `QcRule` trait
- [ ] Document built-in QC presets with their included rules and thresholds
- [ ] Add QC integration guide for CI/CD pipelines (automated media validation)
