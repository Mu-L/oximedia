//! Forensic Report Generation
//!
//! This module handles generation of detailed forensic reports including
//! confidence scoring, visual annotations, and export to various formats.

use crate::TamperingReport;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write as FmtWrite;

/// Report format
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ReportFormat {
    /// Plain text
    Text,
    /// HTML
    Html,
    /// JSON
    Json,
    /// Markdown
    Markdown,
}

/// Evidence chain of custody entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainOfCustodyEntry {
    /// Timestamp
    pub timestamp: String,
    /// Action performed
    pub action: String,
    /// Person responsible
    pub person: String,
    /// Location
    pub location: String,
    /// Notes
    pub notes: String,
}

/// Detailed forensic report with chain of custody
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedForensicReport {
    /// Basic tampering report
    pub report: TamperingReport,
    /// Report ID
    pub report_id: String,
    /// Creation timestamp
    pub created_at: String,
    /// Examiner name
    pub examiner: Option<String>,
    /// Case number
    pub case_number: Option<String>,
    /// Chain of custody
    pub chain_of_custody: Vec<ChainOfCustodyEntry>,
    /// Evidence description
    pub evidence_description: String,
    /// Analysis methodology
    pub methodology: Vec<String>,
    /// Conclusions
    pub conclusions: Vec<String>,
}

impl DetailedForensicReport {
    /// Create a new detailed report
    pub fn new(report: TamperingReport) -> Self {
        Self {
            report,
            report_id: generate_report_id(),
            created_at: current_timestamp(),
            examiner: None,
            case_number: None,
            chain_of_custody: Vec::new(),
            evidence_description: String::new(),
            methodology: Vec::new(),
            conclusions: Vec::new(),
        }
    }

    /// Add chain of custody entry
    pub fn add_custody_entry(&mut self, entry: ChainOfCustodyEntry) {
        self.chain_of_custody.push(entry);
    }

    /// Set examiner
    pub fn set_examiner(&mut self, examiner: String) {
        self.examiner = Some(examiner);
    }

    /// Set case number
    pub fn set_case_number(&mut self, case_number: String) {
        self.case_number = Some(case_number);
    }

    /// Add methodology
    pub fn add_methodology(&mut self, method: String) {
        self.methodology.push(method);
    }

    /// Add conclusion
    pub fn add_conclusion(&mut self, conclusion: String) {
        self.conclusions.push(conclusion);
    }
}

/// Generate a unique report ID
fn generate_report_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();

    format!("FR-{:016X}", timestamp)
}

/// Get current timestamp
#[allow(unused_variables)]
fn current_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Simple ISO 8601 format (simplified)
    "2024-01-01T00:00:00Z".to_string() // Placeholder
}

/// Export report to specified format
pub fn export_report(report: &DetailedForensicReport, format: ReportFormat) -> String {
    match format {
        ReportFormat::Text => export_text(report),
        ReportFormat::Html => export_html(report),
        ReportFormat::Json => export_json(report),
        ReportFormat::Markdown => export_markdown(report),
    }
}

