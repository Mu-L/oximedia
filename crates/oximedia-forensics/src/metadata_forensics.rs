//! File metadata forensics.
//!
//! Analyses EXIF and file-level metadata for signs of editing, date anomalies,
//! GPS spoofing, and software-related inconsistencies.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

/// Types of metadata inconsistency that can be detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataInconsistency {
    /// Creation or modification date is missing, in the future, or otherwise
    /// implausible.
    DateAnomaly,
    /// The software field suggests post-processing (e.g. Photoshop).
    SoftwareMismatch,
    /// A gap in sequential file numbering has been detected.
    GapInSequence,
    /// GPS coordinates point to a suspicious or impossible location.
    SuspiciousGps,
    /// A generic EXIF field is missing or contains an unexpected value.
    ExifAnomaly,
}

impl MetadataInconsistency {
    /// Severity score for ranking issues.
    ///
    /// Higher values indicate more serious inconsistencies.
    #[must_use]
    pub fn severity(&self) -> u32 {
        match self {
            MetadataInconsistency::DateAnomaly => 3,
            MetadataInconsistency::SoftwareMismatch => 2,
            MetadataInconsistency::GapInSequence => 1,
            MetadataInconsistency::SuspiciousGps => 4,
            MetadataInconsistency::ExifAnomaly => 2,
        }
    }
}

/// Parsed EXIF metadata relevant to forensic analysis.
#[derive(Debug, Clone)]
pub struct ExifForensicsData {
    /// Creation date string from EXIF (if present).
    pub creation_date: Option<String>,
    /// Modification date string from the file system or EXIF (if present).
    pub modification_date: Option<String>,
    /// Camera manufacturer, e.g. `"Canon"`.
    pub camera_make: String,
    /// Software that last wrote this file, e.g. `"Adobe Photoshop"`.
    pub software: String,
    /// GPS latitude in decimal degrees (if present).
    pub gps_lat: Option<f32>,
    /// GPS longitude in decimal degrees (if present).
    pub gps_lon: Option<f32>,
}

impl ExifForensicsData {
    /// Return `true` when GPS coordinates are present.
    #[must_use]
    pub fn has_gps(&self) -> bool {
        self.gps_lat.is_some() && self.gps_lon.is_some()
    }

    /// Return `true` when the `software` field contains keywords associated
    /// with image editing tools.
    #[must_use]
    pub fn is_edited(&self) -> bool {
        let lower = self.software.to_lowercase();
        lower.contains("photoshop")
            || lower.contains("gimp")
            || lower.contains("lightroom")
            || lower.contains("capture one")
            || lower.contains("affinity")
    }
}

/// Forensic analysis report for a single file.
#[derive(Debug, Clone)]
pub struct ForensicsReport {
    /// Path of the analysed file.
    pub file_path: String,
    /// List of detected inconsistencies.
    pub inconsistencies: Vec<MetadataInconsistency>,
    /// Overall authenticity score in [0, 1].  Higher = more authentic.
    pub authenticity_score: f32,
}

impl ForensicsReport {
    /// Return `true` when `authenticity_score` is below `0.5`.
    #[must_use]
    pub fn is_suspicious(&self) -> bool {
        self.authenticity_score < 0.5
    }

    /// Return references to all inconsistencies with severity ≥ 3.
    #[must_use]
    pub fn critical_issues(&self) -> Vec<&MetadataInconsistency> {
        self.inconsistencies
            .iter()
            .filter(|i| i.severity() >= 3)
            .collect()
    }
}

