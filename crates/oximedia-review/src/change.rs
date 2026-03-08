//! Change request management.

use crate::{error::ReviewResult, SessionId};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod priority;
pub mod request;
pub mod status;

pub use priority::ChangePriority;
pub use request::ChangeRequest;
pub use status::ChangeStatus;

/// Change request ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChangeRequestId(Uuid);

impl ChangeRequestId {
    /// Create a new change request ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ChangeRequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ChangeRequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Create a new change request.
///
/// # Errors
///
/// Returns error if creation fails.
pub async fn create_change_request(
    session_id: SessionId,
    title: String,
    description: String,
    priority: ChangePriority,
) -> ReviewResult<ChangeRequest> {
    Ok(ChangeRequest {
        id: ChangeRequestId::new(),
        session_id,
        title,
        description,
        priority,
        status: ChangeStatus::Pending,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
        assigned_to: None,
    })
}

/// List change requests for a session.
///
/// # Errors
///
/// Returns error if listing fails.
pub async fn list_change_requests(session_id: SessionId) -> ReviewResult<Vec<ChangeRequest>> {
    // In a real implementation, query database
    let _ = session_id;
    Ok(Vec::new())
}

/// Update change request status.
///
/// # Errors
///
/// Returns error if update fails.
pub async fn update_change_status(
    request_id: ChangeRequestId,
    status: ChangeStatus,
) -> ReviewResult<()> {
    // In a real implementation, update database
    let _ = (request_id, status);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_change_request() {
        let session_id = SessionId::new();
        let result = create_change_request(
            session_id,
            "Fix audio".to_string(),
            "Audio levels too low".to_string(),
            ChangePriority::High,
        )
        .await;

        assert!(result.is_ok());
        let request = result.expect("should succeed in test");
        assert_eq!(request.status, ChangeStatus::Pending);
    }

    #[tokio::test]
    async fn test_list_change_requests() {
        let session_id = SessionId::new();
        let result = list_change_requests(session_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_change_status() {
        let request_id = ChangeRequestId::new();
        let result = update_change_status(request_id, ChangeStatus::InProgress).await;
        assert!(result.is_ok());
    }
}
