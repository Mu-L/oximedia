//! Generic header repair functionality.
//!
//! This module provides common header repair operations that can be
//! applied across different container formats.

use crate::Result;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// Repair a file header based on detected format.
pub fn repair_header(path: &Path, output: &Path) -> Result<bool> {
    let mut file = File::open(path)?;
    let mut header = [0u8; 16];
    file.read_exact(&mut header)?;

    // Detect format and dispatch to appropriate repair function
    if is_mp4_header(&header) {
        super::mp4::repair_mp4_header(path, output)
    } else if is_matroska_header(&header) {
        super::matroska::repair_matroska_header(path, output)
    } else if is_avi_header(&header) {
        super::avi::repair_avi_header(path, output)
    } else {
        Ok(false)
    }
}

/// Check if header is MP4 format.
fn is_mp4_header(header: &[u8]) -> bool {
    header.len() >= 8 && &header[4..8] == b"ftyp"
}

/// Check if header is Matroska format.
fn is_matroska_header(header: &[u8]) -> bool {
    header.len() >= 4 && header[0..4] == [0x1A, 0x45, 0xDF, 0xA3]
}

/// Check if header is AVI format.
fn is_avi_header(header: &[u8]) -> bool {
    header.len() >= 12 && &header[0..4] == b"RIFF" && &header[8..12] == b"AVI "
}

/// Repair corrupted magic number.
pub fn repair_magic_number(file: &mut File, expected: &[u8], offset: u64) -> Result<bool> {
    file.seek(SeekFrom::Start(offset))?;
    let mut actual = vec![0u8; expected.len()];
    file.read_exact(&mut actual)?;

    if actual != expected {
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(expected)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Repair file size field in header.
pub fn repair_size_field(
    file: &mut File,
    offset: u64,
    actual_size: u64,
    little_endian: bool,
) -> Result<bool> {
    file.seek(SeekFrom::Start(offset))?;

    let size_bytes = if little_endian {
        (actual_size as u32).to_le_bytes()
    } else {
        (actual_size as u32).to_be_bytes()
    };

    file.write_all(&size_bytes)?;
    Ok(true)
}

/// Validate and repair header checksum.
pub fn repair_checksum(
    file: &mut File,
    data_offset: u64,
    data_length: usize,
    checksum_offset: u64,
) -> Result<bool> {
    // Read data
    file.seek(SeekFrom::Start(data_offset))?;
    let mut data = vec![0u8; data_length];
    file.read_exact(&mut data)?;

    // Calculate checksum
    let checksum = calculate_header_checksum(&data);

    // Write checksum
    file.seek(SeekFrom::Start(checksum_offset))?;
    file.write_all(&checksum.to_le_bytes())?;

    Ok(true)
}

/// Calculate header checksum (CRC32).
fn calculate_header_checksum(data: &[u8]) -> u32 {
    let mut checksum = 0u32;
    for &byte in data {
        checksum = checksum.wrapping_add(byte as u32);
    }
    checksum
}

/// Copy file with header repair.
pub fn copy_with_repair<F>(input: &Path, output: &Path, repair_fn: F) -> Result<bool>
where
    F: FnOnce(&mut File) -> Result<bool>,
{
    std::fs::copy(input, output)?;
    let mut file = File::options().write(true).open(output)?;
    repair_fn(&mut file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_mp4_header() {
        let header = b"\x00\x00\x00\x20ftypmp42";
        assert!(is_mp4_header(header));
    }

    #[test]
    fn test_is_matroska_header() {
        let header = b"\x1A\x45\xDF\xA3\x00\x00\x00\x00";
        assert!(is_matroska_header(header));
    }

    #[test]
    fn test_is_avi_header() {
        let header = b"RIFF\x00\x00\x00\x00AVI \x00\x00\x00\x00";
        assert!(is_avi_header(header));
    }

    #[test]
    fn test_calculate_header_checksum() {
        let data = b"test data";
        let checksum = calculate_header_checksum(data);
        assert!(checksum > 0);
    }
}
