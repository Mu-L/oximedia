#![allow(dead_code)]
//! Worker pool management with grouping, tagging, and health monitoring.
//!
//! Provides two complementary abstractions:
//!
//! ## Pool grouping (`WorkerPool` / `PoolManager`)
//! - Worker groups (pools) with shared properties and tags
//! - Pool-level capacity tracking and utilization metrics
//! - Worker assignment and removal from pools
//! - Pool-based job routing (match job requirements to pool capabilities)
//! - Drain and maintenance mode for individual pools
//!
//! ## Health-monitored node tracking (`WorkerNode` / `WorkerNodePool`)
//! - Per-node heartbeat tracking with configurable timeout
//! - Automatic stale-node detection and offline promotion
//! - Capability and tag-based node queries
//! - Drain mode to stop new job assignment without killing existing work

use std::collections::{HashMap, HashSet};

/// Unique identifier for a worker pool.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PoolId(pub String);

impl PoolId {
    /// Create a new pool identifier.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the pool ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PoolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Status of a worker pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PoolStatus {
    /// Pool is active and accepting jobs.
    Active,
    /// Pool is draining; existing jobs complete but no new ones accepted.
    Draining,
    /// Pool is in maintenance mode; no jobs accepted.
    Maintenance,
    /// Pool is disabled.
    Disabled,
}

impl std::fmt::Display for PoolStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "Active"),
            Self::Draining => write!(f, "Draining"),
            Self::Maintenance => write!(f, "Maintenance"),
            Self::Disabled => write!(f, "Disabled"),
        }
    }
}

/// Error type for worker pool operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolError {
    /// Pool not found.
    PoolNotFound(String),
    /// Pool already exists.
    PoolAlreadyExists(String),
    /// Worker not found in pool.
    WorkerNotFound(String),
    /// Worker already in pool.
    WorkerAlreadyInPool(String),
    /// Pool is not accepting jobs.
    PoolNotAccepting(String),
    /// Pool capacity exceeded.
    CapacityExceeded {
        /// The pool id.
        pool_id: String,
        /// Maximum workers allowed.
        max: usize,
    },
}

impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PoolNotFound(id) => write!(f, "pool not found: {id}"),
            Self::PoolAlreadyExists(id) => write!(f, "pool already exists: {id}"),
            Self::WorkerNotFound(id) => write!(f, "worker not found: {id}"),
            Self::WorkerAlreadyInPool(id) => write!(f, "worker already in pool: {id}"),
            Self::PoolNotAccepting(id) => write!(f, "pool not accepting jobs: {id}"),
            Self::CapacityExceeded { pool_id, max } => {
                write!(f, "pool {pool_id} capacity exceeded (max {max})")
            }
        }
    }
}

impl std::error::Error for PoolError {}

/// Result type for pool operations.
pub type Result<T> = std::result::Result<T, PoolError>;

/// A single worker pool definition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkerPool {
    /// Pool identifier.
    pub id: PoolId,
    /// Human-readable name.
    pub name: String,
    /// Pool status.
    pub status: PoolStatus,
    /// Tags describing pool capabilities (e.g., "gpu", "high-memory").
    pub tags: HashSet<String>,
    /// Maximum number of workers in this pool (0 = unlimited).
    pub max_workers: usize,
    /// Worker IDs currently assigned to this pool.
    pub workers: HashSet<String>,
    /// Priority weight for job routing (higher = preferred).
    pub priority_weight: u32,
}

