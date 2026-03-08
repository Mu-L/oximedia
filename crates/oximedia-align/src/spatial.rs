//! Spatial registration and geometric alignment.
//!
//! This module provides tools for aligning images geometrically:
//!
//! - Homography estimation
//! - Perspective transformation
//! - RANSAC for robust fitting
//! - Affine transformation

use crate::features::MatchPair;
use crate::{AlignError, AlignResult, Point2D};
use nalgebra::{Matrix3, Vector3};

/// 3x3 homography matrix for perspective transformation
#[derive(Debug, Clone)]
pub struct Homography {
    /// The 3x3 transformation matrix
    pub matrix: Matrix3<f64>,
}

impl Homography {
    /// Create a new homography from a matrix
    #[must_use]
    pub fn new(matrix: Matrix3<f64>) -> Self {
        Self { matrix }
    }

    /// Create identity homography
    #[must_use]
    pub fn identity() -> Self {
        Self {
            matrix: Matrix3::identity(),
        }
    }

    /// Transform a point
    #[must_use]
    pub fn transform(&self, point: &Point2D) -> Point2D {
        let p = Vector3::new(point.x, point.y, 1.0);
        let transformed = self.matrix * p;

        if transformed[2].abs() > f64::EPSILON {
            Point2D::new(
                transformed[0] / transformed[2],
                transformed[1] / transformed[2],
            )
        } else {
            *point
        }
    }

    /// Compute inverse homography
    ///
    /// # Errors
    /// Returns error if matrix is singular
    pub fn inverse(&self) -> AlignResult<Self> {
        self.matrix
            .try_inverse()
            .map(Self::new)
            .ok_or_else(|| AlignError::NumericalError("Singular matrix".to_string()))
    }

    /// Compose two homographies
    #[must_use]
    pub fn compose(&self, other: &Self) -> Self {
        Self::new(self.matrix * other.matrix)
    }
}

/// Configuration for RANSAC
#[derive(Debug, Clone)]
pub struct RansacConfig {
    /// Distance threshold for inliers
    pub threshold: f64,
    /// Maximum number of iterations
    pub max_iterations: usize,
    /// Minimum number of inliers required
    pub min_inliers: usize,
}

impl Default for RansacConfig {
    fn default() -> Self {
        Self {
            threshold: 3.0,
            max_iterations: 1000,
            min_inliers: 8,
        }
    }
}

/// Homography estimator using RANSAC
pub struct HomographyEstimator {
    /// RANSAC configuration
    pub config: RansacConfig,
}

impl HomographyEstimator {
    /// Create a new homography estimator
    #[must_use]
    pub fn new(config: RansacConfig) -> Self {
        Self { config }
    }

    /// Estimate homography from matched points using RANSAC
    ///
    /// # Errors
    /// Returns error if insufficient matches or estimation fails
    #[allow(clippy::too_many_lines)]
    pub fn estimate(&self, matches: &[MatchPair]) -> AlignResult<(Homography, Vec<bool>)> {
        if matches.len() < 4 {
            return Err(AlignError::InsufficientData(
                "Need at least 4 matches for homography".to_string(),
            ));
        }

        let mut best_inliers = Vec::new();
        let mut best_homography = None;
        let mut best_inlier_count = 0;

        // RANSAC iterations
        for _ in 0..self.config.max_iterations {
            // Sample 4 random matches
            let sample = self.sample_matches(matches, 4);

            // Estimate homography from 4 points
            if let Ok(h) = self.estimate_from_4_points(&sample) {
                // Count inliers
                let inliers = self.find_inliers(&h, matches);
                let inlier_count = inliers.iter().filter(|&&x| x).count();

                if inlier_count > best_inlier_count {
                    best_inlier_count = inlier_count;
                    best_inliers = inliers;
                    best_homography = Some(h);

                    // Early termination if we have enough inliers
                    if inlier_count >= self.config.min_inliers.max(matches.len() * 80 / 100) {
                        break;
                    }
                }
            }
        }

        if best_inlier_count < self.config.min_inliers {
            return Err(AlignError::NoSolution(format!(
                "Insufficient inliers: {} < {}",
                best_inlier_count, self.config.min_inliers
            )));
        }

        let homography = best_homography
            .ok_or_else(|| AlignError::NoSolution("No valid homography found".to_string()))?;

        // Refine with all inliers
        let inlier_matches: Vec<&MatchPair> = matches
            .iter()
            .zip(&best_inliers)
            .filter(|(_, &is_inlier)| is_inlier)
            .map(|(m, _)| m)
            .collect();

        let refined = self.refine_homography(&homography, &inlier_matches)?;

        Ok((refined, best_inliers))
    }

