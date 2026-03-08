#![allow(dead_code)]
//! SCTE-35 ad insertion and splice point management for broadcast playout.
//!
//! Provides a splice-event model, scheduling of ad breaks, and a splice
//! decision engine that determines when to cut to/from ad content.

use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Unique identifier for a splice event.
pub type SpliceId = u64;

/// Splice command type (modelled after SCTE-35).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpliceCommandType {
    /// Insert an ad break (splice out).
    SpliceInsert,
    /// Return from ad break (splice in / return).
    SpliceReturn,
    /// Cancel a previously scheduled splice.
    SpliceCancel,
    /// Time signal with segmentation descriptor.
    TimeSignal,
}

/// Status of a splice event in the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpliceStatus {
    /// Scheduled but not yet reached.
    Pending,
    /// Currently active (splice-out in progress).
    Active,
    /// Completed (splice-in has happened).
    Completed,
    /// Cancelled before execution.
    Cancelled,
}

/// A splice event representing one ad insertion point.
#[derive(Debug, Clone)]
pub struct SpliceEvent {
    /// Unique splice identifier.
    pub id: SpliceId,
    /// Command type.
    pub command: SpliceCommandType,
    /// Presentation time in microseconds at which the splice occurs.
    pub pts_us: i64,
    /// Duration of the ad break in microseconds (0 if unknown).
    pub duration_us: i64,
    /// Whether an auto-return is expected at pts_us + duration_us.
    pub auto_return: bool,
    /// Current status.
    pub status: SpliceStatus,
    /// Optional descriptive label.
    pub label: String,
}

/// Configuration for the ad insertion engine.
#[derive(Debug, Clone)]
pub struct AdInsertionConfig {
    /// Minimum gap (microseconds) between consecutive splice events.
    pub min_gap_us: i64,
    /// Default ad break duration (microseconds) when unspecified.
    pub default_duration_us: i64,
    /// Whether to enforce auto-return on all splice-inserts.
    pub force_auto_return: bool,
    /// Maximum number of queued splice events.
    pub max_queue_size: usize,
}

impl Default for AdInsertionConfig {
    fn default() -> Self {
        Self {
            min_gap_us: 5_000_000,           // 5 seconds
            default_duration_us: 30_000_000, // 30 seconds
            force_auto_return: true,
            max_queue_size: 256,
        }
    }
}

/// Result of a splice scheduling attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleResult {
    /// Splice was successfully scheduled.
    Scheduled,
    /// Rejected because it violates the minimum gap constraint.
    TooClose,
    /// Rejected because the queue is full.
    QueueFull,
    /// The splice PTS is in the past.
    InThePast,
}

// ---------------------------------------------------------------------------
// Ad Insertion Engine
// ---------------------------------------------------------------------------

/// Manages the lifecycle of SCTE-35-style splice events during playout.
#[derive(Debug)]
pub struct AdInsertionEngine {
    config: AdInsertionConfig,
    /// Splice events ordered by PTS.
    events: BTreeMap<i64, SpliceEvent>,
    next_id: SpliceId,
    /// Current playout PTS (updated externally).
    current_pts_us: i64,
}