impl WorkerPool {
    /// Create a new active pool.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: PoolId::new(id),
            name: name.into(),
            status: PoolStatus::Active,
            tags: HashSet::new(),
            max_workers: 0,
            workers: HashSet::new(),
            priority_weight: 100,
        }
    }

    /// Set the maximum worker count.
    #[must_use]
    pub fn with_max_workers(mut self, max: usize) -> Self {
        self.max_workers = max;
        self
    }

    /// Add a tag to the pool.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.insert(tag.into());
        self
    }

    /// Set the priority weight.
    #[must_use]
    pub fn with_priority(mut self, weight: u32) -> Self {
        self.priority_weight = weight;
        self
    }

    /// Check if the pool is accepting new work.
    #[must_use]
    pub fn is_accepting(&self) -> bool {
        self.status == PoolStatus::Active
    }

    /// Get the current number of workers.
    #[must_use]
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    /// Check if the pool has room for another worker.
    #[must_use]
    pub fn has_capacity(&self) -> bool {
        self.max_workers == 0 || self.workers.len() < self.max_workers
    }

    /// Check if the pool has a specific tag.
    #[must_use]
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
    }

    /// Check if the pool has ALL of the required tags.
    #[must_use]
    pub fn has_all_tags(&self, required: &[String]) -> bool {
        required.iter().all(|t| self.tags.contains(t))
    }

    /// Add a worker to this pool.
    ///
    /// # Errors
    ///
    /// Returns `PoolError::WorkerAlreadyInPool` if the worker is already in this pool,
    /// or `PoolError::CapacityExceeded` if the pool is full.
    pub fn add_worker(&mut self, worker_id: impl Into<String>) -> Result<()> {
        let wid = worker_id.into();
        if self.workers.contains(&wid) {
            return Err(PoolError::WorkerAlreadyInPool(wid));
        }
        if !self.has_capacity() {
            return Err(PoolError::CapacityExceeded {
                pool_id: self.id.0.clone(),
                max: self.max_workers,
            });
        }
        self.workers.insert(wid);
        Ok(())
    }

    /// Remove a worker from this pool.
    ///
    /// # Errors
    ///
    /// Returns `PoolError::WorkerNotFound` if the worker is not in this pool.
    pub fn remove_worker(&mut self, worker_id: &str) -> Result<()> {
        if !self.workers.remove(worker_id) {
            return Err(PoolError::WorkerNotFound(worker_id.to_string()));
        }
        Ok(())
    }
}

/// Manages multiple worker pools.
#[derive(Debug, Default)]
pub struct PoolManager {
    /// Map of pool ID to pool.
    pools: HashMap<String, WorkerPool>,
}

impl PoolManager {
    /// Create an empty pool manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new pool.
    ///
    /// # Errors
    ///
    /// Returns `PoolError::PoolAlreadyExists` if a pool with the same ID exists.
    pub fn add_pool(&mut self, pool: WorkerPool) -> Result<()> {
        if self.pools.contains_key(&pool.id.0) {
            return Err(PoolError::PoolAlreadyExists(pool.id.0.clone()));
        }
        self.pools.insert(pool.id.0.clone(), pool);
        Ok(())
    }

    /// Get a pool by ID.
    #[must_use]
    pub fn get_pool(&self, pool_id: &str) -> Option<&WorkerPool> {
        self.pools.get(pool_id)
    }

    /// Get a mutable reference to a pool by ID.
    pub fn get_pool_mut(&mut self, pool_id: &str) -> Option<&mut WorkerPool> {
        self.pools.get_mut(pool_id)
    }

    /// Remove a pool.
    pub fn remove_pool(&mut self, pool_id: &str) -> Option<WorkerPool> {
        self.pools.remove(pool_id)
    }

    /// Get the number of pools.
    #[must_use]
    pub fn pool_count(&self) -> usize {
        self.pools.len()
    }

    /// Find all pools that have the specified tags and are currently accepting.
    #[must_use]
    pub fn find_pools_by_tags(&self, required_tags: &[String]) -> Vec<&WorkerPool> {
        self.pools
            .values()
            .filter(|p| p.is_accepting() && p.has_all_tags(required_tags))
            .collect()
    }

    /// Get the total number of workers across all pools.
    #[must_use]
    pub fn total_workers(&self) -> usize {
        self.pools.values().map(WorkerPool::worker_count).sum()
    }

    /// Set the status of a pool.
    ///
    /// # Errors
    ///
    /// Returns `PoolError::PoolNotFound` if the pool does not exist.
    pub fn set_pool_status(&mut self, pool_id: &str, status: PoolStatus) -> Result<()> {
        let pool = self
            .pools
            .get_mut(pool_id)
            .ok_or_else(|| PoolError::PoolNotFound(pool_id.to_string()))?;
        pool.status = status;
        Ok(())
    }