    /// Sample N random matches
    fn sample_matches(&self, matches: &[MatchPair], n: usize) -> Vec<MatchPair> {
        // Simple deterministic sampling (in production, use proper PRNG)
        let step = matches.len() / n;
        (0..n)
            .map(|i| matches[(i * step) % matches.len()].clone())
            .collect()
    }

    /// Estimate homography from exactly 4 point correspondences
    #[allow(clippy::similar_names)]
    fn estimate_from_4_points(&self, matches: &[MatchPair]) -> AlignResult<Homography> {
        if matches.len() != 4 {
            return Err(AlignError::InvalidConfig(
                "Need exactly 4 points".to_string(),
            ));
        }

        // Build the 8x9 matrix for DLT (Direct Linear Transform)
        let mut a = nalgebra::DMatrix::zeros(8, 9);

        for (i, m) in matches.iter().enumerate() {
            let x1 = m.point1.x;
            let y1 = m.point1.y;
            let x2 = m.point2.x;
            let y2 = m.point2.y;

            // First equation for this correspondence
            a[(i * 2, 0)] = -x1;
            a[(i * 2, 1)] = -y1;
            a[(i * 2, 2)] = -1.0;
            a[(i * 2, 6)] = x2 * x1;
            a[(i * 2, 7)] = x2 * y1;
            a[(i * 2, 8)] = x2;

            // Second equation
            a[(i * 2 + 1, 3)] = -x1;
            a[(i * 2 + 1, 4)] = -y1;
            a[(i * 2 + 1, 5)] = -1.0;
            a[(i * 2 + 1, 6)] = y2 * x1;
            a[(i * 2 + 1, 7)] = y2 * y1;
            a[(i * 2 + 1, 8)] = y2;
        }

        // Solve using SVD
        let svd = a.svd(true, true);
        let v = svd
            .v_t
            .ok_or_else(|| AlignError::NumericalError("SVD failed to compute V".to_string()))?;

        // Last row of V is the solution
        let h_vec = v.row(8);

        if h_vec[8].abs() < 1e-10 {
            return Err(AlignError::NumericalError(
                "Degenerate solution".to_string(),
            ));
        }

        // Reshape into 3x3 matrix and normalize
        let scale = h_vec[8];
        let matrix = Matrix3::new(
            h_vec[0] / scale,
            h_vec[1] / scale,
            h_vec[2] / scale,
            h_vec[3] / scale,
            h_vec[4] / scale,
            h_vec[5] / scale,
            h_vec[6] / scale,
            h_vec[7] / scale,
            1.0,
        );

        Ok(Homography::new(matrix))
    }

    /// Find inliers based on reprojection error
    fn find_inliers(&self, homography: &Homography, matches: &[MatchPair]) -> Vec<bool> {
        matches
            .iter()
            .map(|m| {
                let transformed = homography.transform(&m.point1);
                let error = transformed.distance(&m.point2);
                error < self.config.threshold
            })
            .collect()
    }

