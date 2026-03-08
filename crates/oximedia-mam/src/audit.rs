//! Audit logging and activity tracking
//!
//! Provides comprehensive audit logging for:
//! - User activity tracking
//! - Asset access logs
//! - Change history
//! - Compliance reporting
//! - Security monitoring

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::sync::Arc;
use uuid::Uuid;

use crate::database::Database;
use crate::Result;

/// Audit logger handles all audit logging
pub struct AuditLogger {
    db: Arc<Database>,
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditLog {
    pub id: Uuid,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub username: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub details: Option<serde_json::Value>,
    pub changes: Option<serde_json::Value>,
    pub success: bool,
    pub error_message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Audit action type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditAction {
    // Authentication actions
    /// User logged in
    Login,
    /// User logged out
    Logout,
    /// Login failed
    LoginFailed,
    /// Password changed
    PasswordChanged,

    // Asset actions
    /// Asset created
    AssetCreate,
    /// Asset read/viewed
    AssetRead,
    /// Asset updated
    AssetUpdate,
    /// Asset deleted
    AssetDelete,
    /// Asset downloaded
    AssetDownload,
    /// Asset shared
    AssetShare,

    // Collection actions
    /// Collection created
    CollectionCreate,
    /// Collection read
    CollectionRead,
    /// Collection updated
    CollectionUpdate,
    /// Collection deleted
    CollectionDelete,

    // Workflow actions
    /// Workflow created
    WorkflowCreate,
    /// Workflow updated
    WorkflowUpdate,
    /// Workflow approved
    WorkflowApprove,
    /// Workflow rejected
    WorkflowReject,

    // User management actions
    /// User created
    UserCreate,
    /// User updated
    UserUpdate,
    /// User deleted
    UserDelete,
    /// Role assigned
    RoleAssign,
    /// Permission granted
    PermissionGrant,
    /// Permission revoked
    PermissionRevoke,

    // Storage actions
    /// File uploaded
    FileUpload,
    /// File downloaded
    FileDownload,
    /// File deleted
    FileDelete,

    // System actions
    /// Configuration changed
    ConfigChange,
    /// System started
    SystemStart,
    /// System stopped
    SystemStop,

    // Custom action
    /// Custom action
    Custom,
}

impl AuditAction {
    /// Convert to string
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Login => "auth.login",
            Self::Logout => "auth.logout",
            Self::LoginFailed => "auth.login_failed",
            Self::PasswordChanged => "auth.password_changed",
            Self::AssetCreate => "asset.create",
            Self::AssetRead => "asset.read",
            Self::AssetUpdate => "asset.update",
            Self::AssetDelete => "asset.delete",
            Self::AssetDownload => "asset.download",
            Self::AssetShare => "asset.share",
            Self::CollectionCreate => "collection.create",
            Self::CollectionRead => "collection.read",
            Self::CollectionUpdate => "collection.update",
            Self::CollectionDelete => "collection.delete",
            Self::WorkflowCreate => "workflow.create",
            Self::WorkflowUpdate => "workflow.update",
            Self::WorkflowApprove => "workflow.approve",
            Self::WorkflowReject => "workflow.reject",
            Self::UserCreate => "user.create",
            Self::UserUpdate => "user.update",
            Self::UserDelete => "user.delete",
            Self::RoleAssign => "user.role_assign",
            Self::PermissionGrant => "permission.grant",
            Self::PermissionRevoke => "permission.revoke",
            Self::FileUpload => "file.upload",
            Self::FileDownload => "file.download",
            Self::FileDelete => "file.delete",
            Self::ConfigChange => "system.config_change",
            Self::SystemStart => "system.start",
            Self::SystemStop => "system.stop",
            Self::Custom => "custom",
        }
    }
}

/// Audit log filter
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditLogFilter {
    pub user_id: Option<Uuid>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub success: Option<bool>,
    pub ip_address: Option<String>,
}

