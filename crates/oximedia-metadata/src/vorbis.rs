//! Vorbis Comments parsing and writing support.
//!
//! Vorbis Comments are used in Ogg Vorbis, FLAC, Opus, and other formats.
//!
//! # Format
//!
//! Vorbis Comments consist of:
//! - Vendor string (length-prefixed UTF-8 string)
//! - User comment list count (32-bit little-endian)
//! - User comments (each is a length-prefixed UTF-8 string in "NAME=value" format)
//!
//! # Common Fields
//!
//! - **TITLE**: Track title
//! - **ARTIST**: Artist name
//! - **ALBUM**: Album title
//! - **ALBUMARTIST**: Album artist
//! - **TRACKNUMBER**: Track number
//! - **DATE**: Release date
//! - **GENRE**: Genre

use crate::{Error, Metadata, MetadataFormat, MetadataValue};
use std::io::{Cursor, Read};

/// Parse Vorbis Comments from data.
///
/// # Errors
///
/// Returns an error if the data is not valid Vorbis Comments.
pub fn parse(data: &[u8]) -> Result<Metadata, Error> {
    let mut cursor = Cursor::new(data);
    let mut metadata = Metadata::new(MetadataFormat::VorbisComments);

    // Read vendor string length
    let vendor_length = read_u32_le(&mut cursor)?;

    // Read vendor string
    let mut vendor_bytes = vec![0u8; vendor_length as usize];
    cursor
        .read_exact(&mut vendor_bytes)
        .map_err(|e| Error::ParseError(format!("Failed to read vendor string: {e}")))?;

    let vendor = String::from_utf8(vendor_bytes)
        .map_err(|e| Error::EncodingError(format!("Invalid UTF-8 in vendor string: {e}")))?;

    // Store vendor string
    metadata.insert("VENDOR".to_string(), MetadataValue::Text(vendor));

    // Read user comment list count
    let comment_count = read_u32_le(&mut cursor)?;

    // Read user comments
    for _ in 0..comment_count {
        let comment_length = read_u32_le(&mut cursor)?;

        let mut comment_bytes = vec![0u8; comment_length as usize];
        cursor
            .read_exact(&mut comment_bytes)
            .map_err(|e| Error::ParseError(format!("Failed to read comment: {e}")))?;

        let comment = String::from_utf8(comment_bytes)
            .map_err(|e| Error::EncodingError(format!("Invalid UTF-8 in comment: {e}")))?;

        // Parse "NAME=value" format
        if let Some(eq_pos) = comment.find('=') {
            let name = comment[..eq_pos].to_uppercase();
            let value = &comment[eq_pos + 1..];

            // Check if field already exists (for multi-value fields)
            if let Some(existing) = metadata.get(&name) {
                // Convert to list if not already
                match existing {
                    MetadataValue::Text(text) => {
                        let list = vec![text.clone(), value.to_string()];
                        metadata.insert(name, MetadataValue::TextList(list));
                    }
                    MetadataValue::TextList(list) => {
                        let mut new_list = list.clone();
                        new_list.push(value.to_string());
                        metadata.insert(name, MetadataValue::TextList(new_list));
                    }
                    _ => {}
                }
            } else {
                metadata.insert(name, MetadataValue::Text(value.to_string()));
            }
        }
    }

    Ok(metadata)
}

/// Write Vorbis Comments to bytes.
///
/// # Errors
///
/// Returns an error if writing fails.
pub fn write(metadata: &Metadata) -> Result<Vec<u8>, Error> {
    let mut result = Vec::new();

    // Get vendor string (or use default)
    let vendor = metadata
        .get("VENDOR")
        .and_then(|v| v.as_text())
        .unwrap_or("OxiMedia");

    // Write vendor string length and data
    write_u32_le(&mut result, vendor.len() as u32);
    result.extend_from_slice(vendor.as_bytes());

    // Collect comments
    let mut comments = Vec::new();
    for (key, value) in metadata.fields() {
        if key == "VENDOR" {
            continue; // Skip vendor field
        }

        match value {
            MetadataValue::Text(text) => {
                let comment = format!("{key}={text}");
                comments.push(comment);
            }
            MetadataValue::TextList(list) => {
                for text in list {
                    let comment = format!("{key}={text}");
                    comments.push(comment);
                }
            }
            _ => {
                // Skip non-text values
            }
        }
    }

    // Write comment count
    write_u32_le(&mut result, comments.len() as u32);

    // Write comments
    for comment in comments {
        write_u32_le(&mut result, comment.len() as u32);
        result.extend_from_slice(comment.as_bytes());
    }

    Ok(result)
}

