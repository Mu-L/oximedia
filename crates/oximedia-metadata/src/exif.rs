//! EXIF (Exchangeable Image File Format) metadata parsing and writing support.
//!
//! EXIF metadata is commonly used in JPEG and TIFF images.
//!
//! # Format
//!
//! EXIF uses TIFF structure with IFDs (Image File Directories) containing tags.
//!
//! # Common Tags
//!
//! - **0x010F**: Make (camera manufacturer)
//! - **0x0110**: Model (camera model)
//! - **0x0132**: DateTime
//! - **0x013B**: Artist
//! - **0x8298**: Copyright
//! - **0x9003**: DateTimeOriginal
//! - **0x9004**: DateTimeDigitized

use crate::{Error, Metadata, MetadataFormat, MetadataValue};
use std::io::{Cursor, Read, Seek, SeekFrom};

/// EXIF byte order marker (little-endian)
const EXIF_LE: &[u8; 2] = b"II";

/// EXIF byte order marker (big-endian)
const EXIF_BE: &[u8; 2] = b"MM";

/// TIFF magic number
const TIFF_MAGIC: u16 = 0x002A;

/// Byte order for reading multi-byte values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ByteOrder {
    LittleEndian,
    BigEndian,
}

/// EXIF tag types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum TagType {
    Byte = 1,
    Ascii = 2,
    Short = 3,
    Long = 4,
    Rational = 5,
    Undefined = 7,
    SLong = 9,
    SRational = 10,
}

impl TagType {
    fn from_u16(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::Byte),
            2 => Some(Self::Ascii),
            3 => Some(Self::Short),
            4 => Some(Self::Long),
            5 => Some(Self::Rational),
            7 => Some(Self::Undefined),
            9 => Some(Self::SLong),
            10 => Some(Self::SRational),
            _ => None,
        }
    }
}

/// Parse EXIF metadata from data.
///
/// # Errors
///
/// Returns an error if the data is not valid EXIF.
#[allow(clippy::too_many_lines)]
pub fn parse(data: &[u8]) -> Result<Metadata, Error> {
    if data.len() < 8 {
        return Err(Error::ParseError("Data too short for EXIF".to_string()));
    }

    // Determine byte order
    let byte_order = if &data[0..2] == EXIF_LE {
        ByteOrder::LittleEndian
    } else if &data[0..2] == EXIF_BE {
        ByteOrder::BigEndian
    } else {
        return Err(Error::ParseError(
            "Invalid EXIF byte order marker".to_string(),
        ));
    };

    let mut cursor = Cursor::new(data);
    cursor.set_position(2);

    // Read TIFF magic number
    let magic = read_u16(&mut cursor, byte_order)?;
    if magic != TIFF_MAGIC {
        return Err(Error::ParseError("Invalid TIFF magic number".to_string()));
    }

    // Read offset to first IFD
    let ifd_offset = read_u32(&mut cursor, byte_order)?;

    let mut metadata = Metadata::new(MetadataFormat::Exif);

    // Parse IFD
    parse_ifd(&mut cursor, ifd_offset as u64, byte_order, &mut metadata)?;

    Ok(metadata)
}

/// Parse an IFD (Image File Directory).
fn parse_ifd(
    cursor: &mut Cursor<&[u8]>,
    offset: u64,
    byte_order: ByteOrder,
    metadata: &mut Metadata,
) -> Result<(), Error> {
    cursor
        .seek(SeekFrom::Start(offset))
        .map_err(|e| Error::ParseError(format!("Failed to seek to IFD: {e}")))?;

    // Read number of directory entries
    let entry_count = read_u16(cursor, byte_order)?;

    // Read directory entries
    for _ in 0..entry_count {
        let tag = read_u16(cursor, byte_order)?;
        let tag_type = read_u16(cursor, byte_order)?;
        let count = read_u32(cursor, byte_order)?;
        let value_offset = read_u32(cursor, byte_order)?;

        // Parse tag value
        if let Some(value) =
            parse_tag_value(cursor, tag, tag_type, count, value_offset, byte_order)?
        {
            let tag_name = get_tag_name(tag);
            metadata.insert(tag_name, value);
        }
    }

    Ok(())
}

