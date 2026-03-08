#![allow(dead_code)]
//! Resource pool management for workflow execution.
//!
//! Provides a pooling system for shared resources (CPU cores, GPU devices,
//! network bandwidth, disk I/O slots) that workflow tasks can reserve and
//! release during execution. The pool enforces capacity limits and supports
//! fair scheduling of resource requests.

use std::collections::HashMap;

/// Unique identifier for a resource type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceId(String);

impl ResourceId {
    /// Create a new resource identifier.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Return the string name of this resource.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ResourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Describes a resource with capacity and current allocation.
#[derive(Debug, Clone)]
pub struct ResourceDescriptor {
    /// Unique identifier for this resource.
    pub id: ResourceId,
    /// Maximum capacity available.
    pub capacity: u64,
    /// Currently allocated amount.
    pub allocated: u64,
    /// Human-readable label.
    pub label: String,
    /// Unit of measurement (e.g., "cores", "MB", "Mbps").
    pub unit: String,
}

impl ResourceDescriptor {
    /// Create a new resource descriptor.
    pub fn new(
        id: ResourceId,
        capacity: u64,
        label: impl Into<String>,
        unit: impl Into<String>,
    ) -> Self {
        Self {
            id,
            capacity,
            allocated: 0,
            label: label.into(),
            unit: unit.into(),
        }
    }

    /// Return the remaining available capacity.
    #[must_use]
    pub fn available(&self) -> u64 {
        self.capacity.saturating_sub(self.allocated)
    }

    /// Return the utilisation ratio as a value between 0.0 and 1.0.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn utilisation(&self) -> f64 {
        if self.capacity == 0 {
            return 0.0;
        }
        self.allocated as f64 / self.capacity as f64
    }

    /// Check whether the requested amount can be satisfied.
    #[must_use]
    pub fn can_allocate(&self, amount: u64) -> bool {
        self.available() >= amount
    }
}

/// A token representing a successful allocation that must be released.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AllocationToken {
    /// Unique token identifier.
    pub token_id: u64,
    /// Resource this token belongs to.
    pub resource_id: ResourceId,
    /// Amount allocated.
    pub amount: u64,
}

/// A request for resources from the pool.
#[derive(Debug, Clone)]
pub struct ResourceRequest {
    /// Which resource is being requested.
    pub resource_id: ResourceId,
    /// How much is needed.
    pub amount: u64,
    /// Priority of this request (higher = more important).
    pub priority: u32,
    /// Optional requester tag for tracking.
    pub requester: String,
}

impl ResourceRequest {
    /// Create a new resource request.
    #[must_use]
    pub fn new(resource_id: ResourceId, amount: u64) -> Self {
        Self {
            resource_id,
            amount,
            priority: 0,
            requester: String::new(),
        }
    }

    /// Set the priority of this request.
    #[must_use]
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Set the requester tag.
    pub fn with_requester(mut self, requester: impl Into<String>) -> Self {
        self.requester = requester.into();
        self
    }
}

/// Errors that can occur during pool operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolError {
    /// The requested resource does not exist in the pool.
    ResourceNotFound(String),
    /// Insufficient capacity for the requested amount.
    InsufficientCapacity {
        /// Resource identifier.
        resource: String,
        /// Amount requested.
        requested: u64,
        /// Amount available.
        available: u64,
    },
    /// The allocation token is invalid or already released.
    InvalidToken(u64),
}

impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResourceNotFound(id) => write!(f, "resource not found: {id}"),
            Self::InsufficientCapacity {
                resource,
                requested,
                available,
            } => {
                write!(f, "insufficient capacity for '{resource}': requested {requested}, available {available}")
            }
            Self::InvalidToken(id) => write!(f, "invalid allocation token: {id}"),
        }
    }
}

/// The main resource pool that tracks multiple resource types.
#[derive(Debug)]
pub struct ResourcePool {
    /// All resources in this pool.
    resources: HashMap<ResourceId, ResourceDescriptor>,
    /// Outstanding allocations keyed by token id.
    allocations: HashMap<u64, AllocationToken>,
    /// Counter for generating unique token ids.
    next_token_id: u64,
}