    /// Refine homography using all inliers
    fn refine_homography(
        &self,
        _initial: &Homography,
        inliers: &[&MatchPair],
    ) -> AlignResult<Homography> {
        if inliers.len() < 4 {
            return Err(AlignError::InsufficientData(
                "Need at least 4 inliers for refinement".to_string(),
            ));
        }

        // Build overdetermined system
        let n = inliers.len();
        let mut a = nalgebra::DMatrix::zeros(n * 2, 9);

        for (i, m) in inliers.iter().enumerate() {
            let x1 = m.point1.x;
            let y1 = m.point1.y;
            let x2 = m.point2.x;
            let y2 = m.point2.y;

            a[(i * 2, 0)] = -x1;
            a[(i * 2, 1)] = -y1;
            a[(i * 2, 2)] = -1.0;
            a[(i * 2, 6)] = x2 * x1;
            a[(i * 2, 7)] = x2 * y1;
            a[(i * 2, 8)] = x2;

            a[(i * 2 + 1, 3)] = -x1;
            a[(i * 2 + 1, 4)] = -y1;
            a[(i * 2 + 1, 5)] = -1.0;
            a[(i * 2 + 1, 6)] = y2 * x1;
            a[(i * 2 + 1, 7)] = y2 * y1;
            a[(i * 2 + 1, 8)] = y2;
        }

        // Solve using SVD
        let svd = a.svd(true, true);
        let v = svd
            .v_t
            .ok_or_else(|| AlignError::NumericalError("SVD failed".to_string()))?;

        let h_vec = v.row(8);

        if h_vec[8].abs() < 1e-10 {
            return Err(AlignError::NumericalError(
                "Degenerate solution".to_string(),
            ));
        }

        let scale = h_vec[8];
        let matrix = Matrix3::new(
            h_vec[0] / scale,
            h_vec[1] / scale,
            h_vec[2] / scale,
            h_vec[3] / scale,
            h_vec[4] / scale,
            h_vec[5] / scale,
            h_vec[6] / scale,
            h_vec[7] / scale,
            1.0,
        );

        Ok(Homography::new(matrix))
    }
}

/// Affine transformation (6 DOF)
#[derive(Debug, Clone)]
pub struct AffineTransform {
    /// 2x3 affine matrix [a b tx; c d ty]
    pub matrix: nalgebra::Matrix2x3<f64>,
}

impl AffineTransform {
    /// Create a new affine transform
    #[must_use]
    pub fn new(matrix: nalgebra::Matrix2x3<f64>) -> Self {
        Self { matrix }
    }

    /// Create identity transform
    #[must_use]
    pub fn identity() -> Self {
        Self {
            matrix: nalgebra::Matrix2x3::new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0),
        }
    }

    /// Create translation
    #[must_use]
    pub fn translation(tx: f64, ty: f64) -> Self {
        Self {
            matrix: nalgebra::Matrix2x3::new(1.0, 0.0, tx, 0.0, 1.0, ty),
        }
    }

    /// Create rotation
    #[must_use]
    pub fn rotation(angle: f64) -> Self {
        let c = angle.cos();
        let s = angle.sin();
        Self {
            matrix: nalgebra::Matrix2x3::new(c, -s, 0.0, s, c, 0.0),
        }
    }

    /// Create scale
    #[must_use]
    pub fn scale(sx: f64, sy: f64) -> Self {
        Self {
            matrix: nalgebra::Matrix2x3::new(sx, 0.0, 0.0, 0.0, sy, 0.0),
        }
    }

    /// Transform a point
    #[must_use]
    pub fn transform(&self, point: &Point2D) -> Point2D {
        let p = nalgebra::Vector3::new(point.x, point.y, 1.0);
        let transformed = self.matrix * p;
        Point2D::new(transformed[0], transformed[1])
    }
}

/// Affine estimator
pub struct AffineEstimator {
    /// RANSAC configuration
    pub config: RansacConfig,
}

impl AffineEstimator {
    /// Create a new affine estimator
    #[must_use]
    pub fn new(config: RansacConfig) -> Self {
        Self { config }
    }

    /// Estimate affine transform from matched points
    ///
    /// # Errors
    /// Returns error if insufficient matches or estimation fails
    pub fn estimate(&self, matches: &[MatchPair]) -> AlignResult<(AffineTransform, Vec<bool>)> {
        if matches.len() < 3 {
            return Err(AlignError::InsufficientData(
                "Need at least 3 matches for affine".to_string(),
            ));
        }

        let mut best_inliers = Vec::new();
        let mut best_transform = None;
        let mut best_inlier_count = 0;

        // RANSAC iterations
        for _ in 0..self.config.max_iterations {
            // Sample 3 random matches
            let sample = self.sample_matches(matches, 3);

            // Estimate affine from 3 points
            if let Ok(t) = self.estimate_from_3_points(&sample) {
                // Count inliers
                let inliers = self.find_inliers(&t, matches);
                let inlier_count = inliers.iter().filter(|&&x| x).count();

                if inlier_count > best_inlier_count {
                    best_inlier_count = inlier_count;
                    best_inliers = inliers;
                    best_transform = Some(t);

                    if inlier_count >= self.config.min_inliers {
                        break;
                    }
                }
            }
        }

        if best_inlier_count < self.config.min_inliers {
            return Err(AlignError::NoSolution("Insufficient inliers".to_string()));
        }

        let transform = best_transform
            .ok_or_else(|| AlignError::NoSolution("No valid transform found".to_string()))?;

        Ok((transform, best_inliers))
    }