/// Export as plain text
fn export_text(report: &DetailedForensicReport) -> String {
    let mut output = String::new();

    writeln!(&mut output, "=== FORENSIC ANALYSIS REPORT ===").unwrap();
    writeln!(&mut output).unwrap();
    writeln!(&mut output, "Report ID: {}", report.report_id).unwrap();
    writeln!(&mut output, "Created: {}", report.created_at).unwrap();

    if let Some(ref examiner) = report.examiner {
        writeln!(&mut output, "Examiner: {}", examiner).unwrap();
    }

    if let Some(ref case_num) = report.case_number {
        writeln!(&mut output, "Case Number: {}", case_num).unwrap();
    }

    writeln!(&mut output).unwrap();
    writeln!(&mut output, "--- SUMMARY ---").unwrap();
    writeln!(&mut output, "{}", report.report.summary).unwrap();
    writeln!(&mut output).unwrap();

    writeln!(
        &mut output,
        "Overall Tampering Detected: {}",
        report.report.tampering_detected
    )
    .unwrap();
    writeln!(
        &mut output,
        "Overall Confidence: {:.1}%",
        report.report.overall_confidence * 100.0
    )
    .unwrap();
    writeln!(&mut output).unwrap();

    writeln!(&mut output, "--- TEST RESULTS ---").unwrap();
    for (name, test) in &report.report.tests {
        writeln!(&mut output).unwrap();
        writeln!(&mut output, "Test: {}", name).unwrap();
        writeln!(&mut output, "  Tampering: {}", test.tampering_detected).unwrap();
        writeln!(
            &mut output,
            "  Confidence: {:.1}% ({:?})",
            test.confidence * 100.0,
            test.confidence_level()
        )
        .unwrap();

        if !test.findings.is_empty() {
            writeln!(&mut output, "  Findings:").unwrap();
            for finding in &test.findings {
                writeln!(&mut output, "    - {}", finding).unwrap();
            }
        }
    }

    if !report.methodology.is_empty() {
        writeln!(&mut output).unwrap();
        writeln!(&mut output, "--- METHODOLOGY ---").unwrap();
        for (i, method) in report.methodology.iter().enumerate() {
            writeln!(&mut output, "{}. {}", i + 1, method).unwrap();
        }
    }

    if !report.conclusions.is_empty() {
        writeln!(&mut output).unwrap();
        writeln!(&mut output, "--- CONCLUSIONS ---").unwrap();
        for (i, conclusion) in report.conclusions.iter().enumerate() {
            writeln!(&mut output, "{}. {}", i + 1, conclusion).unwrap();
        }
    }

    if !report.report.recommendations.is_empty() {
        writeln!(&mut output).unwrap();
        writeln!(&mut output, "--- RECOMMENDATIONS ---").unwrap();
        for (i, rec) in report.report.recommendations.iter().enumerate() {
            writeln!(&mut output, "{}. {}", i + 1, rec).unwrap();
        }
    }

    if !report.chain_of_custody.is_empty() {
        writeln!(&mut output).unwrap();
        writeln!(&mut output, "--- CHAIN OF CUSTODY ---").unwrap();
        for entry in &report.chain_of_custody {
            writeln!(&mut output, "Timestamp: {}", entry.timestamp).unwrap();
            writeln!(&mut output, "  Action: {}", entry.action).unwrap();
            writeln!(&mut output, "  Person: {}", entry.person).unwrap();
            writeln!(&mut output, "  Location: {}", entry.location).unwrap();
            if !entry.notes.is_empty() {
                writeln!(&mut output, "  Notes: {}", entry.notes).unwrap();
            }
            writeln!(&mut output).unwrap();
        }
    }

    output
}

