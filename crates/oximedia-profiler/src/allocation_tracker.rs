//! Memory allocation type tracking and peak-bytes analysis.
//!
//! # Atomic counter design
//!
//! The three scalar counters (`seq_counter`, `current_bytes`, `peak_bytes`)
//! are stored as `AtomicU64` values so that `record()` and `free()` can take
//! `&self` and be called from multiple threads without locking.
//!
//! The `records` `Vec` is kept behind a `Mutex` because pushing into a `Vec`
//! requires exclusive access.  Making the `Vec` lock-free is out of scope for
//! this implementation; only the hot counters are atomicised.
//!
//! # Usage note
//!
//! Call `record()` / `free()` concurrently from any number of threads.  Call
//! `records()` / `total_bytes()` / `bytes_by_type()` from any thread —
//! they acquire the mutex briefly.  `reset()` requires `&mut self` (exclusive
//! ownership) because it replaces the entire internal state.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

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
///
/// ## Concurrency
///
/// `record()` and `free()` are `&self` — they can be called from multiple
/// threads simultaneously.  The three scalar counters (`seq_counter`,
/// `current_bytes`, `peak_bytes`) use `AtomicU64` for wait-free updates.
/// The record list uses a `Mutex<Vec<…>>` for thread-safe push/read.
#[derive(Debug)]
pub struct AllocationTracker {
    records: Mutex<Vec<AllocRecord>>,
    /// Monotonically increasing sequence counter.
    seq_counter: AtomicU64,
    /// Live byte total (incremented on record, decremented on free).
    current_bytes: AtomicU64,
    /// Peak simultaneous live bytes seen since the last reset.
    peak_bytes: AtomicU64,
}

impl AllocationTracker {
    /// Create a new, empty tracker.
    pub fn new() -> Self {
        Self {
            records: Mutex::new(Vec::new()),
            seq_counter: AtomicU64::new(0),
            current_bytes: AtomicU64::new(0),
            peak_bytes: AtomicU64::new(0),
        }
    }

    /// Record a new allocation.
    ///
    /// Takes `&self` — can be called from multiple threads simultaneously.
    pub fn record(&self, alloc_type: AllocationType, size_bytes: usize, tag: impl Into<String>) {
        // Assign a unique sequence number.
        let seq = self.seq_counter.fetch_add(1, Ordering::Relaxed);

        // Add size to live byte total.
        let new_current = self
            .current_bytes
            .fetch_add(size_bytes as u64, Ordering::Relaxed)
            + size_bytes as u64;

        // CAS-max loop to update peak_bytes.
        let mut old_peak = self.peak_bytes.load(Ordering::Relaxed);
        loop {
            let new_peak = old_peak.max(new_current);
            match self.peak_bytes.compare_exchange_weak(
                old_peak,
                new_peak,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => old_peak = actual,
            }
        }

        // Push record into the protected Vec.
        let record = AllocRecord::new(alloc_type, size_bytes, tag, seq);
        if let Ok(mut guard) = self.records.lock() {
            guard.push(record);
        }
    }

