#![allow(dead_code)]
//! LED volume calibration for virtual production stages.
//!
//! Implements per-panel colour uniformity, brightness matching,
//! geometric alignment, and verification routines used to prepare
//! an LED volume before a shoot.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Calibration target – what aspect of the volume is being calibrated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CalibrationTarget {
    /// Per-panel colour uniformity.
    ColorUniformity,
    /// Brightness matching across panels.
    Brightness,
    /// Geometric alignment (seam correction).
    Geometry,
    /// Black level calibration.
    BlackLevel,
    /// White point calibration.
    WhitePoint,
    /// Full calibration (all targets).
    Full,
}

impl CalibrationTarget {
    /// Human-readable label.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::ColorUniformity => "Color Uniformity",
            Self::Brightness => "Brightness",
            Self::Geometry => "Geometry",
            Self::BlackLevel => "Black Level",
            Self::WhitePoint => "White Point",
            Self::Full => "Full Calibration",
        }
    }

    /// Returns `true` for targets that affect colour output.
    #[must_use]
    pub fn is_color_related(&self) -> bool {
        matches!(
            self,
            Self::ColorUniformity | Self::WhitePoint | Self::BlackLevel | Self::Full
        )
    }
}

/// Measured colour value in CIE xy + Y space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorMeasurement {
    /// CIE x chromaticity.
    pub x: f64,
    /// CIE y chromaticity.
    pub y: f64,
    /// Luminance in cd/m^2.
    pub luminance: f64,
}

impl ColorMeasurement {
    /// Creates a new measurement.
    #[must_use]
    pub fn new(x: f64, y: f64, luminance: f64) -> Self {
        Self { x, y, luminance }
    }

    /// D65 reference white.
    #[must_use]
    pub fn d65() -> Self {
        Self {
            x: 0.3127,
            y: 0.3290,
            luminance: 100.0,
        }
    }

    /// Euclidean distance in CIE xy plane.
    #[must_use]
    pub fn chromaticity_distance(&self, other: &Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Luminance difference as a ratio (other / self).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn luminance_ratio(&self, other: &Self) -> f64 {
        if self.luminance.abs() < f64::EPSILON {
            return 0.0;
        }
        other.luminance / self.luminance
    }
}

/// Per-panel calibration data.
#[derive(Debug, Clone)]
pub struct PanelCalibration {
    /// Panel identifier.
    pub panel_id: String,
    /// Row index in the wall.
    pub row: u32,
    /// Column index in the wall.
    pub col: u32,
    /// Measured white point.
    pub white_point: ColorMeasurement,
    /// Measured black level.
    pub black_level: ColorMeasurement,
    /// Correction gain per RGB channel.
    pub gain: [f64; 3],
    /// Correction offset per RGB channel.
    pub offset: [f64; 3],
    /// Whether this panel passed calibration.
    pub passed: bool,
}

impl PanelCalibration {
    /// Creates a new uncalibrated panel entry.
    pub fn new(panel_id: impl Into<String>, row: u32, col: u32) -> Self {
        Self {
            panel_id: panel_id.into(),
            row,
            col,
            white_point: ColorMeasurement::d65(),
            black_level: ColorMeasurement::new(0.0, 0.0, 0.0),
            gain: [1.0, 1.0, 1.0],
            offset: [0.0, 0.0, 0.0],
            passed: false,
        }
    }

    /// Applies the gain/offset correction to an input RGB triple (0.0–1.0).
    #[must_use]
    pub fn correct(&self, rgb: [f64; 3]) -> [f64; 3] {
        [
            (rgb[0] * self.gain[0] + self.offset[0]).clamp(0.0, 1.0),
            (rgb[1] * self.gain[1] + self.offset[1]).clamp(0.0, 1.0),
            (rgb[2] * self.gain[2] + self.offset[2]).clamp(0.0, 1.0),
        ]
    }

    /// Contrast ratio (white luminance / black luminance).
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn contrast_ratio(&self) -> f64 {
        if self.black_level.luminance.abs() < f64::EPSILON {
            return f64::INFINITY;
        }
        self.white_point.luminance / self.black_level.luminance
    }
}

/// Overall calibration status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationStatus {
    /// Not yet started.
    Pending,
    /// In progress.
    Running,
    /// Completed successfully – all panels passed.
    Passed,
    /// Completed with some panels failing.
    PartialFail,
    /// Calibration failed.
    Failed,
}

/// Configuration for a calibration run.
#[derive(Debug, Clone)]
pub struct VolumeCalibrationConfig {
    /// Which aspect to calibrate.
    pub target: CalibrationTarget,
    /// Maximum acceptable chromaticity drift from reference.
    pub max_chroma_drift: f64,
    /// Maximum acceptable luminance deviation (fraction, e.g. 0.05 = 5%).
    pub max_luma_deviation: f64,
    /// Reference white point.
    pub reference_white: ColorMeasurement,
    /// Number of measurement samples per panel.
    pub samples_per_panel: u32,
}

