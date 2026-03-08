//! Motion models for camera transformation.
//!
//! This module defines different motion models used to represent camera movement:
//! - Translation: Simple pan/tilt
//! - Affine: Translation + rotation + scale
//! - Perspective: Full homography
//! - 3D: Full 3D camera pose

use crate::error::{StabilizeError, StabilizeResult};
use nalgebra as na;
use ndarray::Array2;
use serde::{Deserialize, Serialize};

/// A motion model representing camera transformation between frames.
pub trait MotionModel: Send + Sync {
    /// Get the transformation matrix.
    fn matrix(&self) -> na::Matrix3<f64>;

    /// Set the transformation matrix.
    fn set_matrix(&mut self, matrix: na::Matrix3<f64>) -> StabilizeResult<()>;

    /// Transform a point.
    fn transform_point(&self, x: f64, y: f64) -> (f64, f64);

    /// Get motion parameters as a vector.
    fn parameters(&self) -> Vec<f64>;

    /// Set motion parameters from a vector.
    fn set_parameters(&mut self, params: &[f64]) -> StabilizeResult<()>;

    /// Compose with another model (self * other).
    fn compose(&self, other: &dyn MotionModel) -> StabilizeResult<Box<dyn MotionModel>>;

    /// Invert the transformation.
    fn invert(&self) -> StabilizeResult<Box<dyn MotionModel>>;

    /// Clone the motion model.
    fn clone_box(&self) -> Box<dyn MotionModel>;
}

/// Translation-only motion model (2 parameters: dx, dy).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationModel {
    /// Translation in X
    pub dx: f64,
    /// Translation in Y
    pub dy: f64,
}

impl TranslationModel {
    /// Create a new translation model.
    #[must_use]
    pub const fn new(dx: f64, dy: f64) -> Self {
        Self { dx, dy }
    }

    /// Create an identity translation.
    #[must_use]
    pub const fn identity() -> Self {
        Self { dx: 0.0, dy: 0.0 }
    }

    /// Get magnitude of translation.
    #[must_use]
    pub fn magnitude(&self) -> f64 {
        (self.dx * self.dx + self.dy * self.dy).sqrt()
    }
}

impl MotionModel for TranslationModel {
    fn matrix(&self) -> na::Matrix3<f64> {
        na::Matrix3::new(1.0, 0.0, self.dx, 0.0, 1.0, self.dy, 0.0, 0.0, 1.0)
    }

    fn set_matrix(&mut self, matrix: na::Matrix3<f64>) -> StabilizeResult<()> {
        self.dx = matrix[(0, 2)];
        self.dy = matrix[(1, 2)];
        Ok(())
    }

    fn transform_point(&self, x: f64, y: f64) -> (f64, f64) {
        (x + self.dx, y + self.dy)
    }

    fn parameters(&self) -> Vec<f64> {
        vec![self.dx, self.dy]
    }

    fn set_parameters(&mut self, params: &[f64]) -> StabilizeResult<()> {
        if params.len() != 2 {
            return Err(StabilizeError::invalid_parameter(
                "parameters",
                format!("expected 2, got {}", params.len()),
            ));
        }
        self.dx = params[0];
        self.dy = params[1];
        Ok(())
    }

    fn compose(&self, other: &dyn MotionModel) -> StabilizeResult<Box<dyn MotionModel>> {
        let params = other.parameters();
        if params.len() >= 2 {
            Ok(Box::new(Self::new(
                self.dx + params[0],
                self.dy + params[1],
            )))
        } else {
            Err(StabilizeError::invalid_parameter(
                "other model",
                "incompatible model type",
            ))
        }
    }

    fn invert(&self) -> StabilizeResult<Box<dyn MotionModel>> {
        Ok(Box::new(Self::new(-self.dx, -self.dy)))
    }

    fn clone_box(&self) -> Box<dyn MotionModel> {
        Box::new(self.clone())
    }
}

/// Affine motion model (6 parameters: rotation, scale, translation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffineModel {
    /// Translation in X
    pub dx: f64,
    /// Translation in Y
    pub dy: f64,
    /// Rotation angle in radians
    pub angle: f64,
    /// Scale factor
    pub scale: f64,
    /// Shear X
    pub shear_x: f64,
    /// Shear Y
    pub shear_y: f64,
}

impl AffineModel {
    /// Create a new affine model.
    #[must_use]
    pub const fn new(dx: f64, dy: f64, angle: f64, scale: f64, shear_x: f64, shear_y: f64) -> Self {
        Self {
            dx,
            dy,
            angle,
            scale,
            shear_x,
            shear_y,
        }
    }