/// Read a 32-bit little-endian unsigned integer.
fn read_u32_le(cursor: &mut Cursor<&[u8]>) -> Result<u32, Error> {
    let mut bytes = [0u8; 4];
    cursor
        .read_exact(&mut bytes)
        .map_err(|e| Error::ParseError(format!("Failed to read u32: {e}")))?;
    Ok(u32::from_le_bytes(bytes))
}

/// Write a 32-bit little-endian unsigned integer.
fn write_u32_le(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vorbis_comments_round_trip() {
        let mut metadata = Metadata::new(MetadataFormat::VorbisComments);

        metadata.insert(
            "TITLE".to_string(),
            MetadataValue::Text("Test Title".to_string()),
        );
        metadata.insert(
            "ARTIST".to_string(),
            MetadataValue::Text("Test Artist".to_string()),
        );
        metadata.insert(
            "ALBUM".to_string(),
            MetadataValue::Text("Test Album".to_string()),
        );
        metadata.insert(
            "TRACKNUMBER".to_string(),
            MetadataValue::Text("5".to_string()),
        );
        metadata.insert("DATE".to_string(), MetadataValue::Text("2024".to_string()));

        // Write
        let data = write(&metadata).expect("Write failed");

        // Parse
        let parsed = parse(&data).expect("Parse failed");

        assert_eq!(
            parsed.get("TITLE").and_then(|v| v.as_text()),
            Some("Test Title")
        );
        assert_eq!(
            parsed.get("ARTIST").and_then(|v| v.as_text()),
            Some("Test Artist")
        );
        assert_eq!(
            parsed.get("ALBUM").and_then(|v| v.as_text()),
            Some("Test Album")
        );
        assert_eq!(
            parsed.get("TRACKNUMBER").and_then(|v| v.as_text()),
            Some("5")
        );
        assert_eq!(parsed.get("DATE").and_then(|v| v.as_text()), Some("2024"));
    }

    #[test]
    fn test_vorbis_comments_multivalue() {
        let mut metadata = Metadata::new(MetadataFormat::VorbisComments);

        let artists = vec!["Artist 1".to_string(), "Artist 2".to_string()];
        metadata.insert("ARTIST".to_string(), MetadataValue::TextList(artists));

        // Write
        let data = write(&metadata).expect("Write failed");

        // Parse
        let parsed = parse(&data).expect("Parse failed");

        let parsed_artists = parsed
            .get("ARTIST")
            .and_then(|v| v.as_text_list())
            .expect("Expected text list");

        assert_eq!(parsed_artists.len(), 2);
        assert_eq!(parsed_artists[0], "Artist 1");
        assert_eq!(parsed_artists[1], "Artist 2");
    }

    #[test]
    fn test_read_write_u32_le() {
        let mut buffer = Vec::new();
        write_u32_le(&mut buffer, 12345);

        let mut cursor = Cursor::new(buffer.as_slice());
        let value = read_u32_le(&mut cursor).expect("should succeed in test");

        assert_eq!(value, 12345);
    }

    #[test]
    fn test_empty_vorbis_comments() {
        let metadata = Metadata::new(MetadataFormat::VorbisComments);

        // Write
        let data = write(&metadata).expect("Write failed");

        // Parse
        let parsed = parse(&data).expect("Parse failed");

        // Should have vendor string
        assert!(parsed.get("VENDOR").is_some());
    }
}
