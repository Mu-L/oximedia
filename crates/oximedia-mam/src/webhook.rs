//! Webhook and event notification system
//!
//! Provides event-driven notifications for:
//! - Asset lifecycle events
//! - Workflow status changes
//! - Ingest completion
//! - Proxy generation completion
//! - Custom events
//! - HTTP webhooks
//! - Email notifications

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::database::Database;
use crate::{MamError, Result};

/// Webhook manager handles event notifications
pub struct WebhookManager {
    db: Arc<Database>,
    /// Event channel for internal event distribution
    event_tx: mpsc::UnboundedSender<Event>,
    /// Active webhook subscriptions
    webhooks: Arc<RwLock<HashMap<Uuid, Webhook>>>,
}

/// Webhook subscription
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Webhook {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub event_types: Vec<String>,
    pub secret: Option<String>,
    pub is_active: bool,
    pub retry_count: i32,
    pub timeout_seconds: i32,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Event type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    // Asset events
    /// Asset created
    AssetCreated,
    /// Asset updated
    AssetUpdated,
    /// Asset deleted
    AssetDeleted,
    /// Asset status changed
    AssetStatusChanged,

    // Ingest events
    /// Ingest started
    IngestStarted,
    /// Ingest completed
    IngestCompleted,
    /// Ingest failed
    IngestFailed,

    // Proxy events
    /// Proxy generation started
    ProxyStarted,
    /// Proxy generation completed
    ProxyCompleted,
    /// Proxy generation failed
    ProxyFailed,

    // Workflow events
    /// Workflow created
    WorkflowCreated,
    /// Workflow updated
    WorkflowUpdated,
    /// Workflow completed
    WorkflowCompleted,
    /// Workflow approved
    WorkflowApproved,
    /// Workflow rejected
    WorkflowRejected,

    // Collection events
    /// Collection created
    CollectionCreated,
    /// Collection updated
    CollectionUpdated,
    /// Collection deleted
    CollectionDeleted,

    // User events
    /// User logged in
    UserLoggedIn,
    /// User created
    UserCreated,
    /// User updated
    UserUpdated,

    // Storage events
    /// File uploaded
    FileUploaded,
    /// File deleted
    FileDeleted,
    /// Storage tier changed
    StorageTierChanged,

    // Custom event
    /// Custom event
    Custom,
}

impl EventType {
    /// Convert to string
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::AssetCreated => "asset.created",
            Self::AssetUpdated => "asset.updated",
            Self::AssetDeleted => "asset.deleted",
            Self::AssetStatusChanged => "asset.status_changed",
            Self::IngestStarted => "ingest.started",
            Self::IngestCompleted => "ingest.completed",
            Self::IngestFailed => "ingest.failed",
            Self::ProxyStarted => "proxy.started",
            Self::ProxyCompleted => "proxy.completed",
            Self::ProxyFailed => "proxy.failed",
            Self::WorkflowCreated => "workflow.created",
            Self::WorkflowUpdated => "workflow.updated",
            Self::WorkflowCompleted => "workflow.completed",
            Self::WorkflowApproved => "workflow.approved",
            Self::WorkflowRejected => "workflow.rejected",
            Self::CollectionCreated => "collection.created",
            Self::CollectionUpdated => "collection.updated",
            Self::CollectionDeleted => "collection.deleted",
            Self::UserLoggedIn => "user.logged_in",
            Self::UserCreated => "user.created",
            Self::UserUpdated => "user.updated",
            Self::FileUploaded => "file.uploaded",
            Self::FileDeleted => "file.deleted",
            Self::StorageTierChanged => "storage.tier_changed",
            Self::Custom => "custom",
        }
    }
}

