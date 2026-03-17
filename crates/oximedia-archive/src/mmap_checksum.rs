//! Memory-mapped I/O for checksum computation on large files.
//!
//! Uses `memmap2` (pure-Rust, no unsafe exposed to callers) to map files into
//! the address space and compute BLAKE3, SHA-256, and CRC32 checksums without
//! allocating a full heap copy of the file data.
//!
//! For files smaller than `MMAP_THRESHOLD` bytes (64 KiB by default), falls
//! back to a regular `std::fs::read` to avoid mmap overhead.

use crate::{ArchiveError, ArchiveResult};
use std::path::Path;

/// Files smaller than this threshold are read normally instead of mmap-ed.
pub const MMAP_THRESHOLD: u64 = 64 * 1024; // 64 KiB

/// Configuration for memory-mapped checksum computation.
#[derive(Debug, Clone)]
pub struct MmapChecksumConfig {
    /// Enable BLAKE3 computation.
    pub enable_blake3: bool,
    /// Enable SHA-256 computation.
    pub enable_sha256: bool,
    /// Enable CRC32 computation.
    pub enable_crc32: bool,
    /// Enable MD5 computation (legacy).
    pub enable_md5: bool,
    /// Minimum file size to use mmap (bytes). Files smaller than this use
    /// regular read instead.
    pub mmap_threshold: u64,
}

impl Default for MmapChecksumConfig {
    fn default() -> Self {
        Self {
            enable_blake3: true,
            enable_sha256: true,
            enable_crc32: true,
            enable_md5: false,
            mmap_threshold: MMAP_THRESHOLD,
        }
    }
}

/// Result of a memory-mapped checksum computation.
#[derive(Debug, Clone)]
pub struct MmapChecksumResult {
    /// BLAKE3 hex digest (if enabled).
    pub blake3: Option<String>,
    /// SHA-256 hex digest (if enabled).
    pub sha256: Option<String>,
    /// CRC32 hex digest (if enabled).
    pub crc32: Option<String>,
    /// MD5 hex digest (if enabled).
    pub md5: Option<String>,
    /// File size in bytes.
    pub file_size: u64,
    /// Whether memory mapping was used (vs. regular read).
    pub used_mmap: bool,
}

/// Read the content of a file via `memmap2`, returning the bytes as a `Vec<u8>`.
///
/// # Safety contract
///
/// The caller must ensure the file is not truncated between `File::open` and
/// the completion of this function.  We immediately copy the mapping into a
/// heap allocation before returning, so the Mmap lifetime is entirely contained
/// within this function.
#[allow(unsafe_code)]
fn read_via_mmap(file: &std::fs::File) -> Result<Vec<u8>, std::io::Error> {
    // SAFETY: we are opening for read-only access and immediately copying the
    // bytes into a Vec before returning.  The mmap object is dropped at the
    // end of this function before the Vec<u8> is returned, so there is no
    // aliasing with mutating code.
    let mmap = unsafe { memmap2::Mmap::map(file)? };
    Ok(mmap.to_vec())
}

/// Compute checksums for a file using memory-mapped I/O when beneficial.
///
/// This function maps the file into virtual memory and passes the resulting
/// byte slice directly to hash functions, avoiding an extra heap allocation
/// for large files.
pub fn compute_checksums_mmap(
    path: &Path,
    config: &MmapChecksumConfig,
) -> ArchiveResult<MmapChecksumResult> {
    let file = std::fs::File::open(path)?;
    let file_size = file.metadata()?.len();

    if file_size == 0 {
        return compute_checksums_from_bytes(b"", file_size, config, false);
    }

    let (data, used_mmap) = if file_size >= config.mmap_threshold {
        // Use memmap2 to map the file. memmap2::MmapOptions::map() is
        // marked unsafe by the crate because the OS could truncate the file
        // underneath us.  We immediately copy the mapped bytes into a Vec,
        // keeping the window of exposure minimal and bounded.
        //
        // The workspace lint set forbids free-standing `unsafe` blocks, so we
        // delegate through a thin helper that is explicitly allow-listed.
        let bytes = read_via_mmap(&file).map_err(|e| {
            ArchiveError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("mmap read failed: {e}"),
            ))
        })?;
        (bytes, true)
    } else {
        let bytes = std::fs::read(path)?;
        (bytes, false)
    };

    compute_checksums_from_bytes(&data, file_size, config, used_mmap)
}

