//! QC template system: reusable check configurations for quality control workflows.
//!
//! Allows building named templates of QC checks that can be saved to a library
//! and instantiated per-job.

#![allow(dead_code)]

use std::collections::HashMap;

/// Categories of QC checks with associated severity levels.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum QcCheckType {
    /// Video signal level checks (luma, chroma, clipping).
    VideoLevels,
    /// Audio loudness and level checks (EBU R128, ATSC A/85).
    AudioLoudness,
    /// Codec/container structural validation.
    CodecContainer,
    /// Closed-caption/subtitle presence and validity.
    CaptionCompliance,
    /// Timecode continuity and accuracy.
    TimecodeIntegrity,
    /// Colour space and transfer function compliance.
    ColourSpace,
    /// Black frames and freeze detection.
    BlackFreeze,
    /// Custom check defined by external rule name.
    Custom(String),
}

impl QcCheckType {
    /// Return the default severity string for this check type.
    ///
    /// Severity: `"error"`, `"warning"`, or `"info"`.
    pub fn severity(&self) -> &'static str {
        match self {
            QcCheckType::VideoLevels => "error",
            QcCheckType::AudioLoudness => "error",
            QcCheckType::CodecContainer => "error",
            QcCheckType::CaptionCompliance => "warning",
            QcCheckType::TimecodeIntegrity => "warning",
            QcCheckType::ColourSpace => "warning",
            QcCheckType::BlackFreeze => "info",
            QcCheckType::Custom(_) => "info",
        }
    }

    /// Returns `true` if the severity is `"error"`.
    pub fn is_error_level(&self) -> bool {
        self.severity() == "error"
    }
}

/// A single check entry within a QC template.
#[derive(Debug, Clone)]
pub struct QcTemplateEntry {
    /// The type of check.
    pub check_type: QcCheckType,
    /// Human-readable description of what is being checked.
    pub description: String,
    /// Whether this check must pass for delivery to be allowed.
    pub required: bool,
    /// Optional override severity (if `None`, uses `QcCheckType::severity()`).
    pub severity_override: Option<String>,
}

impl QcTemplateEntry {
    /// Create a new template entry.
    pub fn new(check_type: QcCheckType, description: impl Into<String>, required: bool) -> Self {
        Self {
            check_type,
            description: description.into(),
            required,
            severity_override: None,
        }
    }

    /// Returns `true` if this check is required for delivery.
    pub fn is_required(&self) -> bool {
        self.required
    }

    /// Effective severity (override takes precedence).
    pub fn effective_severity(&self) -> &str {
        self.severity_override
            .as_deref()
            .unwrap_or_else(|| self.check_type.severity())
    }
}

/// A named collection of QC checks forming a reusable template.
#[derive(Debug, Clone)]
pub struct QcTemplate {
    /// Template name, e.g. `"broadcast_hd"`.
    pub name: String,
    /// Optional description of this template's intended use.
    pub description: String,
    /// Ordered list of check entries.
    entries: Vec<QcTemplateEntry>,
}

impl QcTemplate {
    /// Create an empty template.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            entries: Vec::new(),
        }
    }

    /// Add a check entry to the template.
    pub fn add_check(&mut self, entry: QcTemplateEntry) {
        self.entries.push(entry);
    }

    /// Return only the required check entries.
    pub fn required_checks(&self) -> Vec<&QcTemplateEntry> {
        self.entries.iter().filter(|e| e.is_required()).collect()
    }

    /// Return all check entries.
    pub fn all_checks(&self) -> &[QcTemplateEntry] {
        &self.entries
    }

    /// Total number of checks in the template.
    pub fn check_count(&self) -> usize {
        self.entries.len()
    }
}

/// Library of named QC templates.
#[derive(Debug, Default)]
pub struct QcTemplateLibrary {
    templates: HashMap<String, QcTemplate>,
}

impl QcTemplateLibrary {
    /// Create an empty library.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a template, replacing any existing template with the same name.
    pub fn register(&mut self, template: QcTemplate) {
        self.templates.insert(template.name.clone(), template);
    }