    /// Create an identity affine transformation.
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            dx: 0.0,
            dy: 0.0,
            angle: 0.0,
            scale: 1.0,
            shear_x: 0.0,
            shear_y: 0.0,
        }
    }

    /// Create from translation, rotation, and scale only.
    #[must_use]
    pub const fn from_trs(dx: f64, dy: f64, angle: f64, scale: f64) -> Self {
        Self {
            dx,
            dy,
            angle,
            scale,
            shear_x: 0.0,
            shear_y: 0.0,
        }
    }

    /// Get motion magnitude.
    #[must_use]
    pub fn magnitude(&self) -> f64 {
        let trans = (self.dx * self.dx + self.dy * self.dy).sqrt();
        let rot = self.angle.abs();
        let scale_dev = (self.scale - 1.0).abs();
        trans + rot * 10.0 + scale_dev * 10.0
    }
}

impl MotionModel for AffineModel {
    fn matrix(&self) -> na::Matrix3<f64> {
        let cos_a = self.angle.cos();
        let sin_a = self.angle.sin();
        let s = self.scale;

        na::Matrix3::new(
            s * cos_a + self.shear_x,
            -s * sin_a,
            self.dx,
            s * sin_a + self.shear_y,
            s * cos_a,
            self.dy,
            0.0,
            0.0,
            1.0,
        )
    }

    fn set_matrix(&mut self, matrix: na::Matrix3<f64>) -> StabilizeResult<()> {
        self.dx = matrix[(0, 2)];
        self.dy = matrix[(1, 2)];

        let a = matrix[(0, 0)];
        let b = matrix[(0, 1)];
        let c = matrix[(1, 0)];
        let _d = matrix[(1, 1)];

        self.scale = (a * a + b * b).sqrt();
        self.angle = b.atan2(a);

        if self.scale > 0.0 {
            self.shear_x = a - self.scale * self.angle.cos();
            self.shear_y = c - self.scale * self.angle.sin();
        }

        Ok(())
    }

    fn transform_point(&self, x: f64, y: f64) -> (f64, f64) {
        let mat = self.matrix();
        let px = mat[(0, 0)] * x + mat[(0, 1)] * y + mat[(0, 2)];
        let py = mat[(1, 0)] * x + mat[(1, 1)] * y + mat[(1, 2)];
        (px, py)
    }

    fn parameters(&self) -> Vec<f64> {
        vec![
            self.dx,
            self.dy,
            self.angle,
            self.scale,
            self.shear_x,
            self.shear_y,
        ]
    }

    fn set_parameters(&mut self, params: &[f64]) -> StabilizeResult<()> {
        if params.len() != 6 {
            return Err(StabilizeError::invalid_parameter(
                "parameters",
                format!("expected 6, got {}", params.len()),
            ));
        }
        self.dx = params[0];
        self.dy = params[1];
        self.angle = params[2];
        self.scale = params[3];
        self.shear_x = params[4];
        self.shear_y = params[5];
        Ok(())
    }

    fn compose(&self, other: &dyn MotionModel) -> StabilizeResult<Box<dyn MotionModel>> {
        let m1 = self.matrix();
        let m2 = other.matrix();
        let result = m1 * m2;

        let mut model = Self::identity();
        model.set_matrix(result)?;
        Ok(Box::new(model))
    }

    fn invert(&self) -> StabilizeResult<Box<dyn MotionModel>> {
        let mat = self.matrix();
        if let Some(inv) = mat.try_inverse() {
            let mut model = Self::identity();
            model.set_matrix(inv)?;
            Ok(Box::new(model))
        } else {
            Err(StabilizeError::matrix("Matrix is not invertible"))
        }
    }

    fn clone_box(&self) -> Box<dyn MotionModel> {
        Box::new(self.clone())
    }
}

/// Perspective motion model (8 parameters: full homography).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerspectiveModel {
    /// Homography matrix (3x3)
    homography: na::Matrix3<f64>,
}

impl PerspectiveModel {
    /// Create a new perspective model.
    #[must_use]
    pub fn new(homography: na::Matrix3<f64>) -> Self {
        Self { homography }
    }

    /// Create an identity perspective transformation.
    #[must_use]
    pub fn identity() -> Self {
        Self {
            homography: na::Matrix3::identity(),
        }
    }

