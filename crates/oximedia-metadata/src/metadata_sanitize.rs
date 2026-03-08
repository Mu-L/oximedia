#![allow(dead_code)]
//! Metadata sanitization and privacy cleaning.
//!
//! Provides tools to strip, redact, or anonymize sensitive metadata fields
//! (GPS coordinates, personal identifiers, device serials) for privacy
//! compliance and safe distribution.

use std::collections::{HashMap, HashSet};

/// Policy that drives which fields to sanitize and how.
#[derive(Debug, Clone)]
pub struct SanitizePolicy {
    /// Fields to completely remove.
    strip_fields: HashSet<String>,
    /// Fields whose values should be replaced with a redacted placeholder.
    redact_fields: HashSet<String>,
    /// Custom replacement text for redacted fields.
    redact_placeholder: String,
    /// If true, strip all GPS / geolocation fields automatically.
    strip_gps: bool,
    /// If true, strip device / software identification fields automatically.
    strip_device_info: bool,
    /// Maximum allowed value length; longer values are truncated.
    max_value_length: Option<usize>,
}

impl Default for SanitizePolicy {
    fn default() -> Self {
        Self {
            strip_fields: HashSet::new(),
            redact_fields: HashSet::new(),
            redact_placeholder: "[REDACTED]".into(),
            strip_gps: false,
            strip_device_info: false,
            max_value_length: None,
        }
    }
}

impl SanitizePolicy {
    /// Create a new default policy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a field key to be completely stripped.
    pub fn add_strip_field(&mut self, key: impl Into<String>) {
        self.strip_fields.insert(key.into());
    }

    /// Add a field key to be redacted (value replaced).
    pub fn add_redact_field(&mut self, key: impl Into<String>) {
        self.redact_fields.insert(key.into());
    }

    /// Set the redaction placeholder text.
    pub fn set_redact_placeholder(&mut self, placeholder: impl Into<String>) {
        self.redact_placeholder = placeholder.into();
    }

    /// Enable automatic GPS field stripping.
    pub fn enable_strip_gps(&mut self) {
        self.strip_gps = true;
    }

    /// Enable automatic device info stripping.
    pub fn enable_strip_device_info(&mut self) {
        self.strip_device_info = true;
    }

    /// Set a maximum value length (truncates longer values).
    pub fn set_max_value_length(&mut self, max_len: usize) {
        self.max_value_length = Some(max_len);
    }

    /// Get the redaction placeholder.
    pub fn redact_placeholder(&self) -> &str {
        &self.redact_placeholder
    }

    /// Whether GPS stripping is enabled.
    pub fn strip_gps(&self) -> bool {
        self.strip_gps
    }

    /// Whether device info stripping is enabled.
    pub fn strip_device_info(&self) -> bool {
        self.strip_device_info
    }

    /// Get the max value length setting.
    pub fn max_value_length(&self) -> Option<usize> {
        self.max_value_length
    }

    /// Get the set of fields to strip.
    pub fn strip_fields(&self) -> &HashSet<String> {
        &self.strip_fields
    }

    /// Get the set of fields to redact.
    pub fn redact_fields(&self) -> &HashSet<String> {
        &self.redact_fields
    }
}

/// Well-known GPS-related field keys.
const GPS_FIELD_PATTERNS: &[&str] = &[
    "gps",
    "latitude",
    "longitude",
    "altitude",
    "geolocation",
    "location",
    "GPSLatitude",
    "GPSLongitude",
    "GPSAltitude",
    "GPSPosition",
];

/// Well-known device / software identification field keys.
const DEVICE_FIELD_PATTERNS: &[&str] = &[
    "serial",
    "device",
    "software",
    "firmware",
    "camera_model",
    "CameraSerialNumber",
    "LensSerialNumber",
    "Software",
    "Make",
    "Model",
];

/// Check if a field key matches any pattern in a list (case-insensitive substring).
fn matches_patterns(key: &str, patterns: &[&str]) -> bool {
    let lower = key.to_lowercase();
    patterns.iter().any(|p| lower.contains(&p.to_lowercase()))
}

