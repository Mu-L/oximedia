//! Hash algorithm registry for media forensics.
//!
//! Tracks multiple hash algorithms, stores computed media hashes, and
//! detects potential collisions or discrepancies between hash values.

#![allow(dead_code)]

use std::collections::HashMap;

// ── HashAlgorithm ─────────────────────────────────────────────────────────────

/// Supported hash algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HashAlgorithm {
    /// MD5 (legacy; 128-bit)
    Md5,
    /// SHA-256
    Sha256,
    /// SHA-512
    Sha512,
    /// BLAKE3
    Blake3,
    /// Perceptual hash (64-bit)
    Perceptual,
}

impl HashAlgorithm {
    /// Returns the output size in bits.
    #[must_use]
    pub fn output_bits(&self) -> usize {
        match self {
            Self::Md5 => 128,
            Self::Sha256 => 256,
            Self::Sha512 => 512,
            Self::Blake3 => 256,
            Self::Perceptual => 64,
        }
    }

    /// Returns the name of the algorithm as a string.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Md5 => "MD5",
            Self::Sha256 => "SHA-256",
            Self::Sha512 => "SHA-512",
            Self::Blake3 => "BLAKE3",
            Self::Perceptual => "Perceptual",
        }
    }

    /// Returns `true` if this algorithm is considered cryptographically strong.
    #[must_use]
    pub fn is_cryptographic(&self) -> bool {
        matches!(self, Self::Sha256 | Self::Sha512 | Self::Blake3)
    }
}

// ── MediaHash ─────────────────────────────────────────────────────────────────

/// A hash value computed for a piece of media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaHash {
    /// Algorithm used to produce this hash.
    pub algorithm: HashAlgorithm,
    /// Hex-encoded hash string.
    pub hex_value: String,
}

impl MediaHash {
    /// Create a new `MediaHash`.
    #[must_use]
    pub fn new(algorithm: HashAlgorithm, hex_value: impl Into<String>) -> Self {
        Self {
            algorithm,
            hex_value: hex_value.into(),
        }
    }

    /// Returns `true` if this hash matches another (same algorithm and value).
    #[must_use]
    pub fn matches(&self, other: &Self) -> bool {
        self.algorithm == other.algorithm && self.hex_value == other.hex_value
    }

    /// Returns the expected hex string length for this hash's algorithm.
    #[must_use]
    pub fn expected_hex_len(&self) -> usize {
        self.algorithm.output_bits() / 4
    }

    /// Returns `true` if the hex value length matches expectations.
    #[must_use]
    pub fn is_valid_length(&self) -> bool {
        self.hex_value.len() == self.expected_hex_len()
    }
}

// ── HashRegistry ─────────────────────────────────────────────────────────────

/// Registry that stores hashes for media assets and detects collisions.
#[derive(Debug, Default)]
pub struct HashRegistry {
    /// Map: hex_value -> list of asset IDs that produced that hash.
    registry: HashMap<String, Vec<String>>,
    /// Total insertions.
    total_inserts: usize,
}

