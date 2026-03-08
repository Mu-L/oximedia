#![allow(dead_code)]
//! False color mapping for exposure, focus, and motion visualization.
//!
//! Provides flexible false color processing with configurable zones
//! and color mappings for broadcast camera operators.

/// Categorizes the purpose of a false color visualization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FalseColorMapping {
    /// Exposure false color (overexposed / underexposed zones).
    Exposure,
    /// Focus peaking false color (sharp-edge highlight).
    Focus,
    /// Motion vector false color (pixel displacement magnitude).
    Motion,
}

impl FalseColorMapping {
    /// Returns a short human-readable label for the mapping type.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Exposure => "Exposure",
            Self::Focus => "Focus",
            Self::Motion => "Motion",
        }
    }

    /// Returns whether this mapping type uses an overlay rather than a full
    /// frame replace.
    #[must_use]
    pub fn is_overlay(self) -> bool {
        matches!(self, Self::Focus | Self::Motion)
    }
}

/// An RGBA color expressed as four bytes (r, g, b, a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl Rgba {
    /// Fully opaque white.
    pub const WHITE: Self = Self(255, 255, 255, 255);
    /// Fully opaque black.
    pub const BLACK: Self = Self(0, 0, 0, 255);
    /// Fully transparent.
    pub const TRANSPARENT: Self = Self(0, 0, 0, 0);
}

/// A single threshold entry in a false color scale.
///
/// When a luma value falls within `[lower, upper)` the corresponding
/// `color` is applied.
#[derive(Debug, Clone)]
pub struct FalseColorThreshold {
    /// Lower bound (inclusive), in IRE units `[0, 109]`.
    pub lower: f32,
    /// Upper bound (exclusive), in IRE units `[0, 109]`.
    pub upper: f32,
    /// Color to apply for luma values in this range.
    pub color: Rgba,
}

impl FalseColorThreshold {
    /// Creates a new threshold entry.
    #[must_use]
    pub fn new(lower: f32, upper: f32, color: Rgba) -> Self {
        Self {
            lower,
            upper,
            color,
        }
    }

    /// Returns the color this threshold maps to, or `None` when the luma
    /// value falls outside the range.
    #[must_use]
    pub fn maps_to_color(&self, luma_ire: f32) -> Option<Rgba> {
        if luma_ire >= self.lower && luma_ire < self.upper {
            Some(self.color)
        } else {
            None
        }
    }

    /// Returns the midpoint IRE value for this zone.
    #[must_use]
    pub fn midpoint(&self) -> f32 {
        (self.lower + self.upper) * 0.5
    }
}

/// A complete false color scale composed of ordered threshold entries.
#[derive(Debug, Clone, Default)]
pub struct FalseColorScale {
    thresholds: Vec<FalseColorThreshold>,
}

impl FalseColorScale {
    /// Creates an empty scale (all pixels pass through unchanged).
    #[must_use]
    pub fn new() -> Self {
        Self {
            thresholds: Vec::new(),
        }
    }

    /// Adds a threshold zone to the scale.
    pub fn add_threshold(&mut self, t: FalseColorThreshold) {
        self.thresholds.push(t);
    }

    /// Returns the number of defined threshold zones.
    #[must_use]
    pub fn zone_count(&self) -> usize {
        self.thresholds.len()
    }

    /// Looks up the color for a given luma IRE value.
    ///
    /// Returns `None` if no threshold matches.
    #[must_use]
    pub fn lookup(&self, luma_ire: f32) -> Option<Rgba> {
        for t in &self.thresholds {
            if let Some(c) = t.maps_to_color(luma_ire) {
                return Some(c);
            }
        }
        None
    }
}

/// Processes video frame data applying false color visualization.
#[derive(Debug, Clone)]
pub struct FalseColorProcessor {
    mapping: FalseColorMapping,
    scale: FalseColorScale,
}

impl FalseColorProcessor {
    /// Creates a new processor with a given mapping type and color scale.
    #[must_use]
    pub fn new(mapping: FalseColorMapping, scale: FalseColorScale) -> Self {
        Self { mapping, scale }
    }

    /// Returns the mapping type used by this processor.
    #[must_use]
    pub fn mapping(&self) -> FalseColorMapping {
        self.mapping
    }

    /// Applies false color to a single luma IRE value.
    ///
    /// Returns the replacement color, or `None` if the value is in a
    /// "neutral" zone (no threshold matches).
    #[must_use]
    pub fn apply(&self, luma_ire: f32) -> Option<Rgba> {
        self.scale.lookup(luma_ire)
    }

    /// Processes a full luma plane (values in `[0.0, 109.0]` IRE) and
    /// returns per-pixel replacement colors.
    ///
    /// Pixels for which no threshold matches are returned as `None`.
    #[must_use]
    pub fn apply_frame(&self, luma_plane: &[f32]) -> Vec<Option<Rgba>> {
        luma_plane.iter().map(|&v| self.apply(v)).collect()
    }

