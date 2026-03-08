#![allow(dead_code)]
//! Route audit trail for tracking routing changes over time.
//!
//! Every configuration change (connect, disconnect, gain change, preset
//! recall, etc.) is recorded as an [`AuditEntry`] in an [`AuditLog`].
//! The log supports querying by time range, action type, and source/dest
//! identifiers, and can generate summary reports.

use std::fmt;

/// Kind of routing action that was performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuditAction {
    /// A new route was connected.
    Connect,
    /// A route was disconnected.
    Disconnect,
    /// Gain was changed on an existing route.
    GainChange,
    /// A preset was recalled.
    PresetRecall,
    /// A preset was saved.
    PresetSave,
    /// A route was muted.
    Mute,
    /// A route was un-muted.
    Unmute,
    /// A failover event occurred.
    Failover,
}

impl fmt::Display for AuditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect => write!(f, "CONNECT"),
            Self::Disconnect => write!(f, "DISCONNECT"),
            Self::GainChange => write!(f, "GAIN_CHANGE"),
            Self::PresetRecall => write!(f, "PRESET_RECALL"),
            Self::PresetSave => write!(f, "PRESET_SAVE"),
            Self::Mute => write!(f, "MUTE"),
            Self::Unmute => write!(f, "UNMUTE"),
            Self::Failover => write!(f, "FAILOVER"),
        }
    }
}

/// A single audit log entry.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    /// Monotonic sequence number.
    pub seq: u64,
    /// Timestamp in microseconds since log creation.
    pub timestamp_us: u64,
    /// The action that was performed.
    pub action: AuditAction,
    /// Source id involved (if applicable).
    pub source: Option<u32>,
    /// Destination id involved (if applicable).
    pub destination: Option<u32>,
    /// Human-readable detail string.
    pub detail: String,
    /// User / operator who triggered the action.
    pub user: String,
}

impl AuditEntry {
    /// Create a new audit entry.
    pub fn new(seq: u64, timestamp_us: u64, action: AuditAction, detail: &str, user: &str) -> Self {
        Self {
            seq,
            timestamp_us,
            action,
            source: None,
            destination: None,
            detail: detail.to_owned(),
            user: user.to_owned(),
        }
    }

    /// Set source and destination ids.
    pub fn with_route(mut self, source: u32, destination: u32) -> Self {
        self.source = Some(source);
        self.destination = Some(destination);
        self
    }

    /// Set source id only.
    pub fn with_source(mut self, source: u32) -> Self {
        self.source = Some(source);
        self
    }

    /// Timestamp in milliseconds.
    #[allow(clippy::cast_precision_loss)]
    pub fn timestamp_ms(&self) -> f64 {
        self.timestamp_us as f64 / 1_000.0
    }
}

impl fmt::Display for AuditEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:06}] {:.3}ms {} by '{}': {}",
            self.seq,
            self.timestamp_ms(),
            self.action,
            self.user,
            self.detail,
        )
    }
}

/// Query filter for searching the audit log.
#[derive(Debug, Clone, Default)]
pub struct AuditQuery {
    /// Filter by action type.
    pub action: Option<AuditAction>,
    /// Filter by source id.
    pub source: Option<u32>,
    /// Filter by destination id.
    pub destination: Option<u32>,
    /// Filter by user name (exact match).
    pub user: Option<String>,
    /// Start timestamp (inclusive).
    pub from_us: Option<u64>,
    /// End timestamp (inclusive).
    pub to_us: Option<u64>,
    /// Maximum number of results.
    pub limit: Option<usize>,
}

impl AuditQuery {
    /// Create an empty query (matches everything).
    pub fn all() -> Self {
        Self::default()
    }

    /// Filter by action.
    pub fn with_action(mut self, action: AuditAction) -> Self {
        self.action = Some(action);
        self
    }

    /// Filter by source.
    pub fn with_source(mut self, source: u32) -> Self {
        self.source = Some(source);
        self
    }

    /// Filter by destination.
    pub fn with_destination(mut self, dest: u32) -> Self {
        self.destination = Some(dest);
        self
    }

    /// Filter by user.
    pub fn with_user(mut self, user: &str) -> Self {
        self.user = Some(user.to_owned());
        self
    }

    /// Filter by time range.
    pub fn with_time_range(mut self, from_us: u64, to_us: u64) -> Self {
        self.from_us = Some(from_us);
        self.to_us = Some(to_us);
        self
    }