    /// Retrieve a template by name.
    pub fn get(&self, name: &str) -> Option<&QcTemplate> {
        self.templates.get(name)
    }

    /// Number of templates in the library.
    pub fn count(&self) -> usize {
        self.templates.len()
    }

    /// Return the names of all registered templates.
    pub fn names(&self) -> Vec<&str> {
        self.templates
            .keys()
            .map(std::string::String::as_str)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(check_type: QcCheckType, required: bool) -> QcTemplateEntry {
        QcTemplateEntry::new(check_type, "desc", required)
    }

    #[test]
    fn test_check_type_severity_video_levels() {
        assert_eq!(QcCheckType::VideoLevels.severity(), "error");
    }

    #[test]
    fn test_check_type_severity_caption() {
        assert_eq!(QcCheckType::CaptionCompliance.severity(), "warning");
    }

    #[test]
    fn test_check_type_severity_black_freeze() {
        assert_eq!(QcCheckType::BlackFreeze.severity(), "info");
    }

    #[test]
    fn test_check_type_is_error_level_true() {
        assert!(QcCheckType::CodecContainer.is_error_level());
    }

    #[test]
    fn test_check_type_is_error_level_false() {
        assert!(!QcCheckType::ColourSpace.is_error_level());
    }

    #[test]
    fn test_check_type_custom_severity() {
        let ct = QcCheckType::Custom("my_check".to_string());
        assert_eq!(ct.severity(), "info");
    }

    #[test]
    fn test_template_entry_is_required() {
        let e = make_entry(QcCheckType::VideoLevels, true);
        assert!(e.is_required());
    }

    #[test]
    fn test_template_entry_not_required() {
        let e = make_entry(QcCheckType::BlackFreeze, false);
        assert!(!e.is_required());
    }

    #[test]
    fn test_template_entry_effective_severity_default() {
        let e = make_entry(QcCheckType::AudioLoudness, true);
        assert_eq!(e.effective_severity(), "error");
    }

    #[test]
    fn test_template_entry_effective_severity_override() {
        let mut e = make_entry(QcCheckType::VideoLevels, true);
        e.severity_override = Some("warning".to_string());
        assert_eq!(e.effective_severity(), "warning");
    }

    #[test]
    fn test_qc_template_add_check_and_count() {
        let mut t = QcTemplate::new("broadcast", "Standard broadcast checks");
        t.add_check(make_entry(QcCheckType::VideoLevels, true));
        t.add_check(make_entry(QcCheckType::AudioLoudness, true));
        assert_eq!(t.check_count(), 2);
    }

    #[test]
    fn test_qc_template_required_checks() {
        let mut t = QcTemplate::new("test", "");
        t.add_check(make_entry(QcCheckType::VideoLevels, true));
        t.add_check(make_entry(QcCheckType::BlackFreeze, false));
        let req = t.required_checks();
        assert_eq!(req.len(), 1);
        assert_eq!(req[0].check_type, QcCheckType::VideoLevels);
    }

    #[test]
    fn test_template_library_register_and_get() {
        let mut lib = QcTemplateLibrary::new();
        lib.register(QcTemplate::new("t1", "first"));
        assert!(lib.get("t1").is_some());
    }

    #[test]
    fn test_template_library_get_missing() {
        let lib = QcTemplateLibrary::new();
        assert!(lib.get("nope").is_none());
    }

    #[test]
    fn test_template_library_count() {
        let mut lib = QcTemplateLibrary::new();
        lib.register(QcTemplate::new("a", ""));
        lib.register(QcTemplate::new("b", ""));
        assert_eq!(lib.count(), 2);
    }

    #[test]
    fn test_template_library_names() {
        let mut lib = QcTemplateLibrary::new();
        lib.register(QcTemplate::new("alpha", ""));
        let names = lib.names();
        assert!(names.contains(&"alpha"));
    }
}
