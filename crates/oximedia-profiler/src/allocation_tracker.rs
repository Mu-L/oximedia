//! Memory allocation type tracking and peak-bytes analysis.
#![allow(dead_code)]

/// Category of a memory allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationType {
    /// Stack-allocated memory (e.g., local variables, function frames).
    Stack,
    /// Heap-allocated memory (e.g., `Box`, `Vec`, `malloc`).
    Heap,
    /// Memory-mapped region (e.g., `mmap`, file-backed pages).
    Mmap,
}

impl AllocationType {
    /// Returns `true` if this allocation lives on the heap.
    pub fn is_heap(&self) -> bool {
        matches!(self, AllocationType::Heap)
    }

    /// Human-readable label for the allocation type.
    pub fn label(&self) -> &'static str {
        match self {
            AllocationType::Stack => "stack",
            AllocationType::Heap => "heap",
            AllocationType::Mmap => "mmap",
        }
    }
}

/// A single allocation record.
#[derive(Debug, Clone)]
pub struct AllocRecord {
    /// Allocation type.
    pub alloc_type: AllocationType,
    /// Size in bytes.
    pub size_bytes: usize,
    /// Call-site tag (e.g., module::function name).
    pub tag: String,
    /// Monotonic sequence number assigned at record time.
    pub seq: u64,
}

/// Threshold above which an allocation is considered "large".
const LARGE_ALLOC_THRESHOLD: usize = 1 << 20; // 1 MiB

impl AllocRecord {
    /// Create a new allocation record.
    pub fn new(
        alloc_type: AllocationType,
        size_bytes: usize,
        tag: impl Into<String>,
        seq: u64,
    ) -> Self {
        Self {
            alloc_type,
            size_bytes,
            tag: tag.into(),
            seq,
        }
    }

    /// Returns `true` if the allocation is ≥ 1 MiB.
    pub fn is_large(&self) -> bool {
        self.size_bytes >= LARGE_ALLOC_THRESHOLD
    }
}

/// Tracker that accumulates allocation records and computes aggregate stats.
#[derive(Debug, Default)]
pub struct AllocationTracker {
    records: Vec<AllocRecord>,
    seq_counter: u64,
    peak_bytes: usize,
    current_bytes: usize,
}

impl AllocationTracker {
    /// Create a new, empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a new allocation.
    pub fn record(
        &mut self,
        alloc_type: AllocationType,
        size_bytes: usize,
        tag: impl Into<String>,
    ) {
        let seq = self.seq_counter;
        self.seq_counter += 1;
        self.current_bytes += size_bytes;
        if self.current_bytes > self.peak_bytes {
            self.peak_bytes = self.current_bytes;
        }
        self.records
            .push(AllocRecord::new(alloc_type, size_bytes, tag, seq));
    }

    /// Simulate a deallocation (reduces current live byte count).
    /// Does not remove the record — it remains in history.
    pub fn free(&mut self, size_bytes: usize) {
        self.current_bytes = self.current_bytes.saturating_sub(size_bytes);
    }

    /// Sum of all recorded allocation sizes in bytes.
    pub fn total_bytes(&self) -> usize {
        self.records.iter().map(|r| r.size_bytes).sum()
    }

    /// Peak simultaneous live bytes seen during this session.
    pub fn peak_bytes(&self) -> usize {
        self.peak_bytes
    }

