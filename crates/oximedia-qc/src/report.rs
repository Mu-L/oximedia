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

/// Exports the QC report as a self-contained HTML document.
///
/// The generated page includes:
/// - A header with the file path, timestamp, and overall pass/fail badge.
/// - A summary table with check counts broken down by severity.
/// - A detailed findings table showing each failed check with severity,
///   rule name, message, and optional recommendation.
/// - A pass-all notice when no failures were detected.
///
/// All styling is embedded inline; no external resources are required.
///
/// # Errors
///
/// Currently infallible (`Ok` is always returned); the signature uses
/// `Result<String, HtmlExportError>` for forward compatibility.
pub fn report_to_html(report: &QcReport) -> Result<String, HtmlExportError> {
    let status_color = if report.overall_passed {
        "#2e7d32"
    } else {
        "#c62828"
    };
    let status_text = if report.overall_passed {
        "PASS"
    } else {
        "FAIL"
    };

    let critical_count = report.critical_errors().len();
    let error_count = report.errors().len();
    let warning_count = report.warnings().len();
    let info_count = report.info_messages().len();

    // ── Findings rows ────────────────────────────────────────────────────────
    let mut rows = String::new();
    for result in &report.results {
        if result.passed {
            continue;
        }
        let (sev_color, sev_bg) = match result.severity {
            Severity::Critical => ("#b71c1c", "#ffebee"),
            Severity::Error => ("#e65100", "#fff3e0"),
            Severity::Warning => ("#f57f17", "#fffde7"),
            Severity::Info => ("#1565c0", "#e3f2fd"),
        };

        let stream_cell = result
            .stream_index
            .map(|s| s.to_string())
            .unwrap_or_default();
        let ts_cell = result
            .timestamp
            .map(|t| format!("{t:.2}s"))
            .unwrap_or_default();
        let rec_cell = result.recommendation.as_deref().unwrap_or("").to_string();

        rows.push_str(&format!(
            "<tr style=\"background:{sev_bg}\">\
             <td style=\"color:{sev_color};font-weight:bold;padding:6px 8px\">{}</td>\
             <td style=\"padding:6px 8px;font-family:monospace\">{}</td>\
             <td style=\"padding:6px 8px\">{}</td>\
             <td style=\"padding:6px 8px;color:#555\">{stream_cell}</td>\
             <td style=\"padding:6px 8px;color:#555\">{ts_cell}</td>\
             <td style=\"padding:6px 8px;color:#37474f;font-style:italic\">{rec_cell}</td>\
             </tr>",
            html_escape(&result.severity.to_string()),
            html_escape(&result.rule_name),
            html_escape(&result.message),
        ));
    }

    let findings_section = if report.failed_checks == 0 {
        "<p style=\"color:#2e7d32;font-weight:bold\">✓ All checks passed.</p>".to_string()
    } else {
        format!(
            "<table style=\"width:100%;border-collapse:collapse;font-size:0.9em\">\
             <thead><tr style=\"background:#eceff1\">\
             <th style=\"padding:8px;text-align:left\">Severity</th>\
             <th style=\"padding:8px;text-align:left\">Rule</th>\
             <th style=\"padding:8px;text-align:left\">Message</th>\
             <th style=\"padding:8px;text-align:left\">Stream</th>\
             <th style=\"padding:8px;text-align:left\">Timestamp</th>\
             <th style=\"padding:8px;text-align:left\">Recommendation</th>\
             </tr></thead>\
             <tbody>{rows}</tbody>\
             </table>"
        )
    };

    let duration_row = report
        .validation_duration
        .map(|d| {
            format!(
                "<tr><td style=\"padding:4px 8px;color:#555\">Validation Duration</td>\
             <td style=\"padding:4px 8px\">{d:.3}s</td></tr>"
            )
        })
        .unwrap_or_default();

    let html = format!(
        "<!DOCTYPE html>\n\
         <html lang=\"en\">\n\
         <head>\n\
         <meta charset=\"UTF-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n\
         <title>QC Report — {file}</title>\n\
         <style>\
         body{{font-family:system-ui,sans-serif;margin:0;padding:24px;background:#fafafa;color:#212121}}\
         h1{{font-size:1.4em;margin:0 0 4px}}\
         .badge{{display:inline-block;padding:4px 14px;border-radius:4px;\
                 color:#fff;font-weight:bold;font-size:1em;background:{status_color}}}\
         table{{border-collapse:collapse}}td,th{{border-bottom:1px solid #e0e0e0}}\
         </style>\n\
         </head>\n\
         <body>\n\
         <h1>QC Report</h1>\n\
         <p><strong>File:</strong> {file}</p>\n\
         <p><strong>Generated:</strong> {ts}</p>\n\
         <p><strong>Status:</strong> <span class=\"badge\">{status_text}</span></p>\n\
         <h2>Summary</h2>\n\
         <table>\n\
         <tr><td style=\"padding:4px 8px;color:#555\">Total Checks</td>\
             <td style=\"padding:4px 8px\">{total}</td></tr>\n\
         <tr><td style=\"padding:4px 8px;color:#2e7d32\">Passed</td>\
             <td style=\"padding:4px 8px\">{passed}</td></tr>\n\
         <tr><td style=\"padding:4px 8px;color:#b71c1c\">Critical</td>\
             <td style=\"padding:4px 8px\">{critical}</td></tr>\n\
         <tr><td style=\"padding:4px 8px;color:#e65100\">Errors</td>\
             <td style=\"padding:4px 8px\">{errors}</td></tr>\n\
         <tr><td style=\"padding:4px 8px;color:#f57f17\">Warnings</td>\
             <td style=\"padding:4px 8px\">{warnings}</td></tr>\n\
         <tr><td style=\"padding:4px 8px;color:#1565c0\">Info</td>\
             <td style=\"padding:4px 8px\">{info}</td></tr>\n\
         {duration_row}\
         </table>\n\
         <h2>Findings</h2>\n\
         {findings_section}\n\
         </body>\n\
         </html>\n",
        file = html_escape(&report.file_path),
        ts = html_escape(&report.timestamp),
        total = report.total_checks,
        passed = report.passed_checks,
        critical = critical_count,
        errors = error_count,
        warnings = warning_count,
        info = info_count,
    );

    Ok(html)
}

