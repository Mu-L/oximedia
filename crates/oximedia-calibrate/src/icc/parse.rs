//! ICC profile parsing and representation.
//!
//! This module provides tools for parsing and representing ICC color profiles.

use crate::error::{CalibrationError, CalibrationResult};
use crate::{Illuminant, Matrix3x3};
use serde::{Deserialize, Serialize};

/// ICC profile version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IccProfileVersion {
    /// ICC v2.
    V2,
    /// ICC v4.
    V4,
}

/// ICC color profile.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IccProfile {
    /// Profile description.
    pub description: String,
    /// Profile version.
    pub version: IccProfileVersion,
    /// Color space to XYZ transformation matrix.
    pub to_xyz_matrix: Matrix3x3,
    /// XYZ to color space transformation matrix.
    pub from_xyz_matrix: Matrix3x3,
    /// Profile white point.
    pub white_point: Illuminant,
    /// Profile creation date (Unix timestamp).
    pub creation_date: u64,
}

impl IccProfile {
    /// Create a new ICC profile.
    #[must_use]
    pub fn new(description: String, to_xyz_matrix: Matrix3x3, white_point: Illuminant) -> Self {
        let from_xyz_matrix = Self::compute_inverse_matrix(&to_xyz_matrix);

        Self {
            description,
            version: IccProfileVersion::V4,
            to_xyz_matrix,
            from_xyz_matrix,
            white_point,
            creation_date: 0, // Placeholder
        }
    }

    /// Parse an ICC profile from bytes.
    ///
    /// # Arguments
    ///
    /// * `data` - ICC profile data
    ///
    /// # Errors
    ///
    /// Returns an error if parsing fails.
    pub fn from_bytes(_data: &[u8]) -> CalibrationResult<Self> {
        // This is a placeholder implementation
        // A real implementation would parse the ICC profile structure

        Err(CalibrationError::IccParseError(
            "ICC profile parsing not yet implemented".to_string(),
        ))
    }

    /// Serialize the ICC profile to bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_bytes(&self) -> CalibrationResult<Vec<u8>> {
        // This is a placeholder implementation
        // A real implementation would generate a valid ICC profile structure