    /// Sample matches
    fn sample_matches(&self, matches: &[MatchPair], n: usize) -> Vec<MatchPair> {
        let step = matches.len() / n;
        (0..n)
            .map(|i| matches[(i * step) % matches.len()].clone())
            .collect()
    }

    /// Estimate affine from 3 points
    fn estimate_from_3_points(&self, matches: &[MatchPair]) -> AlignResult<AffineTransform> {
        if matches.len() != 3 {
            return Err(AlignError::InvalidConfig(
                "Need exactly 3 points".to_string(),
            ));
        }

        // Build linear system: [x1 y1 1 0  0  0] [a]   [x1']
        //                       [0  0  0 x1 y1 1] [b]   [y1']
        //                       [x2 y2 1 0  0  0] [tx] = [x2']
        //                       [0  0  0 x2 y2 1] [c]   [y2']
        //                       [x3 y3 1 0  0  0] [d]   [x3']
        //                       [0  0  0 x3 y3 1] [ty]  [y3']

        let mut a = nalgebra::DMatrix::zeros(6, 6);
        let mut b_vec = nalgebra::DVector::zeros(6);

        for (i, m) in matches.iter().enumerate() {
            let x = m.point1.x;
            let y = m.point1.y;
            let x_prime = m.point2.x;
            let y_prime = m.point2.y;

            a[(i * 2, 0)] = x;
            a[(i * 2, 1)] = y;
            a[(i * 2, 2)] = 1.0;
            b_vec[i * 2] = x_prime;

            a[(i * 2 + 1, 3)] = x;
            a[(i * 2 + 1, 4)] = y;
            a[(i * 2 + 1, 5)] = 1.0;
            b_vec[i * 2 + 1] = y_prime;
        }

        let decomp = a.lu();
        let solution = decomp.solve(&b_vec).ok_or_else(|| {
            AlignError::NumericalError("Failed to solve linear system".to_string())
        })?;

        let matrix = nalgebra::Matrix2x3::new(
            solution[0],
            solution[1],
            solution[2],
            solution[3],
            solution[4],
            solution[5],
        );

        Ok(AffineTransform::new(matrix))
    }

    /// Find inliers
    fn find_inliers(&self, transform: &AffineTransform, matches: &[MatchPair]) -> Vec<bool> {
        matches
            .iter()
            .map(|m| {
                let transformed = transform.transform(&m.point1);
                let error = transformed.distance(&m.point2);
                error < self.config.threshold
            })
            .collect()
    }
}

/// Perspective correction
pub struct PerspectiveCorrector {
    /// Target width
    pub target_width: usize,
    /// Target height
    pub target_height: usize,
}

impl PerspectiveCorrector {
    /// Create a new perspective corrector
    #[must_use]
    pub fn new(target_width: usize, target_height: usize) -> Self {
        Self {
            target_width,
            target_height,
        }
    }

    /// Compute homography to correct perspective distortion
    ///
    /// # Errors
    /// Returns error if corners are invalid
    pub fn compute_correction(&self, corners: &[Point2D; 4]) -> AlignResult<Homography> {
        // Target corners (rectangle)
        let target = [
            Point2D::new(0.0, 0.0),
            Point2D::new(self.target_width as f64, 0.0),
            Point2D::new(self.target_width as f64, self.target_height as f64),
            Point2D::new(0.0, self.target_height as f64),
        ];

        // Create match pairs
        let matches: Vec<MatchPair> = corners
            .iter()
            .zip(&target)
            .enumerate()
            .map(|(i, (src, dst))| MatchPair::new(i, i, 0, *src, *dst))
            .collect();

        // Estimate homography
        let estimator = HomographyEstimator::new(RansacConfig::default());
        estimator.estimate_from_4_points(&matches)
    }
}

