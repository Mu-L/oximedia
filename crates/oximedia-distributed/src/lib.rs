//! Distributed encoding coordinator for `OxiMedia`.
//!
//! This crate provides a distributed video encoding system with:
//! - Central coordinator for job management
//! - Worker nodes for distributed encoding
//! - Multiple splitting strategies (segment, tile, GOP-based)
//! - Load balancing and fault tolerance
//! - gRPC-based communication

pub mod backpressure;
pub mod checkpointing;
pub mod circuit_breaker;
pub mod cluster;
pub mod consensus;
pub mod coordinator;
pub mod discovery;
pub mod fault_tolerance;
pub mod heartbeat;
pub mod job_tracker;
pub mod leader_election;
pub mod load_balancer;
pub mod message_bus;
pub mod message_queue;
pub mod metrics_aggregator;
pub mod node_health;
pub mod node_registry;
pub mod node_topology;
pub mod partition;
pub mod pb;
pub mod raft_primitives;
pub mod replication;
pub mod resource_quota;
pub mod scheduler;
pub mod segment;
pub mod shard;
pub mod shard_map;
pub mod snapshot_store;
pub mod task_distribution;
pub mod task_priority_queue;
pub mod task_queue;
pub mod task_retry;
pub mod work_stealing;
pub mod worker;

use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

/// Result type for distributed operations
pub type Result<T> = std::result::Result<T, DistributedError>;

/// Errors that can occur in distributed encoding
#[derive(Debug, Error)]
pub enum DistributedError {
    #[error("Worker error: {0}")]
    Worker(String),

    #[error("Coordinator error: {0}")]
    Coordinator(String),

    #[error("Job error: {0}")]
    Job(String),

    #[error("Network error: {0}")]
    Network(#[from] tonic::transport::Error),

    #[error("gRPC status error: {0}")]
    Status(#[from] tonic::Status),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Discovery error: {0}")]
    Discovery(String),

    #[error("Scheduling error: {0}")]
    Scheduling(String),

    #[error("Segmentation error: {0}")]
    Segmentation(String),

    #[error("Timeout error")]
    Timeout,

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),

    #[error("Error: {0}")]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl From<Box<dyn std::error::Error + Send + Sync>> for DistributedError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        DistributedError::Other(err)
    }
}

/// Configuration for the distributed encoder
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DistributedConfig {
    /// Coordinator address
    pub coordinator_addr: String,

    /// Maximum number of retry attempts
    pub max_retries: u32,

    /// Heartbeat interval
    pub heartbeat_interval: Duration,

    /// Job timeout
    pub job_timeout: Duration,

    /// Maximum concurrent jobs per worker
    pub max_concurrent_jobs: u32,

    /// Enable fault tolerance
    pub fault_tolerance: bool,

    /// Worker discovery method
    pub discovery_method: DiscoveryMethod,
}

impl Default for DistributedConfig {
    fn default() -> Self {
        Self {
            coordinator_addr: "127.0.0.1:50051".to_string(),
            max_retries: 3,
            heartbeat_interval: Duration::from_secs(30),
            job_timeout: Duration::from_secs(3600),
            max_concurrent_jobs: 4,
            fault_tolerance: true,
            discovery_method: DiscoveryMethod::Static,
        }
    }
}

/// Worker discovery methods
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub enum DiscoveryMethod {
    /// Static configuration
    Static,
    /// Multicast DNS
    #[allow(clippy::upper_case_acronyms)]
    MDNS,
    /// etcd-based discovery
    Etcd,
    /// Consul-based discovery
    Consul,
}

/// Job splitting strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SplitStrategy {
    /// Split by time segments
    SegmentBased,
    /// Split by spatial tiles
    TileBased,
    /// Split by GOP (Group of Pictures)
    GopBased,
}

impl From<SplitStrategy> for i32 {
    fn from(strategy: SplitStrategy) -> Self {
        match strategy {
            SplitStrategy::SegmentBased => 0,
            SplitStrategy::TileBased => 1,
            SplitStrategy::GopBased => 2,
        }
    }
}

/// Job priority levels
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum JobPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl From<JobPriority> for u32 {
    fn from(priority: JobPriority) -> Self {
        priority as u32
    }
}

/// Represents a distributed encoding job
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DistributedJob {
    /// Unique job identifier
    pub id: Uuid,

    /// Task identifier (multiple jobs can belong to same task)
    pub task_id: Uuid,

    /// Source video URL
    pub source_url: String,

    /// Target codec
    pub codec: String,

    /// Splitting strategy
    pub strategy: SplitStrategy,

    /// Job priority
    pub priority: JobPriority,

    /// Encoding parameters
    pub params: EncodingParams,

    /// Output destination
    pub output_url: String,

    /// Deadline timestamp (Unix epoch)
    pub deadline: Option<i64>,
}

/// Encoding parameters
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EncodingParams {
    pub bitrate: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub preset: Option<String>,
    pub profile: Option<String>,
    pub crf: Option<u32>,
    pub extra_params: std::collections::HashMap<String, String>,
}

impl Default for EncodingParams {
    fn default() -> Self {
        Self {
            bitrate: None,
            width: None,
            height: None,
            preset: Some("medium".to_string()),
            profile: None,
            crf: Some(23),
            extra_params: std::collections::HashMap::new(),
        }
    }
}

/// Main distributed encoder interface
pub struct DistributedEncoder {
    config: DistributedConfig,
}

impl DistributedEncoder {
    /// Create a new distributed encoder with the given configuration
    #[must_use]
    pub fn new(config: DistributedConfig) -> Self {
        Self { config }
    }

    /// Create a new distributed encoder with default configuration
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(DistributedConfig::default())
    }

    /// Get the current configuration
    #[must_use]
    pub fn config(&self) -> &DistributedConfig {
        &self.config
    }

    /// Submit a job for distributed encoding
    ///
    /// # Arguments
    ///
    /// * `job` - The encoding job to submit
    ///
    /// # Returns
    ///
    /// Returns the job ID on success
    pub async fn submit_job(&self, job: DistributedJob) -> Result<Uuid> {
        // This will be implemented by the coordinator
        tracing::info!(
            "Submitting job {} to coordinator at {}",
            job.id,
            self.config.coordinator_addr
        );
        Ok(job.id)
    }

    /// Query job status
    pub async fn job_status(&self, job_id: Uuid) -> Result<JobStatus> {
        tracing::debug!("Querying status for job {}", job_id);
        // This will be implemented by the coordinator
        Ok(JobStatus::Pending)
    }

    /// Cancel a job
    pub async fn cancel_job(&self, job_id: Uuid) -> Result<()> {
        tracing::info!("Cancelling job {}", job_id);
        // This will be implemented by the coordinator
        Ok(())
    }
}

/// Job execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum JobStatus {
    Pending,
    Assigned,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DistributedConfig::default();
        assert_eq!(config.coordinator_addr, "127.0.0.1:50051");
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.max_concurrent_jobs, 4);
    }

    #[test]
    fn test_encoder_creation() {
        let encoder = DistributedEncoder::with_defaults();
        assert_eq!(encoder.config().coordinator_addr, "127.0.0.1:50051");
    }

    #[test]
    fn test_job_priority_ordering() {
        assert!(JobPriority::Critical > JobPriority::High);
        assert!(JobPriority::High > JobPriority::Normal);
        assert!(JobPriority::Normal > JobPriority::Low);
    }
}
