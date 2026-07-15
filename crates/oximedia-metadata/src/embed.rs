//! Metadata embedding and extraction utilities.
//!
//! This module provides utilities for embedding metadata into media files
//! and extracting metadata from them.
//!
//! # Container-aware embedding
//!
//! [`embed`] never falls back to naive byte concatenation for a format whose
//! container it does not understand. Concatenating a raw metadata payload onto an
//! arbitrary file produces a *corrupt* file for every format except ID3v2 (tag
//! prepended to the audio stream) and APEv2 (tag appended as a footer) — both of
//! which are simple by design and do not require understanding the rest of the
//! file. For every other format, `embed` either:
//!
//! - performs a real, container-aware splice (currently: Exif/XMP into a JPEG
//!   `APP1` segment, or a merge into a bare standalone Exif/XMP metadata blob), or
//! - returns [`Error::Unsupported`] with a precise explanation of what a correct
//!   implementation would require.
//!
//! It never silently returns a byte stream that looks successful but does not
//! actually parse as the target format.

use crate::{Error, Metadata, MetadataFormat};

/// Metadata embedding and extraction utilities.
pub struct MetadataEmbed;

impl MetadataEmbed {
    /// Detect metadata format from file data.
    ///
    /// # Errors
    ///
    /// Returns an error if the format cannot be detected.
    pub fn detect_format(data: &[u8]) -> Result<MetadataFormat, Error> {
        detect_format(data)
    }

    /// Extract metadata from file data.
    ///
    /// # Errors
    ///
    /// Returns an error if extraction fails.
    pub fn extract(data: &[u8], format: MetadataFormat) -> Result<Metadata, Error> {
        extract(data, format)
    }

    /// Embed metadata into file data.
    ///
    /// # Errors
    ///
    /// Returns an error if embedding fails.
    pub fn embed(file_data: &[u8], metadata: &Metadata) -> Result<Vec<u8>, Error> {
        embed(file_data, metadata)
    }
}

/// Detect metadata format from file data.
///
/// # Errors
///
/// Returns an error if the format cannot be detected.
pub fn detect_format(data: &[u8]) -> Result<MetadataFormat, Error> {
    if data.len() < 16 {
        return Err(Error::ParseError(
            "Data too short to detect format".to_string(),
        ));
    }

    // Check for ID3v2 (MP3)
    if data.len() >= 3 && &data[0..3] == b"ID3" {
        return Ok(MetadataFormat::Id3v2);
    }

    // Check for APEv2
    if data.len() >= 8 && &data[0..8] == b"APETAGEX" {
        return Ok(MetadataFormat::Apev2);
    }

    // Check for EXIF/TIFF
    if data.len() >= 2 && (&data[0..2] == b"II" || &data[0..2] == b"MM") {
        return Ok(MetadataFormat::Exif);
    }

    // Check for XMP (XML-based)
    if data.len() >= 5 && &data[0..5] == b"<?xpa" {
        return Ok(MetadataFormat::Xmp);
    }

    // Check for Matroska tags (XML-based)
    if data.len() >= 5 && &data[0..5] == b"<Tags" {
        return Ok(MetadataFormat::Matroska);
    }

    // Check for IPTC
    if !data.is_empty() && data[0] == 0x1C {
        return Ok(MetadataFormat::Iptc);
    }

    // Check for Vorbis Comments (requires more context)
    // This is a simplified check
    if data.len() >= 4 {
        // Check for common Vorbis field names
        let text = String::from_utf8_lossy(&data[..data.len().min(100)]);
        if text.contains("TITLE=") || text.contains("ARTIST=") || text.contains("ALBUM=") {
            return Ok(MetadataFormat::VorbisComments);
        }
    }

    Err(Error::Unsupported("Unknown metadata format".to_string()))
}

/// Extract metadata from file data.
///
/// # Errors
///
/// Returns an error if extraction fails.
pub fn extract(data: &[u8], format: MetadataFormat) -> Result<Metadata, Error> {
    Metadata::parse(data, format)
}

/// Extract metadata from file data with automatic format detection.
///
/// # Errors
///
/// Returns an error if extraction fails.
pub fn extract_auto(data: &[u8]) -> Result<Metadata, Error> {
    let format = detect_format(data)?;
    extract(data, format)
}

