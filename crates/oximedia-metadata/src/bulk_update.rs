//! Bulk metadata update: batch field updates, validation pipeline, and rollback support.

#![allow(dead_code)]

use std::collections::HashMap;

/// A single field update operation.
#[derive(Debug, Clone)]
pub struct FieldUpdate {
    /// Field key to update.
    pub key: String,
    /// New value to set. `None` means the field should be removed.
    pub value: Option<String>,
}

impl FieldUpdate {
    /// Create an update that sets a field value.
    #[must_use]
    pub fn set(key: &str, value: &str) -> Self {
        Self {
            key: key.to_string(),
            value: Some(value.to_string()),
        }
    }

    /// Create an update that removes a field.
    #[must_use]
    pub fn remove(key: &str) -> Self {
        Self {
            key: key.to_string(),
            value: None,
        }
    }

    /// Return whether this update removes a field.
    #[must_use]
    pub fn is_removal(&self) -> bool {
        self.value.is_none()
    }
}

/// Validation error returned by the validation pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// Field that failed validation.
    pub field: String,
    /// Human-readable error message.
    pub message: String,
}

impl ValidationError {
    /// Create a new validation error.
    #[must_use]
    pub fn new(field: &str, message: &str) -> Self {
        Self {
            field: field.to_string(),
            message: message.to_string(),
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "field '{}': {}", self.field, self.message)
    }
}

/// A validator function signature: takes field key + value, returns optional error message.
pub type ValidatorFn = Box<dyn Fn(&str, &str) -> Option<String> + Send + Sync>;

/// Validation pipeline that applies a series of validators to field updates.
pub struct ValidationPipeline {
    validators: Vec<ValidatorFn>,
}

impl ValidationPipeline {
    /// Create a new empty pipeline.
    #[must_use]
    pub fn new() -> Self {
        Self {
            validators: Vec::new(),
        }
    }

    /// Add a validator to the pipeline.
    pub fn add_validator<F>(&mut self, validator: F)
    where
        F: Fn(&str, &str) -> Option<String> + Send + Sync + 'static,
    {
        self.validators.push(Box::new(validator));
    }

    /// Run all validators against a field update.
    /// Returns all errors found (empty = valid).
    #[must_use]
    pub fn validate_update(&self, update: &FieldUpdate) -> Vec<ValidationError> {
        if let Some(ref value) = update.value {
            self.validators
                .iter()
                .filter_map(|v| v(&update.key, value))
                .map(|msg| ValidationError::new(&update.key, &msg))
                .collect()
        } else {
            // Removals are always valid in this pipeline
            Vec::new()
        }
    }

    /// Run validators against all updates in a batch.
    /// Returns a map from field key to its errors.
    #[must_use]
    pub fn validate_batch(&self, updates: &[FieldUpdate]) -> HashMap<String, Vec<ValidationError>> {
        let mut result: HashMap<String, Vec<ValidationError>> = HashMap::new();
        for update in updates {
            let errors = self.validate_update(update);
            if !errors.is_empty() {
                result.entry(update.key.clone()).or_default().extend(errors);
            }
        }
        result
    }

    /// Return the number of registered validators.
    #[must_use]
    pub fn validator_count(&self) -> usize {
        self.validators.len()
    }
}

impl Default for ValidationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of applying a bulk update.
#[derive(Debug, Clone)]
pub struct BulkUpdateResult {
    /// Number of fields successfully set.
    pub fields_set: usize,
    /// Number of fields removed.
    pub fields_removed: usize,
    /// Number of fields skipped due to validation errors.
    pub fields_skipped: usize,
    /// Collected validation errors (keyed by field name).
    pub errors: HashMap<String, Vec<String>>,
}

impl BulkUpdateResult {
    /// Return `true` if no validation errors occurred.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.errors.is_empty()
    }

    /// Return the total number of updates applied (sets + removals).
    #[must_use]
    pub fn total_applied(&self) -> usize {
        self.fields_set + self.fields_removed
    }
}

/// Snapshot of metadata state used for rollback.
#[derive(Debug, Clone)]
pub struct MetadataSnapshot {
    data: HashMap<String, String>,
}

impl MetadataSnapshot {
    /// Create a snapshot from an existing data map.
    #[must_use]
    pub fn from_map(data: &HashMap<String, String>) -> Self {
        Self { data: data.clone() }
    }