/// Error type for HTML export operations.
#[derive(Debug)]
pub enum HtmlExportError {
    /// IO error during write.
    Io(std::io::Error),
}

impl std::fmt::Display for HtmlExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "HTML export IO error: {e}"),
        }
    }
}

impl std::error::Error for HtmlExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
        }
    }
}

/// Escapes special HTML characters to prevent injection.
fn html_escape(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '&' => "&amp;".chars().collect::<Vec<_>>(),
            '<' => "&lt;".chars().collect(),
            '>' => "&gt;".chars().collect(),
            '"' => "&quot;".chars().collect(),
            '\'' => "&#39;".chars().collect(),
            other => vec![other],
        })
        .collect()
}

impl QcReport {
    /// Exports the report as a self-contained HTML document.
    ///
    /// # Errors
    ///
    /// Returns an [`HtmlExportError`] if HTML generation fails (currently infallible).
    pub fn to_html(&self) -> Result<String, HtmlExportError> {
        report_to_html(self)
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
    /// Self-contained HTML document.
    Html,
}

impl ReportFormat {
    /// Returns the file extension for this format.
    #[must_use]
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::Text => "txt",
            Self::Json | Self::JsonCompact => "json",
            Self::Xml => "xml",
            Self::Html => "html",
        }
    }
}

#[cfg(test)]
mod report_html_tests {
    use super::*;
    use crate::rules::{CheckResult, Severity};

    fn make_report(pass: bool) -> QcReport {
        let mut r = QcReport::new("test_file.mkv");
        if pass {
            r.add_result(CheckResult::pass("dummy_rule"));
        } else {
            r.add_result(
                CheckResult::fail("codec_check", Severity::Error, "Bad codec")
                    .with_recommendation("Use AV1"),
            );
            r.add_result(CheckResult::fail(
                "loudness",
                Severity::Critical,
                "Too loud",
            ));
            r.add_result(CheckResult::fail(
                "frame_rate",
                Severity::Warning,
                "Non-standard fps",
            ));
        }
        r
    }

    #[test]
    fn test_html_export_pass_contains_pass_badge() {
        let report = make_report(true);
        let html = report.to_html().expect("html export should succeed");
        assert!(html.contains("PASS"));
        assert!(html.contains("All checks passed"));
    }

    #[test]
    fn test_html_export_fail_contains_fail_badge() {
        let report = make_report(false);
        let html = report.to_html().expect("html export should succeed");
        assert!(html.contains("FAIL"));
    }

    #[test]
    fn test_html_export_contains_file_path() {
        let report = make_report(false);
        let html = report.to_html().expect("html export should succeed");
        assert!(html.contains("test_file.mkv"));
    }

    #[test]
    fn test_html_export_contains_rule_names() {
        let report = make_report(false);
        let html = report.to_html().expect("html export should succeed");
        assert!(html.contains("codec_check"));
        assert!(html.contains("loudness"));
        assert!(html.contains("frame_rate"));
    }

    #[test]
    fn test_html_export_contains_recommendation() {
        let report = make_report(false);
        let html = report.to_html().expect("html export should succeed");
        assert!(html.contains("Use AV1"));
    }

    #[test]
    fn test_html_escape_prevents_injection() {
        let mut report = QcReport::new("<script>alert('xss')</script>");
        report.add_result(CheckResult::fail(
            "xss_rule",
            Severity::Warning,
            "msg with <b>html</b>",
        ));
        let html = report.to_html().expect("html export should succeed");
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn test_html_format_extension() {
        assert_eq!(ReportFormat::Html.extension(), "html");
    }

    #[test]
    fn test_html_report_with_stream_and_timestamp() {
        let mut report = QcReport::new("vid.mkv");
        report.add_result(
            CheckResult::fail("test", Severity::Warning, "warn")
                .with_stream(2)
                .with_timestamp(45.0),
        );
        let html = report.to_html().expect("html export should succeed");
        assert!(html.contains("45.00s"));
    }

    #[test]
    fn test_html_report_duration_displayed() {
        let mut report = QcReport::new("vid.mkv");
        report.set_validation_duration(3.141);
        let html = report.to_html().expect("html export should succeed");
        assert!(html.contains("3.141s"));
    }
}

// Use actual chrono crate