impl std::str::FromStr for EventType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "asset.created" => Ok(Self::AssetCreated),
            "asset.updated" => Ok(Self::AssetUpdated),
            "asset.deleted" => Ok(Self::AssetDeleted),
            "asset.status_changed" => Ok(Self::AssetStatusChanged),
            "ingest.started" => Ok(Self::IngestStarted),
            "ingest.completed" => Ok(Self::IngestCompleted),
            "ingest.failed" => Ok(Self::IngestFailed),
            "proxy.started" => Ok(Self::ProxyStarted),
            "proxy.completed" => Ok(Self::ProxyCompleted),
            "proxy.failed" => Ok(Self::ProxyFailed),
            "workflow.created" => Ok(Self::WorkflowCreated),
            "workflow.updated" => Ok(Self::WorkflowUpdated),
            "workflow.completed" => Ok(Self::WorkflowCompleted),
            "workflow.approved" => Ok(Self::WorkflowApproved),
            "workflow.rejected" => Ok(Self::WorkflowRejected),
            "collection.created" => Ok(Self::CollectionCreated),
            "collection.updated" => Ok(Self::CollectionUpdated),
            "collection.deleted" => Ok(Self::CollectionDeleted),
            "user.logged_in" => Ok(Self::UserLoggedIn),
            "user.created" => Ok(Self::UserCreated),
            "user.updated" => Ok(Self::UserUpdated),
            "file.uploaded" => Ok(Self::FileUploaded),
            "file.deleted" => Ok(Self::FileDeleted),
            "storage.tier_changed" => Ok(Self::StorageTierChanged),
            "custom" => Ok(Self::Custom),
            _ => Err(format!("Invalid event type: {s}")),
        }
    }
}

/// Event data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub event_type: EventType,
    pub resource_id: Option<Uuid>,
    pub resource_type: Option<String>,
    pub user_id: Option<Uuid>,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

/// Webhook delivery attempt
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WebhookDelivery {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub event_id: Uuid,
    pub status: String,
    pub response_status: Option<i32>,
    pub response_body: Option<String>,
    pub error_message: Option<String>,
    pub attempt: i32,
    pub delivered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Webhook delivery status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryStatus {
    /// Pending delivery
    Pending,
    /// Successfully delivered
    Delivered,
    /// Failed to deliver
    Failed,
    /// Retrying delivery
    Retrying,
}

impl DeliveryStatus {
    /// Convert to string
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Delivered => "delivered",
            Self::Failed => "failed",
            Self::Retrying => "retrying",
        }
    }
}

/// Webhook creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWebhookRequest {
    pub name: String,
    pub url: String,
    pub event_types: Vec<String>,
    pub secret: Option<String>,
    pub timeout_seconds: Option<i32>,
}