/// Result of a sanitization run.
#[derive(Debug, Clone)]
pub struct SanitizeReport {
    /// Number of fields stripped.
    pub fields_stripped: usize,
    /// Number of fields redacted.
    pub fields_redacted: usize,
    /// Number of fields truncated.
    pub fields_truncated: usize,
    /// Keys that were stripped.
    pub stripped_keys: Vec<String>,
    /// Keys that were redacted.
    pub redacted_keys: Vec<String>,
    /// Keys that were truncated.
    pub truncated_keys: Vec<String>,
}

impl SanitizeReport {
    /// Create an empty report.
    fn new() -> Self {
        Self {
            fields_stripped: 0,
            fields_redacted: 0,
            fields_truncated: 0,
            stripped_keys: Vec::new(),
            redacted_keys: Vec::new(),
            truncated_keys: Vec::new(),
        }
    }

    /// Total number of fields affected.
    pub fn total_affected(&self) -> usize {
        self.fields_stripped + self.fields_redacted + self.fields_truncated
    }
}

/// Apply a sanitization policy to a metadata map in-place.
///
/// Returns a report describing what was changed.
pub fn sanitize(fields: &mut HashMap<String, String>, policy: &SanitizePolicy) -> SanitizeReport {
    let mut report = SanitizeReport::new();

    // Collect keys to strip
    let keys_to_strip: Vec<String> = fields
        .keys()
        .filter(|k| {
            if policy.strip_fields.contains(k.as_str()) {
                return true;
            }
            if policy.strip_gps && matches_patterns(k, GPS_FIELD_PATTERNS) {
                return true;
            }
            if policy.strip_device_info && matches_patterns(k, DEVICE_FIELD_PATTERNS) {
                return true;
            }
            false
        })
        .cloned()
        .collect();

    for key in &keys_to_strip {
        fields.remove(key);
        report.fields_stripped += 1;
        report.stripped_keys.push(key.clone());
    }

    // Redact fields
    for key in &policy.redact_fields {
        if fields.contains_key(key) {
            fields.insert(key.clone(), policy.redact_placeholder.clone());
            report.fields_redacted += 1;
            report.redacted_keys.push(key.clone());
        }
    }

    // Truncate long values
    if let Some(max_len) = policy.max_value_length {
        for (key, value) in fields.iter_mut() {
            if value.len() > max_len {
                value.truncate(max_len);
                report.fields_truncated += 1;
                report.truncated_keys.push(key.clone());
            }
        }
    }

    report
}

/// Create a privacy-focused policy that strips GPS, device info, and common PII.
pub fn privacy_policy() -> SanitizePolicy {
    let mut p = SanitizePolicy::new();
    p.enable_strip_gps();
    p.enable_strip_device_info();
    p.add_strip_field("email");
    p.add_strip_field("phone");
    p.add_strip_field("address");
    p
}

/// Create a distribution-safe policy (strips GPS + redacts author info).
pub fn distribution_policy() -> SanitizePolicy {
    let mut p = SanitizePolicy::new();
    p.enable_strip_gps();
    p.add_redact_field("author");
    p.add_redact_field("creator");
    p
}