impl Default for VolumeCalibrationConfig {
    fn default() -> Self {
        Self {
            target: CalibrationTarget::Full,
            max_chroma_drift: 0.005,
            max_luma_deviation: 0.03,
            reference_white: ColorMeasurement::d65(),
            samples_per_panel: 5,
        }
    }
}

/// Statistics for a calibration run.
#[derive(Debug, Clone)]
pub struct CalibrationStats {
    /// Total panels calibrated.
    pub panels_total: u32,
    /// Panels that passed.
    pub panels_passed: u32,
    /// Panels that failed.
    pub panels_failed: u32,
    /// Average chromaticity drift.
    pub avg_chroma_drift: f64,
    /// Maximum chromaticity drift observed.
    pub max_chroma_drift_observed: f64,
    /// Average luminance deviation.
    pub avg_luma_deviation: f64,
    /// Calibration duration.
    pub duration: Duration,
}

impl CalibrationStats {
    /// Creates zeroed stats.
    #[must_use]
    pub fn new() -> Self {
        Self {
            panels_total: 0,
            panels_passed: 0,
            panels_failed: 0,
            avg_chroma_drift: 0.0,
            max_chroma_drift_observed: 0.0,
            avg_luma_deviation: 0.0,
            duration: Duration::ZERO,
        }
    }

    /// Pass rate as a fraction.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn pass_rate(&self) -> f64 {
        if self.panels_total == 0 {
            return 0.0;
        }
        f64::from(self.panels_passed) / f64::from(self.panels_total)
    }
}

impl Default for CalibrationStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Volume calibration manager.
pub struct VolumeCalibrator {
    /// Configuration.
    config: VolumeCalibrationConfig,
    /// Per-panel calibration data, keyed by `panel_id`.
    panels: HashMap<String, PanelCalibration>,
    /// Current status.
    status: CalibrationStatus,
    /// Statistics.
    stats: CalibrationStats,
    /// When the current run started.
    started_at: Option<Instant>,
}

impl VolumeCalibrator {
    /// Creates a new calibrator.
    #[must_use]
    pub fn new(config: VolumeCalibrationConfig) -> Self {
        Self {
            config,
            panels: HashMap::new(),
            status: CalibrationStatus::Pending,
            stats: CalibrationStats::new(),
            started_at: None,
        }
    }

    /// Registers a panel for calibration.
    pub fn add_panel(&mut self, panel: PanelCalibration) {
        self.panels.insert(panel.panel_id.clone(), panel);
    }

    /// Returns the number of registered panels.
    #[must_use]
    pub fn panel_count(&self) -> usize {
        self.panels.len()
    }

    /// Gets a panel by ID.
    #[must_use]
    pub fn get_panel(&self, panel_id: &str) -> Option<&PanelCalibration> {
        self.panels.get(panel_id)
    }

    /// Starts a calibration run.
    pub fn start(&mut self) {
        self.status = CalibrationStatus::Running;
        self.started_at = Some(Instant::now());
        self.stats = CalibrationStats::new();
    }

    /// Evaluates a single panel against the configuration thresholds.
    pub fn evaluate_panel(&mut self, panel_id: &str) -> bool {
        let config = self.config.clone();
        if let Some(panel) = self.panels.get_mut(panel_id) {
            let chroma_drift = panel
                .white_point
                .chromaticity_distance(&config.reference_white);
            let luma_ratio = config.reference_white.luminance_ratio(&panel.white_point);
            let luma_dev = (luma_ratio - 1.0).abs();

            let pass =
                chroma_drift <= config.max_chroma_drift && luma_dev <= config.max_luma_deviation;

            panel.passed = pass;
            self.stats.panels_total += 1;
            if pass {
                self.stats.panels_passed += 1;
            } else {
                self.stats.panels_failed += 1;
            }

            if chroma_drift > self.stats.max_chroma_drift_observed {
                self.stats.max_chroma_drift_observed = chroma_drift;
            }

            pass
        } else {
            false
        }
    }

    /// Finishes calibration and computes final stats.
    pub fn finish(&mut self) {
        if let Some(start) = self.started_at.take() {
            self.stats.duration = start.elapsed();
        }
        if self.stats.panels_failed == 0 && self.stats.panels_total > 0 {
            self.status = CalibrationStatus::Passed;
        } else if self.stats.panels_passed > 0 {
            self.status = CalibrationStatus::PartialFail;
        } else if self.stats.panels_total > 0 {
            self.status = CalibrationStatus::Failed;
        }
    }

    /// Returns current calibration status.
    #[must_use]
    pub fn status(&self) -> CalibrationStatus {
        self.status
    }

    /// Returns current stats.
    #[must_use]
    pub fn stats(&self) -> &CalibrationStats {
        &self.stats
    }

