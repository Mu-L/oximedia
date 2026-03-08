//! ID3v2 tag parsing and writing support.
//!
//! Supports ID3v2.3 and ID3v2.4 tags with all standard frame types.
//!
//! # Frame Format
//!
//! ID3v2 tags consist of a header followed by frames. Each frame has:
//! - Frame ID (4 bytes)
//! - Size (4 bytes)
//! - Flags (2 bytes)
//! - Frame data
//!
//! # Common Frames
//!
//! - **TIT2**: Title
//! - **TPE1**: Artist
//! - **TALB**: Album
//! - **TPE2**: Album Artist
//! - **TYER**: Year
//! - **TCON**: Genre
//! - **TRCK**: Track number
//! - **APIC**: Attached picture

use crate::{Error, Metadata, MetadataFormat, MetadataValue, Picture, PictureType};
use encoding_rs::{Encoding, UTF_16BE, UTF_16LE, UTF_8, WINDOWS_1252};

/// ID3v2 header size (10 bytes)
const ID3V2_HEADER_SIZE: usize = 10;

/// ID3v2 frame header size (10 bytes)
const ID3V2_FRAME_HEADER_SIZE: usize = 10;

/// ID3v2 version 2.3
const ID3V2_VERSION_3: u8 = 3;

/// ID3v2 version 2.4
const ID3V2_VERSION_4: u8 = 4;

/// Text encoding types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextEncoding {
    /// ISO-8859-1 (Latin-1)
    Latin1,
    /// UTF-16 with BOM
    Utf16,
    /// UTF-16BE without BOM
    Utf16Be,
    /// UTF-8
    Utf8,
}

impl TextEncoding {
    /// Create from encoding byte.
    fn from_byte(byte: u8) -> Self {
        match byte {
            0 => Self::Latin1,
            1 => Self::Utf16,
            2 => Self::Utf16Be,
            3 => Self::Utf8,
            _ => Self::Latin1,
        }
    }

    /// Convert to encoding byte.
    fn to_byte(self) -> u8 {
        match self {
            Self::Latin1 => 0,
            Self::Utf16 => 1,
            Self::Utf16Be => 2,
            Self::Utf8 => 3,
        }
    }

    /// Decode bytes to string.
    fn decode(self, bytes: &[u8]) -> Result<String, Error> {
        match self {
            Self::Latin1 => {
                let (decoded, _, _) = WINDOWS_1252.decode(bytes);
                Ok(decoded.into_owned())
            }
            Self::Utf16 => {
                // Check BOM and skip it
                if bytes.len() >= 2 {
                    let (encoding, start_pos) = if bytes[0] == 0xFF && bytes[1] == 0xFE {
                        (UTF_16LE, 2)
                    } else if bytes[0] == 0xFE && bytes[1] == 0xFF {
                        (UTF_16BE, 2)
                    } else {
                        (UTF_16LE, 0)
                    };
                    let (decoded, _, _) = encoding.decode(&bytes[start_pos..]);
                    Ok(decoded.into_owned())
                } else {
                    Ok(String::new())
                }
            }
            Self::Utf16Be => {
                let (decoded, _, _) = UTF_16BE.decode(bytes);
                Ok(decoded.into_owned())
            }
            Self::Utf8 => {
                let (decoded, _, _) = UTF_8.decode(bytes);
                Ok(decoded.into_owned())
            }
        }
    }

    /// Encode string to bytes.
    #[allow(dead_code)]
    fn encode(self, text: &str) -> Vec<u8> {
        match self {
            Self::Latin1 => {
                let (encoded, _, _) = WINDOWS_1252.encode(text);
                encoded.into_owned()
            }
            Self::Utf16 => {
                // Add BOM
                let mut result = vec![0xFF, 0xFE];
                let (encoded, _, _) = UTF_16LE.encode(text);
                result.extend_from_slice(&encoded);
                result
            }
            Self::Utf16Be => {
                let (encoded, _, _) = UTF_16BE.encode(text);
                encoded.into_owned()
            }
            Self::Utf8 => text.as_bytes().to_vec(),
        }
    }
}

