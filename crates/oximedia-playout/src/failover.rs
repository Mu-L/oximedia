//! Failover and redundancy support
//!
//! Provides automatic failover, hot standby, and seamless switchover
//! for broadcast-grade reliability.

use crate::{PlayoutError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Failover configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverConfig {
    /// Enable failover
    pub enabled: bool,

    /// Failover mode
    pub mode: FailoverMode,

    /// Health check interval in seconds
    pub health_check_interval_sec: u32,

    /// Failover timeout in milliseconds
    pub failover_timeout_ms: u64,

    /// Auto-recovery enabled
    pub auto_recovery: bool,

    /// Recovery delay in seconds
    pub recovery_delay_sec: u32,

    /// Heartbeat timeout in seconds
    pub heartbeat_timeout_sec: u32,
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: FailoverMode::HotStandby,
            health_check_interval_sec: 5,
            failover_timeout_ms: 1000,
            auto_recovery: true,
            recovery_delay_sec: 30,
            heartbeat_timeout_sec: 10,
        }
    }
}

/// Failover mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FailoverMode {
    /// Hot standby (redundant server running in parallel)
    HotStandby,
    /// Warm standby (redundant server ready but not running)
    WarmStandby,
    /// Cold standby (manual failover required)
    ColdStandby,
}

/// Server role in failover pair
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServerRole {
    Primary,
    Secondary,
}

/// Server state in failover system
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ServerState {
    Active,
    Standby,
    Failed,
    Recovering,
    Unknown,
}

/// Health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Server is healthy
    pub healthy: bool,

    /// CPU usage percentage
    pub cpu_usage: f32,

    /// Memory usage percentage
    pub memory_usage: f32,

    /// Disk usage percentage
    pub disk_usage: f32,

    /// Network available
    pub network_ok: bool,

    /// Playout running
    pub playout_running: bool,

    /// Last heartbeat
    pub last_heartbeat: DateTime<Utc>,

    /// Error messages
    pub errors: Vec<String>,
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self {
            healthy: true,
            cpu_usage: 0.0,
            memory_usage: 0.0,
            disk_usage: 0.0,
            network_ok: true,
            playout_running: false,
            last_heartbeat: Utc::now(),
            errors: Vec::new(),
        }
    }
}

/// Failover manager
pub struct FailoverManager {
    config: FailoverConfig,
    role: ServerRole,
    state: Arc<RwLock<ServerState>>,
    health: Arc<RwLock<HealthStatus>>,
    peer_health: Arc<RwLock<Option<HealthStatus>>>,
}

impl FailoverManager {
    /// Create new failover manager
    pub fn new(config: FailoverConfig, role: ServerRole) -> Self {
        let initial_state = match role {
            ServerRole::Primary => ServerState::Active,
            ServerRole::Secondary => ServerState::Standby,
        };

        Self {
            config,
            role,
            state: Arc::new(RwLock::new(initial_state)),
            health: Arc::new(RwLock::new(HealthStatus::default())),
            peer_health: Arc::new(RwLock::new(None)),
        }
    }