    /// Returns the configuration.
    #[must_use]
    pub fn config(&self) -> &VolumeCalibrationConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calibration_target_label() {
        assert_eq!(CalibrationTarget::Full.label(), "Full Calibration");
        assert_eq!(CalibrationTarget::Brightness.label(), "Brightness");
    }

    #[test]
    fn test_calibration_target_is_color_related() {
        assert!(CalibrationTarget::ColorUniformity.is_color_related());
        assert!(CalibrationTarget::Full.is_color_related());
        assert!(!CalibrationTarget::Geometry.is_color_related());
        assert!(!CalibrationTarget::Brightness.is_color_related());
    }

    #[test]
    fn test_color_measurement_distance_zero() {
        let d65 = ColorMeasurement::d65();
        assert!(d65.chromaticity_distance(&d65) < f64::EPSILON);
    }

    #[test]
    fn test_color_measurement_distance_nonzero() {
        let a = ColorMeasurement::new(0.3, 0.3, 100.0);
        let b = ColorMeasurement::new(0.4, 0.3, 100.0);
        assert!((a.chromaticity_distance(&b) - 0.1).abs() < 1e-9);
    }

    #[test]
    fn test_luminance_ratio() {
        let a = ColorMeasurement::new(0.3, 0.3, 100.0);
        let b = ColorMeasurement::new(0.3, 0.3, 95.0);
        assert!((a.luminance_ratio(&b) - 0.95).abs() < 1e-9);
    }

    #[test]
    fn test_panel_correction() {
        let mut panel = PanelCalibration::new("P01", 0, 0);
        panel.gain = [1.1, 0.9, 1.0];
        panel.offset = [0.0, 0.05, 0.0];
        let corrected = panel.correct([0.5, 0.5, 0.5]);
        assert!((corrected[0] - 0.55).abs() < 1e-9);
        assert!((corrected[1] - 0.50).abs() < 1e-9);
        assert!((corrected[2] - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_panel_contrast_ratio() {
        let mut panel = PanelCalibration::new("P01", 0, 0);
        panel.white_point = ColorMeasurement::new(0.31, 0.33, 500.0);
        panel.black_level = ColorMeasurement::new(0.31, 0.33, 0.5);
        assert!((panel.contrast_ratio() - 1000.0).abs() < 1e-6);
    }

    #[test]
    fn test_panel_correction_clamping() {
        let mut panel = PanelCalibration::new("P02", 0, 1);
        panel.gain = [2.0, 2.0, 2.0];
        let corrected = panel.correct([0.8, 0.9, 1.0]);
        assert!((corrected[0] - 1.0).abs() < 1e-9);
        assert!((corrected[1] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_calibrator_add_panels() {
        let mut cal = VolumeCalibrator::new(VolumeCalibrationConfig::default());
        cal.add_panel(PanelCalibration::new("A1", 0, 0));
        cal.add_panel(PanelCalibration::new("A2", 0, 1));
        assert_eq!(cal.panel_count(), 2);
        assert!(cal.get_panel("A1").is_some());
    }

    #[test]
    fn test_calibrator_evaluate_passing_panel() {
        let mut cal = VolumeCalibrator::new(VolumeCalibrationConfig::default());
        let mut panel = PanelCalibration::new("P1", 0, 0);
        panel.white_point = ColorMeasurement::d65();
        cal.add_panel(panel);
        cal.start();
        let pass = cal.evaluate_panel("P1");
        assert!(pass);
        assert_eq!(cal.stats().panels_passed, 1);
    }

    #[test]
    fn test_calibrator_evaluate_failing_panel() {
        let mut cal = VolumeCalibrator::new(VolumeCalibrationConfig::default());
        let mut panel = PanelCalibration::new("P1", 0, 0);
        panel.white_point = ColorMeasurement::new(0.4, 0.4, 50.0);
        cal.add_panel(panel);
        cal.start();
        let pass = cal.evaluate_panel("P1");
        assert!(!pass);
        assert_eq!(cal.stats().panels_failed, 1);
    }

    #[test]
    fn test_calibrator_finish_all_passed() {
        let mut cal = VolumeCalibrator::new(VolumeCalibrationConfig::default());
        cal.add_panel(PanelCalibration::new("P1", 0, 0));
        cal.start();
        cal.evaluate_panel("P1");
        cal.finish();
        assert_eq!(cal.status(), CalibrationStatus::Passed);
    }

    #[test]
    fn test_calibration_stats_pass_rate() {
        let mut stats = CalibrationStats::new();
        stats.panels_total = 10;
        stats.panels_passed = 8;
        stats.panels_failed = 2;
        assert!((stats.pass_rate() - 0.8).abs() < 1e-9);
    }

    #[test]
    fn test_default_config() {
        let cfg = VolumeCalibrationConfig::default();
        assert_eq!(cfg.target, CalibrationTarget::Full);
        assert!((cfg.max_chroma_drift - 0.005).abs() < f64::EPSILON);
        assert_eq!(cfg.samples_per_panel, 5);
    }
}
