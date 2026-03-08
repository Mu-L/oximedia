#![allow(dead_code)]
//! Track content impressions and compute click-through / engagement rates.
//!
//! Every time a recommended item is shown to a user it counts as an
//! impression. If the user clicks or interacts with the item, that is a
//! click. This module records impressions and clicks, computes CTR
//! (click-through rate) per content item and per user, and exposes
//! aggregated metrics for recommendation quality evaluation.

use std::collections::HashMap;

/// Unique identifier for an impression event.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImpressionId(pub String);

impl std::fmt::Display for ImpressionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Position in the recommendation list where the item was shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position(pub u32);

/// A single impression event.
#[derive(Debug, Clone)]
pub struct Impression {
    /// Unique impression ID.
    pub id: ImpressionId,
    /// User who saw the impression.
    pub user_id: String,
    /// Content item that was shown.
    pub content_id: String,
    /// Position in the recommendation list (0-indexed).
    pub position: Position,
    /// Timestamp of the impression (epoch millis).
    pub timestamp_ms: i64,
    /// Whether the user clicked/interacted.
    pub clicked: bool,
    /// Dwell time in milliseconds (0 if not clicked).
    pub dwell_time_ms: u64,
}

/// Aggregated metrics for a single content item.
#[derive(Debug, Clone)]
pub struct ContentMetrics {
    /// Content ID.
    pub content_id: String,
    /// Total impressions.
    pub impressions: u64,
    /// Total clicks.
    pub clicks: u64,
    /// Average position when shown.
    pub avg_position: f64,
    /// Average dwell time in millis (among clicked impressions).
    pub avg_dwell_ms: f64,
}

impl ContentMetrics {
    /// Click-through rate (0.0-1.0).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn ctr(&self) -> f64 {
        if self.impressions == 0 {
            return 0.0;
        }
        self.clicks as f64 / self.impressions as f64
    }
}

/// Aggregated metrics for a single user.
#[derive(Debug, Clone)]
pub struct UserMetrics {
    /// User ID.
    pub user_id: String,
    /// Total impressions shown.
    pub impressions: u64,
    /// Total clicks.
    pub clicks: u64,
    /// Number of distinct content items shown.
    pub unique_items: usize,
}

impl UserMetrics {
    /// Click-through rate for this user.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn ctr(&self) -> f64 {
        if self.impressions == 0 {
            return 0.0;
        }
        self.clicks as f64 / self.impressions as f64
    }
}

/// Global impression statistics.
#[derive(Debug, Clone, Default)]
pub struct ImpressionStats {
    /// Total impressions recorded.
    pub total_impressions: u64,
    /// Total clicks recorded.
    pub total_clicks: u64,
    /// Number of distinct users.
    pub distinct_users: usize,
    /// Number of distinct content items.
    pub distinct_items: usize,
    /// Global CTR.
    pub global_ctr: f64,
}

/// Tracks impressions and computes engagement metrics.
#[derive(Debug)]
pub struct ImpressionTracker {
    /// All impressions indexed by impression ID.
    impressions: HashMap<String, Impression>,
    /// Per-content counters: (impressions, clicks, `sum_position`, `sum_dwell`).
    content_counters: HashMap<String, (u64, u64, u64, u64)>,
    /// Per-user counters: (impressions, clicks, `unique_items` set size tracking).
    user_counters: HashMap<String, (u64, u64, HashMap<String, bool>)>,
    /// Next auto-generated impression ID.
    next_id: u64,
}

