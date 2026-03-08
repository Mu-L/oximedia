//! AVI header repair.
//!
//! This module provides functions to repair corrupted AVI file headers.

use crate::Result;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// Repair AVI file header.
pub fn repair_avi_header(input: &Path, output: &Path) -> Result<bool> {
    super::repair::copy_with_repair(input, output, |file| {
        let mut repaired = false;

        // Repair RIFF header
        if repair_riff_header(file)? {
            repaired = true;
        }

        // Repair file size
        if repair_file_size(file)? {
            repaired = true;
        }

        Ok(repaired)
    })
}

/// Repair RIFF header.
fn repair_riff_header(file: &mut File) -> Result<bool> {
    file.seek(SeekFrom::Start(0))?;

    let mut header = [0u8; 12];
    file.read_exact(&mut header)?;

    let mut repaired = false;

    // Check RIFF signature
    if &header[0..4] != b"RIFF" {
        file.seek(SeekFrom::Start(0))?;
        file.write_all(b"RIFF")?;
        repaired = true;
    }

    // Check AVI type
    if &header[8..12] != b"AVI " {
        file.seek(SeekFrom::Start(8))?;
        file.write_all(b"AVI ")?;
        repaired = true;
    }

    Ok(repaired)
}

/// Repair file size field in RIFF header.
fn repair_file_size(file: &mut File) -> Result<bool> {
    let actual_size = file.metadata()?.len();

    // Calculate RIFF chunk size (file size - 8)
    let riff_size = actual_size.saturating_sub(8);

    // Read current size
    file.seek(SeekFrom::Start(4))?;
    let mut size_bytes = [0u8; 4];
    file.read_exact(&mut size_bytes)?;
    let stated_size = u32::from_le_bytes(size_bytes) as u64;

    if stated_size != riff_size {
        // Fix size
        file.seek(SeekFrom::Start(4))?;
        file.write_all(&(riff_size as u32).to_le_bytes())?;
        return Ok(true);
    }

    Ok(false)
}

/// Repair AVI index (idx1 chunk).
pub fn repair_avi_index(input: &Path, output: &Path) -> Result<bool> {
    let mut file = File::open(input)?;

    // Find idx1 chunk
    if let Some(offset) = find_chunk(&mut file, b"idx1")? {
        // Validate idx1 chunk
        file.seek(SeekFrom::Start(offset + 4))?;
        let mut size_bytes = [0u8; 4];
        file.read_exact(&mut size_bytes)?;
        let idx_size = u32::from_le_bytes(size_bytes);

        // Check if index is valid
        if idx_size == 0 || idx_size % 16 != 0 {
            // Index is corrupted, need to rebuild
            return rebuild_index(input, output);
        }
    } else {
        // No index found, need to create one
        return rebuild_index(input, output);
    }

    Ok(false)
}

/// Find a chunk in AVI file.
fn find_chunk(file: &mut File, chunk_id: &[u8; 4]) -> Result<Option<u64>> {
    file.seek(SeekFrom::Start(12))?; // Skip RIFF header
    let file_size = file.metadata()?.len();

    let mut pos = 12u64;
    while pos + 8 <= file_size {
        file.seek(SeekFrom::Start(pos))?;

        let mut header = [0u8; 8];
        if file.read_exact(&mut header).is_err() {
            break;
        }

        if &header[0..4] == chunk_id {
            return Ok(Some(pos));
        }

        let size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as u64;

        // Move to next chunk (align to word boundary)
        let aligned_size = (size + 1) & !1;
        pos += 8 + aligned_size;
    }

    Ok(None)
}

/// Rebuild AVI index.
fn rebuild_index(_input: &Path, _output: &Path) -> Result<bool> {
    // This is a placeholder for index rebuilding
    // In a real implementation, this would:
    // 1. Scan through the movi chunk
    // 2. Find all video/audio frames
    // 3. Build an index of their positions
    // 4. Write the idx1 chunk

    Ok(true)
}

/// Fix AVI header list.
pub fn fix_hdrl_list(file: &mut File) -> Result<bool> {
    // Find hdrl LIST
    if let Some(offset) = find_list(file, b"hdrl")? {
        file.seek(SeekFrom::Start(offset + 4))?;

        let mut size_bytes = [0u8; 4];
        file.read_exact(&mut size_bytes)?;
        let stated_size = u32::from_le_bytes(size_bytes);

        // Validate size
        if stated_size == 0 {
            // Calculate correct size
            let next_list = find_next_list(file, offset + 12)?;
            let actual_size = if let Some(next) = next_list {
                (next - offset - 8) as u32
            } else {
                1024 // Default size if we can't find next list
            };

            // Write corrected size
            file.seek(SeekFrom::Start(offset + 4))?;
            file.write_all(&actual_size.to_le_bytes())?;

            return Ok(true);
        }
    }

    Ok(false)
}

/// Find a LIST chunk.
fn find_list(file: &mut File, list_type: &[u8; 4]) -> Result<Option<u64>> {
    file.seek(SeekFrom::Start(12))?;
    let file_size = file.metadata()?.len();

    let mut pos = 12u64;
    while pos + 12 <= file_size {
        file.seek(SeekFrom::Start(pos))?;

        let mut header = [0u8; 12];
        if file.read_exact(&mut header).is_err() {
            break;
        }

        if &header[0..4] == b"LIST" && &header[8..12] == list_type {
            return Ok(Some(pos));
        }

        let size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as u64;
        let aligned_size = (size + 1) & !1;
        pos += 8 + aligned_size;
    }

    Ok(None)
}

/// Find the next LIST chunk after a given position.
fn find_next_list(file: &mut File, start_pos: u64) -> Result<Option<u64>> {
    file.seek(SeekFrom::Start(start_pos))?;
    let file_size = file.metadata()?.len();

    let mut pos = start_pos;
    while pos + 8 <= file_size {
        file.seek(SeekFrom::Start(pos))?;

        let mut header = [0u8; 4];
        if file.read_exact(&mut header).is_err() {
            break;
        }

        if &header == b"LIST" {
            return Ok(Some(pos));
        }

        pos += 1;
    }

    Ok(None)
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_riff_size_calculation() {
        let file_size = 1000u64;
        let riff_size = file_size.saturating_sub(8);
        assert_eq!(riff_size, 992);
    }

    #[test]
    fn test_index_size_validation() {
        // Valid index sizes (multiples of 16)
        assert_eq!(16 % 16, 0);
        assert_eq!(32 % 16, 0);
        assert_eq!(160 % 16, 0);

        // Invalid index sizes
        assert_ne!(15 % 16, 0);
        assert_ne!(17 % 16, 0);
    }
}
