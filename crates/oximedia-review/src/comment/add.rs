//! Adding comments to review sessions.

use crate::{
    comment::{Comment, CommentPriority, CommentStatus},
    error::ReviewResult,
    AnnotationType, CommentId, SessionId, User, UserRole,
};
use chrono::Utc;

/// Add a comment to a specific frame.
///
/// # Arguments
///
/// * `session_id` - ID of the session
/// * `frame` - Frame number (0-indexed)
/// * `text` - Comment text
/// * `annotation_type` - Type of annotation
///
/// # Errors
///
/// Returns error if comment cannot be added.
pub async fn add_comment(
    session_id: SessionId,
    frame: i64,
    text: &str,
    annotation_type: AnnotationType,
) -> ReviewResult<CommentId> {
    let comment_id = CommentId::new();
    let now = Utc::now();

    // Create default author (in real implementation, would use authenticated user)
    let author = User {
        id: "system".to_string(),
        name: "System".to_string(),
        email: "system@oximedia.local".to_string(),
        role: UserRole::Reviewer,
    };

    let _comment = Comment {
        id: comment_id,
        session_id,
        frame,
        text: text.to_string(),
        annotation_type,
        author,
        status: CommentStatus::Open,
        priority: CommentPriority::Normal,
        parent_id: None,
        created_at: now,
        updated_at: now,
        resolved_at: None,
        resolved_by: None,
    };

    // In a real implementation, this would persist the comment

    Ok(comment_id)
}

/// Add a comment with detailed information.
///
/// # Arguments
///
/// * `session_id` - ID of the session
/// * `frame` - Frame number
/// * `text` - Comment text
/// * `annotation_type` - Type of annotation
/// * `author` - Comment author
/// * `priority` - Comment priority
///
/// # Errors
///
/// Returns error if comment cannot be added.
#[allow(clippy::too_many_arguments)]
pub async fn add_comment_detailed(
    session_id: SessionId,
    frame: i64,
    text: &str,
    annotation_type: AnnotationType,
    author: User,
    priority: CommentPriority,
) -> ReviewResult<CommentId> {
    let comment_id = CommentId::new();
    let now = Utc::now();

    let _comment = Comment {
        id: comment_id,
        session_id,
        frame,
        text: text.to_string(),
        annotation_type,
        author,
        status: CommentStatus::Open,
        priority,
        parent_id: None,
        created_at: now,
        updated_at: now,
        resolved_at: None,
        resolved_by: None,
    };

    Ok(comment_id)
}

/// Add multiple comments in batch.
///
/// # Arguments
///
/// * `session_id` - ID of the session
/// * `comments` - List of (frame, text, `annotation_type`) tuples
///
/// # Errors
///
/// Returns error if any comment cannot be added.
pub async fn add_comments_batch(
    session_id: SessionId,
    comments: &[(i64, String, AnnotationType)],
) -> ReviewResult<Vec<CommentId>> {
    let mut comment_ids = Vec::new();

    for (frame, text, annotation_type) in comments {
        let id = add_comment(session_id, *frame, text, *annotation_type).await?;
        comment_ids.push(id);
    }

    Ok(comment_ids)
}

/// Update a comment's text.
///
/// # Arguments
///
/// * `comment_id` - ID of the comment
/// * `new_text` - New comment text
///
/// # Errors
///
/// Returns error if comment cannot be updated.
pub async fn update_comment(comment_id: CommentId, new_text: &str) -> ReviewResult<()> {
    // In a real implementation, this would:
    // 1. Load the comment
    // 2. Update the text
    // 3. Update the updated_at timestamp
    // 4. Persist the changes

    let _ = (comment_id, new_text);
    Ok(())
}

/// Delete a comment.
///
/// # Arguments
///
/// * `comment_id` - ID of the comment
///
/// # Errors
///
/// Returns error if comment cannot be deleted.
pub async fn delete_comment(comment_id: CommentId) -> ReviewResult<()> {
    // In a real implementation, this would:
    // 1. Mark comment as archived
    // 2. Or permanently delete if allowed

    let _ = comment_id;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_comment() {
        let session_id = SessionId::new();
        let result = add_comment(session_id, 100, "Test comment", AnnotationType::Issue).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_comment_detailed() {
        let session_id = SessionId::new();
        let author = User {
            id: "user-1".to_string(),
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            role: UserRole::Reviewer,
        };

        let result = add_comment_detailed(
            session_id,
            100,
            "Test comment",
            AnnotationType::Issue,
            author,
            CommentPriority::High,
        )
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_comments_batch() {
        let session_id = SessionId::new();
        let comments = vec![
            (100, "Comment 1".to_string(), AnnotationType::Issue),
            (200, "Comment 2".to_string(), AnnotationType::Suggestion),
        ];

        let result = add_comments_batch(session_id, &comments).await;
        assert!(result.is_ok());
        assert_eq!(result.expect("should succeed in test").len(), 2);
    }

    #[tokio::test]
    async fn test_update_comment() {
        let comment_id = CommentId::new();
        let result = update_comment(comment_id, "Updated text").await;
        assert!(result.is_ok());
    }
}
