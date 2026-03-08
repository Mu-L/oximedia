//! Compliance checking

use crate::{database::RightsDatabase, rights::RightsGrant, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Issue severity level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IssueSeverity {
    /// Low severity
    Low,
    /// Medium severity
    Medium,
    /// High severity
    High,
    /// Critical severity
    Critical,
}

/// Compliance issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceIssue {
    /// Issue type
    pub issue_type: String,
    /// Severity
    pub severity: IssueSeverity,
    /// Description
    pub description: String,
    /// Entity type
    pub entity_type: String,
    /// Entity ID
    pub entity_id: String,
}

impl ComplianceIssue {
    /// Create a new compliance issue
    pub fn new(
        issue_type: impl Into<String>,
        severity: IssueSeverity,
        description: impl Into<String>,
        entity_type: impl Into<String>,
        entity_id: impl Into<String>,
    ) -> Self {
        Self {
            issue_type: issue_type.into(),
            severity,
            description: description.into(),
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
        }
    }
}

/// Compliance checker
pub struct ComplianceChecker<'a> {
    db: &'a RightsDatabase,
}

impl<'a> ComplianceChecker<'a> {
    /// Create a new compliance checker
    pub fn new(db: &'a RightsDatabase) -> Self {
        Self { db }
    }

    /// Check compliance for an asset
    pub async fn check_asset(&self, asset_id: &str) -> Result<Vec<ComplianceIssue>> {
        let mut issues = Vec::new();

        // Check for active grants
        let grants = RightsGrant::list_for_asset(self.db, asset_id).await?;

        if grants.is_empty() {
            issues.push(ComplianceIssue::new(
                "no_grants",
                IssueSeverity::High,
                "No rights grants found for asset",
                "asset",
                asset_id,
            ));
        }

        // Check for expired grants
        for grant in &grants {
            if grant.is_expired() {
                issues.push(ComplianceIssue::new(
                    "expired_grant",
                    IssueSeverity::Critical,
                    format!("Grant {} has expired", grant.id),
                    "grant",
                    &grant.id,
                ));
            }
        }

        // Check for expiring soon grants (within 30 days)
        let now = Utc::now();
        let threshold = now + chrono::Duration::days(30);

        for grant in &grants {
            if let Some(end_date) = grant.end_date {
                if end_date <= threshold && end_date > now {
                    issues.push(ComplianceIssue::new(
                        "expiring_soon",
                        IssueSeverity::Medium,
                        format!("Grant {} expires soon", grant.id),
                        "grant",
                        &grant.id,
                    ));
                }
            }
        }

        Ok(issues)
    }

    /// Check all compliance issues
    pub async fn check_all(&self) -> Result<Vec<ComplianceIssue>> {
        let mut all_issues = Vec::new();

        // Get all assets
        let assets = crate::rights::Asset::list(self.db).await?;

        for asset in assets {
            let issues = self.check_asset(&asset.id).await?;
            all_issues.extend(issues);
        }

        Ok(all_issues)
    }

    /// Get critical issues only
    pub async fn get_critical_issues(&self) -> Result<Vec<ComplianceIssue>> {
        let all_issues = self.check_all().await?;
        Ok(all_issues
            .into_iter()
            .filter(|issue| matches!(issue.severity, IssueSeverity::Critical))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rights::Asset;

    #[tokio::test]
    async fn test_compliance_check() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = format!("sqlite://{}/test.db", temp_dir.path().display());
        let db = RightsDatabase::new(&db_path).await.unwrap();

        let asset = Asset::new("Test Asset", crate::rights::AssetType::Video);
        let asset_id = asset.id.clone();
        asset.save(&db).await.unwrap();

        let checker = ComplianceChecker::new(&db);
        let issues = checker.check_asset(&asset_id).await.unwrap();

        // Should have at least one issue (no grants)
        assert!(!issues.is_empty());
    }
}
