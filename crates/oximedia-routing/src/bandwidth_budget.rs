#![allow(dead_code)]

//! Bandwidth budgeting for media routing.
//!
//! Tracks available link bandwidth and allocates capacity to media
//! streams, preventing over-subscription on routing links.

/// Unit of bandwidth measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandwidthUnit {
    /// Bits per second.
    Bps,
    /// Kilobits per second.
    Kbps,
    /// Megabits per second.
    Mbps,
    /// Gigabits per second.
    Gbps,
}

impl BandwidthUnit {
    /// Convert a value in this unit to bits per second.
    #[allow(clippy::cast_precision_loss)]
    pub fn to_bps(self, value: f64) -> f64 {
        match self {
            Self::Bps => value,
            Self::Kbps => value * 1_000.0,
            Self::Mbps => value * 1_000_000.0,
            Self::Gbps => value * 1_000_000_000.0,
        }
    }

    /// Convert bits per second to this unit.
    #[allow(clippy::cast_precision_loss)]
    pub fn from_bps(self, bps: f64) -> f64 {
        match self {
            Self::Bps => bps,
            Self::Kbps => bps / 1_000.0,
            Self::Mbps => bps / 1_000_000.0,
            Self::Gbps => bps / 1_000_000_000.0,
        }
    }
}

/// An allocation request for a stream.
#[derive(Debug, Clone)]
pub struct AllocationRequest {
    /// Stream identifier.
    pub stream_id: String,
    /// Required bandwidth in bits per second.
    pub required_bps: f64,
    /// Priority (lower number = higher priority).
    pub priority: u32,
}

impl AllocationRequest {
    /// Create a new allocation request.
    pub fn new(stream_id: impl Into<String>, required_bps: f64, priority: u32) -> Self {
        Self {
            stream_id: stream_id.into(),
            required_bps,
            priority,
        }
    }
}

/// Outcome of an allocation attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationResult {
    /// Successfully allocated.
    Granted,
    /// Not enough bandwidth remaining.
    Denied,
    /// Already allocated for this stream.
    AlreadyAllocated,
}

/// Record of an active allocation.
#[derive(Debug, Clone)]
struct ActiveAllocation {
    stream_id: String,
    allocated_bps: f64,
    priority: u32,
}

/// Manages a fixed bandwidth budget for a link or path.
#[derive(Debug, Clone)]
pub struct BandwidthBudget {
    /// Total capacity in bps.
    capacity_bps: f64,
    /// Active allocations.
    allocations: Vec<ActiveAllocation>,
}

impl BandwidthBudget {
    /// Create a new budget with the given capacity.
    pub fn new(capacity: f64, unit: BandwidthUnit) -> Self {
        Self {
            capacity_bps: unit.to_bps(capacity),
            allocations: Vec::new(),
        }
    }

    /// Total capacity in bps.
    pub fn capacity_bps(&self) -> f64 {
        self.capacity_bps
    }

    /// Currently allocated bandwidth in bps.
    pub fn allocated_bps(&self) -> f64 {
        self.allocations.iter().map(|a| a.allocated_bps).sum()
    }

    /// Remaining available bandwidth in bps.
    pub fn available_bps(&self) -> f64 {
        (self.capacity_bps - self.allocated_bps()).max(0.0)
    }

    /// Utilization as a fraction 0.0..=1.0.
    #[allow(clippy::cast_precision_loss)]
    pub fn utilization(&self) -> f64 {
        if self.capacity_bps <= 0.0 {
            return 0.0;
        }
        (self.allocated_bps() / self.capacity_bps).min(1.0)
    }

    /// Number of active allocations.
    pub fn allocation_count(&self) -> usize {
        self.allocations.len()
    }

    /// Try to allocate bandwidth for a stream.
    pub fn allocate(&mut self, request: &AllocationRequest) -> AllocationResult {
        // Check for duplicate
        if self
            .allocations
            .iter()
            .any(|a| a.stream_id == request.stream_id)
        {
            return AllocationResult::AlreadyAllocated;
        }

        if request.required_bps > self.available_bps() {
            return AllocationResult::Denied;
        }

        self.allocations.push(ActiveAllocation {
            stream_id: request.stream_id.clone(),
            allocated_bps: request.required_bps,
            priority: request.priority,
        });

        AllocationResult::Granted
    }

