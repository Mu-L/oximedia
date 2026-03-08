//! LED volume and wall configuration for virtual production.
//!
//! Models LED panel specifications, wall segments, and full LED volumes used
//! in in-camera VFX (ICVFX) productions.  Also provides a simple moiré risk
//! assessor that estimates interference risk between LED pixel pitch and camera
//! sensor characteristics.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

// ---------------------------------------------------------------------------
// LedPanelSpec
// ---------------------------------------------------------------------------

/// Physical and electrical specification of a single LED panel tile.
#[derive(Debug, Clone, PartialEq)]
pub struct LedPanelSpec {
    /// Horizontal resolution of the panel in pixels.
    pub width_px: u32,
    /// Vertical resolution of the panel in pixels.
    pub height_px: u32,
    /// Pixel pitch (centre-to-centre distance) in millimetres.
    pub pitch_mm: f32,
    /// Peak brightness in nits (cd/m²).
    pub nits_max: u32,
    /// Panel refresh rate in Hz.
    pub refresh_hz: f32,
}

impl LedPanelSpec {
    /// Create a new LED panel specification.
    #[must_use]
    pub fn new(
        width_px: u32,
        height_px: u32,
        pitch_mm: f32,
        nits_max: u32,
        refresh_hz: f32,
    ) -> Self {
        Self {
            width_px,
            height_px,
            pitch_mm,
            nits_max,
            refresh_hz,
        }
    }

    /// Physical width of the panel in millimetres.
    ///
    /// `width_mm = width_px * pitch_mm`
    #[must_use]
    pub fn physical_width_mm(&self) -> f32 {
        self.width_px as f32 * self.pitch_mm
    }

    /// Physical height of the panel in millimetres.
    ///
    /// `height_mm = height_px * pitch_mm`
    #[must_use]
    pub fn physical_height_mm(&self) -> f32 {
        self.height_px as f32 * self.pitch_mm
    }

    /// Total number of pixels in this panel.
    #[must_use]
    pub fn pixel_count(&self) -> u64 {
        u64::from(self.width_px) * u64::from(self.height_px)
    }
}

// ---------------------------------------------------------------------------
// LedWallSegment
// ---------------------------------------------------------------------------

/// A rectangular array of [`LedPanelSpec`] tiles forming a wall segment.
#[derive(Debug, Clone, PartialEq)]
pub struct LedWallSegment {
    /// Unique identifier for this segment.
    pub id: u32,
    /// Number of panels arranged horizontally.
    pub panels_wide: u32,
    /// Number of panels arranged vertically.
    pub panels_high: u32,
    /// Specification of each individual panel tile.
    pub spec: LedPanelSpec,
    /// Whether the segment is curved.
    pub curved: bool,
    /// Curvature angle of the full segment in degrees (only meaningful when `curved` is true).
    pub curvature_deg: f32,
}

impl LedWallSegment {
    /// Create a flat (non-curved) wall segment.
    #[must_use]
    pub fn flat(id: u32, panels_wide: u32, panels_high: u32, spec: LedPanelSpec) -> Self {
        Self {
            id,
            panels_wide,
            panels_high,
            spec,
            curved: false,
            curvature_deg: 0.0,
        }
    }

    /// Create a curved wall segment.
    #[must_use]
    pub fn curved(
        id: u32,
        panels_wide: u32,
        panels_high: u32,
        spec: LedPanelSpec,
        curvature_deg: f32,
    ) -> Self {
        Self {
            id,
            panels_wide,
            panels_high,
            spec,
            curved: true,
            curvature_deg,
        }
    }

    /// Total physical width of the segment in millimetres.
    #[must_use]
    pub fn total_width_mm(&self) -> f32 {
        self.panels_wide as f32 * self.spec.physical_width_mm()
    }

    /// Total physical height of the segment in millimetres.
    #[must_use]
    pub fn total_height_mm(&self) -> f32 {
        self.panels_high as f32 * self.spec.physical_height_mm()
    }

    /// Total number of pixels in this segment.
    #[must_use]
    pub fn total_pixels(&self) -> u64 {
        u64::from(self.panels_wide) * u64::from(self.panels_high) * self.spec.pixel_count()
    }
}

// ---------------------------------------------------------------------------
// LedVolume
// ---------------------------------------------------------------------------

