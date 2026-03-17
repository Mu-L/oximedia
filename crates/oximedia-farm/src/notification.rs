#![allow(dead_code)]
//! Email / webhook notification system for job completion and failure events.
//!
//! ## Design
//!
//! Notifications are fired through a [`NotificationDispatcher`] that holds a
//! list of [`NotificationChannel`]s.  Two built-in channel types are provided:
//!
//! - **Webhook** – sends an HTTP POST request carrying a JSON payload to a
//!   configured URL.  Available only on non-WASM targets where `reqwest` is
//!   compiled in.
//! - **Log** – writes a structured log line via `tracing`; useful as a
//!   no-dependency fallback or for testing.
//!
//! Additional channel types may be added by implementing the
//! [`NotificationChannel`] trait.
//!
//! ## Event types
//!
//! - [`NotificationEvent::JobCompleted`] – fired when all tasks of a job reach
//!   the `Completed` state.
//! - [`NotificationEvent::JobFailed`] – fired when a job is moved to the
//!   `Failed` state.
//! - [`NotificationEvent::TaskFailed`] – fired when an individual task fails.

use crate::{JobId, TaskId};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Event
// ---------------------------------------------------------------------------

/// Events that trigger notifications.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NotificationEvent {
    /// A job has completed successfully.
    JobCompleted {
        /// The job identifier.
        job_id: JobId,
        /// Output file path produced by the job.
        output_path: String,
        /// Wall-clock duration in seconds.
        duration_secs: u64,
    },
    /// A job has failed.
    JobFailed {
        /// The job identifier.
        job_id: JobId,
        /// Human-readable reason for the failure.
        reason: String,
        /// Number of retry attempts that were made.
        retries: u32,
    },
    /// An individual task has failed.
    TaskFailed {
        /// The task identifier.
        task_id: TaskId,
        /// The job the task belonged to.
        job_id: JobId,
        /// Human-readable reason.
        reason: String,
        /// Whether a retry will be attempted.
        will_retry: bool,
    },
}

impl NotificationEvent {
    /// Return a short human-readable label for logging.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::JobCompleted { .. } => "JobCompleted",
            Self::JobFailed { .. } => "JobFailed",
            Self::TaskFailed { .. } => "TaskFailed",
        }
    }

    /// Return the job ID associated with this event.
    #[must_use]
    pub fn job_id(&self) -> &JobId {
        match self {
            Self::JobCompleted { job_id, .. }
            | Self::JobFailed { job_id, .. }
            | Self::TaskFailed { job_id, .. } => job_id,
        }
    }
}

// ---------------------------------------------------------------------------
// Notification payload
// ---------------------------------------------------------------------------

/// The JSON payload sent to webhook endpoints.
#[derive(Debug, Clone, serde::Serialize)]
pub struct NotificationPayload {
    /// Event kind ("JobCompleted", "JobFailed", "TaskFailed").
    pub event: String,
    /// ISO-8601 timestamp when the notification was dispatched.
    pub timestamp: String,
    /// Serialised event data.
    pub data: serde_json::Value,
    /// Optional metadata added by the dispatcher (e.g., farm identifier).
    pub meta: HashMap<String, String>,
}

impl NotificationPayload {
    /// Construct a payload from an event.
    pub fn from_event(
        event: &NotificationEvent,
        meta: HashMap<String, String>,
    ) -> Result<Self, serde_json::Error> {
        Ok(Self {
            event: event.label().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            data: serde_json::to_value(event)?,
            meta,
        })
    }
}

// ---------------------------------------------------------------------------
// Channel trait
// ---------------------------------------------------------------------------

/// Error returned by a notification channel.
#[derive(Debug)]
pub enum NotificationError {
    /// The HTTP request to a webhook endpoint failed.
    Http(String),
    /// Serialisation of the payload failed.
    Serialization(String),
    /// A generic channel-specific error.
    Other(String),
}

impl std::fmt::Display for NotificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(s) => write!(f, "HTTP error: {s}"),
            Self::Serialization(s) => write!(f, "serialization error: {s}"),
            Self::Other(s) => write!(f, "notification error: {s}"),
        }
    }
}

impl std::error::Error for NotificationError {}

/// A notification delivery channel.
pub trait NotificationChannel: Send + Sync {
    /// Synchronously send a notification.
    fn send(&self, payload: &NotificationPayload) -> Result<(), NotificationError>;

