//! NDI connection state tracking and lifecycle events.
#![allow(dead_code)]

use std::time::{Duration, Instant};

/// Events that can occur during the lifecycle of an NDI connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionEvent {
    /// The TCP/UDP connection was successfully established.
    Connected,
    /// Connection was cleanly closed by the remote peer.
    Disconnected,
    /// A network or protocol error occurred.
    Error(String),
    /// Connection attempt timed out.
    Timeout,
    /// The connection was intentionally paused (e.g. bandwidth throttle).
    Paused,
    /// A previously paused connection has resumed.
    Resumed,
    /// Authentication succeeded.
    Authenticated,
    /// Authentication failed.
    AuthFailed(String),
}

impl ConnectionEvent {
    /// Returns `true` if this event represents an error condition.
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            ConnectionEvent::Error(_) | ConnectionEvent::Timeout | ConnectionEvent::AuthFailed(_)
        )
    }

    /// A short string identifier for logging.
    pub fn kind(&self) -> &'static str {
        match self {
            ConnectionEvent::Connected => "connected",
            ConnectionEvent::Disconnected => "disconnected",
            ConnectionEvent::Error(_) => "error",
            ConnectionEvent::Timeout => "timeout",
            ConnectionEvent::Paused => "paused",
            ConnectionEvent::Resumed => "resumed",
            ConnectionEvent::Authenticated => "authenticated",
            ConnectionEvent::AuthFailed(_) => "auth_failed",
        }
    }
}

/// High-level states an NDI connection can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NdiConnectionState {
    /// Not yet attempted.
    Idle,
    /// Connection is being established.
    Connecting,
    /// Fully connected and streaming.
    Streaming,
    /// Connection is live but momentarily paused.
    Paused,
    /// A recoverable error occurred; reconnect will be attempted.
    Recovering,
    /// Terminal failure — no further reconnect attempts.
    Failed,
    /// The connection was gracefully closed.
    Closed,
}

impl NdiConnectionState {
    /// Returns `true` for states that represent normal, active operation.
    pub fn is_healthy(&self) -> bool {
        matches!(
            self,
            NdiConnectionState::Streaming | NdiConnectionState::Paused
        )
    }

    /// Returns `true` for terminal states (no further state transitions expected).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            NdiConnectionState::Failed | NdiConnectionState::Closed
        )
    }
}

/// An entry in the connection state history.
#[derive(Debug, Clone)]
struct StateEntry {
    state: NdiConnectionState,
    entered_at: Instant,
}

/// Tracks state transitions for an NDI connection, recording durations.
#[derive(Debug)]
pub struct ConnectionStateTracker {
    current: StateEntry,
    history: Vec<StateEntry>,
    /// Maximum history entries to retain.
    max_history: usize,
}

impl ConnectionStateTracker {
    /// Create a new tracker starting in the `Idle` state.
    pub fn new() -> Self {
        Self {
            current: StateEntry {
                state: NdiConnectionState::Idle,
                entered_at: Instant::now(),
            },
            history: Vec::new(),
            max_history: 64,
        }
    }

    /// Create a tracker with a custom history capacity.
    pub fn with_max_history(max_history: usize) -> Self {
        Self {
            current: StateEntry {
                state: NdiConnectionState::Idle,
                entered_at: Instant::now(),
            },
            history: Vec::new(),
            max_history,
        }
    }

    /// Returns the current connection state.
    pub fn state(&self) -> NdiConnectionState {
        self.current.state
    }

    /// Transition to a new state. Returns the previous state.
    pub fn transition(&mut self, new_state: NdiConnectionState) -> NdiConnectionState {
        let old = self.current.state;
        let old_entry = StateEntry {
            state: old,
            entered_at: self.current.entered_at,
        };
        // Push old entry to history, trimming if needed.
        if self.history.len() >= self.max_history {
            self.history.remove(0);
        }
        self.history.push(old_entry);
        self.current = StateEntry {
            state: new_state,
            entered_at: Instant::now(),
        };
        old
    }

    /// Milliseconds spent in the current state.
    pub fn state_duration_ms(&self) -> u64 {
        self.current.entered_at.elapsed().as_millis() as u64
    }

    /// Duration spent in the current state.
    pub fn state_duration(&self) -> Duration {
        self.current.entered_at.elapsed()
    }

    /// Apply a `ConnectionEvent` and automatically transition to the appropriate state.
    pub fn apply_event(&mut self, event: &ConnectionEvent) {
        let next = match event {
            ConnectionEvent::Connected | ConnectionEvent::Authenticated => {
                NdiConnectionState::Connecting
            }
            ConnectionEvent::Disconnected => NdiConnectionState::Closed,
            ConnectionEvent::Error(_) => NdiConnectionState::Recovering,
            ConnectionEvent::Timeout => NdiConnectionState::Recovering,
            ConnectionEvent::Paused => NdiConnectionState::Paused,
            ConnectionEvent::Resumed => NdiConnectionState::Streaming,
            ConnectionEvent::AuthFailed(_) => NdiConnectionState::Failed,
        };
        self.transition(next);
    }

