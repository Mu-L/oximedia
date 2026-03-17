# oximedia-switcher TODO

## Current Status
- 41 source files covering a professional live production video switcher
- Core: Switcher engine with configurable M/E rows, bus management (program/preview/aux), input routing, frame synchronization
- Keying: chroma key, luma key, upstream/downstream keyers, pattern generator
- Transitions: cut, mix/dissolve, wipe (horizontal/vertical/diagonal), DVE, transition engine
- Tally: tally state management, tally protocol, tally system
- Macros: macro engine, macro execution, macro system with command types (SelectProgram, Cut, Auto, SetKeyer, Wait, RunMacro)
- Media: media pool, media player, still store
- Audio: audio follow video, audio mixer, clip delay
- Other: multiviewer, super source, crosspoint matrix, input bank, output routing, FTB control, preview bus, switcher presets
- Dependencies: oximedia-core, oximedia-codec, oximedia-graph, oximedia-mixer, oximedia-timecode, serde, tokio

## Enhancements
- [ ] Add DVE (Digital Video Effects) fly-key support with position/scale/rotation keyframe animation in `dve` module
- [ ] Extend `chroma::ChromaKey` with spill suppression and edge softening parameters
- [ ] Add stinger transition support in `transition_engine` (animated overlay-based transition)
- [ ] Implement T-bar manual transition control in `TransitionEngine` with position value (0.0-1.0)
- [ ] Add `input_manager` hot-plug detection for dynamically adding/removing inputs during production
- [ ] Extend `macro_engine` with conditional branching (if/else based on tally state or input availability)
- [ ] Add transition preview rendering in `preview_bus` showing upcoming transition result before execution
- [ ] Implement `switcher_preset` save/recall with named presets and smooth recall animation

## New Features
- [ ] Implement `virtual_input` module for generating color bars, test patterns, countdown timers, and text overlays
- [ ] Add `graphics_overlay` module for CG (Character Generator) lower thirds, titles, and logos
- [ ] Implement `replay_server` module for instant replay with variable speed playback (slow-mo, fast-forward)
- [ ] Add `multi_me_link` module for linking M/E rows together (cascade mode for complex productions)
- [ ] Implement `atem_protocol` module for network control compatibility with Blackmagic ATEM protocol
- [ ] Add `ndi_input` module for NDI network video input support (feature-gated)
- [ ] Implement `recording` module for simultaneous program output recording with codec selection
- [ ] Add `intercom` module for production intercom/IFB communication channel management

## Performance
- [ ] Implement lock-free tally state updates in `tally` using atomic operations for real-time tally distribution
- [ ] Add frame buffer pool in `FrameSynchronizer` to eliminate per-frame allocation
- [ ] Use SIMD-accelerated alpha blending in `keyer` for compositing keyed layers onto program output
- [ ] Implement zero-copy frame passing between `bus`, `keyer`, and `transition_engine` using Arc<Frame>
- [ ] Add async output routing in `output_routing` for non-blocking multiviewer rendering

## Testing
- [ ] Add integration test for full switcher lifecycle: create -> set program/preview -> cut -> auto transition -> process frames -> verify tally
- [ ] Test macro recording and playback with all MacroCommand variants and verify state matches direct execution
- [ ] Add concurrent access test: multiple threads calling set_program/set_preview/cut simultaneously
- [ ] Test chroma key with synthetic green-screen frames and verify alpha mask accuracy
- [ ] Add transition completion test: start auto transition, process N frames, verify program/preview swap
- [ ] Test FTB (Fade to Black) control with verify that program output reaches full black

## Documentation
- [ ] Add switcher signal flow diagram showing bus architecture, M/E rows, keyers, and output routing
- [ ] Document macro command reference with all supported MacroCommand variants and parameters
- [ ] Add example showing professional 2-M/E production workflow with keyers and transitions
