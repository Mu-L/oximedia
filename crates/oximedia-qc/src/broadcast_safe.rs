//! Broadcast safe level checking for video frames.
//!
//! This module checks that video pixel values conform to broadcast
//! specifications (NTSC, PAL, HD) for luma and chroma levels.

/// Configuration for broadcast safe checking.
#[derive(Debug, Clone)]
pub struct BroadcastSafeConfig {
    /// Maximum allowed luma value (Y channel).
    pub max_luma: u8,
    /// Minimum allowed luma value (Y channel).
    pub min_luma: u8,
    /// Maximum allowed chroma value (Cb/Cr channels).
    pub max_chroma: u8,
    /// Whether to apply composite-safe constraints (more restrictive).
    pub composite_safe: bool,
}

impl BroadcastSafeConfig {
    /// NTSC broadcast safe levels: luma 16–235, chroma 16–240.
    #[must_use]
    pub fn ntsc() -> Self {
        Self {
            max_luma: 235,
            min_luma: 16,
            max_chroma: 240,
            composite_safe: true,
        }
    }

    /// PAL broadcast safe levels: luma 16–235, chroma 16–240.
    #[must_use]
    pub fn pal() -> Self {
        Self {
            max_luma: 235,
            min_luma: 16,
            max_chroma: 240,
            composite_safe: true,
        }
    }

    /// HD broadcast safe levels: luma 16–235, chroma 16–240.
    #[must_use]
    pub fn hd() -> Self {
        Self {
            max_luma: 235,
            min_luma: 16,
            max_chroma: 240,
            composite_safe: false,
        }
    }

    /// Full range (no restrictions).
    #[must_use]
    pub fn full_range() -> Self {
        Self {
            max_luma: 255,
            min_luma: 0,
            max_chroma: 255,
            composite_safe: false,
        }
    }
}

impl Default for BroadcastSafeConfig {
    fn default() -> Self {
        Self::ntsc()
    }
}

/// The type of pixel level violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationType {
    /// Luma value exceeds maximum.
    LumaAbove,
    /// Luma value is below minimum.
    LumaBelow,
    /// Chroma value exceeds maximum.
    ChromaAbove,
    /// Chroma value is below minimum (16 for broadcast).
    ChromaBelow,
}

impl ViolationType {
    /// Returns a human-readable description of the violation type.
    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            Self::LumaAbove => "Luma exceeds maximum",
            Self::LumaBelow => "Luma below minimum",
            Self::ChromaAbove => "Chroma exceeds maximum",
            Self::ChromaBelow => "Chroma below minimum",
        }
    }
}

/// A pixel that violates broadcast safe levels.
#[derive(Debug, Clone)]
pub struct PixelViolation {
    /// Frame index where the violation occurred.
    pub frame: u64,
    /// X coordinate of the violating pixel.
    pub x: u32,
    /// Y coordinate of the violating pixel.
    pub y: u32,
    /// Type of violation.
    pub violation_type: ViolationType,
    /// The actual pixel value that violated the constraint.
    pub value: u8,
}

impl PixelViolation {
    /// Creates a new pixel violation.
    #[must_use]
    pub fn new(frame: u64, x: u32, y: u32, violation_type: ViolationType, value: u8) -> Self {
        Self {
            frame,
            x,
            y,
            violation_type,
            value,
        }
    }
}

/// Checker for broadcast safe violations in video frames.
#[derive(Debug, Clone, Default)]
pub struct BroadcastSafeChecker;

