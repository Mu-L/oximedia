//! Video and Image Forensics and Tampering Detection
//!
//! This crate provides comprehensive forensic analysis capabilities for detecting
//! image and video tampering, including:
//! - JPEG compression artifact analysis
//! - Error Level Analysis (ELA)
//! - Noise pattern analysis and PRNU
//! - Metadata verification
//! - Copy-move detection
//! - Illumination inconsistency detection
//! - Comprehensive forensic reporting

#![deny(unsafe_code)]
#![allow(dead_code)]

pub mod authenticity;
pub mod blocking;
pub mod chain_of_custody;
pub mod clone_detection;
pub mod compression;
pub mod compression_history;
pub mod copy_detect;
pub mod edit_history;
pub mod ela;
pub mod ela_analysis;
pub mod file_integrity;
pub mod fingerprint;
pub mod format_forensics;
pub mod frame_forensics;
pub mod frequency_forensics;
pub mod geometric;
pub mod hash_registry;
pub mod lighting;
pub mod metadata;
pub mod metadata_forensics;
pub mod noise;
pub mod noise_analysis;
pub mod pattern;
pub mod provenance;
pub mod report;
pub mod shadow_analysis;
pub mod source_camera;
pub mod splicing;
pub mod steganalysis;
pub mod tampering;
pub mod time_forensics;
pub mod watermark_detect;

use ndarray::Array2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during forensic analysis
#[derive(Error, Debug)]
pub enum ForensicsError {
    #[error("Invalid image data: {0}")]
    InvalidImage(String),

    #[error("Analysis failed: {0}")]
    AnalysisFailed(String),

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Image processing error: {0}")]
    ImageError(#[from] image::ImageError),
}

/// Result type for forensic operations
pub type ForensicsResult<T> = Result<T, ForensicsError>;

/// Confidence level for tampering detection
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ConfidenceLevel {
    /// Very low confidence (0-20%)
    VeryLow,
    /// Low confidence (20-40%)
    Low,
    /// Medium confidence (40-60%)
    Medium,
    /// High confidence (60-80%)
    High,
    /// Very high confidence (80-100%)
    VeryHigh,
}

impl ConfidenceLevel {
    /// Convert from a confidence score (0.0 to 1.0)
    pub fn from_score(score: f64) -> Self {
        match score {
            s if s < 0.2 => ConfidenceLevel::VeryLow,
            s if s < 0.4 => ConfidenceLevel::Low,
            s if s < 0.6 => ConfidenceLevel::Medium,
            s if s < 0.8 => ConfidenceLevel::High,
            _ => ConfidenceLevel::VeryHigh,
        }
    }

    /// Convert to a numeric score (0.0 to 1.0)
    pub fn to_score(&self) -> f64 {
        match self {
            ConfidenceLevel::VeryLow => 0.1,
            ConfidenceLevel::Low => 0.3,
            ConfidenceLevel::Medium => 0.5,
            ConfidenceLevel::High => 0.7,
            ConfidenceLevel::VeryHigh => 0.9,
        }
    }
}

/// Result of a single forensic test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForensicTest {
    /// Name of the test
    pub name: String,
    /// Whether tampering was detected
    pub tampering_detected: bool,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Detailed findings
    pub findings: Vec<String>,
    /// Anomaly map (if applicable)
    #[serde(skip)]
    pub anomaly_map: Option<Array2<f64>>,
}

impl ForensicTest {
    /// Create a new forensic test result
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            tampering_detected: false,
            confidence: 0.0,
            findings: Vec::new(),
            anomaly_map: None,
        }
    }

    /// Add a finding to the test
    pub fn add_finding(&mut self, finding: String) {
        self.findings.push(finding);
    }

    /// Set the confidence level
    pub fn set_confidence(&mut self, confidence: f64) {
        self.confidence = confidence.clamp(0.0, 1.0);
    }

    /// Get the confidence level category
    pub fn confidence_level(&self) -> ConfidenceLevel {
        ConfidenceLevel::from_score(self.confidence)
    }
}

/// Comprehensive tampering report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamperingReport {
    /// Overall tampering detected flag
    pub tampering_detected: bool,
    /// Overall confidence score (0.0 to 1.0)
    pub overall_confidence: f64,
    /// Individual test results
    pub tests: HashMap<String, ForensicTest>,
    /// Summary of findings
    pub summary: String,
    /// Recommended actions
    pub recommendations: Vec<String>,
}

impl TamperingReport {
    /// Create a new tampering report
    pub fn new() -> Self {
        Self {
            tampering_detected: false,
            overall_confidence: 0.0,
            tests: HashMap::new(),
            summary: String::new(),
            recommendations: Vec::new(),
        }
    }

    /// Add a test result
    pub fn add_test(&mut self, test: ForensicTest) {
        self.tests.insert(test.name.clone(), test);
    }

    /// Calculate overall confidence from individual tests
    pub fn calculate_overall_confidence(&mut self) {
        if self.tests.is_empty() {
            self.overall_confidence = 0.0;
            return;
        }

        let total: f64 = self.tests.values().map(|t| t.confidence).sum();
        self.overall_confidence = total / self.tests.len() as f64;

        // Determine if tampering was detected based on threshold
        self.tampering_detected = self.overall_confidence > 0.5;
    }