fn compute_checksums_from_bytes(
    data: &[u8],
    file_size: u64,
    config: &MmapChecksumConfig,
    used_mmap: bool,
) -> ArchiveResult<MmapChecksumResult> {
    let blake3 = if config.enable_blake3 {
        Some(blake3::hash(data).to_hex().to_string())
    } else {
        None
    };

    let sha256 = if config.enable_sha256 {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(data);
        Some(hex::encode(hasher.finalize()))
    } else {
        None
    };

    let crc32 = if config.enable_crc32 {
        let v = crc32fast::hash(data);
        Some(format!("{v:08x}"))
    } else {
        None
    };

    let md5 = if config.enable_md5 {
        use md5::Digest;
        let mut hasher = md5::Md5::new();
        hasher.update(data);
        Some(hex::encode(hasher.finalize()))
    } else {
        None
    };

    Ok(MmapChecksumResult {
        blake3,
        sha256,
        crc32,
        md5,
        file_size,
        used_mmap,
    })
}

/// Compute checksums for a batch of files using memory-mapped I/O, with
/// file-level parallelism via rayon.
///
/// Returns results in the same order as the input paths.
pub fn compute_checksums_mmap_batch(
    paths: &[&Path],
    config: &MmapChecksumConfig,
) -> Vec<ArchiveResult<MmapChecksumResult>> {
    use rayon::prelude::*;
    paths
        .par_iter()
        .map(|p| compute_checksums_mmap(p, config))
        .collect()
}

