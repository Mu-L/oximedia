//! Zero-copy buffer pool for GPU-style memory management.
//!
//! Provides a reuse-oriented pool of byte buffers, inspired by GPU memory
//! management patterns.  Buffers are acquired by size and alignment,
//! used by the caller, and released back to the pool rather than freed.
//! Unused buffers older than 60 seconds are evicted by [`BufferPool::defragment`].

#![allow(clippy::cast_precision_loss)]

use std::time::Instant;

// ---------------------------------------------------------------------------
// GpuBuffer
// ---------------------------------------------------------------------------

/// A raw byte buffer managed by a [`BufferPool`].
pub struct GpuBuffer {
    /// Unique identifier assigned by the owning pool.
    pub id: u64,
    /// Allocated capacity in bytes.
    pub size_bytes: usize,
    /// Alignment guarantee (in bytes).
    pub alignment: usize,
    /// Backing storage.
    data: Vec<u8>,
    /// Whether this buffer is currently checked out by a caller.
    pub(crate) in_use: bool,
    /// Monotonic timestamp of the most recent acquisition or release.
    pub(crate) created_at: Instant,
    /// Monotonic timestamp of last release (used for eviction).
    pub(crate) last_released_at: Option<Instant>,
}

impl GpuBuffer {
    /// Allocate a new buffer with the given `size` and `alignment`.
    ///
    /// The alignment hint is recorded but the backing `Vec<u8>` uses the
    /// default allocator.  For truly aligned allocations a custom allocator
    /// would be required; the pool still respects the alignment in
    /// compatibility checks.
    #[must_use]
    pub fn new(id: u64, size: usize, alignment: usize) -> Self {
        let effective_alignment = alignment.max(1);
        Self {
            id,
            size_bytes: size,
            alignment: effective_alignment,
            data: vec![0u8; size],
            in_use: false,
            created_at: Instant::now(),
            last_released_at: None,
        }
    }

    /// View the buffer contents as a byte slice.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// View the buffer contents as a mutable byte slice.
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Fill the entire buffer with `value` (memset equivalent).
    pub fn fill(&mut self, value: u8) {
        self.data.fill(value);
    }

    /// Whether this buffer is currently checked out.
    #[must_use]
    pub fn is_in_use(&self) -> bool {
        self.in_use
    }
}

impl std::fmt::Debug for GpuBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuBuffer")
            .field("id", &self.id)
            .field("size_bytes", &self.size_bytes)
            .field("alignment", &self.alignment)
            .field("in_use", &self.in_use)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Pool statistics
// ---------------------------------------------------------------------------

/// Snapshot of pool health metrics.
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Total number of buffers held by the pool (in-use + available).
    pub total_buffers: usize,
    /// Buffers currently checked out by callers.
    pub in_use_buffers: usize,
    /// Buffers available for immediate reuse.
    pub available_buffers: usize,
    /// Sum of all allocated buffer capacities in bytes.
    pub total_allocated_bytes: usize,
    /// Fraction of acquisitions satisfied from the pool (0.0 – 1.0).
    pub reuse_rate: f64,
}

// ---------------------------------------------------------------------------
// BufferPool
// ---------------------------------------------------------------------------

/// A pool of reusable GPU-style byte buffers.
///
/// Callers acquire a buffer via [`acquire`][BufferPool::acquire] (receiving its
/// ID), read/write via [`get_mut`][BufferPool::get_mut], then return it to the
/// pool with [`release`][BufferPool::release].
pub struct BufferPool {
    buffers: Vec<GpuBuffer>,
    next_id: u64,
    total_allocated: usize,
    max_pool_bytes: usize,
    /// Number of acquisitions satisfied by reusing an existing buffer.
    reuse_count: u64,
    /// Total acquisitions ever made (reused + newly allocated).
    alloc_count: u64,
}

impl BufferPool {
    /// Create a new pool that will hold at most `max_pool_bytes` of backing
    /// storage before refusing new allocations.
    #[must_use]
    pub fn new(max_pool_bytes: usize) -> Self {
        Self {
            buffers: Vec::new(),
            next_id: 1,
            total_allocated: 0,
            max_pool_bytes,
            reuse_count: 0,
            alloc_count: 0,
        }
    }

    // -----------------------------------------------------------------------
    // Acquire
    // -----------------------------------------------------------------------

