//! WebSocket connection manager.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

/// WebSocket message types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    /// Transcoding job progress update
    TranscodeProgress {
        /// Job ID
        job_id: String,
        /// Progress percentage (0-100)
        progress: f64,
        /// Current status
        status: String,
    },
    /// Media processing update
    MediaProcessing {
        /// Media ID
        media_id: String,
        /// Processing status
        status: String,
    },
    /// Upload progress update
    UploadProgress {
        /// Upload ID
        upload_id: String,
        /// Progress percentage (0-100)
        progress: f64,
    },
    /// General notification
    Notification {
        /// Notification message
        message: String,
        /// Severity level
        level: String,
    },
    /// Ping/pong for keep-alive
    Ping,
    /// Pong response
    Pong,
}

/// WebSocket connection manager.
#[derive(Clone)]
pub struct WebSocketManager {
    /// User connections: user_id -> broadcast channel
    connections: Arc<DashMap<String, broadcast::Sender<Message>>>,
}

impl WebSocketManager {
    /// Creates a new WebSocket manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            connections: Arc::new(DashMap::new()),
        }
    }

    /// Registers a new connection for a user.
    #[must_use]
    pub fn register(&self, user_id: String) -> broadcast::Receiver<Message> {
        let (tx, rx) = broadcast::channel(100);
        self.connections.insert(user_id, tx);
        rx
    }

    /// Unregisters a connection.
    pub fn unregister(&self, user_id: &str) {
        self.connections.remove(user_id);
    }

    /// Sends a message to a specific user.
    pub fn send_to_user(&self, user_id: &str, message: Message) {
        if let Some(tx) = self.connections.get(user_id) {
            tx.send(message).ok();
        }
    }

    /// Broadcasts a message to all connected users.
    pub fn broadcast(&self, message: Message) {
        for entry in self.connections.iter() {
            entry.value().send(message.clone()).ok();
        }
    }

    /// Gets the number of active connections.
    #[must_use]
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }
}

impl Default for WebSocketManager {
    fn default() -> Self {
        Self::new()
    }
}
