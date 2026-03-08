//! Reply to comments.

use crate::{
    comment::{Comment, CommentPriority, CommentStatus},
    error::{ReviewError, ReviewResult},
    AnnotationType, CommentId, SessionId, User, UserRole,
};
use chrono::Utc;

/// Add a reply to a comment.
///
/// # Arguments
///
/// * `parent_id` - ID of the parent comment
/// * `text` - Reply text
///
/// # Errors
///
/// Returns error if reply cannot be added.
pub async fn add_reply(parent_id: CommentId, text: &str) -> ReviewResult<CommentId> {
    let reply_id = CommentId::new();
    let now = Utc::now();

    // Create default author
    let author = User {
        id: "system".to_string(),
        name: "System".to_string(),
        email: "system@oximedia.local".to_string(),
        role: UserRole::Reviewer,
    };

    let _reply = Comment {
        id: reply_id,
        session_id: SessionId::new(), // In real implementation, get from parent
        frame: 0,                     // In real implementation, get from parent
        text: text.to_string(),
        annotation_type: AnnotationType::General,
        author,
        status: CommentStatus::Open,
        priority: CommentPriority::Normal,
        parent_id: Some(parent_id),
        created_at: now,
        updated_at: now,
        resolved_at: None,
        resolved_by: None,
    };

    Ok(reply_id)
}

/// Add a reply with author information.
///
/// # Arguments
///
/// * `parent_id` - ID of the parent comment
/// * `text` - Reply text
/// * `author` - Reply author
///
/// # Errors
///
/// Returns error if reply cannot be added.
pub async fn add_reply_with_author(
    parent_id: CommentId,
    text: &str,
    author: User,
) -> ReviewResult<CommentId> {
    let reply_id = CommentId::new();
    let now = Utc::now();

    let _reply = Comment {
        id: reply_id,
        session_id: SessionId::new(),
        frame: 0,
        text: text.to_string(),
        annotation_type: AnnotationType::General,
        author,
        status: CommentStatus::Open,
        priority: CommentPriority::Normal,
        parent_id: Some(parent_id),
        created_at: now,
        updated_at: now,
        resolved_at: None,
        resolved_by: None,
    };

    Ok(reply_id)
}

/// Get all replies to a comment.
///
/// # Arguments
///
/// * `parent_id` - ID of the parent comment
///
/// # Errors
///
/// Returns error if replies cannot be retrieved.
pub async fn get_replies(parent_id: CommentId) -> ReviewResult<Vec<Comment>> {
    // In a real implementation, this would query the database

    let _ = parent_id;
    Ok(Vec::new())
}

/// Count replies to a comment.
///
/// # Arguments
///
/// * `parent_id` - ID of the parent comment
///
/// # Errors
///
/// Returns error if count cannot be retrieved.
pub async fn count_replies(parent_id: CommentId) -> ReviewResult<usize> {
    let replies = get_replies(parent_id).await?;
    Ok(replies.len())
}

/// Check if a comment has replies.
///
/// # Arguments
///
/// * `comment_id` - ID of the comment
///
/// # Errors
///
/// Returns error if check fails.
pub async fn has_replies(comment_id: CommentId) -> ReviewResult<bool> {
    let count = count_replies(comment_id).await?;
    Ok(count > 0)
}

/// Delete all replies to a comment.
///
/// # Arguments
///
/// * `parent_id` - ID of the parent comment
///
/// # Errors
///
/// Returns error if deletion fails.
pub async fn delete_all_replies(parent_id: CommentId) -> ReviewResult<()> {
    // In a real implementation, this would delete all replies

    let _ = parent_id;
    Ok(())
}

/// Validate that a reply can be added.
///
/// # Errors
///
/// Returns error if validation fails.
pub fn validate_reply(parent_status: CommentStatus) -> ReviewResult<()> {
    if parent_status == CommentStatus::Archived {
        return Err(ReviewError::InvalidStateTransition(
            "Cannot reply to archived comment".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_reply() {
        let parent_id = CommentId::new();
        let result = add_reply(parent_id, "Test reply").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_reply_with_author() {
        let parent_id = CommentId::new();
        let author = User {
            id: "user-1".to_string(),
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            role: UserRole::Reviewer,
        };

        let result = add_reply_with_author(parent_id, "Test reply", author).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_replies() {
        let parent_id = CommentId::new();
        let result = get_replies(parent_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_count_replies() {
        let parent_id = CommentId::new();
        let result = count_replies(parent_id).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_reply() {
        assert!(validate_reply(CommentStatus::Open).is_ok());
        assert!(validate_reply(CommentStatus::Resolved).is_ok());
        assert!(validate_reply(CommentStatus::Archived).is_err());
    }
}
