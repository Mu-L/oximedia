# oximedia-packager TODO

## Current Status
- 50+ source files/directories for adaptive streaming packaging
- Formats: HLS (TS, fMP4), DASH (CMAF), with both live and VOD modes
- Core: Packager, HlsPackager, DashPackager with builder patterns
- Features: bitrate ladder generation, segment creation, manifest generation (M3U8, MPD)
- Encryption: AES-128, SAMPLE-AES, CENC (optional feature-gated)
- Cloud: optional S3 upload support (aws-sdk-s3)
- Modules: segment/segment_index/segment_list/segment_timeline/segment_validator, manifest/manifest_builder/manifest_update
- Additional: low_latency, bandwidth_estimator, packaging_optimizer, playlist_generator, subtitle_track, timed_metadata, pssh, drm_info, isobmff_writer
- 793 tests passing (0 failures)

## Enhancements
- [x] Fix `package_both` in Packager::package_both -- uses `unwrap_or` on path conversion, should return error
- [x] Implement actual isobmff_writer for fMP4/CMAF segment creation (init segment + media segments)
      - ftyp+moov init segment with big-endian u32 box sizes and 4-byte type codes
      - moof+mdat media segments with sequence numbers and decode timestamps
      - BoxWriter primitive for correct size-prefix patching
      - write_init_segment() / write_media_segment() public API
- [x] Add segment duration validation in segment_validator -- verify segments are within 0.5-2x target duration
- [x] Implement manifest_update for live streaming -- incremental playlist update without full regeneration
- [x] Add keyframe alignment enforcement across all bitrate variants in the ladder
- [x] Improve encryption_info to support Widevine, FairPlay, and PlayReady DRM system IDs in PSSH boxes
- [ ] Add content-aware segment boundary selection aligned to scene changes (use codec keyframe positions)
- [ ] Implement variant_stream (noted in src but may need wiring) for proper multi-variant playlist generation

## New Features
- [x] Add CMAF byte-range addressing for single-file segment storage (reduces file count on CDN)
      - CmafByteRangeFile: single-file container with init + media segments mapped by byte offset
      - ByteRangeSegmentIndex: lookup by sequence number
      - HLS EXT-X-BYTERANGE tag generation for single-file addressing
- [x] Implement HLS interstitial support (EXT-X-DATERANGE for ad insertion points)
      - HlsInterstitial / HlsInterstitialBuilder with Apple HLS interstitial spec support
      - X-ASSET-URI, X-ASSET-LIST, X-RESUME-OFFSET, X-RESTRICT attributes
      - InterstitialSchedule for managing multiple interstitials per playlist
      - DateRangeClass enum (AppleInterstitial, Scte35, Custom)
- [x] Add DASH event stream support for timed metadata (SCTE-35, ID3 equivalent)
- [ ] Implement automatic bitrate ladder generation from source analysis (per-title encoding)
- [x] Add thumbnail/trick-play track generation (I-frame only playlist for HLS, thumbnail adaptation set for DASH)
      - IFramePlaylist: EXT-X-I-FRAMES-ONLY playlists with EXT-X-BYTERANGE addressing
      - IFramePlaylistBuilder: fluent builder for constructing I-frame playlists
      - ThumbnailTrack: DASH thumbnail adaptation set generation with tile sprites
      - ThumbnailTile: tile grid configuration (columns x rows, spatial fragment URIs)
      - EXT-X-I-FRAME-STREAM-INF tag generation for multivariant playlists
- [ ] Implement pre-roll/bumper injection at packaging time (insert intro segment before content)
- [ ] Add multi-audio track support with language/accessibility metadata in manifests
- [ ] Implement SSAI (Server-Side Ad Insertion) markers and manifest manipulation

## Performance
- [x] Parallelize multi-variant packaging using rayon -- encode each bitrate ladder rung concurrently
      - ParallelPackager: rayon par_iter over PackagingTask list
      - PackagingTask: per-variant work unit with segments and timescale
      - VariantResult: per-variant output (init + media segments, metadata)
      - to_hls_multivariant() / to_dash_period() merge helpers
- [ ] Implement streaming segment output -- write segments as they are produced rather than buffering entire file
- [ ] Add async file I/O throughout packaging pipeline (currently using tokio::fs but verify full async path)
- [ ] Cache parsed source file metadata to avoid re-demuxing when packaging to both HLS and DASH
- [ ] Implement segment output file pre-allocation to reduce filesystem fragmentation

## Testing
- [ ] Test HLS M3U8 output against Apple's mediastreamvalidator or equivalent format checks
- [ ] Verify DASH MPD output validates against MPD schema (ISO 23009-1)
- [ ] Add test for bitrate ladder ordering in manifests (highest to lowest bandwidth)
- [ ] Test encryption: verify AES-128 encrypted segment can be decrypted with known key
- [ ] Add live packaging test: simulate continuous segment arrival and verify rolling playlist window
- [x] Test CMAF segment structure: verify correct moof/mdat box ordering in fragmented MP4
      - BoxWriter tests: size prefix patching, nested boxes, big-endian encoding
      - InitConfig / write_init_segment: ftyp+moov structure, codec fourcc, timescale
      - MediaSample / write_media_segment: moof+mdat, sequence numbers, decode timestamps
      - Roundtrip: init + 2 media segments byte-level verification

## Documentation
- [ ] Document the packaging workflow: source -> demux -> segment -> encrypt -> manifest -> upload
- [ ] Add configuration guide for common deployment targets (Apple HLS spec, DASH-IF guidelines)
- [ ] Document DRM integration setup for Widevine/FairPlay/PlayReady with key server requirements