impl BroadcastSafeChecker {
    /// Creates a new broadcast safe checker.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Checks a single planar YUV frame for broadcast safe violations.
    ///
    /// `frame` is expected to be a planar buffer in Y-then-U-then-V order,
    /// where the Y plane is `width * height` bytes and U/V planes are each
    /// `(width/2) * (height/2)` bytes (4:2:0 subsampling).
    ///
    /// Returns a list of violations found in this frame.
    #[must_use]
    pub fn check_frame(
        frame: &[u8],
        width: u32,
        height: u32,
        frame_idx: u64,
        config: &BroadcastSafeConfig,
    ) -> Vec<PixelViolation> {
        let mut violations = Vec::new();
        let luma_size = (width * height) as usize;

        // Check luma plane (Y)
        let y_plane = &frame[..luma_size.min(frame.len())];
        for (i, &y) in y_plane.iter().enumerate() {
            let x = (i as u32) % width;
            let row = (i as u32) / width;
            if y > config.max_luma {
                violations.push(PixelViolation::new(
                    frame_idx,
                    x,
                    row,
                    ViolationType::LumaAbove,
                    y,
                ));
            } else if y < config.min_luma {
                violations.push(PixelViolation::new(
                    frame_idx,
                    x,
                    row,
                    ViolationType::LumaBelow,
                    y,
                ));
            }
        }

        // Check chroma planes (Cb, Cr) if present in the buffer
        let chroma_size = ((width / 2) * (height / 2)) as usize;
        let chroma_start = luma_size;
        let chroma_end = (chroma_start + chroma_size * 2).min(frame.len());

        if chroma_start < frame.len() {
            let chroma_plane = &frame[chroma_start..chroma_end];
            let chroma_w = width / 2;

            for (i, &c) in chroma_plane.iter().enumerate() {
                let x = (i as u32) % chroma_w;
                let row = (i as u32) / chroma_w;
                if c > config.max_chroma {
                    violations.push(PixelViolation::new(
                        frame_idx,
                        x,
                        row,
                        ViolationType::ChromaAbove,
                        c,
                    ));
                } else if c < config.min_luma {
                    // Chroma minimum is same as luma minimum (16) for broadcast
                    violations.push(PixelViolation::new(
                        frame_idx,
                        x,
                        row,
                        ViolationType::ChromaBelow,
                        c,
                    ));
                }
            }
        }

        violations
    }
}

/// Summary report for broadcast safe checking across all frames.
#[derive(Debug, Clone, Default)]
pub struct BroadcastSafeReport {
    /// Total number of frames analyzed.
    pub total_frames: u64,
    /// Number of frames with at least one violation.
    pub violating_frames: u64,
    /// Total number of pixel violations.
    pub total_violations: u64,
    /// Index of the frame with the most violations (if any).
    pub worst_frame: Option<u64>,
}

impl BroadcastSafeReport {
    /// Creates an empty report.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns whether any violations were found.
    #[must_use]
    pub fn has_violations(&self) -> bool {
        self.total_violations > 0
    }

    /// Returns the violation rate as a fraction of frames.
    #[must_use]
    pub fn violation_rate(&self) -> f64 {
        if self.total_frames == 0 {
            return 0.0;
        }
        self.violating_frames as f64 / self.total_frames as f64
    }