/// Export as HTML
fn export_html(report: &DetailedForensicReport) -> String {
    let mut output = String::new();

    writeln!(&mut output, "<!DOCTYPE html>").unwrap();
    writeln!(&mut output, "<html>").unwrap();
    writeln!(&mut output, "<head>").unwrap();
    writeln!(
        &mut output,
        "<title>Forensic Analysis Report - {}</title>",
        report.report_id
    )
    .unwrap();
    writeln!(&mut output, "<style>").unwrap();
    writeln!(
        &mut output,
        "body {{ font-family: Arial, sans-serif; margin: 40px; }}"
    )
    .unwrap();
    writeln!(&mut output, "h1 {{ color: #333; }}").unwrap();
    writeln!(
        &mut output,
        "h2 {{ color: #666; border-bottom: 2px solid #ddd; padding-bottom: 5px; }}"
    )
    .unwrap();
    writeln!(
        &mut output,
        ".summary {{ background: #f5f5f5; padding: 15px; border-left: 4px solid #007bff; }}"
    )
    .unwrap();
    writeln!(
        &mut output,
        ".test {{ margin: 20px 0; padding: 15px; border: 1px solid #ddd; }}"
    )
    .unwrap();
    writeln!(
        &mut output,
        ".tampering-yes {{ background: #fff3cd; border-left: 4px solid #ffc107; }}"
    )
    .unwrap();
    writeln!(
        &mut output,
        ".tampering-no {{ background: #d4edda; border-left: 4px solid #28a745; }}"
    )
    .unwrap();
    writeln!(&mut output, ".confidence {{ font-weight: bold; }}").unwrap();
    writeln!(&mut output, "</style>").unwrap();
    writeln!(&mut output, "</head>").unwrap();
    writeln!(&mut output, "<body>").unwrap();

    writeln!(&mut output, "<h1>Forensic Analysis Report</h1>").unwrap();

    writeln!(&mut output, "<div class='metadata'>").unwrap();
    writeln!(
        &mut output,
        "<p><strong>Report ID:</strong> {}</p>",
        report.report_id
    )
    .unwrap();
    writeln!(
        &mut output,
        "<p><strong>Created:</strong> {}</p>",
        report.created_at
    )
    .unwrap();

    if let Some(ref examiner) = report.examiner {
        writeln!(
            &mut output,
            "<p><strong>Examiner:</strong> {}</p>",
            examiner
        )
        .unwrap();
    }

    if let Some(ref case_num) = report.case_number {
        writeln!(
            &mut output,
            "<p><strong>Case Number:</strong> {}</p>",
            case_num
        )
        .unwrap();
    }
    writeln!(&mut output, "</div>").unwrap();

    writeln!(&mut output, "<h2>Summary</h2>").unwrap();
    writeln!(&mut output, "<div class='summary'>").unwrap();
    writeln!(&mut output, "<p>{}</p>", report.report.summary).unwrap();
    writeln!(
        &mut output,
        "<p><strong>Tampering Detected:</strong> {}</p>",
        if report.report.tampering_detected {
            "YES"
        } else {
            "NO"
        }
    )
    .unwrap();
    writeln!(
        &mut output,
        "<p><strong>Overall Confidence:</strong> <span class='confidence'>{:.1}%</span></p>",
        report.report.overall_confidence * 100.0
    )
    .unwrap();
    writeln!(&mut output, "</div>").unwrap();

    writeln!(&mut output, "<h2>Test Results</h2>").unwrap();
    for (name, test) in &report.report.tests {
        let class = if test.tampering_detected {
            "test tampering-yes"
        } else {
            "test tampering-no"
        };
        writeln!(&mut output, "<div class='{}'>", class).unwrap();
        writeln!(&mut output, "<h3>{}</h3>", name).unwrap();
        writeln!(
            &mut output,
            "<p><strong>Tampering:</strong> {}</p>",
            if test.tampering_detected { "YES" } else { "NO" }
        )
        .unwrap();
        writeln!(
            &mut output,
            "<p><strong>Confidence:</strong> {:.1}% ({:?})</p>",
            test.confidence * 100.0,
            test.confidence_level()
        )
        .unwrap();

        if !test.findings.is_empty() {
            writeln!(&mut output, "<p><strong>Findings:</strong></p>").unwrap();
            writeln!(&mut output, "<ul>").unwrap();
            for finding in &test.findings {
                writeln!(&mut output, "<li>{}</li>", finding).unwrap();
            }
            writeln!(&mut output, "</ul>").unwrap();
        }
        writeln!(&mut output, "</div>").unwrap();
    }

    if !report.conclusions.is_empty() {
        writeln!(&mut output, "<h2>Conclusions</h2>").unwrap();
        writeln!(&mut output, "<ol>").unwrap();
        for conclusion in &report.conclusions {
            writeln!(&mut output, "<li>{}</li>", conclusion).unwrap();
        }
        writeln!(&mut output, "</ol>").unwrap();
    }

    writeln!(&mut output, "</body>").unwrap();
    writeln!(&mut output, "</html>").unwrap();

    output
}

/// Export as JSON
fn export_json(report: &DetailedForensicReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
}