/// Parse a tag value.
#[allow(clippy::too_many_arguments)]
fn parse_tag_value(
    cursor: &mut Cursor<&[u8]>,
    _tag: u16,
    tag_type: u16,
    count: u32,
    value_offset: u32,
    _byte_order: ByteOrder,
) -> Result<Option<MetadataValue>, Error> {
    let tag_type = TagType::from_u16(tag_type);

    match tag_type {
        Some(TagType::Ascii) => {
            // ASCII string
            let current_pos = cursor.position();

            // Determine if value is inline or at offset
            let value_size = count;
            let value_pos = if value_size <= 4 {
                current_pos - 4 // Inline value
            } else {
                value_offset as u64 // Value at offset
            };

            cursor
                .seek(SeekFrom::Start(value_pos))
                .map_err(|e| Error::ParseError(format!("Failed to seek to value: {e}")))?;

            let mut value_bytes = vec![0u8; count as usize];
            cursor
                .read_exact(&mut value_bytes)
                .map_err(|e| Error::ParseError(format!("Failed to read value: {e}")))?;

            // Remove null terminators
            while let Some(&0) = value_bytes.last() {
                value_bytes.pop();
            }

            let text = String::from_utf8(value_bytes)
                .map_err(|e| Error::EncodingError(format!("Invalid UTF-8 in EXIF tag: {e}")))?;

            cursor.seek(SeekFrom::Start(current_pos))?;

            Ok(Some(MetadataValue::Text(text)))
        }
        Some(TagType::Short) => {
            // 16-bit unsigned integer
            let value = if count == 1 {
                u32::from(value_offset >> 16)
            } else {
                value_offset
            };
            Ok(Some(MetadataValue::Integer(i64::from(value))))
        }
        Some(TagType::Long) => {
            // 32-bit unsigned integer
            Ok(Some(MetadataValue::Integer(i64::from(value_offset))))
        }
        _ => {
            // Unsupported type
            Ok(None)
        }
    }
}

/// Get tag name from tag ID.
fn get_tag_name(tag: u16) -> String {
    match tag {
        0x010F => "Make".to_string(),
        0x0110 => "Model".to_string(),
        0x0132 => "DateTime".to_string(),
        0x013B => "Artist".to_string(),
        0x8298 => "Copyright".to_string(),
        0x9003 => "DateTimeOriginal".to_string(),
        0x9004 => "DateTimeDigitized".to_string(),
        0x010E => "ImageDescription".to_string(),
        0x0131 => "Software".to_string(),
        _ => format!("Tag_{tag:04X}"),
    }
}

/// Write EXIF metadata to data.
///
/// # Errors
///
/// Returns an error if writing fails.
pub fn write(metadata: &Metadata) -> Result<Vec<u8>, Error> {
    let mut result = Vec::new();

    // Write byte order (little-endian)
    result.extend_from_slice(EXIF_LE);

    // Write TIFF magic number
    result.extend_from_slice(&TIFF_MAGIC.to_le_bytes());

    // Write IFD offset
    result.extend_from_slice(&8_u32.to_le_bytes());

    // Count text fields
    let text_fields: Vec<_> = metadata
        .fields()
        .iter()
        .filter(|(_, v)| matches!(v, MetadataValue::Text(_)))
        .collect();

    // Write number of directory entries
    result.extend_from_slice(&(text_fields.len() as u16).to_le_bytes());

    // Write directory entries
    let mut value_offset = 8 + 2 + (text_fields.len() * 12) + 4;

    for (key, value) in &text_fields {
        let tag = get_tag_id(key);
        let text = value.as_text().unwrap_or("");

        // Write tag
        result.extend_from_slice(&tag.to_le_bytes());

        // Write type (ASCII)
        result.extend_from_slice(&(TagType::Ascii as u16).to_le_bytes());

        // Write count (including null terminator)
        let count = text.len() + 1;
        result.extend_from_slice(&(count as u32).to_le_bytes());

        // Write value or offset
        if count <= 4 {
            // Inline value
            let mut inline_value = [0u8; 4];
            inline_value[..text.len()].copy_from_slice(text.as_bytes());
            result.extend_from_slice(&inline_value);
        } else {
            // Value at offset
            result.extend_from_slice(&(value_offset as u32).to_le_bytes());
            value_offset += count;
        }
    }

    // Write next IFD offset (0 = no more IFDs)
    result.extend_from_slice(&0_u32.to_le_bytes());

    // Write values that didn't fit inline
    for (_, value) in &text_fields {
        let text = value.as_text().unwrap_or("");
        if text.len() + 1 > 4 {
            result.extend_from_slice(text.as_bytes());
            result.push(0); // Null terminator
        }
    }

    Ok(result)
}