    /// Builds a report from per-frame violation lists.
    #[must_use]
    pub fn from_frame_violations(violations_per_frame: &[(u64, Vec<PixelViolation>)]) -> Self {
        let total_frames = violations_per_frame.len() as u64;
        let mut total_violations = 0u64;
        let mut violating_frames = 0u64;
        let mut worst_count = 0usize;
        let mut worst_frame = None;

        for (frame_idx, violations) in violations_per_frame {
            let count = violations.len();
            total_violations += count as u64;
            if count > 0 {
                violating_frames += 1;
                if count > worst_count {
                    worst_count = count;
                    worst_frame = Some(*frame_idx);
                }
            }
        }

        Self {
            total_frames,
            violating_frames,
            total_violations,
            worst_frame,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(width: u32, height: u32, y_val: u8, uv_val: u8) -> Vec<u8> {
        let y_size = (width * height) as usize;
        let uv_size = ((width / 2) * (height / 2)) as usize;
        let mut frame = vec![y_val; y_size];
        frame.extend(vec![uv_val; uv_size * 2]);
        frame
    }

    #[test]
    fn test_ntsc_config() {
        let cfg = BroadcastSafeConfig::ntsc();
        assert_eq!(cfg.max_luma, 235);
        assert_eq!(cfg.min_luma, 16);
        assert_eq!(cfg.max_chroma, 240);
        assert!(cfg.composite_safe);
    }

    #[test]
    fn test_pal_config() {
        let cfg = BroadcastSafeConfig::pal();
        assert_eq!(cfg.max_luma, 235);
        assert_eq!(cfg.min_luma, 16);
    }

    #[test]
    fn test_hd_config() {
        let cfg = BroadcastSafeConfig::hd();
        assert!(!cfg.composite_safe);
    }

    #[test]
    fn test_no_violations_in_range() {
        let cfg = BroadcastSafeConfig::ntsc();
        // All pixels at Y=128, UV=128 — within range
        let frame = make_frame(4, 4, 128, 128);
        let violations = BroadcastSafeChecker::check_frame(&frame, 4, 4, 0, &cfg);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_luma_above_violation() {
        let cfg = BroadcastSafeConfig::ntsc();
        let frame = make_frame(4, 4, 255, 128); // Y=255 exceeds 235
        let violations = BroadcastSafeChecker::check_frame(&frame, 4, 4, 0, &cfg);
        assert!(!violations.is_empty());
        assert_eq!(violations[0].violation_type, ViolationType::LumaAbove);
        assert_eq!(violations[0].value, 255);
    }

    #[test]
    fn test_luma_below_violation() {
        let cfg = BroadcastSafeConfig::ntsc();
        let frame = make_frame(4, 4, 0, 128); // Y=0 < 16
        let violations = BroadcastSafeChecker::check_frame(&frame, 4, 4, 0, &cfg);
        assert!(!violations.is_empty());
        assert!(violations
            .iter()
            .any(|v| v.violation_type == ViolationType::LumaBelow));
    }

    #[test]
    fn test_chroma_above_violation() {
        let cfg = BroadcastSafeConfig::ntsc();
        let frame = make_frame(4, 4, 128, 255); // UV=255 exceeds 240
        let violations = BroadcastSafeChecker::check_frame(&frame, 4, 4, 0, &cfg);
        assert!(violations
            .iter()
            .any(|v| v.violation_type == ViolationType::ChromaAbove));
    }

    #[test]
    fn test_chroma_below_violation() {
        let cfg = BroadcastSafeConfig::ntsc();
        let frame = make_frame(4, 4, 128, 0); // UV=0 < 16
        let violations = BroadcastSafeChecker::check_frame(&frame, 4, 4, 0, &cfg);
        assert!(violations
            .iter()
            .any(|v| v.violation_type == ViolationType::ChromaBelow));
    }

    #[test]
    fn test_frame_idx_stored_in_violation() {
        let cfg = BroadcastSafeConfig::ntsc();
        let frame = make_frame(4, 4, 255, 128);
        let violations = BroadcastSafeChecker::check_frame(&frame, 4, 4, 42, &cfg);
        assert!(!violations.is_empty());
        assert_eq!(violations[0].frame, 42);
    }

    #[test]
    fn test_broadcast_safe_report_no_violations() {
        let report = BroadcastSafeReport::from_frame_violations(&[(0, vec![]), (1, vec![])]);
        assert_eq!(report.total_frames, 2);
        assert_eq!(report.violating_frames, 0);
        assert_eq!(report.total_violations, 0);
        assert!(report.worst_frame.is_none());
        assert!(!report.has_violations());
    }

    #[test]
    fn test_broadcast_safe_report_with_violations() {
        let cfg = BroadcastSafeConfig::ntsc();
        let frame = make_frame(4, 4, 255, 128);
        let v = BroadcastSafeChecker::check_frame(&frame, 4, 4, 5, &cfg);
        let report = BroadcastSafeReport::from_frame_violations(&[(5, v)]);
        assert!(report.has_violations());
        assert_eq!(report.worst_frame, Some(5));
    }

    #[test]
    fn test_violation_rate() {
        let report = BroadcastSafeReport {
            total_frames: 10,
            violating_frames: 3,
            total_violations: 15,
            worst_frame: Some(2),
        };
        assert!((report.violation_rate() - 0.3).abs() < 1e-9);
    }

    #[test]
    fn test_violation_type_description() {
        assert!(!ViolationType::LumaAbove.description().is_empty());
        assert!(!ViolationType::ChromaBelow.description().is_empty());
    }
}
