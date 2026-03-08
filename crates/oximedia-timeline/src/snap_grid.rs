//! Snap-to-grid functionality for the timeline editor.
//!
//! Provides configurable snap points (clip edges, markers, bar/beat grid,
//! playhead) and the logic to find the nearest one for an arbitrary frame
//! position.

/// A single point on the timeline that the cursor can snap to.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapPoint {
    /// Frame position of this snap point.
    pub frame: u64,
    /// Human-readable description (e.g. "Clip start", "Bar 4").
    pub label: String,
    /// Category of this snap point.
    pub kind: SnapKind,
}

impl SnapPoint {
    /// Create a new snap point.
    #[must_use]
    pub fn new(frame: u64, label: impl Into<String>, kind: SnapKind) -> Self {
        Self {
            frame,
            label: label.into(),
            kind,
        }
    }
}

/// The type of snap point.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapKind {
    /// Leading edge of a clip.
    ClipStart,
    /// Trailing edge of a clip.
    ClipEnd,
    /// A user-placed marker.
    Marker,
    /// A bar or beat boundary in the musical grid.
    BeatGrid,
    /// The current playhead position.
    Playhead,
    /// A chapter point.
    Chapter,
}

impl SnapKind {
    /// Returns `true` for structural points (clip edges / chapters).
    #[must_use]
    pub fn is_structural(&self) -> bool {
        matches!(self, Self::ClipStart | Self::ClipEnd | Self::Chapter)
    }
}

/// Which categories of snap points are active.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SnapConfig {
    /// Snap to clip start/end edges.
    pub snap_to_clips: bool,
    /// Snap to markers.
    pub snap_to_markers: bool,
    /// Snap to beat-grid points.
    pub snap_to_beats: bool,
    /// Snap to the playhead position.
    pub snap_to_playhead: bool,
    /// Maximum distance (in frames) within which snapping activates.
    pub snap_radius: u64,
}

impl Default for SnapConfig {
    fn default() -> Self {
        Self {
            snap_to_clips: true,
            snap_to_markers: true,
            snap_to_beats: false,
            snap_to_playhead: true,
            snap_radius: 5,
        }
    }
}

impl SnapConfig {
    /// Returns `true` if the given snap kind is enabled.
    #[must_use]
    pub fn is_enabled(&self, kind: SnapKind) -> bool {
        match kind {
            SnapKind::ClipStart | SnapKind::ClipEnd => self.snap_to_clips,
            SnapKind::Marker | SnapKind::Chapter => self.snap_to_markers,
            SnapKind::BeatGrid => self.snap_to_beats,
            SnapKind::Playhead => self.snap_to_playhead,
        }
    }
}

/// Manages a collection of snap points and resolves snap queries.
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct SnapGrid {
    points: Vec<SnapPoint>,
    config: SnapConfig,
}

