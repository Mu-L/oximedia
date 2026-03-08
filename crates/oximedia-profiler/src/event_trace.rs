//! Event tracing for profiler-level diagnostic recording.
//!
//! Captures structured trace events tagged with severity levels, enabling
//! post-hoc filtering and analysis of pipeline execution.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Severity level of a trace event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TraceLevel {
    /// Very fine-grained diagnostic information.
    Trace,
    /// General informational message.
    Info,
    /// Potentially problematic situation.
    Warn,
    /// Error that did not halt execution.
    Error,
    /// Critical failure.
    Critical,
}

impl TraceLevel {
    /// Returns a short string label for the level.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
            Self::Critical => "CRIT",
        }
    }

    /// Returns `true` if this level is at or above `other`.
    #[must_use]
    pub fn is_at_least(&self, other: TraceLevel) -> bool {
        *self >= other
    }
}

/// A single trace event recorded by the event tracer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    /// Wall-clock offset from the tracing session start (nanoseconds).
    pub offset_ns: u64,
    /// Severity of this event.
    pub level: TraceLevel,
    /// Source component or module name.
    pub component: String,
    /// Human-readable message.
    pub message: String,
    /// Optional key-value annotations.
    pub annotations: Vec<(String, String)>,
}

impl TraceEvent {
    /// Creates a new trace event.
    #[must_use]
    pub fn new(
        offset_ns: u64,
        level: TraceLevel,
        component: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            offset_ns,
            level,
            component: component.into(),
            message: message.into(),
            annotations: Vec::new(),
        }
    }

    /// Adds a key-value annotation to this event.
    pub fn annotate(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.annotations.push((key.into(), value.into()));
    }

    /// Builder-style annotation, consuming and returning `self`.
    #[must_use]
    pub fn with_annotation(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.annotate(key, value);
        self
    }

    /// Returns the annotation value for the given key, if present.
    #[must_use]
    pub fn get_annotation(&self, key: &str) -> Option<&str> {
        self.annotations
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// Trace event recorder with in-memory ring buffer and level filtering.
///
/// Captures events emitted during profiling sessions and supports querying by
/// level, component, or time range.
#[derive(Debug)]
pub struct EventTrace {
    /// Internal ring buffer of events.
    events: VecDeque<TraceEvent>,
    /// Maximum number of events to retain.
    capacity: usize,
    /// Minimum level that will be recorded (events below this are dropped).
    min_level: TraceLevel,
    /// Session start instant used to compute event offsets.
    session_start: Option<Instant>,
}

impl EventTrace {
    /// Creates a new `EventTrace` with the given capacity and minimum level.
    #[must_use]
    pub fn new(capacity: usize, min_level: TraceLevel) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity),
            capacity,
            min_level,
            session_start: None,
        }
    }

    /// Creates an `EventTrace` with a 4 096-event capacity and `Trace`-level
    /// recording (captures everything).
    #[must_use]
    pub fn verbose(capacity: usize) -> Self {
        Self::new(capacity, TraceLevel::Trace)
    }

    /// Starts a new tracing session, resetting the clock and buffer.
    pub fn start_session(&mut self) {
        self.session_start = Some(Instant::now());
        self.events.clear();
    }

    /// Returns `true` if a session has been started.
    #[must_use]
    pub fn has_session(&self) -> bool {
        self.session_start.is_some()
    }

    /// Returns the current session duration, if a session is active.
    #[must_use]
    pub fn session_duration(&self) -> Option<Duration> {
        self.session_start.map(|t| t.elapsed())
    }

    /// Emits an event into the trace buffer.
    ///
    /// Events below `min_level` are silently dropped.  When the buffer reaches
    /// capacity the oldest event is evicted.
    pub fn emit(
        &mut self,
        level: TraceLevel,
        component: impl Into<String>,
        message: impl Into<String>,
    ) {
        if level < self.min_level {
            return;
        }
        let offset_ns = self
            .session_start
            .map(|t| t.elapsed().as_nanos() as u64)
            .unwrap_or(0);
        let event = TraceEvent::new(offset_ns, level, component, message);
        if self.events.len() >= self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    /// Inserts a pre-built event, respecting capacity and level filter.
    pub fn push(&mut self, event: TraceEvent) {
        if event.level < self.min_level {
            return;
        }
        if self.events.len() >= self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    /// Returns all events at or above `level`.
    #[must_use]
    pub fn filter_by_level(&self, level: TraceLevel) -> Vec<&TraceEvent> {
        self.events.iter().filter(|e| e.level >= level).collect()
    }

    /// Returns all events from the given component.
    #[must_use]
    pub fn filter_by_component<'a>(&'a self, component: &str) -> Vec<&'a TraceEvent> {
        self.events
            .iter()
            .filter(|e| e.component == component)
            .collect()
    }

    /// Returns events whose offset falls within `[start_ns, end_ns)`.
    #[must_use]
    pub fn filter_by_time(&self, start_ns: u64, end_ns: u64) -> Vec<&TraceEvent> {
        self.events
            .iter()
            .filter(|e| e.offset_ns >= start_ns && e.offset_ns < end_ns)
            .collect()
    }

    /// Returns all recorded events as a slice.
    #[must_use]
    pub fn events(&self) -> &VecDeque<TraceEvent> {
        &self.events
    }

    /// Returns the number of events currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns `true` if no events are stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns the configured minimum recording level.
    #[must_use]
    pub fn min_level(&self) -> TraceLevel {
        self.min_level
    }

    /// Clears all stored events without resetting the session clock.
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn trace_with_events() -> EventTrace {
        let mut t = EventTrace::verbose(64);
        t.start_session();
        t.emit(TraceLevel::Trace, "codec", "frame decoded");
        t.emit(TraceLevel::Info, "pipeline", "stage started");
        t.emit(TraceLevel::Warn, "codec", "pts discontinuity");
        t.emit(TraceLevel::Error, "io", "read timeout");
        t
    }

    #[test]
    fn test_new_empty() {
        let t = EventTrace::new(32, TraceLevel::Info);
        assert!(t.is_empty());
        assert_eq!(t.len(), 0);
    }

    #[test]
    fn test_emit_respects_min_level() {
        let mut t = EventTrace::new(32, TraceLevel::Warn);
        t.start_session();
        t.emit(TraceLevel::Trace, "c", "msg");
        t.emit(TraceLevel::Info, "c", "msg");
        assert_eq!(t.len(), 0);
        t.emit(TraceLevel::Warn, "c", "warning");
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn test_capacity_evicts_oldest() {
        let mut t = EventTrace::new(3, TraceLevel::Trace);
        t.start_session();
        t.emit(TraceLevel::Info, "c", "first");
        t.emit(TraceLevel::Info, "c", "second");
        t.emit(TraceLevel::Info, "c", "third");
        t.emit(TraceLevel::Info, "c", "fourth");
        assert_eq!(t.len(), 3);
        assert_eq!(
            t.events().front().expect("should succeed in test").message,
            "second"
        );
    }

    #[test]
    fn test_filter_by_level() {
        let t = trace_with_events();
        let errors = t.filter_by_level(TraceLevel::Error);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].level, TraceLevel::Error);
    }

    #[test]
    fn test_filter_by_level_warn_and_above() {
        let t = trace_with_events();
        let high = t.filter_by_level(TraceLevel::Warn);
        assert_eq!(high.len(), 2); // Warn + Error
    }

    #[test]
    fn test_filter_by_component() {
        let t = trace_with_events();
        let codec = t.filter_by_component("codec");
        assert_eq!(codec.len(), 2);
    }

    #[test]
    fn test_filter_by_time_all() {
        let t = trace_with_events();
        let all = t.filter_by_time(0, u64::MAX);
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn test_filter_by_time_empty_range() {
        let t = trace_with_events();
        let none = t.filter_by_time(u64::MAX - 1, u64::MAX);
        assert!(none.is_empty());
    }

    #[test]
    fn test_session_has_session() {
        let mut t = EventTrace::verbose(32);
        assert!(!t.has_session());
        t.start_session();
        assert!(t.has_session());
    }

    #[test]
    fn test_session_duration_some_after_start() {
        let mut t = EventTrace::verbose(32);
        t.start_session();
        assert!(t.session_duration().is_some());
    }

    #[test]
    fn test_clear_resets_events() {
        let mut t = trace_with_events();
        t.clear();
        assert!(t.is_empty());
        assert!(t.has_session()); // session clock survives clear
    }

    #[test]
    fn test_trace_level_ordering() {
        assert!(TraceLevel::Critical > TraceLevel::Error);
        assert!(TraceLevel::Error > TraceLevel::Warn);
        assert!(TraceLevel::Warn > TraceLevel::Info);
        assert!(TraceLevel::Info > TraceLevel::Trace);
    }

    #[test]
    fn test_trace_level_is_at_least() {
        assert!(TraceLevel::Error.is_at_least(TraceLevel::Warn));
        assert!(!TraceLevel::Info.is_at_least(TraceLevel::Warn));
    }

    #[test]
    fn test_trace_event_annotation() {
        let e = TraceEvent::new(0, TraceLevel::Info, "comp", "msg").with_annotation("fps", "60");
        assert_eq!(e.get_annotation("fps"), Some("60"));
        assert_eq!(e.get_annotation("missing"), None);
    }

    #[test]
    fn test_trace_level_labels() {
        assert_eq!(TraceLevel::Trace.label(), "TRACE");
        assert_eq!(TraceLevel::Critical.label(), "CRIT");
    }

    #[test]
    fn test_min_level_accessor() {
        let t = EventTrace::new(32, TraceLevel::Warn);
        assert_eq!(t.min_level(), TraceLevel::Warn);
    }
}
