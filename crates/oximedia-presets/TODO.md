# oximedia-presets TODO

## Current Status
- 30 modules covering comprehensive encoding preset library with 200+ presets
- Categories: platform (YouTube, Vimeo, Facebook, Instagram, TikTok, Twitter, LinkedIn), broadcast (ATSC, DVB, ISDB), streaming (HLS, DASH, Smooth, RTMP, SRT), archive (lossless, mezzanine), mobile (iOS, Android), web (HTML5, progressive), social (stories, reels, feed), quality tiers, codec profiles (AV1, VP9, VP8, Opus, H.264, HEVC)
- Key types: PresetLibrary, PresetRegistry, PresetMetadata, Preset, AbrLadder, OptimalPreset, BitrateRange
- Additional modules: preset_benchmark, preset_chain, preset_diff, preset_export, preset_import, preset_manager, preset_metadata, preset_override, preset_resolver, preset_scoring, preset_tags, preset_versioning, color_preset, delivery_preset, ingest_preset

## Enhancements
- [ ] Add preset inheritance in `PresetLibrary` — derive presets from base presets with overrides
- [ ] Implement `PresetRegistry` fuzzy search (Levenshtein distance) for typo-tolerant lookups
- [ ] Extend `OptimalPreset::select()` to consider resolution and frame rate, not just bitrate
- [ ] Add platform spec auto-update mechanism in `platform` modules (fetch latest requirements)
- [ ] Implement preset compatibility matrix — check if source media matches preset requirements
- [ ] Extend `preset_chain` to validate chained preset compatibility (output format of N matches input of N+1)
- [ ] Add `preset_scoring` weight customization per use-case (latency-sensitive vs quality-sensitive)

## New Features
- [ ] Add Twitch streaming presets in `platform` module (low-latency, different ingest servers)
- [ ] Implement per-scene adaptive preset selection based on content complexity analysis
- [ ] Add AV1 film grain synthesis presets for archival/restoration workflows
- [ ] Implement preset A/B comparison tool in `preset_benchmark` (encode same source, compare metrics)
- [ ] Add FLAC/Opus audio-only presets for podcast and music distribution
- [ ] Implement preset recommendation from source media analysis (resolution, noise, motion)
- [ ] Add Cinema DCP (Digital Cinema Package) presets for theatrical distribution
- [ ] Implement user preset sharing via import/export with signature verification

## Performance
- [ ] Lazy-load preset modules — only instantiate presets for requested categories
- [ ] Cache `PresetLibrary::new()` initialization since it loads all 200+ presets eagerly
- [ ] Optimize `PresetLibrary::search()` with a pre-built inverted index on name/description tokens
- [ ] Use `Arc<Preset>` in `PresetRegistry` to avoid cloning preset configs during lookup

## Testing
- [ ] Add validation tests ensuring all platform presets meet their respective platform requirements
- [ ] Test `OptimalPreset` selection with edge cases (zero bitrate, u64::MAX bitrate)
- [ ] Add round-trip tests for `preset_export` and `preset_import` (export -> import -> compare)
- [ ] Test `AbrLadder` generation for all streaming protocols with expected rung counts
- [ ] Verify `preset_diff` correctly identifies all parameter changes between preset versions

## Documentation
- [ ] Add preset selection guide — decision tree for choosing the right preset by use case
- [ ] Document ABR ladder design principles with recommended bitrate/resolution combinations
- [ ] Add platform-specific encoding guidelines with links to official specifications