    /// Check out a buffer of at least `size_bytes` with at least `alignment`.
    ///
    /// Strategy: find the *smallest* existing compatible free buffer to
    /// minimise fragmentation.  If none exists, allocate a new one (provided
    /// the pool is below its byte budget).
    ///
    /// Returns the buffer `id` on success, or `None` if no buffer is available
    /// and allocating a new one would exceed the pool's byte budget.
    pub fn acquire(&mut self, size_bytes: usize, alignment: usize) -> Option<u64> {
        self.alloc_count += 1;

        // Find the best (smallest compatible) free buffer.
        let best_idx = self
            .buffers
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                !b.in_use && b.size_bytes >= size_bytes && b.alignment >= alignment.max(1)
            })
            .min_by_key(|(_, b)| b.size_bytes)
            .map(|(idx, _)| idx);

        if let Some(idx) = best_idx {
            self.buffers[idx].in_use = true;
            self.buffers[idx].created_at = Instant::now();
            self.reuse_count += 1;
            return Some(self.buffers[idx].id);
        }

        // No compatible free buffer — try to allocate a new one.
        let effective_alignment = alignment.max(1);
        let new_size = self.total_allocated + size_bytes;
        if new_size > self.max_pool_bytes {
            return None; // over budget
        }

        let id = self.next_id;
        self.next_id += 1;

        let mut buf = GpuBuffer::new(id, size_bytes, effective_alignment);
        buf.in_use = true;
        self.total_allocated += size_bytes;
        self.buffers.push(buf);

        Some(id)
    }

    // -----------------------------------------------------------------------
    // Release
    // -----------------------------------------------------------------------

    /// Return a buffer to the pool by `id`.
    ///
    /// The buffer is kept for future reuse but marked as available.
    /// Returns `true` if the buffer was found and released, `false` otherwise.
    pub fn release(&mut self, id: u64) -> bool {
        if let Some(buf) = self.buffers.iter_mut().find(|b| b.id == id) {
            buf.in_use = false;
            buf.last_released_at = Some(Instant::now());
            true
        } else {
            false
        }
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Borrow the buffer with the given `id`.
    #[must_use]
    pub fn get(&self, id: u64) -> Option<&GpuBuffer> {
        self.buffers.iter().find(|b| b.id == id)
    }

    /// Mutably borrow the buffer with the given `id`.
    #[must_use]
    pub fn get_mut(&mut self, id: u64) -> Option<&mut GpuBuffer> {
        self.buffers.iter_mut().find(|b| b.id == id)
    }

    // -----------------------------------------------------------------------
    // Defragmentation
    // -----------------------------------------------------------------------

    /// Evict all free buffers that have not been used for more than 60 seconds.
    ///
    /// In-use buffers are never evicted.
    pub fn defragment(&mut self) {
        let now = Instant::now();
        let eviction_threshold = std::time::Duration::from_secs(60);

        let mut bytes_freed = 0usize;
        self.buffers.retain(|buf| {
            if buf.in_use {
                return true; // never evict live buffers
            }
            let idle_since = buf.last_released_at.unwrap_or(buf.created_at);
            if now.duration_since(idle_since) > eviction_threshold {
                bytes_freed += buf.size_bytes;
                false // evict
            } else {
                true
            }
        });
        self.total_allocated = self.total_allocated.saturating_sub(bytes_freed);
    }

    // -----------------------------------------------------------------------
    // Stats
    // -----------------------------------------------------------------------

    /// Snapshot of pool metrics.
    #[must_use]
    pub fn stats(&self) -> PoolStats {
        let in_use = self.buffers.iter().filter(|b| b.in_use).count();
        let available = self.buffers.len() - in_use;
        let reuse_rate = if self.alloc_count == 0 {
            0.0
        } else {
            self.reuse_count as f64 / self.alloc_count as f64
        };
        PoolStats {
            total_buffers: self.buffers.len(),
            in_use_buffers: in_use,
            available_buffers: available,
            total_allocated_bytes: self.total_allocated,
            reuse_rate,
        }
    }

    /// Total bytes currently under management.
    #[must_use]
    pub fn total_allocated_bytes(&self) -> usize {
        self.total_allocated
    }

    /// Maximum pool capacity in bytes.
    #[must_use]
    pub fn max_pool_bytes(&self) -> usize {
        self.max_pool_bytes
    }
}