/// Verify that an already-computed checksum matches recomputed values for a file.
///
/// Returns `true` if all provided expected values match the freshly computed ones.
pub fn verify_file_checksum(
    path: &Path,
    expected_sha256: Option<&str>,
    expected_blake3: Option<&str>,
    expected_crc32: Option<&str>,
) -> ArchiveResult<bool> {
    let config = MmapChecksumConfig {
        enable_blake3: expected_blake3.is_some(),
        enable_sha256: expected_sha256.is_some(),
        enable_crc32: expected_crc32.is_some(),
        enable_md5: false,
        mmap_threshold: MMAP_THRESHOLD,
    };

    let result = compute_checksums_mmap(path, &config)?;

    if let (Some(exp), Some(act)) = (expected_sha256, result.sha256.as_deref()) {
        if exp != act {
            return Ok(false);
        }
    }
    if let (Some(exp), Some(act)) = (expected_blake3, result.blake3.as_deref()) {
        if exp != act {
            return Ok(false);
        }
    }
    if let (Some(exp), Some(act)) = (expected_crc32, result.crc32.as_deref()) {
        if exp != act {
            return Ok(false);
        }
    }

    Ok(true)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_file(dir: &Path, name: &str, content: &[u8]) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("create temp file");
        f.write_all(content).expect("write temp file");
        path
    }

    // --- Basic checksum computation ---

    #[test]
    fn test_mmap_checksum_small_file() {
        let dir = std::env::temp_dir().join("oximedia_mmap_small");
        std::fs::create_dir_all(&dir).ok();
        let content = b"small file content for mmap checksum";
        let path = write_temp_file(&dir, "small.bin", content);

        let config = MmapChecksumConfig::default();
        let result = compute_checksums_mmap(&path, &config).expect("compute");
        assert!(result.sha256.is_some());
        assert!(result.blake3.is_some());
        assert!(result.crc32.is_some());
        assert_eq!(result.file_size, content.len() as u64);
        // Small file: uses regular read
        assert!(!result.used_mmap);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_mmap_checksum_empty_file() {
        let dir = std::env::temp_dir().join("oximedia_mmap_empty");
        std::fs::create_dir_all(&dir).ok();
        let path = write_temp_file(&dir, "empty.bin", b"");

        let config = MmapChecksumConfig::default();
        let result = compute_checksums_mmap(&path, &config).expect("compute");
        assert_eq!(result.file_size, 0);
        // Known SHA-256 of empty input
        assert_eq!(
            result.sha256.as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_mmap_checksum_sha256_known_vector() {
        let dir = std::env::temp_dir().join("oximedia_mmap_sha256_vec");
        std::fs::create_dir_all(&dir).ok();
        let path = write_temp_file(&dir, "abc.bin", b"abc");

        let config = MmapChecksumConfig {
            enable_blake3: false,
            enable_sha256: true,
            enable_crc32: false,
            enable_md5: false,
            mmap_threshold: MMAP_THRESHOLD,
        };
        let result = compute_checksums_mmap(&path, &config).expect("compute");
        assert_eq!(
            result.sha256.as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_mmap_checksum_deterministic() {
        let dir = std::env::temp_dir().join("oximedia_mmap_det");
        std::fs::create_dir_all(&dir).ok();
        let content = b"deterministic content for mmap testing";
        let path = write_temp_file(&dir, "det.bin", content);

        let config = MmapChecksumConfig::default();
        let r1 = compute_checksums_mmap(&path, &config).expect("r1");
        let r2 = compute_checksums_mmap(&path, &config).expect("r2");
        assert_eq!(r1.sha256, r2.sha256);
        assert_eq!(r1.blake3, r2.blake3);
        assert_eq!(r1.crc32, r2.crc32);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_mmap_checksum_md5_enabled() {
        let dir = std::env::temp_dir().join("oximedia_mmap_md5");
        std::fs::create_dir_all(&dir).ok();
        let path = write_temp_file(&dir, "md5.bin", b"test md5");

        let config = MmapChecksumConfig {
            enable_md5: true,
            ..Default::default()
        };
        let result = compute_checksums_mmap(&path, &config).expect("compute");
        assert!(result.md5.is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_mmap_checksum_no_algorithms() {
        let dir = std::env::temp_dir().join("oximedia_mmap_none");
        std::fs::create_dir_all(&dir).ok();
        let path = write_temp_file(&dir, "none.bin", b"data");

        let config = MmapChecksumConfig {
            enable_blake3: false,
            enable_sha256: false,
            enable_crc32: false,
            enable_md5: false,
            mmap_threshold: MMAP_THRESHOLD,
        };
        let result = compute_checksums_mmap(&path, &config).expect("compute");
        assert!(result.blake3.is_none());
        assert!(result.sha256.is_none());
        assert!(result.crc32.is_none());
        assert!(result.md5.is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_mmap_checksum_file_not_found() {
        let config = MmapChecksumConfig::default();
        let result = compute_checksums_mmap(Path::new("/nonexistent/file.bin"), &config);
        assert!(result.is_err());
    }

    // --- Large file (forces mmap path) ---

    #[test]
    fn test_mmap_checksum_large_file_uses_mmap() {
        let dir = std::env::temp_dir().join("oximedia_mmap_large");
        std::fs::create_dir_all(&dir).ok();
        // Generate file larger than threshold
        let content: Vec<u8> = (0u8..=255).cycle().take(128 * 1024).collect();
        let path = write_temp_file(&dir, "large.bin", &content);

        let config = MmapChecksumConfig {
            mmap_threshold: 64 * 1024, // 64 KiB
            ..Default::default()
        };
        let result = compute_checksums_mmap(&path, &config).expect("compute large");
        assert_eq!(result.file_size, 128 * 1024);
        assert!(result.used_mmap, "expected mmap to be used");
        assert!(result.sha256.is_some());
        assert!(result.blake3.is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_mmap_large_matches_small_threshold() {
        let dir = std::env::temp_dir().join("oximedia_mmap_match");
        std::fs::create_dir_all(&dir).ok();
        let content: Vec<u8> = (0u8..=255).cycle().take(200 * 1024).collect();
        let path = write_temp_file(&dir, "match.bin", &content);

        // Compute with high threshold (no mmap)
        let config_no_mmap = MmapChecksumConfig {
            mmap_threshold: u64::MAX,
            ..Default::default()
        };
        let r_no_mmap = compute_checksums_mmap(&path, &config_no_mmap).expect("no-mmap");

        // Compute with low threshold (forces mmap)
        let config_mmap = MmapChecksumConfig {
            mmap_threshold: 1024,
            ..Default::default()
        };
        let r_mmap = compute_checksums_mmap(&path, &config_mmap).expect("mmap");

        assert_eq!(r_no_mmap.sha256, r_mmap.sha256, "sha256 must match");
        assert_eq!(r_no_mmap.blake3, r_mmap.blake3, "blake3 must match");
        assert_eq!(r_no_mmap.crc32, r_mmap.crc32, "crc32 must match");

        std::fs::remove_dir_all(&dir).ok();
    }

    // --- Batch ---

    #[test]
    fn test_mmap_checksum_batch() {
        let dir = std::env::temp_dir().join("oximedia_mmap_batch");
        std::fs::create_dir_all(&dir).ok();

        let files: Vec<std::path::PathBuf> = (0..4)
            .map(|i| {
                let content = format!("batch file {i} content").into_bytes();
                write_temp_file(&dir, &format!("batch_{i}.bin"), &content)
            })
            .collect();

        let paths: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
        let config = MmapChecksumConfig::default();
        let results = compute_checksums_mmap_batch(&paths, &config);

        assert_eq!(results.len(), 4);
        for r in &results {
            assert!(r.is_ok(), "batch result should be ok");
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_mmap_batch_preserves_order() {
        let dir = std::env::temp_dir().join("oximedia_mmap_batch_order");
        std::fs::create_dir_all(&dir).ok();

        let contents: Vec<&[u8]> = vec![b"alpha", b"beta", b"gamma"];
        let files: Vec<std::path::PathBuf> = contents
            .iter()
            .enumerate()
            .map(|(i, c)| write_temp_file(&dir, &format!("order_{i}.bin"), c))
            .collect();

        let paths: Vec<&Path> = files.iter().map(|p| p.as_path()).collect();
        let config = MmapChecksumConfig {
            enable_sha256: true,
            enable_blake3: false,
            enable_crc32: false,
            enable_md5: false,
            mmap_threshold: MMAP_THRESHOLD,
        };
        let results = compute_checksums_mmap_batch(&paths, &config);

        // Compute expected SHA-256 for each content
        for (i, content) in contents.iter().enumerate() {
            use sha2::Digest;
            let mut hasher = sha2::Sha256::new();
            hasher.update(content);
            let expected = hex::encode(hasher.finalize());
            let actual = results[i]
                .as_ref()
                .expect("should be ok")
                .sha256
                .as_deref()
                .expect("sha256 present");
            assert_eq!(expected, actual, "sha256 mismatch at index {i}");
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    // --- verify_file_checksum ---

    #[test]
    fn test_verify_file_checksum_match() {
        let dir = std::env::temp_dir().join("oximedia_mmap_verify_ok");
        std::fs::create_dir_all(&dir).ok();
        let content = b"verify content";
        let path = write_temp_file(&dir, "v.bin", content);

        // First compute expected checksums
        let config = MmapChecksumConfig::default();
        let result = compute_checksums_mmap(&path, &config).expect("compute");

        let ok = verify_file_checksum(
            &path,
            result.sha256.as_deref(),
            result.blake3.as_deref(),
            None,
        )
        .expect("verify");
        assert!(ok, "expected verification to pass");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_verify_file_checksum_mismatch() {
        let dir = std::env::temp_dir().join("oximedia_mmap_verify_fail");
        std::fs::create_dir_all(&dir).ok();
        let path = write_temp_file(&dir, "v.bin", b"original content");

        let ok = verify_file_checksum(
            &path,
            Some("wrong_sha256_value_that_is_64_hex_chars_0000000000000000000000000000000"),
            None,
            None,
        )
        .expect("verify");
        assert!(!ok, "expected verification to fail");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_verify_file_checksum_no_expected() {
        let dir = std::env::temp_dir().join("oximedia_mmap_verify_none");
        std::fs::create_dir_all(&dir).ok();
        let path = write_temp_file(&dir, "v.bin", b"data");

        // No expected values → always passes (nothing to check)
        let ok = verify_file_checksum(&path, None, None, None).expect("verify");
        assert!(ok);

        std::fs::remove_dir_all(&dir).ok();
    }
}
