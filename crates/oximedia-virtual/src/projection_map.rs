#![allow(dead_code)]
//! Projection mapping and conversion utilities for virtual production.
//!
//! Supports equirectangular, cubemap, and fisheye projections with coordinate
//! validation and cross-projection conversion.

use std::f64::consts::PI;

/// The type of spherical projection used to encode imagery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionType {
    /// Latitude/longitude mapping onto a flat 2-D image.
    Equirectangular,
    /// Six-face cube unwrapping.
    Cubemap,
    /// Circular fisheye lens projection.
    Fisheye,
}

impl ProjectionType {
    /// Nominal horizontal field of view in degrees for this projection.
    #[must_use]
    pub fn field_of_view_deg(&self) -> f64 {
        match self {
            Self::Equirectangular => 360.0,
            Self::Cubemap => 90.0,
            Self::Fisheye => 180.0,
        }
    }
}

/// A UV-coordinate pair in normalised image space \[0.0, 1.0\].
#[derive(Debug, Clone, Copy)]
pub struct UvCoord {
    /// Horizontal position.
    pub u: f64,
    /// Vertical position.
    pub v: f64,
}

impl UvCoord {
    /// Create a new `UvCoord`.
    #[must_use]
    pub fn new(u: f64, v: f64) -> Self {
        Self { u, v }
    }
}

/// A spherical direction described by azimuth and elevation in radians.
#[derive(Debug, Clone, Copy)]
pub struct SphericalCoord {
    /// Azimuth (yaw) in radians, \[-π, π\].
    pub azimuth: f64,
    /// Elevation (pitch) in radians, \[-π/2, π/2\].
    pub elevation: f64,
}

impl SphericalCoord {
    /// Create a new `SphericalCoord`.
    #[must_use]
    pub fn new(azimuth: f64, elevation: f64) -> Self {
        Self { azimuth, elevation }
    }
}

/// Maps between UV image coordinates and spherical directions.
#[derive(Debug, Clone)]
pub struct ProjectionMap {
    projection: ProjectionType,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

impl ProjectionMap {
    /// Create a new `ProjectionMap`.
    #[must_use]
    pub fn new(projection: ProjectionType, width: u32, height: u32) -> Self {
        Self {
            projection,
            width,
            height,
        }
    }

    /// Convert a normalised UV coordinate to a spherical direction.
    ///
    /// Returns `None` for projections/coordinates where mapping is undefined.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn map_point(&self, uv: UvCoord) -> Option<SphericalCoord> {
        match self.projection {
            ProjectionType::Equirectangular => {
                let az = (uv.u - 0.5) * 2.0 * PI;
                let el = (0.5 - uv.v) * PI;
                Some(SphericalCoord::new(az, el))
            }
            ProjectionType::Fisheye => {
                let cx = uv.u - 0.5;
                let cy = uv.v - 0.5;
                let r = (cx * cx + cy * cy).sqrt();
                if r > 0.5 {
                    return None; // outside fisheye circle
                }
                let theta = r * PI; // maps [0, 0.5] → [0, π/2]
                let phi = cy.atan2(cx);
                let el = PI / 2.0 - theta;
                Some(SphericalCoord::new(phi, el))
            }
            ProjectionType::Cubemap => {
                // Simplified: treat single face as equirectangular over 90°
                let az = (uv.u - 0.5) * PI / 2.0;
                let el = (0.5 - uv.v) * PI / 2.0;
                Some(SphericalCoord::new(az, el))
            }
        }
    }

    /// Returns `true` if the given UV coordinate is within the valid image area.
    #[must_use]
    pub fn is_valid_coord(&self, uv: UvCoord) -> bool {
        if uv.u < 0.0 || uv.u > 1.0 || uv.v < 0.0 || uv.v > 1.0 {
            return false;
        }
        // For fisheye, additionally check that the point lies within the circle.
        if self.projection == ProjectionType::Fisheye {
            let cx = uv.u - 0.5;
            let cy = uv.v - 0.5;
            return (cx * cx + cy * cy).sqrt() <= 0.5;
        }
        true
    }
}

/// Converts coordinates between two different projection types.
#[derive(Debug)]
pub struct ProjectionConverter {
    src: ProjectionMap,
    dst: ProjectionMap,
}

impl ProjectionConverter {
    /// Create a new `ProjectionConverter` from `src` to `dst`.
    #[must_use]
    pub fn new(src: ProjectionMap, dst: ProjectionMap) -> Self {
        Self { src, dst }
    }