impl WebhookManager {
    /// Create a new webhook manager
    #[must_use]
    pub fn new(db: Arc<Database>) -> Self {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let webhooks = Arc::new(RwLock::new(HashMap::new()));

        // Spawn event processor
        let webhooks_clone = Arc::clone(&webhooks);
        let db_clone = Arc::clone(&db);
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                Self::process_event_internal(
                    event,
                    Arc::clone(&webhooks_clone),
                    Arc::clone(&db_clone),
                )
                .await;
            }
        });

        Self {
            db,
            event_tx,
            webhooks,
        }
    }

    /// Emit an event
    ///
    /// # Errors
    ///
    /// Returns an error if event emission fails
    pub fn emit(&self, event: Event) -> Result<()> {
        self.event_tx
            .send(event)
            .map_err(|e| MamError::Internal(format!("Failed to emit event: {e}")))?;
        Ok(())
    }

    /// Process event (internal)
    async fn process_event_internal(
        event: Event,
        webhooks: Arc<RwLock<HashMap<Uuid, Webhook>>>,
        db: Arc<Database>,
    ) {
        // Store event in database
        if let Err(e) = Self::store_event(&event, &db).await {
            tracing::error!("Failed to store event: {}", e);
            return;
        }

        // Find matching webhooks
        let webhooks_map = webhooks.read().await;
        let matching_webhooks: Vec<Webhook> = webhooks_map
            .values()
            .filter(|w| {
                w.is_active
                    && w.event_types
                        .contains(&event.event_type.as_str().to_string())
            })
            .cloned()
            .collect();

        drop(webhooks_map);

        // Deliver to each webhook
        for webhook in matching_webhooks {
            tokio::spawn(Self::deliver_webhook(event.clone(), webhook, db.clone()));
        }
    }

    /// Store event in database
    async fn store_event(event: &Event, db: &Database) -> Result<()> {
        sqlx::query(
            "INSERT INTO events
             (id, event_type, resource_id, resource_type, user_id, data, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(event.id)
        .bind(event.event_type.as_str())
        .bind(event.resource_id)
        .bind(&event.resource_type)
        .bind(event.user_id)
        .bind(&event.data)
        .bind(event.timestamp)
        .execute(db.pool())
        .await?;

        Ok(())
    }

    /// Deliver webhook
    async fn deliver_webhook(event: Event, webhook: Webhook, db: Arc<Database>) {
        let delivery_id = Uuid::new_v4();
        let mut attempt = 1;
        let max_retries = webhook.retry_count;

        loop {
            // Create delivery record
            if let Err(e) = sqlx::query(
                "INSERT INTO webhook_deliveries
                 (id, webhook_id, event_id, status, attempt, created_at)
                 VALUES ($1, $2, $3, 'pending', $4, NOW())",
            )
            .bind(delivery_id)
            .bind(webhook.id)
            .bind(event.id)
            .bind(attempt)
            .execute(db.pool())
            .await
            {
                tracing::error!("Failed to create delivery record: {}", e);
                return;
            }

            // Build webhook payload
            let payload = serde_json::json!({
                "event_id": event.id,
                "event_type": event.event_type.as_str(),
                "resource_id": event.resource_id,
                "resource_type": event.resource_type,
                "user_id": event.user_id,
                "data": event.data,
                "timestamp": event.timestamp,
            });

            // Send HTTP request
            let client = reqwest::Client::new();
            let mut request =
                client
                    .post(&webhook.url)
                    .json(&payload)
                    .timeout(std::time::Duration::from_secs(
                        webhook.timeout_seconds as u64,
                    ));

            // Add signature if secret is set
            if let Some(secret) = &webhook.secret {
                let signature = Self::calculate_signature(&payload, secret);
                request = request.header("X-Webhook-Signature", signature);
            }

            match request.send().await {
                Ok(response) => {
                    let status = response.status().as_u16() as i32;
                    let body = response.text().await.ok();

                    if (200..300).contains(&(status as u16)) {
                        // Success
                        let _ = sqlx::query(
                            "UPDATE webhook_deliveries
                             SET status = 'delivered', response_status = $2, response_body = $3, delivered_at = NOW()
                             WHERE id = $1",
                        )
                        .bind(delivery_id)
                        .bind(status)
                        .bind(body)
                        .execute(db.pool())
                        .await;

                        return;
                    } else {
                        // HTTP error
                        let _ = sqlx::query(
                            "UPDATE webhook_deliveries
                             SET status = 'failed', response_status = $2, response_body = $3, error_message = $4
                             WHERE id = $1",
                        )
                        .bind(delivery_id)
                        .bind(status)
                        .bind(&body)
                        .bind(format!("HTTP error: {status}"))
                        .execute(db.pool())
                        .await;
                    }
                }
                Err(e) => {
                    // Network error
                    let _ = sqlx::query(
                        "UPDATE webhook_deliveries
                         SET status = 'failed', error_message = $2
                         WHERE id = $1",
                    )
                    .bind(delivery_id)
                    .bind(e.to_string())
                    .execute(db.pool())
                    .await;
                }
            }

            // Retry logic
            attempt += 1;
            if attempt > max_retries {
                tracing::error!(
                    "Webhook delivery failed after {} attempts: {}",
                    max_retries,
                    webhook.url
                );
                return;
            }

            // Exponential backoff
            let delay = std::time::Duration::from_secs(2_u64.pow((attempt - 2) as u32));
            tokio::time::sleep(delay).await;
        }
    }

    /// Calculate webhook signature
    fn calculate_signature(payload: &serde_json::Value, secret: &str) -> String {
        use sha2::{Digest, Sha256};

        let payload_str = serde_json::to_string(payload).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(secret.as_bytes());
        hasher.update(payload_str.as_bytes());
        let result = hasher.finalize();
        format!("{result:x}")
    }

    /// Create a webhook subscription
    ///
    /// # Errors
    ///
    /// Returns an error if creation fails
    pub async fn create_webhook(
        &self,
        req: CreateWebhookRequest,
        created_by: Option<Uuid>,
    ) -> Result<Webhook> {
        let webhook = sqlx::query_as::<_, Webhook>(
            "INSERT INTO webhooks
             (id, name, url, event_types, secret, is_active, retry_count, timeout_seconds, created_by, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, true, 3, $6, $7, NOW(), NOW())
             RETURNING *",
        )
        .bind(Uuid::new_v4())
        .bind(&req.name)
        .bind(&req.url)
        .bind(&req.event_types)
        .bind(&req.secret)
        .bind(req.timeout_seconds.unwrap_or(30))
        .bind(created_by)
        .fetch_one(self.db.pool())
        .await?;

        // Add to active webhooks
        self.webhooks
            .write()
            .await
            .insert(webhook.id, webhook.clone());

        Ok(webhook)
    }

    /// Get webhook by ID
    ///
    /// # Errors
    ///
    /// Returns an error if webhook not found
    pub async fn get_webhook(&self, webhook_id: Uuid) -> Result<Webhook> {
        let webhook = sqlx::query_as::<_, Webhook>("SELECT * FROM webhooks WHERE id = $1")
            .bind(webhook_id)
            .fetch_one(self.db.pool())
            .await?;

        Ok(webhook)
    }

    /// List all webhooks
    ///
    /// # Errors
    ///
    /// Returns an error if query fails
    pub async fn list_webhooks(&self) -> Result<Vec<Webhook>> {
        let webhooks =
            sqlx::query_as::<_, Webhook>("SELECT * FROM webhooks ORDER BY created_at DESC")
                .fetch_all(self.db.pool())
                .await?;

        Ok(webhooks)
    }

    /// Delete webhook
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails
    pub async fn delete_webhook(&self, webhook_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM webhooks WHERE id = $1")
            .bind(webhook_id)
            .execute(self.db.pool())
            .await?;

        self.webhooks.write().await.remove(&webhook_id);

        Ok(())
    }

    /// Get webhook deliveries
    ///
    /// # Errors
    ///
    /// Returns an error if query fails
    pub async fn get_webhook_deliveries(
        &self,
        webhook_id: Uuid,
        limit: i64,
    ) -> Result<Vec<WebhookDelivery>> {
        let deliveries = sqlx::query_as::<_, WebhookDelivery>(
            "SELECT * FROM webhook_deliveries WHERE webhook_id = $1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(webhook_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;

        Ok(deliveries)
    }

    /// Load all webhooks from database
    ///
    /// # Errors
    ///
    /// Returns an error if query fails
    pub async fn load_webhooks(&self) -> Result<()> {
        let webhooks = self.list_webhooks().await?;

        let mut webhooks_map = self.webhooks.write().await;
        webhooks_map.clear();

        for webhook in webhooks {
            webhooks_map.insert(webhook.id, webhook);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_as_str() {
        assert_eq!(EventType::AssetCreated.as_str(), "asset.created");
        assert_eq!(EventType::AssetUpdated.as_str(), "asset.updated");
        assert_eq!(EventType::ProxyCompleted.as_str(), "proxy.completed");
    }

    #[test]
    fn test_event_type_from_str() {
        use std::str::FromStr;
        assert_eq!(
            EventType::from_str("asset.created").ok(),
            Some(EventType::AssetCreated)
        );
        assert_eq!(
            EventType::from_str("proxy.completed").ok(),
            Some(EventType::ProxyCompleted)
        );
        assert!(EventType::from_str("invalid").is_err());
    }

    #[test]
    fn test_delivery_status_as_str() {
        assert_eq!(DeliveryStatus::Pending.as_str(), "pending");
        assert_eq!(DeliveryStatus::Delivered.as_str(), "delivered");
        assert_eq!(DeliveryStatus::Failed.as_str(), "failed");
    }

    #[test]
    fn test_event_serialization() {
        let event = Event {
            id: Uuid::new_v4(),
            event_type: EventType::AssetCreated,
            resource_id: Some(Uuid::new_v4()),
            resource_type: Some("asset".to_string()),
            user_id: None,
            data: serde_json::json!({"test": "data"}),
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&event).expect("should succeed in test");
        let deserialized: Event = serde_json::from_str(&json).expect("should succeed in test");

        assert_eq!(deserialized.event_type, EventType::AssetCreated);
    }

    #[test]
    fn test_create_webhook_request() {
        let req = CreateWebhookRequest {
            name: "Test Webhook".to_string(),
            url: "https://example.com/webhook".to_string(),
            event_types: vec!["asset.created".to_string(), "asset.updated".to_string()],
            secret: Some("secret123".to_string()),
            timeout_seconds: Some(30),
        };

        assert_eq!(req.name, "Test Webhook");
        assert_eq!(req.event_types.len(), 2);
    }
}
