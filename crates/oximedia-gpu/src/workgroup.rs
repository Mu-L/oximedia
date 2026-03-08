#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]
//! GPU workgroup configuration and dispatch sizing.
//!
//! This module provides utilities for computing optimal workgroup sizes
//! and dispatch dimensions for GPU compute shaders. Proper workgroup sizing
//! is critical for achieving good GPU utilization.

/// Maximum workgroup size per dimension on most GPUs.
const MAX_WORKGROUP_DIM: u32 = 1024;

/// Maximum total invocations per workgroup (typical limit).
const MAX_WORKGROUP_TOTAL: u32 = 1024;

/// Preferred warp/wavefront size for NVIDIA/AMD GPUs.
const WARP_SIZE: u32 = 32;

/// Workgroup size in 3 dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkgroupSize {
    /// Size in X dimension.
    pub x: u32,
    /// Size in Y dimension.
    pub y: u32,
    /// Size in Z dimension.
    pub z: u32,
}

impl WorkgroupSize {
    /// Create a new workgroup size.
    #[must_use]
    pub fn new(x: u32, y: u32, z: u32) -> Self {
        Self { x, y, z }
    }

    /// Create a 1D workgroup size.
    #[must_use]
    pub fn linear(size: u32) -> Self {
        Self {
            x: size,
            y: 1,
            z: 1,
        }
    }

    /// Create a 2D workgroup size.
    #[must_use]
    pub fn flat(x: u32, y: u32) -> Self {
        Self { x, y, z: 1 }
    }

    /// Total number of invocations in this workgroup.
    #[must_use]
    pub fn total(&self) -> u32 {
        self.x * self.y * self.z
    }

    /// Check if the workgroup size is valid (within typical limits).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.x > 0
            && self.y > 0
            && self.z > 0
            && self.x <= MAX_WORKGROUP_DIM
            && self.y <= MAX_WORKGROUP_DIM
            && self.z <= MAX_WORKGROUP_DIM
            && self.total() <= MAX_WORKGROUP_TOTAL
    }

    /// Check if the total size is a multiple of the warp size.
    #[must_use]
    pub fn is_warp_aligned(&self) -> bool {
        self.total() % WARP_SIZE == 0
    }
}

impl Default for WorkgroupSize {
    fn default() -> Self {
        Self { x: 8, y: 8, z: 1 }
    }
}

/// Dispatch dimensions for launching a compute shader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DispatchDimensions {
    /// Number of workgroups in X.
    pub groups_x: u32,
    /// Number of workgroups in Y.
    pub groups_y: u32,
    /// Number of workgroups in Z.
    pub groups_z: u32,
}

impl DispatchDimensions {
    /// Create new dispatch dimensions.
    #[must_use]
    pub fn new(groups_x: u32, groups_y: u32, groups_z: u32) -> Self {
        Self {
            groups_x,
            groups_y,
            groups_z,
        }
    }

    /// Create 1D dispatch dimensions.
    #[must_use]
    pub fn linear(groups: u32) -> Self {
        Self {
            groups_x: groups,
            groups_y: 1,
            groups_z: 1,
        }
    }

    /// Total number of workgroups.
    #[must_use]
    pub fn total_groups(&self) -> u64 {
        u64::from(self.groups_x) * u64::from(self.groups_y) * u64::from(self.groups_z)
    }

    /// Total number of invocations given a workgroup size.
    #[must_use]
    pub fn total_invocations(&self, workgroup: &WorkgroupSize) -> u64 {
        self.total_groups() * u64::from(workgroup.total())
    }
}

/// Strategy for choosing workgroup sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkgroupStrategy {
    /// Use a square workgroup (e.g. 16x16 for 2D).
    Square,
    /// Prefer wide workgroups (e.g. 256x1).
    Wide,
    /// Prefer tall workgroups (e.g. 1x256).
    Tall,
    /// Optimize for warp/wavefront alignment.
    WarpAligned,
    /// Use the smallest valid workgroup size.
    Minimal,
}

/// Compute optimal workgroup size and dispatch dimensions.
pub struct WorkgroupPlanner;

impl WorkgroupPlanner {
    /// Compute 1D dispatch dimensions for a linear problem.
    ///
    /// Returns `(workgroup_size, dispatch_dims)`.
    #[must_use]
    pub fn plan_1d(
        total_elements: u32,
        strategy: WorkgroupStrategy,
    ) -> (WorkgroupSize, DispatchDimensions) {
        let wg_size = match strategy {
            WorkgroupStrategy::WarpAligned => 256,
            WorkgroupStrategy::Minimal => 64,
            _ => 128,
        };
        let wg = WorkgroupSize::linear(wg_size);
        let groups = div_ceil(total_elements, wg_size);
        (wg, DispatchDimensions::linear(groups))
    }

