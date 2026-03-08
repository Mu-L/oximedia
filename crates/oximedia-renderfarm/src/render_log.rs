//! Render farm logging — log levels, individual entries, and a bounded log store.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

/// Severity level of a render log entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RenderLogLevel {
    /// Verbose diagnostic information.
    Debug,
    /// Normal operational information.
    Info,
    /// Potential issue that does not stop rendering.
    Warning,
    /// Recoverable error condition.
    Error,
    /// Unrecoverable failure requiring immediate attention.
    Critical,
}

impl RenderLogLevel {
    /// Returns `true` for levels that indicate a problem (`Warning` and above).
    #[must_use]
    pub fn is_problem(&self) -> bool {
        matches!(self, Self::Warning | Self::Error | Self::Critical)
    }

    /// Returns a numeric code for the level (Debug=0 … Critical=4).
    #[must_use]
    pub fn code(&self) -> u8 {
        match self {
            Self::Debug => 0,
            Self::Info => 1,
            Self::Warning => 2,
            Self::Error => 3,
            Self::Critical => 4,
        }
    }
}

/// A single entry in the render log.
#[derive(Debug, Clone)]
pub struct RenderLogEntry {
    /// ID of the render job this entry belongs to.
    pub job_id: u64,
    /// Optional frame number (absent for job-level messages).
    pub frame: Option<u32>,
    /// Severity level.
    pub level: RenderLogLevel,
    /// Human-readable message.
    pub message: String,
    /// Unix epoch timestamp (seconds).
    pub timestamp_epoch: u64,
}

impl RenderLogEntry {
    /// Creates a new job-level log entry (no frame number).
    #[must_use]
    pub fn new(job_id: u64, level: RenderLogLevel, msg: impl Into<String>, epoch: u64) -> Self {
        Self {
            job_id,
            frame: None,
            level,
            message: msg.into(),
            timestamp_epoch: epoch,
        }
    }

    /// Creates a new frame-level log entry.
    #[must_use]
    pub fn with_frame(
        job_id: u64,
        frame: u32,
        level: RenderLogLevel,
        msg: impl Into<String>,
        epoch: u64,
    ) -> Self {
        Self {
            job_id,
            frame: Some(frame),
            level,
            message: msg.into(),
            timestamp_epoch: epoch,
        }
    }
}

/// A bounded, append-only collection of [`RenderLogEntry`] entries.
///
/// When the capacity is reached, the oldest entry is dropped to make room.
#[derive(Debug)]
pub struct RenderLog {
    entries: Vec<RenderLogEntry>,
    max_entries: usize,
}

impl RenderLog {
    /// Creates a new `RenderLog` with the given maximum capacity.
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Appends an entry.  If at capacity, the oldest entry is discarded.
    pub fn add(&mut self, entry: RenderLogEntry) {
        if self.max_entries == 0 {
            return;
        }
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    /// Returns all `Error` and `Critical` entries belonging to `id`.
    #[must_use]
    pub fn errors_for_job(&self, id: u64) -> Vec<&RenderLogEntry> {
        self.entries
            .iter()
            .filter(|e| {
                e.job_id == id
                    && matches!(e.level, RenderLogLevel::Error | RenderLogLevel::Critical)
            })
            .collect()
    }

    /// Returns all entries at the `Warning` level (across all jobs).
    #[must_use]
    pub fn warnings(&self) -> Vec<&RenderLogEntry> {
        self.entries
            .iter()
            .filter(|e| e.level == RenderLogLevel::Warning)
            .collect()
    }

    /// Returns the current number of entries in the log.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_not_problem() {
        assert!(!RenderLogLevel::Debug.is_problem());
    }

    #[test]
    fn test_info_not_problem() {
        assert!(!RenderLogLevel::Info.is_problem());
    }

    #[test]
    fn test_warning_is_problem() {
        assert!(RenderLogLevel::Warning.is_problem());
    }

