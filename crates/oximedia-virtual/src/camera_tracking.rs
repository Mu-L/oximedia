//! Camera tracking for virtual production.
//!
//! Provides low-pass filtering and pose interpolation for camera tracking
//! data used in LED volume and in-camera VFX workflows.

#![allow(dead_code)]

// ---------------------------------------------------------------------------
// Tracked pose
// ---------------------------------------------------------------------------

/// A single tracked camera pose sample.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackedPose {
    /// X position in metres.
    pub x: f64,
    /// Y position in metres.
    pub y: f64,
    /// Z position in metres.
    pub z: f64,
    /// Rotation around X axis (pitch) in degrees.
    pub rx: f64,
    /// Rotation around Y axis (yaw) in degrees.
    pub ry: f64,
    /// Rotation around Z axis (roll) in degrees.
    pub rz: f64,
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
}

impl TrackedPose {
    /// Create a new pose at position `(x, y, z)` with zero rotation.
    #[must_use]
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self {
            x,
            y,
            z,
            rx: 0.0,
            ry: 0.0,
            rz: 0.0,
            timestamp_ms: 0,
        }
    }

    /// Euclidean distance to another pose (translation only).
    #[must_use]
    pub fn distance_to(&self, other: &TrackedPose) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Linearly interpolate between `self` and `other`.
    ///
    /// `t = 0.0` returns a copy of `self`; `t = 1.0` returns a copy of `other`.
    #[must_use]
    pub fn interpolate(&self, other: &TrackedPose, t: f64) -> TrackedPose {
        let lerp = |a: f64, b: f64| a + (b - a) * t;
        TrackedPose {
            x: lerp(self.x, other.x),
            y: lerp(self.y, other.y),
            z: lerp(self.z, other.z),
            rx: lerp(self.rx, other.rx),
            ry: lerp(self.ry, other.ry),
            rz: lerp(self.rz, other.rz),
            timestamp_ms: lerp(self.timestamp_ms as f64, other.timestamp_ms as f64) as u64,
        }
    }
}

// ---------------------------------------------------------------------------
// Low-pass filter helper
// ---------------------------------------------------------------------------

/// Single-pole low-pass filter.
///
/// `alpha` is in `[0, 1]`: 0 = no update, 1 = no filtering.
#[must_use]
pub fn low_pass_filter(current: f64, prev: f64, alpha: f64) -> f64 {
    alpha * current + (1.0 - alpha) * prev
}

// ---------------------------------------------------------------------------
// Camera tracker
// ---------------------------------------------------------------------------

/// Real-time camera tracker with low-pass filtering.
#[derive(Debug)]
pub struct CameraTracker {
    /// History of received poses (most recent last).
    pub poses: Vec<TrackedPose>,
    /// Low-pass filter coefficient in `[0, 1]`.
    pub filter_alpha: f64,
}

impl CameraTracker {
    /// Create a new tracker with the given filter coefficient.
    #[must_use]
    pub fn new(filter_alpha: f64) -> Self {
        Self {
            poses: Vec::new(),
            filter_alpha,
        }
    }

    /// Submit a new raw pose, apply smoothing, store, and return the filtered pose.
    pub fn update(&mut self, pose: TrackedPose) -> TrackedPose {
        let smoothed = self.smooth_pose(&pose);
        self.poses.push(smoothed.clone());
        smoothed
    }

    /// Return a smoothed version of `raw` based on the previous pose (if any).
    #[must_use]
    pub fn smooth_pose(&self, raw: &TrackedPose) -> TrackedPose {
        let alpha = self.filter_alpha;
        match self.poses.last() {
            None => raw.clone(),
            Some(prev) => TrackedPose {
                x: low_pass_filter(raw.x, prev.x, alpha),
                y: low_pass_filter(raw.y, prev.y, alpha),
                z: low_pass_filter(raw.z, prev.z, alpha),
                rx: low_pass_filter(raw.rx, prev.rx, alpha),
                ry: low_pass_filter(raw.ry, prev.ry, alpha),
                rz: low_pass_filter(raw.rz, prev.rz, alpha),
                timestamp_ms: raw.timestamp_ms,
            },
        }
    }

