//! Clip-level metadata attached to timeline clips.

/// A single metadata field attached to a clip.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct ClipMetadataField {
    /// Metadata key.
    pub key: String,
    /// Metadata value.
    pub value: String,
    /// Whether this field may be changed by the user.
    pub editable: bool,
}

impl ClipMetadataField {
    /// Create a new metadata field.
    #[must_use]
    pub fn new(key: impl Into<String>, value: impl Into<String>, editable: bool) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            editable,
        }
    }

    /// Returns `true` when this field cannot be modified.
    #[must_use]
    pub fn is_readonly(&self) -> bool {
        !self.editable
    }
}

/// All metadata fields associated with one timeline clip.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TimelineClipMetadata {
    /// Identifier of the clip this metadata belongs to.
    pub clip_id: u64,
    /// Ordered list of metadata fields.
    pub fields: Vec<ClipMetadataField>,
}

impl TimelineClipMetadata {
    /// Create empty metadata for the specified clip.
    #[must_use]
    pub fn new(clip_id: u64) -> Self {
        Self {
            clip_id,
            fields: Vec::new(),
        }
    }

    /// Retrieve the value of a field by key.
    ///
    /// Returns `None` when the key is not present.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|f| f.key == key)
            .map(|f| f.value.as_str())
    }

    /// Update the value of an existing field.
    ///
    /// Returns `true` on success, `false` when the key is not found or the
    /// field is read-only.
    pub fn set(&mut self, key: &str, value: &str) -> bool {
        if let Some(field) = self.fields.iter_mut().find(|f| f.key == key) {
            if field.editable {
                field.value = value.to_string();
                return true;
            }
        }
        false
    }

    /// Append a new field.
    pub fn add_field(&mut self, key: impl Into<String>, value: impl Into<String>, editable: bool) {
        self.fields
            .push(ClipMetadataField::new(key, value, editable));
    }

    /// Number of fields currently stored.
    #[must_use]
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Iterate over all editable fields.
    pub fn editable_fields(&self) -> impl Iterator<Item = &ClipMetadataField> {
        self.fields.iter().filter(|f| f.editable)
    }
}

/// A reusable template that produces pre-populated [`TimelineClipMetadata`].
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ClipMetadataTemplate {
    /// Human-readable name for this template.
    pub name: String,
    /// Field definitions: (key, `default_value`, editable).
    pub fields: Vec<(String, String, bool)>,
}

impl ClipMetadataTemplate {
    /// Create a new template with the given name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            fields: Vec::new(),
        }
    }

    /// Add a field definition to this template.
    pub fn add(
        &mut self,
        key: impl Into<String>,
        default_value: impl Into<String>,
        editable: bool,
    ) {
        self.fields
            .push((key.into(), default_value.into(), editable));
    }

    /// Instantiate metadata for `clip_id` using the template defaults.
    #[must_use]
    pub fn apply(&self, clip_id: u64) -> TimelineClipMetadata {
        let mut meta = TimelineClipMetadata::new(clip_id);
        for (key, value, editable) in &self.fields {
            meta.add_field(key.clone(), value.clone(), *editable);
        }
        meta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_readonly() {
        let f = ClipMetadataField::new("source", "camera_a", false);
        assert!(f.is_readonly());
    }

    #[test]
    fn test_field_editable() {
        let f = ClipMetadataField::new("comment", "review later", true);
        assert!(!f.is_readonly());
    }

    #[test]
    fn test_metadata_new_empty() {
        let m = TimelineClipMetadata::new(42);
        assert_eq!(m.clip_id, 42);
        assert_eq!(m.field_count(), 0);
    }

    #[test]
    fn test_add_and_get_field() {
        let mut m = TimelineClipMetadata::new(1);
        m.add_field("scene", "int_office_day", true);
        assert_eq!(m.get("scene"), Some("int_office_day"));
    }

    #[test]
    fn test_get_missing_key() {
        let m = TimelineClipMetadata::new(1);
        assert_eq!(m.get("nothing"), None);
    }

    #[test]
    fn test_set_editable_field() {
        let mut m = TimelineClipMetadata::new(5);
        m.add_field("note", "original", true);
        let ok = m.set("note", "updated");
        assert!(ok);
        assert_eq!(m.get("note"), Some("updated"));
    }

    #[test]
    fn test_set_readonly_field_fails() {
        let mut m = TimelineClipMetadata::new(5);
        m.add_field("tc_in", "01:00:00:00", false);
        let ok = m.set("tc_in", "01:00:01:00");
        assert!(!ok);
        assert_eq!(m.get("tc_in"), Some("01:00:00:00"));
    }

    #[test]
    fn test_set_missing_key_fails() {
        let mut m = TimelineClipMetadata::new(5);
        let ok = m.set("does_not_exist", "value");
        assert!(!ok);
    }

    #[test]
    fn test_field_count() {
        let mut m = TimelineClipMetadata::new(7);
        m.add_field("a", "1", true);
        m.add_field("b", "2", false);
        m.add_field("c", "3", true);
        assert_eq!(m.field_count(), 3);
    }

    #[test]
    fn test_editable_fields_iterator() {
        let mut m = TimelineClipMetadata::new(9);
        m.add_field("ro", "x", false);
        m.add_field("rw1", "y", true);
        m.add_field("rw2", "z", true);
        let editable: Vec<_> = m.editable_fields().collect();
        assert_eq!(editable.len(), 2);
    }

    #[test]
    fn test_template_apply() {
        let mut tmpl = ClipMetadataTemplate::new("broadcast");
        tmpl.add("channel", "BBC1", false);
        tmpl.add("rating", "PG", true);
        let meta = tmpl.apply(99);
        assert_eq!(meta.clip_id, 99);
        assert_eq!(meta.field_count(), 2);
        assert_eq!(meta.get("channel"), Some("BBC1"));
        assert_eq!(meta.get("rating"), Some("PG"));
    }

    #[test]
    fn test_template_readonly_field_preserved() {
        let mut tmpl = ClipMetadataTemplate::new("archive");
        tmpl.add("ingest_date", "2026-01-01", false);
        let meta = tmpl.apply(1);
        let field = meta
            .fields
            .iter()
            .find(|f| f.key == "ingest_date")
            .expect("should succeed in test");
        assert!(field.is_readonly());
    }

    #[test]
    fn test_template_empty_apply() {
        let tmpl = ClipMetadataTemplate::new("empty");
        let meta = tmpl.apply(0);
        assert_eq!(meta.field_count(), 0);
    }
}
