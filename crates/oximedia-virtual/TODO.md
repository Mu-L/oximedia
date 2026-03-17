# oximedia-virtual TODO

## Current Status
- 91 source files covering virtual production and LED wall tools
- Key features: VirtualProduction main system, camera tracking/calibration, LED wall rendering with perspective correction, ICVFX compositing, color pipeline management, genlock synchronization, motion capture integration, green screen keying, Unreal Engine integration, multi-camera support
- Submodules: math (linear algebra), color (pipeline), greenscreen, icvfx (composite), keying, led (render), lens, mocap, multicam (manager), preview, sync (genlock), tracking (camera), unreal, virtual_set
- Additional: background_plate, camera_frustum, camera_rig, frustum_culling, frustum, led_volume, led_wall, light_rig, metrics, motion_path, ndi_bridge, panel_topology, pixel_mapping, projection_map, render_layer, render_output, scene, scene_setup, stage, stage_layout, stage_manager, talent_keying, tracking_data, tracking_session, volume_calibration, workflows, virtual_studio
- Dependencies: thiserror, serde, serde_json (minimal -- no external media deps)

## Enhancements
- [ ] Integrate with oximedia-core frame types instead of standalone data structures for interop with other crates
- [ ] Extend `icvfx::composite` to support multi-layer compositing (foreground talent + LED content + CG overlay)
- [ ] Add real lens distortion models (Brown-Conrady, fisheye) to `lens` module beyond basic radial correction
- [x] Implement actual frustum-to-LED-panel UV mapping in `projection_map` with sub-pixel accuracy
- [x] Extend `color::pipeline` with ACES color management support (ACEScg working space, RRT+ODT transforms)
- [x] Add latency measurement and compensation in `sync::genlock` for tracking-to-render pipeline
- [ ] Improve `mocap` integration with standard mocap data formats (BVH, C3D, FBX skeleton import)
- [x] Extend `multicam::manager` with automatic camera selection based on talent tracking position
- [x] Add thermal drift compensation to `volume_calibration` for LED panels that shift color temperature over time

## New Features
- [ ] Implement `ar_overlay` module for augmented reality marker-based object placement on live camera feed
- [ ] Add `stage_visualization` module for 3D wireframe preview of stage layout, LED panels, and camera positions
- [ ] Implement `talent_tracking` module using 2D pose estimation for automatic talent masking without green screen
- [ ] Add `set_extension` module for extending physical sets with virtual elements beyond LED wall boundaries
- [ ] Implement `hdri_capture` module for capturing real-world lighting and applying to virtual scenes
- [ ] Add `previz` module for pre-visualization workflows (storyboard-to-virtual-set blocking)
- [ ] Implement `remote_session` module for remote virtual production monitoring and control over network
- [ ] Add `stage_safety` module for tracking safe zones, warning when talent/equipment approach LED wall boundaries

## Performance
- [ ] Implement GPU-accelerated frustum rendering in `led::render` for real-time 4K+ LED wall content
- [x] Add frame prediction in `camera_tracking` to compensate for tracking latency with motion extrapolation
- [ ] Optimize `pixel_mapping` with lookup table caching for LED panel pixel-to-UV mapping
- [ ] Implement tile-based rendering in `icvfx::composite` for parallel compositing of independent screen regions
- [ ] Add LOD (level-of-detail) system in `render_layer` for rendering distant virtual objects at lower resolution
- [ ] Profile `math` module and add SIMD-optimized matrix multiplication for 4x4 transforms

## Testing
- [ ] Add integration test for full pipeline: camera_tracker -> frustum -> led_render -> compositor
- [ ] Test `genlock` synchronization accuracy with simulated frame timing jitter
- [ ] Add calibration round-trip tests: calibrate LED panel, render test pattern, verify pixel accuracy
- [ ] Test `multicam::manager` camera switching with concurrent tracking data from multiple cameras
- [ ] Add stress test for `stage_manager` with 100+ LED panels and 8 tracked cameras
- [ ] Test `color::pipeline` color accuracy with known reference color checker values

## Documentation
- [ ] Document virtual production stage setup guide (LED panel arrangement, camera placement, calibration workflow)
- [ ] Add architecture diagram showing data flow from tracking system through render to LED output
- [ ] Document Unreal Engine integration protocol and message format in `unreal` module