    /// Calculates the fraction of pixels that fall within any threshold zone.
    ///
    /// Returns a value in `[0.0, 1.0]`.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn zone_coverage_pct(&self, luma_plane: &[f32]) -> f32 {
        if luma_plane.is_empty() {
            return 0.0;
        }
        let hits = luma_plane
            .iter()
            .filter(|&&v| self.apply(v).is_some())
            .count();
        hits as f32 / luma_plane.len() as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exposure_scale() -> FalseColorScale {
        let mut s = FalseColorScale::new();
        // underexposed zone: 0–20 IRE → blue
        s.add_threshold(FalseColorThreshold::new(0.0, 20.0, Rgba(0, 0, 255, 255)));
        // skin tone zone: 55–65 IRE → pink
        s.add_threshold(FalseColorThreshold::new(
            55.0,
            65.0,
            Rgba(255, 128, 128, 255),
        ));
        // overexposed zone: 90–109 IRE → red
        s.add_threshold(FalseColorThreshold::new(90.0, 109.0, Rgba(255, 0, 0, 255)));
        s
    }

    #[test]
    fn test_mapping_label_exposure() {
        assert_eq!(FalseColorMapping::Exposure.label(), "Exposure");
    }

    #[test]
    fn test_mapping_label_focus() {
        assert_eq!(FalseColorMapping::Focus.label(), "Focus");
    }

    #[test]
    fn test_mapping_label_motion() {
        assert_eq!(FalseColorMapping::Motion.label(), "Motion");
    }

    #[test]
    fn test_mapping_is_overlay_focus() {
        assert!(FalseColorMapping::Focus.is_overlay());
    }

    #[test]
    fn test_mapping_is_overlay_exposure_false() {
        assert!(!FalseColorMapping::Exposure.is_overlay());
    }

    #[test]
    fn test_threshold_maps_to_color_hit() {
        let t = FalseColorThreshold::new(0.0, 20.0, Rgba(0, 0, 255, 255));
        assert_eq!(t.maps_to_color(10.0), Some(Rgba(0, 0, 255, 255)));
    }

    #[test]
    fn test_threshold_maps_to_color_miss() {
        let t = FalseColorThreshold::new(0.0, 20.0, Rgba(0, 0, 255, 255));
        assert_eq!(t.maps_to_color(50.0), None);
    }

    #[test]
    fn test_threshold_lower_bound_inclusive() {
        let t = FalseColorThreshold::new(20.0, 40.0, Rgba(255, 255, 0, 255));
        assert!(t.maps_to_color(20.0).is_some());
    }

    #[test]
    fn test_threshold_upper_bound_exclusive() {
        let t = FalseColorThreshold::new(20.0, 40.0, Rgba(255, 255, 0, 255));
        assert!(t.maps_to_color(40.0).is_none());
    }

    #[test]
    fn test_threshold_midpoint() {
        let t = FalseColorThreshold::new(10.0, 30.0, Rgba::WHITE);
        assert!((t.midpoint() - 20.0).abs() < 1e-6);
    }

    #[test]
    fn test_scale_zone_count() {
        let s = exposure_scale();
        assert_eq!(s.zone_count(), 3);
    }

    #[test]
    fn test_processor_apply_underexposed() {
        let proc = FalseColorProcessor::new(FalseColorMapping::Exposure, exposure_scale());
        assert_eq!(proc.apply(10.0), Some(Rgba(0, 0, 255, 255)));
    }

    #[test]
    fn test_processor_apply_neutral_zone() {
        let proc = FalseColorProcessor::new(FalseColorMapping::Exposure, exposure_scale());
        // 70 IRE has no threshold
        assert_eq!(proc.apply(70.0), None);
    }

    #[test]
    fn test_processor_zone_coverage_pct() {
        let proc = FalseColorProcessor::new(FalseColorMapping::Exposure, exposure_scale());
        // 2 out of 4 values fall in a zone (10 and 95)
        let plane = vec![10.0, 50.0, 70.0, 95.0];
        let pct = proc.zone_coverage_pct(&plane);
        assert!((pct - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_processor_zone_coverage_empty() {
        let proc = FalseColorProcessor::new(FalseColorMapping::Exposure, exposure_scale());
        assert_eq!(proc.zone_coverage_pct(&[]), 0.0);
    }

    #[test]
    fn test_rgba_constants() {
        assert_eq!(Rgba::WHITE, Rgba(255, 255, 255, 255));
        assert_eq!(Rgba::BLACK, Rgba(0, 0, 0, 255));
        assert_eq!(Rgba::TRANSPARENT.3, 0);
    }
}
