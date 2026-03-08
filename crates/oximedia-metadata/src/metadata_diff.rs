#![allow(dead_code)]
//! Metadata diff computation and application.
//!
//! Provides tools to compare two metadata states and produce a structured
//! diff that can be applied or inspected.

use std::collections::HashMap;

/// Describes a single change to a metadata field.
#[derive(Debug, Clone, PartialEq)]
pub enum MetadataChange {
    /// A new field was added with the given value.
    Added(String),
    /// A field was removed; the old value is stored.
    Removed(String),
    /// A field's value changed from old to new.
    Modified { old: String, new: String },
}

impl MetadataChange {
    /// Returns `true` if this change is considered destructive (removes or modifies data).
    pub fn is_destructive(&self) -> bool {
        matches!(self, Self::Removed(_) | Self::Modified { .. })
    }

    /// Returns the new value string if one exists after the change.
    pub fn new_value(&self) -> Option<&str> {
        match self {
            Self::Added(v) | Self::Modified { new: v, .. } => Some(v),
            Self::Removed(_) => None,
        }
    }

    /// Returns the old value string if one existed before the change.
    pub fn old_value(&self) -> Option<&str> {
        match self {
            Self::Removed(v) | Self::Modified { old: v, .. } => Some(v),
            Self::Added(_) => None,
        }
    }
}

/// A complete diff between two metadata snapshots.
#[derive(Debug, Clone, Default)]
pub struct MetadataDiff {
    changes: HashMap<String, MetadataChange>,
}

impl MetadataDiff {
    /// Create an empty diff.
    pub fn new() -> Self {
        Self {
            changes: HashMap::new(),
        }
    }

    /// Compute the diff between `before` and `after` snapshots.
    ///
    /// Both arguments are `&HashMap<String, String>` representing field name → text value.
    pub fn compute(before: &HashMap<String, String>, after: &HashMap<String, String>) -> Self {
        let mut changes = HashMap::new();

        // Detect added and modified fields.
        for (key, new_val) in after {
            match before.get(key) {
                None => {
                    changes.insert(key.clone(), MetadataChange::Added(new_val.clone()));
                }
                Some(old_val) if old_val != new_val => {
                    changes.insert(
                        key.clone(),
                        MetadataChange::Modified {
                            old: old_val.clone(),
                            new: new_val.clone(),
                        },
                    );
                }
                _ => {}
            }
        }

        // Detect removed fields.
        for key in before.keys() {
            if !after.contains_key(key) {
                let old_val = before[key].clone();
                changes.insert(key.clone(), MetadataChange::Removed(old_val));
            }
        }

        Self { changes }
    }

