# oximedia-timeline TODO

## Current Status
- 50 source files across 48 public modules providing a professional multi-track timeline editor
- Core: Timeline, Track (video/audio), Clip with MediaSource, Position/Duration/Speed types
- Editing: EditMode/EditOperation, razor tool, snap grid, clip sequence, compound clip, compound region
- Effects: EffectStack with keyframe animation (Keyframe, KeyframeInterpolation, KeyframeValue), keyframe_animation
- Transitions: TransitionType (dissolve, etc.), TransitionEngine, transition alignment
- Multi-camera: multicam editing, nested sequences (nested, nested_timeline, nested_compound)
- Export/Import: EDL, XML, AAF via timeline_exporter; export, export_settings, import modules
- Rendering: renderer (PixelBuffer, RenderedFrame, FrameRenderSettings), render, render_queue
- Playback: async playback (non-wasm), cache module
- Other: markers, metadata, clip_metadata, audio mixer, timecode, timeline_diff, timeline_event, timeline_lock, track_color, track_group, track_routing, gap_filler, conform, version_snapshot, color_correction_track, sequence_range
- Dependencies: oximedia-core, oximedia-container, oximedia-graph, oximedia-edl, oximedia-aaf, oximedia-audio, tokio, serde, uuid, parking_lot, chrono, dashmap, quick-xml

## Enhancements
- [ ] Add three-point and four-point editing support in `edit::EditOperation` (mark in/out on source and timeline)
- [ ] Extend `multicam` to support angle switching based on timecode match (not just frame position)
- [x] Improve `snap_grid` to support magnetic snap to clip boundaries, markers, and playhead position
- [ ] Add `clip_metadata` support for embedded media metadata (codec info, color space, audio channels)
- [ ] Extend `transition_engine` to support asymmetric transitions (different in/out curves)
- [ ] Add linked clip groups in `track` where moving one clip auto-moves linked clips on other tracks
- [ ] Implement `timeline_lock` per-track locking (currently likely timeline-level) for collaborative editing
- [x] Extend `keyframe_animation` with bezier curve interpolation for smooth easing

## New Features
- [ ] Implement `proxy_workflow` module for offline/online editing with proxy media and auto-conform to full resolution
- [x] Add `adjustment_layer` module for applying effects to all tracks below without modifying individual clips
- [x] Implement `freeze_frame` module for creating freeze frame clips from any point in a source clip
- [ ] Add `speed_ramp` module for variable-speed effects with keyframeable speed curves (optical flow interpolation)
- [ ] Implement `multi_select` module for group operations on multiple clips (move, trim, delete, copy)
- [ ] Add `auto_sequence` module for automatic assembly editing based on subclip markers
- [ ] Implement `fcpxml_export` module for Final Cut Pro XML timeline interchange format
- [ ] Add `otio_export` module for OpenTimelineIO format support (industry-standard timeline interchange)
- [ ] Implement `timeline_compare` module for diffing two timeline versions with visual conflict resolution

## Performance
- [x] Implement interval tree in `timeline` for O(log n) clip lookup by time position instead of linear scan
- [ ] Add frame cache warming in `playback` that pre-renders upcoming frames based on playhead velocity
- [ ] Use memory-mapped audio file access in `audio` mixer for large project with many audio clips
- [ ] Implement incremental render in `renderer` that only re-composites tracks with changed clips
- [ ] Add lazy clip loading in `import` that defers media probing until clip is first accessed on timeline
- [ ] Parallelize multi-track rendering in `render` using rayon for compositing independent track layers

## Testing
- [ ] Add integration test for full edit workflow: create timeline -> add tracks -> insert clips -> add transition -> export EDL -> verify output
- [ ] Test `razor_tool` split at various positions (clip start, middle, end, on transition boundary)
- [ ] Add `nested_timeline` depth test with 3+ levels of nesting and verify correct frame resolution
- [ ] Test `multicam` angle switching with 4+ camera angles and verify correct source selection per cut
- [ ] Add round-trip test: Timeline -> EDL export -> EDL import -> compare timelines for equivalence
- [ ] Test `timeline_diff` with known edit operations and verify diff correctly identifies changes

## Documentation
- [ ] Add NLE (Non-Linear Editor) concepts guide explaining tracks, clips, transitions, and keyframes
- [ ] Document supported import/export formats with feature matrix (EDL, XML, AAF, FCPXML, OTIO)
- [ ] Add example showing multi-camera editing workflow from sync to final output
