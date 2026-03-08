//! MP4 box (atom) parsing.
//!
//! This module provides parsers for various MP4 box types including:
//! - `ftyp` - File type and compatibility
//! - `moov` - Movie container (metadata)
//! - `mvhd` - Movie header
//! - `trak` - Track container
//! - `tkhd` - Track header
//! - Sample tables (`stts`, `stsc`, `stsz`, `stco`, `co64`, `stss`, `ctts`)

use super::atom::Mp4Atom;
use oximedia_core::{OxiError, OxiResult};

/// Box header containing size and type information.
///
/// Every MP4 box starts with an 8-byte header (or 16 bytes for extended size).
#[derive(Clone, Debug)]
pub struct BoxHeader {
    /// Total box size including header (0 means extends to end of file).
    pub size: u64,
    /// Box type (4CC).
    pub box_type: BoxType,
    /// Header size in bytes (8 for normal, 16 for extended).
    pub header_size: u8,
}

/// Box type represented as a 4-byte code (4CC).
///
/// Common box types are available as constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BoxType(pub [u8; 4]);

impl BoxType {
    /// Creates a box type from a 4-byte array.
    #[must_use]
    pub const fn new(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }

    /// Creates a box type from a string.
    ///
    /// The string should be exactly 4 ASCII characters.
    /// Missing bytes are padded with zeros.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        let bytes = s.as_bytes();
        Self([
            bytes.first().copied().unwrap_or(0),
            bytes.get(1).copied().unwrap_or(0),
            bytes.get(2).copied().unwrap_or(0),
            bytes.get(3).copied().unwrap_or(0),
        ])
    }

    /// Returns the box type as a string slice.
    ///
    /// Returns `"????"` if the bytes are not valid UTF-8.
    #[must_use]
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("????")
    }

    /// Returns the box type as a `u32` for easy comparison.
    #[must_use]
    pub const fn as_u32(&self) -> u32 {
        u32::from_be_bytes(self.0)
    }
}

impl std::fmt::Display for BoxType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// Common box type constants
impl BoxType {
    /// File type box.
    pub const FTYP: Self = Self::new(*b"ftyp");
    /// Movie container box.
    pub const MOOV: Self = Self::new(*b"moov");
    /// Movie header box.
    pub const MVHD: Self = Self::new(*b"mvhd");
    /// Track container box.
    pub const TRAK: Self = Self::new(*b"trak");
    /// Track header box.
    pub const TKHD: Self = Self::new(*b"tkhd");
    /// Media container box.
    pub const MDIA: Self = Self::new(*b"mdia");
    /// Media header box.
    pub const MDHD: Self = Self::new(*b"mdhd");
    /// Handler reference box.
    pub const HDLR: Self = Self::new(*b"hdlr");
    /// Media information box.
    pub const MINF: Self = Self::new(*b"minf");
    /// Sample table box.
    pub const STBL: Self = Self::new(*b"stbl");
    /// Sample description box.
    pub const STSD: Self = Self::new(*b"stsd");
    /// Time-to-sample box.
    pub const STTS: Self = Self::new(*b"stts");
    /// Sample-to-chunk box.
    pub const STSC: Self = Self::new(*b"stsc");
    /// Sample size box.
    pub const STSZ: Self = Self::new(*b"stsz");
    /// Chunk offset box (32-bit).
    pub const STCO: Self = Self::new(*b"stco");
    /// Chunk offset box (64-bit).
    pub const CO64: Self = Self::new(*b"co64");
    /// Sync sample box.
    pub const STSS: Self = Self::new(*b"stss");
    /// Composition time-to-sample box.
    pub const CTTS: Self = Self::new(*b"ctts");
    /// Media data box.
    pub const MDAT: Self = Self::new(*b"mdat");
    /// Free space box.
    pub const FREE: Self = Self::new(*b"free");
    /// Free space box (alternate).
    pub const SKIP: Self = Self::new(*b"skip");
    /// User data box.
    pub const UDTA: Self = Self::new(*b"udta");
    /// Metadata box.
    pub const META: Self = Self::new(*b"meta");
    /// Edit list container box.
    pub const EDTS: Self = Self::new(*b"edts");
    /// Edit list box.
    pub const ELST: Self = Self::new(*b"elst");
}

