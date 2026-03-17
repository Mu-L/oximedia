//! Clip color labels and metadata tags for organizational workflow.
//!
//! Provides a labeling system where clips can be assigned named colors
//! and arbitrary tags. This helps editors organize large projects by
//! visually distinguishing clips (e.g., interview = blue, b-roll = green)
//! and filtering/searching by tags.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use crate::clip::ClipId;

/// A named color label.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColorLabel {
    /// Label name (e.g. "Interview", "B-Roll", "Music").
    pub name: String,
    /// Color as RGB hex string (e.g. "#FF0000").
    pub color: String,
    /// Optional keyboard shortcut digit (1-9).
    pub shortcut: Option<u8>,
}

impl ColorLabel {
    /// Create a new color label.
    #[must_use]
    pub fn new(name: impl Into<String>, color: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            color: color.into(),
            shortcut: None,
        }
    }

    /// Set a keyboard shortcut.
    #[must_use]
    pub fn with_shortcut(mut self, shortcut: u8) -> Self {
        self.shortcut = Some(shortcut.min(9));
        self
    }

    /// Parse the color as RGB bytes. Returns `None` if invalid.
    #[must_use]
    pub fn rgb(&self) -> Option<(u8, u8, u8)> {
        let hex = self.color.trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some((r, g, b))
    }
}

/// A set of standard color labels commonly used in NLE workflows.
pub struct StandardLabels;

impl StandardLabels {
    /// Get a set of standard production labels.
    #[must_use]
    pub fn production() -> Vec<ColorLabel> {
        vec![
            ColorLabel::new("Interview", "#4A90D9").with_shortcut(1),
            ColorLabel::new("B-Roll", "#7ED321").with_shortcut(2),
            ColorLabel::new("Music", "#BD10E0").with_shortcut(3),
            ColorLabel::new("SFX", "#F5A623").with_shortcut(4),
            ColorLabel::new("Graphics", "#D0021B").with_shortcut(5),
            ColorLabel::new("Voiceover", "#50E3C2").with_shortcut(6),
            ColorLabel::new("Approved", "#417505").with_shortcut(7),
            ColorLabel::new("Rejected", "#9B9B9B").with_shortcut(8),
            ColorLabel::new("Review", "#F8E71C").with_shortcut(9),
        ]
    }
}

/// A metadata tag that can be attached to a clip.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tag {
    /// Tag key (e.g. "scene", "take", "rating").
    pub key: String,
    /// Tag value (e.g. "Scene 3", "Take 2", "5").
    pub value: String,
}

impl Tag {
    /// Create a new tag.
    #[must_use]
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }

    /// Create a simple (key-only) tag with empty value.
    #[must_use]
    pub fn simple(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
        }
    }
}

/// Manages color labels and tags for clips.
#[derive(Debug, Default)]
pub struct LabelManager {
    /// Available color labels.
    labels: Vec<ColorLabel>,
    /// Clip to label name mapping.
    clip_labels: HashMap<ClipId, String>,
    /// Clip to tags mapping.
    clip_tags: HashMap<ClipId, Vec<Tag>>,
    /// All known tag keys (for autocomplete).
    known_tag_keys: HashSet<String>,
}