    /// Human-readable name for logging.
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Log channel (always available)
// ---------------------------------------------------------------------------

/// A channel that emits notifications as `tracing` log entries.
///
/// Useful as a lightweight default or in test environments.
pub struct LogChannel {
    label: String,
}

impl LogChannel {
    /// Create a log channel with a custom label.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into() }
    }
}

impl Default for LogChannel {
    fn default() -> Self {
        Self::new("default")
    }
}

impl NotificationChannel for LogChannel {
    fn send(&self, payload: &NotificationPayload) -> Result<(), NotificationError> {
        tracing::info!(
            channel = %self.label,
            event = %payload.event,
            timestamp = %payload.timestamp,
            "farm notification"
        );
        Ok(())
    }

    fn name(&self) -> &str {
        &self.label
    }
}

// ---------------------------------------------------------------------------
// Webhook channel (non-WASM only)
// ---------------------------------------------------------------------------

/// Configuration for a webhook notification channel.
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    /// Endpoint URL (HTTP or HTTPS).
    pub url: String,
    /// Optional bearer token for authentication.
    pub bearer_token: Option<String>,
    /// Additional HTTP headers to include.
    pub headers: HashMap<String, String>,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
}

impl WebhookConfig {
    /// Create a minimal configuration pointing to `url`.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            bearer_token: None,
            headers: HashMap::new(),
            timeout_secs: 10,
        }
    }

    /// Set a bearer-token for HTTP `Authorization: Bearer …`.
    #[must_use]
    pub fn with_bearer(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }

    /// Add an arbitrary HTTP header.
    #[must_use]
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
}

/// A channel that sends notifications as HTTP POST requests to a webhook URL
/// using a minimal raw TCP connection.
///
/// The body is a JSON-encoded [`NotificationPayload`].  This implementation
/// does **not** require extra HTTP-client crate features — it uses only the
/// Rust standard library's `TcpStream`.
///
/// **Note:** HTTPS is not supported by this implementation.  For production
/// use with TLS endpoints, wrap the channel in a TLS-aware transport.
#[cfg(not(target_arch = "wasm32"))]
pub struct WebhookChannel {
    config: WebhookConfig,
}

#[cfg(not(target_arch = "wasm32"))]
impl WebhookChannel {
    /// Create a new webhook channel.
    #[must_use]
    pub fn new(config: WebhookConfig) -> Self {
        Self { config }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl NotificationChannel for WebhookChannel {
    fn send(&self, payload: &NotificationPayload) -> Result<(), NotificationError> {
        use std::io::Write as IoWrite;

        let body = serde_json::to_string(payload)
            .map_err(|e| NotificationError::Serialization(e.to_string()))?;

        // Parse host and path from the URL (HTTP only).
        let url = &self.config.url;
        let stripped = url
            .strip_prefix("http://")
            .unwrap_or(url.strip_prefix("https://").unwrap_or(url));
        let (hostport, path) = stripped.split_once('/').map_or_else(
            || (stripped, "/"),
            |(h, p)| (h, if p.is_empty() { "/" } else { p }),
        );
        let path = format!("/{path}");

        // Default to port 80 when no port is specified.
        let addr = if hostport.contains(':') {
            hostport.to_string()
        } else {
            format!("{hostport}:80")
        };

        let mut stream = std::net::TcpStream::connect(&addr)
            .map_err(|e| NotificationError::Http(format!("connect to {addr}: {e}")))?;

        stream
            .set_write_timeout(Some(std::time::Duration::from_secs(self.config.timeout_secs)))
            .map_err(|e| NotificationError::Http(e.to_string()))?;
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(self.config.timeout_secs)))
            .map_err(|e| NotificationError::Http(e.to_string()))?;

        // Build HTTP/1.1 request headers.
        let mut headers = format!(
            "POST {path} HTTP/1.1\r\nHost: {hostport}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n",
            len = body.len()
        );
        if let Some(ref token) = self.config.bearer_token {
            headers.push_str(&format!("Authorization: Bearer {token}\r\n"));
        }
        for (k, v) in &self.config.headers {
            headers.push_str(&format!("{k}: {v}\r\n"));
        }
        headers.push_str("\r\n");
        headers.push_str(&body);

        stream
            .write_all(headers.as_bytes())
            .map_err(|e| NotificationError::Http(e.to_string()))?;

        // Read the status line from the response.
        use std::io::BufRead as _;
        let mut reader = std::io::BufReader::new(stream);
        let mut status_line = String::new();
        reader
            .read_line(&mut status_line)
            .map_err(|e| NotificationError::Http(e.to_string()))?;

        // HTTP/1.1 200 OK
        if !status_line.contains("200") && !status_line.contains("201") && !status_line.contains("204") {
            return Err(NotificationError::Http(format!(
                "webhook returned non-2xx: {status_line}"
            )));
        }

        tracing::debug!(url = %self.config.url, "webhook notification sent");
        Ok(())
    }

