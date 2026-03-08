//! Memory-mapped I/O simulation.
//!
//! Provides in-process simulation of memory-mapped file I/O, including
//! region-based slicing, typed reads, and page-aligned buffer allocation.

#![allow(dead_code)]

// ──────────────────────────────────────────────────────────────────────────────
// MmapRegion
// ──────────────────────────────────────────────────────────────────────────────

/// A simulated memory-mapped region backed by a `Vec<u8>`.
#[derive(Debug, Clone)]
pub struct MmapRegion {
    /// Raw bytes of this region.
    pub data: Vec<u8>,
    /// Byte offset of this region within the file.
    pub offset: u64,
    /// Number of bytes in this region.
    pub length: u64,
}

impl MmapRegion {
    /// Create a new region.
    #[must_use]
    pub fn new(data: Vec<u8>, offset: u64) -> Self {
        let length = data.len() as u64;
        Self {
            data,
            offset,
            length,
        }
    }

    /// Return a slice of the region's data starting at `start` with `len` bytes.
    ///
    /// Returns `None` if the range lies outside the region.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn slice(&self, start: u64, len: usize) -> Option<&[u8]> {
        let end = start.checked_add(len as u64)?;
        if end > self.length {
            return None;
        }
        let s = start as usize;
        let e = end as usize;
        Some(&self.data[s..e])
    }

    /// Read a little-endian `u32` from `offset` within this region.
    ///
    /// Returns `None` if fewer than 4 bytes remain.
    #[must_use]
    pub fn read_u32_le(&self, offset: u64) -> Option<u32> {
        let bytes = self.slice(offset, 4)?;
        Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Read a little-endian `u64` from `offset` within this region.
    ///
    /// Returns `None` if fewer than 8 bytes remain.
    #[must_use]
    pub fn read_u64_le(&self, offset: u64) -> Option<u64> {
        let bytes = self.slice(offset, 8)?;
        Some(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// MmapFile
// ──────────────────────────────────────────────────────────────────────────────

/// A simulated memory-mapped file composed of multiple [`MmapRegion`]s.
#[derive(Debug, Default)]
pub struct MmapFile {
    /// Human-readable path (not used for actual file access in simulation).
    pub path: String,
    /// Mapped regions, in the order they were added.
    pub regions: Vec<MmapRegion>,
    /// Running total of all mapped bytes.
    pub total_size: u64,
}

impl MmapFile {
    /// Create a new (empty) simulated mapped file.
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            regions: Vec::new(),
            total_size: 0,
        }
    }

    /// Add a new region containing `data` at file `offset`.
    ///
    /// Returns the index of the newly added region.
    pub fn map_region(&mut self, data: Vec<u8>, offset: u64) -> usize {
        let region = MmapRegion::new(data, offset);
        self.total_size += region.length;
        self.regions.push(region);
        self.regions.len() - 1
    }

    /// Retrieve a reference to the region at `idx`, or `None` if out of bounds.
    #[must_use]
    pub fn get_region(&self, idx: usize) -> Option<&MmapRegion> {
        self.regions.get(idx)
    }

    /// Return the total number of mapped bytes across all regions.
    #[must_use]
    pub fn total_mapped_bytes(&self) -> u64 {
        self.total_size
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// PageAlignedBuffer
// ──────────────────────────────────────────────────────────────────────────────

/// A buffer whose logical size is rounded up to a multiple of `page_size`.
#[derive(Debug)]
pub struct PageAlignedBuffer {
    /// Underlying data; length is always a multiple of `page_size`.
    pub data: Vec<u8>,
    /// Page size in bytes.
    pub page_size: usize,
}

impl PageAlignedBuffer {
    /// Allocate a `PageAlignedBuffer` of at least `size` bytes, rounded up
    /// to the next page boundary.  The default page size is 4096 bytes.
    #[must_use]
    pub fn new(size: usize) -> Self {
        const DEFAULT_PAGE_SIZE: usize = 4096;
        Self::with_page_size(size, DEFAULT_PAGE_SIZE)
    }

    /// Allocate with an explicit `page_size`.
    ///
    /// # Panics
    ///
    /// Panics if `page_size` is zero.
    #[must_use]
    pub fn with_page_size(size: usize, page_size: usize) -> Self {
        assert!(page_size > 0, "page_size must be non-zero");
        let pages = size.div_ceil(page_size).max(1);
        let aligned = pages * page_size;
        Self {
            data: vec![0u8; aligned],
            page_size,
        }
    }

    /// Return the aligned length of the buffer (always a multiple of `page_size`).
    #[must_use]
    pub fn aligned_len(&self) -> usize {
        self.data.len()
    }

    /// Return the number of pages occupied.
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.data.len() / self.page_size
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // MmapRegion ──────────────────────────────────────────────────────────────

    #[test]
    fn test_region_new_sets_length() {
        let r = MmapRegion::new(vec![1, 2, 3, 4], 0);
        assert_eq!(r.length, 4);
        assert_eq!(r.offset, 0);
    }

    #[test]
    fn test_region_slice_full() {
        let r = MmapRegion::new(vec![10, 20, 30], 0);
        assert_eq!(r.slice(0, 3), Some([10u8, 20, 30].as_slice()));
    }

    #[test]
    fn test_region_slice_partial() {
        let r = MmapRegion::new(vec![1, 2, 3, 4, 5], 0);
        assert_eq!(r.slice(1, 3), Some([2u8, 3, 4].as_slice()));
    }

    #[test]
    fn test_region_slice_out_of_bounds() {
        let r = MmapRegion::new(vec![0u8; 4], 0);
        assert!(r.slice(3, 2).is_none());
    }

    #[test]
    fn test_region_slice_empty() {
        let r = MmapRegion::new(vec![9u8; 8], 100);
        assert_eq!(r.slice(0, 0), Some([].as_slice()));
    }

    #[test]
    fn test_region_read_u32_le() {
        // 0x01020304 in little-endian = bytes [04, 03, 02, 01]
        let r = MmapRegion::new(vec![0x04, 0x03, 0x02, 0x01], 0);
        assert_eq!(r.read_u32_le(0), Some(0x0102_0304));
    }

    #[test]
    fn test_region_read_u32_le_not_enough_bytes() {
        let r = MmapRegion::new(vec![0, 1, 2], 0);
        assert!(r.read_u32_le(0).is_none());
    }

    #[test]
    fn test_region_read_u64_le() {
        let bytes: Vec<u8> = (0u8..8).collect();
        let r = MmapRegion::new(bytes, 0);
        let expected = u64::from_le_bytes([0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(r.read_u64_le(0), Some(expected));
    }

    #[test]
    fn test_region_read_u64_le_not_enough_bytes() {
        let r = MmapRegion::new(vec![0u8; 7], 0);
        assert!(r.read_u64_le(0).is_none());
    }

    // MmapFile ────────────────────────────────────────────────────────────────

    #[test]
    fn test_mmap_file_map_region_returns_index() {
        let mut f = MmapFile::new("test.bin");
        let idx = f.map_region(vec![1, 2, 3], 0);
        assert_eq!(idx, 0);
        let idx2 = f.map_region(vec![4, 5], 3);
        assert_eq!(idx2, 1);
    }

    #[test]
    fn test_mmap_file_total_mapped_bytes() {
        let mut f = MmapFile::new("x");
        f.map_region(vec![0u8; 100], 0);
        f.map_region(vec![0u8; 200], 100);
        assert_eq!(f.total_mapped_bytes(), 300);
    }

    #[test]
    fn test_mmap_file_get_region_valid() {
        let mut f = MmapFile::new("x");
        f.map_region(vec![42u8; 4], 0);
        let r = f.get_region(0).expect("region 0 must exist");
        assert_eq!(r.data, vec![42u8; 4]);
    }

    #[test]
    fn test_mmap_file_get_region_out_of_bounds() {
        let f = MmapFile::new("x");
        assert!(f.get_region(0).is_none());
    }

    // PageAlignedBuffer ───────────────────────────────────────────────────────

    #[test]
    fn test_page_aligned_buffer_exact_page() {
        let buf = PageAlignedBuffer::new(4096);
        assert_eq!(buf.aligned_len(), 4096);
        assert_eq!(buf.page_count(), 1);
    }

    #[test]
    fn test_page_aligned_buffer_rounds_up() {
        let buf = PageAlignedBuffer::new(1);
        assert_eq!(buf.aligned_len(), 4096);
        assert_eq!(buf.page_count(), 1);
    }

    #[test]
    fn test_page_aligned_buffer_multiple_pages() {
        let buf = PageAlignedBuffer::new(8193);
        assert_eq!(buf.aligned_len(), 4096 * 3);
        assert_eq!(buf.page_count(), 3);
    }

    #[test]
    fn test_page_aligned_buffer_custom_page_size() {
        let buf = PageAlignedBuffer::with_page_size(100, 64);
        assert_eq!(buf.aligned_len(), 128);
        assert_eq!(buf.page_count(), 2);
    }

    #[test]
    fn test_page_aligned_buffer_zero_size() {
        // Zero size should still allocate one page.
        let buf = PageAlignedBuffer::new(0);
        assert_eq!(buf.aligned_len(), 4096);
    }
}