    /// Estimate instantaneous velocity `(vx, vy, vz)` in m/ms from the last two poses.
    ///
    /// Returns `None` if fewer than two poses have been recorded, or if the
    /// timestamps are identical.
    #[must_use]
    pub fn velocity(&self) -> Option<(f64, f64, f64)> {
        if self.poses.len() < 2 {
            return None;
        }
        let len = self.poses.len();
        let a = &self.poses[len - 2];
        let b = &self.poses[len - 1];
        let dt = b.timestamp_ms as f64 - a.timestamp_ms as f64;
        if dt == 0.0 {
            return None;
        }
        Some(((b.x - a.x) / dt, (b.y - a.y) / dt, (b.z - a.z) / dt))
    }

    /// Return a reference to the most recently recorded (filtered) pose.
    #[must_use]
    pub fn latest(&self) -> Option<&TrackedPose> {
        self.poses.last()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracked_pose_new() {
        let p = TrackedPose::new(1.0, 2.0, 3.0);
        assert_eq!(p.x, 1.0);
        assert_eq!(p.y, 2.0);
        assert_eq!(p.z, 3.0);
        assert_eq!(p.rx, 0.0);
        assert_eq!(p.timestamp_ms, 0);
    }

    #[test]
    fn test_tracked_pose_distance_to_same() {
        let p = TrackedPose::new(0.0, 0.0, 0.0);
        assert_eq!(p.distance_to(&p), 0.0);
    }

    #[test]
    fn test_tracked_pose_distance_to_unit() {
        let a = TrackedPose::new(0.0, 0.0, 0.0);
        let b = TrackedPose::new(1.0, 0.0, 0.0);
        assert!((a.distance_to(&b) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_tracked_pose_distance_to_3d() {
        let a = TrackedPose::new(0.0, 0.0, 0.0);
        let b = TrackedPose::new(3.0, 4.0, 0.0);
        assert!((a.distance_to(&b) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_tracked_pose_interpolate_t0() {
        let a = TrackedPose::new(0.0, 0.0, 0.0);
        let b = TrackedPose::new(10.0, 10.0, 10.0);
        let mid = a.interpolate(&b, 0.0);
        assert!((mid.x - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_tracked_pose_interpolate_t1() {
        let a = TrackedPose::new(0.0, 0.0, 0.0);
        let b = TrackedPose::new(10.0, 10.0, 10.0);
        let mid = a.interpolate(&b, 1.0);
        assert!((mid.x - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_tracked_pose_interpolate_midpoint() {
        let a = TrackedPose::new(0.0, 0.0, 0.0);
        let b = TrackedPose::new(10.0, 20.0, 30.0);
        let mid = a.interpolate(&b, 0.5);
        assert!((mid.x - 5.0).abs() < 1e-10);
        assert!((mid.y - 10.0).abs() < 1e-10);
        assert!((mid.z - 15.0).abs() < 1e-10);
    }

    #[test]
    fn test_low_pass_filter_alpha_one() {
        // alpha=1 means no filtering
        assert_eq!(low_pass_filter(5.0, 0.0, 1.0), 5.0);
    }

    #[test]
    fn test_low_pass_filter_alpha_zero() {
        // alpha=0 means no update
        assert_eq!(low_pass_filter(5.0, 3.0, 0.0), 3.0);
    }

    #[test]
    fn test_low_pass_filter_half() {
        let v = low_pass_filter(10.0, 0.0, 0.5);
        assert!((v - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_camera_tracker_no_poses_initially() {
        let t = CameraTracker::new(0.9);
        assert!(t.latest().is_none());
        assert!(t.velocity().is_none());
    }

    #[test]
    fn test_camera_tracker_update_stores_pose() {
        let mut t = CameraTracker::new(1.0); // alpha=1 → no filtering
        let p = TrackedPose::new(1.0, 2.0, 3.0);
        t.update(p);
        let latest = t.latest().expect("should succeed in test");
        assert!((latest.x - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_camera_tracker_velocity() {
        let mut t = CameraTracker::new(1.0);
        let mut p1 = TrackedPose::new(0.0, 0.0, 0.0);
        p1.timestamp_ms = 0;
        let mut p2 = TrackedPose::new(1.0, 0.0, 0.0);
        p2.timestamp_ms = 1000;
        t.update(p1);
        t.update(p2);
        let (vx, vy, vz) = t.velocity().expect("should succeed in test");
        assert!((vx - 0.001).abs() < 1e-10); // 1m / 1000ms
        assert_eq!(vy, 0.0);
        assert_eq!(vz, 0.0);
    }
}