/// Export as Markdown
fn export_markdown(report: &DetailedForensicReport) -> String {
    let mut output = String::new();

    writeln!(&mut output, "# Forensic Analysis Report").unwrap();
    writeln!(&mut output).unwrap();

    writeln!(&mut output, "## Metadata").unwrap();
    writeln!(&mut output).unwrap();
    writeln!(&mut output, "- **Report ID**: {}", report.report_id).unwrap();
    writeln!(&mut output, "- **Created**: {}", report.created_at).unwrap();

    if let Some(ref examiner) = report.examiner {
        writeln!(&mut output, "- **Examiner**: {}", examiner).unwrap();
    }

    if let Some(ref case_num) = report.case_number {
        writeln!(&mut output, "- **Case Number**: {}", case_num).unwrap();
    }

    writeln!(&mut output).unwrap();
    writeln!(&mut output, "## Summary").unwrap();
    writeln!(&mut output).unwrap();
    writeln!(&mut output, "{}", report.report.summary).unwrap();
    writeln!(&mut output).unwrap();
    writeln!(
        &mut output,
        "- **Tampering Detected**: {}",
        if report.report.tampering_detected {
            "**YES**"
        } else {
            "NO"
        }
    )
    .unwrap();
    writeln!(
        &mut output,
        "- **Overall Confidence**: {:.1}%",
        report.report.overall_confidence * 100.0
    )
    .unwrap();
    writeln!(&mut output).unwrap();

    writeln!(&mut output, "## Test Results").unwrap();
    writeln!(&mut output).unwrap();

    for (name, test) in &report.report.tests {
        writeln!(&mut output, "### {}", name).unwrap();
        writeln!(&mut output).unwrap();
        writeln!(
            &mut output,
            "- **Tampering**: {}",
            if test.tampering_detected {
                "**YES**"
            } else {
                "NO"
            }
        )
        .unwrap();
        writeln!(
            &mut output,
            "- **Confidence**: {:.1}% ({:?})",
            test.confidence * 100.0,
            test.confidence_level()
        )
        .unwrap();

        if !test.findings.is_empty() {
            writeln!(&mut output).unwrap();
            writeln!(&mut output, "**Findings**:").unwrap();
            writeln!(&mut output).unwrap();
            for finding in &test.findings {
                writeln!(&mut output, "- {}", finding).unwrap();
            }
        }
        writeln!(&mut output).unwrap();
    }

    if !report.conclusions.is_empty() {
        writeln!(&mut output, "## Conclusions").unwrap();
        writeln!(&mut output).unwrap();
        for (i, conclusion) in report.conclusions.iter().enumerate() {
            writeln!(&mut output, "{}. {}", i + 1, conclusion).unwrap();
        }
        writeln!(&mut output).unwrap();
    }

    output
}

/// Generate confidence score visualization
pub fn visualize_confidence_scores(report: &TamperingReport) -> String {
    let mut output = String::new();

    writeln!(&mut output, "Confidence Scores:").unwrap();
    writeln!(&mut output).unwrap();

    for (name, test) in &report.tests {
        let bar_length = (test.confidence * 50.0) as usize;
        let bar = "█".repeat(bar_length);
        let empty = "░".repeat(50 - bar_length);

        writeln!(
            &mut output,
            "{:30} [{}{}] {:.1}%",
            name,
            bar,
            empty,
            test.confidence * 100.0
        )
        .unwrap();
    }

    writeln!(&mut output).unwrap();
    writeln!(
        &mut output,
        "{:30} [{}] {:.1}%",
        "OVERALL",
        "█".repeat((report.overall_confidence * 50.0) as usize),
        report.overall_confidence * 100.0
    )
    .unwrap();

    output
}