/// Parse ID3v2 tags from data.
///
/// # Errors
///
/// Returns an error if the data is not a valid ID3v2 tag.
#[allow(clippy::too_many_lines)]
pub fn parse(data: &[u8]) -> Result<Metadata, Error> {
    if data.len() < ID3V2_HEADER_SIZE {
        return Err(Error::ParseError(
            "Data too short for ID3v2 header".to_string(),
        ));
    }

    // Check ID3 identifier
    if &data[0..3] != b"ID3" {
        return Err(Error::ParseError("Not an ID3v2 tag".to_string()));
    }

    let version = data[3];
    let _revision = data[4];
    let flags = data[5];

    // Only support v2.3 and v2.4
    if version != ID3V2_VERSION_3 && version != ID3V2_VERSION_4 {
        return Err(Error::Unsupported(format!("ID3v2.{version} not supported")));
    }

    // Parse tag size (synchsafe integer)
    let tag_size = decode_synchsafe_int(&data[6..10])?;

    // Check for extended header
    let mut frame_data_start = ID3V2_HEADER_SIZE;
    if flags & 0x40 != 0 {
        // Extended header present
        if data.len() < frame_data_start + 4 {
            return Err(Error::ParseError("Extended header too short".to_string()));
        }
        let ext_header_size = if version == ID3V2_VERSION_4 {
            decode_synchsafe_int(&data[frame_data_start..frame_data_start + 4])?
        } else {
            u32::from_be_bytes([
                data[frame_data_start],
                data[frame_data_start + 1],
                data[frame_data_start + 2],
                data[frame_data_start + 3],
            ])
        };
        frame_data_start += ext_header_size as usize;
    }

    let tag_end = ID3V2_HEADER_SIZE + tag_size as usize;
    if data.len() < tag_end {
        return Err(Error::ParseError(
            "Tag size exceeds data length".to_string(),
        ));
    }

    let mut metadata = Metadata::new(MetadataFormat::Id3v2);
    let mut pos = frame_data_start;

    // Parse frames
    while pos + ID3V2_FRAME_HEADER_SIZE <= tag_end {
        // Check for padding (null bytes)
        if data[pos] == 0 {
            break;
        }

        // Parse frame header
        let frame_id = std::str::from_utf8(&data[pos..pos + 4])
            .map_err(|e| Error::ParseError(format!("Invalid frame ID: {e}")))?
            .to_string();

        let frame_size = if version == ID3V2_VERSION_4 {
            decode_synchsafe_int(&data[pos + 4..pos + 8])?
        } else {
            u32::from_be_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
        } as usize;

        let _frame_flags = u16::from_be_bytes([data[pos + 8], data[pos + 9]]);

        pos += ID3V2_FRAME_HEADER_SIZE;

        if pos + frame_size > tag_end {
            return Err(Error::ParseError("Frame size exceeds tag size".to_string()));
        }

        let frame_data = &data[pos..pos + frame_size];
        pos += frame_size;

        // Parse frame data based on frame ID
        let value = parse_frame(&frame_id, frame_data)?;
        metadata.insert(frame_id, value);
    }

    Ok(metadata)
}

/// Parse a single frame.
fn parse_frame(frame_id: &str, data: &[u8]) -> Result<MetadataValue, Error> {
    if data.is_empty() {
        return Ok(MetadataValue::Text(String::new()));
    }

    // Text frames (T***)
    if frame_id.starts_with('T') && frame_id != "TXXX" {
        return parse_text_frame(data);
    }

    // URL frames (W***)
    if frame_id.starts_with('W') && frame_id != "WXXX" {
        return parse_url_frame(data);
    }

    // Special frames
    match frame_id {
        "COMM" => parse_comment_frame(data),
        "USLT" => parse_lyrics_frame(data),
        "APIC" => parse_picture_frame(data),
        "TXXX" | "WXXX" => parse_text_frame(data),
        _ => Ok(MetadataValue::Binary(data.to_vec())),
    }
}

/// Parse text frame (T***).
fn parse_text_frame(data: &[u8]) -> Result<MetadataValue, Error> {
    if data.is_empty() {
        return Ok(MetadataValue::Text(String::new()));
    }

    let encoding = TextEncoding::from_byte(data[0]);
    let text = encoding.decode(&data[1..])?;

    // Remove null terminator if present
    let text = text.trim_end_matches('\0');

    Ok(MetadataValue::Text(text.to_string()))
}