impl BoxHeader {
    /// Parses a box header from the beginning of a byte slice.
    ///
    /// The header is 8 bytes for normal boxes, or 16 bytes when using
    /// extended size (size field == 1).
    ///
    /// # Errors
    ///
    /// Returns an error if the data is too short.
    pub fn parse(data: &[u8]) -> OxiResult<Self> {
        if data.len() < 8 {
            return Err(OxiError::UnexpectedEof);
        }

        let size32 = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let mut box_type = [0u8; 4];
        box_type.copy_from_slice(&data[4..8]);

        let (size, header_size) = if size32 == 1 {
            // Extended size (64-bit)
            if data.len() < 16 {
                return Err(OxiError::UnexpectedEof);
            }
            let size64 = u64::from_be_bytes([
                data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
            ]);
            (size64, 16)
        } else if size32 == 0 {
            // Box extends to end of file (size unknown)
            (0, 8)
        } else {
            (u64::from(size32), 8)
        };

        Ok(Self {
            size,
            box_type: BoxType(box_type),
            header_size,
        })
    }

    /// Returns the content size (excluding header).
    ///
    /// Returns 0 if the box extends to end of file.
    #[must_use]
    pub const fn content_size(&self) -> u64 {
        if self.size == 0 {
            0 // Unknown - extends to EOF
        } else {
            self.size - self.header_size as u64
        }
    }
}

/// File type box (`ftyp`).
///
/// Identifies the file type and lists compatible brands.
#[derive(Clone, Debug)]
pub struct FtypBox {
    /// Major brand (e.g., "isom", "mp42").
    pub major_brand: BoxType,
    /// Minor version number.
    pub minor_version: u32,
    /// List of compatible brands.
    pub compatible_brands: Vec<BoxType>,
}

impl FtypBox {
    /// Parses the content of an `ftyp` box.
    ///
    /// # Arguments
    ///
    /// * `data` - Box content (after the header)
    ///
    /// # Errors
    ///
    /// Returns an error if the data is malformed.
    pub fn parse(data: &[u8]) -> OxiResult<Self> {
        if data.len() < 8 {
            return Err(OxiError::Parse {
                offset: 0,
                message: "ftyp box too short".into(),
            });
        }

        let mut major_brand = [0u8; 4];
        major_brand.copy_from_slice(&data[0..4]);
        let minor_version = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);

        let mut compatible_brands = Vec::new();
        let mut offset = 8;
        while offset + 4 <= data.len() {
            let mut brand = [0u8; 4];
            brand.copy_from_slice(&data[offset..offset + 4]);
            compatible_brands.push(BoxType(brand));
            offset += 4;
        }

        Ok(Self {
            major_brand: BoxType(major_brand),
            minor_version,
            compatible_brands,
        })
    }

    /// Checks if this file is a valid MP4/ISOBMFF container.
    ///
    /// Returns `true` if the major brand or any compatible brand
    /// is a recognized MP4 brand.
    #[must_use]
    pub fn is_mp4(&self) -> bool {
        let mp4_brands = [
            BoxType::from_str("isom"),
            BoxType::from_str("iso2"),
            BoxType::from_str("iso3"),
            BoxType::from_str("iso4"),
            BoxType::from_str("iso5"),
            BoxType::from_str("iso6"),
            BoxType::from_str("mp41"),
            BoxType::from_str("mp42"),
            BoxType::from_str("M4V "),
            BoxType::from_str("M4A "),
            BoxType::from_str("M4P "),
            BoxType::from_str("av01"), // AV1
            BoxType::from_str("avis"), // AV1 image sequence
        ];
        mp4_brands.contains(&self.major_brand)
            || self
                .compatible_brands
                .iter()
                .any(|b| mp4_brands.contains(b))
    }
}

/// Movie box (`moov`) containing all metadata.
#[derive(Clone, Debug, Default)]
pub struct MoovBox {
    /// Movie header (`mvhd`).
    pub mvhd: Option<MvhdBox>,
    /// Track boxes (`trak`).
    pub traks: Vec<TrakBox>,
}

