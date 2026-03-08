//! Report generation.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Profiling report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// Report title.
    pub title: String,

    /// Timestamp.
    pub timestamp: String,

    /// Total duration.
    pub duration: Duration,

    /// Sections.
    pub sections: Vec<ReportSection>,
}

/// Report section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSection {
    /// Section title.
    pub title: String,

    /// Section content.
    pub content: String,

    /// Subsections.
    pub subsections: Vec<ReportSection>,
}

impl ReportSection {
    /// Create a new report section.
    pub fn new(title: String, content: String) -> Self {
        Self {
            title,
            content,
            subsections: Vec::new(),
        }
    }

    /// Add a subsection.
    pub fn add_subsection(&mut self, section: ReportSection) {
        self.subsections.push(section);
    }
}

/// Report generator.
#[derive(Debug)]
pub struct ReportGenerator {
    title: String,
    start_time: Instant,
    sections: Vec<ReportSection>,
}

impl ReportGenerator {
    /// Create a new report generator.
    pub fn new(title: String) -> Self {
        Self {
            title,
            start_time: Instant::now(),
            sections: Vec::new(),
        }
    }

    /// Add a section.
    pub fn add_section(&mut self, section: ReportSection) {
        self.sections.push(section);
    }

    /// Add a simple section.
    pub fn add_simple_section(&mut self, title: String, content: String) {
        self.sections.push(ReportSection::new(title, content));
    }

    /// Generate the report.
    pub fn generate(&self) -> Report {
        Report {
            title: self.title.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            duration: self.start_time.elapsed(),
            sections: self.sections.clone(),
        }
    }

    /// Generate a text report.
    pub fn generate_text(&self) -> String {
        let report = self.generate();
        let mut text = String::new();

        text.push_str(&format!("=== {} ===\n\n", report.title));
        text.push_str(&format!("Generated: {}\n", report.timestamp));
        text.push_str(&format!("Duration: {:?}\n\n", report.duration));

        for section in &report.sections {
            self.render_section(&mut text, section, 0);
        }

        text
    }

    /// Render a section to text.
    fn render_section(&self, text: &mut String, section: &ReportSection, level: usize) {
        let indent = "  ".repeat(level);

        text.push_str(&format!("{}{}:\n", indent, section.title));
        if !section.content.is_empty() {
            for line in section.content.lines() {
                text.push_str(&format!("{}  {}\n", indent, line));
            }
        }

        for subsection in &section.subsections {
            self.render_section(text, subsection, level + 1);
        }

        text.push('\n');
    }
}

impl Default for ReportGenerator {
    fn default() -> Self {
        Self::new("OxiMedia Profiler Report".to_string())
    }
}

// Stub for chrono compatibility
mod chrono {
    pub struct Utc;
    impl Utc {
        pub fn now() -> Self {
            Self
        }
        pub fn to_rfc3339(&self) -> String {
            "2024-01-01T00:00:00Z".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_generator() {
        let mut generator = ReportGenerator::new("Test Report".to_string());
        generator.add_simple_section("Section 1".to_string(), "Content 1".to_string());

        let report = generator.generate();
        assert_eq!(report.title, "Test Report");
        assert_eq!(report.sections.len(), 1);
    }

    #[test]
    fn test_report_section() {
        let mut section = ReportSection::new("Main".to_string(), "Content".to_string());
        section.add_subsection(ReportSection::new(
            "Sub".to_string(),
            "Sub content".to_string(),
        ));

        assert_eq!(section.subsections.len(), 1);
    }

    #[test]
    fn test_text_generation() {
        let mut generator = ReportGenerator::new("Test".to_string());
        generator.add_simple_section("Test Section".to_string(), "Test content".to_string());

        let text = generator.generate_text();
        assert!(text.contains("Test"));
        assert!(text.contains("Test Section"));
    }
}