    /// Convert a UV coordinate in the source projection to the destination.
    ///
    /// Returns `None` if the source coordinate is invalid or the resulting
    /// direction cannot be represented in the destination projection.
    #[must_use]
    pub fn convert(&self, uv: UvCoord) -> Option<UvCoord> {
        if !self.src.is_valid_coord(uv) {
            return None;
        }
        let spherical = self.src.map_point(uv)?;
        // Back-project spherical → UV in destination.
        match self.dst.projection {
            ProjectionType::Equirectangular => {
                let u = spherical.azimuth / (2.0 * PI) + 0.5;
                let v = 0.5 - spherical.elevation / PI;
                Some(UvCoord::new(u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)))
            }
            ProjectionType::Fisheye => {
                let theta = PI / 2.0 - spherical.elevation;
                let r = theta / PI; // [0, 0.5] for valid hemisphere
                if r > 0.5 {
                    return None;
                }
                let u = 0.5 + r * spherical.azimuth.cos();
                let v = 0.5 + r * spherical.azimuth.sin();
                Some(UvCoord::new(u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)))
            }
            ProjectionType::Cubemap => {
                let u = spherical.azimuth / (PI / 2.0) + 0.5;
                let v = 0.5 - spherical.elevation / (PI / 2.0);
                Some(UvCoord::new(u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equirectangular_fov() {
        assert!((ProjectionType::Equirectangular.field_of_view_deg() - 360.0).abs() < 1e-9);
    }

    #[test]
    fn test_cubemap_fov() {
        assert!((ProjectionType::Cubemap.field_of_view_deg() - 90.0).abs() < 1e-9);
    }

    #[test]
    fn test_fisheye_fov() {
        assert!((ProjectionType::Fisheye.field_of_view_deg() - 180.0).abs() < 1e-9);
    }

    #[test]
    fn test_equirect_center_maps_to_origin() {
        let map = ProjectionMap::new(ProjectionType::Equirectangular, 3840, 1920);
        let coord = map
            .map_point(UvCoord::new(0.5, 0.5))
            .expect("should succeed in test");
        assert!(coord.azimuth.abs() < 1e-9);
        assert!(coord.elevation.abs() < 1e-9);
    }

    #[test]
    fn test_fisheye_center_maps() {
        let map = ProjectionMap::new(ProjectionType::Fisheye, 1024, 1024);
        let coord = map
            .map_point(UvCoord::new(0.5, 0.5))
            .expect("should succeed in test");
        // Center of fisheye → elevation = π/2 (straight up).
        assert!((coord.elevation - PI / 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_fisheye_outside_circle_returns_none() {
        let map = ProjectionMap::new(ProjectionType::Fisheye, 1024, 1024);
        let result = map.map_point(UvCoord::new(0.0, 0.0));
        assert!(result.is_none());
    }

    #[test]
    fn test_is_valid_coord_in_range() {
        let map = ProjectionMap::new(ProjectionType::Equirectangular, 3840, 1920);
        assert!(map.is_valid_coord(UvCoord::new(0.5, 0.5)));
        assert!(map.is_valid_coord(UvCoord::new(0.0, 0.0)));
        assert!(map.is_valid_coord(UvCoord::new(1.0, 1.0)));
    }

    #[test]
    fn test_is_valid_coord_out_of_range() {
        let map = ProjectionMap::new(ProjectionType::Equirectangular, 3840, 1920);
        assert!(!map.is_valid_coord(UvCoord::new(-0.1, 0.5)));
        assert!(!map.is_valid_coord(UvCoord::new(1.1, 0.5)));
    }

    #[test]
    fn test_fisheye_valid_coord_in_circle() {
        let map = ProjectionMap::new(ProjectionType::Fisheye, 1024, 1024);
        assert!(map.is_valid_coord(UvCoord::new(0.5, 0.5)));
    }

    #[test]
    fn test_fisheye_invalid_coord_outside_circle() {
        let map = ProjectionMap::new(ProjectionType::Fisheye, 1024, 1024);
        assert!(!map.is_valid_coord(UvCoord::new(0.05, 0.05)));
    }

    #[test]
    fn test_converter_equirect_to_equirect_roundtrip() {
        let src = ProjectionMap::new(ProjectionType::Equirectangular, 3840, 1920);
        let dst = ProjectionMap::new(ProjectionType::Equirectangular, 3840, 1920);
        let converter = ProjectionConverter::new(src, dst);
        let uv_in = UvCoord::new(0.5, 0.5);
        let uv_out = converter.convert(uv_in).expect("should succeed in test");
        assert!((uv_out.u - 0.5).abs() < 1e-9);
        assert!((uv_out.v - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_converter_invalid_src_returns_none() {
        let src = ProjectionMap::new(ProjectionType::Equirectangular, 3840, 1920);
        let dst = ProjectionMap::new(ProjectionType::Equirectangular, 3840, 1920);
        let converter = ProjectionConverter::new(src, dst);
        assert!(converter.convert(UvCoord::new(-1.0, 0.5)).is_none());
    }

    #[test]
    fn test_cubemap_maps_to_some() {
        let map = ProjectionMap::new(ProjectionType::Cubemap, 1024, 1024);
        let result = map.map_point(UvCoord::new(0.5, 0.5));
        assert!(result.is_some());
    }
}