/// A complete LED volume composed of wall segments plus optional ceiling/floor.
#[derive(Debug, Clone)]
pub struct LedVolume {
    /// Side-wall segments (in display order).
    pub segments: Vec<LedWallSegment>,
    /// Optional ceiling LED panel.
    pub ceiling: Option<LedWallSegment>,
    /// Optional floor LED panel.
    pub floor: Option<LedWallSegment>,
}

impl LedVolume {
    /// Create an empty LED volume.
    #[must_use]
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            ceiling: None,
            floor: None,
        }
    }

    /// Add a wall segment to the volume.
    pub fn add_segment(&mut self, seg: LedWallSegment) {
        self.segments.push(seg);
    }

    /// Maximum nits across all segments (and ceiling/floor if present).
    ///
    /// Returns 0 when the volume is empty.
    #[must_use]
    pub fn total_nits_max(&self) -> u32 {
        let mut max_nits = 0u32;
        for seg in &self.segments {
            if seg.spec.nits_max > max_nits {
                max_nits = seg.spec.nits_max;
            }
        }
        if let Some(c) = &self.ceiling {
            if c.spec.nits_max > max_nits {
                max_nits = c.spec.nits_max;
            }
        }
        if let Some(f) = &self.floor {
            if f.spec.nits_max > max_nits {
                max_nits = f.spec.nits_max;
            }
        }
        max_nits
    }

    /// Total pixel count across all segments, ceiling, and floor.
    #[must_use]
    pub fn total_pixels(&self) -> u64 {
        let mut total: u64 = self.segments.iter().map(LedWallSegment::total_pixels).sum();
        if let Some(c) = &self.ceiling {
            total += c.total_pixels();
        }
        if let Some(f) = &self.floor {
            total += f.total_pixels();
        }
        total
    }

    /// Whether a ceiling panel is installed.
    #[must_use]
    pub fn has_ceiling(&self) -> bool {
        self.ceiling.is_some()
    }
}

