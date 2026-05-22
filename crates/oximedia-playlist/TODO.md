# oximedia-playlist TODO

## Current Status
- 39 modules covering broadcast playlist, scheduling, automation, transitions, EPG, and more
- Key features: PlayoutEngine, ClockSync, ScheduleEngine, crossfade, commercial breaks (SCTE-35)
- Modules include: automation, backup, clock, commercial, continuity, crossfade, epg, gap_filler, interstitial, live, multichannel, playlist_diff/export/filter/health/merge/priority/rules/segment/stats/sync/tempo/validator, queue_manager, recommendation_engine, repeat_policy, schedule, secondary, shuffle, smart_play, track_metadata, track_order, transition
- Dependencies: oximedia-core, oximedia-timecode, tokio, serde, chrono, tracing

## Enhancements
- [x] Add undo/redo support for playlist editing operations in `playlist` module (verified 2026-05-16; src/playlist/undo.rs, src/playlist_history.rs:23 PlaylistEditOperation undo/redo:336)
- [x] Implement playlist versioning with diff tracking in `playlist_diff` (verified 2026-05-16; src/playlist_diff.rs:75 PlaylistDiff, src/playlist_archive.rs:145 PlaylistArchive versioned snapshots)
- [x] Extend `crossfade` module with logarithmic and equal-power crossfade curves (verified 2026-05-16; src/crossfade.rs:70 Logarithmic/EqualPower crossfade variants, test:366)
- [x] Add playlist validation for total duration vs. scheduled time window in `playlist_validator` (verified 2026-05-16; src/playlist_validator.rs:127 max_total_duration, test_total_duration_violation:327)
- [x] Implement weighted shuffle algorithm in `shuffle` module to bias by genre/mood (verified 2026-05-16; src/shuffle.rs:63 WeightedShuffler, weighted_shuffle fn:89)
- [ ] Enhance `gap_filler` with content-aware filler selection (match genre/mood of surrounding items) (verified-open 2026-05-16: no genre/mood matching in gap_filler.rs)
- [x] Add conflict resolution strategies to `playlist_merge` (priority, interleave, overlap trim) (verified 2026-05-16; src/playlist_merge.rs:98 PlaylistMergeEngine, MergeStrategy::PriorityMerge/Interleave:285)
- [x] Extend `commercial` module with SCTE-35 splice_insert and time_signal command generation (verified 2026-05-16; src/commercial/scte35.rs:3 SpliceInsert:161, TimeSignal:179)

## New Features
- [x] Add M3U8/M3U playlist import/export support (parse and generate) (verified 2026-05-16; src/m3u.rs M3uPlaylist parser/writer, src/m3u8.rs M3U8 format)
- [x] Implement playlist archival with compression for long-term storage (verified 2026-05-16; src/playlist_archive.rs:145 PlaylistArchive versioned immutable snapshots)
- [x] Add real-time playlist notification system (webhook/callback on item transitions) (verified 2026-05-16; src/playlist_notify.rs:241 notify, PlaylistEventKind:408 lines)
- [x] Implement playlist rotation policies (daily, weekly, seasonal schedules) (verified 2026-05-16; src/playlist_rotation.rs:18 RotationStrategy, RotationPool:102, Daypart:111)
- [ ] Add A/B testing support for playlist ordering strategies (verified-open 2026-05-16: no ab_test/AbVariant module found in playlist)
- [ ] Implement playlist analytics dashboard data export (JSON/CSV metrics) (verified-open 2026-05-16: no analytics_export in playlist_stats.rs)
- [ ] Add multi-language EPG metadata support in `epg` module (verified-open 2026-05-16: no multilang/multilingual in epg/ modules)

## Performance
- [ ] Cache computed playlist statistics in `playlist_stats` to avoid recalculation
- [ ] Optimize `recommendation_engine` similarity computation with pre-computed feature vectors
- [ ] Use interval trees in `schedule` for O(log n) overlap detection instead of linear scan
- [ ] Batch database writes in `play_history` for high-throughput playout logging

## Testing
- [ ] Add integration tests for full playlist lifecycle (create, schedule, play, log)
- [ ] Test `crossfade` module with edge cases (zero-duration items, single-item playlists)
- [ ] Add stress tests for `multichannel` module with 50+ simultaneous channels
- [ ] Test `clock` synchronization accuracy under simulated clock drift

## Documentation
- [ ] Document SCTE-35 integration workflow with real-world examples
- [ ] Add EPG generation tutorial with XMLTV output format specification
- [ ] Document playlist state machine transitions (idle -> scheduled -> playing -> completed)