    /// Release an allocation by stream id. Returns the released bps or `None`.
    pub fn release(&mut self, stream_id: &str) -> Option<f64> {
        if let Some(pos) = self
            .allocations
            .iter()
            .position(|a| a.stream_id == stream_id)
        {
            let removed = self.allocations.remove(pos);
            Some(removed.allocated_bps)
        } else {
            None
        }
    }

    /// Release all allocations.
    pub fn release_all(&mut self) {
        self.allocations.clear();
    }

    /// Check if a stream has an active allocation.
    pub fn is_allocated(&self, stream_id: &str) -> bool {
        self.allocations.iter().any(|a| a.stream_id == stream_id)
    }

    /// Try to preempt the lowest-priority allocation to make room.
    /// Returns the preempted stream id if successful.
    pub fn preempt_lowest_priority(&mut self, request: &AllocationRequest) -> Option<String> {
        if self.allocations.is_empty() {
            return None;
        }

        // Find lowest priority (highest number) that is lower priority than request
        let worst_idx = self
            .allocations
            .iter()
            .enumerate()
            .filter(|(_, a)| a.priority > request.priority)
            .max_by_key(|(_, a)| a.priority)
            .map(|(i, _)| i);

        if let Some(idx) = worst_idx {
            let freed_bps = self.allocations[idx].allocated_bps;
            let freed_id = self.allocations[idx].stream_id.clone();
            self.allocations.remove(idx);

            // Now check if there's room
            if request.required_bps <= self.available_bps() {
                self.allocations.push(ActiveAllocation {
                    stream_id: request.stream_id.clone(),
                    allocated_bps: request.required_bps,
                    priority: request.priority,
                });
                return Some(freed_id);
            }
            // Put it back if still not enough room
            self.allocations.push(ActiveAllocation {
                stream_id: freed_id,
                allocated_bps: freed_bps,
                priority: self.allocations.len() as u32, // restore doesn't need exact priority
            });
        }
        None
    }
}

/// Allocator that manages budgets across multiple links.
#[derive(Debug, Clone)]
pub struct BudgetAllocator {
    budgets: Vec<(String, BandwidthBudget)>,
}

impl BudgetAllocator {
    /// Create a new allocator.
    pub fn new() -> Self {
        Self {
            budgets: Vec::new(),
        }
    }

    /// Add a link budget.
    pub fn add_link(&mut self, name: impl Into<String>, budget: BandwidthBudget) {
        self.budgets.push((name.into(), budget));
    }

    /// Number of managed links.
    pub fn link_count(&self) -> usize {
        self.budgets.len()
    }

    /// Get a budget by link name.
    pub fn get_budget(&self, name: &str) -> Option<&BandwidthBudget> {
        self.budgets.iter().find(|(n, _)| n == name).map(|(_, b)| b)
    }

    /// Get a mutable budget by link name.
    pub fn get_budget_mut(&mut self, name: &str) -> Option<&mut BandwidthBudget> {
        self.budgets
            .iter_mut()
            .find(|(n, _)| n == name)
            .map(|(_, b)| b)
    }

    /// Total capacity across all links.
    pub fn total_capacity_bps(&self) -> f64 {
        self.budgets.iter().map(|(_, b)| b.capacity_bps()).sum()
    }

    /// Total available across all links.
    pub fn total_available_bps(&self) -> f64 {
        self.budgets.iter().map(|(_, b)| b.available_bps()).sum()
    }
}