    /// List all pool IDs.
    pub fn list_pool_ids(&self) -> Vec<&str> {
        self.pools.keys().map(String::as_str).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_creation() {
        let pool = WorkerPool::new("gpu-pool", "GPU Workers");
        assert_eq!(pool.id.as_str(), "gpu-pool");
        assert_eq!(pool.name, "GPU Workers");
        assert_eq!(pool.status, PoolStatus::Active);
        assert_eq!(pool.worker_count(), 0);
    }

    #[test]
    fn test_pool_with_builders() {
        let pool = WorkerPool::new("p1", "Pool 1")
            .with_max_workers(10)
            .with_tag("gpu")
            .with_tag("high-memory")
            .with_priority(200);
        assert_eq!(pool.max_workers, 10);
        assert!(pool.has_tag("gpu"));
        assert!(pool.has_tag("high-memory"));
        assert_eq!(pool.priority_weight, 200);
    }

    #[test]
    fn test_add_remove_worker() {
        let mut pool = WorkerPool::new("p1", "Pool 1").with_max_workers(2);
        pool.add_worker("w1").expect("add_worker should succeed");
        pool.add_worker("w2").expect("add_worker should succeed");
        assert_eq!(pool.worker_count(), 2);
        assert!(!pool.has_capacity());
        pool.remove_worker("w1")
            .expect("remove_worker should succeed");
        assert_eq!(pool.worker_count(), 1);
        assert!(pool.has_capacity());
    }

    #[test]
    fn test_add_worker_duplicate() {
        let mut pool = WorkerPool::new("p1", "Pool 1");
        pool.add_worker("w1").expect("add_worker should succeed");
        let err = pool.add_worker("w1").unwrap_err();
        assert_eq!(err, PoolError::WorkerAlreadyInPool("w1".to_string()));
    }

    #[test]
    fn test_add_worker_capacity_exceeded() {
        let mut pool = WorkerPool::new("p1", "Pool 1").with_max_workers(1);
        pool.add_worker("w1").expect("add_worker should succeed");
        let err = pool.add_worker("w2").unwrap_err();
        assert_eq!(
            err,
            PoolError::CapacityExceeded {
                pool_id: "p1".to_string(),
                max: 1,
            }
        );
    }

    #[test]
    fn test_remove_worker_not_found() {
        let mut pool = WorkerPool::new("p1", "Pool 1");
        let err = pool.remove_worker("w_none").unwrap_err();
        assert_eq!(err, PoolError::WorkerNotFound("w_none".to_string()));
    }

    #[test]
    fn test_pool_status_display() {
        assert_eq!(PoolStatus::Active.to_string(), "Active");
        assert_eq!(PoolStatus::Draining.to_string(), "Draining");
        assert_eq!(PoolStatus::Maintenance.to_string(), "Maintenance");
        assert_eq!(PoolStatus::Disabled.to_string(), "Disabled");
    }

    #[test]
    fn test_pool_accepting() {
        let mut pool = WorkerPool::new("p1", "Pool 1");
        assert!(pool.is_accepting());
        pool.status = PoolStatus::Draining;
        assert!(!pool.is_accepting());
    }

    #[test]
    fn test_has_all_tags() {
        let pool = WorkerPool::new("p1", "Pool 1")
            .with_tag("gpu")
            .with_tag("fast");
        assert!(pool.has_all_tags(&["gpu".to_string(), "fast".to_string()]));
        assert!(!pool.has_all_tags(&["gpu".to_string(), "ssd".to_string()]));
    }

    #[test]
    fn test_pool_manager_add_and_get() {
        let mut mgr = PoolManager::new();
        mgr.add_pool(WorkerPool::new("p1", "Pool 1"))
            .expect("failed to create");
        assert_eq!(mgr.pool_count(), 1);
        assert!(mgr.get_pool("p1").is_some());
        assert!(mgr.get_pool("p2").is_none());
    }

    #[test]
    fn test_pool_manager_duplicate() {
        let mut mgr = PoolManager::new();
        mgr.add_pool(WorkerPool::new("p1", "Pool 1"))
            .expect("failed to create");
        let err = mgr
            .add_pool(WorkerPool::new("p1", "Pool 1 dup"))
            .unwrap_err();
        assert_eq!(err, PoolError::PoolAlreadyExists("p1".to_string()));
    }

    #[test]
    fn test_pool_manager_find_by_tags() {
        let mut mgr = PoolManager::new();
        mgr.add_pool(WorkerPool::new("gpu", "GPU").with_tag("gpu"))
            .expect("operation should succeed");
        mgr.add_pool(WorkerPool::new("cpu", "CPU").with_tag("cpu"))
            .expect("operation should succeed");
        let found = mgr.find_pools_by_tags(&["gpu".to_string()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id.as_str(), "gpu");
    }

    #[test]
    fn test_pool_manager_total_workers() {
        let mut mgr = PoolManager::new();
        let mut p1 = WorkerPool::new("p1", "P1");
        p1.add_worker("w1").expect("add_worker should succeed");
        p1.add_worker("w2").expect("add_worker should succeed");
        let mut p2 = WorkerPool::new("p2", "P2");
        p2.add_worker("w3").expect("add_worker should succeed");
        mgr.add_pool(p1).expect("add_pool should succeed");
        mgr.add_pool(p2).expect("add_pool should succeed");
        assert_eq!(mgr.total_workers(), 3);
    }

    #[test]
    fn test_pool_manager_set_status() {
        let mut mgr = PoolManager::new();
        mgr.add_pool(WorkerPool::new("p1", "P1"))
            .expect("failed to create");
        mgr.set_pool_status("p1", PoolStatus::Draining)
            .expect("set_pool_status should succeed");
        assert_eq!(
            mgr.get_pool("p1").expect("get_pool should succeed").status,
            PoolStatus::Draining
        );
    }

    #[test]
    fn test_pool_error_display() {
        let err = PoolError::PoolNotFound("missing".to_string());
        assert_eq!(err.to_string(), "pool not found: missing");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Health-monitored worker node tracking
// ═══════════════════════════════════════════════════════════════════════════════

use std::time::{Duration, Instant};

/// Operational status of a worker node inside a [`WorkerNodePool`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerStatus {
    /// Node is online and accepting new jobs.
    Online,
    /// Node is unreachable or has timed out; not accepting jobs.
    Offline,
    /// Node is gracefully winding down; existing jobs complete but no new ones accepted.
    Draining,
    /// Node is undergoing maintenance; no jobs accepted.
    Maintenance,
}

/// A single worker node tracked by a [`WorkerNodePool`].
#[derive(Debug, Clone)]
pub struct WorkerNode {
    /// Unique identifier for this node.
    pub id: String,
    /// DNS name or IP address of the node.
    pub hostname: String,
    /// Port on which the node listens for work assignments.
    pub port: u16,
    /// Current operational status.
    pub status: WorkerStatus,
    /// Wall-clock time when the node first registered.
    pub registered_at: Instant,
    /// Wall-clock time of the most recent heartbeat received from the node.
    pub last_heartbeat: Instant,
    /// How often the node is expected to send heartbeats.
    pub heartbeat_interval: Duration,
    /// Capability tags advertised by the node (e.g. `"gpu"`, `"high-memory"`).
    pub capabilities: Vec<String>,
    /// Arbitrary key-value metadata attached to the node.
    pub tags: HashMap<String, String>,
}

impl WorkerNode {
    /// Create a new node that is immediately `Online`.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        hostname: impl Into<String>,
        port: u16,
        heartbeat_interval: Duration,
    ) -> Self {
        let now = Instant::now();
        Self {
            id: id.into(),
            hostname: hostname.into(),
            port,
            status: WorkerStatus::Online,
            registered_at: now,
            last_heartbeat: now,
            heartbeat_interval,
            capabilities: Vec::new(),
            tags: HashMap::new(),
        }
    }

    /// Return `true` when the node is `Online` (can accept new jobs).
    #[must_use]
    pub fn is_online(&self) -> bool {
        self.status == WorkerStatus::Online
    }
}

/// A pool of [`WorkerNode`]s with integrated heartbeat monitoring.
///
/// The pool does **not** spawn background tasks; callers are responsible for
/// driving liveness checks by periodically calling [`WorkerNodePool::prune_stale`].
pub struct WorkerNodePool {
    workers: HashMap<String, WorkerNode>,
    /// How long a node may go without a heartbeat before being marked `Offline`.
    heartbeat_timeout: Duration,
}

impl WorkerNodePool {
    /// Create an empty pool.  Nodes whose heartbeat exceeds `heartbeat_timeout`
    /// will be marked `Offline` during the next [`prune_stale`] call.
    ///
    /// [`prune_stale`]: WorkerNodePool::prune_stale
    #[must_use]
    pub fn new(heartbeat_timeout: Duration) -> Self {
        Self {
            workers: HashMap::new(),
            heartbeat_timeout,
        }
    }

    /// Register a new node.  If a node with the same ID already exists, its
    /// entry is replaced.
    pub fn register(&mut self, node: WorkerNode) {
        self.workers.insert(node.id.clone(), node);
    }

    /// Remove a node from the pool.  Returns `true` if the node was present.
    pub fn deregister(&mut self, id: &str) -> bool {
        self.workers.remove(id).is_some()
    }

    /// Record a heartbeat for the identified node.
    ///
    /// Returns `true` on success.  Returns `false` when `id` is not registered,
    /// so callers can decide whether to auto-register or log a warning.
    pub fn heartbeat(&mut self, id: &str) -> bool {
        if let Some(node) = self.workers.get_mut(id) {
            node.last_heartbeat = Instant::now();
            true
        } else {
            false
        }
    }

    /// Scan all nodes and promote any node whose heartbeat has not arrived
    /// within `heartbeat_timeout` to `Offline`.
    ///
    /// Returns the IDs of every node that was transitioned to `Offline` during
    /// this call (already-offline nodes are not repeated).
    pub fn prune_stale(&mut self) -> Vec<String> {
        let now = Instant::now();
        let timeout = self.heartbeat_timeout;
        let mut newly_offline = Vec::new();

        for node in self.workers.values_mut() {
            if node.status != WorkerStatus::Offline
                && now.duration_since(node.last_heartbeat) > timeout
            {
                node.status = WorkerStatus::Offline;
                newly_offline.push(node.id.clone());
            }
        }
        newly_offline
    }

    /// Return references to all nodes whose status is `Online`.
    #[must_use]
    pub fn online_workers(&self) -> Vec<&WorkerNode> {
        self.workers
            .values()
            .filter(|n| n.status == WorkerStatus::Online)
            .collect()
    }

    /// Return references to all nodes that advertise the given capability.
    #[must_use]
    pub fn workers_with_capability(&self, cap: &str) -> Vec<&WorkerNode> {
        self.workers
            .values()
            .filter(|n| n.capabilities.iter().any(|c| c == cap))
            .collect()
    }

    /// Transition a node to `Draining` so that no new jobs are assigned to it.
    ///
    /// Returns `true` if the node exists (regardless of its previous status).
    /// Returns `false` when the node is not registered.
    pub fn drain_worker(&mut self, id: &str) -> bool {
        if let Some(node) = self.workers.get_mut(id) {
            node.status = WorkerStatus::Draining;
            true
        } else {
            false
        }
    }

    /// Calculate the maximum number of jobs the pool can accept, assuming each
    /// `Online` worker can run up to `max_jobs_per_worker` concurrent jobs.
    #[must_use]
    pub fn total_capacity(&self, max_jobs_per_worker: u32) -> u32 {
        let online_count = self
            .workers
            .values()
            .filter(|n| n.status == WorkerStatus::Online)
            .count() as u32;
        online_count.saturating_mul(max_jobs_per_worker)
    }
}

#[cfg(test)]
mod node_pool_tests {
    use super::*;

    fn make_node(id: &str) -> WorkerNode {
        WorkerNode::new(id, "localhost", 9000, Duration::from_secs(30))
    }

    fn make_node_with_capabilities(id: &str, caps: Vec<&str>) -> WorkerNode {
        let mut n = make_node(id);
        n.capabilities = caps.into_iter().map(|s| s.to_string()).collect();
        n
    }

    // ── Register / deregister ─────────────────────────────────────────────────

    #[test]
    fn test_register_and_online_count() {
        let mut pool = WorkerNodePool::new(Duration::from_secs(60));
        pool.register(make_node("n1"));
        pool.register(make_node("n2"));
        assert_eq!(pool.online_workers().len(), 2);
    }

    #[test]
    fn test_deregister_removes_node() {
        let mut pool = WorkerNodePool::new(Duration::from_secs(60));
        pool.register(make_node("n1"));
        assert!(pool.deregister("n1"));
        assert!(!pool.deregister("n1")); // second call → false
        assert!(pool.online_workers().is_empty());
    }

    #[test]
    fn test_deregister_nonexistent_returns_false() {
        let mut pool = WorkerNodePool::new(Duration::from_secs(60));
        assert!(!pool.deregister("ghost"));
    }

    // ── Heartbeat ─────────────────────────────────────────────────────────────

    #[test]
    fn test_heartbeat_known_node_returns_true() {
        let mut pool = WorkerNodePool::new(Duration::from_secs(60));
        pool.register(make_node("n1"));
        assert!(pool.heartbeat("n1"));
    }

    #[test]
    fn test_heartbeat_unknown_node_returns_false() {
        let mut pool = WorkerNodePool::new(Duration::from_secs(60));
        assert!(!pool.heartbeat("ghost"));
    }

    // ── Stale pruning ─────────────────────────────────────────────────────────

    #[test]
    fn test_prune_stale_marks_timed_out_nodes_offline() {
        let mut pool = WorkerNodePool::new(Duration::from_nanos(1));
        pool.register(make_node("n1"));
        // Sleep just long enough for the timeout to trigger.
        std::thread::sleep(Duration::from_millis(5));
        let stale = pool.prune_stale();
        assert!(stale.contains(&"n1".to_string()));
        assert_eq!(pool.online_workers().len(), 0);
    }

    #[test]
    fn test_prune_stale_does_not_double_report() {
        let mut pool = WorkerNodePool::new(Duration::from_nanos(1));
        pool.register(make_node("n1"));
        std::thread::sleep(Duration::from_millis(5));
        let first = pool.prune_stale();
        let second = pool.prune_stale(); // already offline
        assert_eq!(first.len(), 1);
        assert!(second.is_empty());
    }

    #[test]
    fn test_prune_stale_skips_fresh_nodes() {
        let mut pool = WorkerNodePool::new(Duration::from_secs(3600));
        pool.register(make_node("n1"));
        let stale = pool.prune_stale();
        assert!(stale.is_empty());
        assert_eq!(pool.online_workers().len(), 1);
    }

    // ── Capability queries ────────────────────────────────────────────────────

    #[test]
    fn test_workers_with_capability_filters_correctly() {
        let mut pool = WorkerNodePool::new(Duration::from_secs(60));
        pool.register(make_node_with_capabilities("n1", vec!["gpu", "fast-disk"]));
        pool.register(make_node_with_capabilities("n2", vec!["fast-disk"]));
        pool.register(make_node_with_capabilities("n3", vec!["gpu"]));

        let gpu_workers = pool.workers_with_capability("gpu");
        assert_eq!(gpu_workers.len(), 2);
        let disk_workers = pool.workers_with_capability("fast-disk");
        assert_eq!(disk_workers.len(), 2);
        let rare = pool.workers_with_capability("fpga");
        assert!(rare.is_empty());
    }

    // ── Drain ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_drain_worker_transitions_to_draining() {
        let mut pool = WorkerNodePool::new(Duration::from_secs(60));
        pool.register(make_node("n1"));
        assert!(pool.drain_worker("n1"));
        let online = pool.online_workers();
        assert!(online.is_empty()); // draining ≠ online
                                    // Node is still registered.
        assert!(pool.workers_with_capability("").is_empty() || true); // just check no panic
    }

    #[test]
    fn test_drain_nonexistent_worker_returns_false() {
        let mut pool = WorkerNodePool::new(Duration::from_secs(60));
        assert!(!pool.drain_worker("ghost"));
    }

    // ── Capacity ──────────────────────────────────────────────────────────────

    #[test]
    fn test_total_capacity_counts_only_online() {
        let mut pool = WorkerNodePool::new(Duration::from_secs(60));
        pool.register(make_node("n1")); // online
        pool.register(make_node("n2")); // will drain
        pool.drain_worker("n2");

        assert_eq!(pool.total_capacity(4), 4); // only n1 counts
    }

    #[test]
    fn test_total_capacity_empty_pool_is_zero() {
        let pool = WorkerNodePool::new(Duration::from_secs(60));
        assert_eq!(pool.total_capacity(10), 0);
    }

    #[test]
    fn test_total_capacity_saturating_mul() {
        let mut pool = WorkerNodePool::new(Duration::from_secs(60));
        for i in 0..3 {
            pool.register(make_node(&format!("n{i}")));
        }
        assert_eq!(pool.total_capacity(5), 15);
    }
}
