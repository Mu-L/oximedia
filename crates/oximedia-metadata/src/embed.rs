//! Metadata embedding and extraction utilities.
//!
//! This module provides utilities for embedding metadata into media files
//! and extracting metadata from them.

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
/// # Errors
///
/// Returns an error if embedding fails.
pub fn embed(file_data: &[u8], metadata: &Metadata) -> Result<Vec<u8>, Error> {
    let metadata_bytes = metadata.write()?;

    // Simple concatenation - in a real implementation, this would need to
    // properly integrate the metadata into the file structure based on format
    let mut result = Vec::new();

    match metadata.format() {
        MetadataFormat::Id3v2 => {
            // ID3v2 tags go at the beginning of the file
            result.extend_from_slice(&metadata_bytes);
            result.extend_from_slice(file_data);
        }
        MetadataFormat::Apev2 => {
            // APEv2 tags go at the end of the file
            result.extend_from_slice(file_data);
            result.extend_from_slice(&metadata_bytes);
        }
        _ => {
            // For other formats, prepend metadata
            result.extend_from_slice(&metadata_bytes);
            result.extend_from_slice(file_data);
        }
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
}
