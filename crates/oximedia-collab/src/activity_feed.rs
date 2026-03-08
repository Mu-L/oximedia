#![allow(dead_code)]
//! Activity feed for tracking and displaying collaboration events.
//!
//! Records user actions (edits, locks, comments, approvals) in a chronological
//! feed with filtering, pagination, and aggregation capabilities.

use std::collections::HashMap;
use std::fmt;

/// The kind of activity that occurred.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActivityKind {
    /// A user joined the session.
    UserJoined,
    /// A user left the session.
    UserLeft,
    /// A timeline edit was made.
    TimelineEdit,
    /// An audio track was edited.
    AudioEdit,
    /// A clip was added.
    ClipAdded,
    /// A clip was removed.
    ClipRemoved,
    /// An effect was applied.
    EffectApplied,
    /// Color grading was changed.
    ColorGradeChanged,
    /// A comment was added.
    CommentAdded,
    /// An approval was granted.
    ApprovalGranted,
    /// An approval was rejected.
    ApprovalRejected,
    /// A lock was acquired.
    LockAcquired,
    /// A lock was released.
    LockReleased,
    /// An export was started.
    ExportStarted,
    /// An export completed.
    ExportCompleted,
    /// Metadata was updated.
    MetadataUpdated,
}

impl fmt::Display for ActivityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ActivityKind::UserJoined => "user_joined",
            ActivityKind::UserLeft => "user_left",
            ActivityKind::TimelineEdit => "timeline_edit",
            ActivityKind::AudioEdit => "audio_edit",
            ActivityKind::ClipAdded => "clip_added",
            ActivityKind::ClipRemoved => "clip_removed",
            ActivityKind::EffectApplied => "effect_applied",
            ActivityKind::ColorGradeChanged => "color_grade_changed",
            ActivityKind::CommentAdded => "comment_added",
            ActivityKind::ApprovalGranted => "approval_granted",
            ActivityKind::ApprovalRejected => "approval_rejected",
            ActivityKind::LockAcquired => "lock_acquired",
            ActivityKind::LockReleased => "lock_released",
            ActivityKind::ExportStarted => "export_started",
            ActivityKind::ExportCompleted => "export_completed",
            ActivityKind::MetadataUpdated => "metadata_updated",
        };
        write!(f, "{s}")
    }
}

/// A single activity entry in the feed.
#[derive(Debug, Clone)]
pub struct ActivityEntry {
    /// Unique activity ID.
    pub id: u64,
    /// Kind of activity.
    pub kind: ActivityKind,
    /// User who performed the action.
    pub user_id: String,
    /// Human-readable user name.
    pub user_name: String,
    /// Timestamp (epoch milliseconds).
    pub timestamp: u64,
    /// Human-readable description.
    pub description: String,
    /// Optional target resource identifier.
    pub target_id: Option<String>,
    /// Optional additional key-value metadata.
    pub metadata: HashMap<String, String>,
}

impl ActivityEntry {
    /// Create a new activity entry.
    pub fn new(
        id: u64,
        kind: ActivityKind,
        user_id: impl Into<String>,
        user_name: impl Into<String>,
        timestamp: u64,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id,
            kind,
            user_id: user_id.into(),
            user_name: user_name.into(),
            timestamp,
            description: description.into(),
            target_id: None,
            metadata: HashMap::new(),
        }
    }

    /// Set the target resource.
    pub fn with_target(mut self, target_id: impl Into<String>) -> Self {
        self.target_id = Some(target_id.into());
        self
    }

    /// Add a metadata key-value pair.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

impl fmt::Display for ActivityEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} ({}): {}",
            self.timestamp, self.user_name, self.kind, self.description
        )
    }
}

/// Filter criteria for querying the activity feed.
#[derive(Debug, Clone, Default)]
pub struct ActivityFilter {
    /// Filter by user ID.
    pub user_id: Option<String>,
    /// Filter by activity kind.
    pub kind: Option<ActivityKind>,
    /// Filter by minimum timestamp (inclusive).
    pub since: Option<u64>,
    /// Filter by maximum timestamp (inclusive).
    pub until: Option<u64>,
    /// Filter by target resource.
    pub target_id: Option<String>,
}