    /// Current live byte total (after any `free` calls).
    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }

    /// Number of recorded allocations.
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// All records.
    pub fn records(&self) -> &[AllocRecord] {
        &self.records
    }

    /// Records filtered by allocation type.
    pub fn records_by_type(&self, alloc_type: AllocationType) -> Vec<&AllocRecord> {
        self.records
            .iter()
            .filter(|r| r.alloc_type == alloc_type)
            .collect()
    }

    /// Records that exceed the large-allocation threshold.
    pub fn large_allocations(&self) -> Vec<&AllocRecord> {
        self.records.iter().filter(|r| r.is_large()).collect()
    }

    /// Bytes allocated broken down by type: (stack, heap, mmap).
    pub fn bytes_by_type(&self) -> (usize, usize, usize) {
        let stack: usize = self
            .records_by_type(AllocationType::Stack)
            .iter()
            .map(|r| r.size_bytes)
            .sum();
        let heap: usize = self
            .records_by_type(AllocationType::Heap)
            .iter()
            .map(|r| r.size_bytes)
            .sum();
        let mmap: usize = self
            .records_by_type(AllocationType::Mmap)
            .iter()
            .map(|r| r.size_bytes)
            .sum();
        (stack, heap, mmap)
    }

    /// Clear all records and reset counters.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocation_type_is_heap() {
        assert!(AllocationType::Heap.is_heap());
        assert!(!AllocationType::Stack.is_heap());
        assert!(!AllocationType::Mmap.is_heap());
    }

    #[test]
    fn test_allocation_type_labels() {
        assert_eq!(AllocationType::Stack.label(), "stack");
        assert_eq!(AllocationType::Heap.label(), "heap");
        assert_eq!(AllocationType::Mmap.label(), "mmap");
    }

    #[test]
    fn test_alloc_record_is_large_false() {
        let r = AllocRecord::new(AllocationType::Heap, 512, "fn_a", 0);
        assert!(!r.is_large());
    }

    #[test]
    fn test_alloc_record_is_large_true() {
        let r = AllocRecord::new(AllocationType::Heap, 2 * 1024 * 1024, "fn_b", 1);
        assert!(r.is_large());
    }

    #[test]
    fn test_alloc_record_is_large_boundary() {
        let r = AllocRecord::new(AllocationType::Heap, LARGE_ALLOC_THRESHOLD, "fn_c", 2);
        assert!(r.is_large());
    }

    #[test]
    fn test_tracker_total_bytes() {
        let mut t = AllocationTracker::new();
        t.record(AllocationType::Heap, 100, "a");
        t.record(AllocationType::Stack, 50, "b");
        assert_eq!(t.total_bytes(), 150);
    }

    #[test]
    fn test_tracker_peak_bytes() {
        let mut t = AllocationTracker::new();
        t.record(AllocationType::Heap, 1000, "alloc1");
        t.record(AllocationType::Heap, 500, "alloc2");
        t.free(1000);
        t.record(AllocationType::Heap, 100, "alloc3");
        // Peak was 1500 before the free.
        assert_eq!(t.peak_bytes(), 1500);
    }

    #[test]
    fn test_tracker_current_bytes_after_free() {
        let mut t = AllocationTracker::new();
        t.record(AllocationType::Heap, 200, "x");
        t.free(100);
        assert_eq!(t.current_bytes(), 100);
    }

    #[test]
    fn test_tracker_free_saturates_at_zero() {
        let mut t = AllocationTracker::new();
        t.record(AllocationType::Heap, 10, "y");
        t.free(9999);
        assert_eq!(t.current_bytes(), 0);
    }

    #[test]
    fn test_tracker_record_count() {
        let mut t = AllocationTracker::new();
        for i in 0..5 {
            t.record(AllocationType::Heap, i * 10, "item");
        }
        assert_eq!(t.record_count(), 5);
    }

    #[test]
    fn test_tracker_records_by_type() {
        let mut t = AllocationTracker::new();
        t.record(AllocationType::Heap, 100, "h1");
        t.record(AllocationType::Stack, 50, "s1");
        t.record(AllocationType::Heap, 200, "h2");
        let heap_records = t.records_by_type(AllocationType::Heap);
        assert_eq!(heap_records.len(), 2);
    }

    #[test]
    fn test_tracker_large_allocations() {
        let mut t = AllocationTracker::new();
        t.record(AllocationType::Heap, 512, "small");
        t.record(AllocationType::Heap, 2 * 1024 * 1024, "big");
        let large = t.large_allocations();
        assert_eq!(large.len(), 1);
        assert_eq!(large[0].tag, "big");
    }

    #[test]
    fn test_tracker_bytes_by_type() {
        let mut t = AllocationTracker::new();
        t.record(AllocationType::Stack, 100, "s");
        t.record(AllocationType::Heap, 200, "h");
        t.record(AllocationType::Mmap, 400, "m");
        let (stack, heap, mmap) = t.bytes_by_type();
        assert_eq!(stack, 100);
        assert_eq!(heap, 200);
        assert_eq!(mmap, 400);
    }

    #[test]
    fn test_tracker_reset() {
        let mut t = AllocationTracker::new();
        t.record(AllocationType::Heap, 1000, "x");
        t.reset();
        assert_eq!(t.record_count(), 0);
        assert_eq!(t.total_bytes(), 0);
        assert_eq!(t.peak_bytes(), 0);
    }

    #[test]
    fn test_seq_monotonic() {
        let mut t = AllocationTracker::new();
        t.record(AllocationType::Heap, 1, "a");
        t.record(AllocationType::Heap, 2, "b");
        let seqs: Vec<u64> = t.records().iter().map(|r| r.seq).collect();
        assert_eq!(seqs, vec![0, 1]);
    }
}