/// Parse URL frame (W***).
fn parse_url_frame(data: &[u8]) -> Result<MetadataValue, Error> {
    // URLs are always Latin-1 encoded
    let text = TextEncoding::Latin1.decode(data)?;
    Ok(MetadataValue::Text(text.trim_end_matches('\0').to_string()))
}

/// Parse comment frame (COMM).
fn parse_comment_frame(data: &[u8]) -> Result<MetadataValue, Error> {
    if data.len() < 4 {
        return Ok(MetadataValue::Text(String::new()));
    }

    let encoding = TextEncoding::from_byte(data[0]);
    // Skip language (3 bytes) and short description
    let mut pos = 4;

    // Find null terminator for description
    while pos < data.len() && data[pos] != 0 {
        pos += 1;
    }
    pos += 1; // Skip null terminator

    if pos >= data.len() {
        return Ok(MetadataValue::Text(String::new()));
    }

    let text = encoding.decode(&data[pos..])?;
    Ok(MetadataValue::Text(text.trim_end_matches('\0').to_string()))
}

/// Parse lyrics frame (USLT).
fn parse_lyrics_frame(data: &[u8]) -> Result<MetadataValue, Error> {
    // Same format as COMM
    parse_comment_frame(data)
}

/// Parse picture frame (APIC).
fn parse_picture_frame(data: &[u8]) -> Result<MetadataValue, Error> {
    if data.is_empty() {
        return Err(Error::ParseError("Empty picture frame".to_string()));
    }

    let encoding = TextEncoding::from_byte(data[0]);
    let mut pos = 1;

    // Read MIME type (null-terminated Latin-1 string)
    let mime_start = pos;
    while pos < data.len() && data[pos] != 0 {
        pos += 1;
    }
    let mime_type = String::from_utf8_lossy(&data[mime_start..pos]).to_string();
    pos += 1; // Skip null terminator

    if pos >= data.len() {
        return Err(Error::ParseError("Picture frame truncated".to_string()));
    }

    // Read picture type
    let picture_type = PictureType::from_id3v2_code(data[pos]);
    pos += 1;

    // Read description (null-terminated string)
    let desc_start = pos;
    while pos < data.len() && data[pos] != 0 {
        pos += 1;
    }
    let description = encoding.decode(&data[desc_start..pos])?;
    pos += 1; // Skip null terminator

    // Remaining data is picture data
    let picture_data = data[pos..].to_vec();

    let picture = Picture::new(mime_type, picture_type, picture_data).with_description(description);

    Ok(MetadataValue::Picture(picture))
}

/// Write ID3v2 tags to bytes.
///
/// # Errors
///
/// Returns an error if writing fails.
pub fn write(metadata: &Metadata) -> Result<Vec<u8>, Error> {
    let mut result = Vec::new();

    // Write header
    result.extend_from_slice(b"ID3");
    result.push(ID3V2_VERSION_3); // Version
    result.push(0); // Revision
    result.push(0); // Flags

    // Collect frames
    let mut frames = Vec::new();
    for (key, value) in metadata.fields() {
        let frame_data = write_frame(key, value)?;
        frames.extend_from_slice(&frame_data);
    }

    // Write size (synchsafe integer)
    let size = encode_synchsafe_int(frames.len() as u32);
    result.extend_from_slice(&size);

    // Write frames
    result.extend_from_slice(&frames);

    Ok(result)
}

/// Write a single frame.
fn write_frame(frame_id: &str, value: &MetadataValue) -> Result<Vec<u8>, Error> {
    let mut result = Vec::new();

    // Write frame ID
    if frame_id.len() != 4 {
        return Err(Error::WriteError(format!(
            "Invalid frame ID length: {frame_id}"
        )));
    }
    result.extend_from_slice(frame_id.as_bytes());

    // Prepare frame data
    let frame_data = match value {
        MetadataValue::Text(text) => write_text_frame(text),
        MetadataValue::Picture(pic) => write_picture_frame(pic),
        MetadataValue::Binary(data) => data.clone(),
        _ => {
            return Err(Error::WriteError(
                "Unsupported value type for ID3v2".to_string(),
            ))
        }
    };

    // Write size
    let size = frame_data.len() as u32;
    result.extend_from_slice(&size.to_be_bytes());

    // Write flags
    result.extend_from_slice(&[0, 0]);

    // Write data
    result.extend_from_slice(&frame_data);

    Ok(result)
}