    /// Limit results.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Check whether a single entry matches this query.
    pub fn matches(&self, entry: &AuditEntry) -> bool {
        if let Some(a) = self.action {
            if entry.action != a {
                return false;
            }
        }
        if let Some(s) = self.source {
            if entry.source != Some(s) {
                return false;
            }
        }
        if let Some(d) = self.destination {
            if entry.destination != Some(d) {
                return false;
            }
        }
        if let Some(ref u) = self.user {
            if entry.user != *u {
                return false;
            }
        }
        if let Some(from) = self.from_us {
            if entry.timestamp_us < from {
                return false;
            }
        }
        if let Some(to) = self.to_us {
            if entry.timestamp_us > to {
                return false;
            }
        }
        true
    }
}

/// Per-action count summary.
#[derive(Debug, Clone)]
pub struct ActionCount {
    /// Action type.
    pub action: AuditAction,
    /// Number of occurrences.
    pub count: usize,
}

/// The audit log.
#[derive(Debug)]
pub struct AuditLog {
    /// All entries in chronological order.
    entries: Vec<AuditEntry>,
    /// Next sequence number.
    next_seq: u64,
    /// Maximum entries to retain (0 = unlimited).
    max_entries: usize,
}

impl AuditLog {
    /// Create a new empty audit log.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_seq: 0,
            max_entries: 0,
        }
    }

    /// Create with a maximum entry limit. Oldest entries are evicted first.
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            next_seq: 0,
            max_entries,
        }
    }

    /// Record a new entry. Returns the sequence number.
    pub fn record(
        &mut self,
        timestamp_us: u64,
        action: AuditAction,
        detail: &str,
        user: &str,
    ) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.entries
            .push(AuditEntry::new(seq, timestamp_us, action, detail, user));

        // Evict oldest if needed
        if self.max_entries > 0 && self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }

        seq
    }

    /// Record an entry with route information.
    pub fn record_route(
        &mut self,
        timestamp_us: u64,
        action: AuditAction,
        source: u32,
        destination: u32,
        detail: &str,
        user: &str,
    ) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        let entry = AuditEntry::new(seq, timestamp_us, action, detail, user)
            .with_route(source, destination);
        self.entries.push(entry);

        if self.max_entries > 0 && self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }

        seq
    }

    /// Total entries in the log.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get an entry by sequence number.
    pub fn get(&self, seq: u64) -> Option<&AuditEntry> {
        self.entries.iter().find(|e| e.seq == seq)
    }

    /// Query the log with a filter.
    pub fn query(&self, q: &AuditQuery) -> Vec<&AuditEntry> {
        let mut results: Vec<&AuditEntry> = self.entries.iter().filter(|e| q.matches(e)).collect();
        if let Some(limit) = q.limit {
            results.truncate(limit);
        }
        results
    }

    /// Count entries per action type.
    pub fn action_counts(&self) -> Vec<ActionCount> {
        use std::collections::HashMap;
        let mut map: HashMap<AuditAction, usize> = HashMap::new();
        for e in &self.entries {
            *map.entry(e.action).or_default() += 1;
        }
        let mut out: Vec<ActionCount> = map
            .into_iter()
            .map(|(action, count)| ActionCount { action, count })
            .collect();
        out.sort_by(|a, b| b.count.cmp(&a.count));
        out
    }

    /// Clear the log.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Most recent entry.
    pub fn last_entry(&self) -> Option<&AuditEntry> {
        self.entries.last()
    }

    /// Generate a text report of the log.
    pub fn report(&self) -> String {
        let mut lines = vec![format!("Audit Log ({} entries)", self.entries.len())];
        for entry in &self.entries {
            lines.push(format!("  {entry}"));
        }
        lines.join("\n")
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_new() {
        let e = AuditEntry::new(0, 1000, AuditAction::Connect, "connected", "admin");
        assert_eq!(e.seq, 0);
        assert_eq!(e.action, AuditAction::Connect);
        assert_eq!(e.user, "admin");
    }

    #[test]
    fn test_entry_with_route() {
        let e = AuditEntry::new(0, 0, AuditAction::Connect, "route", "op").with_route(1, 2);
        assert_eq!(e.source, Some(1));
        assert_eq!(e.destination, Some(2));
    }

    #[test]
    fn test_entry_timestamp_ms() {
        let e = AuditEntry::new(0, 5_000, AuditAction::Connect, "", "");
        assert!((e.timestamp_ms() - 5.0).abs() < 1e-9);
    }

    #[test]
    fn test_entry_display() {
        let e = AuditEntry::new(1, 2000, AuditAction::GainChange, "gain set", "tech");
        let s = format!("{e}");
        assert!(s.contains("GAIN_CHANGE"));
        assert!(s.contains("tech"));
    }

    #[test]
    fn test_log_record() {
        let mut log = AuditLog::new();
        let seq = log.record(100, AuditAction::Connect, "route added", "admin");
        assert_eq!(seq, 0);
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn test_log_record_route() {
        let mut log = AuditLog::new();
        log.record_route(100, AuditAction::Connect, 0, 1, "S0->D1", "op");
        let entry = log.get(0).expect("should succeed in test");
        assert_eq!(entry.source, Some(0));
        assert_eq!(entry.destination, Some(1));
    }

    #[test]
    fn test_log_capacity_eviction() {
        let mut log = AuditLog::with_capacity(2);
        log.record(100, AuditAction::Connect, "first", "a");
        log.record(200, AuditAction::Connect, "second", "a");
        log.record(300, AuditAction::Connect, "third", "a");
        assert_eq!(log.len(), 2);
        // First entry should have been evicted
        assert!(log.get(0).is_none());
        assert!(log.get(1).is_some());
    }

    #[test]
    fn test_query_all() {
        let mut log = AuditLog::new();
        log.record(100, AuditAction::Connect, "a", "x");
        log.record(200, AuditAction::Disconnect, "b", "y");
        let results = log.query(&AuditQuery::all());
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_query_by_action() {
        let mut log = AuditLog::new();
        log.record(100, AuditAction::Connect, "a", "x");
        log.record(200, AuditAction::Disconnect, "b", "y");
        log.record(300, AuditAction::Connect, "c", "z");
        let q = AuditQuery::all().with_action(AuditAction::Connect);
        let results = log.query(&q);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_query_by_user() {
        let mut log = AuditLog::new();
        log.record(100, AuditAction::Connect, "a", "alice");
        log.record(200, AuditAction::Connect, "b", "bob");
        let q = AuditQuery::all().with_user("alice");
        let results = log.query(&q);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_query_by_time_range() {
        let mut log = AuditLog::new();
        log.record(100, AuditAction::Connect, "a", "x");
        log.record(500, AuditAction::Connect, "b", "x");
        log.record(900, AuditAction::Connect, "c", "x");
        let q = AuditQuery::all().with_time_range(200, 600);
        let results = log.query(&q);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_query_with_limit() {
        let mut log = AuditLog::new();
        for i in 0..10 {
            log.record(i * 100, AuditAction::Connect, "x", "x");
        }
        let q = AuditQuery::all().with_limit(3);
        let results = log.query(&q);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_action_counts() {
        let mut log = AuditLog::new();
        log.record(100, AuditAction::Connect, "a", "x");
        log.record(200, AuditAction::Connect, "b", "x");
        log.record(300, AuditAction::Disconnect, "c", "x");
        let counts = log.action_counts();
        let connect_count = counts.iter().find(|c| c.action == AuditAction::Connect);
        assert_eq!(connect_count.expect("should succeed in test").count, 2);
    }

    #[test]
    fn test_clear() {
        let mut log = AuditLog::new();
        log.record(100, AuditAction::Connect, "a", "x");
        log.clear();
        assert!(log.is_empty());
    }

    #[test]
    fn test_last_entry() {
        let mut log = AuditLog::new();
        assert!(log.last_entry().is_none());
        log.record(100, AuditAction::Connect, "first", "x");
        log.record(200, AuditAction::Mute, "second", "y");
        assert_eq!(
            log.last_entry().expect("should succeed in test").action,
            AuditAction::Mute
        );
    }

    #[test]
    fn test_report() {
        let mut log = AuditLog::new();
        log.record(100, AuditAction::Connect, "route added", "admin");
        let r = log.report();
        assert!(r.contains("Audit Log"));
        assert!(r.contains("CONNECT"));
    }

    #[test]
    fn test_audit_action_display() {
        assert_eq!(format!("{}", AuditAction::Failover), "FAILOVER");
        assert_eq!(format!("{}", AuditAction::PresetRecall), "PRESET_RECALL");
    }
}