/// Movie header box (`mvhd`).
#[derive(Clone, Debug)]
pub struct MvhdBox {
    /// Version (0 for 32-bit times, 1 for 64-bit).
    pub version: u8,
    /// Creation time (seconds since 1904).
    pub creation_time: u64,
    /// Modification time (seconds since 1904).
    pub modification_time: u64,
    /// Time units per second.
    pub timescale: u32,
    /// Duration in timescale units.
    pub duration: u64,
    /// Preferred playback rate (1.0 = normal).
    pub rate: f64,
    /// Preferred volume (1.0 = full).
    pub volume: f64,
    /// Next track ID to use.
    pub next_track_id: u32,
}

impl MvhdBox {
    /// Parses the content of an `mvhd` box.
    ///
    /// # Arguments
    ///
    /// * `data` - Box content (after the header)
    ///
    /// # Errors
    ///
    /// Returns an error if the data is malformed.
    pub fn parse(data: &[u8]) -> OxiResult<Self> {
        let mut atom = Mp4Atom::new(data);

        let version = atom.read_u8()?;
        atom.skip(3)?; // flags

        let (creation_time, modification_time, timescale, duration) = if version == 1 {
            (
                atom.read_u64()?,
                atom.read_u64()?,
                atom.read_u32()?,
                atom.read_u64()?,
            )
        } else {
            (
                u64::from(atom.read_u32()?),
                u64::from(atom.read_u32()?),
                atom.read_u32()?,
                u64::from(atom.read_u32()?),
            )
        };

        let rate = atom.read_fixed_16_16()?;
        let volume = atom.read_fixed_8_8()?;

        // Skip: reserved (2 bytes) + reserved (2 * 4 bytes)
        atom.skip(2 + 8)?;
        // Skip: matrix (9 * 4 bytes)
        atom.skip(36)?;
        // Skip: pre_defined (6 * 4 bytes)
        atom.skip(24)?;

        let next_track_id = atom.read_u32()?;

        Ok(Self {
            version,
            creation_time,
            modification_time,
            timescale,
            duration,
            rate,
            volume,
            next_track_id,
        })
    }

    /// Returns the duration in seconds.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn duration_seconds(&self) -> f64 {
        if self.timescale == 0 {
            0.0
        } else {
            self.duration as f64 / f64::from(self.timescale)
        }
    }
}

/// Track box (`trak`) containing track metadata.
#[derive(Clone, Debug, Default)]
pub struct TrakBox {
    /// Track header (`tkhd`).
    pub tkhd: Option<TkhdBox>,
    /// Media timescale (from `mdhd`).
    pub timescale: u32,
    /// Handler type ("vide", "soun", "text", etc.).
    pub handler_type: String,
    /// Codec tag from sample description.
    pub codec_tag: u32,
    /// Video width in pixels (if video track).
    pub width: Option<u32>,
    /// Video height in pixels (if video track).
    pub height: Option<u32>,
    /// Audio sample rate in Hz (if audio track).
    pub sample_rate: Option<u32>,
    /// Audio channel count (if audio track).
    pub channels: Option<u16>,
    /// Time-to-sample entries (`stts`).
    pub stts_entries: Vec<SttsEntry>,
    /// Sample-to-chunk entries (`stsc`).
    pub stsc_entries: Vec<StscEntry>,
    /// Individual sample sizes (if variable, from `stsz`).
    pub sample_sizes: Vec<u32>,
    /// Default sample size (if all samples are the same size).
    pub default_sample_size: u32,
    /// Chunk offsets (`stco` or `co64`).
    pub chunk_offsets: Vec<u64>,
    /// Sync sample numbers (`stss`), `None` if all samples are sync.
    pub sync_samples: Option<Vec<u32>>,
    /// Composition time offsets (`ctts`).
    pub ctts_entries: Vec<CttsEntry>,
    /// Codec-specific extradata (e.g., AV1 config).
    pub extradata: Option<Vec<u8>>,
}

