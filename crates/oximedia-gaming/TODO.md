# oximedia-gaming TODO

## Current Status
- 80 source files across capture, encoding, audio, input, overlay, scene, replay, platform, metrics, and streaming domains
- GameStreamer with lifecycle management (start/stop/pause/resume)
- Capture: screen, window, region, cursor, game hooks
- Encoding: NVENC, QSV, VCE hardware presets, low-latency mode
- Audio: game capture, microphone, music mixing
- Input: keyboard, mouse, controller capture with overlay rendering
- Overlay: alerts, scoreboards, widgets, HUD, stream overlay
- Scene: manager, hotkeys, transitions
- Replay: buffer, highlight detection, save/export
- Platform: Twitch, YouTube, Facebook Gaming integration
- Additional: chat integration, monetization, tournament, spectator mode, VOD manager
- Dependencies: oximedia-core, oximedia-codec, oximedia-audio, oximedia-graph, tokio

## Enhancements
- [x] Implement actual screen capture in `capture/screen.rs` (currently stub) (verified 2026-05-16; src/capture/screen.rs:478 lines ScreenCapture)
- [x] Implement actual hardware encoder integration in `encode/nvenc.rs`, `encode/qsv.rs`, `encode/vce.rs` (verified 2026-05-16; src/encode/nvenc.rs:771 lines NvencEncoder)
- [x] Make `GameStreamer::get_stats()` return real metrics instead of hardcoded values (verified 2026-05-16; src/lib.rs:612 get_stats() real metrics comment, test_stream_stats_with_real_metrics:864)
- [x] Implement actual replay buffer ring-buffer storage in `replay/buffer.rs` (verified 2026-05-16; src/replay/buffer.rs:24 ReplayBuffer ring-buffer:3)
- [ ] Add `save_replay()` actual file writing with encoding in `replay/save.rs` (verified-open 2026-05-16: not yet verified as fully implemented)
- [ ] Enhance `highlight/detector.rs` with configurable detection thresholds per game genre (verified-open 2026-05-16: not yet implemented)
- [x] Add `frame_pacing.rs` adaptive frame pacing that adjusts to encoder backpressure (verified 2026-05-16; src/frame_pacing.rs:349 lines, backpressure in async_encoder.rs:101)
- [x] Implement `network_quality.rs` real-time bitrate adaptation based on network conditions (verified 2026-05-16; src/network_quality.rs:398 lines NetworkQualityMonitor)
- [x] Add `stream_analytics.rs` viewer count, chat activity, and engagement metrics (verified 2026-05-16; src/stream_analytics.rs:372 lines StreamAnalytics)
- [ ] Enhance `webcam/chroma.rs` with edge refinement for better green screen keying (verified-open 2026-05-16: not yet implemented)

## New Features
- [x] Implement RTMP/SRT/WHIP output protocol support for actual streaming
- [x] Add multi-platform simultaneous streaming (Twitch + YouTube + Facebook at once) (verified 2026-05-16; src/multi_stream.rs:1308 lines MultiStreamManager)
- [ ] Implement custom stinger transition support in `scene/transition.rs` (verified-open 2026-05-16: no StingerTransition in scene/transition.rs)
- [x] Add stream deck / hotkey integration for scene switching and actions
- [x] Implement AI-free game event detection using audio cues (kill sounds, announcements) (verified 2026-05-16; src/game_event.rs:331 lines GameEventDetector)
- [x] Add recording-only mode with higher quality settings than live streaming
- [x] Implement `clip_manager.rs` automatic clip creation from highlight markers (verified 2026-05-16; src/clip_manager.rs:1247 lines ClipManager)
- [x] Add `spectator_mode.rs` multi-POV spectator stream with camera switching (verified 2026-05-16; src/spectator_mode.rs:480 lines SpectatorMode)
- [x] Implement chat bot integration in `chat_integration.rs` with command handling (verified 2026-05-16; src/chat_integration.rs:409 lines ChatIntegration)

## Performance
- [ ] Add GPU-based frame scaling and color conversion via oximedia-gpu (verified-open 2026-05-16: no oximedia-gpu integration in capture/encode path)
- [x] Implement zero-copy frame pipeline from capture to encoder (verified 2026-05-16; src/zero_copy_pipeline.rs:815 lines ZeroCopyPipeline)
- [x] Add frame dropping strategy in `pacing/frame.rs` when encoder cannot keep up (verified 2026-05-16; src/async_encoder.rs:101 drop_on_backpressure)
- [ ] Optimize `overlay/system.rs` rendering with dirty-region compositing (verified-open 2026-05-16: not yet implemented)
- [ ] Implement async encoder output with double-buffered frame submission (verified-open 2026-05-16: not yet implemented)
- [ ] Add memory-mapped ring buffer for replay storage to avoid copying frames (verified-open 2026-05-16: not yet implemented)

## Testing
- [ ] Add integration test for full streaming pipeline: capture -> encode -> mux -> output
- [ ] Test `StreamConfigBuilder` validation with edge cases (1x1 resolution, 1 fps)
- [ ] Test scene switching with active transitions and verify no dropped frames
- [ ] Add `replay/buffer.rs` ring buffer overflow tests with various durations
- [ ] Test `platform/twitch.rs` metadata API integration with mock responses
- [ ] Test `audio/mix.rs` multi-source mixing with level normalization
- [ ] Add latency measurement tests verifying <100ms glass-to-glass target

## Documentation
- [ ] Document the streaming pipeline architecture from capture to output
- [ ] Add encoder preset comparison table (latency, quality, CPU/GPU usage)
- [ ] Document platform-specific configuration requirements (Twitch ingest, YouTube key, etc.)
