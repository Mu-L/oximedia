# oximedia-metadata TODO

## Current Status
- 42 modules implementing comprehensive metadata standards support
- Formats: ID3v2 (v2.3/v2.4), Vorbis Comments, APEv2, iTunes/MP4, XMP, EXIF, IPTC/IIM, QuickTime, Matroska
- Core: Metadata container, MetadataValue (Text, TextList, Binary, Integer, Float, Picture, Boolean, DateTime)
- Picture handling: 21 PictureType variants, MIME type, dimensions, color depth
- Conversion: MetadataConverter for cross-format field mapping, CommonFields abstraction
- Parsing/writing: parse()/write() dispatch to format-specific implementations
- Specialized: av1_metadata, exif_parse, iptc_iim, linked_data, opengraph, schema_org, provenance, rights_metadata
- Management: metadata_diff, metadata_merge, metadata_export, metadata_history, metadata_sanitize, metadata_template
- Search/index: search, schema_registry, tag_normalize, sidecar, field_validator
- Advanced: bulk_update, embedding, media_metadata, schema, opus_tags, musicbrainz, geotag
- Dependencies: oximedia-core, quick-xml, encoding_rs, serde, bytes

## Enhancements
- [ ] Add ID3v2.4 UTF-8 text encoding preference in `id3v2.rs` (currently defaults to UTF-16)
- [ ] Extend `vorbis.rs` with multi-value tag support (repeated keys per Vorbis Comment spec)
- [ ] Add `itunes.rs` support for all iTunes atom types including tempo, compilation, gapless flags
- [ ] Implement `xmp.rs` structured property support (arrays, alternatives, bags per XMP spec)
- [ ] Extend `exif.rs` with Makernote parsing for Canon, Nikon, Sony camera-specific tags
- [ ] Add `matroska.rs` support for nested Matroska tag elements (Targets/SimpleTags hierarchy)
- [x] Implement `metadata_sanitize.rs` with configurable sanitization rules (strip GPS, strip personal data)
- [x] Extend `metadata_diff.rs` with three-way merge for resolving concurrent metadata edits

## New Features
- [x] Add an `opus_tags.rs` module for Opus-specific metadata (R128 gain, output gain) per RFC 7845
- [x] Implement a `chapter.rs` module for chapter metadata (Matroska chapters, MP4 chapters, ID3 CHAP)
- [x] Add a `lyrics.rs` module for synchronized lyrics (ID3 SYLT, LRC format) and unsynchronized lyrics
- [x] Implement a `musicbrainz.rs` module for MusicBrainz tag mappings and MBID validation
- [x] Add a `replaygain.rs` module for ReplayGain metadata (track gain, album gain, peak)
- [ ] Implement a `c2pa.rs` module for Content Credentials / C2PA provenance metadata
- [ ] Add a `podcast.rs` module for podcast-specific metadata (iTunes podcast tags, RSS mapping)
- [x] Implement a `geotag.rs` module for GPS coordinate extraction, display, and reverse geocoding
- [ ] Add a `metadata_streaming.rs` module for parsing metadata from streaming data (partial buffers)

## Performance
- [ ] Add lazy parsing in `id3v2.rs` (parse frame headers only, defer body parsing until accessed)
- [ ] Implement zero-copy XMP parsing in `xmp.rs` using borrowed strings from the input buffer
- [ ] Add parallel metadata extraction in `media_metadata.rs` for multi-format probing
- [ ] Cache encoding_rs decoders in `id3v2.rs` to avoid re-initialization per text frame
- [ ] Optimize `tag_normalize.rs` with pre-compiled regex patterns for common tag normalization rules
- [ ] Add batch metadata write in `bulk_update.rs` to reduce I/O operations for multi-file updates

## Testing
- [ ] Add round-trip tests for all 9 metadata formats: parse -> modify -> write -> parse -> compare
- [ ] Test `id3v2.rs` with ID3v2.3 and v2.4 tags from real-world MP3 files (various encoders)
- [ ] Add `exif.rs` tests with EXIF data from multiple camera manufacturers
- [ ] Test `converter.rs` cross-format conversion preserving all CommonFields
- [ ] Add `xmp.rs` tests with complex XMP documents containing nested arrays and alternatives
- [ ] Test `metadata_merge.rs` conflict resolution with overlapping fields from different sources
- [ ] Add `encoding_rs` integration tests for Shift_JIS, EUC-KR, and Latin-1 text frames

## Documentation
- [ ] Add a cross-format field mapping table (ID3v2 TIT2 = Vorbis TITLE = iTunes nam = etc.)
- [ ] Document the MetadataValue type coercion rules when converting between formats
- [ ] Add examples for common metadata workflows (tag music files, strip EXIF from photos, batch rename)