impl HashRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a hash for the given `asset_id`.
    pub fn insert(&mut self, asset_id: impl Into<String>, hash: &MediaHash) {
        self.registry
            .entry(hash.hex_value.clone())
            .or_default()
            .push(asset_id.into());
        self.total_inserts += 1;
    }

    /// Look up all asset IDs that produced the given hash hex value.
    #[must_use]
    pub fn lookup(&self, hex_value: &str) -> &[String] {
        self.registry
            .get(hex_value)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Returns the number of hash values that map to more than one asset
    /// (potential collision or genuine duplicate).
    #[must_use]
    pub fn collision_count(&self) -> usize {
        self.registry.values().filter(|ids| ids.len() > 1).count()
    }

    /// Returns the total number of hash insertions.
    #[must_use]
    pub fn total_inserts(&self) -> usize {
        self.total_inserts
    }

    /// Returns the number of unique hash values stored.
    #[must_use]
    pub fn unique_hash_count(&self) -> usize {
        self.registry.len()
    }

    /// Returns all asset IDs involved in any collision.
    #[must_use]
    pub fn colliding_assets(&self) -> Vec<&str> {
        self.registry
            .values()
            .filter(|ids| ids.len() > 1)
            .flat_map(|ids| ids.iter().map(String::as_str))
            .collect()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sha256_hash(hex: &str) -> MediaHash {
        MediaHash::new(HashAlgorithm::Sha256, hex)
    }

    #[test]
    fn test_algorithm_output_bits_md5() {
        assert_eq!(HashAlgorithm::Md5.output_bits(), 128);
    }

    #[test]
    fn test_algorithm_output_bits_sha256() {
        assert_eq!(HashAlgorithm::Sha256.output_bits(), 256);
    }

    #[test]
    fn test_algorithm_output_bits_blake3() {
        assert_eq!(HashAlgorithm::Blake3.output_bits(), 256);
    }

    #[test]
    fn test_algorithm_output_bits_perceptual() {
        assert_eq!(HashAlgorithm::Perceptual.output_bits(), 64);
    }

    #[test]
    fn test_algorithm_is_cryptographic() {
        assert!(HashAlgorithm::Sha256.is_cryptographic());
        assert!(HashAlgorithm::Blake3.is_cryptographic());
        assert!(!HashAlgorithm::Md5.is_cryptographic());
        assert!(!HashAlgorithm::Perceptual.is_cryptographic());
    }

    #[test]
    fn test_media_hash_matches_same() {
        let h1 = sha256_hash("abcdef");
        let h2 = sha256_hash("abcdef");
        assert!(h1.matches(&h2));
    }

    #[test]
    fn test_media_hash_no_match_different_value() {
        let h1 = sha256_hash("aaaa");
        let h2 = sha256_hash("bbbb");
        assert!(!h1.matches(&h2));
    }

    #[test]
    fn test_media_hash_no_match_different_algorithm() {
        let h1 = MediaHash::new(HashAlgorithm::Sha256, "abcd");
        let h2 = MediaHash::new(HashAlgorithm::Blake3, "abcd");
        assert!(!h1.matches(&h2));
    }

    #[test]
    fn test_registry_insert_and_lookup() {
        let mut reg = HashRegistry::new();
        let h = sha256_hash("deadbeef");
        reg.insert("asset_1", &h);
        let found = reg.lookup("deadbeef");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], "asset_1");
    }

    #[test]
    fn test_registry_collision_count_zero() {
        let mut reg = HashRegistry::new();
        reg.insert("a", &sha256_hash("hash1"));
        reg.insert("b", &sha256_hash("hash2"));
        assert_eq!(reg.collision_count(), 0);
    }

    #[test]
    fn test_registry_collision_count_nonzero() {
        let mut reg = HashRegistry::new();
        reg.insert("a", &sha256_hash("samehash"));
        reg.insert("b", &sha256_hash("samehash"));
        assert_eq!(reg.collision_count(), 1);
    }

    #[test]
    fn test_registry_total_inserts() {
        let mut reg = HashRegistry::new();
        reg.insert("a", &sha256_hash("h1"));
        reg.insert("b", &sha256_hash("h2"));
        reg.insert("c", &sha256_hash("h1")); // collision
        assert_eq!(reg.total_inserts(), 3);
    }

    #[test]
    fn test_registry_unique_hash_count() {
        let mut reg = HashRegistry::new();
        reg.insert("a", &sha256_hash("h1"));
        reg.insert("b", &sha256_hash("h1")); // same hash
        reg.insert("c", &sha256_hash("h2"));
        assert_eq!(reg.unique_hash_count(), 2);
    }

    #[test]
    fn test_registry_colliding_assets() {
        let mut reg = HashRegistry::new();
        reg.insert("asset_x", &sha256_hash("collision_hash"));
        reg.insert("asset_y", &sha256_hash("collision_hash"));
        let colliders = reg.colliding_assets();
        assert!(colliders.contains(&"asset_x"));
        assert!(colliders.contains(&"asset_y"));
    }

    #[test]
    fn test_algorithm_name() {
        assert_eq!(HashAlgorithm::Sha256.name(), "SHA-256");
        assert_eq!(HashAlgorithm::Md5.name(), "MD5");
        assert_eq!(HashAlgorithm::Blake3.name(), "BLAKE3");
    }
}
