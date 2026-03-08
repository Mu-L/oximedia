#![allow(dead_code)]
//! Proxy fingerprinting for integrity verification.
//!
//! Generates and verifies content-based fingerprints for proxy files to ensure
//! they have not been corrupted or tampered with during transfer, storage,
//! or editing workflows.

use std::collections::HashMap;
use std::fmt;

/// Hash algorithm used for fingerprinting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FingerprintAlgorithm {
    /// CRC-32 (fast but weak).
    Crc32,
    /// Adler-32 (fast checksum).
    Adler32,
    /// Simple XOR-based hash (very fast, low quality).
    XorHash,
    /// Block-level content hash.
    BlockHash,
}

impl FingerprintAlgorithm {
    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Crc32 => "CRC-32",
            Self::Adler32 => "Adler-32",
            Self::XorHash => "XOR Hash",
            Self::BlockHash => "Block Hash",
        }
    }
}

/// A content fingerprint for a proxy file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    /// The algorithm used to generate this fingerprint.
    pub algorithm: FingerprintAlgorithm,
    /// The fingerprint value as a hex string.
    pub hash: String,
    /// File size in bytes at the time of fingerprinting.
    pub file_size: u64,
    /// Number of blocks processed.
    pub blocks_processed: u64,
}

impl Fingerprint {
    /// Create a new fingerprint.
    pub fn new(algorithm: FingerprintAlgorithm, hash: &str, file_size: u64) -> Self {
        Self {
            algorithm,
            hash: hash.to_string(),
            file_size,
            blocks_processed: 0,
        }
    }

    /// Set the blocks processed count.
    pub fn with_blocks(mut self, blocks: u64) -> Self {
        self.blocks_processed = blocks;
        self
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.algorithm.name(), self.hash)
    }
}

/// Result of a fingerprint verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyResult {
    /// Fingerprints match.
    Match,
    /// Fingerprints do not match.
    Mismatch {
        /// Expected hash.
        expected: String,
        /// Actual hash.
        actual: String,
    },
    /// File size changed.
    SizeChanged {
        /// Expected size.
        expected: u64,
        /// Actual size.
        actual: u64,
    },
}

impl VerifyResult {
    /// Whether verification passed.
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Match)
    }
}

/// Simple CRC-32 computation (non-cryptographic, for proxy integrity only).
fn compute_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Simple Adler-32 computation.
fn compute_adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Simple XOR hash.
fn compute_xor_hash(data: &[u8]) -> u32 {
    let mut hash: u32 = 0;
    for chunk in data.chunks(4) {
        let mut val: u32 = 0;
        for (i, &byte) in chunk.iter().enumerate() {
            val |= u32::from(byte) << (i * 8);
        }
        hash ^= val;
    }
    hash
}

/// Block-level hash: hash each block and combine.
#[allow(clippy::cast_precision_loss)]
fn compute_block_hash(data: &[u8], block_size: usize) -> (u32, u64) {
    let mut combined: u32 = 0;
    let mut blocks: u64 = 0;
    for chunk in data.chunks(block_size.max(1)) {
        let block_crc = compute_crc32(chunk);
        combined = combined.wrapping_add(block_crc);
        blocks += 1;
    }
    (combined, blocks)
}

/// Engine for computing and verifying proxy fingerprints.
pub struct FingerprintEngine {
    /// Default algorithm.
    algorithm: FingerprintAlgorithm,
    /// Block size for block-based hashing.
    block_size: usize,
    /// Cache of computed fingerprints.
    cache: HashMap<String, Fingerprint>,
}

impl FingerprintEngine {
    /// Create a new fingerprint engine.
    pub fn new(algorithm: FingerprintAlgorithm) -> Self {
        Self {
            algorithm,
            block_size: 4096,
            cache: HashMap::new(),
        }
    }

    /// Set the block size for block-based hashing.
    pub fn with_block_size(mut self, size: usize) -> Self {
        self.block_size = size;
        self
    }

    /// Compute a fingerprint for the given data.
    #[allow(clippy::cast_precision_loss)]
    pub fn compute(&self, data: &[u8]) -> Fingerprint {
        let file_size = data.len() as u64;
        match self.algorithm {
            FingerprintAlgorithm::Crc32 => {
                let crc = compute_crc32(data);
                Fingerprint::new(self.algorithm, &format!("{crc:08x}"), file_size)
            }
            FingerprintAlgorithm::Adler32 => {
                let adler = compute_adler32(data);
                Fingerprint::new(self.algorithm, &format!("{adler:08x}"), file_size)
            }
            FingerprintAlgorithm::XorHash => {
                let xor = compute_xor_hash(data);
                Fingerprint::new(self.algorithm, &format!("{xor:08x}"), file_size)
            }
            FingerprintAlgorithm::BlockHash => {
                let (hash, blocks) = compute_block_hash(data, self.block_size);
                Fingerprint::new(self.algorithm, &format!("{hash:08x}"), file_size)
                    .with_blocks(blocks)
            }
        }
    }

    /// Compute and cache a fingerprint for a named proxy.
    pub fn compute_and_cache(&mut self, name: &str, data: &[u8]) -> Fingerprint {
        let fp = self.compute(data);
        self.cache.insert(name.to_string(), fp.clone());
        fp
    }

