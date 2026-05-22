# oximedia-optimize TODO

## Current Status
- 43 source files/directories covering codec optimization and encoding tuning
- Core: Optimizer with RdoEngine, PsychoAnalyzer, MotionOptimizer, AqEngine
- RDO: rate-distortion optimization, lambda calculation, RDOQ (rate-distortion optimized quantization)
- Psychovisual: contrast sensitivity, visual masking models, perceptual optimization
- Motion: TZSearch, EPZS, UMH algorithms, bidirectional optimization, subpel refinement, MV prediction
- Encoding tools: partition selection, intra mode optimization, transform selection, loop filter tuning
- Presets: Fast/Medium/Slow/Placebo levels, content-type adaptation (Animation/Film/Screen/Generic)
- Advanced: lookahead analysis, GOP optimizer, two-pass encoding, CRF sweep, adaptive ladder, bitrate control

## Enhancements
- [x] Implement vmaf_predict module -- currently a file exists but needs actual VMAF score prediction from features (verified 2026-05-16; src/vmaf_predict.rs:491 fn predict, :523 predict_from_stats, :532 predict_from_pixels, 1249 total lines)
- [x] Wire roi_encode into the main Optimizer pipeline for region-of-interest based quality allocation (verified 2026-05-16; src/roi_encode.rs:756 lines, no stubs)
- [x] Connect temporal_aq to the AqEngine for frame-level QP adaptation based on temporal complexity (verified 2026-05-16; src/temporal_aq.rs:674 lines)
- [ ] Improve scene_encode to use lookahead data for scene-cut-aware QP adjustment (verified-open 2026-05-16: scene_encode.rs exists but lookahead wiring not complete)
- [x] Add content-adaptive GOP structure selection in gop_optimizer (longer GOPs for static, shorter for action) (verified 2026-05-16; src/gop_optimizer.rs:237 fn content_type_base_rules, ContentAdaptiveGop)
- [x] Implement actual CABAC context optimization in entropy module (currently may be structural) (verified 2026-05-16; src/entropy/cabac.rs:192 CabacContext with LPS/MPS probability update)
- [x] Extend crf_sweep to output Pareto-optimal bitrate/quality curves for automated quality targeting (verified 2026-05-16; src/crf_sweep.rs:604 lines)
- [x] Add grain synthesis detection in psycho module to preserve film grain without wasting bits (verified 2026-05-16; src/psycho/grain.rs:1140 lines)

## New Features
- [x] Implement AV1 tile/frame parallel optimization -- select tile partitioning based on content complexity
- [x] Add per-frame bitrate allocation using Viterbi algorithm for optimal constant-quality distribution
- [ ] Implement machine-learning-based mode decision using oximedia-neural for fast RDO approximation (verified-open 2026-05-16: no neural/ML mode in decision.rs)
- [x] Add encoding quality estimation without full encode (fast VMAF/SSIM prediction from frame features) (verified 2026-05-16; src/vmaf_predict.rs:5 fast quality score estimator without full VMAF, predict fn:391)
- [x] Implement denoising-aware optimization -- coordinate with pre-filter to avoid encoding noise
- [ ] Add HDR-aware psychovisual model using PQ/HLG transfer function aware masking thresholds (verified-open 2026-05-16: no HDR/PQ/HLG masking in perceptual_optimization.rs)
- [x] Implement multi-pass encoding with rate redistribution between passes in two_pass module (verified 2026-05-16; src/multi_pass.rs:520 RateRedistributor, redistribute:569)

## Performance
- [ ] Use rayon for parallel RDO evaluation across partition candidates (when parallel_rdo=true)
- [ ] Implement SIMD-accelerated SAD/SATD computation in motion search (currently scalar)
- [ ] Add block-level caching in RdoEngine -- cache cost estimates for repeated partition evaluations
- [ ] Implement early termination in partition search when cost clearly exceeds parent split
- [ ] Profile cache_optimizer and prefetch modules -- ensure they actually improve memory access patterns
- [ ] Use thread-local storage for per-thread scratch buffers in parallel motion search

## Testing
- [ ] Add RDO cost monotonicity test: higher lambda should prefer lower-rate modes
- [ ] Test motion search accuracy: known displacement input should find correct motion vector
- [ ] Verify AQ produces higher QP for flat regions and lower QP for detailed regions
- [ ] Test partition decision against brute-force: verify faster presets don't miss optimal split by >5% RD cost
- [ ] Add benchmark comparing Fast/Medium/Slow/Placebo encoding speed and quality metrics
- [ ] Test psychovisual masking: verify high-texture regions tolerate more quantization noise

## Documentation
- [ ] Document the optimization level trade-offs with encoding speed and quality impact measurements
- [ ] Add guide for tuning custom OptimizerConfig for specific content types
- [ ] Document the RDO pipeline with mathematical formulation of cost = distortion + lambda * rate