    /// Returns `true` if the current state is healthy.
    pub fn is_healthy(&self) -> bool {
        self.current.state.is_healthy()
    }

    /// Number of state transitions recorded in history.
    pub fn transition_count(&self) -> usize {
        self.history.len()
    }

    /// Returns the history of previous states (oldest first).
    pub fn history(&self) -> impl Iterator<Item = NdiConnectionState> + '_ {
        self.history.iter().map(|e| e.state)
    }
}

impl Default for ConnectionStateTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_event_is_error_true() {
        assert!(ConnectionEvent::Error("oops".into()).is_error());
        assert!(ConnectionEvent::Timeout.is_error());
        assert!(ConnectionEvent::AuthFailed("bad".into()).is_error());
    }

    #[test]
    fn test_connection_event_is_error_false() {
        assert!(!ConnectionEvent::Connected.is_error());
        assert!(!ConnectionEvent::Disconnected.is_error());
        assert!(!ConnectionEvent::Paused.is_error());
        assert!(!ConnectionEvent::Resumed.is_error());
        assert!(!ConnectionEvent::Authenticated.is_error());
    }

    #[test]
    fn test_connection_event_kind() {
        assert_eq!(ConnectionEvent::Connected.kind(), "connected");
        assert_eq!(ConnectionEvent::Error("x".into()).kind(), "error");
        assert_eq!(ConnectionEvent::Timeout.kind(), "timeout");
    }

    #[test]
    fn test_ndi_connection_state_is_healthy() {
        assert!(NdiConnectionState::Streaming.is_healthy());
        assert!(NdiConnectionState::Paused.is_healthy());
        assert!(!NdiConnectionState::Idle.is_healthy());
        assert!(!NdiConnectionState::Failed.is_healthy());
    }

    #[test]
    fn test_ndi_connection_state_is_terminal() {
        assert!(NdiConnectionState::Failed.is_terminal());
        assert!(NdiConnectionState::Closed.is_terminal());
        assert!(!NdiConnectionState::Streaming.is_terminal());
    }

    #[test]
    fn test_tracker_initial_state() {
        let tracker = ConnectionStateTracker::new();
        assert_eq!(tracker.state(), NdiConnectionState::Idle);
    }

    #[test]
    fn test_tracker_transition() {
        let mut tracker = ConnectionStateTracker::new();
        let prev = tracker.transition(NdiConnectionState::Connecting);
        assert_eq!(prev, NdiConnectionState::Idle);
        assert_eq!(tracker.state(), NdiConnectionState::Connecting);
    }

    #[test]
    fn test_tracker_transition_count() {
        let mut tracker = ConnectionStateTracker::new();
        tracker.transition(NdiConnectionState::Connecting);
        tracker.transition(NdiConnectionState::Streaming);
        assert_eq!(tracker.transition_count(), 2);
    }

    #[test]
    fn test_tracker_state_duration_ms_non_negative() {
        let tracker = ConnectionStateTracker::new();
        // Should be >= 0 and very small (just created).
        assert!(tracker.state_duration_ms() < 1000);
    }

    #[test]
    fn test_tracker_apply_event_error_goes_recovering() {
        let mut tracker = ConnectionStateTracker::new();
        tracker.apply_event(&ConnectionEvent::Error("net fail".into()));
        assert_eq!(tracker.state(), NdiConnectionState::Recovering);
    }

    #[test]
    fn test_tracker_apply_event_auth_failed_goes_failed() {
        let mut tracker = ConnectionStateTracker::new();
        tracker.apply_event(&ConnectionEvent::AuthFailed("bad token".into()));
        assert_eq!(tracker.state(), NdiConnectionState::Failed);
    }

    #[test]
    fn test_tracker_apply_event_resumed_goes_streaming() {
        let mut tracker = ConnectionStateTracker::new();
        tracker.apply_event(&ConnectionEvent::Resumed);
        assert_eq!(tracker.state(), NdiConnectionState::Streaming);
    }

    #[test]
    fn test_tracker_is_healthy_after_stream() {
        let mut tracker = ConnectionStateTracker::new();
        tracker.apply_event(&ConnectionEvent::Resumed);
        assert!(tracker.is_healthy());
    }

    #[test]
    fn test_tracker_history_iteration() {
        let mut tracker = ConnectionStateTracker::new();
        tracker.transition(NdiConnectionState::Connecting);
        tracker.transition(NdiConnectionState::Streaming);
        let hist: Vec<_> = tracker.history().collect();
        assert_eq!(hist[0], NdiConnectionState::Idle);
        assert_eq!(hist[1], NdiConnectionState::Connecting);
    }

    #[test]
    fn test_tracker_max_history_trimmed() {
        let mut tracker = ConnectionStateTracker::with_max_history(3);
        for _ in 0..5 {
            tracker.transition(NdiConnectionState::Connecting);
            tracker.transition(NdiConnectionState::Streaming);
        }
        assert!(tracker.transition_count() <= 3);
    }
}