/// Audit log request
#[derive(Debug, Clone)]
pub struct AuditLogRequest {
    pub action: AuditAction,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub username: Option<String>,
    pub ip_address: Option<IpAddr>,
    pub user_agent: Option<String>,
    pub details: Option<serde_json::Value>,
    pub changes: Option<serde_json::Value>,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Change tracking for audit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub field: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}

/// Audit statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditStatistics {
    pub total_events: i64,
    pub failed_events: i64,
    pub unique_users: i64,
    pub most_common_actions: Vec<ActionCount>,
    pub most_active_users: Vec<UserActivity>,
}

/// Action count for statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionCount {
    pub action: String,
    pub count: i64,
}

/// User activity for statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserActivity {
    pub user_id: Uuid,
    pub username: String,
    pub action_count: i64,
}

impl AuditLogger {
    /// Create a new audit logger
    #[must_use]
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Log an audit event
    ///
    /// # Errors
    ///
    /// Returns an error if logging fails
    pub async fn log(&self, req: AuditLogRequest) -> Result<Uuid> {
        let log_id = Uuid::new_v4();

        sqlx::query(
            "INSERT INTO audit_logs
             (id, action, resource_type, resource_id, user_id, username, ip_address, user_agent,
              details, changes, success, error_message, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW())",
        )
        .bind(log_id)
        .bind(req.action.as_str())
        .bind(&req.resource_type)
        .bind(req.resource_id)
        .bind(req.user_id)
        .bind(&req.username)
        .bind(req.ip_address.map(|ip| ip.to_string()))
        .bind(&req.user_agent)
        .bind(&req.details)
        .bind(&req.changes)
        .bind(req.success)
        .bind(&req.error_message)
        .execute(self.db.pool())
        .await?;

        Ok(log_id)
    }

    /// Log successful action
    ///
    /// # Errors
    ///
    /// Returns an error if logging fails
    pub async fn log_success(
        &self,
        action: AuditAction,
        user_id: Option<Uuid>,
        resource_type: Option<String>,
        resource_id: Option<Uuid>,
        details: Option<serde_json::Value>,
    ) -> Result<Uuid> {
        self.log(AuditLogRequest {
            action,
            resource_type,
            resource_id,
            user_id,
            username: None,
            ip_address: None,
            user_agent: None,
            details,
            changes: None,
            success: true,
            error_message: None,
        })
        .await
    }

    /// Log failed action
    ///
    /// # Errors
    ///
    /// Returns an error if logging fails
    pub async fn log_failure(
        &self,
        action: AuditAction,
        user_id: Option<Uuid>,
        resource_type: Option<String>,
        resource_id: Option<Uuid>,
        error: String,
    ) -> Result<Uuid> {
        self.log(AuditLogRequest {
            action,
            resource_type,
            resource_id,
            user_id,
            username: None,
            ip_address: None,
            user_agent: None,
            details: None,
            changes: None,
            success: false,
            error_message: Some(error),
        })
        .await
    }

    /// Log with changes
    ///
    /// # Errors
    ///
    /// Returns an error if logging fails
    pub async fn log_with_changes(
        &self,
        action: AuditAction,
        user_id: Option<Uuid>,
        resource_type: Option<String>,
        resource_id: Option<Uuid>,
        changes: Vec<Change>,
    ) -> Result<Uuid> {
        let changes_json = serde_json::to_value(changes)?;

        self.log(AuditLogRequest {
            action,
            resource_type,
            resource_id,
            user_id,
            username: None,
            ip_address: None,
            user_agent: None,
            details: None,
            changes: Some(changes_json),
            success: true,
            error_message: None,
        })
        .await
    }

    /// Get audit log by ID
    ///
    /// # Errors
    ///
    /// Returns an error if log not found
    pub async fn get_log(&self, log_id: Uuid) -> Result<AuditLog> {
        let log = sqlx::query_as::<_, AuditLog>("SELECT * FROM audit_logs WHERE id = $1")
            .bind(log_id)
            .fetch_one(self.db.pool())
            .await?;

        Ok(log)
    }