    /// Restore the snapshot into a mutable data map.
    pub fn restore(&self, target: &mut HashMap<String, String>) {
        *target = self.data.clone();
    }

    /// Return the number of fields in the snapshot.
    #[must_use]
    pub fn field_count(&self) -> usize {
        self.data.len()
    }
}

/// Engine for applying bulk metadata updates with optional validation and rollback.
pub struct BulkUpdateEngine {
    /// Current metadata fields.
    data: HashMap<String, String>,
    /// Validation pipeline (optional).
    pipeline: Option<ValidationPipeline>,
    /// History of snapshots for rollback (newest last).
    history: Vec<MetadataSnapshot>,
    /// Maximum number of snapshots to retain.
    max_history: usize,
}

impl BulkUpdateEngine {
    /// Create a new engine with empty metadata.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            pipeline: None,
            history: Vec::new(),
            max_history: 10,
        }
    }

    /// Create an engine pre-populated with existing data.
    #[must_use]
    pub fn with_data(data: HashMap<String, String>) -> Self {
        Self {
            data,
            pipeline: None,
            history: Vec::new(),
            max_history: 10,
        }
    }

    /// Attach a validation pipeline.
    pub fn set_pipeline(&mut self, pipeline: ValidationPipeline) {
        self.pipeline = Some(pipeline);
    }

    /// Set maximum number of undo snapshots to retain.
    pub fn set_max_history(&mut self, max: usize) {
        self.max_history = max;
    }

    /// Take a snapshot of current state.
    fn take_snapshot(&mut self) {
        let snapshot = MetadataSnapshot::from_map(&self.data);
        self.history.push(snapshot);
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
    }

    /// Apply a batch of updates. Snapshots state first for rollback.
    /// Returns a `BulkUpdateResult` describing the outcome.
    #[must_use]
    pub fn apply(&mut self, updates: &[FieldUpdate]) -> BulkUpdateResult {
        self.take_snapshot();

        let mut fields_set = 0usize;
        let mut fields_removed = 0usize;
        let mut fields_skipped = 0usize;
        let mut errors: HashMap<String, Vec<String>> = HashMap::new();

        for update in updates {
            // Run validation if pipeline is present and it's a set operation
            if let Some(ref pipeline) = self.pipeline {
                let validation_errors = pipeline.validate_update(update);
                if !validation_errors.is_empty() {
                    let msgs: Vec<String> = validation_errors
                        .iter()
                        .map(|e| e.message.clone())
                        .collect();
                    errors.insert(update.key.clone(), msgs);
                    fields_skipped += 1;
                    continue;
                }
            }

            match &update.value {
                Some(v) => {
                    self.data.insert(update.key.clone(), v.clone());
                    fields_set += 1;
                }
                None => {
                    self.data.remove(&update.key);
                    fields_removed += 1;
                }
            }
        }

        BulkUpdateResult {
            fields_set,
            fields_removed,
            fields_skipped,
            errors,
        }
    }

    /// Roll back to the previous snapshot. Returns `true` on success.
    pub fn rollback(&mut self) -> bool {
        if let Some(snapshot) = self.history.pop() {
            snapshot.restore(&mut self.data);
            true
        } else {
            false
        }
    }

    /// Get the current value of a field.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(String::as_str)
    }

    /// Return a read-only reference to all current data.
    #[must_use]
    pub fn data(&self) -> &HashMap<String, String> {
        &self.data
    }

    /// Number of undo levels available.
    #[must_use]
    pub fn history_depth(&self) -> usize {
        self.history.len()
    }
}