    /// Compute 2D dispatch dimensions for an image-like problem.
    ///
    /// Returns `(workgroup_size, dispatch_dims)`.
    #[must_use]
    pub fn plan_2d(
        width: u32,
        height: u32,
        strategy: WorkgroupStrategy,
    ) -> (WorkgroupSize, DispatchDimensions) {
        let (wg_x, wg_y) = match strategy {
            WorkgroupStrategy::Square => (16, 16),
            WorkgroupStrategy::Wide => (32, 8),
            WorkgroupStrategy::Tall => (8, 32),
            WorkgroupStrategy::WarpAligned => (16, 16),
            WorkgroupStrategy::Minimal => (8, 8),
        };
        let wg = WorkgroupSize::flat(wg_x, wg_y);
        let groups_x = div_ceil(width, wg_x);
        let groups_y = div_ceil(height, wg_y);
        (wg, DispatchDimensions::new(groups_x, groups_y, 1))
    }

    /// Compute 3D dispatch dimensions.
    ///
    /// Returns `(workgroup_size, dispatch_dims)`.
    #[must_use]
    pub fn plan_3d(width: u32, height: u32, depth: u32) -> (WorkgroupSize, DispatchDimensions) {
        let wg = WorkgroupSize::new(8, 8, 4);
        let groups_x = div_ceil(width, 8);
        let groups_y = div_ceil(height, 8);
        let groups_z = div_ceil(depth, 4);
        (wg, DispatchDimensions::new(groups_x, groups_y, groups_z))
    }

    /// Estimate efficiency ratio of a dispatch (useful work / total work).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn efficiency(
        problem_size: (u32, u32),
        workgroup: &WorkgroupSize,
        dispatch: &DispatchDimensions,
    ) -> f64 {
        let useful = u64::from(problem_size.0) * u64::from(problem_size.1);
        let total = dispatch.total_invocations(workgroup);
        if total == 0 {
            return 0.0;
        }
        useful as f64 / total as f64
    }
}

/// Integer ceiling division.
fn div_ceil(a: u32, b: u32) -> u32 {
    a.div_ceil(b)
}

/// Shared memory layout descriptor for a workgroup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedMemoryLayout {
    /// Size in bytes per workgroup.
    pub size_bytes: u32,
    /// Alignment requirement in bytes.
    pub alignment: u32,
    /// Number of elements (stride-based).
    pub element_count: u32,
    /// Size per element in bytes.
    pub element_size: u32,
}

impl SharedMemoryLayout {
    /// Create a new shared memory layout.
    #[must_use]
    pub fn new(element_count: u32, element_size: u32, alignment: u32) -> Self {
        let aligned_element = round_up(element_size, alignment);
        Self {
            size_bytes: element_count * aligned_element,
            alignment,
            element_count,
            element_size,
        }
    }

    /// Create a layout for float data.
    #[must_use]
    pub fn floats(count: u32) -> Self {
        Self::new(count, 4, 4)
    }

    /// Create a layout for vec4 data.
    #[must_use]
    pub fn vec4s(count: u32) -> Self {
        Self::new(count, 16, 16)
    }

    /// Check if the layout fits within the typical shared memory limit (48 KB).
    #[must_use]
    pub fn fits_in_shared_memory(&self) -> bool {
        self.size_bytes <= 49152 // 48 * 1024
    }
}