    /// Query audit logs with filters
    ///
    /// # Errors
    ///
    /// Returns an error if query fails
    pub async fn query_logs(
        &self,
        filter: AuditLogFilter,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditLog>> {
        let mut query = String::from("SELECT * FROM audit_logs WHERE 1=1");
        let mut bindings = Vec::new();
        let mut param_num = 1;

        if let Some(user_id) = filter.user_id {
            query.push_str(&format!(" AND user_id = ${param_num}"));
            bindings.push(user_id);
            param_num += 1;
        }

        if let Some(_action) = &filter.action {
            query.push_str(&format!(" AND action = ${param_num}"));
            param_num += 1;
        }

        if let Some(_resource_type) = &filter.resource_type {
            query.push_str(&format!(" AND resource_type = ${param_num}"));
            param_num += 1;
        }

        if let Some(resource_id) = filter.resource_id {
            query.push_str(&format!(" AND resource_id = ${param_num}"));
            bindings.push(resource_id);
            param_num += 1;
        }

        if let Some(_success) = filter.success {
            query.push_str(&format!(" AND success = ${param_num}"));
            param_num += 1;
        }

        if let Some(_start_time) = filter.start_time {
            query.push_str(&format!(" AND timestamp >= ${param_num}"));
            param_num += 1;
        }

        if let Some(_end_time) = filter.end_time {
            query.push_str(&format!(" AND timestamp <= ${param_num}"));
            param_num += 1;
        }

        query.push_str(&format!(
            " ORDER BY timestamp DESC LIMIT ${param_num} OFFSET ${}",
            param_num + 1
        ));

        let logs = sqlx::query_as::<_, AuditLog>(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(self.db.pool())
            .await?;

        Ok(logs)
    }

    /// Get logs for user
    ///
    /// # Errors
    ///
    /// Returns an error if query fails
    pub async fn get_user_logs(&self, user_id: Uuid, limit: i64) -> Result<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            "SELECT * FROM audit_logs WHERE user_id = $1 ORDER BY timestamp DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;

        Ok(logs)
    }

    /// Get logs for resource
    ///
    /// # Errors
    ///
    /// Returns an error if query fails
    pub async fn get_resource_logs(
        &self,
        resource_type: &str,
        resource_id: Uuid,
        limit: i64,
    ) -> Result<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            "SELECT * FROM audit_logs
             WHERE resource_type = $1 AND resource_id = $2
             ORDER BY timestamp DESC
             LIMIT $3",
        )
        .bind(resource_type)
        .bind(resource_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;

        Ok(logs)
    }

    /// Get audit statistics
    ///
    /// # Errors
    ///
    /// Returns an error if query fails
    pub async fn get_statistics(&self, days: i32) -> Result<AuditStatistics> {
        let since = Utc::now() - chrono::Duration::days(days as i64);

        let total_events: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM audit_logs WHERE timestamp >= $1")
                .bind(since)
                .fetch_one(self.db.pool())
                .await?;

        let failed_events: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_logs WHERE timestamp >= $1 AND success = false",
        )
        .bind(since)
        .fetch_one(self.db.pool())
        .await?;

