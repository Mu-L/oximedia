#![allow(dead_code)]
//! Archive-level deduplication
//!
//! Provides content-addressable deduplication for archived media assets.
//! Uses rolling hashes and fingerprints to detect duplicate content at the
//! file level and chunk level, reducing storage costs for large archives.

use std::collections::HashMap;
use std::fmt;

/// A content fingerprint used for dedup comparison.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentFingerprint {
    /// Hex-encoded digest string.
    digest: String,
    /// Size in bytes of the content that was fingerprinted.
    size_bytes: u64,
}

impl ContentFingerprint {
    /// Create a new content fingerprint.
    pub fn new(digest: impl Into<String>, size_bytes: u64) -> Self {
        Self {
            digest: digest.into(),
            size_bytes,
        }
    }

    /// Return the digest string.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Return the size in bytes.
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

impl fmt::Display for ContentFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.digest, self.size_bytes)
    }
}

/// Deduplication level granularity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DedupLevel {
    /// Whole-file deduplication.
    File,
    /// Fixed-size chunk deduplication.
    FixedChunk,
    /// Variable-size (content-defined) chunk deduplication.
    VariableChunk,
}

impl fmt::Display for DedupLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::File => "file",
            Self::FixedChunk => "fixed-chunk",
            Self::VariableChunk => "variable-chunk",
        };
        write!(f, "{s}")
    }
}

/// Configuration for dedup operations.
#[derive(Debug, Clone)]
pub struct DedupConfig {
    /// Dedup granularity level.
    pub level: DedupLevel,
    /// Fixed chunk size in bytes (used when level is FixedChunk).
    pub chunk_size: usize,
    /// Minimum chunk size for variable chunking.
    pub min_chunk: usize,
    /// Maximum chunk size for variable chunking.
    pub max_chunk: usize,
    /// Whether to keep a reference count.
    pub track_refcount: bool,
}

impl Default for DedupConfig {
    fn default() -> Self {
        Self {
            level: DedupLevel::File,
            chunk_size: 4 * 1024 * 1024, // 4 MiB
            min_chunk: 512 * 1024,       // 512 KiB
            max_chunk: 16 * 1024 * 1024, // 16 MiB
            track_refcount: true,
        }
    }
}

/// A record for a stored chunk or file entry.
#[derive(Debug, Clone)]
pub struct DedupEntry {
    /// The content fingerprint.
    pub fingerprint: ContentFingerprint,
    /// Path where the canonical copy is stored.
    pub canonical_path: String,
    /// Number of references to this entry.
    pub refcount: u32,
}

impl DedupEntry {
    /// Create a new dedup entry.
    pub fn new(fingerprint: ContentFingerprint, canonical_path: impl Into<String>) -> Self {
        Self {
            fingerprint,
            canonical_path: canonical_path.into(),
            refcount: 1,
        }
    }

    /// Increment the reference count.
    pub fn add_ref(&mut self) {
        self.refcount = self.refcount.saturating_add(1);
    }

    /// Decrement the reference count, returning the new count.
    pub fn release_ref(&mut self) -> u32 {
        self.refcount = self.refcount.saturating_sub(1);
        self.refcount
    }
}

/// Result of a dedup lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DedupResult {
    /// Content is new and was stored.
    Stored,
    /// Content was a duplicate; reference added.
    Duplicate,
    /// Content was skipped (error or policy).
    Skipped,
}

/// Simple rolling hash for content-defined chunking.
#[allow(clippy::cast_precision_loss)]
pub fn rolling_hash(data: &[u8], window: usize) -> Vec<u64> {
    if data.len() < window || window == 0 {
        return vec![];
    }
    let mut hashes = Vec::with_capacity(data.len() - window + 1);
    let mut h: u64 = 0;
    // Initial window
    for &b in &data[..window] {
        h = h.wrapping_mul(31).wrapping_add(u64::from(b));
    }
    hashes.push(h);
    // Rolling
    let base_pow = 31_u64.wrapping_pow(window as u32);
    for i in window..data.len() {
        h = h
            .wrapping_mul(31)
            .wrapping_add(u64::from(data[i]))
            .wrapping_sub(base_pow.wrapping_mul(u64::from(data[i - window])));
        hashes.push(h);
    }
    hashes
}

/// Compute a simple 64-bit fingerprint for a byte slice.
pub fn compute_fingerprint(data: &[u8]) -> ContentFingerprint {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    ContentFingerprint::new(format!("{h:016x}"), data.len() as u64)
}

/// In-memory dedup index for archive content.
#[derive(Debug)]
pub struct DedupIndex {
    /// Configuration.
    config: DedupConfig,
    /// Map from fingerprint digest to entry.
    entries: HashMap<String, DedupEntry>,
    /// Total bytes saved by dedup.
    bytes_saved: u64,
}

impl DedupIndex {
    /// Create a new dedup index with the given config.
    pub fn new(config: DedupConfig) -> Self {
        Self {
            config,
            entries: HashMap::new(),
            bytes_saved: 0,
        }
    }

    /// Create with default config.
    pub fn with_defaults() -> Self {
        Self::new(DedupConfig::default())
    }

    /// Get the current config.
    pub fn config(&self) -> &DedupConfig {
        &self.config
    }

    /// Return number of unique entries.
    pub fn unique_count(&self) -> usize {
        self.entries.len()
    }

    /// Return total bytes saved.
    pub fn bytes_saved(&self) -> u64 {
        self.bytes_saved
    }