    #[test]
    fn test_error_is_problem() {
        assert!(RenderLogLevel::Error.is_problem());
    }

    #[test]
    fn test_critical_is_problem() {
        assert!(RenderLogLevel::Critical.is_problem());
    }

    #[test]
    fn test_level_codes() {
        assert_eq!(RenderLogLevel::Debug.code(), 0);
        assert_eq!(RenderLogLevel::Info.code(), 1);
        assert_eq!(RenderLogLevel::Warning.code(), 2);
        assert_eq!(RenderLogLevel::Error.code(), 3);
        assert_eq!(RenderLogLevel::Critical.code(), 4);
    }

    #[test]
    fn test_entry_new_no_frame() {
        let e = RenderLogEntry::new(42, RenderLogLevel::Info, "started", 1_000_000);
        assert_eq!(e.job_id, 42);
        assert!(e.frame.is_none());
        assert_eq!(e.message, "started");
    }

    #[test]
    fn test_entry_with_frame() {
        let e = RenderLogEntry::with_frame(1, 99, RenderLogLevel::Warning, "slow", 5000);
        assert_eq!(e.frame, Some(99));
    }

    #[test]
    fn test_log_add_and_count() {
        let mut log = RenderLog::new(100);
        log.add(RenderLogEntry::new(1, RenderLogLevel::Info, "ok", 0));
        log.add(RenderLogEntry::new(1, RenderLogLevel::Error, "fail", 1));
        assert_eq!(log.entry_count(), 2);
    }

    #[test]
    fn test_log_capacity_enforced() {
        let mut log = RenderLog::new(3);
        for i in 0..5u64 {
            log.add(RenderLogEntry::new(i, RenderLogLevel::Debug, "msg", i));
        }
        assert_eq!(log.entry_count(), 3);
    }

    #[test]
    fn test_log_errors_for_job() {
        let mut log = RenderLog::new(50);
        log.add(RenderLogEntry::new(7, RenderLogLevel::Error, "e1", 0));
        log.add(RenderLogEntry::new(7, RenderLogLevel::Info, "i1", 1));
        log.add(RenderLogEntry::new(8, RenderLogLevel::Error, "other", 2));
        log.add(RenderLogEntry::new(7, RenderLogLevel::Critical, "c1", 3));
        let errs = log.errors_for_job(7);
        assert_eq!(errs.len(), 2);
    }

    #[test]
    fn test_log_errors_for_job_none() {
        let mut log = RenderLog::new(10);
        log.add(RenderLogEntry::new(1, RenderLogLevel::Info, "ok", 0));
        assert!(log.errors_for_job(1).is_empty());
    }

    #[test]
    fn test_log_warnings() {
        let mut log = RenderLog::new(20);
        log.add(RenderLogEntry::new(1, RenderLogLevel::Warning, "w1", 0));
        log.add(RenderLogEntry::new(2, RenderLogLevel::Error, "e1", 1));
        log.add(RenderLogEntry::new(3, RenderLogLevel::Warning, "w2", 2));
        let warns = log.warnings();
        assert_eq!(warns.len(), 2);
    }

    #[test]
    fn test_log_zero_capacity_ignores_entries() {
        let mut log = RenderLog::new(0);
        log.add(RenderLogEntry::new(1, RenderLogLevel::Info, "msg", 0));
        assert_eq!(log.entry_count(), 0);
    }

    #[test]
    fn test_log_oldest_dropped_on_overflow() {
        let mut log = RenderLog::new(2);
        log.add(RenderLogEntry::new(1, RenderLogLevel::Debug, "first", 0));
        log.add(RenderLogEntry::new(2, RenderLogLevel::Debug, "second", 1));
        log.add(RenderLogEntry::new(3, RenderLogLevel::Debug, "third", 2));
        // Only "second" and "third" should remain.
        let ids: Vec<u64> = log.entries.iter().map(|e| e.job_id).collect();
        assert_eq!(ids, vec![2, 3]);
    }
}