impl Default for BulkUpdateEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_update_set() {
        let u = FieldUpdate::set("title", "Hello");
        assert_eq!(u.key, "title");
        assert_eq!(u.value.as_deref(), Some("Hello"));
        assert!(!u.is_removal());
    }

    #[test]
    fn test_field_update_remove() {
        let u = FieldUpdate::remove("artist");
        assert!(u.is_removal());
    }

    #[test]
    fn test_validation_error_display() {
        let e = ValidationError::new("genre", "must not be empty");
        let s = e.to_string();
        assert!(s.contains("genre"));
        assert!(s.contains("must not be empty"));
    }

    #[test]
    fn test_pipeline_no_validators_always_valid() {
        let pipeline = ValidationPipeline::new();
        let update = FieldUpdate::set("title", "something");
        let errors = pipeline.validate_update(&update);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_pipeline_validator_catches_empty() {
        let mut pipeline = ValidationPipeline::new();
        pipeline.add_validator(|_key, value| {
            if value.is_empty() {
                Some("must not be empty".to_string())
            } else {
                None
            }
        });
        let update = FieldUpdate::set("title", "");
        let errors = pipeline.validate_update(&update);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].field, "title");
    }

    #[test]
    fn test_pipeline_removal_always_valid() {
        let mut pipeline = ValidationPipeline::new();
        pipeline.add_validator(|_, _| Some("always error".to_string()));
        let update = FieldUpdate::remove("field");
        let errors = pipeline.validate_update(&update);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_pipeline_validate_batch() {
        let mut pipeline = ValidationPipeline::new();
        pipeline.add_validator(|_, v| {
            if v.len() > 3 {
                Some("too long".to_string())
            } else {
                None
            }
        });
        let updates = vec![
            FieldUpdate::set("a", "ok"),
            FieldUpdate::set("b", "toolong"),
        ];
        let result = pipeline.validate_batch(&updates);
        assert!(!result.contains_key("a"));
        assert!(result.contains_key("b"));
    }

    #[test]
    fn test_engine_apply_sets_fields() {
        let mut engine = BulkUpdateEngine::new();
        let updates = vec![
            FieldUpdate::set("title", "My Song"),
            FieldUpdate::set("artist", "Artist"),
        ];
        let result = engine.apply(&updates);
        assert_eq!(result.fields_set, 2);
        assert_eq!(result.fields_removed, 0);
        assert_eq!(engine.get("title"), Some("My Song"));
    }

    #[test]
    fn test_engine_apply_removes_field() {
        let mut data = HashMap::new();
        data.insert("title".to_string(), "Old Title".to_string());
        let mut engine = BulkUpdateEngine::with_data(data);
        let updates = vec![FieldUpdate::remove("title")];
        let result = engine.apply(&updates);
        assert_eq!(result.fields_removed, 1);
        assert!(engine.get("title").is_none());
    }

    #[test]
    fn test_engine_rollback() {
        let mut engine = BulkUpdateEngine::new();
        let _ = engine.apply(&[FieldUpdate::set("title", "v1")]);
        let _ = engine.apply(&[FieldUpdate::set("title", "v2")]);
        assert_eq!(engine.get("title"), Some("v2"));
        let ok = engine.rollback();
        assert!(ok);
        assert_eq!(engine.get("title"), Some("v1"));
    }

    #[test]
    fn test_engine_rollback_empty_history() {
        let mut engine = BulkUpdateEngine::new();
        assert!(!engine.rollback());
    }

    #[test]
    fn test_engine_validation_skips_invalid() {
        let mut pipeline = ValidationPipeline::new();
        pipeline.add_validator(|_, v| {
            if v.is_empty() {
                Some("empty not allowed".to_string())
            } else {
                None
            }
        });
        let mut engine = BulkUpdateEngine::new();
        engine.set_pipeline(pipeline);
        let updates = vec![
            FieldUpdate::set("title", "Good"),
            FieldUpdate::set("bad_field", ""),
        ];
        let result = engine.apply(&updates);
        assert_eq!(result.fields_set, 1);
        assert_eq!(result.fields_skipped, 1);
        assert!(result.errors.contains_key("bad_field"));
    }

    #[test]
    fn test_engine_history_depth() {
        let mut engine = BulkUpdateEngine::new();
        let _ = engine.apply(&[FieldUpdate::set("a", "1")]);
        let _ = engine.apply(&[FieldUpdate::set("a", "2")]);
        assert_eq!(engine.history_depth(), 2);
    }

    #[test]
    fn test_snapshot_restore() {
        let mut data = HashMap::new();
        data.insert("x".to_string(), "original".to_string());
        let snapshot = MetadataSnapshot::from_map(&data);
        data.insert("x".to_string(), "modified".to_string());
        snapshot.restore(&mut data);
        assert_eq!(data.get("x").map(String::as_str), Some("original"));
    }

    #[test]
    fn test_result_total_applied() {
        let result = BulkUpdateResult {
            fields_set: 3,
            fields_removed: 2,
            fields_skipped: 1,
            errors: HashMap::new(),
        };
        assert_eq!(result.total_applied(), 5);
        assert!(result.is_clean());
    }
}
