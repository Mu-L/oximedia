#![allow(dead_code)]
//! Synchronization audit logging and quality metrics.
//!
//! This module records synchronization events, quality snapshots, and alarm
//! conditions for compliance and diagnostics. It maintains a rolling audit log
//! with configurable retention.

use std::collections::VecDeque;

/// Category of a synchronization audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuditEventKind {
    /// Clock synchronized to reference.
    LockAcquired,
    /// Clock lost synchronization.
    LockLost,
    /// Phase step was applied.
    PhaseStep,
    /// Frequency adjustment was applied.
    FrequencyAdjust,
    /// Holdover mode entered.
    HoldoverEnter,
    /// Holdover mode exited.
    HoldoverExit,
    /// Reference source changed.
    SourceChange,
    /// Quality threshold alarm triggered.
    QualityAlarm,
    /// Manual time set was performed.
    ManualSet,
    /// Configuration was changed.
    ConfigChange,
}

impl AuditEventKind {
    /// Returns a human-readable label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::LockAcquired => "Lock Acquired",
            Self::LockLost => "Lock Lost",
            Self::PhaseStep => "Phase Step",
            Self::FrequencyAdjust => "Frequency Adjust",
            Self::HoldoverEnter => "Holdover Enter",
            Self::HoldoverExit => "Holdover Exit",
            Self::SourceChange => "Source Change",
            Self::QualityAlarm => "Quality Alarm",
            Self::ManualSet => "Manual Set",
            Self::ConfigChange => "Config Change",
        }
    }

    /// Returns `true` if this event indicates a problem.
    #[must_use]
    pub const fn is_alarm(&self) -> bool {
        matches!(
            self,
            Self::LockLost | Self::QualityAlarm | Self::HoldoverEnter
        )
    }
}

/// A single audit event record.
#[derive(Debug, Clone)]
pub struct AuditEvent {
    /// Monotonic sequence number.
    pub seq: u64,
    /// Elapsed seconds since the audit log started (monotonic).
    pub elapsed_secs: f64,
    /// The kind of event.
    pub kind: AuditEventKind,
    /// Measured offset in nanoseconds at the time of the event.
    pub offset_ns: f64,
    /// Optional detail message.
    pub detail: String,
}

impl AuditEvent {
    /// Creates a new audit event.
    #[must_use]
    pub fn new(seq: u64, elapsed_secs: f64, kind: AuditEventKind, offset_ns: f64) -> Self {
        Self {
            seq,
            elapsed_secs,
            kind,
            offset_ns,
            detail: String::new(),
        }
    }

    /// Adds a detail message.
    #[must_use]
    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = detail.to_string();
        self
    }
}

/// Quality snapshot recorded periodically.
#[derive(Debug, Clone, Copy)]
pub struct QualitySnapshot {
    /// Elapsed seconds since log start.
    pub elapsed_secs: f64,
    /// Mean offset in nanoseconds.
    pub mean_offset_ns: f64,
    /// Maximum offset in nanoseconds.
    pub max_offset_ns: f64,
    /// Drift rate in nanoseconds per second.
    pub drift_rate_ns_per_sec: f64,
    /// Whether the clock is currently locked.
    pub locked: bool,
}

/// Alarm threshold configuration.
#[derive(Debug, Clone, Copy)]
pub struct AlarmThresholds {
    /// Maximum acceptable offset in nanoseconds.
    pub max_offset_ns: f64,
    /// Maximum acceptable drift rate in ns/s.
    pub max_drift_ns_per_sec: f64,
    /// Maximum holdover duration in seconds before alarm.
    pub max_holdover_secs: f64,
}

impl Default for AlarmThresholds {
    fn default() -> Self {
        Self {
            max_offset_ns: 100_000.0,      // 100 us
            max_drift_ns_per_sec: 1_000.0, // 1 us/s
            max_holdover_secs: 60.0,       // 1 minute
        }
    }
}

/// Rolling audit log for synchronization events.
#[derive(Debug)]
pub struct SyncAuditLog {
    /// Maximum number of events to retain.
    max_events: usize,
    /// Event log.
    events: VecDeque<AuditEvent>,
    /// Quality snapshots.
    snapshots: VecDeque<QualitySnapshot>,
    /// Maximum number of snapshots to retain.
    max_snapshots: usize,
    /// Next sequence number.
    next_seq: u64,
    /// Alarm thresholds.
    thresholds: AlarmThresholds,
    /// Number of alarm events triggered.
    alarm_count: u64,
}