impl ImpressionTracker {
    /// Create a new impression tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            impressions: HashMap::new(),
            content_counters: HashMap::new(),
            user_counters: HashMap::new(),
            next_id: 0,
        }
    }

    /// Record an impression (shown but not clicked).
    pub fn record_impression(
        &mut self,
        user_id: &str,
        content_id: &str,
        position: u32,
        timestamp_ms: i64,
    ) -> ImpressionId {
        let id = ImpressionId(format!("imp_{}", self.next_id));
        self.next_id += 1;

        let impression = Impression {
            id: id.clone(),
            user_id: user_id.to_string(),
            content_id: content_id.to_string(),
            position: Position(position),
            timestamp_ms,
            clicked: false,
            dwell_time_ms: 0,
        };

        // Update content counters.
        let entry = self
            .content_counters
            .entry(content_id.to_string())
            .or_insert((0, 0, 0, 0));
        entry.0 += 1;
        entry.2 += u64::from(position);

        // Update user counters.
        let user_entry = self
            .user_counters
            .entry(user_id.to_string())
            .or_insert_with(|| (0, 0, HashMap::new()));
        user_entry.0 += 1;
        user_entry.2.insert(content_id.to_string(), true);

        self.impressions.insert(id.0.clone(), impression);
        id
    }

    /// Record a click on an existing impression.
    pub fn record_click(&mut self, impression_id: &str, dwell_time_ms: u64) -> bool {
        let Some(imp) = self.impressions.get_mut(impression_id) else {
            return false;
        };
        if imp.clicked {
            return false; // Already clicked.
        }
        imp.clicked = true;
        imp.dwell_time_ms = dwell_time_ms;

        // Update content counters.
        if let Some(entry) = self.content_counters.get_mut(&imp.content_id) {
            entry.1 += 1;
            entry.3 += dwell_time_ms;
        }

        // Update user counters.
        if let Some(user_entry) = self.user_counters.get_mut(&imp.user_id) {
            user_entry.1 += 1;
        }

        true
    }

    /// Get metrics for a specific content item.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn content_metrics(&self, content_id: &str) -> Option<ContentMetrics> {
        let &(impressions, clicks, sum_pos, sum_dwell) = self.content_counters.get(content_id)?;
        let avg_position = if impressions > 0 {
            sum_pos as f64 / impressions as f64
        } else {
            0.0
        };
        let avg_dwell_ms = if clicks > 0 {
            sum_dwell as f64 / clicks as f64
        } else {
            0.0
        };
        Some(ContentMetrics {
            content_id: content_id.to_string(),
            impressions,
            clicks,
            avg_position,
            avg_dwell_ms,
        })
    }

    /// Get metrics for a specific user.
    #[must_use]
    pub fn user_metrics(&self, user_id: &str) -> Option<UserMetrics> {
        let (impressions, clicks, ref items) = *self.user_counters.get(user_id)?;
        Some(UserMetrics {
            user_id: user_id.to_string(),
            impressions,
            clicks,
            unique_items: items.len(),
        })
    }

    /// Get global statistics.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn global_stats(&self) -> ImpressionStats {
        let total_impressions: u64 = self.content_counters.values().map(|c| c.0).sum();
        let total_clicks: u64 = self.content_counters.values().map(|c| c.1).sum();
        let global_ctr = if total_impressions > 0 {
            total_clicks as f64 / total_impressions as f64
        } else {
            0.0
        };
        ImpressionStats {
            total_impressions,
            total_clicks,
            distinct_users: self.user_counters.len(),
            distinct_items: self.content_counters.len(),
            global_ctr,
        }
    }

    /// Total number of recorded impressions.
    #[must_use]
    pub fn total_impressions(&self) -> usize {
        self.impressions.len()
    }

    /// Top content items by CTR (minimum impression threshold).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn top_by_ctr(&self, min_impressions: u64, limit: usize) -> Vec<ContentMetrics> {
        let mut items: Vec<ContentMetrics> = self
            .content_counters
            .iter()
            .filter(|(_, &(imps, _, _, _))| imps >= min_impressions)
            .map(|(cid, &(imps, clicks, sum_pos, sum_dwell))| {
                let avg_position = if imps > 0 {
                    sum_pos as f64 / imps as f64
                } else {
                    0.0
                };
                let avg_dwell_ms = if clicks > 0 {
                    sum_dwell as f64 / clicks as f64
                } else {
                    0.0
                };
                ContentMetrics {
                    content_id: cid.clone(),
                    impressions: imps,
                    clicks,
                    avg_position,
                    avg_dwell_ms,
                }
            })
            .collect();
        items.sort_by(|a, b| {
            b.ctr()
                .partial_cmp(&a.ctr())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        items.truncate(limit);
        items
    }

    /// Clear all tracked data.
    pub fn clear(&mut self) {
        self.impressions.clear();
        self.content_counters.clear();
        self.user_counters.clear();
    }
}