impl Default for BudgetAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bandwidth_unit_to_bps() {
        assert!((BandwidthUnit::Kbps.to_bps(1.0) - 1000.0).abs() < 0.01);
        assert!((BandwidthUnit::Mbps.to_bps(1.0) - 1_000_000.0).abs() < 0.01);
        assert!((BandwidthUnit::Gbps.to_bps(1.0) - 1_000_000_000.0).abs() < 0.01);
    }

    #[test]
    fn test_bandwidth_unit_from_bps() {
        assert!((BandwidthUnit::Mbps.from_bps(1_000_000.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_new_budget_capacity() {
        let b = BandwidthBudget::new(10.0, BandwidthUnit::Gbps);
        assert!((b.capacity_bps() - 10_000_000_000.0).abs() < 1.0);
    }

    #[test]
    fn test_allocate_success() {
        let mut b = BandwidthBudget::new(100.0, BandwidthUnit::Mbps);
        let req = AllocationRequest::new("s1", 50_000_000.0, 1);
        assert_eq!(b.allocate(&req), AllocationResult::Granted);
        assert_eq!(b.allocation_count(), 1);
    }

    #[test]
    fn test_allocate_denied() {
        let mut b = BandwidthBudget::new(10.0, BandwidthUnit::Mbps);
        let req = AllocationRequest::new("big", 20_000_000.0, 1);
        assert_eq!(b.allocate(&req), AllocationResult::Denied);
    }

    #[test]
    fn test_allocate_duplicate() {
        let mut b = BandwidthBudget::new(100.0, BandwidthUnit::Mbps);
        let req = AllocationRequest::new("s1", 10_000_000.0, 1);
        b.allocate(&req);
        assert_eq!(b.allocate(&req), AllocationResult::AlreadyAllocated);
    }

    #[test]
    fn test_release() {
        let mut b = BandwidthBudget::new(100.0, BandwidthUnit::Mbps);
        let req = AllocationRequest::new("s1", 30_000_000.0, 1);
        b.allocate(&req);
        let freed = b.release("s1");
        assert!(freed.is_some());
        assert!((freed.expect("should succeed in test") - 30_000_000.0).abs() < 1.0);
        assert_eq!(b.allocation_count(), 0);
    }

    #[test]
    fn test_release_nonexistent() {
        let mut b = BandwidthBudget::new(100.0, BandwidthUnit::Mbps);
        assert!(b.release("ghost").is_none());
    }

    #[test]
    fn test_available_bps() {
        let mut b = BandwidthBudget::new(100.0, BandwidthUnit::Mbps);
        let req = AllocationRequest::new("s1", 40_000_000.0, 1);
        b.allocate(&req);
        assert!((b.available_bps() - 60_000_000.0).abs() < 1.0);
    }

    #[test]
    fn test_utilization() {
        let mut b = BandwidthBudget::new(100.0, BandwidthUnit::Mbps);
        let req = AllocationRequest::new("s1", 50_000_000.0, 1);
        b.allocate(&req);
        assert!((b.utilization() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_utilization_zero_capacity() {
        let b = BandwidthBudget::new(0.0, BandwidthUnit::Bps);
        assert!((b.utilization()).abs() < 0.01);
    }

    #[test]
    fn test_release_all() {
        let mut b = BandwidthBudget::new(100.0, BandwidthUnit::Mbps);
        b.allocate(&AllocationRequest::new("a", 10_000_000.0, 1));
        b.allocate(&AllocationRequest::new("b", 20_000_000.0, 2));
        b.release_all();
        assert_eq!(b.allocation_count(), 0);
    }

    #[test]
    fn test_is_allocated() {
        let mut b = BandwidthBudget::new(100.0, BandwidthUnit::Mbps);
        b.allocate(&AllocationRequest::new("s1", 10_000_000.0, 1));
        assert!(b.is_allocated("s1"));
        assert!(!b.is_allocated("s2"));
    }

    #[test]
    fn test_budget_allocator_multi_link() {
        let mut alloc = BudgetAllocator::new();
        alloc.add_link("link1", BandwidthBudget::new(10.0, BandwidthUnit::Gbps));
        alloc.add_link("link2", BandwidthBudget::new(1.0, BandwidthUnit::Gbps));
        assert_eq!(alloc.link_count(), 2);
        assert!((alloc.total_capacity_bps() - 11_000_000_000.0).abs() < 1.0);
    }

    #[test]
    fn test_budget_allocator_get_budget() {
        let mut alloc = BudgetAllocator::new();
        alloc.add_link("primary", BandwidthBudget::new(10.0, BandwidthUnit::Gbps));
        assert!(alloc.get_budget("primary").is_some());
        assert!(alloc.get_budget("missing").is_none());
    }

    #[test]
    fn test_preempt_lowest_priority() {
        let mut b = BandwidthBudget::new(100.0, BandwidthUnit::Mbps);
        // Fill up with low priority
        b.allocate(&AllocationRequest::new("low", 80_000_000.0, 10));
        // High priority request
        let req = AllocationRequest::new("high", 80_000_000.0, 1);
        let preempted = b.preempt_lowest_priority(&req);
        assert_eq!(preempted, Some("low".to_string()));
        assert!(b.is_allocated("high"));
        assert!(!b.is_allocated("low"));
    }
}