/// Generate executive summary
pub fn generate_executive_summary(report: &TamperingReport) -> String {
    let mut summary = String::new();

    if report.tampering_detected {
        writeln!(&mut summary, "ALERT: Potential image tampering detected.").unwrap();
        writeln!(&mut summary).unwrap();
        writeln!(
            &mut summary,
            "Analysis indicates evidence of manipulation with {:.1}% confidence.",
            report.overall_confidence * 100.0
        )
        .unwrap();
        writeln!(&mut summary).unwrap();

        let positive_tests: Vec<_> = report
            .tests
            .iter()
            .filter(|(_, test)| test.tampering_detected)
            .collect();

        writeln!(
            &mut summary,
            "Tests indicating tampering ({}/{}):",
            positive_tests.len(),
            report.tests.len()
        )
        .unwrap();

        for (name, test) in positive_tests {
            writeln!(
                &mut summary,
                "  - {}: {:.1}% confidence",
                name,
                test.confidence * 100.0
            )
            .unwrap();
        }
    } else {
        writeln!(&mut summary, "No significant tampering detected.").unwrap();
        writeln!(&mut summary).unwrap();
        writeln!(
            &mut summary,
            "Analysis found no strong evidence of manipulation."
        )
        .unwrap();
        writeln!(
            &mut summary,
            "Overall confidence in authenticity: {:.1}%",
            (1.0 - report.overall_confidence) * 100.0
        )
        .unwrap();
    }

    summary
}

/// Calculate test statistics
pub fn calculate_statistics(report: &TamperingReport) -> HashMap<String, f64> {
    let mut stats = HashMap::new();

    let total_tests = report.tests.len() as f64;
    let positive_tests = report
        .tests
        .iter()
        .filter(|(_, test)| test.tampering_detected)
        .count() as f64;

    stats.insert("total_tests".to_string(), total_tests);
    stats.insert("positive_tests".to_string(), positive_tests);
    stats.insert("negative_tests".to_string(), total_tests - positive_tests);
    stats.insert("positive_rate".to_string(), positive_tests / total_tests);

    let mean_confidence: f64 = report
        .tests
        .values()
        .map(|test| test.confidence)
        .sum::<f64>()
        / total_tests;

    stats.insert("mean_confidence".to_string(), mean_confidence);
    stats.insert("overall_confidence".to_string(), report.overall_confidence);

    stats
}

/// Generate recommendations based on findings
pub fn generate_recommendations(report: &TamperingReport) -> Vec<String> {
    let mut recommendations = Vec::new();

    if report.tampering_detected {
        recommendations.push("Further investigation recommended".to_string());
        recommendations.push("Consider manual expert review".to_string());

        if report.overall_confidence > 0.8 {
            recommendations.push("High confidence - strong evidence of tampering".to_string());
        } else if report.overall_confidence > 0.5 {
            recommendations.push("Moderate confidence - additional analysis suggested".to_string());
        } else {
            recommendations.push("Low confidence - results inconclusive".to_string());
        }

        // Specific recommendations based on tests
        for (name, test) in &report.tests {
            if test.tampering_detected && test.confidence > 0.7 {
                recommendations.push(format!("Pay particular attention to {} findings", name));
            }
        }
    } else {
        recommendations.push("Image appears authentic".to_string());
        recommendations.push("No immediate concerns identified".to_string());
    }

    recommendations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TamperingReport;

    #[test]
    fn test_report_id_generation() {
        let id1 = generate_report_id();
        assert!(id1.starts_with("FR-"));
        assert_eq!(id1.len(), 19); // FR- (3) + 16 hex digits = 19
    }

    #[test]
    fn test_export_text() {
        let report = TamperingReport::new();
        let detailed = DetailedForensicReport::new(report);
        let text = export_text(&detailed);
        assert!(text.contains("FORENSIC ANALYSIS REPORT"));
    }

    #[test]
    fn test_export_json() {
        let report = TamperingReport::new();
        let detailed = DetailedForensicReport::new(report);
        let json = export_json(&detailed);
        assert!(json.contains("report_id"));
    }

    #[test]
    fn test_export_markdown() {
        let report = TamperingReport::new();
        let detailed = DetailedForensicReport::new(report);
        let md = export_markdown(&detailed);
        assert!(md.contains("# Forensic Analysis Report"));
    }

    #[test]
    fn test_statistics_calculation() {
        let report = TamperingReport::new();
        let stats = calculate_statistics(&report);
        assert!(stats.contains_key("total_tests"));
    }

    #[test]
    fn test_recommendations_generation() {
        let report = TamperingReport::new();
        let recommendations = generate_recommendations(&report);
        assert!(!recommendations.is_empty());
    }
}