impl ResourcePool {
    /// Create a new empty resource pool.
    #[must_use]
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
            allocations: HashMap::new(),
            next_token_id: 1,
        }
    }

    /// Register a resource with the pool.
    pub fn register(&mut self, descriptor: ResourceDescriptor) {
        self.resources.insert(descriptor.id.clone(), descriptor);
    }

    /// Attempt to allocate a resource, returning a token on success.
    ///
    /// # Errors
    ///
    /// Returns `PoolError` if the resource is not found or has insufficient capacity.
    pub fn allocate(&mut self, request: &ResourceRequest) -> Result<AllocationToken, PoolError> {
        let resource = self
            .resources
            .get_mut(&request.resource_id)
            .ok_or_else(|| PoolError::ResourceNotFound(request.resource_id.name().to_string()))?;

        if !resource.can_allocate(request.amount) {
            return Err(PoolError::InsufficientCapacity {
                resource: resource.id.name().to_string(),
                requested: request.amount,
                available: resource.available(),
            });
        }

        resource.allocated += request.amount;

        let token = AllocationToken {
            token_id: self.next_token_id,
            resource_id: request.resource_id.clone(),
            amount: request.amount,
        };
        self.next_token_id += 1;
        self.allocations.insert(token.token_id, token.clone());
        Ok(token)
    }

    /// Release a previous allocation using its token.
    ///
    /// # Errors
    ///
    /// Returns `PoolError::InvalidToken` if the token does not match any active allocation.
    pub fn release(&mut self, token_id: u64) -> Result<(), PoolError> {
        let token = self
            .allocations
            .remove(&token_id)
            .ok_or(PoolError::InvalidToken(token_id))?;

        if let Some(resource) = self.resources.get_mut(&token.resource_id) {
            resource.allocated = resource.allocated.saturating_sub(token.amount);
        }

        Ok(())
    }

    /// Return a snapshot of all resources and their current state.
    #[must_use]
    pub fn snapshot(&self) -> Vec<ResourceDescriptor> {
        self.resources.values().cloned().collect()
    }

    /// Return the descriptor for a specific resource.
    #[must_use]
    pub fn get_resource(&self, id: &ResourceId) -> Option<&ResourceDescriptor> {
        self.resources.get(id)
    }

    /// Return the number of active allocations.
    #[must_use]
    pub fn active_allocations(&self) -> usize {
        self.allocations.len()
    }

    /// Return the total number of registered resources.
    #[must_use]
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    /// Check whether a request can be satisfied without actually allocating.
    #[must_use]
    pub fn can_satisfy(&self, request: &ResourceRequest) -> bool {
        self.resources
            .get(&request.resource_id)
            .is_some_and(|r| r.can_allocate(request.amount))
    }

    /// Reset all allocations (useful for testing or emergency recovery).
    pub fn reset_all(&mut self) {
        self.allocations.clear();
        for resource in self.resources.values_mut() {
            resource.allocated = 0;
        }
    }
}

impl Default for ResourcePool {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary statistics for the entire pool.
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Total capacity across all resources.
    pub total_capacity: u64,
    /// Total allocated across all resources.
    pub total_allocated: u64,
    /// Number of resources.
    pub resource_count: usize,
    /// Number of active allocations.
    pub active_allocations: usize,
    /// Average utilisation across resources.
    pub average_utilisation: f64,
}

impl ResourcePool {
    /// Compute aggregate statistics for the pool.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn stats(&self) -> PoolStats {
        let total_capacity: u64 = self.resources.values().map(|r| r.capacity).sum();
        let total_allocated: u64 = self.resources.values().map(|r| r.allocated).sum();
        let resource_count = self.resources.len();
        let active_allocations = self.allocations.len();
        let average_utilisation = if resource_count == 0 {
            0.0
        } else {
            let sum: f64 = self
                .resources
                .values()
                .map(ResourceDescriptor::utilisation)
                .sum();
            sum / resource_count as f64
        };

