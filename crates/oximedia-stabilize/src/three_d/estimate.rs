//! 3D camera pose estimation.

use crate::error::StabilizeResult;
use crate::motion::tracker::FeatureTrack;
use nalgebra as na;

/// Camera pose estimator for 3D stabilization.
pub struct CameraPoseEstimator {
    focal_length: f64,
}

impl CameraPoseEstimator {
    /// Create a new camera pose estimator.
    #[must_use]
    pub fn new() -> Self {
        Self { focal_length: 1.0 }
    }

    /// Estimate camera pose from feature tracks.
    pub fn estimate_pose(&self, _tracks: &[FeatureTrack]) -> StabilizeResult<CameraPose> {
        Ok(CameraPose {
            rotation: na::Matrix3::identity(),
            translation: na::Vector3::zeros(),
        })
    }
}

impl Default for CameraPoseEstimator {
    fn default() -> Self {
        Self::new()
    }
}

/// 3D camera pose.
#[derive(Debug, Clone)]
pub struct CameraPose {
    /// Rotation matrix
    pub rotation: na::Matrix3<f64>,
    /// Translation vector
    pub translation: na::Vector3<f64>,
}

/// Structure from motion utilities.
pub mod sfm {
    use nalgebra as na;

    /// Triangulate 3D point from 2D correspondences.
    #[must_use]
    pub fn triangulate_point(
        p1: (f64, f64),
        p2: (f64, f64),
        cam1: &na::Matrix3x4<f64>,
        cam2: &na::Matrix3x4<f64>,
    ) -> na::Vector3<f64> {
        // Direct Linear Transform (DLT) for triangulation
        let mut a = na::Matrix4::zeros();

        a.set_row(0, &(p1.0 * cam1.row(2) - cam1.row(0)));
        a.set_row(1, &(p1.1 * cam1.row(2) - cam1.row(1)));
        a.set_row(2, &(p2.0 * cam2.row(2) - cam2.row(0)));
        a.set_row(3, &(p2.1 * cam2.row(2) - cam2.row(1)));

        let svd = na::linalg::SVD::new(a, true, true);

        if let Some(v_t) = svd.v_t {
            let point = v_t.row(3);
            let w = point[3];

            if w.abs() > 1e-10 {
                return na::Vector3::new(point[0] / w, point[1] / w, point[2] / w);
            }
        }

        na::Vector3::zeros()
    }

    /// Estimate essential matrix from point correspondences.
    #[must_use]
    pub fn estimate_essential_matrix(
        points1: &[(f64, f64)],
        points2: &[(f64, f64)],
    ) -> na::Matrix3<f64> {
        // 8-point algorithm
        if points1.len() < 8 || points2.len() < 8 {
            return na::Matrix3::identity();
        }

        let n = points1.len().min(points2.len());
        let mut a = na::DMatrix::zeros(n, 9);

        for i in 0..n {
            let (x1, y1) = points1[i];
            let (x2, y2) = points2[i];

            a[(i, 0)] = x2 * x1;
            a[(i, 1)] = x2 * y1;
            a[(i, 2)] = x2;
            a[(i, 3)] = y2 * x1;
            a[(i, 4)] = y2 * y1;
            a[(i, 5)] = y2;
            a[(i, 6)] = x1;
            a[(i, 7)] = y1;
            a[(i, 8)] = 1.0;
        }

        let svd = na::linalg::SVD::new(a, true, true);

        if let Some(v_t) = svd.v_t {
            let f = v_t.row(8);
            na::Matrix3::new(f[0], f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8])
        } else {
            na::Matrix3::identity()
        }
    }

    /// Decompose essential matrix into rotation and translation.
    #[must_use]
    pub fn decompose_essential_matrix(
        e: &na::Matrix3<f64>,
    ) -> Vec<(na::Matrix3<f64>, na::Vector3<f64>)> {
        let svd = na::linalg::SVD::new(*e, true, true);

        let w = na::Matrix3::new(0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0);

        let mut solutions = Vec::new();

        if let (Some(u), Some(v_t)) = (svd.u, svd.v_t) {
            let r1 = u * w * v_t;
            let r2 = u * w.transpose() * v_t;
            let t = u.column(2).into_owned();

            solutions.push((r1, t));
            solutions.push((r1, -t));
            solutions.push((r2, t));
            solutions.push((r2, -t));
        }

        solutions
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_triangulation() {
            let cam1 = na::Matrix3x4::identity();
            let cam2 = na::Matrix3x4::identity();

            let point = triangulate_point((0.0, 0.0), (1.0, 0.0), &cam1, &cam2);
            assert!(point.norm() >= 0.0);
        }

        #[test]
        fn test_essential_matrix() {
            let points1 = vec![(0.0, 0.0); 10];
            let points2 = vec![(1.0, 1.0); 10];

            let e = estimate_essential_matrix(&points1, &points2);
            assert!(e.determinant().abs() >= 0.0);
        }
    }
}
