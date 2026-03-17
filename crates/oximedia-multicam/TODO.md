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
- [ ] Add sub-frame sync precision in angle_sync -- interpolate between frames for <1 frame accuracy
- [x] Improve auto switcher scoring in angle_score to weight composition quality using rule of thirds detection
- [x] Add smooth transition blending in edit module -- dissolve/wipe between angles during switch points
- [ ] Implement genlock_master drift measurement and correction loop for long-form recording (>1 hour)
- [ ] Add multi-angle audio selection -- allow independent audio source selection separate from video angle
- [ ] Extend color matching in color module to handle LOG/RAW footage with LUT-based matching

## New Features
- [x] Implement automatic highlight detection using cut_analysis + audio energy peaks for sports/events
- [ ] Add multi-angle proxy generation for lightweight preview of all angles simultaneously
- [ ] Implement XML export (Final Cut Pro XML, AAF) in multicam_export for NLE interchange
- [ ] Add remote camera control protocol support (VISCA over IP) in addition to existing PTZ concepts
- [ ] Implement automatic camera framing suggestions based on face detection and composition rules
- [ ] Add multi-camera audio mixing -- combine boom mic, lavaliers, and camera audio with automatic switching
- [ ] Implement virtual camera output (v4l2/AVFoundation loopback) for live production preview

## Performance
- [ ] Parallelize sync verification across all angle pairs using rayon in sync_verify
- [ ] Cache angle score computations per frame to avoid redundant face/motion detection in auto switcher
- [ ] Implement lazy frame loading in MultiCamTimeline -- only decode frames for active/preview angles
- [ ] Use tile-based color matching in color module to reduce full-frame processing to sampled regions
- [ ] Add incremental coverage_map updates instead of full recomputation when angles change

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
