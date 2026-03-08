//! ICC profile parser for detailed tag extraction.

use crate::error::{ColorError, Result};
use std::collections::HashMap;

/// ICC tag signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TagSignature(pub [u8; 4]);

impl TagSignature {
    /// Creates a new tag signature from bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }

    /// Red colorant tag.
    pub const RED_COLORANT: Self = Self(*b"rXYZ");
    /// Green colorant tag.
    pub const GREEN_COLORANT: Self = Self(*b"gXYZ");
    /// Blue colorant tag.
    pub const BLUE_COLORANT: Self = Self(*b"bXYZ");
    /// Red TRC tag.
    pub const RED_TRC: Self = Self(*b"rTRC");
    /// Green TRC tag.
    pub const GREEN_TRC: Self = Self(*b"gTRC");
    /// Blue TRC tag.
    pub const BLUE_TRC: Self = Self(*b"bTRC");
    /// Profile description tag.
    pub const DESCRIPTION: Self = Self(*b"desc");
    /// Copyright tag.
    pub const COPYRIGHT: Self = Self(*b"cprt");
    /// White point tag.
    pub const WHITE_POINT: Self = Self(*b"wtpt");
    /// Media white point tag.
    pub const MEDIA_WHITE_POINT: Self = Self(*b"wtpt");
    /// Chromatic adaptation tag.
    pub const CHROMATIC_ADAPTATION: Self = Self(*b"chad");
}

/// ICC profile tag table entry.
#[derive(Clone, Debug)]
pub struct TagEntry {
    /// Tag signature
    pub signature: TagSignature,
    /// Offset to tag data
    pub offset: u32,
    /// Size of tag data
    pub size: u32,
}

/// ICC profile parser.
pub struct IccParser {
    data: Vec<u8>,
    tag_table: HashMap<TagSignature, TagEntry>,
}

impl IccParser {
    /// Parses an ICC profile from bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the profile is invalid.
    pub fn new(data: Vec<u8>) -> Result<Self> {
        if data.len() < 128 {
            return Err(ColorError::IccProfile("Profile too small".to_string()));
        }

        // Verify signature
        if &data[36..40] != b"acsp" {
            return Err(ColorError::IccProfile("Invalid signature".to_string()));
        }

        // Parse tag table
        let tag_count = u32::from_be_bytes([data[128], data[129], data[130], data[131]]) as usize;
        let mut tag_table = HashMap::new();

        for i in 0..tag_count {
            let offset = 132 + i * 12;
            if offset + 12 > data.len() {
                break;
            }

            let sig_bytes = [
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ];
            let signature = TagSignature(sig_bytes);

            let tag_offset = u32::from_be_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            let tag_size = u32::from_be_bytes([
                data[offset + 8],
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
            ]);

            tag_table.insert(
                signature,
                TagEntry {
                    signature,
                    offset: tag_offset,
                    size: tag_size,
                },
            );
        }

        Ok(Self { data, tag_table })
    }

    /// Gets a tag's data.
    ///
    /// # Errors
    ///
    /// Returns an error if the tag doesn't exist or is invalid.
    pub fn get_tag(&self, signature: TagSignature) -> Result<&[u8]> {
        let entry = self
            .tag_table
            .get(&signature)
            .ok_or_else(|| ColorError::IccProfile(format!("Tag not found: {:?}", signature.0)))?;

        let start = entry.offset as usize;
        let end = start + entry.size as usize;

        if end > self.data.len() {
            return Err(ColorError::IccProfile(
                "Tag extends beyond profile".to_string(),
            ));
        }

        Ok(&self.data[start..end])
    }

    /// Parses an XYZ value from tag data.
    ///
    /// # Errors
    ///
    /// Returns an error if the tag is invalid.
    pub fn parse_xyz(&self, signature: TagSignature) -> Result<[f64; 3]> {
        let data = self.get_tag(signature)?;

        if data.len() < 20 {
            return Err(ColorError::IccProfile("XYZ tag too small".to_string()));
        }

        // XYZ type signature should be "XYZ " (0x58595A20)
        if &data[0..4] != b"XYZ " {
            return Err(ColorError::IccProfile("Invalid XYZ tag type".to_string()));
        }

        // Parse X, Y, Z as s15Fixed16Number (signed 32-bit fixed point)
        let x = parse_s15fixed16(&data[8..12]);
        let y = parse_s15fixed16(&data[12..16]);
        let z = parse_s15fixed16(&data[16..20]);

        Ok([x, y, z])
    }

    /// Parses a curve (TRC) from tag data.
    ///
    /// # Errors
    ///
    /// Returns an error if the tag is invalid.
    pub fn parse_curve(&self, signature: TagSignature) -> Result<Vec<f32>> {
        let data = self.get_tag(signature)?;

        if data.len() < 12 {
            return Err(ColorError::IccProfile("Curve tag too small".to_string()));
        }

        // Curve type signature should be "curv" (0x63757276)
        if &data[0..4] != b"curv" {
            return Err(ColorError::IccProfile("Invalid curve tag type".to_string()));
        }

        let count = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;

        if count == 0 {
            // Linear curve
            return Ok(vec![0.0, 1.0]);
        }

        if count == 1 {
            // Gamma curve (single value)
            let gamma = f32::from(u16::from_be_bytes([data[12], data[13]])) / 256.0;
            // Generate a LUT for the gamma curve
            let mut lut = Vec::with_capacity(256);
            for i in 0..256 {
                let t = i as f32 / 255.0;
                lut.push(t.powf(gamma));
            }
            return Ok(lut);
        }

        // Table curve
        let mut curve = Vec::with_capacity(count);
        for i in 0..count {
            let offset = 12 + i * 2;
            if offset + 2 > data.len() {
                break;
            }
            let value = f32::from(u16::from_be_bytes([data[offset], data[offset + 1]])) / 65535.0;
            curve.push(value);
        }

        Ok(curve)
    }

    /// Parses profile description.
    ///
    /// # Errors
    ///
    /// Returns an error if the tag is invalid.
    pub fn parse_description(&self) -> Result<String> {
        let data = self.get_tag(TagSignature::DESCRIPTION)?;

        if data.len() < 12 {
            return Ok(String::new());
        }

        // Description can be either "desc" or "mluc" (multilingual)
        if &data[0..4] == b"desc" {
            // Legacy text description
            let count = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;
            if 12 + count <= data.len() {
                let text = String::from_utf8_lossy(&data[12..12 + count]);
                return Ok(text.trim_end_matches('\0').to_string());
            }
        }

        Ok(String::new())
    }

    /// Lists all tags in the profile.
    #[must_use]
    pub fn list_tags(&self) -> Vec<TagSignature> {
        self.tag_table.keys().copied().collect()
    }
}

/// Parses s15Fixed16Number format (ICC spec).
fn parse_s15fixed16(bytes: &[u8]) -> f64 {
    if bytes.len() < 4 {
        return 0.0;
    }

    let value = i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    f64::from(value) / 65536.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_signature() {
        let sig = TagSignature::RED_COLORANT;
        assert_eq!(sig.0, *b"rXYZ");
    }

    #[test]
    fn test_parse_s15fixed16() {
        // Test 1.0
        let value = parse_s15fixed16(&[0x00, 0x01, 0x00, 0x00]);
        assert!((value - 1.0).abs() < 0.001);

        // Test 0.5
        let value = parse_s15fixed16(&[0x00, 0x00, 0x80, 0x00]);
        assert!((value - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_invalid_profile() {
        let data = vec![0u8; 100];
        assert!(IccParser::new(data).is_err());
    }
}