impl Default for LedVolume {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// MoireRiskAssessor
// ---------------------------------------------------------------------------

/// Estimates moiré interference risk when shooting an LED wall with a camera.
///
/// Returns a risk score in `[0.0, 1.0]` where 1.0 indicates maximum risk.
pub struct MoireRiskAssessor;

impl MoireRiskAssessor {
    /// Assess moiré risk for a given panel, shooting distance, and lens.
    ///
    /// The heuristic computes the apparent pixel pitch in millimetres as seen
    /// by the sensor, then compares it to a typical sensor pixel pitch.
    ///
    /// # Arguments
    /// * `panel` – LED panel specification.
    /// * `camera_distance_m` – Distance from camera to LED wall in metres.
    /// * `lens_focal_mm` – Effective focal length in millimetres.
    ///
    /// Returns a score in `[0.0, 1.0]`.
    #[must_use]
    pub fn assess(panel: &LedPanelSpec, camera_distance_m: f64, lens_focal_mm: f32) -> f32 {
        if camera_distance_m <= 0.0 || lens_focal_mm <= 0.0 {
            return 1.0; // degenerate case → maximum risk flag
        }
        // Apparent size of one LED pixel on the sensor (mm).
        let pitch_m = f64::from(panel.pitch_mm) / 1_000.0;
        let apparent_mm = (pitch_m / camera_distance_m) * f64::from(lens_focal_mm);

        // Typical full-frame sensor pixel pitch ≈ 0.006 mm.
        let sensor_pixel_pitch_mm = 0.006_f64;

        // Risk is high when apparent_mm approaches an integer multiple of sensor pitch.
        let ratio = apparent_mm / sensor_pixel_pitch_mm;
        let frac = ratio - ratio.floor();
        // Map fractional distance to 0.5 (worst = integer multiple) → risk in [0,1].
        let risk = 1.0 - (2.0 * (frac - 0.5).abs());
        risk.clamp(0.0, 1.0) as f32
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_panel() -> LedPanelSpec {
        LedPanelSpec::new(256, 128, 2.8, 1500, 3840.0)
    }

    #[test]
    fn test_panel_physical_width() {
        let p = LedPanelSpec::new(256, 128, 2.8, 1500, 3840.0);
        let expected = 256.0_f32 * 2.8_f32;
        assert!((p.physical_width_mm() - expected).abs() < 1e-3);
    }

    #[test]
    fn test_panel_physical_height() {
        let p = LedPanelSpec::new(256, 128, 2.8, 1500, 3840.0);
        let expected = 128.0_f32 * 2.8_f32;
        assert!((p.physical_height_mm() - expected).abs() < 1e-3);
    }

    #[test]
    fn test_panel_pixel_count() {
        let p = LedPanelSpec::new(256, 128, 2.8, 1500, 3840.0);
        assert_eq!(p.pixel_count(), 256 * 128);
    }

    #[test]
    fn test_segment_flat_creation() {
        let seg = LedWallSegment::flat(1, 4, 2, sample_panel());
        assert!(!seg.curved);
        assert_eq!(seg.curvature_deg, 0.0);
    }

    #[test]
    fn test_segment_curved_creation() {
        let seg = LedWallSegment::curved(2, 6, 3, sample_panel(), 180.0);
        assert!(seg.curved);
        assert!((seg.curvature_deg - 180.0).abs() < 1e-5);
    }

    #[test]
    fn test_segment_total_width() {
        let spec = LedPanelSpec::new(256, 128, 2.8, 1500, 3840.0);
        let seg = LedWallSegment::flat(1, 4, 2, spec);
        // 4 panels × 256 px × 2.8 mm/px
        let expected = 4.0_f32 * 256.0_f32 * 2.8_f32;
        assert!((seg.total_width_mm() - expected).abs() < 1e-2);
    }

    #[test]
    fn test_segment_total_height() {
        let spec = LedPanelSpec::new(256, 128, 2.8, 1500, 3840.0);
        let seg = LedWallSegment::flat(1, 4, 2, spec);
        let expected = 2.0_f32 * 128.0_f32 * 2.8_f32;
        assert!((seg.total_height_mm() - expected).abs() < 1e-2);
    }

    #[test]
    fn test_segment_total_pixels() {
        let spec = LedPanelSpec::new(256, 128, 2.8, 1500, 3840.0);
        let seg = LedWallSegment::flat(1, 4, 2, spec);
        assert_eq!(seg.total_pixels(), 4 * 2 * 256 * 128);
    }

    #[test]
    fn test_volume_add_segment() {
        let mut vol = LedVolume::new();
        vol.add_segment(LedWallSegment::flat(1, 4, 2, sample_panel()));
        assert_eq!(vol.segments.len(), 1);
    }

    #[test]
    fn test_volume_total_nits_max_empty() {
        let vol = LedVolume::new();
        assert_eq!(vol.total_nits_max(), 0);
    }

    #[test]
    fn test_volume_total_nits_max_segments() {
        let mut vol = LedVolume::new();
        let spec_hi = LedPanelSpec::new(256, 128, 2.8, 2000, 3840.0);
        vol.add_segment(LedWallSegment::flat(1, 4, 2, sample_panel())); // 1500 nits
        vol.add_segment(LedWallSegment::flat(2, 4, 2, spec_hi)); // 2000 nits
        assert_eq!(vol.total_nits_max(), 2000);
    }

    #[test]
    fn test_volume_has_ceiling_false() {
        let vol = LedVolume::new();
        assert!(!vol.has_ceiling());
    }

    #[test]
    fn test_volume_has_ceiling_true() {
        let mut vol = LedVolume::new();
        vol.ceiling = Some(LedWallSegment::flat(99, 4, 2, sample_panel()));
        assert!(vol.has_ceiling());
    }

    #[test]
    fn test_volume_total_pixels_with_ceiling() {
        let mut vol = LedVolume::new();
        let seg = LedWallSegment::flat(1, 1, 1, LedPanelSpec::new(10, 10, 1.0, 1000, 60.0));
        let ceil = LedWallSegment::flat(2, 1, 1, LedPanelSpec::new(10, 10, 1.0, 1000, 60.0));
        vol.add_segment(seg);
        vol.ceiling = Some(ceil);
        // 10*10 + 10*10 = 200
        assert_eq!(vol.total_pixels(), 200);
    }

    #[test]
    fn test_moire_risk_assessor_range() {
        let panel = sample_panel();
        let risk = MoireRiskAssessor::assess(&panel, 5.0, 50.0);
        assert!((0.0..=1.0).contains(&risk));
    }

    #[test]
    fn test_moire_risk_assessor_degenerate() {
        let panel = sample_panel();
        let risk = MoireRiskAssessor::assess(&panel, 0.0, 50.0);
        assert_eq!(risk, 1.0);
    }
}
