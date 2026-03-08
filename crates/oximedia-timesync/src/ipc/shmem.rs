//! Shared memory for low-latency time access.

use crate::error::{TimeSyncError, TimeSyncResult};
use std::path::Path;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

/// Shared memory time data structure.
#[repr(C)]
pub struct SharedTimeData {
    /// Sequence number (for lock-free reading)
    sequence: AtomicU64,
    /// Timestamp (nanoseconds since epoch)
    timestamp_ns: AtomicU64,
    /// Offset from reference (nanoseconds)
    offset_ns: AtomicI64,
    /// Frequency offset (ppb * 1000 for precision)
    freq_offset_ppb_scaled: AtomicI64,
    /// Synchronized flag (0 = no, 1 = yes)
    synchronized: AtomicU64,
}

impl SharedTimeData {
    /// Create new shared time data.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            timestamp_ns: AtomicU64::new(0),
            offset_ns: AtomicI64::new(0),
            freq_offset_ppb_scaled: AtomicI64::new(0),
            synchronized: AtomicU64::new(0),
        }
    }

    /// Write time data (lock-free).
    pub fn write(
        &self,
        timestamp_ns: u64,
        offset_ns: i64,
        freq_offset_ppb: f64,
        synchronized: bool,
    ) {
        // Increment sequence (odd = writing)
        let seq = self.sequence.fetch_add(1, Ordering::Release);

        // Write data
        self.timestamp_ns.store(timestamp_ns, Ordering::Relaxed);
        self.offset_ns.store(offset_ns, Ordering::Relaxed);
        self.freq_offset_ppb_scaled
            .store((freq_offset_ppb * 1000.0) as i64, Ordering::Relaxed);
        self.synchronized
            .store(u64::from(synchronized), Ordering::Relaxed);

        // Increment sequence (even = done writing)
        self.sequence.store(seq + 2, Ordering::Release);
    }

    /// Read time data (lock-free).
    pub fn read(&self) -> TimeSyncResult<TimeSnapshot> {
        // Read with sequence number check for consistency
        const MAX_RETRIES: usize = 10;

        for _ in 0..MAX_RETRIES {
            let seq1 = self.sequence.load(Ordering::Acquire);

            // Check if sequence is even (not being written)
            if seq1 % 2 == 1 {
                std::hint::spin_loop();
                continue;
            }

            // Read data
            let timestamp_ns = self.timestamp_ns.load(Ordering::Relaxed);
            let offset_ns = self.offset_ns.load(Ordering::Relaxed);
            let freq_offset_ppb_scaled = self.freq_offset_ppb_scaled.load(Ordering::Relaxed);
            let synchronized = self.synchronized.load(Ordering::Relaxed);

            // Check sequence again
            let seq2 = self.sequence.load(Ordering::Acquire);

            if seq1 == seq2 {
                // Consistent read
                return Ok(TimeSnapshot {
                    timestamp_ns,
                    offset_ns,
                    freq_offset_ppb: freq_offset_ppb_scaled as f64 / 1000.0,
                    synchronized: synchronized != 0,
                });
            }

            std::hint::spin_loop();
        }

        Err(TimeSyncError::SharedMemory(
            "Failed to read consistent data".to_string(),
        ))
    }
}

impl Default for SharedTimeData {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of time data.
#[derive(Debug, Clone, Copy)]
pub struct TimeSnapshot {
    /// Timestamp (nanoseconds since epoch)
    pub timestamp_ns: u64,
    /// Offset from reference (nanoseconds)
    pub offset_ns: i64,
    /// Frequency offset (ppb)
    pub freq_offset_ppb: f64,
    /// Synchronized flag
    pub synchronized: bool,
}

/// Shared memory manager (placeholder - would use memmap2 in full implementation).
pub struct SharedMemoryManager {
    /// Shared data
    data: SharedTimeData,
}

impl SharedMemoryManager {
    /// Create new shared memory manager.
    pub fn new(_path: &Path) -> TimeSyncResult<Self> {
        // In a full implementation, this would:
        // 1. Create or open a shared memory file using memmap2
        // 2. Map it into memory
        // 3. Initialize the SharedTimeData structure

        Ok(Self {
            data: SharedTimeData::new(),
        })
    }

    /// Get reference to shared data.
    pub fn data(&self) -> &SharedTimeData {
        &self.data
    }

    /// Update time data.
    pub fn update(
        &self,
        timestamp_ns: u64,
        offset_ns: i64,
        freq_offset_ppb: f64,
        synchronized: bool,
    ) {
        self.data
            .write(timestamp_ns, offset_ns, freq_offset_ppb, synchronized);
    }

    /// Read current snapshot.
    pub fn read_snapshot(&self) -> TimeSyncResult<TimeSnapshot> {
        self.data.read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_time_data() {
        let data = SharedTimeData::new();

        data.write(1000, 100, 10.5, true);

        let snapshot = data.read().expect("should succeed in test");
        assert_eq!(snapshot.timestamp_ns, 1000);
        assert_eq!(snapshot.offset_ns, 100);
        assert!((snapshot.freq_offset_ppb - 10.5).abs() < 0.1);
        assert!(snapshot.synchronized);
    }

    #[test]
    fn test_lock_free_read_write() {
        let data = SharedTimeData::new();

        // Write multiple times
        for i in 0..100 {
            data.write(i, i as i64, i as f64, true);
        }

        // Should always get a consistent read
        let snapshot = data.read().expect("should succeed in test");
        assert!(snapshot.synchronized);
    }
}