        let unique_users: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT user_id) FROM audit_logs WHERE timestamp >= $1 AND user_id IS NOT NULL",
        )
        .bind(since)
        .fetch_one(self.db.pool())
        .await?;

        // Most common actions
        let action_counts = sqlx::query_as::<_, (String, i64)>(
            "SELECT action, COUNT(*) as count
             FROM audit_logs
             WHERE timestamp >= $1
             GROUP BY action
             ORDER BY count DESC
             LIMIT 10",
        )
        .bind(since)
        .fetch_all(self.db.pool())
        .await?;

        let most_common_actions: Vec<ActionCount> = action_counts
            .into_iter()
            .map(|(action, count)| ActionCount { action, count })
            .collect();

        // Most active users
        let user_activities = sqlx::query_as::<_, (Uuid, String, i64)>(
            "SELECT user_id, username, COUNT(*) as count
             FROM audit_logs
             WHERE timestamp >= $1 AND user_id IS NOT NULL
             GROUP BY user_id, username
             ORDER BY count DESC
             LIMIT 10",
        )
        .bind(since)
        .fetch_all(self.db.pool())
        .await?;

        let most_active_users: Vec<UserActivity> = user_activities
            .into_iter()
            .map(|(user_id, username, action_count)| UserActivity {
                user_id,
                username,
                action_count,
            })
            .collect();

        Ok(AuditStatistics {
            total_events,
            failed_events,
            unique_users,
            most_common_actions,
            most_active_users,
        })
    }

    /// Export audit logs to JSON
    ///
    /// # Errors
    ///
    /// Returns an error if export fails
    pub async fn export_logs(&self, filter: AuditLogFilter, limit: i64) -> Result<String> {
        let logs = self.query_logs(filter, limit, 0).await?;
        let json = serde_json::to_string_pretty(&logs)?;
        Ok(json)
    }

    /// Delete old audit logs (for retention policy)
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails
    pub async fn cleanup_old_logs(&self, days: i32) -> Result<u64> {
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);

        let result = sqlx::query("DELETE FROM audit_logs WHERE timestamp < $1")
            .bind(cutoff)
            .execute(self.db.pool())
            .await?;

        Ok(result.rows_affected())
    }

    /// Get failed login attempts
    ///
    /// # Errors
    ///
    /// Returns an error if query fails
    pub async fn get_failed_logins(&self, since: DateTime<Utc>) -> Result<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            "SELECT * FROM audit_logs
             WHERE action = 'auth.login_failed'
             AND timestamp >= $1
             ORDER BY timestamp DESC",
        )
        .bind(since)
        .fetch_all(self.db.pool())
        .await?;

        Ok(logs)
    }

    /// Get suspicious activity (multiple failed attempts from same IP)
    ///
    /// # Errors
    ///
    /// Returns an error if query fails
    pub async fn get_suspicious_activity(
        &self,
        threshold: i64,
        hours: i32,
    ) -> Result<Vec<(String, i64)>> {
        let since = Utc::now() - chrono::Duration::hours(hours as i64);

        let suspicious = sqlx::query_as::<_, (String, i64)>(
            "SELECT ip_address, COUNT(*) as count
             FROM audit_logs
             WHERE action = 'auth.login_failed'
             AND timestamp >= $1
             AND ip_address IS NOT NULL
             GROUP BY ip_address
             HAVING COUNT(*) >= $2
             ORDER BY count DESC",
        )
        .bind(since)
        .bind(threshold)
        .fetch_all(self.db.pool())
        .await?;

        Ok(suspicious)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_action_as_str() {
        assert_eq!(AuditAction::Login.as_str(), "auth.login");
        assert_eq!(AuditAction::AssetCreate.as_str(), "asset.create");
        assert_eq!(AuditAction::WorkflowApprove.as_str(), "workflow.approve");
    }

    #[test]
    fn test_change_serialization() {
        let change = Change {
            field: "title".to_string(),
            old_value: Some(serde_json::json!("Old Title")),
            new_value: Some(serde_json::json!("New Title")),
        };

        let json = serde_json::to_string(&change).expect("should succeed in test");
        let deserialized: Change = serde_json::from_str(&json).expect("should succeed in test");

        assert_eq!(deserialized.field, "title");
    }

    #[test]
    fn test_audit_log_filter() {
        let filter = AuditLogFilter {
            user_id: Some(Uuid::new_v4()),
            action: Some("asset.create".to_string()),
            resource_type: Some("asset".to_string()),
            resource_id: None,
            start_time: Some(Utc::now()),
            end_time: None,
            success: Some(true),
            ip_address: None,
        };

        assert_eq!(filter.action, Some("asset.create".to_string()));
        assert_eq!(filter.success, Some(true));
    }

    #[test]
    fn test_action_count() {
        let action_count = ActionCount {
            action: "asset.create".to_string(),
            count: 100,
        };

        assert_eq!(action_count.action, "asset.create");
        assert_eq!(action_count.count, 100);
    }
}