/// Strip all metadata except for an explicit allow-list of keys.
pub fn strip_except(fields: &mut HashMap<String, String>, allow_keys: &HashSet<String>) -> usize {
    let before = fields.len();
    fields.retain(|k, _| allow_keys.contains(k));
    before - fields.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_fields() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("title".into(), "My Video".into());
        m.insert("GPSLatitude".into(), "40.7128".into());
        m.insert("GPSLongitude".into(), "-74.0060".into());
        m.insert("CameraSerialNumber".into(), "ABC123".into());
        m.insert("author".into(), "John Doe".into());
        m.insert("email".into(), "john@example.com".into());
        m
    }

    #[test]
    fn test_default_policy_no_changes() {
        let policy = SanitizePolicy::new();
        let mut fields = sample_fields();
        let orig_len = fields.len();
        let report = sanitize(&mut fields, &policy);
        assert_eq!(report.total_affected(), 0);
        assert_eq!(fields.len(), orig_len);
    }

    #[test]
    fn test_strip_specific_field() {
        let mut policy = SanitizePolicy::new();
        policy.add_strip_field("email");
        let mut fields = sample_fields();
        let report = sanitize(&mut fields, &policy);
        assert_eq!(report.fields_stripped, 1);
        assert!(!fields.contains_key("email"));
    }

    #[test]
    fn test_strip_gps() {
        let mut policy = SanitizePolicy::new();
        policy.enable_strip_gps();
        let mut fields = sample_fields();
        let report = sanitize(&mut fields, &policy);
        assert!(report.fields_stripped >= 2);
        assert!(!fields.contains_key("GPSLatitude"));
        assert!(!fields.contains_key("GPSLongitude"));
    }

    #[test]
    fn test_strip_device_info() {
        let mut policy = SanitizePolicy::new();
        policy.enable_strip_device_info();
        let mut fields = sample_fields();
        let report = sanitize(&mut fields, &policy);
        assert!(report.fields_stripped >= 1);
        assert!(!fields.contains_key("CameraSerialNumber"));
    }

    #[test]
    fn test_redact_field() {
        let mut policy = SanitizePolicy::new();
        policy.add_redact_field("author");
        let mut fields = sample_fields();
        let report = sanitize(&mut fields, &policy);
        assert_eq!(report.fields_redacted, 1);
        assert_eq!(
            fields.get("author").expect("should succeed in test"),
            "[REDACTED]"
        );
    }

    #[test]
    fn test_custom_redact_placeholder() {
        let mut policy = SanitizePolicy::new();
        policy.add_redact_field("author");
        policy.set_redact_placeholder("***");
        let mut fields = sample_fields();
        sanitize(&mut fields, &policy);
        assert_eq!(fields.get("author").expect("should succeed in test"), "***");
    }

    #[test]
    fn test_max_value_length_truncation() {
        let mut policy = SanitizePolicy::new();
        policy.set_max_value_length(5);
        let mut fields = HashMap::new();
        fields.insert("long".into(), "abcdefghij".into());
        fields.insert("short".into(), "ab".into());
        let report = sanitize(&mut fields, &policy);
        assert_eq!(report.fields_truncated, 1);
        assert_eq!(fields.get("long").expect("should succeed in test"), "abcde");
        assert_eq!(fields.get("short").expect("should succeed in test"), "ab");
    }

    #[test]
    fn test_privacy_policy() {
        let policy = privacy_policy();
        assert!(policy.strip_gps());
        assert!(policy.strip_device_info());
        assert!(policy.strip_fields().contains("email"));
    }

    #[test]
    fn test_distribution_policy() {
        let policy = distribution_policy();
        assert!(policy.strip_gps());
        assert!(policy.redact_fields().contains("author"));
    }

    #[test]
    fn test_strip_except() {
        let mut fields = sample_fields();
        let mut allow = HashSet::new();
        allow.insert("title".into());
        let removed = strip_except(&mut fields, &allow);
        assert!(removed >= 4);
        assert_eq!(fields.len(), 1);
        assert!(fields.contains_key("title"));
    }

    #[test]
    fn test_combined_strip_and_redact() {
        let mut policy = SanitizePolicy::new();
        policy.add_strip_field("email");
        policy.add_redact_field("author");
        let mut fields = sample_fields();
        let report = sanitize(&mut fields, &policy);
        assert_eq!(report.fields_stripped, 1);
        assert_eq!(report.fields_redacted, 1);
        assert!(!fields.contains_key("email"));
        assert_eq!(
            fields.get("author").expect("should succeed in test"),
            "[REDACTED]"
        );
    }

    #[test]
    fn test_report_total_affected() {
        let mut policy = SanitizePolicy::new();
        policy.enable_strip_gps();
        policy.add_redact_field("author");
        let mut fields = sample_fields();
        let report = sanitize(&mut fields, &policy);
        assert!(report.total_affected() >= 3);
    }

    #[test]
    fn test_matches_patterns_case_insensitive() {
        assert!(matches_patterns("myGpsField", GPS_FIELD_PATTERNS));
        assert!(matches_patterns("LATITUDE_INFO", GPS_FIELD_PATTERNS));
        assert!(!matches_patterns("title", GPS_FIELD_PATTERNS));
    }

    #[test]
    fn test_empty_fields_no_panic() {
        let policy = privacy_policy();
        let mut fields = HashMap::new();
        let report = sanitize(&mut fields, &policy);
        assert_eq!(report.total_affected(), 0);
    }
}