/// Similarity transform (4 DOF: translation, rotation, uniform scale)
#[derive(Debug, Clone)]
pub struct SimilarityTransform {
    /// Scale factor
    pub scale: f64,
    /// Rotation angle (radians)
    pub rotation: f64,
    /// Translation X
    pub tx: f64,
    /// Translation Y
    pub ty: f64,
}

impl SimilarityTransform {
    /// Create a new similarity transform
    #[must_use]
    pub fn new(scale: f64, rotation: f64, tx: f64, ty: f64) -> Self {
        Self {
            scale,
            rotation,
            tx,
            ty,
        }
    }

    /// Create identity transform
    #[must_use]
    pub fn identity() -> Self {
        Self {
            scale: 1.0,
            rotation: 0.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// Transform a point
    #[must_use]
    pub fn transform(&self, point: &Point2D) -> Point2D {
        let c = self.rotation.cos();
        let s = self.rotation.sin();

        let x = self.scale * (c * point.x - s * point.y) + self.tx;
        let y = self.scale * (s * point.x + c * point.y) + self.ty;

        Point2D::new(x, y)
    }

    /// Convert to affine transform
    #[must_use]
    pub fn to_affine(&self) -> AffineTransform {
        let c = self.rotation.cos();
        let s = self.rotation.sin();
        let sc = self.scale * c;
        let ss = self.scale * s;

        let matrix = nalgebra::Matrix2x3::new(sc, -ss, self.tx, ss, sc, self.ty);

        AffineTransform::new(matrix)
    }
}

/// Similarity transform estimator
pub struct SimilarityEstimator {
    /// RANSAC configuration
    pub config: RansacConfig,
}

impl SimilarityEstimator {
    /// Create a new similarity estimator
    #[must_use]
    pub fn new(config: RansacConfig) -> Self {
        Self { config }
    }

    /// Estimate similarity transform from matched points
    ///
    /// # Errors
    /// Returns error if estimation fails
    pub fn estimate(&self, matches: &[MatchPair]) -> AlignResult<(SimilarityTransform, Vec<bool>)> {
        if matches.len() < 2 {
            return Err(AlignError::InsufficientData(
                "Need at least 2 matches for similarity".to_string(),
            ));
        }

        let mut best_inliers = Vec::new();
        let mut best_transform = None;
        let mut best_inlier_count = 0;

        // RANSAC iterations
        for _ in 0..self.config.max_iterations {
            // Sample 2 random matches
            let sample = self.sample_matches(matches, 2);

            // Estimate similarity from 2 points
            if let Ok(t) = self.estimate_from_2_points(&sample) {
                // Count inliers
                let inliers = self.find_inliers(&t, matches);
                let inlier_count = inliers.iter().filter(|&&x| x).count();

                if inlier_count > best_inlier_count {
                    best_inlier_count = inlier_count;
                    best_inliers = inliers;
                    best_transform = Some(t);

                    if inlier_count >= self.config.min_inliers {
                        break;
                    }
                }
            }
        }

        if best_inlier_count < self.config.min_inliers {
            return Err(AlignError::NoSolution("Insufficient inliers".to_string()));
        }

        let transform = best_transform
            .ok_or_else(|| AlignError::NoSolution("No valid transform found".to_string()))?;

        Ok((transform, best_inliers))
    }

    /// Sample matches
    fn sample_matches(&self, matches: &[MatchPair], n: usize) -> Vec<MatchPair> {
        let step = matches.len() / n;
        (0..n)
            .map(|i| matches[(i * step) % matches.len()].clone())
            .collect()
    }

    /// Estimate similarity from 2 points
    fn estimate_from_2_points(&self, matches: &[MatchPair]) -> AlignResult<SimilarityTransform> {
        if matches.len() != 2 {
            return Err(AlignError::InvalidConfig(
                "Need exactly 2 points".to_string(),
            ));
        }

        let p1 = &matches[0].point1;
        let p2 = &matches[1].point1;
        let q1 = &matches[0].point2;
        let q2 = &matches[1].point2;

        // Compute centroid
        let pc = Point2D::new((p1.x + p2.x) / 2.0, (p1.y + p2.y) / 2.0);
        let qc = Point2D::new((q1.x + q2.x) / 2.0, (q1.y + q2.y) / 2.0);

        // Center points
        let p1c = Point2D::new(p1.x - pc.x, p1.y - pc.y);
        let p2c = Point2D::new(p2.x - pc.x, p2.y - pc.y);
        let q1c = Point2D::new(q1.x - qc.x, q1.y - qc.y);
        let q2c = Point2D::new(q2.x - qc.x, q2.y - qc.y);

        // Compute scale
        let dist_p = (p1c.distance_squared(&p2c)).sqrt();
        let dist_q = (q1c.distance_squared(&q2c)).sqrt();

        if dist_p < 1e-10 {
            return Err(AlignError::NumericalError("Degenerate points".to_string()));
        }

        let scale = dist_q / dist_p;

        // Compute rotation
        let angle_p = (p2c.y - p1c.y).atan2(p2c.x - p1c.x);
        let angle_q = (q2c.y - q1c.y).atan2(q2c.x - q1c.x);
        let rotation = angle_q - angle_p;

        // Compute translation
        let c = rotation.cos();
        let s = rotation.sin();
        let tx = qc.x - scale * (c * pc.x - s * pc.y);
        let ty = qc.y - scale * (s * pc.x + c * pc.y);

        Ok(SimilarityTransform::new(scale, rotation, tx, ty))
    }

    /// Find inliers
    fn find_inliers(&self, transform: &SimilarityTransform, matches: &[MatchPair]) -> Vec<bool> {
        matches
            .iter()
            .map(|m| {
                let transformed = transform.transform(&m.point1);
                let error = transformed.distance(&m.point2);
                error < self.config.threshold
            })
            .collect()
    }
}

/// Fundamental matrix for epipolar geometry
#[derive(Debug, Clone)]
pub struct FundamentalMatrix {
    /// The 3x3 fundamental matrix
    pub matrix: Matrix3<f64>,
}

impl FundamentalMatrix {
    /// Create a new fundamental matrix
    #[must_use]
    pub fn new(matrix: Matrix3<f64>) -> Self {
        Self { matrix }
    }

    /// Compute epipolar line in second image for a point in first image
    #[must_use]
    pub fn compute_epipolar_line(&self, point: &Point2D) -> (f64, f64, f64) {
        let p = Vector3::new(point.x, point.y, 1.0);
        let line = self.matrix * p;
        (line[0], line[1], line[2])
    }

    /// Compute distance from point to epipolar line
    #[must_use]
    pub fn epipolar_distance(&self, point1: &Point2D, point2: &Point2D) -> f64 {
        let (a, b, c) = self.compute_epipolar_line(point1);
        let denominator = (a * a + b * b).sqrt();

        if denominator < 1e-10 {
            return f64::INFINITY;
        }

        (a * point2.x + b * point2.y + c).abs() / denominator
    }
}

/// Essential matrix for calibrated camera geometry
#[derive(Debug, Clone)]
pub struct EssentialMatrix {
    /// The 3x3 essential matrix
    pub matrix: Matrix3<f64>,
}

impl EssentialMatrix {
    /// Create a new essential matrix
    #[must_use]
    pub fn new(matrix: Matrix3<f64>) -> Self {
        Self { matrix }
    }

    /// Decompose into rotation and translation (up to scale)
    #[must_use]
    pub fn decompose(&self) -> Vec<(Matrix3<f64>, Vector3<f64>)> {
        // Simplified decomposition (in production, use proper SVD)
        // Returns 4 possible solutions
        vec![]
    }
}

/// Homography decomposition for plane-based structure
pub struct HomographyDecomposer;

impl HomographyDecomposer {
    /// Decompose homography into rotation, translation, and normal
    ///
    /// # Errors
    /// Returns error if decomposition fails
    #[allow(dead_code)]
    pub fn decompose(
        _homography: &Homography,
        _k1: &Matrix3<f64>,
        _k2: &Matrix3<f64>,
    ) -> AlignResult<Vec<(Matrix3<f64>, Vector3<f64>, Vector3<f64>)>> {
        // Simplified placeholder (in production, implement Faugeras-Lustman decomposition)
        Ok(vec![])
    }
}

/// Planar rectification for document scanning
pub struct PlanarRectifier {
    /// Target aspect ratio
    pub aspect_ratio: f64,
}

impl PlanarRectifier {
    /// Create a new planar rectifier
    #[must_use]
    pub fn new(aspect_ratio: f64) -> Self {
        Self { aspect_ratio }
    }

    /// Rectify a planar surface
    ///
    /// # Errors
    /// Returns error if rectification fails
    pub fn rectify(&self, corners: &[Point2D; 4], output_width: usize) -> AlignResult<Homography> {
        let output_height = (output_width as f64 / self.aspect_ratio) as usize;

        let target = [
            Point2D::new(0.0, 0.0),
            Point2D::new(output_width as f64, 0.0),
            Point2D::new(output_width as f64, output_height as f64),
            Point2D::new(0.0, output_height as f64),
        ];

        // Create match pairs
        let matches: Vec<MatchPair> = corners
            .iter()
            .zip(&target)
            .enumerate()
            .map(|(i, (src, dst))| MatchPair::new(i, i, 0, *src, *dst))
            .collect();

        let estimator = HomographyEstimator::new(RansacConfig::default());
        estimator.estimate_from_4_points(&matches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_homography_identity() {
        let h = Homography::identity();
        let p = Point2D::new(10.0, 20.0);
        let transformed = h.transform(&p);
        assert!((transformed.x - 10.0).abs() < 1e-10);
        assert!((transformed.y - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_affine_identity() {
        let t = AffineTransform::identity();
        let p = Point2D::new(10.0, 20.0);
        let transformed = t.transform(&p);
        assert!((transformed.x - 10.0).abs() < 1e-10);
        assert!((transformed.y - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_affine_translation() {
        let t = AffineTransform::translation(5.0, 10.0);
        let p = Point2D::new(10.0, 20.0);
        let transformed = t.transform(&p);
        assert!((transformed.x - 15.0).abs() < 1e-10);
        assert!((transformed.y - 30.0).abs() < 1e-10);
    }

    #[test]
    fn test_affine_scale() {
        let t = AffineTransform::scale(2.0, 3.0);
        let p = Point2D::new(10.0, 20.0);
        let transformed = t.transform(&p);
        assert!((transformed.x - 20.0).abs() < 1e-10);
        assert!((transformed.y - 60.0).abs() < 1e-10);
    }

    #[test]
    fn test_ransac_config() {
        let config = RansacConfig::default();
        assert_eq!(config.threshold, 3.0);
        assert_eq!(config.max_iterations, 1000);
        assert_eq!(config.min_inliers, 8);
    }

    #[test]
    fn test_similarity_identity() {
        let t = SimilarityTransform::identity();
        let p = Point2D::new(10.0, 20.0);
        let transformed = t.transform(&p);
        assert!((transformed.x - 10.0).abs() < 1e-10);
        assert!((transformed.y - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_similarity_scale() {
        let t = SimilarityTransform::new(2.0, 0.0, 0.0, 0.0);
        let p = Point2D::new(10.0, 20.0);
        let transformed = t.transform(&p);
        assert!((transformed.x - 20.0).abs() < 1e-10);
        assert!((transformed.y - 40.0).abs() < 1e-10);
    }

    #[test]
    fn test_similarity_to_affine() {
        let sim = SimilarityTransform::new(2.0, std::f64::consts::PI / 2.0, 10.0, 20.0);
        let affine = sim.to_affine();

        let p = Point2D::new(1.0, 0.0);
        let t1 = sim.transform(&p);
        let t2 = affine.transform(&p);

        assert!((t1.x - t2.x).abs() < 1e-10);
        assert!((t1.y - t2.y).abs() < 1e-10);
    }

    #[test]
    fn test_fundamental_matrix() {
        let f = FundamentalMatrix::new(Matrix3::identity());
        let p = Point2D::new(10.0, 20.0);
        let (a, b, c) = f.compute_epipolar_line(&p);
        assert!(a.is_finite() && b.is_finite() && c.is_finite());
    }

    #[test]
    fn test_planar_rectifier() {
        let rectifier = PlanarRectifier::new(1.5);
        assert_eq!(rectifier.aspect_ratio, 1.5);
    }
}
