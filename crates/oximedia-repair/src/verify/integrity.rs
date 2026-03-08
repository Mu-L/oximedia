//! Integrity verification.
//!
//! This module provides functions to verify file integrity after repair.

use crate::{RepairError, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Verify file integrity.
pub fn verify_integrity(path: &Path) -> Result<()> {
    let mut file = File::open(path)?;

    // Check file is not empty
    let size = file.metadata()?.len();
    if size == 0 {
        return Err(RepairError::VerificationFailed("File is empty".to_string()));
    }

    // Verify header
    verify_header(&mut file)?;

    // Verify structure
    verify_structure(&mut file)?;

    Ok(())
}

/// Verify file header.
fn verify_header(file: &mut File) -> Result<()> {
    file.seek(SeekFrom::Start(0))?;

    let mut header = [0u8; 16];
    file.read_exact(&mut header)?;

    // Check for valid format
    let valid = is_valid_header(&header);

    if !valid {
        return Err(RepairError::VerificationFailed(
            "Invalid file header".to_string(),
        ));
    }

    Ok(())
}

/// Check if header is valid.
fn is_valid_header(header: &[u8]) -> bool {
    // Check for known formats
    if header.len() >= 12 {
        if &header[0..4] == b"RIFF" && &header[8..12] == b"AVI " {
            return true;
        }
        if &header[4..8] == b"ftyp" {
            return true;
        }
        if header[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
            return true;
        }
    }

    false
}

/// Verify file structure.
fn verify_structure(file: &mut File) -> Result<()> {
    let size = file.metadata()?.len();

    // Check file doesn't end with zeros
    if size > 16 {
        file.seek(SeekFrom::End(-16))?;
        let mut tail = [0u8; 16];
        file.read_exact(&mut tail)?;

        if tail.iter().all(|&b| b == 0) {
            return Err(RepairError::VerificationFailed(
                "File ends with zeros".to_string(),
            ));
        }
    }

    Ok(())
}

/// Calculate file checksum.
pub fn calculate_checksum(path: &Path) -> Result<u32> {
    let mut file = File::open(path)?;
    let mut checksum = 0u32;
    let mut buffer = [0u8; 4096];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        for &byte in &buffer[..bytes_read] {
            checksum = checksum.wrapping_add(byte as u32);
        }
    }

    Ok(checksum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_header_avi() {
        let header = b"RIFF\x00\x00\x00\x00AVI \x00\x00\x00\x00";
        assert!(is_valid_header(header));
    }

    #[test]
    fn test_is_valid_header_mp4() {
        let header = b"\x00\x00\x00\x20ftypmp42";
        assert!(is_valid_header(header));
    }

    #[test]
    fn test_is_valid_header_invalid() {
        let header = b"\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF";
        assert!(!is_valid_header(header));
    }
}