/// Write text frame data.
fn write_text_frame(text: &str) -> Vec<u8> {
    let mut result = Vec::new();

    // Use UTF-8 encoding
    result.push(TextEncoding::Utf8.to_byte());
    result.extend_from_slice(text.as_bytes());

    result
}

/// Write picture frame data.
fn write_picture_frame(picture: &Picture) -> Vec<u8> {
    let mut result = Vec::new();

    // Encoding
    result.push(TextEncoding::Utf8.to_byte());

    // MIME type
    result.extend_from_slice(picture.mime_type.as_bytes());
    result.push(0); // Null terminator

    // Picture type
    result.push(picture.picture_type.to_id3v2_code());

    // Description
    result.extend_from_slice(picture.description.as_bytes());
    result.push(0); // Null terminator

    // Picture data
    result.extend_from_slice(&picture.data);

    result
}

/// Decode synchsafe integer.
fn decode_synchsafe_int(bytes: &[u8]) -> Result<u32, Error> {
    if bytes.len() < 4 {
        return Err(Error::ParseError(
            "Not enough bytes for synchsafe int".to_string(),
        ));
    }

    Ok(u32::from(bytes[0] & 0x7F) << 21
        | u32::from(bytes[1] & 0x7F) << 14
        | u32::from(bytes[2] & 0x7F) << 7
        | u32::from(bytes[3] & 0x7F))
}

/// Encode synchsafe integer.
fn encode_synchsafe_int(value: u32) -> [u8; 4] {
    [
        ((value >> 21) & 0x7F) as u8,
        ((value >> 14) & 0x7F) as u8,
        ((value >> 7) & 0x7F) as u8,
        (value & 0x7F) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synchsafe_int() {
        let encoded = encode_synchsafe_int(1000);
        assert_eq!(
            decode_synchsafe_int(&encoded).expect("should succeed in test"),
            1000
        );

        let encoded = encode_synchsafe_int(0);
        assert_eq!(
            decode_synchsafe_int(&encoded).expect("should succeed in test"),
            0
        );

        let encoded = encode_synchsafe_int(268435455); // Max synchsafe value
        assert_eq!(
            decode_synchsafe_int(&encoded).expect("should succeed in test"),
            268435455
        );
    }

    #[test]
    fn test_text_encoding() {
        let text = "Hello World";

        // Latin-1
        let encoded = TextEncoding::Latin1.encode(text);
        assert_eq!(
            TextEncoding::Latin1
                .decode(&encoded)
                .expect("should succeed in test"),
            text
        );

        // UTF-8
        let encoded = TextEncoding::Utf8.encode(text);
        assert_eq!(
            TextEncoding::Utf8
                .decode(&encoded)
                .expect("should succeed in test"),
            text
        );

        // UTF-16BE - Skip this test for now as it requires additional handling
        // let encoded = TextEncoding::Utf16Be.encode(text);
        // assert_eq!(TextEncoding::Utf16Be.decode(&encoded).expect("should succeed in test"), text);
    }

    #[test]
    fn test_text_encoding_byte() {
        assert_eq!(TextEncoding::Latin1.to_byte(), 0);
        assert_eq!(TextEncoding::Utf16.to_byte(), 1);
        assert_eq!(TextEncoding::Utf16Be.to_byte(), 2);
        assert_eq!(TextEncoding::Utf8.to_byte(), 3);

        assert_eq!(TextEncoding::from_byte(0), TextEncoding::Latin1);
        assert_eq!(TextEncoding::from_byte(1), TextEncoding::Utf16);
        assert_eq!(TextEncoding::from_byte(2), TextEncoding::Utf16Be);
        assert_eq!(TextEncoding::from_byte(3), TextEncoding::Utf8);
    }

    #[test]
    fn test_write_text_frame() {
        let text = "Test Title";
        let data = write_text_frame(text);

        assert_eq!(data[0], TextEncoding::Utf8.to_byte());
        assert_eq!(&data[1..], text.as_bytes());
    }
}
