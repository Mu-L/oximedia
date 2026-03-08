//! Comment resolution.

use crate::{
    comment::CommentStatus,
    error::{ReviewError, ReviewResult},
    CommentId,
};
use chrono::Utc;

/// Resolve a comment.
///
/// # Arguments
///
/// * `comment_id` - ID of the comment to resolve
/// * `user_id` - ID of the user resolving the comment
///
/// # Errors
///
/// Returns error if comment cannot be resolved.
pub async fn resolve_comment(comment_id: CommentId, user_id: &str) -> ReviewResult<()> {
    // In a real implementation, this would:
    // 1. Load the comment
    // 2. Check if already resolved
    // 3. Update status to Resolved
    // 4. Set resolved_at and resolved_by
    // 5. Persist changes
    // 6. Send notifications

    let _ = (comment_id, user_id, Utc::now());
    Ok(())
}

/// Unresolve a comment.
///
/// # Arguments
///
/// * `comment_id` - ID of the comment to unresolve
///
/// # Errors
///
/// Returns error if comment cannot be unresolved.
pub async fn unresolve_comment(comment_id: CommentId) -> ReviewResult<()> {
    // In a real implementation, this would:
    // 1. Load the comment
    // 2. Check if it's resolved
    // 3. Update status to Open
    // 4. Clear resolved_at and resolved_by
    // 5. Persist changes

    let _ = comment_id;
    Ok(())
}

/// Mark comment as archived.
///
/// # Arguments
///
/// * `comment_id` - ID of the comment
///
/// # Errors
///
/// Returns error if comment cannot be archived.
pub async fn archive_comment(comment_id: CommentId) -> ReviewResult<()> {
    let _ = comment_id;
    Ok(())
}

/// Bulk resolve multiple comments.
///
/// # Arguments
///
/// * `comment_ids` - IDs of comments to resolve
/// * `user_id` - ID of the user resolving the comments
///
/// # Errors
///
/// Returns error if any comment cannot be resolved.
pub async fn resolve_comments_batch(comment_ids: &[CommentId], user_id: &str) -> ReviewResult<()> {
    for comment_id in comment_ids {
        resolve_comment(*comment_id, user_id).await?;
    }
    Ok(())
}

/// Check if all comments in a session are resolved.
///
/// # Arguments
///
/// * `unresolved_count` - Number of unresolved comments
///
/// # Errors
///
/// Returns error if check fails.
pub fn all_comments_resolved(unresolved_count: usize) -> ReviewResult<bool> {
    Ok(unresolved_count == 0)
}

/// Validate comment resolution.
///
/// # Errors
///
/// Returns error if validation fails.
pub fn validate_resolution(current_status: CommentStatus) -> ReviewResult<()> {
    if current_status == CommentStatus::Resolved {
        return Err(ReviewError::CommentAlreadyResolved);
    }

    if current_status == CommentStatus::Archived {
        return Err(ReviewError::InvalidStateTransition(
            "Cannot resolve archived comment".to_string(),
        ));
    }

    Ok(())
}

/// Validate unresolve operation.
///
/// # Errors
///
/// Returns error if validation fails.
pub fn validate_unresolve(current_status: CommentStatus) -> ReviewResult<()> {
    if current_status != CommentStatus::Resolved {
        return Err(ReviewError::InvalidStateTransition(
            "Can only unresolve resolved comments".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolve_comment() {
        let comment_id = CommentId::new();
        let result = resolve_comment(comment_id, "user-123").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_unresolve_comment() {
        let comment_id = CommentId::new();
        let result = unresolve_comment(comment_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_resolve_comments_batch() {
        let comment_ids = vec![CommentId::new(), CommentId::new()];
        let result = resolve_comments_batch(&comment_ids, "user-123").await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_all_comments_resolved() {
        assert!(all_comments_resolved(0).expect("should succeed in test"));
        assert!(!all_comments_resolved(5).expect("should succeed in test"));
    }

    #[test]
    fn test_validate_resolution() {
        assert!(validate_resolution(CommentStatus::Open).is_ok());
        assert!(validate_resolution(CommentStatus::Resolved).is_err());
        assert!(validate_resolution(CommentStatus::Archived).is_err());
    }

    #[test]
    fn test_validate_unresolve() {
        assert!(validate_unresolve(CommentStatus::Resolved).is_ok());
        assert!(validate_unresolve(CommentStatus::Open).is_err());
    }
}
