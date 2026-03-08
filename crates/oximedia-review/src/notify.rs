//! Notification system.

use crate::{error::ReviewResult, SessionId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub mod email;
pub mod system;
pub mod webhook;

pub use email::send_email_notification;
pub use system::SystemNotification;
pub use webhook::send_webhook;

/// Notification type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationType {
    /// Comment added.
    CommentAdded,
    /// Comment resolved.
    CommentResolved,
    /// Task assigned.
    TaskAssigned,
    /// Task completed.
    TaskCompleted,
    /// Approval requested.
    ApprovalRequested,
    /// Approval granted.
    ApprovalGranted,
    /// Approval rejected.
    ApprovalRejected,
    /// Session invited.
    SessionInvited,
    /// Session closed.
    SessionClosed,
    /// Deadline approaching.
    DeadlineApproaching,
    /// Deadline passed.
    DeadlinePassed,
}

/// Notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// Notification ID.
    pub id: String,
    /// Session ID.
    pub session_id: SessionId,
    /// Notification type.
    pub notification_type: NotificationType,
    /// Recipient user ID.
    pub recipient: String,
    /// Notification title.
    pub title: String,
    /// Notification message.
    pub message: String,
    /// Link/URL (if any).
    pub link: Option<String>,
    /// Read status.
    pub read: bool,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
}

impl Notification {
    /// Create a new notification.
    #[must_use]
    pub fn new(
        session_id: SessionId,
        notification_type: NotificationType,
        recipient: String,
        title: String,
        message: String,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            notification_type,
            recipient,
            title,
            message,
            link: None,
            read: false,
            created_at: Utc::now(),
        }
    }

    /// Mark notification as read.
    pub fn mark_read(&mut self) {
        self.read = true;
    }

    /// Set link.
    #[must_use]
    pub fn with_link(mut self, link: impl Into<String>) -> Self {
        self.link = Some(link.into());
        self
    }
}

/// Send a notification.
///
/// # Errors
///
/// Returns error if notification fails to send.
pub async fn send_notification(notification: Notification) -> ReviewResult<()> {
    // In a real implementation, this would:
    // 1. Store notification in database
    // 2. Send via configured channels (email, webhook, push, etc.)
    // 3. Handle delivery failures

    let _ = notification;
    Ok(())
}

/// Get notifications for a user.
///
/// # Errors
///
/// Returns error if retrieval fails.
pub async fn get_user_notifications(user_id: &str) -> ReviewResult<Vec<Notification>> {
    // In a real implementation, query database
    let _ = user_id;
    Ok(Vec::new())
}

/// Mark notification as read.
///
/// # Errors
///
/// Returns error if update fails.
pub async fn mark_notification_read(notification_id: &str) -> ReviewResult<()> {
    // In a real implementation, update database
    let _ = notification_id;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_creation() {
        let session_id = SessionId::new();
        let notification = Notification::new(
            session_id,
            NotificationType::CommentAdded,
            "user-1".to_string(),
            "New Comment".to_string(),
            "A new comment was added".to_string(),
        );

        assert_eq!(notification.recipient, "user-1");
        assert!(!notification.read);
        assert!(notification.link.is_none());
    }

    #[test]
    fn test_notification_mark_read() {
        let session_id = SessionId::new();
        let mut notification = Notification::new(
            session_id,
            NotificationType::CommentAdded,
            "user-1".to_string(),
            "New Comment".to_string(),
            "A new comment was added".to_string(),
        );

        assert!(!notification.read);
        notification.mark_read();
        assert!(notification.read);
    }

    #[test]
    fn test_notification_with_link() {
        let session_id = SessionId::new();
        let notification = Notification::new(
            session_id,
            NotificationType::CommentAdded,
            "user-1".to_string(),
            "New Comment".to_string(),
            "A new comment was added".to_string(),
        )
        .with_link("https://example.com/comment/123");

        assert!(notification.link.is_some());
    }

    #[tokio::test]
    async fn test_send_notification() {
        let session_id = SessionId::new();
        let notification = Notification::new(
            session_id,
            NotificationType::CommentAdded,
            "user-1".to_string(),
            "Test".to_string(),
            "Test message".to_string(),
        );

        let result = send_notification(notification).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_user_notifications() {
        let result = get_user_notifications("user-1").await;
        assert!(result.is_ok());
    }
}