    /// Create from a 3x3 array.
    ///
    /// # Errors
    ///
    /// Returns an error if the array dimensions are not 3x3.
    pub fn from_array(array: &Array2<f64>) -> StabilizeResult<Self> {
        if array.dim() != (3, 3) {
            return Err(StabilizeError::dimension_mismatch(
                "3x3",
                format!("{:?}", array.dim()),
            ));
        }

        let mut mat = na::Matrix3::zeros();
        for i in 0..3 {
            for j in 0..3 {
                mat[(i, j)] = array[[i, j]];
            }
        }

        Ok(Self::new(mat))
    }

    /// Get motion magnitude.
    #[must_use]
    pub fn magnitude(&self) -> f64 {
        // Frobenius norm of deviation from identity
        let identity = na::Matrix3::identity();
        let diff = self.homography - identity;
        diff.norm()
    }
}

impl MotionModel for PerspectiveModel {
    fn matrix(&self) -> na::Matrix3<f64> {
        self.homography
    }

    fn set_matrix(&mut self, matrix: na::Matrix3<f64>) -> StabilizeResult<()> {
        self.homography = matrix;
        Ok(())
    }

    fn transform_point(&self, x: f64, y: f64) -> (f64, f64) {
        let p = na::Vector3::new(x, y, 1.0);
        let transformed = self.homography * p;

        if transformed[2].abs() < 1e-10 {
            return (x, y); // Degenerate case
        }

        let w = transformed[2];
        (transformed[0] / w, transformed[1] / w)
    }

    fn parameters(&self) -> Vec<f64> {
        let mut params = Vec::with_capacity(9);
        for i in 0..3 {
            for j in 0..3 {
                params.push(self.homography[(i, j)]);
            }
        }
        params
    }

    fn set_parameters(&mut self, params: &[f64]) -> StabilizeResult<()> {
        if params.len() != 9 {
            return Err(StabilizeError::invalid_parameter(
                "parameters",
                format!("expected 9, got {}", params.len()),
            ));
        }

        for i in 0..3 {
            for j in 0..3 {
                self.homography[(i, j)] = params[i * 3 + j];
            }
        }
        Ok(())
    }

    fn compose(&self, other: &dyn MotionModel) -> StabilizeResult<Box<dyn MotionModel>> {
        let result = self.homography * other.matrix();
        Ok(Box::new(Self::new(result)))
    }

    fn invert(&self) -> StabilizeResult<Box<dyn MotionModel>> {
        if let Some(inv) = self.homography.try_inverse() {
            Ok(Box::new(Self::new(inv)))
        } else {
            Err(StabilizeError::matrix("Homography is not invertible"))
        }
    }

    fn clone_box(&self) -> Box<dyn MotionModel> {
        Box::new(self.clone())
    }
}

/// 3D camera pose model (12 parameters: rotation matrix + translation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreeDModel {
    /// Rotation matrix (3x3)
    pub rotation: na::Matrix3<f64>,
    /// Translation vector
    pub translation: na::Vector3<f64>,
    /// Camera focal length
    pub focal_length: f64,
    /// Camera principal point (cx, cy)
    pub principal_point: (f64, f64),
}

impl ThreeDModel {
    /// Create a new 3D model.
    #[must_use]
    pub fn new(
        rotation: na::Matrix3<f64>,
        translation: na::Vector3<f64>,
        focal_length: f64,
        principal_point: (f64, f64),
    ) -> Self {
        Self {
            rotation,
            translation,
            focal_length,
            principal_point,
        }
    }

    /// Create an identity 3D transformation.
    #[must_use]
    pub fn identity() -> Self {
        Self {
            rotation: na::Matrix3::identity(),
            translation: na::Vector3::zeros(),
            focal_length: 1.0,
            principal_point: (0.0, 0.0),
        }
    }

    /// Get motion magnitude.
    #[must_use]
    pub fn magnitude(&self) -> f64 {
        let trans_mag = self.translation.norm();
        let rot_mag = (self.rotation - na::Matrix3::identity()).norm();
        trans_mag + rot_mag * 10.0
    }

    /// Project 3D point to 2D.
    #[must_use]
    pub fn project_3d(&self, point: na::Vector3<f64>) -> (f64, f64) {
        let transformed = self.rotation * point + self.translation;

        if transformed[2].abs() < 1e-10 {
            return self.principal_point;
        }

        let x = self.focal_length * transformed[0] / transformed[2] + self.principal_point.0;
        let y = self.focal_length * transformed[1] / transformed[2] + self.principal_point.1;

        (x, y)
    }
}

impl MotionModel for ThreeDModel {
    fn matrix(&self) -> na::Matrix3<f64> {
        // Return homography approximation for 2D projection
        self.rotation
    }

    fn set_matrix(&mut self, matrix: na::Matrix3<f64>) -> StabilizeResult<()> {
        self.rotation = matrix;
        Ok(())
    }