impl LabelManager {
    /// Create a new label manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            labels: Vec::new(),
            clip_labels: HashMap::new(),
            clip_tags: HashMap::new(),
            known_tag_keys: HashSet::new(),
        }
    }

    /// Create a label manager with standard production labels.
    #[must_use]
    pub fn with_standard_labels() -> Self {
        let mut mgr = Self::new();
        mgr.labels = StandardLabels::production();
        mgr
    }

    /// Add a custom color label.
    pub fn add_label(&mut self, label: ColorLabel) {
        if !self.labels.iter().any(|l| l.name == label.name) {
            self.labels.push(label);
        }
    }

    /// Remove a color label by name.
    pub fn remove_label(&mut self, name: &str) -> bool {
        let len_before = self.labels.len();
        self.labels.retain(|l| l.name != name);
        // Also remove from clips
        self.clip_labels.retain(|_, v| v != name);
        self.labels.len() < len_before
    }

    /// Get all available labels.
    #[must_use]
    pub fn all_labels(&self) -> &[ColorLabel] {
        &self.labels
    }

    /// Get a label by name.
    #[must_use]
    pub fn get_label(&self, name: &str) -> Option<&ColorLabel> {
        self.labels.iter().find(|l| l.name == name)
    }

    /// Assign a color label to a clip.
    pub fn set_clip_label(&mut self, clip_id: ClipId, label_name: &str) -> bool {
        if self.labels.iter().any(|l| l.name == label_name) {
            self.clip_labels.insert(clip_id, label_name.to_string());
            true
        } else {
            false
        }
    }

    /// Remove the color label from a clip.
    pub fn remove_clip_label(&mut self, clip_id: ClipId) -> Option<String> {
        self.clip_labels.remove(&clip_id)
    }

    /// Get the color label for a clip.
    #[must_use]
    pub fn get_clip_label(&self, clip_id: ClipId) -> Option<&ColorLabel> {
        let label_name = self.clip_labels.get(&clip_id)?;
        self.get_label(label_name)
    }

    /// Add a tag to a clip.
    pub fn add_clip_tag(&mut self, clip_id: ClipId, tag: Tag) {
        self.known_tag_keys.insert(tag.key.clone());
        let tags = self.clip_tags.entry(clip_id).or_default();
        // Don't add duplicate key-value pairs
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }

    /// Remove a tag from a clip by key.
    pub fn remove_clip_tag(&mut self, clip_id: ClipId, key: &str) -> bool {
        if let Some(tags) = self.clip_tags.get_mut(&clip_id) {
            let len_before = tags.len();
            tags.retain(|t| t.key != key);
            tags.len() < len_before
        } else {
            false
        }
    }

    /// Get all tags for a clip.
    #[must_use]
    pub fn get_clip_tags(&self, clip_id: ClipId) -> &[Tag] {
        self.clip_tags
            .get(&clip_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Find clips by label name.
    #[must_use]
    pub fn clips_with_label(&self, label_name: &str) -> Vec<ClipId> {
        self.clip_labels
            .iter()
            .filter(|(_, v)| v.as_str() == label_name)
            .map(|(&k, _)| k)
            .collect()
    }

    /// Find clips by tag key.
    #[must_use]
    pub fn clips_with_tag_key(&self, key: &str) -> Vec<ClipId> {
        self.clip_tags
            .iter()
            .filter(|(_, tags)| tags.iter().any(|t| t.key == key))
            .map(|(&id, _)| id)
            .collect()
    }

    /// Find clips by tag key-value pair.
    #[must_use]
    pub fn clips_with_tag(&self, key: &str, value: &str) -> Vec<ClipId> {
        self.clip_tags
            .iter()
            .filter(|(_, tags)| tags.iter().any(|t| t.key == key && t.value == value))
            .map(|(&id, _)| id)
            .collect()
    }

    /// Get all known tag keys.
    #[must_use]
    pub fn known_tag_keys(&self) -> Vec<&str> {
        self.known_tag_keys.iter().map(String::as_str).collect()
    }

    /// Remove all labels and tags for a clip.
    pub fn remove_clip(&mut self, clip_id: ClipId) {
        self.clip_labels.remove(&clip_id);
        self.clip_tags.remove(&clip_id);
    }

    /// Clear everything.
    pub fn clear(&mut self) {
        self.clip_labels.clear();
        self.clip_tags.clear();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_label_rgb() {
        let label = ColorLabel::new("Test", "#FF8800");
        let rgb = label.rgb();
        assert_eq!(rgb, Some((255, 136, 0)));
    }

    #[test]
    fn test_color_label_invalid_rgb() {
        let label = ColorLabel::new("Bad", "not-a-color");
        assert!(label.rgb().is_none());
    }

    #[test]
    fn test_color_label_shortcut() {
        let label = ColorLabel::new("Test", "#000000").with_shortcut(5);
        assert_eq!(label.shortcut, Some(5));
        // Clamp to 9
        let label2 = ColorLabel::new("Test2", "#000000").with_shortcut(15);
        assert_eq!(label2.shortcut, Some(9));
    }

    #[test]
    fn test_standard_labels() {
        let labels = StandardLabels::production();
        assert_eq!(labels.len(), 9);
        assert_eq!(labels[0].name, "Interview");
        assert!(labels[0].rgb().is_some());
    }

    #[test]
    fn test_tag_creation() {
        let tag = Tag::new("scene", "Scene 1");
        assert_eq!(tag.key, "scene");
        assert_eq!(tag.value, "Scene 1");

        let simple = Tag::simple("favorite");
        assert_eq!(simple.key, "favorite");
        assert!(simple.value.is_empty());
    }

    #[test]
    fn test_label_manager_add_remove() {
        let mut mgr = LabelManager::new();
        mgr.add_label(ColorLabel::new("Test", "#FF0000"));
        assert_eq!(mgr.all_labels().len(), 1);

        // Duplicate name should not add
        mgr.add_label(ColorLabel::new("Test", "#00FF00"));
        assert_eq!(mgr.all_labels().len(), 1);

        assert!(mgr.remove_label("Test"));
        assert_eq!(mgr.all_labels().len(), 0);
    }

    #[test]
    fn test_label_manager_clip_label() {
        let mut mgr = LabelManager::with_standard_labels();

        // Assign label
        assert!(mgr.set_clip_label(1, "Interview"));
        assert!(!mgr.set_clip_label(2, "NonExistent"));

        // Get label
        let label = mgr.get_clip_label(1);
        assert!(label.is_some());
        assert_eq!(label.expect("should exist").name, "Interview");

        assert!(mgr.get_clip_label(2).is_none());

        // Remove label
        assert!(mgr.remove_clip_label(1).is_some());
        assert!(mgr.get_clip_label(1).is_none());
    }

    #[test]
    fn test_label_manager_clip_tags() {
        let mut mgr = LabelManager::new();

        mgr.add_clip_tag(1, Tag::new("scene", "1"));
        mgr.add_clip_tag(1, Tag::new("take", "3"));
        mgr.add_clip_tag(1, Tag::new("scene", "1")); // duplicate

        assert_eq!(mgr.get_clip_tags(1).len(), 2);
        assert_eq!(mgr.get_clip_tags(999).len(), 0);

        assert!(mgr.remove_clip_tag(1, "scene"));
        assert_eq!(mgr.get_clip_tags(1).len(), 1);
        assert!(!mgr.remove_clip_tag(1, "nonexistent"));
    }

    #[test]
    fn test_label_manager_find_clips() {
        let mut mgr = LabelManager::with_standard_labels();
        mgr.set_clip_label(1, "Interview");
        mgr.set_clip_label(2, "Interview");
        mgr.set_clip_label(3, "B-Roll");

        let interviews = mgr.clips_with_label("Interview");
        assert_eq!(interviews.len(), 2);

        let broll = mgr.clips_with_label("B-Roll");
        assert_eq!(broll.len(), 1);
    }

    #[test]
    fn test_label_manager_find_by_tag() {
        let mut mgr = LabelManager::new();
        mgr.add_clip_tag(1, Tag::new("scene", "1"));
        mgr.add_clip_tag(2, Tag::new("scene", "2"));
        mgr.add_clip_tag(3, Tag::new("take", "1"));

        assert_eq!(mgr.clips_with_tag_key("scene").len(), 2);
        assert_eq!(mgr.clips_with_tag("scene", "1").len(), 1);
    }

    #[test]
    fn test_label_manager_known_keys() {
        let mut mgr = LabelManager::new();
        mgr.add_clip_tag(1, Tag::new("scene", "1"));
        mgr.add_clip_tag(2, Tag::new("take", "1"));
        let keys = mgr.known_tag_keys();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_label_manager_remove_clip() {
        let mut mgr = LabelManager::with_standard_labels();
        mgr.set_clip_label(1, "Interview");
        mgr.add_clip_tag(1, Tag::new("scene", "1"));
        mgr.remove_clip(1);
        assert!(mgr.get_clip_label(1).is_none());
        assert!(mgr.get_clip_tags(1).is_empty());
    }

    #[test]
    fn test_label_manager_clear() {
        let mut mgr = LabelManager::with_standard_labels();
        mgr.set_clip_label(1, "Interview");
        mgr.add_clip_tag(1, Tag::new("scene", "1"));
        mgr.clear();
        assert!(mgr.get_clip_label(1).is_none());
        assert!(mgr.get_clip_tags(1).is_empty());
    }

    #[test]
    fn test_removing_label_definition_removes_from_clips() {
        let mut mgr = LabelManager::new();
        mgr.add_label(ColorLabel::new("Custom", "#AABBCC"));
        mgr.set_clip_label(1, "Custom");
        assert!(mgr.get_clip_label(1).is_some());
        mgr.remove_label("Custom");
        assert!(mgr.get_clip_label(1).is_none());
    }
}