impl ActivityFilter {
    /// Create an empty filter (matches all).
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by user.
    pub fn for_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Filter by kind.
    pub fn of_kind(mut self, kind: ActivityKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Filter by time range.
    pub fn in_range(mut self, since: u64, until: u64) -> Self {
        self.since = Some(since);
        self.until = Some(until);
        self
    }

    /// Filter by target.
    pub fn for_target(mut self, target_id: impl Into<String>) -> Self {
        self.target_id = Some(target_id.into());
        self
    }

    /// Check whether an entry matches this filter.
    pub fn matches(&self, entry: &ActivityEntry) -> bool {
        if let Some(ref uid) = self.user_id {
            if entry.user_id != *uid {
                return false;
            }
        }
        if let Some(ref k) = self.kind {
            if entry.kind != *k {
                return false;
            }
        }
        if let Some(since) = self.since {
            if entry.timestamp < since {
                return false;
            }
        }
        if let Some(until) = self.until {
            if entry.timestamp > until {
                return false;
            }
        }
        if let Some(ref tid) = self.target_id {
            if entry.target_id.as_deref() != Some(tid.as_str()) {
                return false;
            }
        }
        true
    }
}

/// Aggregated activity statistics.
#[derive(Debug, Clone, Default)]
pub struct ActivityStats {
    /// Total number of activities.
    pub total: usize,
    /// Count by activity kind.
    pub by_kind: HashMap<ActivityKind, usize>,
    /// Count by user.
    pub by_user: HashMap<String, usize>,
    /// Earliest timestamp in the dataset.
    pub earliest: Option<u64>,
    /// Latest timestamp in the dataset.
    pub latest: Option<u64>,
}

/// The activity feed manager.
#[derive(Debug)]
pub struct ActivityFeed {
    /// All entries in chronological order.
    entries: Vec<ActivityEntry>,
    /// Next ID to assign.
    next_id: u64,
    /// Maximum entries to retain (0 = unlimited).
    max_entries: usize,
}

impl ActivityFeed {
    /// Create a new activity feed with an optional capacity limit.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
            max_entries,
        }
    }

    /// Record a new activity, returning the assigned ID.
    pub fn record(
        &mut self,
        kind: ActivityKind,
        user_id: impl Into<String>,
        user_name: impl Into<String>,
        timestamp: u64,
        description: impl Into<String>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let entry = ActivityEntry::new(id, kind, user_id, user_name, timestamp, description);
        self.entries.push(entry);
        self.enforce_limit();
        id
    }

    /// Record a full activity entry.
    pub fn record_entry(&mut self, mut entry: ActivityEntry) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        entry.id = id;
        self.entries.push(entry);
        self.enforce_limit();
        id
    }

    /// Enforce the maximum entries limit by removing the oldest.
    fn enforce_limit(&mut self) {
        if self.max_entries > 0 && self.entries.len() > self.max_entries {
            let excess = self.entries.len() - self.max_entries;
            self.entries.drain(0..excess);
        }
    }

    /// Query entries matching a filter, with pagination.
    pub fn query(
        &self,
        filter: &ActivityFilter,
        offset: usize,
        limit: usize,
    ) -> Vec<&ActivityEntry> {
        self.entries
            .iter()
            .filter(|e| filter.matches(e))
            .skip(offset)
            .take(limit)
            .collect()
    }

    /// Query all entries matching a filter (no pagination).
    pub fn query_all(&self, filter: &ActivityFilter) -> Vec<&ActivityEntry> {
        self.entries.iter().filter(|e| filter.matches(e)).collect()
    }

    /// Return the latest N entries.
    pub fn latest(&self, n: usize) -> Vec<&ActivityEntry> {
        let start = self.entries.len().saturating_sub(n);
        self.entries[start..].iter().collect()
    }

    /// Compute aggregate statistics for entries matching a filter.
    pub fn stats(&self, filter: &ActivityFilter) -> ActivityStats {
        let matching: Vec<&ActivityEntry> =
            self.entries.iter().filter(|e| filter.matches(e)).collect();
        let mut stats = ActivityStats {
            total: matching.len(),
            ..Default::default()
        };
        for entry in &matching {
            *stats.by_kind.entry(entry.kind.clone()).or_insert(0) += 1;
            *stats.by_user.entry(entry.user_id.clone()).or_insert(0) += 1;
            match stats.earliest {
                None => stats.earliest = Some(entry.timestamp),
                Some(e) if entry.timestamp < e => stats.earliest = Some(entry.timestamp),
                _ => {}
            }
            match stats.latest {
                None => stats.latest = Some(entry.timestamp),
                Some(l) if entry.timestamp > l => stats.latest = Some(entry.timestamp),
                _ => {}
            }
        }
        stats
    }

    /// Total number of entries in the feed.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the feed is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for ActivityFeed {
    fn default() -> Self {
        Self::new(10_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn populated_feed() -> ActivityFeed {
        let mut feed = ActivityFeed::new(100);
        feed.record(
            ActivityKind::UserJoined,
            "u1",
            "Alice",
            1000,
            "Alice joined",
        );
        feed.record(
            ActivityKind::TimelineEdit,
            "u1",
            "Alice",
            2000,
            "Alice edited timeline",
        );
        feed.record(
            ActivityKind::CommentAdded,
            "u2",
            "Bob",
            3000,
            "Bob added comment",
        );
        feed.record(
            ActivityKind::ClipAdded,
            "u1",
            "Alice",
            4000,
            "Alice added clip",
        );
        feed.record(
            ActivityKind::ApprovalGranted,
            "u2",
            "Bob",
            5000,
            "Bob approved",
        );
        feed
    }

    #[test]
    fn test_record_and_len() {
        let feed = populated_feed();
        assert_eq!(feed.len(), 5);
        assert!(!feed.is_empty());
    }

    #[test]
    fn test_query_all_no_filter() {
        let feed = populated_feed();
        let filter = ActivityFilter::new();
        let results = feed.query_all(&filter);
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn test_filter_by_user() {
        let feed = populated_feed();
        let filter = ActivityFilter::new().for_user("u1");
        let results = feed.query_all(&filter);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_filter_by_kind() {
        let feed = populated_feed();
        let filter = ActivityFilter::new().of_kind(ActivityKind::CommentAdded);
        let results = feed.query_all(&filter);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].user_id, "u2");
    }

    #[test]
    fn test_filter_by_time_range() {
        let feed = populated_feed();
        let filter = ActivityFilter::new().in_range(2000, 4000);
        let results = feed.query_all(&filter);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_pagination() {
        let feed = populated_feed();
        let filter = ActivityFilter::new();
        let page1 = feed.query(&filter, 0, 2);
        let page2 = feed.query(&filter, 2, 2);
        assert_eq!(page1.len(), 2);
        assert_eq!(page2.len(), 2);
        assert_ne!(page1[0].id, page2[0].id);
    }

    #[test]
    fn test_latest() {
        let feed = populated_feed();
        let latest = feed.latest(2);
        assert_eq!(latest.len(), 2);
        assert_eq!(latest[1].kind, ActivityKind::ApprovalGranted);
    }

    #[test]
    fn test_stats() {
        let feed = populated_feed();
        let filter = ActivityFilter::new();
        let stats = feed.stats(&filter);
        assert_eq!(stats.total, 5);
        assert_eq!(stats.by_user.get("u1"), Some(&3));
        assert_eq!(stats.by_user.get("u2"), Some(&2));
        assert_eq!(stats.earliest, Some(1000));
        assert_eq!(stats.latest, Some(5000));
    }

    #[test]
    fn test_max_entries_enforcement() {
        let mut feed = ActivityFeed::new(3);
        for i in 0..5 {
            feed.record(ActivityKind::TimelineEdit, "u1", "Alice", i * 1000, "edit");
        }
        assert_eq!(feed.len(), 3);
    }

    #[test]
    fn test_clear() {
        let mut feed = populated_feed();
        feed.clear();
        assert!(feed.is_empty());
    }

    #[test]
    fn test_activity_kind_display() {
        assert_eq!(ActivityKind::UserJoined.to_string(), "user_joined");
        assert_eq!(ActivityKind::TimelineEdit.to_string(), "timeline_edit");
        assert_eq!(
            ActivityKind::ExportCompleted.to_string(),
            "export_completed"
        );
    }

    #[test]
    fn test_entry_display() {
        let entry = ActivityEntry::new(
            1,
            ActivityKind::ClipAdded,
            "u1",
            "Alice",
            1000,
            "Added clip X",
        );
        let display = entry.to_string();
        assert!(display.contains("Alice"));
        assert!(display.contains("clip_added"));
    }

    #[test]
    fn test_entry_with_target_and_metadata() {
        let entry = ActivityEntry::new(
            1,
            ActivityKind::EffectApplied,
            "u1",
            "Alice",
            1000,
            "Applied blur",
        )
        .with_target("clip_42")
        .with_metadata("effect_type", "gaussian_blur");
        assert_eq!(entry.target_id, Some("clip_42".to_string()));
        assert_eq!(
            entry.metadata.get("effect_type"),
            Some(&"gaussian_blur".to_string())
        );
    }

    #[test]
    fn test_filter_by_target() {
        let mut feed = ActivityFeed::new(100);
        let e1 = ActivityEntry::new(0, ActivityKind::ClipAdded, "u1", "A", 1000, "desc")
            .with_target("clip_1");
        let e2 = ActivityEntry::new(0, ActivityKind::ClipRemoved, "u1", "A", 2000, "desc")
            .with_target("clip_2");
        feed.record_entry(e1);
        feed.record_entry(e2);
        let filter = ActivityFilter::new().for_target("clip_1");
        let results = feed.query_all(&filter);
        assert_eq!(results.len(), 1);
    }
}
