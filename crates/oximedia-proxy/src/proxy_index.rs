//! In-memory index mapping original media paths to proxy entries.
//!
//! Provides `ProxyEntry` and `ProxyIndex` for fast lookup of proxies
//! by their originating source file path.

#![allow(dead_code)]

use std::collections::HashMap;

/// A record in the proxy index describing one proxy asset.
#[derive(Debug, Clone, PartialEq)]
pub struct ProxyEntry {
    /// Absolute path to the original high-resolution media file.
    pub original_path: String,
    /// Absolute path to the proxy media file.
    pub proxy_path: String,
    /// Width of the proxy in pixels.
    pub width: u32,
    /// Height of the proxy in pixels.
    pub height: u32,
    /// Bitrate of the proxy in kbps.
    pub bitrate_kbps: u32,
    /// Optional codec identifier (e.g. "h264").
    pub codec: Option<String>,
}

impl ProxyEntry {
    /// Create a new proxy entry.
    pub fn new(
        original_path: impl Into<String>,
        proxy_path: impl Into<String>,
        width: u32,
        height: u32,
        bitrate_kbps: u32,
    ) -> Self {
        Self {
            original_path: original_path.into(),
            proxy_path: proxy_path.into(),
            width,
            height,
            bitrate_kbps,
            codec: None,
        }
    }

    /// Return `true` when required fields are non-empty and dimensions are > 0.
    pub fn is_valid(&self) -> bool {
        !self.original_path.is_empty()
            && !self.proxy_path.is_empty()
            && self.width > 0
            && self.height > 0
            && self.bitrate_kbps > 0
    }

    /// Return a display label combining resolution and bitrate.
    pub fn display_label(&self) -> String {
        format!("{}x{}@{}kbps", self.width, self.height, self.bitrate_kbps)
    }

    /// Return total pixel count (width × height).
    pub fn pixel_count(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

/// An in-memory index of proxy entries keyed by original file path.
#[derive(Debug, Default)]
pub struct ProxyIndex {
    // Maps original_path → Vec<ProxyEntry> (multiple qualities possible).
    map: HashMap<String, Vec<ProxyEntry>>,
}

impl ProxyIndex {
    /// Create an empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a `ProxyEntry`.  Entries with the same `original_path` are accumulated.
    pub fn insert(&mut self, entry: ProxyEntry) {
        self.map
            .entry(entry.original_path.clone())
            .or_default()
            .push(entry);
    }

    /// Find all proxy entries for a given original path.
    pub fn find_by_original(&self, original_path: &str) -> &[ProxyEntry] {
        self.map
            .get(original_path)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Remove all entries for a given original path.  Returns the removed entries.
    pub fn remove(&mut self, original_path: &str) -> Vec<ProxyEntry> {
        self.map.remove(original_path).unwrap_or_default()
    }

    /// Return the total number of proxy entries across all originals.
    pub fn count(&self) -> usize {
        self.map.values().map(Vec::len).sum()
    }

    /// Return the number of unique originals in the index.
    pub fn original_count(&self) -> usize {
        self.map.len()
    }

    /// Return `true` if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Return all entries as a flat iterator.
    pub fn all_entries(&self) -> impl Iterator<Item = &ProxyEntry> {
        self.map.values().flat_map(|v| v.iter())
    }

    /// Return `true` if any proxy entry exists for the given original path.
    pub fn contains(&self, original_path: &str) -> bool {
        self.map.contains_key(original_path)
    }

    /// Find the entry with the highest bitrate for a given original path.
    pub fn best_quality(&self, original_path: &str) -> Option<&ProxyEntry> {
        self.find_by_original(original_path)
            .iter()
            .max_by_key(|e| e.bitrate_kbps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(orig: &str, proxy: &str, w: u32, h: u32, br: u32) -> ProxyEntry {
        ProxyEntry::new(orig, proxy, w, h, br)
    }

    #[test]
    fn test_entry_is_valid() {
        let e = make_entry("/media/orig.mov", "/proxy/p.mp4", 640, 360, 500);
        assert!(e.is_valid());
    }

    #[test]
    fn test_entry_invalid_empty_path() {
        let e = make_entry("", "/proxy/p.mp4", 640, 360, 500);
        assert!(!e.is_valid());
    }

    #[test]
    fn test_entry_invalid_zero_dimension() {
        let e = make_entry("/media/orig.mov", "/proxy/p.mp4", 0, 360, 500);
        assert!(!e.is_valid());
    }

    #[test]
    fn test_entry_invalid_zero_bitrate() {
        let e = make_entry("/media/orig.mov", "/proxy/p.mp4", 640, 360, 0);
        assert!(!e.is_valid());
    }

    #[test]
    fn test_entry_display_label() {
        let e = make_entry("/orig.mov", "/p.mp4", 1280, 720, 2000);
        assert_eq!(e.display_label(), "1280x720@2000kbps");
    }

    #[test]
    fn test_entry_pixel_count() {
        let e = make_entry("/orig.mov", "/p.mp4", 1920, 1080, 8000);
        assert_eq!(e.pixel_count(), 1920 * 1080);
    }

    #[test]
    fn test_index_insert_and_count() {
        let mut idx = ProxyIndex::new();
        idx.insert(make_entry("/orig.mov", "/p1.mp4", 640, 360, 500));
        idx.insert(make_entry("/orig.mov", "/p2.mp4", 1280, 720, 2000));
        assert_eq!(idx.count(), 2);
        assert_eq!(idx.original_count(), 1);
    }

    #[test]
    fn test_index_find_by_original() {
        let mut idx = ProxyIndex::new();
        idx.insert(make_entry("/orig.mov", "/p.mp4", 640, 360, 500));
        let found = idx.find_by_original("/orig.mov");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].proxy_path, "/p.mp4");
    }

    #[test]
    fn test_index_find_by_original_not_found() {
        let idx = ProxyIndex::new();
        assert!(idx.find_by_original("/missing.mov").is_empty());
    }

    #[test]
    fn test_index_remove() {
        let mut idx = ProxyIndex::new();
        idx.insert(make_entry("/orig.mov", "/p.mp4", 640, 360, 500));
        let removed = idx.remove("/orig.mov");
        assert_eq!(removed.len(), 1);
        assert_eq!(idx.count(), 0);
    }

    #[test]
    fn test_index_contains() {
        let mut idx = ProxyIndex::new();
        idx.insert(make_entry("/orig.mov", "/p.mp4", 640, 360, 500));
        assert!(idx.contains("/orig.mov"));
        assert!(!idx.contains("/other.mov"));
    }

    #[test]
    fn test_index_best_quality() {
        let mut idx = ProxyIndex::new();
        idx.insert(make_entry("/orig.mov", "/p_draft.mp4", 640, 360, 500));
        idx.insert(make_entry("/orig.mov", "/p_delivery.mp4", 1920, 1080, 8000));
        let best = idx
            .best_quality("/orig.mov")
            .expect("should succeed in test");
        assert_eq!(best.proxy_path, "/p_delivery.mp4");
    }

    #[test]
    fn test_index_is_empty() {
        let idx = ProxyIndex::new();
        assert!(idx.is_empty());
    }
}