    /// Simulate a deallocation (reduces current live byte count).
    ///
    /// Takes `&self` — can be called from multiple threads simultaneously.
    /// Does not remove the record — it remains in history.
    pub fn free(&self, size_bytes: usize) {
        // Saturating subtract via a fetch_update loop to avoid underflow.
        let _ = self
            .current_bytes
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.saturating_sub(size_bytes as u64))
            });
    }

    /// Sum of all recorded allocation sizes in bytes.
    pub fn total_bytes(&self) -> usize {
        let guard = self.records.lock().unwrap_or_else(|e| e.into_inner());
        guard.iter().map(|r| r.size_bytes).sum()
    }

    /// Peak simultaneous live bytes seen during this session.
    pub fn peak_bytes(&self) -> usize {
        self.peak_bytes.load(Ordering::Relaxed) as usize
    }

    /// Current live byte total (after any `free` calls).
    pub fn current_bytes(&self) -> usize {
        self.current_bytes.load(Ordering::Relaxed) as usize
    }

    /// Number of recorded allocations.
    pub fn record_count(&self) -> usize {
        let guard = self.records.lock().unwrap_or_else(|e| e.into_inner());
        guard.len()
    }

    /// All records — returns a cloned snapshot to avoid holding the lock.
    pub fn records(&self) -> Vec<AllocRecord> {
        let guard = self.records.lock().unwrap_or_else(|e| e.into_inner());
        guard.clone()
    }

    /// Records filtered by allocation type (cloned snapshot).
    pub fn records_by_type(&self, alloc_type: AllocationType) -> Vec<AllocRecord> {
        let guard = self.records.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .iter()
            .filter(|r| r.alloc_type == alloc_type)
            .cloned()
            .collect()
    }

    /// Records that exceed the large-allocation threshold (cloned snapshot).
    pub fn large_allocations(&self) -> Vec<AllocRecord> {
        let guard = self.records.lock().unwrap_or_else(|e| e.into_inner());
        guard.iter().filter(|r| r.is_large()).cloned().collect()
    }

    /// Bytes allocated broken down by type: (stack, heap, mmap).
    pub fn bytes_by_type(&self) -> (usize, usize, usize) {
        let guard = self.records.lock().unwrap_or_else(|e| e.into_inner());
        let mut stack = 0usize;
        let mut heap = 0usize;
        let mut mmap = 0usize;
        for r in guard.iter() {
            match r.alloc_type {
                AllocationType::Stack => stack += r.size_bytes,
                AllocationType::Heap => heap += r.size_bytes,
                AllocationType::Mmap => mmap += r.size_bytes,
            }
        }
        (stack, heap, mmap)
    }

    /// Clear all records and reset counters.
    ///
    /// Requires `&mut self` for exclusive ownership — this is intentional
    /// so that resets do not race with concurrent record/free operations.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for AllocationTracker {
    fn default() -> Self {
        Self::new()
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
        let t = AllocationTracker::new();
        t.record(AllocationType::Heap, 100, "a");
        t.record(AllocationType::Stack, 50, "b");
        assert_eq!(t.total_bytes(), 150);
    }

    #[test]
    fn test_tracker_peak_bytes() {
        let t = AllocationTracker::new();
        t.record(AllocationType::Heap, 1000, "alloc1");
        t.record(AllocationType::Heap, 500, "alloc2");
        t.free(1000);
        t.record(AllocationType::Heap, 100, "alloc3");
        // Peak was 1500 before the free.
        assert_eq!(t.peak_bytes(), 1500);
    }

    #[test]
    fn test_tracker_current_bytes_after_free() {
        let t = AllocationTracker::new();
        t.record(AllocationType::Heap, 200, "x");
        t.free(100);
        assert_eq!(t.current_bytes(), 100);
    }

    #[test]
    fn test_tracker_free_saturates_at_zero() {
        let t = AllocationTracker::new();
        t.record(AllocationType::Heap, 10, "y");
        t.free(9999);
        assert_eq!(t.current_bytes(), 0);
    }

    #[test]
    fn test_tracker_record_count() {
        let t = AllocationTracker::new();
        for i in 0..5 {
            t.record(AllocationType::Heap, i * 10, "item");
        }
        assert_eq!(t.record_count(), 5);
    }

    #[test]
    fn test_tracker_records_by_type() {
        let t = AllocationTracker::new();
        t.record(AllocationType::Heap, 100, "h1");
        t.record(AllocationType::Stack, 50, "s1");
        t.record(AllocationType::Heap, 200, "h2");
        let heap_records = t.records_by_type(AllocationType::Heap);
        assert_eq!(heap_records.len(), 2);
    }

    #[test]
    fn test_tracker_large_allocations() {
        let t = AllocationTracker::new();
        t.record(AllocationType::Heap, 512, "small");
        t.record(AllocationType::Heap, 2 * 1024 * 1024, "big");
        let large = t.large_allocations();
        assert_eq!(large.len(), 1);
        assert_eq!(large[0].tag, "big");
    }

    #[test]
    fn test_tracker_bytes_by_type() {
        let t = AllocationTracker::new();
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
        let t = AllocationTracker::new();
        t.record(AllocationType::Heap, 1, "a");
        t.record(AllocationType::Heap, 2, "b");
        let records = t.records();
        let seqs: Vec<u64> = records.iter().map(|r| r.seq).collect();
        assert_eq!(seqs, vec![0, 1]);
    }

    // -----------------------------------------------------------------------
    // Atomic counter concurrency tests (Wave 15, Slice H)
    // -----------------------------------------------------------------------

    /// Spawn 8 threads, each recording 1_000 allocations of 100 B.
    /// After joining, `current_bytes` must equal 800_000.
    /// Then free all; `current_bytes` must reach 0 and `peak_bytes` ≥ 800_000.
    #[test]
    fn test_atomic_counter_concurrent() {
        use std::sync::Arc;

        const N_THREADS: usize = 8;
        const RECORDS_PER_THREAD: usize = 1_000;
        const ALLOC_SIZE: usize = 100;
        const EXPECTED_TOTAL: usize = N_THREADS * RECORDS_PER_THREAD * ALLOC_SIZE;

        let tracker = Arc::new(AllocationTracker::new());

        let handles: Vec<_> = (0..N_THREADS)
            .map(|tid| {
                let t = Arc::clone(&tracker);
                std::thread::spawn(move || {
                    for _ in 0..RECORDS_PER_THREAD {
                        t.record(AllocationType::Heap, ALLOC_SIZE, format!("thread_{}", tid));
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread panicked");
        }

        assert_eq!(
            tracker.current_bytes(),
            EXPECTED_TOTAL,
            "current_bytes after all records must equal {}",
            EXPECTED_TOTAL
        );

        // Free all allocations.
        for _ in 0..N_THREADS * RECORDS_PER_THREAD {
            tracker.free(ALLOC_SIZE);
        }

        assert_eq!(
            tracker.current_bytes(),
            0,
            "current_bytes must reach zero after all frees"
        );
        assert!(
            tracker.peak_bytes() >= EXPECTED_TOTAL,
            "peak_bytes must be >= {}; got {}",
            EXPECTED_TOTAL,
            tracker.peak_bytes()
        );
    }

    /// Single-threaded: allocate 100 B + 200 B, free 100 B, free 200 B.
    /// Peak must be exactly 300 B (point when both were live).
    #[test]
    fn test_atomic_peak_bytes_correctness() {
        let t = AllocationTracker::new();

        t.record(AllocationType::Heap, 100, "a"); // current=100, peak=100
        t.record(AllocationType::Heap, 200, "b"); // current=300, peak=300
        t.free(100); //                              current=200, peak=300
        t.free(200); //                              current=0,   peak=300

        assert_eq!(t.current_bytes(), 0, "current must be 0 after all frees");
        assert_eq!(
            t.peak_bytes(),
            300,
            "peak must be exactly 300 (both allocations live simultaneously)"
        );
    }
}
