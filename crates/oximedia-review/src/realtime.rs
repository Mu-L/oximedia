//! Real-time collaboration features.

use crate::{error::ReviewResult, SessionId, User};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub mod cursor;
pub mod presence;
pub mod sync;

pub use cursor::{CursorPosition, UserCursor};
pub use presence::{PresenceStatus, UserPresence};
pub use sync::SyncMessage;

/// Real-time event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RealtimeEvent {
    /// User joined the session.
    UserJoined(User),
    /// User left the session.
    UserLeft(String),
    /// Comment added.
    CommentAdded {
        /// Comment ID.
        comment_id: crate::CommentId,
        /// User who added the comment.
        user_id: String,
    },
    /// Drawing added.
    DrawingAdded {
        /// Drawing ID.
        drawing_id: crate::DrawingId,
        /// User who added the drawing.
        user_id: String,
    },
    /// Cursor moved.
    CursorMoved {
        /// User ID.
        user_id: String,
        /// Frame number.
        frame: i64,
        /// Position.
        position: CursorPosition,
    },
    /// Presence updated.
    PresenceUpdated {
        /// User ID.
        user_id: String,
        /// New status.
        status: PresenceStatus,
    },
}

/// Real-time session.
pub struct RealtimeSession {
    session_id: SessionId,
    active_users: Vec<User>,
}

impl RealtimeSession {
    /// Create a new real-time session.
    #[must_use]
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            active_users: Vec::new(),
        }
    }

    /// Add a user to the session.
    pub fn add_user(&mut self, user: User) {
        if !self.active_users.iter().any(|u| u.id == user.id) {
            self.active_users.push(user);
        }
    }

    /// Remove a user from the session.
    pub fn remove_user(&mut self, user_id: &str) {
        self.active_users.retain(|u| u.id != user_id);
    }

    /// Get all active users.
    #[must_use]
    pub fn active_users(&self) -> &[User] {
        &self.active_users
    }

    /// Count active users.
    #[must_use]
    pub fn user_count(&self) -> usize {
        self.active_users.len()
    }

    /// Get session ID.
    #[must_use]
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }
}

/// Broadcast an event to all users in a session.
///
/// # Errors
///
/// Returns error if broadcast fails.
pub async fn broadcast_event(session_id: SessionId, event: RealtimeEvent) -> ReviewResult<()> {
    // In a real implementation, this would:
    // 1. Serialize the event
    // 2. Send to all connected clients via WebSocket
    // 3. Handle delivery failures

    let _ = (session_id, event);
    Ok(())
}

/// Activity log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityLogEntry {
    /// Entry ID.
    pub id: String,
    /// Session ID.
    pub session_id: SessionId,
    /// User who performed the activity.
    pub user_id: String,
    /// Activity type.
    pub activity_type: String,
    /// Activity details.
    pub details: String,
    /// Timestamp.
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UserRole;

    fn create_test_user(id: &str) -> User {
        User {
            id: id.to_string(),
            name: format!("User {}", id),
            email: format!("{}@example.com", id),
            role: UserRole::Reviewer,
        }
    }

    #[test]
    fn test_realtime_session_creation() {
        let session_id = SessionId::new();
        let session = RealtimeSession::new(session_id);
        assert_eq!(session.user_count(), 0);
    }

    #[test]
    fn test_realtime_session_add_user() {
        let session_id = SessionId::new();
        let mut session = RealtimeSession::new(session_id);

        let user = create_test_user("user1");
        session.add_user(user);

        assert_eq!(session.user_count(), 1);
    }

    #[test]
    fn test_realtime_session_remove_user() {
        let session_id = SessionId::new();
        let mut session = RealtimeSession::new(session_id);

        let user = create_test_user("user1");
        session.add_user(user);
        assert_eq!(session.user_count(), 1);

        session.remove_user("user1");
        assert_eq!(session.user_count(), 0);
    }

    #[test]
    fn test_realtime_session_duplicate_user() {
        let session_id = SessionId::new();
        let mut session = RealtimeSession::new(session_id);

        let user = create_test_user("user1");
        session.add_user(user.clone());
        session.add_user(user);

        assert_eq!(session.user_count(), 1);
    }

    #[tokio::test]
    async fn test_broadcast_event() {
        let session_id = SessionId::new();
        let event = RealtimeEvent::UserJoined(create_test_user("user1"));

        let result = broadcast_event(session_id, event).await;
        assert!(result.is_ok());
    }
}