    /// Returns the names of all fields that were added.
    pub fn added_fields(&self) -> Vec<&str> {
        self.changes
            .iter()
            .filter_map(|(k, v)| {
                if matches!(v, MetadataChange::Added(_)) {
                    Some(k.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Returns the names of all fields that were removed.
    pub fn removed_fields(&self) -> Vec<&str> {
        self.changes
            .iter()
            .filter_map(|(k, v)| {
                if matches!(v, MetadataChange::Removed(_)) {
                    Some(k.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Returns the names of all fields whose values changed.
    pub fn changed_fields(&self) -> Vec<&str> {
        self.changes
            .iter()
            .filter_map(|(k, v)| {
                if matches!(v, MetadataChange::Modified { .. }) {
                    Some(k.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Returns the full change map.
    pub fn changes(&self) -> &HashMap<String, MetadataChange> {
        &self.changes
    }

    /// Returns `true` if there are no changes.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Returns the total number of changes.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Returns `true` if any change in the diff is destructive.
    pub fn has_destructive_changes(&self) -> bool {
        self.changes.values().any(MetadataChange::is_destructive)
    }
}

/// Applies a [`MetadataDiff`] to a metadata snapshot.
pub struct MetadataDiffApplier;

impl MetadataDiffApplier {
    /// Apply the diff to `target`, returning a new snapshot.
    pub fn apply(target: &HashMap<String, String>, diff: &MetadataDiff) -> HashMap<String, String> {
        let mut result = target.clone();

        for (key, change) in diff.changes() {
            match change {
                MetadataChange::Added(val) => {
                    result.insert(key.clone(), val.clone());
                }
                MetadataChange::Removed(_) => {
                    result.remove(key);
                }
                MetadataChange::Modified { new, .. } => {
                    result.insert(key.clone(), new.clone());
                }
            }
        }

        result
    }

    /// Apply the diff in-place to `target`.
    pub fn apply_in_place(target: &mut HashMap<String, String>, diff: &MetadataDiff) {
        for (key, change) in diff.changes() {
            match change {
                MetadataChange::Added(val) | MetadataChange::Modified { new: val, .. } => {
                    target.insert(key.clone(), val.clone());
                }
                MetadataChange::Removed(_) => {
                    target.remove(key);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_change_added_not_destructive() {
        let c = MetadataChange::Added("val".to_string());
        assert!(!c.is_destructive());
    }

    #[test]
    fn test_change_removed_is_destructive() {
        let c = MetadataChange::Removed("val".to_string());
        assert!(c.is_destructive());
    }

    #[test]
    fn test_change_modified_is_destructive() {
        let c = MetadataChange::Modified {
            old: "a".to_string(),
            new: "b".to_string(),
        };
        assert!(c.is_destructive());
    }

    #[test]
    fn test_change_new_value() {
        let c = MetadataChange::Added("hello".to_string());
        assert_eq!(c.new_value(), Some("hello"));
        let r = MetadataChange::Removed("old".to_string());
        assert_eq!(r.new_value(), None);
    }

    #[test]
    fn test_change_old_value() {
        let c = MetadataChange::Modified {
            old: "prev".to_string(),
            new: "next".to_string(),
        };
        assert_eq!(c.old_value(), Some("prev"));
        let a = MetadataChange::Added("x".to_string());
        assert_eq!(a.old_value(), None);
    }

    #[test]
    fn test_diff_compute_added() {
        let before = map(&[]);
        let after = map(&[("title", "Song")]);
        let diff = MetadataDiff::compute(&before, &after);
        assert_eq!(diff.added_fields(), vec!["title"]);
        assert!(diff.removed_fields().is_empty());
        assert!(diff.changed_fields().is_empty());
    }

    #[test]
    fn test_diff_compute_removed() {
        let before = map(&[("title", "Song")]);
        let after = map(&[]);
        let diff = MetadataDiff::compute(&before, &after);
        assert_eq!(diff.removed_fields(), vec!["title"]);
        assert!(diff.added_fields().is_empty());
    }

    #[test]
    fn test_diff_compute_modified() {
        let before = map(&[("title", "Old")]);
        let after = map(&[("title", "New")]);
        let diff = MetadataDiff::compute(&before, &after);
        assert_eq!(diff.changed_fields(), vec!["title"]);
    }

    #[test]
    fn test_diff_compute_no_change() {
        let before = map(&[("title", "Same")]);
        let after = map(&[("title", "Same")]);
        let diff = MetadataDiff::compute(&before, &after);
        assert!(diff.is_empty());
    }

    #[test]
    fn test_diff_len() {
        let before = map(&[("a", "1"), ("b", "2")]);
        let after = map(&[("b", "3"), ("c", "4")]);
        let diff = MetadataDiff::compute(&before, &after);
        assert_eq!(diff.len(), 3); // a removed, b modified, c added
    }

    #[test]
    fn test_diff_has_destructive_changes() {
        let before = map(&[("a", "1")]);
        let after = map(&[]);
        let diff = MetadataDiff::compute(&before, &after);
        assert!(diff.has_destructive_changes());
    }

    #[test]
    fn test_applier_apply() {
        let before = map(&[("title", "Old"), ("artist", "A")]);
        let after = map(&[("title", "New"), ("album", "X")]);
        let diff = MetadataDiff::compute(&before, &after);
        let result = MetadataDiffApplier::apply(&before, &diff);
        assert_eq!(result.get("title").map(String::as_str), Some("New"));
        assert_eq!(result.get("album").map(String::as_str), Some("X"));
        assert!(!result.contains_key("artist"));
    }

    #[test]
    fn test_applier_apply_in_place() {
        let mut target = map(&[("x", "1")]);
        let before = target.clone();
        let after = map(&[("x", "2"), ("y", "3")]);
        let diff = MetadataDiff::compute(&before, &after);
        MetadataDiffApplier::apply_in_place(&mut target, &diff);
        assert_eq!(target.get("x").map(String::as_str), Some("2"));
        assert_eq!(target.get("y").map(String::as_str), Some("3"));
    }

    #[test]
    fn test_diff_default_is_empty() {
        let diff = MetadataDiff::default();
        assert!(diff.is_empty());
        assert_eq!(diff.len(), 0);
    }
}
