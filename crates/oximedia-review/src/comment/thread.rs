//! Threaded comment discussions.

use crate::{
    comment::{Comment, CommentStatus},
    error::ReviewResult,
    CommentId,
};
use serde::{Deserialize, Serialize};

/// A threaded comment discussion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentThread {
    /// Root comment.
    pub root: Comment,
    /// Replies to the root comment.
    pub replies: Vec<Comment>,
    /// Total reply count.
    pub reply_count: usize,
    /// Unresolved reply count.
    pub unresolved_count: usize,
}

impl CommentThread {
    /// Check if the entire thread is resolved.
    #[must_use]
    pub fn is_fully_resolved(&self) -> bool {
        self.root.is_resolved() && self.unresolved_count == 0
    }

    /// Get the latest comment in the thread.
    #[must_use]
    pub fn latest_comment(&self) -> &Comment {
        self.replies
            .iter()
            .max_by_key(|c| c.created_at)
            .unwrap_or(&self.root)
    }

    /// Count participants in the thread.
    #[must_use]
    pub fn participant_count(&self) -> usize {
        let mut participants = std::collections::HashSet::new();
        participants.insert(&self.root.author.id);
        for reply in &self.replies {
            participants.insert(&reply.author.id);
        }
        participants.len()
    }
}

/// Create a comment thread.
///
/// # Arguments
///
/// * `root_id` - ID of the root comment
///
/// # Errors
///
/// Returns error if thread cannot be created.
pub async fn create_thread(root_id: CommentId) -> ReviewResult<CommentThread> {
    // In a real implementation, this would:
    // 1. Load the root comment
    // 2. Load all replies
    // 3. Calculate counts
    // 4. Build the thread structure

    let _ = root_id;

    // Placeholder implementation
    let root = Comment {
        id: root_id,
        session_id: crate::SessionId::new(),
        frame: 0,
        text: String::new(),
        annotation_type: crate::AnnotationType::General,
        author: crate::User {
            id: "system".to_string(),
            name: "System".to_string(),
            email: "system@oximedia.local".to_string(),
            role: crate::UserRole::Reviewer,
        },
        status: CommentStatus::Open,
        priority: crate::comment::CommentPriority::Normal,
        parent_id: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        resolved_at: None,
        resolved_by: None,
    };

    Ok(CommentThread {
        root,
        replies: Vec::new(),
        reply_count: 0,
        unresolved_count: 0,
    })
}

/// Get all threads for a session.
///
/// # Arguments
///
/// * `session_id` - ID of the session
///
/// # Errors
///
/// Returns error if threads cannot be retrieved.
pub async fn get_session_threads(session_id: crate::SessionId) -> ReviewResult<Vec<CommentThread>> {
    // In a real implementation, this would query all root comments
    // and build threads for each

    let _ = session_id;
    Ok(Vec::new())
}

/// Get threads for a specific frame.
///
/// # Arguments
///
/// * `session_id` - ID of the session
/// * `frame` - Frame number
///
/// # Errors
///
/// Returns error if threads cannot be retrieved.
pub async fn get_frame_threads(
    session_id: crate::SessionId,
    frame: i64,
) -> ReviewResult<Vec<CommentThread>> {
    let _ = (session_id, frame);
    Ok(Vec::new())
}

/// Resolve an entire thread.
///
/// # Arguments
///
/// * `root_id` - ID of the root comment
/// * `user_id` - ID of the user resolving the thread
///
/// # Errors
///
/// Returns error if thread cannot be resolved.
pub async fn resolve_thread(root_id: CommentId, user_id: &str) -> ReviewResult<()> {
    // In a real implementation, this would:
    // 1. Resolve the root comment
    // 2. Resolve all replies

    let _ = (root_id, user_id);
    Ok(())
}

/// Get unresolved threads count.
///
/// # Arguments
///
/// * `session_id` - ID of the session
///
/// # Errors
///
/// Returns error if count cannot be retrieved.
pub async fn get_unresolved_thread_count(session_id: crate::SessionId) -> ReviewResult<usize> {
    let threads = get_session_threads(session_id).await?;
    Ok(threads.iter().filter(|t| !t.is_fully_resolved()).count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AnnotationType, SessionId, User, UserRole};
    use chrono::Utc;

    fn create_test_thread() -> CommentThread {
        let root = Comment {
            id: CommentId::new(),
            session_id: SessionId::new(),
            frame: 100,
            text: "Root comment".to_string(),
            annotation_type: AnnotationType::Issue,
            author: User {
                id: "user-1".to_string(),
                name: "User 1".to_string(),
                email: "user1@example.com".to_string(),
                role: UserRole::Reviewer,
            },
            status: CommentStatus::Open,
            priority: crate::comment::CommentPriority::Normal,
            parent_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            resolved_at: None,
            resolved_by: None,
        };

        CommentThread {
            root,
            replies: Vec::new(),
            reply_count: 0,
            unresolved_count: 0,
        }
    }

    #[test]
    fn test_thread_is_fully_resolved() {
        let mut thread = create_test_thread();
        assert!(!thread.is_fully_resolved());

        thread.root.status = CommentStatus::Resolved;
        thread.unresolved_count = 0;
        assert!(thread.is_fully_resolved());
    }

    #[test]
    fn test_thread_latest_comment() {
        let thread = create_test_thread();
        let latest = thread.latest_comment();
        assert_eq!(latest.id, thread.root.id);
    }

    #[test]
    fn test_thread_participant_count() {
        let thread = create_test_thread();
        assert_eq!(thread.participant_count(), 1);
    }

    #[tokio::test]
    async fn test_create_thread() {
        let root_id = CommentId::new();
        let result = create_thread(root_id).await;
        assert!(result.is_ok());
    }
}
