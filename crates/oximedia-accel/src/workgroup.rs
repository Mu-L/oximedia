//! Compute workgroup size management for GPU dispatch.
//!
//! Helps choose optimal local workgroup dimensions given device limits
//! and the logical problem size.  Provides helpers for 1-D, 2-D, and
//! 3-D dispatch as well as occupancy estimation.

#![allow(dead_code)]

use std::fmt;

/// Workgroup size in up to three dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkgroupSize {
    /// X dimension (columns / width).
    pub x: u32,
    /// Y dimension (rows / height).
    pub y: u32,
    /// Z dimension (depth / layers).
    pub z: u32,
}

impl WorkgroupSize {
    /// Create a 1-D workgroup size.
    #[must_use]
    pub fn new_1d(x: u32) -> Self {
        Self { x, y: 1, z: 1 }
    }

    /// Create a 2-D workgroup size.
    #[must_use]
    pub fn new_2d(x: u32, y: u32) -> Self {
        Self { x, y, z: 1 }
    }

    /// Create a 3-D workgroup size.
    #[must_use]
    pub fn new_3d(x: u32, y: u32, z: u32) -> Self {
        Self { x, y, z }
    }

    /// Total number of work items per workgroup.
    #[must_use]
    pub fn total_invocations(&self) -> u32 {
        self.x.saturating_mul(self.y).saturating_mul(self.z)
    }

    /// Whether this workgroup fits within the given device limits.
    #[must_use]
    pub fn fits_within(&self, limits: &DeviceLimits) -> bool {
        self.x <= limits.max_workgroup_size_x
            && self.y <= limits.max_workgroup_size_y
            && self.z <= limits.max_workgroup_size_z
            && self.total_invocations() <= limits.max_workgroup_invocations
    }
}

impl fmt::Display for WorkgroupSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.z > 1 {
            write!(f, "{}x{}x{}", self.x, self.y, self.z)
        } else if self.y > 1 {
            write!(f, "{}x{}", self.x, self.y)
        } else {
            write!(f, "{}", self.x)
        }
    }
}

/// Device limits relevant to workgroup dispatch.
#[derive(Debug, Clone, Copy)]
pub struct DeviceLimits {
    /// Maximum workgroup size in X.
    pub max_workgroup_size_x: u32,
    /// Maximum workgroup size in Y.
    pub max_workgroup_size_y: u32,
    /// Maximum workgroup size in Z.
    pub max_workgroup_size_z: u32,
    /// Maximum total invocations per workgroup.
    pub max_workgroup_invocations: u32,
    /// Maximum dispatch count in X.
    pub max_dispatch_x: u32,
    /// Maximum dispatch count in Y.
    pub max_dispatch_y: u32,
    /// Maximum dispatch count in Z.
    pub max_dispatch_z: u32,
}

impl Default for DeviceLimits {
    fn default() -> Self {
        Self {
            max_workgroup_size_x: 1024,
            max_workgroup_size_y: 1024,
            max_workgroup_size_z: 64,
            max_workgroup_invocations: 1024,
            max_dispatch_x: 65535,
            max_dispatch_y: 65535,
            max_dispatch_z: 65535,
        }
    }
}

/// Dispatch dimensions (number of workgroups to launch per axis).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DispatchSize {
    /// Number of workgroups in X.
    pub x: u32,
    /// Number of workgroups in Y.
    pub y: u32,
    /// Number of workgroups in Z.
    pub z: u32,
}

impl DispatchSize {
    /// Total number of workgroups.
    #[must_use]
    pub fn total_workgroups(&self) -> u64 {
        u64::from(self.x) * u64::from(self.y) * u64::from(self.z)
    }

    /// Total work items = total workgroups * invocations per workgroup.
    #[must_use]
    pub fn total_invocations(&self, wg: &WorkgroupSize) -> u64 {
        self.total_workgroups() * u64::from(wg.total_invocations())
    }

    /// Whether this dispatch fits within device limits.
    #[must_use]
    pub fn fits_within(&self, limits: &DeviceLimits) -> bool {
        self.x <= limits.max_dispatch_x
            && self.y <= limits.max_dispatch_y
            && self.z <= limits.max_dispatch_z
    }
}

impl fmt::Display for DispatchSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dispatch({}x{}x{})", self.x, self.y, self.z)
    }
}

/// Compute the dispatch size needed to cover `problem_size` elements
/// given `workgroup_size` elements per group in each dimension.
#[must_use]
pub fn compute_dispatch_1d(problem_size: u32, workgroup_x: u32) -> DispatchSize {
    let x = div_ceil(problem_size, workgroup_x.max(1));
    DispatchSize { x, y: 1, z: 1 }
}

/// Compute the 2-D dispatch size for an image-like problem.
#[must_use]
pub fn compute_dispatch_2d(width: u32, height: u32, wg: &WorkgroupSize) -> DispatchSize {
    let x = div_ceil(width, wg.x.max(1));
    let y = div_ceil(height, wg.y.max(1));
    DispatchSize { x, y, z: 1 }
}

/// Compute the 3-D dispatch size.
#[must_use]
pub fn compute_dispatch_3d(
    width: u32,
    height: u32,
    depth: u32,
    wg: &WorkgroupSize,
) -> DispatchSize {
    let x = div_ceil(width, wg.x.max(1));
    let y = div_ceil(height, wg.y.max(1));
    let z = div_ceil(depth, wg.z.max(1));
    DispatchSize { x, y, z }
}