impl std::fmt::Debug for BufferPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BufferPool")
            .field("buffers", &self.buffers.len())
            .field("total_allocated", &self.total_allocated)
            .field("max_pool_bytes", &self.max_pool_bytes)
            .field("alloc_count", &self.alloc_count)
            .field("reuse_count", &self.reuse_count)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- GpuBuffer ---

    #[test]
    fn test_gpu_buffer_new() {
        let buf = GpuBuffer::new(1, 1024, 64);
        assert_eq!(buf.id, 1);
        assert_eq!(buf.size_bytes, 1024);
        assert_eq!(buf.alignment, 64);
        assert_eq!(buf.as_slice().len(), 1024);
        assert!(!buf.is_in_use());
    }

    #[test]
    fn test_gpu_buffer_fill() {
        let mut buf = GpuBuffer::new(2, 16, 4);
        buf.fill(0xAB);
        assert!(buf.as_slice().iter().all(|&b| b == 0xAB));
    }

    #[test]
    fn test_gpu_buffer_as_mut_slice() {
        let mut buf = GpuBuffer::new(3, 8, 1);
        buf.as_mut_slice()[0] = 42;
        assert_eq!(buf.as_slice()[0], 42);
    }

    // --- BufferPool::new ---

    #[test]
    fn test_pool_new_empty() {
        let pool = BufferPool::new(1024 * 1024);
        let stats = pool.stats();
        assert_eq!(stats.total_buffers, 0);
        assert_eq!(stats.reuse_rate, 0.0);
    }

    // --- acquire / release ---

    #[test]
    fn test_pool_acquire_and_release() {
        let mut pool = BufferPool::new(1024 * 1024);
        let id = pool.acquire(256, 4).expect("acquire failed");
        assert!(pool.get(id).expect("missing").is_in_use());

        let released = pool.release(id);
        assert!(released, "release should succeed");
        assert!(!pool.get(id).expect("missing").is_in_use());
    }

    #[test]
    fn test_pool_reuse() {
        let mut pool = BufferPool::new(1024 * 1024);
        let id1 = pool.acquire(512, 4).expect("first acquire");
        pool.release(id1);
        let id2 = pool.acquire(512, 4).expect("second acquire");
        // The pool should have reused the same buffer.
        assert_eq!(id1, id2, "expected buffer reuse");
        let stats = pool.stats();
        assert!(stats.reuse_rate > 0.0);
    }

    #[test]
    fn test_pool_smallest_compatible_preferred() {
        let mut pool = BufferPool::new(4 * 1024 * 1024);
        // Allocate two free buffers of different sizes.
        let big = pool.acquire(4096, 4).expect("big");
        let small = pool.acquire(256, 4).expect("small");
        pool.release(big);
        pool.release(small);
        // Requesting 128 bytes: should get the 256-byte buffer (smallest compat).
        let id = pool.acquire(128, 4).expect("reacquire");
        assert_eq!(id, small, "should prefer smaller buffer");
    }

    #[test]
    fn test_pool_budget_exceeded() {
        let mut pool = BufferPool::new(100);
        // First acquisition should succeed.
        let id = pool.acquire(80, 1).expect("first");
        // Second would exceed budget while first is in use.
        let result = pool.acquire(80, 1);
        assert!(result.is_none(), "should fail over budget");
        pool.release(id);
    }

    #[test]
    fn test_pool_release_unknown_id() {
        let mut pool = BufferPool::new(1024);
        assert!(
            !pool.release(9999),
            "releasing unknown id should return false"
        );
    }

    #[test]
    fn test_pool_get_missing() {
        let pool = BufferPool::new(1024);
        assert!(pool.get(42).is_none());
    }

    // --- get_mut ---

    #[test]
    fn test_pool_get_mut_write() {
        let mut pool = BufferPool::new(1024 * 1024);
        let id = pool.acquire(64, 1).expect("acquire");
        {
            let buf = pool.get_mut(id).expect("get_mut");
            buf.as_mut_slice()[0] = 0xFF;
        }
        assert_eq!(pool.get(id).expect("get").as_slice()[0], 0xFF);
    }

    // --- stats ---

    #[test]
    fn test_pool_stats_in_use_count() {
        let mut pool = BufferPool::new(1024 * 1024);
        let id1 = pool.acquire(128, 1).expect("a1");
        let _id2 = pool.acquire(128, 1).expect("a2");
        pool.release(id1);
        let stats = pool.stats();
        assert_eq!(stats.total_buffers, 2);
        assert_eq!(stats.in_use_buffers, 1);
        assert_eq!(stats.available_buffers, 1);
    }

    // --- defragment ---

    #[test]
    fn test_pool_defragment_keeps_in_use() {
        let mut pool = BufferPool::new(1024 * 1024);
        let id = pool.acquire(64, 1).expect("acquire");
        // Run defragment while buffer is in use — it should survive.
        pool.defragment();
        assert!(
            pool.get(id).is_some(),
            "in-use buffer should not be evicted"
        );
    }

    #[test]
    fn test_pool_defragment_recently_released_kept() {
        let mut pool = BufferPool::new(1024 * 1024);
        let id = pool.acquire(64, 1).expect("acquire");
        pool.release(id);
        // Buffer was just released — defragment should keep it (not 60s old).
        pool.defragment();
        assert!(
            pool.get(id).is_some(),
            "recently released buffer should survive"
        );
    }
}