/// Track header box (`tkhd`).
#[derive(Clone, Debug)]
pub struct TkhdBox {
    /// Track ID (1-based).
    pub track_id: u32,
    /// Duration in movie timescale units.
    pub duration: u64,
    /// Display width (16.16 fixed-point).
    pub width: f64,
    /// Display height (16.16 fixed-point).
    pub height: f64,
}

impl TkhdBox {
    /// Parses the content of a `tkhd` box.
    ///
    /// # Arguments
    ///
    /// * `data` - Box content (after the header)
    ///
    /// # Errors
    ///
    /// Returns an error if the data is malformed.
    pub fn parse(data: &[u8]) -> OxiResult<Self> {
        let mut atom = Mp4Atom::new(data);

        let version = atom.read_u8()?;
        atom.skip(3)?; // flags

        let (creation_time, modification_time, track_id, duration) = if version == 1 {
            let ct = atom.read_u64()?;
            let mt = atom.read_u64()?;
            let tid = atom.read_u32()?;
            atom.skip(4)?; // reserved
            let dur = atom.read_u64()?;
            (ct, mt, tid, dur)
        } else {
            let ct = u64::from(atom.read_u32()?);
            let mt = u64::from(atom.read_u32()?);
            let tid = atom.read_u32()?;
            atom.skip(4)?; // reserved
            let dur = u64::from(atom.read_u32()?);
            (ct, mt, tid, dur)
        };

        // Silence unused variable warnings
        let _ = (creation_time, modification_time);

        // Skip: reserved (2 * 4 bytes)
        atom.skip(8)?;
        // Skip: layer, alternate_group
        atom.skip(4)?;
        // Skip: volume, reserved
        atom.skip(4)?;
        // Skip: matrix (9 * 4 bytes)
        atom.skip(36)?;

        let width = atom.read_fixed_16_16()?;
        let height = atom.read_fixed_16_16()?;

        Ok(Self {
            track_id,
            duration,
            width,
            height,
        })
    }
}

/// Time-to-sample entry (from `stts` box).
#[derive(Clone, Debug)]
pub struct SttsEntry {
    /// Number of consecutive samples with this duration.
    pub sample_count: u32,
    /// Duration of each sample in timescale units.
    pub sample_delta: u32,
}

/// Sample-to-chunk entry (from `stsc` box).
#[derive(Clone, Debug)]
pub struct StscEntry {
    /// First chunk number using this entry (1-based).
    pub first_chunk: u32,
    /// Number of samples in each chunk.
    pub samples_per_chunk: u32,
    /// Sample description index (1-based).
    pub sample_description_index: u32,
}

/// Composition time offset entry (from `ctts` box).
#[derive(Clone, Debug)]
pub struct CttsEntry {
    /// Number of consecutive samples with this offset.
    pub sample_count: u32,
    /// Composition time offset (can be negative in version 1).
    pub sample_offset: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_header_normal() {
        let data = [0x00, 0x00, 0x00, 0x14, b'f', b't', b'y', b'p'];
        let header = BoxHeader::parse(&data).unwrap();
        assert_eq!(header.size, 20);
        assert_eq!(header.box_type, BoxType::FTYP);
        assert_eq!(header.header_size, 8);
        assert_eq!(header.content_size(), 12);
    }

    #[test]
    fn test_box_header_extended() {
        let data = [
            0x00, 0x00, 0x00, 0x01, // size = 1 (extended)
            b'm', b'd', b'a', b't', 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
            0x00, // extended size = 256
        ];
        let header = BoxHeader::parse(&data).unwrap();
        assert_eq!(header.size, 256);
        assert_eq!(header.box_type, BoxType::MDAT);
        assert_eq!(header.header_size, 16);
        assert_eq!(header.content_size(), 240);
    }

    #[test]
    fn test_box_header_to_eof() {
        let data = [0x00, 0x00, 0x00, 0x00, b'm', b'd', b'a', b't'];
        let header = BoxHeader::parse(&data).unwrap();
        assert_eq!(header.size, 0);
        assert_eq!(header.content_size(), 0);
    }

    #[test]
    fn test_box_type_constants() {
        assert_eq!(BoxType::FTYP.as_str(), "ftyp");
        assert_eq!(BoxType::MOOV.as_str(), "moov");
        assert_eq!(BoxType::MDAT.as_str(), "mdat");
    }