impl AdInsertionEngine {
    /// Create a new engine with the given configuration.
    pub fn new(config: AdInsertionConfig) -> Self {
        Self {
            config,
            events: BTreeMap::new(),
            next_id: 1,
            current_pts_us: 0,
        }
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &AdInsertionConfig {
        &self.config
    }

    /// Update the current playout position.
    pub fn set_current_pts(&mut self, pts_us: i64) {
        self.current_pts_us = pts_us;
    }

    /// Return the current playout PTS.
    pub fn current_pts(&self) -> i64 {
        self.current_pts_us
    }

    /// Schedule a new splice-insert (ad break).
    pub fn schedule_insert(
        &mut self,
        pts_us: i64,
        duration_us: Option<i64>,
        label: &str,
    ) -> (ScheduleResult, Option<SpliceId>) {
        if pts_us < self.current_pts_us {
            return (ScheduleResult::InThePast, None);
        }
        if self.events.len() >= self.config.max_queue_size {
            return (ScheduleResult::QueueFull, None);
        }
        // Check minimum gap
        if let Some((&prev_pts, _)) = self.events.range(..pts_us).next_back() {
            if pts_us - prev_pts < self.config.min_gap_us {
                return (ScheduleResult::TooClose, None);
            }
        }
        if let Some((&next_pts, _)) = self.events.range(pts_us + 1..).next() {
            if next_pts - pts_us < self.config.min_gap_us {
                return (ScheduleResult::TooClose, None);
            }
        }

        let dur = duration_us.unwrap_or(self.config.default_duration_us);
        let id = self.next_id;
        self.next_id += 1;

        let event = SpliceEvent {
            id,
            command: SpliceCommandType::SpliceInsert,
            pts_us,
            duration_us: dur,
            auto_return: self.config.force_auto_return,
            status: SpliceStatus::Pending,
            label: label.to_string(),
        };

        self.events.insert(pts_us, event);
        (ScheduleResult::Scheduled, Some(id))
    }

    /// Cancel a splice event by its PTS.
    pub fn cancel_at(&mut self, pts_us: i64) -> bool {
        if let Some(ev) = self.events.get_mut(&pts_us) {
            if ev.status == SpliceStatus::Pending {
                ev.status = SpliceStatus::Cancelled;
                return true;
            }
        }
        false
    }

    /// Advance the engine to the given PTS, activating and completing
    /// events as needed. Returns a list of events that changed status.
    pub fn advance_to(&mut self, pts_us: i64) -> Vec<SpliceEvent> {
        self.current_pts_us = pts_us;
        let mut changed = Vec::new();

        for ev in self.events.values_mut() {
            match ev.status {
                SpliceStatus::Pending if pts_us >= ev.pts_us => {
                    ev.status = SpliceStatus::Active;
                    changed.push(ev.clone());
                }
                SpliceStatus::Active if ev.auto_return && pts_us >= ev.pts_us + ev.duration_us => {
                    ev.status = SpliceStatus::Completed;
                    changed.push(ev.clone());
                }
                _ => {}
            }
        }

        changed
    }

    /// Return the number of pending splice events.
    pub fn pending_count(&self) -> usize {
        self.events
            .values()
            .filter(|e| e.status == SpliceStatus::Pending)
            .count()
    }

    /// Return all events (regardless of status).
    pub fn all_events(&self) -> Vec<&SpliceEvent> {
        self.events.values().collect()
    }

    /// Return the next pending splice event (by PTS).
    pub fn next_pending(&self) -> Option<&SpliceEvent> {
        self.events
            .values()
            .find(|e| e.status == SpliceStatus::Pending)
    }

    /// Remove all completed and cancelled events, returning the count removed.
    pub fn purge_finished(&mut self) -> usize {
        let before = self.events.len();
        self.events.retain(|_, ev| {
            ev.status != SpliceStatus::Completed && ev.status != SpliceStatus::Cancelled
        });
        before - self.events.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = AdInsertionConfig::default();
        assert_eq!(cfg.min_gap_us, 5_000_000);
        assert!(cfg.force_auto_return);
    }

    #[test]
    fn test_schedule_and_count() {
        let mut eng = AdInsertionEngine::new(AdInsertionConfig::default());
        let (res, id) = eng.schedule_insert(10_000_000, None, "ad1");
        assert_eq!(res, ScheduleResult::Scheduled);
        assert!(id.is_some());
        assert_eq!(eng.pending_count(), 1);
    }

    #[test]
    fn test_reject_past_pts() {
        let mut eng = AdInsertionEngine::new(AdInsertionConfig::default());
        eng.set_current_pts(100_000_000);
        let (res, _) = eng.schedule_insert(50_000_000, None, "old");
        assert_eq!(res, ScheduleResult::InThePast);
    }

    #[test]
    fn test_reject_too_close() {
        let mut eng = AdInsertionEngine::new(AdInsertionConfig::default());
        eng.schedule_insert(10_000_000, None, "a");
        let (res, _) = eng.schedule_insert(11_000_000, None, "b");
        assert_eq!(res, ScheduleResult::TooClose);
    }

    #[test]
    fn test_queue_full() {
        let cfg = AdInsertionConfig {
            max_queue_size: 2,
            min_gap_us: 0,
            ..AdInsertionConfig::default()
        };
        let mut eng = AdInsertionEngine::new(cfg);
        eng.schedule_insert(10_000_000, None, "a");
        eng.schedule_insert(20_000_000, None, "b");
        let (res, _) = eng.schedule_insert(30_000_000, None, "c");
        assert_eq!(res, ScheduleResult::QueueFull);
    }

    #[test]
    fn test_advance_activates() {
        let mut eng = AdInsertionEngine::new(AdInsertionConfig::default());
        eng.schedule_insert(10_000_000, Some(5_000_000), "x");
        let changed = eng.advance_to(10_000_000);
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].status, SpliceStatus::Active);
    }

    #[test]
    fn test_advance_completes_auto_return() {
        let mut eng = AdInsertionEngine::new(AdInsertionConfig::default());
        eng.schedule_insert(10_000_000, Some(5_000_000), "x");
        eng.advance_to(10_000_000); // activate
        let changed = eng.advance_to(15_000_000); // complete
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].status, SpliceStatus::Completed);
    }

    #[test]
    fn test_cancel_pending() {
        let mut eng = AdInsertionEngine::new(AdInsertionConfig::default());
        eng.schedule_insert(10_000_000, None, "c");
        assert!(eng.cancel_at(10_000_000));
        assert_eq!(eng.pending_count(), 0);
    }

    #[test]
    fn test_cancel_non_existent() {
        let mut eng = AdInsertionEngine::new(AdInsertionConfig::default());
        assert!(!eng.cancel_at(99_000_000));
    }

    #[test]
    fn test_purge_finished() {
        let mut eng = AdInsertionEngine::new(AdInsertionConfig::default());
        eng.schedule_insert(10_000_000, Some(5_000_000), "a");
        eng.advance_to(10_000_000);
        eng.advance_to(15_000_000);
        let removed = eng.purge_finished();
        assert_eq!(removed, 1);
        assert!(eng.all_events().is_empty());
    }

    #[test]
    fn test_next_pending() {
        let cfg = AdInsertionConfig {
            min_gap_us: 0,
            ..AdInsertionConfig::default()
        };
        let mut eng = AdInsertionEngine::new(cfg);
        eng.schedule_insert(20_000_000, None, "later");
        eng.schedule_insert(10_000_000, None, "sooner");
        let next = eng.next_pending().expect("should succeed in test");
        assert_eq!(next.pts_us, 10_000_000);
    }

    #[test]
    fn test_all_events_returns_all() {
        let cfg = AdInsertionConfig {
            min_gap_us: 0,
            ..AdInsertionConfig::default()
        };
        let mut eng = AdInsertionEngine::new(cfg);
        eng.schedule_insert(10_000_000, None, "a");
        eng.schedule_insert(20_000_000, None, "b");
        assert_eq!(eng.all_events().len(), 2);
    }

    #[test]
    fn test_current_pts_accessor() {
        let mut eng = AdInsertionEngine::new(AdInsertionConfig::default());
        eng.set_current_pts(42_000);
        assert_eq!(eng.current_pts(), 42_000);
    }
}