        Err(CalibrationError::IccParseError(
            "ICC profile serialization not yet implemented".to_string(),
        ))
    }

    /// Convert an RGB color to XYZ using this profile.
    #[must_use]
    pub fn rgb_to_xyz(&self, rgb: &[f64; 3]) -> [f64; 3] {
        self.apply_matrix(&self.to_xyz_matrix, rgb)
    }

    /// Convert an XYZ color to RGB using this profile.
    #[must_use]
    pub fn xyz_to_rgb(&self, xyz: &[f64; 3]) -> [f64; 3] {
        self.apply_matrix(&self.from_xyz_matrix, xyz)
    }

    /// Apply a 3x3 matrix to a color.
    fn apply_matrix(&self, matrix: &Matrix3x3, color: &[f64; 3]) -> [f64; 3] {
        [
            matrix[0][0] * color[0] + matrix[0][1] * color[1] + matrix[0][2] * color[2],
            matrix[1][0] * color[0] + matrix[1][1] * color[1] + matrix[1][2] * color[2],
            matrix[2][0] * color[0] + matrix[2][1] * color[1] + matrix[2][2] * color[2],
        ]
    }

    /// Compute the inverse of a 3x3 matrix.
    fn compute_inverse_matrix(matrix: &Matrix3x3) -> Matrix3x3 {
        // Compute determinant
        let det = matrix[0][0] * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
            - matrix[0][1] * (matrix[1][0] * matrix[2][2] - matrix[1][2] * matrix[2][0])
            + matrix[0][2] * (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0]);

        if det.abs() < 1e-10 {
            // Matrix is singular, return identity
            return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        }

        let inv_det = 1.0 / det;

        // Compute inverse using adjugate method
        [
            [
                (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1]) * inv_det,
                (matrix[0][2] * matrix[2][1] - matrix[0][1] * matrix[2][2]) * inv_det,
                (matrix[0][1] * matrix[1][2] - matrix[0][2] * matrix[1][1]) * inv_det,
            ],
            [
                (matrix[1][2] * matrix[2][0] - matrix[1][0] * matrix[2][2]) * inv_det,
                (matrix[0][0] * matrix[2][2] - matrix[0][2] * matrix[2][0]) * inv_det,
                (matrix[0][2] * matrix[1][0] - matrix[0][0] * matrix[1][2]) * inv_det,
            ],
            [
                (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0]) * inv_det,
                (matrix[0][1] * matrix[2][0] - matrix[0][0] * matrix[2][1]) * inv_det,
                (matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0]) * inv_det,
            ],
        ]
    }

    /// Validate the ICC profile.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails.
    pub fn validate(&self) -> CalibrationResult<()> {
        // Check that matrices are not all zeros
        let to_xyz_sum: f64 = self.to_xyz_matrix.iter().flatten().sum();
        let from_xyz_sum: f64 = self.from_xyz_matrix.iter().flatten().sum();

        if to_xyz_sum.abs() < 1e-10 {
            return Err(CalibrationError::IccInvalidProfile(
                "to_xyz_matrix is zero".to_string(),
            ));
        }

        if from_xyz_sum.abs() < 1e-10 {
            return Err(CalibrationError::IccInvalidProfile(
                "from_xyz_matrix is zero".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icc_profile_new() {
        let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

        let profile = IccProfile::new("Test Profile".to_string(), identity, Illuminant::D65);

        assert_eq!(profile.description, "Test Profile");
        assert_eq!(profile.version, IccProfileVersion::V4);
        assert_eq!(profile.white_point, Illuminant::D65);
    }

    #[test]
    fn test_icc_profile_rgb_to_xyz() {
        let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

        let profile = IccProfile::new("Test Profile".to_string(), identity, Illuminant::D65);

        let rgb = [0.5, 0.6, 0.7];
        let xyz = profile.rgb_to_xyz(&rgb);

        assert!((xyz[0] - 0.5).abs() < 1e-10);
        assert!((xyz[1] - 0.6).abs() < 1e-10);
        assert!((xyz[2] - 0.7).abs() < 1e-10);
    }

    #[test]
    fn test_icc_profile_xyz_to_rgb() {
        let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

        let profile = IccProfile::new("Test Profile".to_string(), identity, Illuminant::D65);

        let xyz = [0.5, 0.6, 0.7];
        let rgb = profile.xyz_to_rgb(&xyz);

        assert!((rgb[0] - 0.5).abs() < 1e-10);
        assert!((rgb[1] - 0.6).abs() < 1e-10);
        assert!((rgb[2] - 0.7).abs() < 1e-10);
    }

    #[test]
    fn test_icc_profile_roundtrip() {
        let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

        let profile = IccProfile::new("Test Profile".to_string(), identity, Illuminant::D65);

        let rgb = [0.3, 0.5, 0.7];
        let xyz = profile.rgb_to_xyz(&rgb);
        let rgb2 = profile.xyz_to_rgb(&xyz);

        assert!((rgb2[0] - rgb[0]).abs() < 1e-10);
        assert!((rgb2[1] - rgb[1]).abs() < 1e-10);
        assert!((rgb2[2] - rgb[2]).abs() < 1e-10);
    }

    #[test]
    fn test_icc_profile_validate() {
        let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

        let profile = IccProfile::new("Test Profile".to_string(), identity, Illuminant::D65);

        assert!(profile.validate().is_ok());
    }

    #[test]
    fn test_icc_profile_validate_invalid() {
        let zero_matrix = [[0.0; 3]; 3];

        let profile = IccProfile::new("Test Profile".to_string(), zero_matrix, Illuminant::D65);

        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_compute_inverse_matrix() {
        let matrix = [[2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0]];

        let inverse = IccProfile::compute_inverse_matrix(&matrix);

        // Inverse of 2I should be 0.5I
        assert!((inverse[0][0] - 0.5).abs() < 1e-10);
        assert!((inverse[1][1] - 0.5).abs() < 1e-10);
        assert!((inverse[2][2] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_compute_inverse_matrix_singular() {
        let singular = [[0.0; 3]; 3];

        let inverse = IccProfile::compute_inverse_matrix(&singular);

        // Should return identity for singular matrix
        assert!((inverse[0][0] - 1.0).abs() < 1e-10);
        assert!((inverse[1][1] - 1.0).abs() < 1e-10);
        assert!((inverse[2][2] - 1.0).abs() < 1e-10);
    }
}