        PoolStats {
            total_capacity,
            total_allocated,
            resource_count,
            active_allocations,
            average_utilisation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cpu_resource() -> ResourceDescriptor {
        ResourceDescriptor::new(ResourceId::new("cpu"), 8, "CPU Cores", "cores")
    }

    fn gpu_resource() -> ResourceDescriptor {
        ResourceDescriptor::new(ResourceId::new("gpu"), 2, "GPU Devices", "devices")
    }

    #[test]
    fn test_resource_descriptor_available() {
        let mut r = cpu_resource();
        assert_eq!(r.available(), 8);
        r.allocated = 3;
        assert_eq!(r.available(), 5);
    }

    #[test]
    fn test_resource_descriptor_utilisation() {
        let mut r = cpu_resource();
        assert!((r.utilisation() - 0.0).abs() < f64::EPSILON);
        r.allocated = 4;
        assert!((r.utilisation() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_resource_descriptor_zero_capacity() {
        let r = ResourceDescriptor::new(ResourceId::new("empty"), 0, "Empty", "units");
        assert!((r.utilisation() - 0.0).abs() < f64::EPSILON);
        assert!(!r.can_allocate(1));
    }

    #[test]
    fn test_pool_register_and_count() {
        let mut pool = ResourcePool::new();
        assert_eq!(pool.resource_count(), 0);
        pool.register(cpu_resource());
        assert_eq!(pool.resource_count(), 1);
        pool.register(gpu_resource());
        assert_eq!(pool.resource_count(), 2);
    }

    #[test]
    fn test_pool_allocate_success() {
        let mut pool = ResourcePool::new();
        pool.register(cpu_resource());
        let req = ResourceRequest::new(ResourceId::new("cpu"), 4);
        let token = pool.allocate(&req).expect("should succeed in test");
        assert_eq!(token.amount, 4);
        assert_eq!(pool.active_allocations(), 1);
        let r = pool
            .get_resource(&ResourceId::new("cpu"))
            .expect("should succeed in test");
        assert_eq!(r.allocated, 4);
    }

    #[test]
    fn test_pool_allocate_insufficient() {
        let mut pool = ResourcePool::new();
        pool.register(cpu_resource());
        let req = ResourceRequest::new(ResourceId::new("cpu"), 100);
        let err = pool.allocate(&req).unwrap_err();
        assert!(matches!(err, PoolError::InsufficientCapacity { .. }));
    }

    #[test]
    fn test_pool_allocate_unknown_resource() {
        let mut pool = ResourcePool::new();
        let req = ResourceRequest::new(ResourceId::new("missing"), 1);
        let err = pool.allocate(&req).unwrap_err();
        assert!(matches!(err, PoolError::ResourceNotFound(_)));
    }

    #[test]
    fn test_pool_release_success() {
        let mut pool = ResourcePool::new();
        pool.register(cpu_resource());
        let req = ResourceRequest::new(ResourceId::new("cpu"), 4);
        let token = pool.allocate(&req).expect("should succeed in test");
        pool.release(token.token_id)
            .expect("should succeed in test");
        assert_eq!(pool.active_allocations(), 0);
        let r = pool
            .get_resource(&ResourceId::new("cpu"))
            .expect("should succeed in test");
        assert_eq!(r.allocated, 0);
    }

    #[test]
    fn test_pool_release_invalid_token() {
        let mut pool = ResourcePool::new();
        let err = pool.release(9999).unwrap_err();
        assert!(matches!(err, PoolError::InvalidToken(9999)));
    }

    #[test]
    fn test_pool_can_satisfy() {
        let mut pool = ResourcePool::new();
        pool.register(cpu_resource());
        let req_ok = ResourceRequest::new(ResourceId::new("cpu"), 4);
        assert!(pool.can_satisfy(&req_ok));
        let req_too_much = ResourceRequest::new(ResourceId::new("cpu"), 100);
        assert!(!pool.can_satisfy(&req_too_much));
    }

    #[test]
    fn test_pool_reset_all() {
        let mut pool = ResourcePool::new();
        pool.register(cpu_resource());
        pool.register(gpu_resource());
        let req1 = ResourceRequest::new(ResourceId::new("cpu"), 4);
        let req2 = ResourceRequest::new(ResourceId::new("gpu"), 1);
        let _t1 = pool.allocate(&req1).expect("should succeed in test");
        let _t2 = pool.allocate(&req2).expect("should succeed in test");
        assert_eq!(pool.active_allocations(), 2);
        pool.reset_all();
        assert_eq!(pool.active_allocations(), 0);
        for r in pool.snapshot() {
            assert_eq!(r.allocated, 0);
        }
    }

    #[test]
    fn test_pool_stats() {
        let mut pool = ResourcePool::new();
        pool.register(cpu_resource());
        pool.register(gpu_resource());
        let req = ResourceRequest::new(ResourceId::new("cpu"), 4);
        let _t = pool.allocate(&req).expect("should succeed in test");

        let stats = pool.stats();
        assert_eq!(stats.total_capacity, 10); // 8 + 2
        assert_eq!(stats.total_allocated, 4);
        assert_eq!(stats.resource_count, 2);
        assert_eq!(stats.active_allocations, 1);
        assert!(stats.average_utilisation > 0.0);
    }

    #[test]
    fn test_resource_request_builder() {
        let req = ResourceRequest::new(ResourceId::new("cpu"), 2)
            .with_priority(10)
            .with_requester("task-1");
        assert_eq!(req.priority, 10);
        assert_eq!(req.requester, "task-1");
    }

    #[test]
    fn test_multiple_allocations_same_resource() {
        let mut pool = ResourcePool::new();
        pool.register(cpu_resource());
        let req = ResourceRequest::new(ResourceId::new("cpu"), 3);
        let t1 = pool.allocate(&req).expect("should succeed in test");
        let t2 = pool.allocate(&req).expect("should succeed in test");
        assert_eq!(pool.active_allocations(), 2);
        let r = pool
            .get_resource(&ResourceId::new("cpu"))
            .expect("should succeed in test");
        assert_eq!(r.allocated, 6);

        // Third should fail (only 2 left)
        let req_too_much = ResourceRequest::new(ResourceId::new("cpu"), 3);
        assert!(pool.allocate(&req_too_much).is_err());

        pool.release(t1.token_id).expect("should succeed in test");
        // Now 5 available — 3 should work
        let _t3 = pool
            .allocate(&req_too_much)
            .expect("should succeed in test");
        assert_eq!(pool.active_allocations(), 2);
        pool.release(t2.token_id).expect("should succeed in test");
    }
}
