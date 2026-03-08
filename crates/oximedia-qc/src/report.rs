//! Quality control report generation.
//!
//! This module provides the [`QcReport`] type and utilities for generating
//! detailed validation reports in various formats (JSON, XML, plain text).

use crate::rules::{CheckResult, RuleCategory, Severity};
use std::collections::HashMap;
use std::fmt;

/// Quality control report.
///
/// Contains the results of all QC checks performed on a file,
/// along with summary statistics and metadata.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "json", derive(serde::Serialize, serde::Deserialize))]
pub struct QcReport {
    /// Path to the file that was validated.
    pub file_path: String,

    /// Total number of checks performed.
    pub total_checks: usize,

    /// Number of checks that passed.
    pub passed_checks: usize,

    /// Number of checks that failed.
    pub failed_checks: usize,

    /// Overall validation result.
    pub overall_passed: bool,

    /// All check results.
    pub results: Vec<CheckResult>,

    /// Timestamp when the report was generated.
    pub timestamp: String,

    /// Duration of the validation process in seconds.
    pub validation_duration: Option<f64>,
}

impl QcReport {
    /// Creates a new QC report.
    #[must_use]
    pub fn new(file_path: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            total_checks: 0,
            passed_checks: 0,
            failed_checks: 0,
            overall_passed: true,
            results: Vec::new(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            validation_duration: None,
        }
    }

    /// Adds a check result to the report.
    pub fn add_result(&mut self, result: CheckResult) {
        self.total_checks += 1;
        if result.passed {
            self.passed_checks += 1;
        } else {
            self.failed_checks += 1;
            // If any check fails with Error or Critical severity, mark overall as failed
            if result.severity >= Severity::Error {
                self.overall_passed = false;
            }
        }
        self.results.push(result);
    }

    /// Adds multiple check results to the report.
    pub fn add_results(&mut self, results: Vec<CheckResult>) {
        for result in results {
            self.add_result(result);
        }
    }

    /// Sets the validation duration.
    pub fn set_validation_duration(&mut self, duration: f64) {
        self.validation_duration = Some(duration);
    }

    /// Returns results filtered by severity.
    #[must_use]
    pub fn results_by_severity(&self, severity: Severity) -> Vec<&CheckResult> {
        self.results
            .iter()
            .filter(|r| !r.passed && r.severity == severity)
            .collect()
    }

    /// Returns results filtered by category.
    #[must_use]
    pub fn results_by_category(&self, _category: RuleCategory) -> Vec<&CheckResult> {
        // Note: We would need to store category in CheckResult to implement this fully
        self.results.iter().filter(|r| !r.passed).collect()
    }

    /// Returns critical errors only.
    #[must_use]
    pub fn critical_errors(&self) -> Vec<&CheckResult> {
        self.results_by_severity(Severity::Critical)
    }

    /// Returns errors only.
    #[must_use]
    pub fn errors(&self) -> Vec<&CheckResult> {
        self.results_by_severity(Severity::Error)
    }

    /// Returns warnings only.
    #[must_use]
    pub fn warnings(&self) -> Vec<&CheckResult> {
        self.results_by_severity(Severity::Warning)
    }

    /// Returns info messages only.
    #[must_use]
    pub fn info_messages(&self) -> Vec<&CheckResult> {
        self.results_by_severity(Severity::Info)
    }

    /// Generates a plain text summary of the report.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut summary = String::new();
        summary.push_str(&format!("QC Report for: {}\n", self.file_path));
        summary.push_str(&format!("Generated: {}\n", self.timestamp));
        summary.push_str(&format!(
            "Overall Status: {}\n",
            if self.overall_passed { "PASS" } else { "FAIL" }
        ));
        summary.push_str(&format!(
            "Checks: {} total, {} passed, {} failed\n",
            self.total_checks, self.passed_checks, self.failed_checks
        ));

        if let Some(duration) = self.validation_duration {
            summary.push_str(&format!("Validation Duration: {duration:.2}s\n"));
        }

        summary.push('\n');

        let critical = self.critical_errors();
        if !critical.is_empty() {
            summary.push_str(&format!("Critical Errors: {}\n", critical.len()));
        }

        let errors = self.errors();
        if !errors.is_empty() {
            summary.push_str(&format!("Errors: {}\n", errors.len()));
        }

        let warnings = self.warnings();
        if !warnings.is_empty() {
            summary.push_str(&format!("Warnings: {}\n", warnings.len()));
        }

