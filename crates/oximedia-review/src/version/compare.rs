//! Version comparison.

use crate::{
    error::ReviewResult,
    version::{diff::VersionDiff, Version},
    VersionId,
};
use serde::{Deserialize, Serialize};

/// Comparison result between two versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionComparison {
    /// Version A (older).
    pub version_a: VersionId,
    /// Version B (newer).
    pub version_b: VersionId,
    /// List of differences.
    pub differences: Vec<VersionDiff>,
    /// Similarity score (0.0-1.0).
    pub similarity: f64,
    /// Total frames changed.
    pub frames_changed: usize,
    /// Metadata changes.
    pub metadata_changes: Vec<MetadataChange>,
}

/// Metadata change between versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataChange {
    /// Field name.
    pub field: String,
    /// Old value.
    pub old_value: String,
    /// New value.
    pub new_value: String,
}

/// Compare two versions.
///
/// # Errors
///
/// Returns error if comparison fails.
pub async fn compare_versions(
    version_a: &Version,
    version_b: &Version,
) -> ReviewResult<VersionComparison> {
    let mut metadata_changes = Vec::new();

    // Compare resolution
    if version_a.resolution != version_b.resolution {
        metadata_changes.push(MetadataChange {
            field: "resolution".to_string(),
            old_value: version_a.resolution_string(),
            new_value: version_b.resolution_string(),
        });
    }

    // Compare duration
    if version_a.duration_frames != version_b.duration_frames {
        metadata_changes.push(MetadataChange {
            field: "duration_frames".to_string(),
            old_value: version_a.duration_frames.to_string(),
            new_value: version_b.duration_frames.to_string(),
        });
    }

    // Compare frame rate
    if (version_a.frame_rate - version_b.frame_rate).abs() > 0.001 {
        metadata_changes.push(MetadataChange {
            field: "frame_rate".to_string(),
            old_value: version_a.frame_rate.to_string(),
            new_value: version_b.frame_rate.to_string(),
        });
    }

    // Calculate similarity (simplified)
    let similarity =
        if version_a.content_hash == version_b.content_hash && metadata_changes.is_empty() {
            1.0 // Same content and metadata
        } else if version_a.content_hash == version_b.content_hash {
            0.95 // Same content, different metadata
        } else if metadata_changes.is_empty() {
            0.90 // Different content, same metadata
        } else {
            0.80 // Different content and metadata
        };

    Ok(VersionComparison {
        version_a: version_a.id,
        version_b: version_b.id,
        differences: Vec::new(),
        similarity,
        frames_changed: 0,
        metadata_changes,
    })
}

/// Compare multiple versions.
///
/// # Errors
///
/// Returns error if comparison fails.
pub async fn compare_multiple(versions: &[Version]) -> ReviewResult<Vec<VersionComparison>> {
    let mut comparisons = Vec::new();

    for i in 0..versions.len().saturating_sub(1) {
        let comparison = compare_versions(&versions[i], &versions[i + 1]).await?;
        comparisons.push(comparison);
    }

    Ok(comparisons)
}

/// Find differences in frame range.
///
/// # Errors
///
/// Returns error if operation fails.
pub async fn find_frame_differences(
    _version_a: &Version,
    _version_b: &Version,
    _start_frame: i64,
    _end_frame: i64,
) -> ReviewResult<Vec<i64>> {
    // In a real implementation, this would:
    // 1. Load frames from both versions
    // 2. Compare frame by frame
    // 3. Identify changed frames

    Ok(Vec::new())
}

/// Calculate perceptual difference score.
///
/// # Errors
///
/// Returns error if calculation fails.
pub async fn calculate_perceptual_difference(
    _version_a: &Version,
    _version_b: &Version,
) -> ReviewResult<f64> {
    // In a real implementation, this would use SSIM or similar metrics
    Ok(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SessionId;
    use chrono::Utc;

    fn create_test_version(number: u32) -> Version {
        Version {
            id: VersionId::new(),
            session_id: SessionId::new(),
            number,
            label: format!("Version {}", number),
            description: None,
            content_url: String::new(),
            content_hash: "hash123".to_string(),
            file_size: 1000,
            duration_frames: 240,
            frame_rate: 24.0,
            resolution: (1920, 1080),
            created_by: "test".to_string(),
            created_at: Utc::now(),
            parent_id: None,
        }
    }

    #[tokio::test]
    async fn test_compare_versions_identical() {
        let version1 = create_test_version(1);
        let version2 = create_test_version(2);

        let comparison = compare_versions(&version1, &version2)
            .await
            .expect("should succeed in test");
        assert!((comparison.similarity - 1.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_compare_versions_different_resolution() {
        let version1 = create_test_version(1);
        let mut version2 = create_test_version(2);
        version2.resolution = (3840, 2160);

        let comparison = compare_versions(&version1, &version2)
            .await
            .expect("should succeed in test");
        assert!(!comparison.metadata_changes.is_empty());
        assert!(comparison.similarity < 1.0);
    }

    #[tokio::test]
    async fn test_compare_multiple() {
        let versions = vec![
            create_test_version(1),
            create_test_version(2),
            create_test_version(3),
        ];

        let comparisons = compare_multiple(&versions)
            .await
            .expect("should succeed in test");
        assert_eq!(comparisons.len(), 2);
    }
}