impl SyncAuditLog {
    /// Creates a new audit log with the given capacity.
    #[must_use]
    pub fn new(max_events: usize, max_snapshots: usize) -> Self {
        Self {
            max_events,
            events: VecDeque::with_capacity(max_events),
            snapshots: VecDeque::with_capacity(max_snapshots),
            max_snapshots,
            next_seq: 1,
            thresholds: AlarmThresholds::default(),
            alarm_count: 0,
        }
    }

    /// Sets alarm thresholds.
    #[must_use]
    pub fn with_thresholds(mut self, thresholds: AlarmThresholds) -> Self {
        self.thresholds = thresholds;
        self
    }

    /// Records an audit event.
    pub fn record_event(
        &mut self,
        elapsed_secs: f64,
        kind: AuditEventKind,
        offset_ns: f64,
        detail: &str,
    ) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;

        if kind.is_alarm() {
            self.alarm_count += 1;
        }

        let event = AuditEvent::new(seq, elapsed_secs, kind, offset_ns).with_detail(detail);

        if self.events.len() >= self.max_events {
            self.events.pop_front();
        }
        self.events.push_back(event);
        seq
    }

    /// Records a quality snapshot.
    pub fn record_snapshot(&mut self, snapshot: QualitySnapshot) {
        if self.snapshots.len() >= self.max_snapshots {
            self.snapshots.pop_front();
        }
        self.snapshots.push_back(snapshot);
    }

    /// Checks the given metrics against alarm thresholds and records alarms.
    pub fn check_alarms(&mut self, elapsed_secs: f64, offset_ns: f64, drift_ns_per_sec: f64) {
        if offset_ns.abs() > self.thresholds.max_offset_ns {
            self.record_event(
                elapsed_secs,
                AuditEventKind::QualityAlarm,
                offset_ns,
                &format!(
                    "Offset {offset_ns:.0} ns exceeds threshold {:.0} ns",
                    self.thresholds.max_offset_ns
                ),
            );
        }
        if drift_ns_per_sec.abs() > self.thresholds.max_drift_ns_per_sec {
            self.record_event(
                elapsed_secs,
                AuditEventKind::QualityAlarm,
                offset_ns,
                &format!(
                    "Drift {drift_ns_per_sec:.1} ns/s exceeds threshold {:.1} ns/s",
                    self.thresholds.max_drift_ns_per_sec
                ),
            );
        }
    }

    /// Returns the total number of events recorded (including evicted ones).
    #[must_use]
    pub fn total_events_recorded(&self) -> u64 {
        self.next_seq - 1
    }

    /// Returns the number of events currently in the log.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Returns the number of snapshots currently in the log.
    #[must_use]
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }

    /// Returns the number of alarm events.
    #[must_use]
    pub fn alarm_count(&self) -> u64 {
        self.alarm_count
    }

    /// Returns a slice of all current events.
    #[must_use]
    pub fn events(&self) -> &VecDeque<AuditEvent> {
        &self.events
    }

    /// Returns events of a given kind.
    #[must_use]
    pub fn events_of_kind(&self, kind: AuditEventKind) -> Vec<&AuditEvent> {
        self.events.iter().filter(|e| e.kind == kind).collect()
    }

    /// Returns the most recent event, if any.
    #[must_use]
    pub fn latest_event(&self) -> Option<&AuditEvent> {
        self.events.back()
    }

    /// Returns the most recent quality snapshot, if any.
    #[must_use]
    pub fn latest_snapshot(&self) -> Option<&QualitySnapshot> {
        self.snapshots.back()
    }

    /// Clears all events and snapshots.
    pub fn clear(&mut self) {
        self.events.clear();
        self.snapshots.clear();
        self.alarm_count = 0;
        self.next_seq = 1;
    }

    /// Returns `true` if the log is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty() && self.snapshots.is_empty()
    }
}

