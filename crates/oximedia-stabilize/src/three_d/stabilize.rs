//! 3D-based stabilization.

use crate::error::{StabilizeError, StabilizeResult};
use crate::motion::tracker::FeatureTrack;
use crate::transform::calculate::StabilizationTransform;

/// 3D stabilizer.
#[derive(Debug)]
pub struct ThreeDStabilizer {
    enable_full_3d: bool,
}

impl ThreeDStabilizer {
    /// Create a new 3D stabilizer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            enable_full_3d: true,
        }
    }

    /// Apply 3D stabilization to transforms.
    ///
    /// # Errors
    ///
    /// Returns an error if transforms are empty.
    pub fn stabilize_3d(
        &self,
        transforms: &[StabilizationTransform],
        _tracks: &[FeatureTrack],
    ) -> StabilizeResult<Vec<StabilizationTransform>> {
        if transforms.is_empty() {
            return Err(StabilizeError::EmptyFrameSequence);
        }

        // Return transforms as-is for now
        // Full implementation would use 3D pose estimation
        Ok(transforms.to_vec())
    }
}

impl Default for ThreeDStabilizer {
    fn default() -> Self {
        Self::new()
    }
}
