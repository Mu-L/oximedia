# oximedia-graphics TODO

## Current Status
- 46+ modules implementing a broadcast graphics engine
- Core rendering: primitives, render, shape_render, path_builder, svg_renderer
- Broadcast elements: lower_third, ticker, scoreboard, clock_widget, countdown_timer, weather_widget, virtual_set
- Animation: animation, animation_curve, keyframe, transitions, transition_wipe, particles
- Text: text, text_renderer, text_layout, bitmap_font, font_metrics
- Color: color, color_blend, color_grade, color_picker, gradient_fill, hdr_composite, lut_apply
- Compositing: overlay, mask_layer, effects, sprite_sheet
- GPU: wgpu-based GPU rendering (feature-gated), WGSL shaders
- Server: axum-based control server (feature-gated)
- Dependencies: tiny-skia, fontdue, ab_glyph, kurbo, palette, wgpu, tera

## Enhancements
- [x] Add anti-aliased rendering to `shape_render.rs` for smoother vector output at broadcast resolution
- [x] Extend `animation_curve.rs` with spring physics and damped oscillation easing functions
- [x] Add multi-line text wrapping with justification modes to `text_layout.rs` (verified 2026-05-16; src/text_layout.rs:44 TextAlign::Justify, justify multi-line:627, test:926)
- [x] Implement text outline and drop shadow directly in `text_renderer.rs` (not as post-effect) (verified 2026-05-16; src/text_renderer.rs:272 TextOutline struct, DropShadow:319)
- [x] Add real-time data binding to `scoreboard.rs` (live score updates via data source trait) (verified 2026-05-16; src/scoreboard.rs:369 ScoreDataSource trait, LiveScoreboard:426)
- [x] Extend `ticker.rs` with variable scroll speed, pause-on-item, and looping modes (verified 2026-05-16; src/ticker.rs:101 scroll_speed_pps, pause_on_item:115, item_speed_multiplier:239)
- [x] Add configurable safe-area margins to `layout_engine.rs` for broadcast compliance (verified 2026-05-16; src/layout_margins.rs:152 SafeAreaMargins, BroadcastStandard, SafeRect)
- [x] Implement sub-pixel font rendering in `font_metrics.rs` for sharper text at small sizes (verified 2026-05-16; src/font_metrics.rs:279 SubPixelMode enum, SubPixelMetrics:388)
- [x] Add motion blur to `particles.rs` for more realistic particle trails (verified 2026-05-16; src/particles.rs:318 render_to_rgba_motion_blur)

## New Features
- [x] Add a `chroma_key.rs` module for green/blue screen keying with spill suppression
- [x] Implement a `picture_in_picture.rs` module with configurable position, size, border, and shadow
- [x] Add a `crawl.rs` module for vertical text crawl (end credits style) (verified 2026-05-16; src/crawl.rs:1021 lines)
- [x] Implement a `data_visualization.rs` module for live charts/graphs (bar, line, pie) in broadcast
- [x] Add a `stinger_transition.rs` module for animated full-screen transitions with alpha channel (verified 2026-05-16; src/stinger_transition.rs:465 lines)
- [x] Implement a `logo_bug.rs` module for persistent corner logo placement with fade in/out (verified 2026-05-16; src/logo_bug.rs:504 lines)
- [x] Add an `audio_waveform.rs` module for rendering audio waveform/spectrum visualizations (verified 2026-05-16; src/audio_waveform.rs:668 lines)
- [x] Implement `template_variables.rs` for tera template variable hot-reload from JSON/API (verified 2026-05-16; src/template_variables.rs:484 lines)

## Performance
- [ ] Batch draw calls in `render.rs` to reduce per-primitive overhead
- [ ] Implement dirty-region tracking in `overlay.rs` to only re-render changed areas
- [ ] Add GPU-accelerated text rendering path in `gpu.rs` using glyph atlas textures
- [ ] Cache rasterized glyphs in `bitmap_font.rs` to avoid redundant rasterization per frame
- [ ] Parallelize `gradient_fill.rs` scanline computation with rayon for large surfaces
- [ ] Add texture atlas for `sprite_sheet.rs` to reduce GPU texture switches

## Testing
- [ ] Add visual regression tests for `lower_third.rs`, `ticker.rs`, and `scoreboard.rs` output
- [ ] Test `animation.rs` keyframe interpolation at boundary conditions (t=0.0, t=1.0, overshoot)
- [ ] Add tests for `hdr_composite.rs` tone mapping correctness against reference values
- [ ] Test `svg_renderer.rs` with complex paths including curves, arcs, and clip regions
- [ ] Add GPU rendering integration tests that verify `gpu.rs` output matches software path

## Documentation
- [ ] Add visual examples (rendered PNGs) for each broadcast element in module docs
- [ ] Document the tera template variable schema used by `graphic_template.rs`
- [ ] Add a guide for creating custom broadcast graphics presets using `presets.rs`

## Wave 4 Progress (2026-04-18)
- [x] wasm-gpu-compute-fix: cfg-gate gpu_compute.rs Send+Sync assumptions for wasm32 — Wave 4 Slice B