impl SnapGrid {
    /// Create a new, empty snap grid with default config.
    #[must_use]
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            config: SnapConfig::default(),
        }
    }

    /// Create a snap grid with custom configuration.
    #[must_use]
    pub fn with_config(config: SnapConfig) -> Self {
        Self {
            points: Vec::new(),
            config,
        }
    }

    /// Add a snap point.
    pub fn add(&mut self, point: SnapPoint) {
        self.points.push(point);
    }

    /// Add a beat-grid at a regular interval starting from `origin`.
    pub fn add_beat_grid(&mut self, origin: u64, interval: u64, count: u32) {
        for i in 0..count {
            let frame = origin + interval * u64::from(i);
            self.add(SnapPoint::new(
                frame,
                format!("Beat {}", i + 1),
                SnapKind::BeatGrid,
            ));
        }
    }

    /// Total number of snap points registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Returns `true` when there are no snap points.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Clear all snap points.
    pub fn clear(&mut self) {
        self.points.clear();
    }

    /// Find the nearest enabled snap point within the snap radius.
    ///
    /// Returns `None` when no snap point is close enough.
    #[must_use]
    pub fn nearest(&self, frame: u64) -> Option<&SnapPoint> {
        self.points
            .iter()
            .filter(|p| self.config.is_enabled(p.kind))
            .filter_map(|p| {
                let dist = p.frame.abs_diff(frame);
                if dist <= self.config.snap_radius {
                    Some((dist, p))
                } else {
                    None
                }
            })
            .min_by_key(|(dist, _)| *dist)
            .map(|(_, p)| p)
    }

    /// Snap `frame` to the nearest enabled point, or return `frame` unchanged.
    #[must_use]
    pub fn snap(&self, frame: u64) -> u64 {
        self.nearest(frame).map_or(frame, |p| p.frame)
    }

    /// All snap points of the given kind.
    pub fn by_kind(&self, kind: SnapKind) -> impl Iterator<Item = &SnapPoint> {
        self.points.iter().filter(move |p| p.kind == kind)
    }

    /// Current snap configuration.
    #[must_use]
    pub fn config(&self) -> &SnapConfig {
        &self.config
    }

    /// Update the snap configuration.
    pub fn set_config(&mut self, config: SnapConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_grid() -> SnapGrid {
        let mut g = SnapGrid::new();
        g.add(SnapPoint::new(0, "Clip start", SnapKind::ClipStart));
        g.add(SnapPoint::new(100, "Clip end", SnapKind::ClipEnd));
        g.add(SnapPoint::new(50, "Marker", SnapKind::Marker));
        g
    }

    #[test]
    fn test_snap_to_exact() {
        let g = make_grid();
        assert_eq!(g.snap(100), 100);
    }

    #[test]
    fn test_snap_within_radius() {
        let g = make_grid();
        // frame 3 is within default radius (5) of snap point 0
        assert_eq!(g.snap(3), 0);
    }

    #[test]
    fn test_snap_outside_radius_unchanged() {
        let g = make_grid();
        // frame 20 is far from all points (nearest is 0, distance 20)
        assert_eq!(g.snap(20), 20);
    }

    #[test]
    fn test_nearest_returns_closest() {
        let g = make_grid();
        // frame 48 is 2 away from 50 (Marker) and 48 away from 0
        let p = g.nearest(48).expect("should succeed in test");
        assert_eq!(p.frame, 50);
    }

    #[test]
    fn test_nearest_none_when_out_of_range() {
        let g = make_grid();
        assert!(g.nearest(30).is_none());
    }

    #[test]
    fn test_disabled_kind_not_snapped() {
        let config = SnapConfig {
            snap_to_markers: false,
            snap_radius: 10,
            ..SnapConfig::default()
        };
        let mut g = SnapGrid::with_config(config);
        g.add(SnapPoint::new(50, "Marker", SnapKind::Marker));
        // Marker is disabled, so no snap
        assert_eq!(g.snap(50), 50);
    }

    #[test]
    fn test_beat_grid_added() {
        let mut g = SnapGrid::new();
        g.add_beat_grid(0, 24, 4);
        assert_eq!(g.len(), 4);
        let frames: Vec<u64> = g.by_kind(SnapKind::BeatGrid).map(|p| p.frame).collect();
        assert_eq!(frames, vec![0, 24, 48, 72]);
    }

    #[test]
    fn test_clear() {
        let mut g = make_grid();
        g.clear();
        assert!(g.is_empty());
    }

    #[test]
    fn test_len() {
        let g = make_grid();
        assert_eq!(g.len(), 3);
    }

    #[test]
    fn test_by_kind_filter() {
        let g = make_grid();
        let clips: Vec<_> = g.by_kind(SnapKind::ClipStart).collect();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].frame, 0);
    }

    #[test]
    fn test_snap_kind_is_structural() {
        assert!(SnapKind::ClipStart.is_structural());
        assert!(SnapKind::ClipEnd.is_structural());
        assert!(SnapKind::Chapter.is_structural());
        assert!(!SnapKind::Marker.is_structural());
        assert!(!SnapKind::BeatGrid.is_structural());
    }

    #[test]
    fn test_config_is_enabled() {
        let c = SnapConfig::default();
        assert!(c.is_enabled(SnapKind::ClipStart));
        assert!(!c.is_enabled(SnapKind::BeatGrid));
    }

    #[test]
    fn test_set_config() {
        let mut g = SnapGrid::new();
        let cfg = SnapConfig {
            snap_radius: 20,
            ..SnapConfig::default()
        };
        g.set_config(cfg);
        assert_eq!(g.config().snap_radius, 20);
    }
}