/// Embed metadata into file data.
///
/// * [`MetadataFormat::Id3v2`] tags are prepended (ID3v2 is defined to sit at the
///   start of the audio stream).
/// * [`MetadataFormat::Apev2`] tags are appended as a footer (APEv2 is defined to
///   sit at the end of the file).
/// * [`MetadataFormat::Exif`] and [`MetadataFormat::Xmp`] are embedded
///   container-aware: into a JPEG `APP1` segment when `file_data` is a JPEG file,
///   or merged into a bare standalone Exif/XMP metadata blob when `file_data` is
///   one (the same shape [`exif::write`](crate::exif::write) /
///   [`xmp::write`](crate::xmp::write) produce). Any other container shape is
///   rejected with [`Error::Unsupported`].
/// * [`MetadataFormat::Matroska`], [`MetadataFormat::Iptc`],
///   [`MetadataFormat::VorbisComments`], [`MetadataFormat::iTunes`] and
///   [`MetadataFormat::QuickTime`] are not yet container-aware and always return
///   [`Error::Unsupported`] rather than risking a corrupting concatenation — see
///   the `// TODO(0.2.x):` markers on each arm below for what a correct
///   implementation requires.
///
/// # Errors
///
/// Returns an error if embedding fails, or if `metadata`'s format is not (yet)
/// supported by a real, non-corrupting embed strategy.
pub fn embed(file_data: &[u8], metadata: &Metadata) -> Result<Vec<u8>, Error> {
    match metadata.format() {
        MetadataFormat::Id3v2 => {
            let metadata_bytes = metadata.write()?;
            // ID3v2 tags go at the beginning of the file.
            let mut result = Vec::with_capacity(metadata_bytes.len() + file_data.len());
            result.extend_from_slice(&metadata_bytes);
            result.extend_from_slice(file_data);
            Ok(result)
        }
        MetadataFormat::Apev2 => {
            let metadata_bytes = metadata.write()?;
            // APEv2 tags go at the end of the file.
            let mut result = Vec::with_capacity(file_data.len() + metadata_bytes.len());
            result.extend_from_slice(file_data);
            result.extend_from_slice(&metadata_bytes);
            Ok(result)
        }
        MetadataFormat::Exif => embed_exif(file_data, metadata),
        MetadataFormat::Xmp => embed_xmp(file_data, metadata),
        MetadataFormat::Matroska => Err(Error::Unsupported(
            "embed() does not yet support Matroska Tags: a correct implementation must \
             locate (or create) the EBML `Tags` master element inside the Matroska \
             `Segment`, splice in the new `SimpleTag` children, and renumber the variable- \
             length EBML size prefixes of every enclosing element. Naive concatenation would \
             produce an unreadable file, so this is an honest error instead. \
             TODO(0.2.x): implement EBML-aware Tags-element embed."
                .to_string(),
        )),
        MetadataFormat::Iptc => Err(Error::Unsupported(
            "embed() does not yet support IPTC IIM: a correct implementation must write a \
             JPEG APP13 \"Photoshop 3.0\\0\" segment containing an 8BIM Image Resource Block \
             (resource ID 0x0404) wrapping the IIM datasets, mirroring the APP1 splice used \
             for Exif/XMP. Naive concatenation would produce an unreadable file, so this is \
             an honest error instead. \
             TODO(0.2.x): implement Photoshop-IRB-aware IPTC embed (JPEG APP13)."
                .to_string(),
        )),
        MetadataFormat::VorbisComments => Err(Error::Unsupported(
            "embed() does not yet support Vorbis Comments: a correct implementation must \
             locate the comment header packet inside its container (the second packet of an \
             Ogg logical stream, or the VORBIS_COMMENT METADATA_BLOCK of a FLAC file), \
             re-lace/replace it, and recompute any container checksums (Ogg page CRC32). \
             Naive concatenation would produce an unreadable file, so this is an honest error \
             instead. \
             TODO(0.2.x): implement Ogg-page-aware / FLAC-METADATA_BLOCK-aware VorbisComments \
             embed with CRC recomputation."
                .to_string(),
        )),
        format @ (MetadataFormat::iTunes | MetadataFormat::QuickTime) => {
            Err(Error::Unsupported(format!(
                "embed() does not yet support {format} atom metadata: a correct \
                 implementation must splice a `moov/udta/meta` (or `moov/udta`) atom tree \
                 into the MP4/QuickTime box structure and fix up every enclosing box's size \
                 field. Naive concatenation would produce an unreadable file, so this is an \
                 honest error instead. \
                 TODO(0.2.x): implement MP4/QuickTime atom-tree-aware embed."
            )))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JPEG APP1 (Exif / XMP) container-aware embedding
// ─────────────────────────────────────────────────────────────────────────────

/// JPEG APP0 marker byte (JFIF), following the `0xFF` marker prefix.
const JPEG_APP0: u8 = 0xE0;
/// JPEG APP1 marker byte (Exif / XMP), following the `0xFF` marker prefix.
const JPEG_APP1: u8 = 0xE1;
/// JPEG start-of-scan marker byte: header segments never continue past this point.
const JPEG_SOS: u8 = 0xDA;
/// JPEG end-of-image marker byte.
const JPEG_EOI: u8 = 0xD9;

/// Identifier prefix for an Exif `APP1` segment payload (immediately followed by
/// the raw TIFF/Exif blob, i.e. exactly [`exif::write`](crate::exif::write)'s output).
const EXIF_APP1_PREFIX: &[u8] = b"Exif\0\0";
/// Identifier prefix for an XMP `APP1` segment payload (Adobe's registered GUID,
/// immediately followed by the XMP packet, i.e. exactly
/// [`xmp::write`](crate::xmp::write)'s output).
const XMP_APP1_PREFIX: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

/// `true` when `data` begins with a JPEG Start-Of-Image marker (`FF D8 FF`).
fn is_jpeg(data: &[u8]) -> bool {
    data.len() >= 3 && data[0] == 0xFF && data[1] == 0xD8 && data[2] == 0xFF
}

/// Embed Exif metadata into `file_data`.
///
/// Two container shapes are recognised:
/// - **JPEG**: the Exif payload is written into an `APP1` segment, replacing an
///   existing Exif `APP1` segment in place if one is found, or inserting a new one
///   immediately after the leading `SOI`/`APP0` run (the position mandated by the
///   Exif specification).
/// - **Bare Exif/TIFF blob** (the shape [`exif::write`](crate::exif::write)
///   produces and [`exif::parse`](crate::exif::parse) accepts): the new fields are
///   merged into the existing blob's fields (new values win on conflict) and the
///   merged result is re-serialized as a standalone blob.
///
/// Any other container shape is rejected with [`Error::Unsupported`] rather than
/// risking a naive concatenation that would corrupt the file.
fn embed_exif(file_data: &[u8], metadata: &Metadata) -> Result<Vec<u8>, Error> {
    if is_jpeg(file_data) {
        let payload = crate::exif::write(metadata)?;
        return jpeg_upsert_app1(file_data, EXIF_APP1_PREFIX, &payload);
    }
    if file_data.len() >= 4 && (&file_data[0..2] == b"II" || &file_data[0..2] == b"MM") {
        let mut existing = crate::exif::parse(file_data)?;
        for (key, value) in metadata.fields() {
            existing.insert(key.clone(), value.clone());
        }
        return crate::exif::write(&existing);
    }
    Err(Error::Unsupported(
        "embed(Exif) only supports JPEG (APP1 segment) or a bare Exif/TIFF blob as the \
         target container; the given file_data matches neither shape, and naive \
         concatenation would corrupt it"
            .to_string(),
    ))
}

/// Embed XMP metadata into `file_data`.
///
/// Two container shapes are recognised:
/// - **JPEG**: the XMP packet is written into an `APP1` segment (Adobe's
///   registered `http://ns.adobe.com/xap/1.0/` GUID), replacing an existing XMP
///   `APP1` segment in place if one is found, or inserting a new one immediately
///   after the leading `SOI`/`APP0`/Exif-`APP1` run.
/// - **Bare XMP packet** (the shape [`xmp::write`](crate::xmp::write) produces and
///   [`xmp::parse`](crate::xmp::parse) accepts, optionally without the `<?xpacket`
///   wrapper): the new fields are merged into the existing packet's fields (new
///   values win on conflict) and the merged result is re-serialized.
///
/// Any other container shape is rejected with [`Error::Unsupported`] rather than
/// risking a naive concatenation that would corrupt the file.
fn embed_xmp(file_data: &[u8], metadata: &Metadata) -> Result<Vec<u8>, Error> {
    if is_jpeg(file_data) {
        let payload = crate::xmp::write(metadata)?;
        return jpeg_upsert_app1(file_data, XMP_APP1_PREFIX, &payload);
    }
    if file_data.starts_with(b"<?xpacket") || file_data.starts_with(b"<x:xmpmeta") {
        let mut existing = crate::xmp::parse(file_data)?;
        for (key, value) in metadata.fields() {
            existing.insert(key.clone(), value.clone());
        }
        return crate::xmp::write(&existing);
    }
    Err(Error::Unsupported(
        "embed(Xmp) only supports JPEG (APP1 segment) or a bare XMP packet as the target \
         container; the given file_data matches neither shape, and naive concatenation \
         would corrupt it"
            .to_string(),
    ))
}

/// Insert or replace an `APP1` segment carrying `id_prefix` immediately followed by
/// `payload`, in a JPEG byte stream — without disturbing any other segment or the
/// entropy-coded scan data.
///
/// If an existing `APP1` segment whose payload starts with `id_prefix` is found
/// before the scan (`SOS`), it is replaced in place. Otherwise a new segment is
/// inserted immediately after the leading `SOI` (and the `APP0`/JFIF segment, if
/// the file has one as its very first segment) — the position mandated for the
/// Exif `APP1` segment by the Exif specification, and a safe, widely-accepted
/// position for XMP too.
///
/// # Errors
///
/// Returns [`Error::ParseError`] if `file_data` is not well-formed enough to
/// locate a safe splice point (missing `SOI`, a truncated segment, or no
/// `SOS`/`EOI` found before the data ends), and [`Error::Unsupported`] if the new
/// segment would not fit in a single 64 KiB JPEG segment (splitting an oversized
/// payload across multiple `APP1` segments, as JPEG does for large XMP extension
/// blocks, is not implemented).
fn jpeg_upsert_app1(file_data: &[u8], id_prefix: &[u8], payload: &[u8]) -> Result<Vec<u8>, Error> {
    if !is_jpeg(file_data) {
        return Err(Error::ParseError(
            "Not a JPEG file (missing SOI marker)".to_string(),
        ));
    }

    let segment_payload_len = id_prefix.len() + payload.len();
    // The 16-bit segment length field counts itself (2 bytes) plus the payload.
    if segment_payload_len + 2 > 0xFFFF {
        return Err(Error::Unsupported(format!(
            "metadata payload ({segment_payload_len} bytes) does not fit in a single JPEG \
             APP1 segment (max {} bytes); multi-segment splitting is not implemented. \
             TODO(0.2.x): implement multi-segment APP1 splitting for oversized payloads.",
            0xFFFF - 2
        )));
    }

    let mut pos = 2usize; // Just past SOI.
    let mut insert_at: Option<usize> = None;
    let mut existing_range: Option<(usize, usize)> = None;

    loop {
        if pos + 2 > file_data.len() {
            return Err(Error::ParseError(
                "Truncated JPEG: ran out of data while scanning header segments".to_string(),
            ));
        }
        if file_data[pos] != 0xFF {
            return Err(Error::ParseError(format!(
                "Malformed JPEG: expected a marker (0xFF) at offset {pos}"
            )));
        }
        // Skip fill bytes (0xFF repeated) before the real marker code byte.
        let mut marker_pos = pos;
        while marker_pos + 1 < file_data.len() && file_data[marker_pos + 1] == 0xFF {
            marker_pos += 1;
        }
        let marker = file_data[marker_pos + 1];
        let seg_start = pos;
        let header_end = marker_pos + 2;

        if marker == JPEG_SOS || marker == JPEG_EOI {
            // Header segments are over: this is the fallback insertion point.
            break;
        }

        if header_end + 2 > file_data.len() {
            return Err(Error::ParseError(
                "Truncated JPEG: segment length field missing".to_string(),
            ));
        }
        let seg_len = usize::from(u16::from_be_bytes([
            file_data[header_end],
            file_data[header_end + 1],
        ]));
        if seg_len < 2 || header_end + seg_len > file_data.len() {
            return Err(Error::ParseError(format!(
                "Malformed JPEG: invalid segment length at offset {header_end}"
            )));
        }
        let seg_end = header_end + seg_len;
        let payload_start = header_end + 2;

        if insert_at.is_none() {
            // The very first header segment decides the splice point: right after a
            // leading APP0/JFIF segment, or right before the first non-APP0 segment.
            insert_at = Some(if marker == JPEG_APP0 {
                seg_end
            } else {
                seg_start
            });
        }

        if marker == JPEG_APP1 && file_data[payload_start..seg_end].starts_with(id_prefix) {
            existing_range = Some((seg_start, seg_end));
        }

        pos = seg_end;
    }
    let insert_at = insert_at.unwrap_or(2);

    let mut new_segment = Vec::with_capacity(4 + segment_payload_len);
    new_segment.push(0xFF);
    new_segment.push(JPEG_APP1);
    new_segment.extend_from_slice(&((segment_payload_len + 2) as u16).to_be_bytes());
    new_segment.extend_from_slice(id_prefix);
    new_segment.extend_from_slice(payload);

    let mut result = Vec::with_capacity(file_data.len() + new_segment.len());
    if let Some((start, end)) = existing_range {
        result.extend_from_slice(&file_data[..start]);
        result.extend_from_slice(&new_segment);
        result.extend_from_slice(&file_data[end..]);
    } else {
        result.extend_from_slice(&file_data[..insert_at]);
        result.extend_from_slice(&new_segment);
        result.extend_from_slice(&file_data[insert_at..]);
    }
    Ok(result)
}

/// Remove metadata from file data.
///
/// # Errors
///
/// Returns an error if removal fails.
pub fn remove_metadata(file_data: &[u8], format: MetadataFormat) -> Result<Vec<u8>, Error> {
    match format {
        MetadataFormat::Id3v2 => {
            // Remove ID3v2 tags from beginning
            if file_data.len() >= 10 && &file_data[0..3] == b"ID3" {
                // Parse header to get tag size
                let tag_size = (u32::from(file_data[6] & 0x7F) << 21)
                    | (u32::from(file_data[7] & 0x7F) << 14)
                    | (u32::from(file_data[8] & 0x7F) << 7)
                    | u32::from(file_data[9] & 0x7F);

                let total_size = 10 + tag_size as usize;
                if total_size <= file_data.len() {
                    return Ok(file_data[total_size..].to_vec());
                }
            }
            Ok(file_data.to_vec())
        }
        MetadataFormat::Apev2 => {
            // Remove APEv2 tags from end
            if file_data.len() >= 32 {
                let footer_pos = file_data.len() - 32;
                if &file_data[footer_pos..footer_pos + 8] == b"APETAGEX" {
                    // Parse footer to get tag size
                    let tag_size = u32::from_le_bytes([
                        file_data[footer_pos + 12],
                        file_data[footer_pos + 13],
                        file_data[footer_pos + 14],
                        file_data[footer_pos + 15],
                    ]);

                    let total_size = tag_size as usize + 32;
                    if total_size <= file_data.len() {
                        return Ok(file_data[..file_data.len() - total_size].to_vec());
                    }
                }
            }
            Ok(file_data.to_vec())
        }
        _ => {
            // For other formats, return data as-is
            Ok(file_data.to_vec())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MetadataValue;

    #[test]
    fn test_detect_format_id3v2() {
        let data = b"ID3\x03\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(
            detect_format(data).expect("should succeed in test"),
            MetadataFormat::Id3v2
        );
    }

    #[test]
    fn test_detect_format_apev2() {
        let data = b"APETAGEX\x00\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(
            detect_format(data).expect("should succeed in test"),
            MetadataFormat::Apev2
        );
    }

    #[test]
    fn test_detect_format_exif() {
        let data = b"II*\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(
            detect_format(data).expect("should succeed in test"),
            MetadataFormat::Exif
        );

        let data = b"MM\x00*\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(
            detect_format(data).expect("should succeed in test"),
            MetadataFormat::Exif
        );
    }

    #[test]
    fn test_embed_id3v2() {
        let mut metadata = Metadata::new(MetadataFormat::Id3v2);
        metadata.insert("TIT2".to_string(), MetadataValue::Text("Test".to_string()));

        let file_data = b"audio data here";
        let result = embed(file_data, &metadata).expect("Embed failed");

        // Result should start with ID3v2 tag
        assert!(result.len() > file_data.len());
        assert_eq!(&result[0..3], b"ID3");
    }

    #[test]
    fn test_remove_metadata_id3v2() {
        // Create a simple ID3v2 tag
        let mut tag_data = vec![b'I', b'D', b'3', 0x03, 0x00, 0x00];
        // Size: 20 bytes (synchsafe)
        tag_data.extend_from_slice(&[0x00, 0x00, 0x00, 0x14]);
        // Tag data (20 bytes)
        tag_data.extend_from_slice(&[0u8; 20]);
        // File data
        tag_data.extend_from_slice(b"audio data");

        let result = remove_metadata(&tag_data, MetadataFormat::Id3v2).expect("Remove failed");
        assert_eq!(result, b"audio data");
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Build a minimal, structurally-valid JPEG: SOI, a JFIF APP0 segment, an SOS
    /// segment with a plausible (but not decoder-accurate — no real image data is
    /// needed for container-splicing tests) header, a few bytes of opaque "scan
    /// data", and EOI.
    fn minimal_jpeg() -> Vec<u8> {
        let mut jpeg = Vec::new();
        jpeg.extend_from_slice(&[0xFF, 0xD8]); // SOI
                                               // APP0 (JFIF): len=0x0010, "JFIF\0", version 1.1, units=0, density 1x1, no thumbnail.
        jpeg.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
        jpeg.extend_from_slice(b"JFIF\0");
        jpeg.extend_from_slice(&[0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);
        // SOS: len=0x0008, Ns=1, (component=1, tables=0), Ss=0, Se=0x3F, Ah/Al=0.
        jpeg.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08]);
        jpeg.extend_from_slice(&[0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
        // Opaque entropy-coded scan bytes (never interpreted by embed()).
        jpeg.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);
        jpeg.extend_from_slice(&[0xFF, 0xD9]); // EOI
        jpeg
    }

    /// Locate the `[start, end)` byte range of the first APP1 segment whose payload
    /// starts with `id_prefix`, or `None` if not present.
    fn find_app1_segment(jpeg: &[u8], id_prefix: &[u8]) -> Option<(usize, usize)> {
        let mut pos = 2usize;
        while pos + 4 <= jpeg.len() {
            assert_eq!(jpeg[pos], 0xFF, "expected marker at {pos}");
            let marker = jpeg[pos + 1];
            if marker == JPEG_SOS || marker == JPEG_EOI {
                break;
            }
            let seg_len = usize::from(u16::from_be_bytes([jpeg[pos + 2], jpeg[pos + 3]]));
            let seg_end = pos + 2 + seg_len;
            let payload_start = pos + 4;
            if marker == JPEG_APP1 && jpeg[payload_start..seg_end].starts_with(id_prefix) {
                return Some((pos, seg_end));
            }
            pos = seg_end;
        }
        None
    }

    // ── Exif / JPEG ──────────────────────────────────────────────────────────

    #[test]
    fn test_embed_exif_into_jpeg_round_trips_via_temp_file() {
        let tmp = std::env::temp_dir().join("oximedia_metadata_embed_exif_test.jpg");
        std::fs::write(&tmp, minimal_jpeg()).expect("write temp jpeg");

        let mut metadata = Metadata::new(MetadataFormat::Exif);
        metadata.insert(
            "Artist".to_string(),
            MetadataValue::Text("Jane Doe".to_string()),
        );

        let file_data = std::fs::read(&tmp).expect("read temp jpeg");
        let embedded = embed(&file_data, &metadata).expect("embed exif into jpeg");
        std::fs::write(&tmp, &embedded).expect("write embedded jpeg");

        let round_tripped = std::fs::read(&tmp).expect("read embedded jpeg");
        let _ = std::fs::remove_file(&tmp);

        // The file must still be a well-formed JPEG (SOI at the start, EOI at the end).
        assert_eq!(&round_tripped[0..2], &[0xFF, 0xD8]);
        assert_eq!(&round_tripped[round_tripped.len() - 2..], &[0xFF, 0xD9]);

        // The opaque scan tail (SOS marker onward) must be byte-for-byte preserved.
        let original = minimal_jpeg();
        let original_sos = original
            .windows(2)
            .position(|w| w == [0xFF, 0xDA])
            .expect("original has SOS");
        let new_sos = round_tripped
            .windows(2)
            .position(|w| w == [0xFF, 0xDA])
            .expect("embedded has SOS");
        assert_eq!(&round_tripped[new_sos..], &original[original_sos..]);

        // The Exif APP1 segment must be present and parse back to the inserted field.
        let (start, end) =
            find_app1_segment(&round_tripped, EXIF_APP1_PREFIX).expect("exif app1 present");
        let tiff_blob = &round_tripped[start + 4 + EXIF_APP1_PREFIX.len()..end];
        let parsed = crate::exif::parse(tiff_blob).expect("parse embedded exif blob");
        assert_eq!(
            parsed.get("Artist").and_then(MetadataValue::as_text),
            Some("Jane Doe")
        );
    }

    #[test]
    fn test_embed_exif_replaces_existing_app1_in_place() {
        let jpeg = minimal_jpeg();

        let mut first = Metadata::new(MetadataFormat::Exif);
        first.insert(
            "Artist".to_string(),
            MetadataValue::Text("First Artist".to_string()),
        );
        let after_first = embed(&jpeg, &first).expect("first embed");

        let mut second = Metadata::new(MetadataFormat::Exif);
        second.insert(
            "Artist".to_string(),
            MetadataValue::Text("Second Artist".to_string()),
        );
        let after_second = embed(&after_first, &second).expect("second embed (replace)");

        // Only one Exif APP1 segment should exist, and it must carry the *second* value.
        let mut count = 0usize;
        let mut pos = 2usize;
        while pos + 4 <= after_second.len() {
            let marker = after_second[pos + 1];
            if marker == JPEG_SOS || marker == JPEG_EOI {
                break;
            }
            let seg_len = usize::from(u16::from_be_bytes([
                after_second[pos + 2],
                after_second[pos + 3],
            ]));
            let seg_end = pos + 2 + seg_len;
            if marker == JPEG_APP1 && after_second[pos + 4..seg_end].starts_with(EXIF_APP1_PREFIX) {
                count += 1;
            }
            pos = seg_end;
        }
        assert_eq!(
            count, 1,
            "expected exactly one Exif APP1 segment after replace"
        );

        let (start, end) =
            find_app1_segment(&after_second, EXIF_APP1_PREFIX).expect("exif app1 present");
        let tiff_blob = &after_second[start + 4 + EXIF_APP1_PREFIX.len()..end];
        let parsed = crate::exif::parse(tiff_blob).expect("parse replaced exif blob");
        assert_eq!(
            parsed.get("Artist").and_then(MetadataValue::as_text),
            Some("Second Artist")
        );
    }

    #[test]
    fn test_embed_exif_bare_tiff_blob_merges_fields() {
        let mut base = Metadata::new(MetadataFormat::Exif);
        base.insert("Make".to_string(), MetadataValue::Text("Acme".to_string()));
        let base_blob = crate::exif::write(&base).expect("write base exif blob");

        let mut addition = Metadata::new(MetadataFormat::Exif);
        addition.insert(
            "Artist".to_string(),
            MetadataValue::Text("Jane Doe".to_string()),
        );
        let merged_blob = embed(&base_blob, &addition).expect("merge into bare exif blob");

        let parsed = crate::exif::parse(&merged_blob).expect("parse merged exif blob");
        assert_eq!(
            parsed.get("Make").and_then(MetadataValue::as_text),
            Some("Acme"),
            "pre-existing field must survive the merge"
        );
        assert_eq!(
            parsed.get("Artist").and_then(MetadataValue::as_text),
            Some("Jane Doe"),
            "new field must be present after the merge"
        );
    }

    #[test]
    fn test_embed_exif_rejects_unrecognized_container() {
        let mut metadata = Metadata::new(MetadataFormat::Exif);
        metadata.insert("Artist".to_string(), MetadataValue::Text("X".to_string()));

        let random_bytes = b"this is not a jpeg or a tiff blob at all";
        let result = embed(random_bytes, &metadata);
        assert!(
            result.is_err(),
            "unrecognized container must be an honest error"
        );
        assert!(matches!(result.unwrap_err(), Error::Unsupported(_)));
    }

    // ── XMP / JPEG ───────────────────────────────────────────────────────────

    #[test]
    fn test_embed_xmp_into_jpeg_round_trips() {
        let jpeg = minimal_jpeg();

        let mut metadata = Metadata::new(MetadataFormat::Xmp);
        metadata.insert(
            "dc:creator".to_string(),
            MetadataValue::Text("Jane Doe".to_string()),
        );

        let embedded = embed(&jpeg, &metadata).expect("embed xmp into jpeg");

        assert_eq!(&embedded[0..2], &[0xFF, 0xD8]);
        assert_eq!(&embedded[embedded.len() - 2..], &[0xFF, 0xD9]);

        let (start, end) = find_app1_segment(&embedded, XMP_APP1_PREFIX).expect("xmp app1 present");
        let xmp_packet = &embedded[start + 4 + XMP_APP1_PREFIX.len()..end];
        let parsed = crate::xmp::parse(xmp_packet).expect("parse embedded xmp packet");
        assert_eq!(
            parsed.get("dc:creator").and_then(MetadataValue::as_text),
            Some("Jane Doe")
        );
    }

    #[test]
    fn test_embed_xmp_bare_packet_merges_fields() {
        let mut base = Metadata::new(MetadataFormat::Xmp);
        base.insert(
            "dc:title".to_string(),
            MetadataValue::Text("Original Title".to_string()),
        );
        let base_packet = crate::xmp::write(&base).expect("write base xmp packet");

        let mut addition = Metadata::new(MetadataFormat::Xmp);
        addition.insert(
            "dc:creator".to_string(),
            MetadataValue::Text("Jane Doe".to_string()),
        );
        let merged = embed(&base_packet, &addition).expect("merge into bare xmp packet");

        let parsed = crate::xmp::parse(&merged).expect("parse merged xmp packet");
        assert_eq!(
            parsed.get("dc:title").and_then(MetadataValue::as_text),
            Some("Original Title"),
            "pre-existing field must survive the merge"
        );
        assert_eq!(
            parsed.get("dc:creator").and_then(MetadataValue::as_text),
            Some("Jane Doe"),
            "new field must be present after the merge"
        );
    }

    // ── Honest errors for not-yet-container-aware formats ──────────────────────

    #[test]
    fn test_embed_matroska_is_honest_unsupported_error() {
        let metadata = Metadata::new(MetadataFormat::Matroska);
        let result = embed(b"not a real matroska file", &metadata);
        assert!(matches!(result, Err(Error::Unsupported(_))));
    }

    #[test]
    fn test_embed_iptc_is_honest_unsupported_error() {
        let metadata = Metadata::new(MetadataFormat::Iptc);
        let result = embed(b"not a real iptc file", &metadata);
        assert!(matches!(result, Err(Error::Unsupported(_))));
    }

    #[test]
    fn test_embed_vorbis_comments_is_honest_unsupported_error() {
        let metadata = Metadata::new(MetadataFormat::VorbisComments);
        let result = embed(b"not a real ogg/flac file", &metadata);
        assert!(matches!(result, Err(Error::Unsupported(_))));
    }

    #[test]
    fn test_embed_itunes_is_honest_unsupported_error() {
        let metadata = Metadata::new(MetadataFormat::iTunes);
        let result = embed(b"not a real mp4 file", &metadata);
        assert!(matches!(result, Err(Error::Unsupported(_))));
    }
}