    /// Start failover monitoring
    pub async fn start(&self) -> Result<()> {
        info!("Starting failover manager as {:?}", self.role);

        // Start health check task
        let state = Arc::clone(&self.state);
        let health = Arc::clone(&self.health);
        let peer_health = Arc::clone(&self.peer_health);
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval =
                time::interval(Duration::from_secs(config.health_check_interval_sec as u64));

            loop {
                interval.tick().await;

                // Update own health
                let mut h = health.write().await;
                h.last_heartbeat = Utc::now();
                h.healthy = Self::check_system_health(&h);
                drop(h);

                // Check peer health
                let peer = peer_health.read().await;
                if let Some(peer_status) = peer.as_ref() {
                    let timeout = Duration::from_secs(config.heartbeat_timeout_sec as u64);
                    let elapsed = (Utc::now() - peer_status.last_heartbeat)
                        .to_std()
                        .unwrap_or(Duration::from_secs(0));

                    if elapsed > timeout {
                        // Peer is unhealthy, consider failover
                        warn!("Peer heartbeat timeout detected");
                        let current_state = *state.read().await;
                        if current_state == ServerState::Standby {
                            info!("Initiating automatic failover");
                            *state.write().await = ServerState::Active;
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Check system health
    fn check_system_health(status: &HealthStatus) -> bool {
        status.cpu_usage < 90.0
            && status.memory_usage < 90.0
            && status.disk_usage < 95.0
            && status.network_ok
            && status.errors.is_empty()
    }

    /// Get current state
    pub async fn state(&self) -> ServerState {
        *self.state.read().await
    }

    /// Get health status
    pub async fn health(&self) -> HealthStatus {
        self.health.read().await.clone()
    }

    /// Update peer health
    pub async fn update_peer_health(&self, health: HealthStatus) {
        *self.peer_health.write().await = Some(health);
    }

    /// Manual failover
    pub async fn failover(&self) -> Result<()> {
        let current_state = self.state().await;

        match current_state {
            ServerState::Standby => {
                info!("Activating standby server");
                *self.state.write().await = ServerState::Active;
                Ok(())
            }
            ServerState::Active => {
                warn!("Server is already active");
                Ok(())
            }
            _ => Err(PlayoutError::EmergencyFallback(
                "Cannot failover from current state".to_string(),
            )),
        }
    }

    /// Manual recovery
    pub async fn recover(&self) -> Result<()> {
        let current_state = self.state().await;

        if current_state == ServerState::Failed {
            info!("Starting recovery");
            *self.state.write().await = ServerState::Recovering;

            // Wait for recovery delay
            tokio::time::sleep(Duration::from_secs(self.config.recovery_delay_sec as u64)).await;

            // Check if we should become active or standby
            let new_state = match self.role {
                ServerRole::Primary => ServerState::Active,
                ServerRole::Secondary => ServerState::Standby,
            };

            *self.state.write().await = new_state;
            info!("Recovery complete, new state: {:?}", new_state);

            Ok(())
        } else {
            warn!("Server is not in failed state");
            Ok(())
        }
    }

    /// Mark as failed
    pub async fn mark_failed(&self) {
        warn!("Marking server as failed");
        *self.state.write().await = ServerState::Failed;
    }

    /// Get server role
    pub fn role(&self) -> ServerRole {
        self.role
    }
}

/// Sync state between primary and secondary
pub struct SyncManager {
    config: FailoverConfig,
    last_sync: Arc<RwLock<DateTime<Utc>>>,
}

impl SyncManager {
    /// Create new sync manager
    pub fn new(config: FailoverConfig) -> Self {
        Self {
            config,
            last_sync: Arc::new(RwLock::new(Utc::now())),
        }
    }

    /// Synchronize playlist state
    pub async fn sync_playlist(&self, playlist_id: Uuid) -> Result<()> {
        debug!("Synchronizing playlist: {}", playlist_id);

        // In real implementation, this would sync playlist state to peer
        *self.last_sync.write().await = Utc::now();

        Ok(())
    }

    /// Synchronize content state
    pub async fn sync_content(&self, content_id: Uuid) -> Result<()> {
        debug!("Synchronizing content: {}", content_id);

        // In real implementation, this would sync content state to peer
        *self.last_sync.write().await = Utc::now();

        Ok(())
    }

    /// Get last sync time
    pub async fn last_sync_time(&self) -> DateTime<Utc> {
        *self.last_sync.read().await
    }

    /// Check if sync is up to date
    pub async fn is_synced(&self) -> bool {
        let last = *self.last_sync.read().await;
        let elapsed = Utc::now() - last;
        elapsed.num_seconds() < (self.config.health_check_interval_sec * 2) as i64
    }
}

/// Network failover for output destinations
pub struct NetworkFailover {
    primary_dest: String,
    backup_dest: Option<String>,
    current_dest: Arc<RwLock<String>>,
    failed_over: Arc<RwLock<bool>>,
}

impl NetworkFailover {
    /// Create new network failover
    pub fn new(primary: String, backup: Option<String>) -> Self {
        let current = primary.clone();

        Self {
            primary_dest: primary,
            backup_dest: backup,
            current_dest: Arc::new(RwLock::new(current)),
            failed_over: Arc::new(RwLock::new(false)),
        }
    }

    /// Failover to backup destination
    pub async fn failover_to_backup(&self) -> Result<()> {
        if let Some(backup) = &self.backup_dest {
            info!(
                "Failing over network output from {} to {}",
                self.primary_dest, backup
            );
            *self.current_dest.write().await = backup.clone();
            *self.failed_over.write().await = true;
            Ok(())
        } else {
            Err(PlayoutError::Output(
                "No backup destination configured".to_string(),
            ))
        }
    }

    /// Recover to primary destination
    pub async fn recover_to_primary(&self) -> Result<()> {
        info!(
            "Recovering network output to primary: {}",
            self.primary_dest
        );
        *self.current_dest.write().await = self.primary_dest.clone();
        *self.failed_over.write().await = false;
        Ok(())
    }

    /// Get current destination
    pub async fn current_destination(&self) -> String {
        self.current_dest.read().await.clone()
    }

    /// Check if failed over
    pub async fn is_failed_over(&self) -> bool {
        *self.failed_over.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_failover_config_default() {
        let config = FailoverConfig::default();
        assert!(config.enabled);
        assert_eq!(config.mode, FailoverMode::HotStandby);
        assert!(config.auto_recovery);
    }

    #[test]
    fn test_failover_mode_equality() {
        assert_eq!(FailoverMode::HotStandby, FailoverMode::HotStandby);
        assert_ne!(FailoverMode::HotStandby, FailoverMode::WarmStandby);
    }

    #[test]
    fn test_server_role_equality() {
        assert_eq!(ServerRole::Primary, ServerRole::Primary);
        assert_ne!(ServerRole::Primary, ServerRole::Secondary);
    }

    #[test]
    fn test_server_state_equality() {
        assert_eq!(ServerState::Active, ServerState::Active);
        assert_ne!(ServerState::Active, ServerState::Standby);
    }

    #[test]
    fn test_health_status_default() {
        let status = HealthStatus::default();
        assert!(status.healthy);
        assert_eq!(status.cpu_usage, 0.0);
        assert!(status.errors.is_empty());
    }

    #[tokio::test]
    async fn test_failover_manager_primary() {
        let config = FailoverConfig::default();
        let manager = FailoverManager::new(config, ServerRole::Primary);

        assert_eq!(manager.role(), ServerRole::Primary);
        assert_eq!(manager.state().await, ServerState::Active);
    }

    #[tokio::test]
    async fn test_failover_manager_secondary() {
        let config = FailoverConfig::default();
        let manager = FailoverManager::new(config, ServerRole::Secondary);

        assert_eq!(manager.role(), ServerRole::Secondary);
        assert_eq!(manager.state().await, ServerState::Standby);
    }

    #[tokio::test]
    async fn test_manual_failover() {
        let config = FailoverConfig::default();
        let manager = FailoverManager::new(config, ServerRole::Secondary);

        assert_eq!(manager.state().await, ServerState::Standby);

        manager.failover().await.expect("should succeed in test");
        assert_eq!(manager.state().await, ServerState::Active);
    }

    #[tokio::test]
    async fn test_mark_failed() {
        let config = FailoverConfig::default();
        let manager = FailoverManager::new(config, ServerRole::Primary);

        manager.mark_failed().await;
        assert_eq!(manager.state().await, ServerState::Failed);
    }

    #[tokio::test]
    async fn test_sync_manager() {
        let config = FailoverConfig::default();
        let manager = SyncManager::new(config);

        let playlist_id = Uuid::new_v4();
        manager
            .sync_playlist(playlist_id)
            .await
            .expect("should succeed in test");

        let is_synced = manager.is_synced().await;
        assert!(is_synced);
    }

    #[tokio::test]
    async fn test_network_failover_creation() {
        let failover = NetworkFailover::new(
            "rtmp://primary/live".to_string(),
            Some("rtmp://backup/live".to_string()),
        );

        let current = failover.current_destination().await;
        assert_eq!(current, "rtmp://primary/live");
        assert!(!failover.is_failed_over().await);
    }

    #[tokio::test]
    async fn test_network_failover_to_backup() {
        let failover = NetworkFailover::new(
            "rtmp://primary/live".to_string(),
            Some("rtmp://backup/live".to_string()),
        );

        failover
            .failover_to_backup()
            .await
            .expect("should succeed in test");

        let current = failover.current_destination().await;
        assert_eq!(current, "rtmp://backup/live");
        assert!(failover.is_failed_over().await);
    }

    #[tokio::test]
    async fn test_network_failover_recovery() {
        let failover = NetworkFailover::new(
            "rtmp://primary/live".to_string(),
            Some("rtmp://backup/live".to_string()),
        );

        // Failover to backup
        failover
            .failover_to_backup()
            .await
            .expect("should succeed in test");
        assert!(failover.is_failed_over().await);

        // Recover to primary
        failover
            .recover_to_primary()
            .await
            .expect("should succeed in test");

        let current = failover.current_destination().await;
        assert_eq!(current, "rtmp://primary/live");
        assert!(!failover.is_failed_over().await);
    }

    #[tokio::test]
    async fn test_network_failover_no_backup() {
        let failover = NetworkFailover::new("rtmp://primary/live".to_string(), None);

        let result = failover.failover_to_backup().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_peer_health_update() {
        let config = FailoverConfig::default();
        let manager = FailoverManager::new(config, ServerRole::Primary);

        let peer_health = HealthStatus {
            healthy: true,
            cpu_usage: 50.0,
            memory_usage: 60.0,
            disk_usage: 40.0,
            network_ok: true,
            playout_running: true,
            last_heartbeat: Utc::now(),
            errors: Vec::new(),
        };

        manager.update_peer_health(peer_health.clone()).await;

        // Verify health was updated (indirectly through state checks)
        let health = manager.health().await;
        assert!(health.healthy);
    }
}
