# oximedia-multicam TODO

## Current Status
- 38 source files/directories for multi-camera synchronization, editing, and switching
- Sync: temporal, audio cross-correlation, timecode (LTC/VITC/SMPTE), visual markers, genlock, drift correction
- Editing: MultiCamTimeline, angle switching, transitions, edit decision lists
- Auto switching: AI-based camera selection, rules engine (speaker detection, action following, shot variety)
- Composition: picture-in-picture, split-screen, grid layouts
- Color matching and spatial alignment/stitching
- ISO recording, replay buffer, tally system, bank control, coverage mapping

## Enhancements
- [x] Implement actual audio cross-correlation in sync module using FFT-based correlation (currently stubbed)
- [x] Add sub-frame sync precision in angle_sync -- interpolate between frames for <1 frame accuracy (verified 2026-05-16; src/sub_frame_sync.rs:110 SubFrameSync, parabolic interpolation:13)
- [x] Improve auto switcher scoring in angle_score to weight composition quality using rule of thirds detection
- [x] Add smooth transition blending in edit module -- dissolve/wipe between angles during switch points
- [x] Implement genlock_master drift measurement and correction loop for long-form recording (>1 hour) (verified 2026-05-16; src/genlock_master.rs:486 GenlockMaster drift_threshold_ns:144, drifted_count tracking:119)
- [x] Add multi-angle audio selection -- allow independent audio source selection separate from video angle (verified 2026-05-16; src/audio_selection.rs:100 MultiAngleMixer, independent audio_angle:112)
- [x] Extend color matching in color module to handle LOG/RAW footage with LUT-based matching (verified 2026-05-16; src/color/match_color.rs:199 3D LUT-based transfer variant)

## New Features
- [x] Implement automatic highlight detection using cut_analysis + audio energy peaks for sports/events
- [x] Add multi-angle proxy generation for lightweight preview of all angles simultaneously (verified 2026-05-16; src/proxy_generator.rs:87 MultiAngleProxyGenerator, src/proxy_gen.rs)
- [x] Implement XML export (Final Cut Pro XML, AAF) in multicam_export for NLE interchange (verified 2026-05-16; src/fcp_xml.rs:395 FCP XML export, src/multicam_export.rs)
- [x] Add remote camera control protocol support (VISCA over IP) in addition to existing PTZ concepts (verified 2026-05-16; src/visca.rs:454 VISCA over UDP/TCP, VISCA_COMMAND encoding)
- [x] Implement automatic camera framing suggestions based on face detection and composition rules (verified 2026-05-16; src/framing_suggest.rs:107 FramingSuggestion, face detection:18)
- [x] Add multi-camera audio mixing -- combine boom mic, lavaliers, and camera audio with automatic switching (verified 2026-05-16; src/audio_selection.rs:73 audio mix config, boom-mic reference:5)
- [ ] Implement virtual camera output (v4l2/AVFoundation loopback) for live production preview (verified-open 2026-05-16: no v4l2/AVFoundation loopback module found)

## Performance
- [x] Parallelize sync verification across all angle pairs using rayon in sync_verify (verified 2026-05-16; src/sync_verify_parallel.rs:158 par_iter() across angle pairs)
- [ ] Cache angle score computations per frame to avoid redundant face/motion detection in auto switcher (verified-open 2026-05-16: no score cache in angle_score.rs)
- [ ] Implement lazy frame loading in MultiCamTimeline -- only decode frames for active/preview angles (verified-open 2026-05-16: no lazy frame loading found)
- [ ] Use tile-based color matching in color module to reduce full-frame processing to sampled regions (verified-open 2026-05-16: no tile/sampled region in color/match_color.rs)
- [ ] Add incremental coverage_map updates instead of full recomputation when angles change (verified-open 2026-05-16: no incremental update in coverage_map.rs)

## Testing
- [x] Test audio sync with known-offset synthetic signals (1kHz tone with 100ms offset between channels)
- [ ] Add test for auto switcher with synthetic frame scores verifying angle selection logic
- [ ] Test PIP composition output dimensions and placement for all corner positions
- [ ] Verify tally_system state transitions: preview -> program -> off lifecycle
- [ ] Test ISO recording with simulated multi-angle input verifying per-angle file output

## Documentation
- [ ] Add multi-camera production workflow guide (setup -> sync -> edit -> export)
- [ ] Document the auto-switching rules engine with examples for common production scenarios
- [ ] Add architectural diagram showing the relationship between sync, edit, auto, composite modules