    fn transform_point(&self, x: f64, y: f64) -> (f64, f64) {
        // Simple 2D approximation
        let mat = self.rotation;
        let px = mat[(0, 0)] * x + mat[(0, 1)] * y + self.translation[0];
        let py = mat[(1, 0)] * x + mat[(1, 1)] * y + self.translation[1];
        (px, py)
    }

    fn parameters(&self) -> Vec<f64> {
        let mut params = Vec::with_capacity(15);

        // Rotation matrix (9 values)
        for i in 0..3 {
            for j in 0..3 {
                params.push(self.rotation[(i, j)]);
            }
        }

        // Translation (3 values)
        params.push(self.translation[0]);
        params.push(self.translation[1]);
        params.push(self.translation[2]);

        // Intrinsics (3 values)
        params.push(self.focal_length);
        params.push(self.principal_point.0);
        params.push(self.principal_point.1);

        params
    }

    fn set_parameters(&mut self, params: &[f64]) -> StabilizeResult<()> {
        if params.len() != 15 {
            return Err(StabilizeError::invalid_parameter(
                "parameters",
                format!("expected 15, got {}", params.len()),
            ));
        }

        // Rotation matrix
        for i in 0..3 {
            for j in 0..3 {
                self.rotation[(i, j)] = params[i * 3 + j];
            }
        }

        // Translation
        self.translation[0] = params[9];
        self.translation[1] = params[10];
        self.translation[2] = params[11];

        // Intrinsics
        self.focal_length = params[12];
        self.principal_point = (params[13], params[14]);

        Ok(())
    }

    fn compose(&self, other: &dyn MotionModel) -> StabilizeResult<Box<dyn MotionModel>> {
        let params = other.parameters();
        if params.len() >= 12 {
            // Compose rotations and translations
            let other_rot = other.matrix();
            let result_rot = self.rotation * other_rot;

            let other_trans = na::Vector3::new(params[9], params[10], params[11]);
            let result_trans = self.rotation * other_trans + self.translation;

            Ok(Box::new(Self::new(
                result_rot,
                result_trans,
                self.focal_length,
                self.principal_point,
            )))
        } else {
            Err(StabilizeError::invalid_parameter(
                "other model",
                "incompatible model type",
            ))
        }
    }

    fn invert(&self) -> StabilizeResult<Box<dyn MotionModel>> {
        if let Some(inv_rot) = self.rotation.try_inverse() {
            let inv_trans = -inv_rot * self.translation;
            Ok(Box::new(Self::new(
                inv_rot,
                inv_trans,
                self.focal_length,
                self.principal_point,
            )))
        } else {
            Err(StabilizeError::matrix("Rotation matrix is not invertible"))
        }
    }

    fn clone_box(&self) -> Box<dyn MotionModel> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translation_model() {
        let model = TranslationModel::new(10.0, 20.0);
        let (x, y) = model.transform_point(0.0, 0.0);
        assert!((x - 10.0).abs() < f64::EPSILON);
        assert!((y - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_translation_identity() {
        let model = TranslationModel::identity();
        let (x, y) = model.transform_point(5.0, 7.0);
        assert!((x - 5.0).abs() < f64::EPSILON);
        assert!((y - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_affine_model() {
        let model = AffineModel::from_trs(10.0, 20.0, 0.0, 1.0);
        let (x, y) = model.transform_point(0.0, 0.0);
        assert!((x - 10.0).abs() < 1e-10);
        assert!((y - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_affine_rotation() {
        let model = AffineModel::from_trs(0.0, 0.0, std::f64::consts::PI / 2.0, 1.0);
        let (x, y) = model.transform_point(1.0, 0.0);
        assert!((x - 0.0).abs() < 1e-10);
        assert!((y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_perspective_identity() {
        let model = PerspectiveModel::identity();
        let (x, y) = model.transform_point(5.0, 7.0);
        assert!((x - 5.0).abs() < f64::EPSILON);
        assert!((y - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_model_parameters() {
        let model = TranslationModel::new(10.0, 20.0);
        let params = model.parameters();
        assert_eq!(params.len(), 2);
        assert!((params[0] - 10.0).abs() < f64::EPSILON);
        assert!((params[1] - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_model_inversion() {
        let model = TranslationModel::new(10.0, 20.0);
        let inv = model.invert().expect("should succeed in test");
        let params = inv.parameters();
        assert!((params[0] + 10.0).abs() < f64::EPSILON);
        assert!((params[1] + 20.0).abs() < f64::EPSILON);
    }
}
