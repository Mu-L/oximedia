# oximedia-playlist TODO

## Current Status
- 39 modules covering broadcast playlist, scheduling, automation, transitions, EPG, and more
- Key features: PlayoutEngine, ClockSync, ScheduleEngine, crossfade, commercial breaks (SCTE-35)
- Modules include: automation, backup, clock, commercial, continuity, crossfade, epg, gap_filler, interstitial, live, multichannel, playlist_diff/export/filter/health/merge/priority/rules/segment/stats/sync/tempo/validator, queue_manager, recommendation_engine, repeat_policy, schedule, secondary, shuffle, smart_play, track_metadata, track_order, transition
- Dependencies: oximedia-core, oximedia-timecode, tokio, serde, chrono, tracing

## Enhancements
- [ ] Add undo/redo support for playlist editing operations in `playlist` module
- [ ] Implement playlist versioning with diff tracking in `playlist_diff`
- [ ] Extend `crossfade` module with logarithmic and equal-power crossfade curves
- [ ] Add playlist validation for total duration vs. scheduled time window in `playlist_validator`
- [ ] Implement weighted shuffle algorithm in `shuffle` module to bias by genre/mood
- [ ] Enhance `gap_filler` with content-aware filler selection (match genre/mood of surrounding items)
- [ ] Add conflict resolution strategies to `playlist_merge` (priority, interleave, overlap trim)
- [ ] Extend `commercial` module with SCTE-35 splice_insert and time_signal command generation

## New Features
- [ ] Add M3U8/M3U playlist import/export support (parse and generate)
- [ ] Implement playlist archival with compression for long-term storage
- [ ] Add real-time playlist notification system (webhook/callback on item transitions)
- [ ] Implement playlist rotation policies (daily, weekly, seasonal schedules)
- [ ] Add A/B testing support for playlist ordering strategies
- [ ] Implement playlist analytics dashboard data export (JSON/CSV metrics)
- [ ] Add multi-language EPG metadata support in `epg` module

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