/// Analyse metadata from `data` and produce a [`ForensicsReport`].
///
/// The authenticity score starts at `1.0` and is reduced by `0.1` for each
/// detected inconsistency, weighted by severity.
#[must_use]
pub fn analyze_metadata(data: &ExifForensicsData) -> ForensicsReport {
    let mut inconsistencies = Vec::new();

    // Check for editing software
    if data.is_edited() {
        inconsistencies.push(MetadataInconsistency::SoftwareMismatch);
    }

    // Check for missing creation date (suspicious for an authentic camera file)
    if data.creation_date.is_none() {
        inconsistencies.push(MetadataInconsistency::DateAnomaly);
    }

    // Check modification date exists when creation date does
    if data.creation_date.is_some() && data.modification_date.is_some() {
        // If both are present but identical software is "unknown", flag ExifAnomaly
        if data.software.is_empty() {
            inconsistencies.push(MetadataInconsistency::ExifAnomaly);
        }
    }

    // GPS: latitude out of valid range
    if let Some(lat) = data.gps_lat {
        if !(-90.0..=90.0).contains(&lat) {
            inconsistencies.push(MetadataInconsistency::SuspiciousGps);
        }
    }
    // GPS: longitude out of valid range
    if let Some(lon) = data.gps_lon {
        if !(-180.0..=180.0).contains(&lon) {
            inconsistencies.push(MetadataInconsistency::SuspiciousGps);
        }
    }

    // Derive authenticity score
    let total_severity: u32 = inconsistencies.iter().map(|i| i.severity()).sum();
    let score = (1.0_f32 - total_severity as f32 * 0.1).max(0.0);

    ForensicsReport {
        file_path: String::new(),
        inconsistencies,
        authenticity_score: score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean_data() -> ExifForensicsData {
        ExifForensicsData {
            creation_date: Some("2024-01-15 10:30:00".to_string()),
            modification_date: Some("2024-01-15 10:30:00".to_string()),
            camera_make: "Canon".to_string(),
            software: "Canon EOS Utility".to_string(),
            gps_lat: Some(35.6762),
            gps_lon: Some(139.6503),
        }
    }

    // ── MetadataInconsistency ──────────────────────────────────────────────────

    #[test]
    fn test_date_anomaly_severity() {
        assert_eq!(MetadataInconsistency::DateAnomaly.severity(), 3);
    }

    #[test]
    fn test_software_mismatch_severity() {
        assert_eq!(MetadataInconsistency::SoftwareMismatch.severity(), 2);
    }

    #[test]
    fn test_gap_in_sequence_severity() {
        assert_eq!(MetadataInconsistency::GapInSequence.severity(), 1);
    }

    #[test]
    fn test_suspicious_gps_severity() {
        assert_eq!(MetadataInconsistency::SuspiciousGps.severity(), 4);
    }

    #[test]
    fn test_exif_anomaly_severity() {
        assert_eq!(MetadataInconsistency::ExifAnomaly.severity(), 2);
    }

    // ── ExifForensicsData ──────────────────────────────────────────────────────

    #[test]
    fn test_has_gps_true() {
        let d = clean_data();
        assert!(d.has_gps());
    }

    #[test]
    fn test_has_gps_false_when_missing_lat() {
        let mut d = clean_data();
        d.gps_lat = None;
        assert!(!d.has_gps());
    }

    #[test]
    fn test_is_edited_photoshop() {
        let mut d = clean_data();
        d.software = "Adobe Photoshop CC 2024".to_string();
        assert!(d.is_edited());
    }

    #[test]
    fn test_is_edited_gimp() {
        let mut d = clean_data();
        d.software = "GIMP 2.10".to_string();
        assert!(d.is_edited());
    }

    #[test]
    fn test_is_not_edited_camera_software() {
        let d = clean_data();
        assert!(!d.is_edited());
    }

    // ── ForensicsReport ────────────────────────────────────────────────────────

    #[test]
    fn test_report_is_suspicious_low_score() {
        let r = ForensicsReport {
            file_path: "test.jpg".to_string(),
            inconsistencies: vec![MetadataInconsistency::SuspiciousGps],
            authenticity_score: 0.3,
        };
        assert!(r.is_suspicious());
    }

    #[test]
    fn test_report_is_not_suspicious_high_score() {
        let r = ForensicsReport {
            file_path: "test.jpg".to_string(),
            inconsistencies: Vec::new(),
            authenticity_score: 0.9,
        };
        assert!(!r.is_suspicious());
    }

    #[test]
    fn test_critical_issues_filters_by_severity() {
        let r = ForensicsReport {
            file_path: "test.jpg".to_string(),
            inconsistencies: vec![
                MetadataInconsistency::GapInSequence, // severity 1 — not critical
                MetadataInconsistency::SuspiciousGps, // severity 4 — critical
                MetadataInconsistency::DateAnomaly,   // severity 3 — critical
            ],
            authenticity_score: 0.2,
        };
        let critical = r.critical_issues();
        assert_eq!(critical.len(), 2);
    }

    // ── analyze_metadata ───────────────────────────────────────────────────────

    #[test]
    fn test_analyze_metadata_clean_image() {
        let d = clean_data();
        let report = analyze_metadata(&d);
        // Clean image should score higher than 0.5
        assert!(report.authenticity_score > 0.5);
    }

    #[test]
    fn test_analyze_metadata_photoshop_flags_mismatch() {
        let mut d = clean_data();
        d.software = "Adobe Photoshop".to_string();
        let report = analyze_metadata(&d);
        assert!(report
            .inconsistencies
            .contains(&MetadataInconsistency::SoftwareMismatch));
    }

    #[test]
    fn test_analyze_metadata_missing_creation_date_flags_anomaly() {
        let mut d = clean_data();
        d.creation_date = None;
        let report = analyze_metadata(&d);
        assert!(report
            .inconsistencies
            .contains(&MetadataInconsistency::DateAnomaly));
    }

    #[test]
    fn test_analyze_metadata_invalid_gps_lat() {
        let mut d = clean_data();
        d.gps_lat = Some(200.0); // impossible latitude
        let report = analyze_metadata(&d);
        assert!(report
            .inconsistencies
            .contains(&MetadataInconsistency::SuspiciousGps));
    }
}