/// Round a value up to a given alignment.
fn round_up(value: u32, alignment: u32) -> u32 {
    if alignment == 0 {
        return value;
    }
    value.div_ceil(alignment) * alignment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workgroup_size_default() {
        let wg = WorkgroupSize::default();
        assert_eq!(wg.x, 8);
        assert_eq!(wg.y, 8);
        assert_eq!(wg.z, 1);
        assert_eq!(wg.total(), 64);
    }

    #[test]
    fn test_workgroup_size_linear() {
        let wg = WorkgroupSize::linear(256);
        assert_eq!(wg.total(), 256);
        assert!(wg.is_valid());
        assert!(wg.is_warp_aligned());
    }

    #[test]
    fn test_workgroup_size_flat() {
        let wg = WorkgroupSize::flat(16, 16);
        assert_eq!(wg.total(), 256);
        assert!(wg.is_valid());
    }

    #[test]
    fn test_workgroup_size_3d() {
        let wg = WorkgroupSize::new(8, 8, 4);
        assert_eq!(wg.total(), 256);
        assert!(wg.is_valid());
    }

    #[test]
    fn test_workgroup_size_invalid_exceeds_max() {
        let wg = WorkgroupSize::new(1025, 1, 1);
        assert!(!wg.is_valid());
    }

    #[test]
    fn test_workgroup_size_invalid_exceeds_total() {
        let wg = WorkgroupSize::new(32, 64, 1);
        assert_eq!(wg.total(), 2048);
        assert!(!wg.is_valid());
    }

    #[test]
    fn test_dispatch_dimensions_linear() {
        let d = DispatchDimensions::linear(10);
        assert_eq!(d.total_groups(), 10);
    }

    #[test]
    fn test_dispatch_total_invocations() {
        let wg = WorkgroupSize::flat(16, 16);
        let d = DispatchDimensions::new(4, 4, 1);
        assert_eq!(d.total_invocations(&wg), 4096);
    }

    #[test]
    fn test_plan_1d() {
        let (wg, d) = WorkgroupPlanner::plan_1d(1000, WorkgroupStrategy::WarpAligned);
        assert_eq!(wg.x, 256);
        assert!(d.groups_x * wg.x >= 1000);
    }

    #[test]
    fn test_plan_2d_square() {
        let (wg, d) = WorkgroupPlanner::plan_2d(1920, 1080, WorkgroupStrategy::Square);
        assert_eq!(wg.x, 16);
        assert_eq!(wg.y, 16);
        assert!(d.groups_x * wg.x >= 1920);
        assert!(d.groups_y * wg.y >= 1080);
    }

    #[test]
    fn test_plan_2d_wide() {
        let (wg, d) = WorkgroupPlanner::plan_2d(3840, 2160, WorkgroupStrategy::Wide);
        assert_eq!(wg.x, 32);
        assert_eq!(wg.y, 8);
        assert!(d.groups_x * wg.x >= 3840);
        assert!(d.groups_y * wg.y >= 2160);
    }

    #[test]
    fn test_plan_3d() {
        let (wg, d) = WorkgroupPlanner::plan_3d(64, 64, 16);
        assert_eq!(wg.total(), 256);
        assert_eq!(d.groups_x, 8);
        assert_eq!(d.groups_y, 8);
        assert_eq!(d.groups_z, 4);
    }

    #[test]
    fn test_efficiency_perfect() {
        let wg = WorkgroupSize::flat(16, 16);
        let d = DispatchDimensions::new(2, 2, 1);
        let eff = WorkgroupPlanner::efficiency((32, 32), &wg, &d);
        assert!((eff - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_efficiency_partial() {
        let wg = WorkgroupSize::flat(16, 16);
        let d = DispatchDimensions::new(1, 1, 1);
        let eff = WorkgroupPlanner::efficiency((10, 10), &wg, &d);
        assert!(eff < 1.0);
        assert!(eff > 0.0);
    }

    #[test]
    fn test_shared_memory_floats() {
        let layout = SharedMemoryLayout::floats(256);
        assert_eq!(layout.size_bytes, 1024);
        assert!(layout.fits_in_shared_memory());
    }

    #[test]
    fn test_shared_memory_vec4s() {
        let layout = SharedMemoryLayout::vec4s(64);
        assert_eq!(layout.size_bytes, 1024);
        assert!(layout.fits_in_shared_memory());
    }

    #[test]
    fn test_shared_memory_exceeds_limit() {
        let layout = SharedMemoryLayout::new(50000, 4, 4);
        assert!(!layout.fits_in_shared_memory());
    }

    #[test]
    fn test_div_ceil() {
        assert_eq!(div_ceil(10, 3), 4);
        assert_eq!(div_ceil(9, 3), 3);
        assert_eq!(div_ceil(1, 256), 1);
    }

    #[test]
    fn test_round_up() {
        assert_eq!(round_up(5, 4), 8);
        assert_eq!(round_up(8, 4), 8);
        assert_eq!(round_up(0, 4), 0);
        assert_eq!(round_up(7, 0), 7);
    }

    #[test]
    fn test_warp_alignment() {
        let wg = WorkgroupSize::linear(64);
        assert!(wg.is_warp_aligned());
        let wg2 = WorkgroupSize::linear(33);
        assert!(!wg2.is_warp_aligned());
    }
}