    fn name(&self) -> &str {
        &self.config.url
    }
}

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

/// Dispatches notifications to all registered channels.
///
/// Channels are stored as trait objects so heterogeneous implementations can
/// coexist in a single dispatcher.  If a channel fails, the error is logged
/// and processing continues to the remaining channels.
pub struct NotificationDispatcher {
    channels: Vec<Box<dyn NotificationChannel>>,
    /// Metadata included in every payload (e.g., `"farm_id"`, `"region"`).
    meta: HashMap<String, String>,
    /// Filter: only events whose label is in this set will be dispatched.
    /// An empty set means *all* events pass through.
    event_filter: std::collections::HashSet<String>,
}

impl NotificationDispatcher {
    /// Create a dispatcher with no channels.
    #[must_use]
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            meta: HashMap::new(),
            event_filter: std::collections::HashSet::new(),
        }
    }

    /// Register a notification channel.
    pub fn add_channel(&mut self, channel: Box<dyn NotificationChannel>) {
        self.channels.push(channel);
    }

    /// Add a metadata key-value pair included in every payload.
    pub fn add_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.meta.insert(key.into(), value.into());
    }

    /// Restrict dispatching to events whose label is in `labels`.
    ///
    /// Pass an empty slice to clear the filter (accept all events).
    pub fn set_event_filter(&mut self, labels: &[&str]) {
        self.event_filter = labels.iter().map(|s| s.to_string()).collect();
    }

    /// Dispatch `event` to all registered channels.
    ///
    /// Returns the number of channels to which the notification was delivered
    /// successfully.  Failures are logged via `tracing::warn`.
    pub fn dispatch(&self, event: &NotificationEvent) -> usize {
        // Event filter
        if !self.event_filter.is_empty() && !self.event_filter.contains(event.label()) {
            return 0;
        }

        let payload = match NotificationPayload::from_event(event, self.meta.clone()) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("failed to build notification payload: {e}");
                return 0;
            }
        };

        let mut delivered = 0usize;
        for channel in &self.channels {
            match channel.send(&payload) {
                Ok(()) => delivered += 1,
                Err(e) => {
                    tracing::warn!(
                        channel = channel.name(),
                        event = event.label(),
                        error = %e,
                        "notification delivery failed"
                    );
                }
            }
        }
        delivered
    }

    /// Return the number of registered channels.
    #[must_use]
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }
}

