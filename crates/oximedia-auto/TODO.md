# oximedia-auto TODO

## Current Status
- 55 modules providing automated video editing: highlights, cuts, assembly, rules, scoring, pacing curves, subject tracking, reframing, and more
- Core modules: `smart_crop`, `smart_reframe` (+ `SubjectTracker`, `VerticalToHorizontalParams`), `smart_trim`, `music_sync`, `tempo_detect`, `narrative`, `scene_classifier`, `color_match`, `subtitle_sync`, `tag_suggest`, `visual_theme`
- Pacing: `pacing_curve` (`PacingCurve` with 7 `CurveShape` variants, `CurveAnalyser`, `CurveStats`)
- Complete `AutoEditor` pipeline with `auto_edit()` orchestrating detect-score-cut-assemble workflow
- All numerical operations use plain Rust primitives (f32/f64/Vec/fixed arrays); no ndarray dependency

## Enhancements
- [x] Replace `ndarray` dependency with SciRS2-Core per SCIRS2 policy (confirmed: ndarray was never present in Cargo.toml or source; all array ops use plain Rust)
- [x] Improve `highlights::HighlightDetector` with configurable multi-pass analysis (coarse then fine) (verified 2026-05-16; src/highlights.rs:253 MultiPassConfig, coarse-to-fine detection:267)
- [x] Add confidence scores to `cuts::CutPoint` for user review prioritization (verified 2026-05-16; src/cuts.rs:80 CutPoint struct, confidence field:85)
- [x] Extend `assembly::AssemblyType` with `Recap` variant for episode/series recaps (verified 2026-05-16; src/assembly.rs:39 Recap variant)
- [x] Add `rules::PacingPreset::Custom` with user-defined shot duration curves (`pacing_curve` module: `PacingCurve`, `CurveShape` 7 variants, `CurveKeyframe`, `CurveAnalyser`; `distribute_clips`/`compute_cut_positions`; 27 tests)
- [x] Improve `scoring::SceneScorer` with temporal context (score relative to neighbors) (verified 2026-05-16; src/scoring.rs:509 TemporalContextConfig, neighbor bonus:516)
- [x] Extend `smart_reframe` with subject tracking across multiple frames for smooth panning (`SubjectTracker` + `SubjectBounds` with EMA; `generate_sequence`/`generate_smooth_sequence`; 10 tests)
- [x] Add vertical-to-horizontal reframing in `smart_reframe` (`VerticalToHorizontalParams`, `VerticalToHorizontalStrategy` 5 variants, `FrameOrientation`; `primary_placement`/`side_regions`/`saliency_crop_window`; 7 tests)
- [x] Improve `music_sync` with downbeat vs upbeat distinction for edit point selection (verified 2026-05-16; src/music_sync.rs:12 BeatGrid, downbeats_ms:16)

## New Features
- [x] Add `auto_thumbnail` module for automatic thumbnail selection from best frames (verified 2026-05-16; src/auto_thumbnail.rs:903 lines)
- [x] Implement `auto_chaptering` module to generate chapter points from scene analysis (verified 2026-05-16; src/auto_chaptering.rs:1099 lines)
- [x] Add `content_warning` module for automatic content classification (violence, language) (verified 2026-05-16; src/content_warning.rs:1064 lines)
- [x] Implement `engagement_predictor` module using interest curve analysis for audience retention (verified 2026-05-16; src/engagement_predictor.rs:853 lines)
- [x] Add `a_b_roll` module for automatic B-roll insertion suggestions based on dialogue content (verified 2026-05-16; src/a_b_roll.rs:1131 lines)
- [x] Implement `color_continuity` checker that flags jarring color shifts between assembled clips (verified 2026-05-16; src/color_continuity.rs:767 lines)
- [x] Add platform-specific export presets for YouTube Shorts, Instagram Reels, TikTok in `assembly` (verified 2026-05-16; src/assembly.rs:79 PlatformPreset enum, YouTubeShorts:81, InstagramReels:83, TikTok:85)

## Performance
- [ ] Parallelize `highlights::detect_highlights()` across frame batches using rayon
- [ ] Cache scene features in `scoring::SceneScorer` to avoid recomputation on config changes
- [ ] Add early termination to `cuts::detect_cuts()` when sufficient cut points found for target duration
- [ ] Use downscaled frames for initial `smart_crop` pass, refine on full resolution only for final crops
- [ ] Implement lazy evaluation for `AutoEditResult` fields that may not be needed by caller

## Testing
- [ ] Add test for `auto_edit()` end-to-end with synthetic video frames and audio
- [ ] Test `rules::RulesEngine::apply_rules()` enforces minimum shot duration constraints
- [ ] Add regression test for `assembly::generate_social_clip()` ensuring output within duration tolerance
- [ ] Test `smart_trim` preserves dialogue boundaries when trimming
- [ ] Add benchmark comparing `scoring` performance across different `ScoringConfig` settings
- [ ] Test `color_match` produces consistent results for identical input pairs

## Documentation
- [ ] Document use-case presets ("trailer", "highlights", "social") with expected behavior
- [ ] Add flowchart for `auto_edit()` pipeline showing data flow between stages
- [ ] Document `scoring::FeatureWeights` tuning guidelines for different content types