/// Choose an optimal 2-D workgroup size for the given image dimensions
/// and device limits, preferring square-ish groups.
#[must_use]
pub fn optimal_2d_workgroup(_width: u32, _height: u32, limits: &DeviceLimits) -> WorkgroupSize {
    // Try common sizes from largest to smallest
    let candidates: &[(u32, u32)] = &[(16, 16), (32, 8), (8, 32), (16, 8), (8, 16), (8, 8), (4, 4)];
    for &(x, y) in candidates {
        let wg = WorkgroupSize::new_2d(x, y);
        if wg.fits_within(limits) {
            return wg;
        }
    }
    WorkgroupSize::new_2d(1, 1)
}

/// Estimated occupancy ratio given workgroup size and device limits.
///
/// This is a rough heuristic: higher invocation counts relative to the
/// max tend to yield better occupancy on typical GPUs.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn estimate_occupancy(wg: &WorkgroupSize, limits: &DeviceLimits) -> f64 {
    if limits.max_workgroup_invocations == 0 {
        return 0.0;
    }
    let ratio = f64::from(wg.total_invocations()) / f64::from(limits.max_workgroup_invocations);
    ratio.min(1.0)
}

/// Integer ceiling division without overflow for small values.
#[must_use]
fn div_ceil(a: u32, b: u32) -> u32 {
    if b == 0 {
        return 0;
    }
    a.div_ceil(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workgroup_1d() {
        let wg = WorkgroupSize::new_1d(256);
        assert_eq!(wg.total_invocations(), 256);
        assert_eq!(wg.to_string(), "256");
    }

    #[test]
    fn test_workgroup_2d() {
        let wg = WorkgroupSize::new_2d(16, 16);
        assert_eq!(wg.total_invocations(), 256);
        assert_eq!(wg.to_string(), "16x16");
    }

    #[test]
    fn test_workgroup_3d() {
        let wg = WorkgroupSize::new_3d(8, 8, 4);
        assert_eq!(wg.total_invocations(), 256);
        assert_eq!(wg.to_string(), "8x8x4");
    }

    #[test]
    fn test_fits_within() {
        let limits = DeviceLimits::default();
        let wg = WorkgroupSize::new_2d(16, 16);
        assert!(wg.fits_within(&limits));

        let big = WorkgroupSize::new_2d(2048, 1);
        assert!(!big.fits_within(&limits));
    }

    #[test]
    fn test_dispatch_1d() {
        let d = compute_dispatch_1d(1000, 256);
        assert_eq!(d.x, 4); // ceil(1000/256) = 4
        assert_eq!(d.y, 1);
    }

    #[test]
    fn test_dispatch_2d() {
        let wg = WorkgroupSize::new_2d(16, 16);
        let d = compute_dispatch_2d(1920, 1080, &wg);
        assert_eq!(d.x, 120); // ceil(1920/16) = 120
        assert_eq!(d.y, 68); // ceil(1080/16) = 67.5 -> 68
    }

    #[test]
    fn test_dispatch_3d() {
        let wg = WorkgroupSize::new_3d(8, 8, 4);
        let d = compute_dispatch_3d(64, 64, 16, &wg);
        assert_eq!(d.x, 8);
        assert_eq!(d.y, 8);
        assert_eq!(d.z, 4);
    }

    #[test]
    fn test_dispatch_total_invocations() {
        let wg = WorkgroupSize::new_1d(64);
        let d = compute_dispatch_1d(256, 64);
        assert_eq!(d.total_invocations(&wg), 256);
    }

    #[test]
    fn test_dispatch_display() {
        let d = DispatchSize { x: 3, y: 4, z: 1 };
        assert_eq!(d.to_string(), "dispatch(3x4x1)");
    }

    #[test]
    fn test_optimal_2d_workgroup() {
        let limits = DeviceLimits::default();
        let wg = optimal_2d_workgroup(1920, 1080, &limits);
        assert!(wg.fits_within(&limits));
        assert_eq!(wg.x, 16);
        assert_eq!(wg.y, 16);
    }

    #[test]
    fn test_optimal_2d_restricted_limits() {
        let limits = DeviceLimits {
            max_workgroup_invocations: 32,
            ..DeviceLimits::default()
        };
        let wg = optimal_2d_workgroup(1920, 1080, &limits);
        assert!(wg.fits_within(&limits));
        assert!(wg.total_invocations() <= 32);
    }

    #[test]
    fn test_estimate_occupancy() {
        let limits = DeviceLimits::default();
        let wg = WorkgroupSize::new_1d(512);
        let occ = estimate_occupancy(&wg, &limits);
        assert!((occ - 0.5).abs() < 1e-9); // 512/1024 = 0.5
    }

    #[test]
    fn test_estimate_occupancy_capped() {
        let limits = DeviceLimits {
            max_workgroup_invocations: 128,
            ..DeviceLimits::default()
        };
        let wg = WorkgroupSize::new_1d(256);
        // 256/128 = 2.0, capped to 1.0
        let occ = estimate_occupancy(&wg, &limits);
        assert!((occ - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_div_ceil_basic() {
        assert_eq!(div_ceil(10, 3), 4);
        assert_eq!(div_ceil(9, 3), 3);
        assert_eq!(div_ceil(0, 5), 0);
        assert_eq!(div_ceil(5, 0), 0);
    }
}