impl Default for SyncAuditLog {
    fn default() -> Self {
        Self::new(1000, 100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_kind_labels() {
        assert_eq!(AuditEventKind::LockAcquired.label(), "Lock Acquired");
        assert_eq!(AuditEventKind::PhaseStep.label(), "Phase Step");
        assert_eq!(AuditEventKind::HoldoverEnter.label(), "Holdover Enter");
    }

    #[test]
    fn test_event_kind_is_alarm() {
        assert!(AuditEventKind::LockLost.is_alarm());
        assert!(AuditEventKind::QualityAlarm.is_alarm());
        assert!(AuditEventKind::HoldoverEnter.is_alarm());
        assert!(!AuditEventKind::LockAcquired.is_alarm());
        assert!(!AuditEventKind::FrequencyAdjust.is_alarm());
    }

    #[test]
    fn test_new_audit_log() {
        let log = SyncAuditLog::new(100, 50);
        assert_eq!(log.event_count(), 0);
        assert_eq!(log.snapshot_count(), 0);
        assert_eq!(log.alarm_count(), 0);
        assert!(log.is_empty());
    }

    #[test]
    fn test_record_event() {
        let mut log = SyncAuditLog::new(100, 50);
        let seq = log.record_event(1.0, AuditEventKind::LockAcquired, 50.0, "PTP lock");
        assert_eq!(seq, 1);
        assert_eq!(log.event_count(), 1);
        assert!(!log.is_empty());
    }

    #[test]
    fn test_event_eviction() {
        let mut log = SyncAuditLog::new(3, 10);
        log.record_event(1.0, AuditEventKind::LockAcquired, 0.0, "");
        log.record_event(2.0, AuditEventKind::FrequencyAdjust, 10.0, "");
        log.record_event(3.0, AuditEventKind::PhaseStep, 20.0, "");
        log.record_event(4.0, AuditEventKind::LockLost, 500.0, "");
        assert_eq!(log.event_count(), 3);
        assert_eq!(log.total_events_recorded(), 4);
    }

    #[test]
    fn test_alarm_count() {
        let mut log = SyncAuditLog::new(100, 50);
        log.record_event(1.0, AuditEventKind::LockAcquired, 0.0, "");
        log.record_event(2.0, AuditEventKind::LockLost, 500.0, "");
        log.record_event(3.0, AuditEventKind::QualityAlarm, 200_000.0, "");
        assert_eq!(log.alarm_count(), 2);
    }

    #[test]
    fn test_events_of_kind() {
        let mut log = SyncAuditLog::new(100, 50);
        log.record_event(1.0, AuditEventKind::PhaseStep, 100.0, "step1");
        log.record_event(2.0, AuditEventKind::LockAcquired, 0.0, "lock");
        log.record_event(3.0, AuditEventKind::PhaseStep, 200.0, "step2");
        let steps = log.events_of_kind(AuditEventKind::PhaseStep);
        assert_eq!(steps.len(), 2);
    }

    #[test]
    fn test_latest_event() {
        let mut log = SyncAuditLog::new(100, 50);
        assert!(log.latest_event().is_none());
        log.record_event(1.0, AuditEventKind::LockAcquired, 0.0, "first");
        log.record_event(2.0, AuditEventKind::LockLost, 100.0, "second");
        let latest = log.latest_event().expect("should succeed in test");
        assert_eq!(latest.kind, AuditEventKind::LockLost);
        assert_eq!(latest.detail, "second");
    }

    #[test]
    fn test_record_snapshot() {
        let mut log = SyncAuditLog::new(100, 3);
        for i in 0..5 {
            #[allow(clippy::cast_precision_loss)]
            let secs = i as f64;
            log.record_snapshot(QualitySnapshot {
                elapsed_secs: secs,
                mean_offset_ns: 50.0,
                max_offset_ns: 100.0,
                drift_rate_ns_per_sec: 5.0,
                locked: true,
            });
        }
        assert_eq!(log.snapshot_count(), 3); // capped at max
    }

    #[test]
    fn test_check_alarms_offset() {
        let mut log = SyncAuditLog::new(100, 50);
        log.check_alarms(1.0, 200_000.0, 0.0); // exceeds 100us threshold
        assert_eq!(log.alarm_count(), 1);
    }

    #[test]
    fn test_check_alarms_drift() {
        let mut log = SyncAuditLog::new(100, 50);
        log.check_alarms(1.0, 50.0, 5_000.0); // exceeds 1us/s threshold
        assert_eq!(log.alarm_count(), 1);
    }

    #[test]
    fn test_check_alarms_no_alarm() {
        let mut log = SyncAuditLog::new(100, 50);
        log.check_alarms(1.0, 50.0, 100.0); // within thresholds
        assert_eq!(log.alarm_count(), 0);
    }

    #[test]
    fn test_clear() {
        let mut log = SyncAuditLog::new(100, 50);
        log.record_event(1.0, AuditEventKind::LockLost, 100.0, "");
        log.record_snapshot(QualitySnapshot {
            elapsed_secs: 1.0,
            mean_offset_ns: 50.0,
            max_offset_ns: 100.0,
            drift_rate_ns_per_sec: 5.0,
            locked: false,
        });
        log.clear();
        assert!(log.is_empty());
        assert_eq!(log.alarm_count(), 0);
    }
}