    /// Generate summary
    pub fn generate_summary(&mut self) {
        let num_tests = self.tests.len();
        let num_positive = self.tests.values().filter(|t| t.tampering_detected).count();

        if self.tampering_detected {
            self.summary = format!(
                "Tampering detected with {:.1}% confidence. {} out of {} tests indicated manipulation.",
                self.overall_confidence * 100.0,
                num_positive,
                num_tests
            );
        } else {
            self.summary = format!(
                "No significant tampering detected. {} out of {} tests passed.",
                num_tests - num_positive,
                num_tests
            );
        }
    }
}

impl Default for TamperingReport {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for forensic analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForensicsConfig {
    /// Enable compression analysis
    pub enable_compression_analysis: bool,
    /// Enable ELA
    pub enable_ela: bool,
    /// Enable noise analysis
    pub enable_noise_analysis: bool,
    /// Enable metadata analysis
    pub enable_metadata_analysis: bool,
    /// Enable geometric analysis
    pub enable_geometric_analysis: bool,
    /// Enable lighting analysis
    pub enable_lighting_analysis: bool,
    /// Minimum confidence threshold for reporting
    pub min_confidence_threshold: f64,
}

impl Default for ForensicsConfig {
    fn default() -> Self {
        Self {
            enable_compression_analysis: true,
            enable_ela: true,
            enable_noise_analysis: true,
            enable_metadata_analysis: true,
            enable_geometric_analysis: true,
            enable_lighting_analysis: true,
            min_confidence_threshold: 0.5,
        }
    }
}

/// Main forensics analyzer
pub struct ForensicsAnalyzer {
    config: ForensicsConfig,
}

impl ForensicsAnalyzer {
    /// Create a new forensics analyzer with default configuration
    pub fn new() -> Self {
        Self {
            config: ForensicsConfig::default(),
        }
    }

    /// Create a new forensics analyzer with custom configuration
    pub fn with_config(config: ForensicsConfig) -> Self {
        Self { config }
    }

    /// Analyze an image for tampering
    pub fn analyze(&self, image_data: &[u8]) -> ForensicsResult<TamperingReport> {
        let mut report = TamperingReport::new();

        // Parse image
        let image = image::load_from_memory(image_data)?;
        let rgb_image = image.to_rgb8();

        // Run compression analysis
        if self.config.enable_compression_analysis {
            let test = compression::analyze_compression(&rgb_image)?;
            report.add_test(test);
        }

        // Run ELA
        if self.config.enable_ela {
            let test = ela::perform_ela(&rgb_image)?;
            report.add_test(test);
        }

        // Run noise analysis
        if self.config.enable_noise_analysis {
            let test = noise::analyze_noise(&rgb_image)?;
            report.add_test(test);
        }

        // Run metadata analysis
        if self.config.enable_metadata_analysis {
            let test = metadata::analyze_metadata(image_data)?;
            report.add_test(test);
        }

        // Run geometric analysis
        if self.config.enable_geometric_analysis {
            let test = geometric::detect_copy_move(&rgb_image)?;
            report.add_test(test);
        }

        // Run lighting analysis
        if self.config.enable_lighting_analysis {
            let test = lighting::analyze_lighting(&rgb_image)?;
            report.add_test(test);
        }

        // Calculate overall results
        report.calculate_overall_confidence();
        report.generate_summary();

        Ok(report)
    }

    /// Get the current configuration
    pub fn config(&self) -> &ForensicsConfig {
        &self.config
    }

    /// Update the configuration
    pub fn set_config(&mut self, config: ForensicsConfig) {
        self.config = config;
    }
}

impl Default for ForensicsAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_level_conversion() {
        assert_eq!(ConfidenceLevel::from_score(0.1), ConfidenceLevel::VeryLow);
        assert_eq!(ConfidenceLevel::from_score(0.3), ConfidenceLevel::Low);
        assert_eq!(ConfidenceLevel::from_score(0.5), ConfidenceLevel::Medium);
        assert_eq!(ConfidenceLevel::from_score(0.7), ConfidenceLevel::High);
        assert_eq!(ConfidenceLevel::from_score(0.9), ConfidenceLevel::VeryHigh);
    }

    #[test]
    fn test_forensic_test_creation() {
        let mut test = ForensicTest::new("Test");
        assert_eq!(test.name, "Test");
        assert!(!test.tampering_detected);
        assert_eq!(test.confidence, 0.0);

        test.add_finding("Finding 1".to_string());
        test.set_confidence(0.75);
        assert_eq!(test.findings.len(), 1);
        assert_eq!(test.confidence, 0.75);
    }

    #[test]
    fn test_tampering_report() {
        let mut report = TamperingReport::new();

        let mut test1 = ForensicTest::new("Test1");
        test1.set_confidence(0.8);
        test1.tampering_detected = true;

        let mut test2 = ForensicTest::new("Test2");
        test2.set_confidence(0.6);
        test2.tampering_detected = true;

        report.add_test(test1);
        report.add_test(test2);
        report.calculate_overall_confidence();

        assert_eq!(report.tests.len(), 2);
        assert_eq!(report.overall_confidence, 0.7);
        assert!(report.tampering_detected);
    }
}