        summary
    }

    /// Exports the report as JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    #[cfg(feature = "json")]
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Exports the report as compact JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    #[cfg(feature = "json")]
    pub fn to_json_compact(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Exports the report as XML.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    #[cfg(feature = "xml")]
    pub fn to_xml(&self) -> Result<String, quick_xml::Error> {
        let mut buffer = Vec::new();
        let mut writer = quick_xml::Writer::new(&mut buffer);

        // Write XML header
        writer.write_event(quick_xml::events::Event::Decl(
            quick_xml::events::BytesDecl::new("1.0", Some("UTF-8"), None),
        ))?;

        // Write root element
        writer.write_event(quick_xml::events::Event::Start(
            quick_xml::events::BytesStart::new("qc_report"),
        ))?;

        // Write basic info
        self.write_xml_element(&mut writer, "file_path", &self.file_path)?;
        self.write_xml_element(&mut writer, "timestamp", &self.timestamp)?;
        self.write_xml_element(
            &mut writer,
            "overall_passed",
            &self.overall_passed.to_string(),
        )?;
        self.write_xml_element(&mut writer, "total_checks", &self.total_checks.to_string())?;
        self.write_xml_element(
            &mut writer,
            "passed_checks",
            &self.passed_checks.to_string(),
        )?;
        self.write_xml_element(
            &mut writer,
            "failed_checks",
            &self.failed_checks.to_string(),
        )?;

        // Write results
        writer.write_event(quick_xml::events::Event::Start(
            quick_xml::events::BytesStart::new("results"),
        ))?;

        for result in &self.results {
            writer.write_event(quick_xml::events::Event::Start(
                quick_xml::events::BytesStart::new("check"),
            ))?;

            self.write_xml_element(&mut writer, "rule_name", &result.rule_name)?;
            self.write_xml_element(&mut writer, "passed", &result.passed.to_string())?;
            self.write_xml_element(&mut writer, "severity", &result.severity.to_string())?;
            self.write_xml_element(&mut writer, "message", &result.message)?;

            if let Some(rec) = &result.recommendation {
                self.write_xml_element(&mut writer, "recommendation", rec)?;
            }

            writer.write_event(quick_xml::events::Event::End(
                quick_xml::events::BytesEnd::new("check"),
            ))?;
        }

        writer.write_event(quick_xml::events::Event::End(
            quick_xml::events::BytesEnd::new("results"),
        ))?;

        // Close root element
        writer.write_event(quick_xml::events::Event::End(
            quick_xml::events::BytesEnd::new("qc_report"),
        ))?;

        // Convert buffer to string
        // This should not fail since we only write valid UTF-8 to the buffer
        Ok(String::from_utf8(buffer).expect("buffer should contain valid UTF-8"))
    }

    #[cfg(feature = "xml")]
    fn write_xml_element(
        &self,
        writer: &mut quick_xml::Writer<&mut Vec<u8>>,
        name: &str,
        value: &str,
    ) -> Result<(), quick_xml::Error> {
        writer.write_event(quick_xml::events::Event::Start(
            quick_xml::events::BytesStart::new(name),
        ))?;
        writer.write_event(quick_xml::events::Event::Text(
            quick_xml::events::BytesText::new(value),
        ))?;
        writer.write_event(quick_xml::events::Event::End(
            quick_xml::events::BytesEnd::new(name),
        ))?;
        Ok(())
    }

    /// Generates a detailed text report.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut text = self.summary();
        text.push_str("\nDetailed Results:\n");
        text.push_str("=================\n\n");

        // Group results by severity
        let mut by_severity: HashMap<Severity, Vec<&CheckResult>> = HashMap::new();
        for result in &self.results {
            if !result.passed {
                by_severity.entry(result.severity).or_default().push(result);
            }
        }

        // Display in order of severity
        for severity in &[
            Severity::Critical,
            Severity::Error,
            Severity::Warning,
            Severity::Info,
        ] {
            if let Some(results) = by_severity.get(severity) {
                text.push_str(&format!("{severity} ({}):\n", results.len()));
                for result in results {
                    text.push_str(&format!("  [{}] {}\n", result.rule_name, result.message));
                    if let Some(stream_index) = result.stream_index {
                        text.push_str(&format!("    Stream: {stream_index}\n"));
                    }
                    if let Some(timestamp) = result.timestamp {
                        text.push_str(&format!("    Timestamp: {timestamp:.2}s\n"));
                    }
                    if let Some(rec) = &result.recommendation {
                        text.push_str(&format!("    Recommendation: {rec}\n"));
                    }
                }
                text.push('\n');
            }
        }

        text
    }
}

impl fmt::Display for QcReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_text())
    }
}

/// Report format for export.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportFormat {
    /// Plain text format.
    Text,
    /// JSON format.
    Json,
    /// Compact JSON format (no pretty printing).
    JsonCompact,
    /// XML format.
    Xml,
}

impl ReportFormat {
    /// Returns the file extension for this format.
    #[must_use]
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::Text => "txt",
            Self::Json | Self::JsonCompact => "json",
            Self::Xml => "xml",
        }
    }
}

// Use actual chrono crate