    /// Ingest content: returns whether it was stored or a duplicate.
    pub fn ingest(&mut self, data: &[u8], path: &str) -> DedupResult {
        let fp = compute_fingerprint(data);
        let digest = fp.digest().to_string();
        if let Some(entry) = self.entries.get_mut(&digest) {
            entry.add_ref();
            self.bytes_saved += fp.size_bytes();
            DedupResult::Duplicate
        } else {
            self.entries.insert(digest, DedupEntry::new(fp, path));
            DedupResult::Stored
        }
    }

    /// Look up whether content already exists.
    pub fn contains(&self, data: &[u8]) -> bool {
        let fp = compute_fingerprint(data);
        self.entries.contains_key(fp.digest())
    }

    /// Release a reference; returns true if entry was removed.
    pub fn release(&mut self, data: &[u8]) -> bool {
        let fp = compute_fingerprint(data);
        let digest = fp.digest().to_string();
        if let Some(entry) = self.entries.get_mut(&digest) {
            if entry.release_ref() == 0 {
                self.entries.remove(&digest);
                return true;
            }
        }
        false
    }

    /// Get statistics about the index.
    pub fn stats(&self) -> DedupStats {
        let total_refs: u64 = self.entries.values().map(|e| u64::from(e.refcount)).sum();
        DedupStats {
            unique_entries: self.entries.len(),
            total_references: total_refs,
            bytes_saved: self.bytes_saved,
        }
    }
}

/// Statistics about dedup index.
#[derive(Debug, Clone)]
pub struct DedupStats {
    /// Number of unique entries.
    pub unique_entries: usize,
    /// Total number of references.
    pub total_references: u64,
    /// Bytes saved through dedup.
    pub bytes_saved: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_creation() {
        let fp = ContentFingerprint::new("abc123", 1024);
        assert_eq!(fp.digest(), "abc123");
        assert_eq!(fp.size_bytes(), 1024);
    }

    #[test]
    fn test_fingerprint_display() {
        let fp = ContentFingerprint::new("abc123", 1024);
        assert_eq!(fp.to_string(), "abc123:1024");
    }

    #[test]
    fn test_dedup_level_display() {
        assert_eq!(DedupLevel::File.to_string(), "file");
        assert_eq!(DedupLevel::FixedChunk.to_string(), "fixed-chunk");
        assert_eq!(DedupLevel::VariableChunk.to_string(), "variable-chunk");
    }

    #[test]
    fn test_default_config() {
        let cfg = DedupConfig::default();
        assert_eq!(cfg.level, DedupLevel::File);
        assert_eq!(cfg.chunk_size, 4 * 1024 * 1024);
        assert!(cfg.track_refcount);
    }

    #[test]
    fn test_dedup_entry_refcount() {
        let fp = ContentFingerprint::new("abc", 100);
        let mut entry = DedupEntry::new(fp, "/store/abc");
        assert_eq!(entry.refcount, 1);
        entry.add_ref();
        assert_eq!(entry.refcount, 2);
        entry.release_ref();
        assert_eq!(entry.refcount, 1);
        entry.release_ref();
        assert_eq!(entry.refcount, 0);
        // saturating: doesn't go below 0
        entry.release_ref();
        assert_eq!(entry.refcount, 0);
    }

    #[test]
    fn test_compute_fingerprint_deterministic() {
        let data = b"hello world archive";
        let fp1 = compute_fingerprint(data);
        let fp2 = compute_fingerprint(data);
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_compute_fingerprint_different_data() {
        let fp1 = compute_fingerprint(b"data_a");
        let fp2 = compute_fingerprint(b"data_b");
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_rolling_hash_basic() {
        let data = b"abcdefghij";
        let hashes = rolling_hash(data, 4);
        assert_eq!(hashes.len(), 7); // 10 - 4 + 1
    }

    #[test]
    fn test_rolling_hash_empty() {
        let hashes = rolling_hash(b"ab", 5);
        assert!(hashes.is_empty());
    }

    #[test]
    fn test_index_ingest_new() {
        let mut idx = DedupIndex::with_defaults();
        let result = idx.ingest(b"content_one", "/archive/a.mxf");
        assert_eq!(result, DedupResult::Stored);
        assert_eq!(idx.unique_count(), 1);
    }

    #[test]
    fn test_index_ingest_duplicate() {
        let mut idx = DedupIndex::with_defaults();
        idx.ingest(b"content_one", "/archive/a.mxf");
        let result = idx.ingest(b"content_one", "/archive/b.mxf");
        assert_eq!(result, DedupResult::Duplicate);
        assert_eq!(idx.unique_count(), 1);
        assert!(idx.bytes_saved() > 0);
    }

    #[test]
    fn test_index_contains() {
        let mut idx = DedupIndex::with_defaults();
        idx.ingest(b"content_one", "/archive/a.mxf");
        assert!(idx.contains(b"content_one"));
        assert!(!idx.contains(b"content_two"));
    }

    #[test]
    fn test_index_release() {
        let mut idx = DedupIndex::with_defaults();
        idx.ingest(b"content_one", "/archive/a.mxf");
        let removed = idx.release(b"content_one");
        assert!(removed);
        assert_eq!(idx.unique_count(), 0);
    }

    #[test]
    fn test_index_stats() {
        let mut idx = DedupIndex::with_defaults();
        idx.ingest(b"content_one", "/archive/a.mxf");
        idx.ingest(b"content_one", "/archive/b.mxf");
        idx.ingest(b"content_two", "/archive/c.mxf");
        let stats = idx.stats();
        assert_eq!(stats.unique_entries, 2);
        assert_eq!(stats.total_references, 3); // 2 refs for one, 1 for other
    }
}
