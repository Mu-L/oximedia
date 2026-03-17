# oximedia-dedup TODO

## Current Status
- 34 source files; media deduplication and duplicate detection
- Hashing: BLAKE3 cryptographic hashing for exact duplicates, rolling hash, frame hash, perceptual hash (pHash)
- Visual similarity: SSIM, histogram comparison, feature vector cosine similarity
- Audio: audio fingerprinting with bit-level Hamming distance comparison
- Metadata: weighted metadata comparison (duration, resolution, codec, container)
- Storage: SQLite-based indexing (feature-gated), bloom filter, LSH index, hash store, similarity index
- Detection strategies: ExactHash, PerceptualHash, SSIM, Histogram, FeatureMatch, AudioFingerprint, Metadata, All, VisualAll, Fast
- Modules: hash, visual, audio, metadata, database, report, bloom_filter, cluster, content_id, content_signature, dedup_cache, dedup_index, dedup_policy, dedup_report, dedup_stats, frame_hash, fuzzy_match, lsh_index, merge_strategy, near_duplicate, perceptual_hash, phash, rolling_hash, segment_dedup, similarity_index, video_dedup

## Enhancements
- [x] Replace O(n^2) pairwise comparison in `find_perceptual_duplicates()` with LSH-based approximate nearest neighbor via `lsh_index.rs`
- [ ] Extend `visual.rs` with dHash and wHash alongside pHash for robustness against different transformations
- [ ] Add configurable thumbnail resolution in `find_ssim_duplicates()` (currently hardcoded 8x8)
- [ ] Improve `metadata.rs` comparison with normalized title/filename matching using `fuzzy_match.rs`
- [ ] Extend `dedup_policy.rs` with configurable actions per duplicate group (keep newest, keep highest quality, prompt user)
- [x] Add progress reporting callbacks to `DuplicateDetector::find_duplicates()` for large libraries
- [ ] Improve `audio.rs` fingerprinting with chromagram-based features for better music matching
- [ ] Extend `dedup_report.rs` with disk space savings estimation per duplicate group

## New Features
- [x] Implement incremental deduplication in `dedup_index.rs` (only scan new/modified files)
- [ ] Add video segment deduplication in `segment_dedup.rs` (detect shared clips within different videos)
- [x] Implement cross-format duplicate detection (same content in different containers/codecs)
- [ ] Add `merge_strategy.rs` implementation for automatic duplicate resolution (symlinks, hardlinks, deletion)
- [ ] Implement hierarchical deduplication: fast pass (hash) -> medium pass (perceptual) -> slow pass (SSIM)
- [ ] Add `cluster.rs` transitive closure grouping (if A~B and B~C, group {A,B,C} together)
- [ ] Implement network-aware deduplication for distributed media libraries
- [x] Add `content_signature.rs` robust signature that survives transcoding, cropping, and watermarking

## Performance
- [x] Replace `rustfft` with OxiFFT per COOLJAPAN policy for audio fingerprinting FFT
- [ ] Replace `ndarray` with native implementations per COOLJAPAN policy
- [ ] Parallelize `add_files()` with rayon for bulk indexing (currently sequential)
- [ ] Implement batch database insertions in `database.rs` for faster indexing
- [ ] Add bloom filter pre-screening in `bloom_filter.rs` before expensive pairwise comparisons
- [ ] Optimize `rolling_hash.rs` for streaming duplicate detection without loading entire files
- [ ] Cache decoded thumbnails and fingerprints in `dedup_cache.rs` across sessions

## Testing
- [ ] Add end-to-end dedup test: index files -> find duplicates -> verify correct grouping
- [ ] Test `bloom_filter.rs` false positive rate at different fill levels
- [ ] Test `lsh_index.rs` recall accuracy for near-duplicate queries
- [ ] Add benchmark for 10K+ file library deduplication throughput
- [ ] Test `perceptual_hash.rs` robustness against common transformations (resize, crop, brightness)
- [ ] Test `audio.rs` fingerprint matching across different encodings of the same audio
- [ ] Add integration test with `sqlite` feature enabled for full `DuplicateDetector` workflow

## Documentation
- [ ] Document detection strategy selection guide (when to use Fast vs All vs specific strategies)
- [ ] Add accuracy/performance trade-off analysis for each detection method
- [ ] Document database schema and migration strategy for `database.rs`