    #[test]
    fn test_box_type_from_str() {
        assert_eq!(BoxType::from_str("moov"), BoxType::MOOV);
        assert_eq!(BoxType::from_str("ftyp"), BoxType::FTYP);
    }

    #[test]
    fn test_box_type_display() {
        assert_eq!(format!("{}", BoxType::FTYP), "ftyp");
    }

    #[test]
    fn test_ftyp_parse() {
        // "isom" + version 0 + compatible brand "mp41"
        let data = [
            b'i', b's', b'o', b'm', // major brand
            0x00, 0x00, 0x00, 0x00, // minor version
            b'm', b'p', b'4', b'1', // compatible brand
        ];
        let ftyp = FtypBox::parse(&data).unwrap();
        assert_eq!(ftyp.major_brand.as_str(), "isom");
        assert_eq!(ftyp.minor_version, 0);
        assert_eq!(ftyp.compatible_brands.len(), 1);
        assert_eq!(ftyp.compatible_brands[0].as_str(), "mp41");
        assert!(ftyp.is_mp4());
    }

    #[test]
    fn test_ftyp_is_mp4() {
        let ftyp = FtypBox {
            major_brand: BoxType::from_str("av01"),
            minor_version: 0,
            compatible_brands: vec![],
        };
        assert!(ftyp.is_mp4());
    }

    #[test]
    fn test_mvhd_parse_v0() {
        #[rustfmt::skip]
        let data = [
            0x00, // version
            0x00, 0x00, 0x00, // flags
            0x00, 0x00, 0x00, 0x01, // creation_time
            0x00, 0x00, 0x00, 0x02, // modification_time
            0x00, 0x00, 0x03, 0xE8, // timescale = 1000
            0x00, 0x00, 0x27, 0x10, // duration = 10000 (10 seconds)
            0x00, 0x01, 0x00, 0x00, // rate = 1.0
            0x01, 0x00, // volume = 1.0
            0x00, 0x00, // reserved
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // reserved
            // matrix (36 bytes)
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00,
            // pre_defined (24 bytes)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x02, // next_track_id = 2
        ];
        let mvhd = MvhdBox::parse(&data).unwrap();
        assert_eq!(mvhd.version, 0);
        assert_eq!(mvhd.timescale, 1000);
        assert_eq!(mvhd.duration, 10000);
        assert!((mvhd.rate - 1.0).abs() < f64::EPSILON);
        assert!((mvhd.volume - 1.0).abs() < f64::EPSILON);
        assert_eq!(mvhd.next_track_id, 2);
        assert!((mvhd.duration_seconds() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tkhd_parse_v0() {
        #[rustfmt::skip]
        let data = [
            0x00, // version
            0x00, 0x00, 0x03, // flags (enabled, in_movie, in_preview)
            0x00, 0x00, 0x00, 0x01, // creation_time
            0x00, 0x00, 0x00, 0x02, // modification_time
            0x00, 0x00, 0x00, 0x01, // track_id = 1
            0x00, 0x00, 0x00, 0x00, // reserved
            0x00, 0x00, 0x27, 0x10, // duration = 10000
            // reserved (8 bytes)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // layer, alternate_group
            0x00, 0x00, 0x00, 0x00,
            // volume, reserved
            0x01, 0x00, 0x00, 0x00,
            // matrix (36 bytes)
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00,
            // width = 1920 (in 16.16 fixed-point)
            0x07, 0x80, 0x00, 0x00,
            // height = 1080 (in 16.16 fixed-point)
            0x04, 0x38, 0x00, 0x00,
        ];
        let tkhd = TkhdBox::parse(&data).unwrap();
        assert_eq!(tkhd.track_id, 1);
        assert_eq!(tkhd.duration, 10000);
        assert!((tkhd.width - 1920.0).abs() < 1.0);
        assert!((tkhd.height - 1080.0).abs() < 1.0);
    }

    #[test]
    fn test_box_type_as_u32() {
        assert_eq!(BoxType::FTYP.as_u32(), 0x66747970); // "ftyp"
    }
}