/// Get tag ID from tag name.
fn get_tag_id(name: &str) -> u16 {
    match name {
        "Make" => 0x010F,
        "Model" => 0x0110,
        "DateTime" => 0x0132,
        "Artist" => 0x013B,
        "Copyright" => 0x8298,
        "DateTimeOriginal" => 0x9003,
        "DateTimeDigitized" => 0x9004,
        "ImageDescription" => 0x010E,
        "Software" => 0x0131,
        _ => 0xFFFF,
    }
}

/// Read a 16-bit unsigned integer.
fn read_u16(cursor: &mut Cursor<&[u8]>, byte_order: ByteOrder) -> Result<u16, Error> {
    let mut bytes = [0u8; 2];
    cursor
        .read_exact(&mut bytes)
        .map_err(|e| Error::ParseError(format!("Failed to read u16: {e}")))?;

    Ok(match byte_order {
        ByteOrder::LittleEndian => u16::from_le_bytes(bytes),
        ByteOrder::BigEndian => u16::from_be_bytes(bytes),
    })
}

/// Read a 32-bit unsigned integer.
fn read_u32(cursor: &mut Cursor<&[u8]>, byte_order: ByteOrder) -> Result<u32, Error> {
    let mut bytes = [0u8; 4];
    cursor
        .read_exact(&mut bytes)
        .map_err(|e| Error::ParseError(format!("Failed to read u32: {e}")))?;

    Ok(match byte_order {
        ByteOrder::LittleEndian => u32::from_le_bytes(bytes),
        ByteOrder::BigEndian => u32::from_be_bytes(bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exif_round_trip() {
        let mut metadata = Metadata::new(MetadataFormat::Exif);

        metadata.insert(
            "Artist".to_string(),
            MetadataValue::Text("Test Artist".to_string()),
        );
        metadata.insert(
            "Copyright".to_string(),
            MetadataValue::Text("Copyright 2024".to_string()),
        );

        // Write
        let data = write(&metadata).expect("Write failed");

        // Parse
        let parsed = parse(&data).expect("Parse failed");

        assert_eq!(
            parsed.get("Artist").and_then(|v| v.as_text()),
            Some("Test Artist")
        );
        assert_eq!(
            parsed.get("Copyright").and_then(|v| v.as_text()),
            Some("Copyright 2024")
        );
    }

    #[test]
    fn test_get_tag_name() {
        assert_eq!(get_tag_name(0x010F), "Make");
        assert_eq!(get_tag_name(0x0110), "Model");
        assert_eq!(get_tag_name(0x013B), "Artist");
        assert_eq!(get_tag_name(0x8298), "Copyright");
    }

    #[test]
    fn test_get_tag_id() {
        assert_eq!(get_tag_id("Make"), 0x010F);
        assert_eq!(get_tag_id("Model"), 0x0110);
        assert_eq!(get_tag_id("Artist"), 0x013B);
        assert_eq!(get_tag_id("Copyright"), 0x8298);
    }
}
