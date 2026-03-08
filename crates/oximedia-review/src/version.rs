//! Version management and comparison.

use crate::{error::ReviewResult, SessionId, VersionId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub mod compare;
pub mod diff;
pub mod timeline;

pub use compare::compare_versions;
pub use diff::{DiffType, VersionDiff};
pub use timeline::VersionTimeline;

/// Content version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    /// Version ID.
    pub id: VersionId,
    /// Session ID.
    pub session_id: SessionId,
    /// Version number (sequential).
    pub number: u32,
    /// Version label.
    pub label: String,
    /// Description of changes.
    pub description: Option<String>,
    /// Content URL or path.
    pub content_url: String,
    /// Content hash (for integrity).
    pub content_hash: String,
    /// File size in bytes.
    pub file_size: u64,
    /// Duration in frames.
    pub duration_frames: i64,
    /// Frame rate.
    pub frame_rate: f64,
    /// Resolution (width, height).
    pub resolution: (u32, u32),
    /// Creator user ID.
    pub created_by: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Parent version ID (if any).
    pub parent_id: Option<VersionId>,
}

impl Version {
    /// Check if this is the initial version.
    #[must_use]
    pub fn is_initial(&self) -> bool {
        self.parent_id.is_none() && self.number == 1
    }

    /// Get duration in seconds.
    #[must_use]
    pub fn duration_seconds(&self) -> f64 {
        self.duration_frames as f64 / self.frame_rate
    }

    /// Get formatted resolution string.
    #[must_use]
    pub fn resolution_string(&self) -> String {
        format!("{}x{}", self.resolution.0, self.resolution.1)
    }
}

/// Create a new version.
///
/// # Errors
///
/// Returns error if version creation fails.
pub async fn create_version(
    session_id: SessionId,
    label: String,
    content_url: String,
) -> ReviewResult<Version> {
    let version = Version {
        id: VersionId::new(),
        session_id,
        number: 1,
        label,
        description: None,
        content_url,
        content_hash: String::new(),
        file_size: 0,
        duration_frames: 0,
        frame_rate: 24.0,
        resolution: (1920, 1080),
        created_by: "system".to_string(),
        created_at: Utc::now(),
        parent_id: None,
    };

    Ok(version)
}

/// Get version by ID.
///
/// # Errors
///
/// Returns error if version not found.
pub async fn get_version(version_id: VersionId) -> ReviewResult<Version> {
    // In a real implementation, this would load from database
    let _ = version_id;
    Err(crate::error::ReviewError::VersionNotFound(
        version_id.to_string(),
    ))
}

/// List all versions for a session.
///
/// # Errors
///
/// Returns error if listing fails.
pub async fn list_versions(session_id: SessionId) -> ReviewResult<Vec<Version>> {
    // In a real implementation, this would query database
    let _ = session_id;
    Ok(Vec::new())
}

/// Delete a version.
///
/// # Errors
///
/// Returns error if deletion fails.
pub async fn delete_version(version_id: VersionId) -> ReviewResult<()> {
    let _ = version_id;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_version() {
        let session_id = SessionId::new();
        let result = create_version(
            session_id,
            "Version 1".to_string(),
            "http://example.com/video.mp4".to_string(),
        )
        .await;

        assert!(result.is_ok());
        let version = result.expect("should succeed in test");
        assert_eq!(version.number, 1);
        assert!(version.is_initial());
    }

    #[test]
    fn test_version_duration_seconds() {
        let version = Version {
            id: VersionId::new(),
            session_id: SessionId::new(),
            number: 1,
            label: "Test".to_string(),
            description: None,
            content_url: String::new(),
            content_hash: String::new(),
            file_size: 0,
            duration_frames: 240,
            frame_rate: 24.0,
            resolution: (1920, 1080),
            created_by: "test".to_string(),
            created_at: Utc::now(),
            parent_id: None,
        };

        assert!((version.duration_seconds() - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_version_resolution_string() {
        let version = Version {
            id: VersionId::new(),
            session_id: SessionId::new(),
            number: 1,
            label: "Test".to_string(),
            description: None,
            content_url: String::new(),
            content_hash: String::new(),
            file_size: 0,
            duration_frames: 0,
            frame_rate: 24.0,
            resolution: (3840, 2160),
            created_by: "test".to_string(),
            created_at: Utc::now(),
            parent_id: None,
        };

        assert_eq!(version.resolution_string(), "3840x2160");
    }
}