    /// Verify data against a stored fingerprint.
    pub fn verify(&self, data: &[u8], expected: &Fingerprint) -> VerifyResult {
        #[allow(clippy::cast_precision_loss)]
        let actual_size = data.len() as u64;
        if actual_size != expected.file_size {
            return VerifyResult::SizeChanged {
                expected: expected.file_size,
                actual: actual_size,
            };
        }
        let actual_fp = self.compute(data);
        if actual_fp.hash == expected.hash {
            VerifyResult::Match
        } else {
            VerifyResult::Mismatch {
                expected: expected.hash.clone(),
                actual: actual_fp.hash,
            }
        }
    }

    /// Look up a cached fingerprint by name.
    pub fn get_cached(&self, name: &str) -> Option<&Fingerprint> {
        self.cache.get(name)
    }

    /// Number of cached fingerprints.
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Clear the fingerprint cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DATA: &[u8] = b"Hello, proxy fingerprint test data for OxiMedia framework!";

    #[test]
    fn test_algorithm_name() {
        assert_eq!(FingerprintAlgorithm::Crc32.name(), "CRC-32");
        assert_eq!(FingerprintAlgorithm::Adler32.name(), "Adler-32");
        assert_eq!(FingerprintAlgorithm::XorHash.name(), "XOR Hash");
        assert_eq!(FingerprintAlgorithm::BlockHash.name(), "Block Hash");
    }

    #[test]
    fn test_crc32_deterministic() {
        let a = compute_crc32(TEST_DATA);
        let b = compute_crc32(TEST_DATA);
        assert_eq!(a, b);
    }

    #[test]
    fn test_adler32_deterministic() {
        let a = compute_adler32(TEST_DATA);
        let b = compute_adler32(TEST_DATA);
        assert_eq!(a, b);
    }

    #[test]
    fn test_xor_hash_deterministic() {
        let a = compute_xor_hash(TEST_DATA);
        let b = compute_xor_hash(TEST_DATA);
        assert_eq!(a, b);
    }

    #[test]
    fn test_crc32_different_data() {
        let a = compute_crc32(b"hello");
        let b = compute_crc32(b"world");
        assert_ne!(a, b);
    }

    #[test]
    fn test_compute_crc32_fingerprint() {
        let engine = FingerprintEngine::new(FingerprintAlgorithm::Crc32);
        let fp = engine.compute(TEST_DATA);
        assert_eq!(fp.algorithm, FingerprintAlgorithm::Crc32);
        assert_eq!(fp.file_size, TEST_DATA.len() as u64);
        assert!(!fp.hash.is_empty());
    }

    #[test]
    fn test_compute_adler32_fingerprint() {
        let engine = FingerprintEngine::new(FingerprintAlgorithm::Adler32);
        let fp = engine.compute(TEST_DATA);
        assert_eq!(fp.algorithm, FingerprintAlgorithm::Adler32);
    }

    #[test]
    fn test_compute_block_hash_fingerprint() {
        let engine = FingerprintEngine::new(FingerprintAlgorithm::BlockHash).with_block_size(16);
        let fp = engine.compute(TEST_DATA);
        assert_eq!(fp.algorithm, FingerprintAlgorithm::BlockHash);
        assert!(fp.blocks_processed > 0);
    }

    #[test]
    fn test_verify_match() {
        let engine = FingerprintEngine::new(FingerprintAlgorithm::Crc32);
        let fp = engine.compute(TEST_DATA);
        let result = engine.verify(TEST_DATA, &fp);
        assert!(result.is_ok());
        assert_eq!(result, VerifyResult::Match);
    }

    #[test]
    fn test_verify_mismatch() {
        let engine = FingerprintEngine::new(FingerprintAlgorithm::Crc32);
        let fp = engine.compute(TEST_DATA);
        let _tampered = b"Tampered data that is different from the original proxy data!";
        // Make tampered same length as TEST_DATA for size match
        let mut tampered_same_size = TEST_DATA.to_vec();
        tampered_same_size[0] = b'X';
        let result = engine.verify(&tampered_same_size, &fp);
        assert!(!result.is_ok());
        assert!(matches!(result, VerifyResult::Mismatch { .. }));
    }

    #[test]
    fn test_verify_size_changed() {
        let engine = FingerprintEngine::new(FingerprintAlgorithm::Crc32);
        let fp = engine.compute(TEST_DATA);
        let shorter = &TEST_DATA[..10];
        let result = engine.verify(shorter, &fp);
        assert!(matches!(result, VerifyResult::SizeChanged { .. }));
    }

    #[test]
    fn test_cache_operations() {
        let mut engine = FingerprintEngine::new(FingerprintAlgorithm::Crc32);
        assert_eq!(engine.cache_size(), 0);
        engine.compute_and_cache("proxy_a.mp4", TEST_DATA);
        assert_eq!(engine.cache_size(), 1);
        assert!(engine.get_cached("proxy_a.mp4").is_some());
        assert!(engine.get_cached("nonexistent").is_none());
        engine.clear_cache();
        assert_eq!(engine.cache_size(), 0);
    }

    #[test]
    fn test_fingerprint_display() {
        let fp = Fingerprint::new(FingerprintAlgorithm::Crc32, "abcd1234", 100);
        let display = format!("{fp}");
        assert_eq!(display, "CRC-32:abcd1234");
    }

    #[test]
    fn test_empty_data() {
        let engine = FingerprintEngine::new(FingerprintAlgorithm::Crc32);
        let fp = engine.compute(b"");
        assert_eq!(fp.file_size, 0);
        // CRC32 of empty data should be deterministic
        let fp2 = engine.compute(b"");
        assert_eq!(fp.hash, fp2.hash);
    }
}