impl Default for ImpressionTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_tracker_is_empty() {
        let tracker = ImpressionTracker::new();
        assert_eq!(tracker.total_impressions(), 0);
        let stats = tracker.global_stats();
        assert_eq!(stats.total_impressions, 0);
        assert_eq!(stats.global_ctr, 0.0);
    }

    #[test]
    fn test_record_impression() {
        let mut tracker = ImpressionTracker::new();
        let id = tracker.record_impression("user1", "video1", 0, 1000);
        assert_eq!(id.to_string(), "imp_0");
        assert_eq!(tracker.total_impressions(), 1);
    }

    #[test]
    fn test_record_click_success() {
        let mut tracker = ImpressionTracker::new();
        let id = tracker.record_impression("user1", "video1", 0, 1000);
        assert!(tracker.record_click(&id.0, 5000));
    }

    #[test]
    fn test_record_click_nonexistent() {
        let mut tracker = ImpressionTracker::new();
        assert!(!tracker.record_click("nonexistent", 5000));
    }

    #[test]
    fn test_double_click_rejected() {
        let mut tracker = ImpressionTracker::new();
        let id = tracker.record_impression("user1", "video1", 0, 1000);
        assert!(tracker.record_click(&id.0, 5000));
        assert!(!tracker.record_click(&id.0, 6000));
    }

    #[test]
    fn test_content_metrics_ctr() {
        let mut tracker = ImpressionTracker::new();
        let id1 = tracker.record_impression("u1", "vid", 0, 100);
        tracker.record_impression("u2", "vid", 1, 200);
        tracker.record_click(&id1.0, 3000);

        let metrics = tracker
            .content_metrics("vid")
            .expect("should succeed in test");
        assert_eq!(metrics.impressions, 2);
        assert_eq!(metrics.clicks, 1);
        assert!((metrics.ctr() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_content_metrics_avg_position() {
        let mut tracker = ImpressionTracker::new();
        tracker.record_impression("u1", "vid", 0, 100);
        tracker.record_impression("u2", "vid", 4, 200);
        let metrics = tracker
            .content_metrics("vid")
            .expect("should succeed in test");
        assert!((metrics.avg_position - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_user_metrics() {
        let mut tracker = ImpressionTracker::new();
        let id1 = tracker.record_impression("alice", "v1", 0, 100);
        tracker.record_impression("alice", "v2", 1, 200);
        tracker.record_click(&id1.0, 2000);

        let um = tracker
            .user_metrics("alice")
            .expect("should succeed in test");
        assert_eq!(um.impressions, 2);
        assert_eq!(um.clicks, 1);
        assert_eq!(um.unique_items, 2);
        assert!((um.ctr() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_global_stats() {
        let mut tracker = ImpressionTracker::new();
        let id1 = tracker.record_impression("u1", "v1", 0, 100);
        tracker.record_impression("u2", "v2", 1, 200);
        tracker.record_impression("u1", "v2", 2, 300);
        tracker.record_click(&id1.0, 1000);

        let stats = tracker.global_stats();
        assert_eq!(stats.total_impressions, 3);
        assert_eq!(stats.total_clicks, 1);
        assert_eq!(stats.distinct_users, 2);
        assert_eq!(stats.distinct_items, 2);
    }

    #[test]
    fn test_top_by_ctr() {
        let mut tracker = ImpressionTracker::new();
        // video_a: 2 impressions, 2 clicks => CTR 1.0
        let a1 = tracker.record_impression("u1", "video_a", 0, 100);
        let a2 = tracker.record_impression("u2", "video_a", 0, 200);
        tracker.record_click(&a1.0, 1000);
        tracker.record_click(&a2.0, 2000);
        // video_b: 2 impressions, 1 click => CTR 0.5
        let b1 = tracker.record_impression("u1", "video_b", 1, 300);
        tracker.record_impression("u2", "video_b", 1, 400);
        tracker.record_click(&b1.0, 500);

        let top = tracker.top_by_ctr(2, 10);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].content_id, "video_a");
    }

    #[test]
    fn test_clear() {
        let mut tracker = ImpressionTracker::new();
        tracker.record_impression("u1", "v1", 0, 100);
        tracker.clear();
        assert_eq!(tracker.total_impressions(), 0);
        assert_eq!(tracker.global_stats().distinct_items, 0);
    }

    #[test]
    fn test_content_metrics_none_for_unknown() {
        let tracker = ImpressionTracker::new();
        assert!(tracker.content_metrics("nonexistent").is_none());
    }

    #[test]
    fn test_impression_id_display() {
        let id = ImpressionId("imp_42".to_string());
        assert_eq!(id.to_string(), "imp_42");
    }
}