impl Default for NotificationDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct CountingChannel {
        name: String,
        count: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }

    impl CountingChannel {
        fn new(name: &str) -> (Self, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
            let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            (
                Self {
                    name: name.to_string(),
                    count: count.clone(),
                },
                count,
            )
        }
    }

    impl NotificationChannel for CountingChannel {
        fn send(&self, _payload: &NotificationPayload) -> Result<(), NotificationError> {
            self.count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(())
        }
        fn name(&self) -> &str {
            &self.name
        }
    }

    struct FailingChannel;

    impl NotificationChannel for FailingChannel {
        fn send(&self, _: &NotificationPayload) -> Result<(), NotificationError> {
            Err(NotificationError::Other("deliberate failure".to_string()))
        }
        fn name(&self) -> &str {
            "failing"
        }
    }

    fn completed_event() -> NotificationEvent {
        NotificationEvent::JobCompleted {
            job_id: crate::JobId::new(),
            output_path: "/out/test.mp4".to_string(),
            duration_secs: 120,
        }
    }

    fn failed_event() -> NotificationEvent {
        NotificationEvent::JobFailed {
            job_id: crate::JobId::new(),
            reason: "encoder crashed".to_string(),
            retries: 3,
        }
    }

    #[test]
    fn test_dispatcher_delivers_to_all_channels() {
        let mut dispatcher = NotificationDispatcher::new();
        let (ch1, count1) = CountingChannel::new("ch1");
        let (ch2, count2) = CountingChannel::new("ch2");
        dispatcher.add_channel(Box::new(ch1));
        dispatcher.add_channel(Box::new(ch2));

        let delivered = dispatcher.dispatch(&completed_event());
        assert_eq!(delivered, 2);
        assert_eq!(count1.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert_eq!(count2.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[test]
    fn test_dispatcher_continues_on_channel_failure() {
        let mut dispatcher = NotificationDispatcher::new();
        dispatcher.add_channel(Box::new(FailingChannel));
        let (good, count) = CountingChannel::new("good");
        dispatcher.add_channel(Box::new(good));

        let delivered = dispatcher.dispatch(&completed_event());
        // Only the good channel succeeded
        assert_eq!(delivered, 1);
        assert_eq!(count.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[test]
    fn test_event_filter_blocks_unwanted_events() {
        let mut dispatcher = NotificationDispatcher::new();
        let (ch, count) = CountingChannel::new("filtered");
        dispatcher.add_channel(Box::new(ch));
        // Only allow JobCompleted
        dispatcher.set_event_filter(&["JobCompleted"]);

        dispatcher.dispatch(&failed_event()); // should be blocked
        assert_eq!(count.load(std::sync::atomic::Ordering::Relaxed), 0);

        dispatcher.dispatch(&completed_event()); // should pass
        assert_eq!(count.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[test]
    fn test_event_filter_empty_allows_all() {
        let mut dispatcher = NotificationDispatcher::new();
        let (ch, count) = CountingChannel::new("all");
        dispatcher.add_channel(Box::new(ch));
        dispatcher.set_event_filter(&[]); // clear filter

        dispatcher.dispatch(&completed_event());
        dispatcher.dispatch(&failed_event());
        assert_eq!(count.load(std::sync::atomic::Ordering::Relaxed), 2);
    }

    #[test]
    fn test_dispatcher_meta_included_in_payload() {
        let mut dispatcher = NotificationDispatcher::new();
        // Use a capturing channel to inspect the payload.
        struct CapturingChannel {
            captured: std::sync::Arc<parking_lot::Mutex<Vec<NotificationPayload>>>,
        }
        impl NotificationChannel for CapturingChannel {
            fn send(&self, payload: &NotificationPayload) -> Result<(), NotificationError> {
                self.captured.lock().push(payload.clone());
                Ok(())
            }
            fn name(&self) -> &str { "capturing" }
        }

        let captured = std::sync::Arc::new(parking_lot::Mutex::new(Vec::new()));
        dispatcher.add_channel(Box::new(CapturingChannel {
            captured: captured.clone(),
        }));
        dispatcher.add_meta("farm_id", "prod-west-1");

        dispatcher.dispatch(&completed_event());

        let payloads = captured.lock();
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].meta.get("farm_id").map(String::as_str), Some("prod-west-1"));
    }

    #[test]
    fn test_no_channels_returns_zero() {
        let dispatcher = NotificationDispatcher::new();
        assert_eq!(dispatcher.dispatch(&completed_event()), 0);
    }

    #[test]
    fn test_log_channel_does_not_panic() {
        let ch = LogChannel::default();
        let payload = NotificationPayload::from_event(
            &completed_event(),
            HashMap::new(),
        )
        .expect("payload");
        assert!(ch.send(&payload).is_ok());
    }

    #[test]
    fn test_event_label() {
        assert_eq!(completed_event().label(), "JobCompleted");
        assert_eq!(failed_event().label(), "JobFailed");
    }

    #[test]
    fn test_notification_error_display() {
        let err = NotificationError::Http("timeout".to_string());
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn test_webhook_config_builder() {
        let cfg = WebhookConfig::new("http://example.com/hook")
            .with_bearer("tok-abc")
            .with_header("X-Farm", "test");
        assert_eq!(cfg.url, "http://example.com/hook");
        assert_eq!(cfg.bearer_token.as_deref(), Some("tok-abc"));
        assert_eq!(cfg.headers.get("X-Farm").map(String::as_str), Some("test"));
    }
}
