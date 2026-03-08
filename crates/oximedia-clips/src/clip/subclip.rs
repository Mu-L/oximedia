//! Subclip creation and management.

use super::ClipId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a subclip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubClipId(Uuid);

impl SubClipId {
    /// Creates a new random subclip ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates a subclip ID from a UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the inner UUID.
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for SubClipId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SubClipId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A subclip represents a portion of a parent clip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubClip {
    /// Unique identifier.
    pub id: SubClipId,

    /// Parent clip ID.
    pub parent_clip_id: ClipId,

    /// Display name.
    pub name: String,

    /// In point (frame number relative to parent).
    pub in_point: i64,

    /// Out point (frame number relative to parent).
    pub out_point: i64,

    /// Optional description.
    pub description: Option<String>,
}

impl SubClip {
    /// Creates a new subclip.
    #[must_use]
    pub fn new(
        parent_clip_id: ClipId,
        name: impl Into<String>,
        in_point: i64,
        out_point: i64,
    ) -> Self {
        Self {
            id: SubClipId::new(),
            parent_clip_id,
            name: name.into(),
            in_point,
            out_point,
            description: None,
        }
    }

    /// Returns the duration of this subclip.
    #[must_use]
    pub const fn duration(&self) -> i64 {
        self.out_point - self.in_point
    }

    /// Checks if the subclip has a valid range.
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.in_point < self.out_point
    }

    /// Sets the description.
    pub fn set_description(&mut self, description: impl Into<String>) {
        self.description = Some(description.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subclip_creation() {
        let parent_id = ClipId::new();
        let subclip = SubClip::new(parent_id, "Action Scene", 100, 500);
        assert_eq!(subclip.name, "Action Scene");
        assert_eq!(subclip.in_point, 100);
        assert_eq!(subclip.out_point, 500);
        assert_eq!(subclip.duration(), 400);
        assert!(subclip.is_valid());
    }

    #[test]
    fn test_invalid_subclip() {
        let parent_id = ClipId::new();
        let subclip = SubClip::new(parent_id, "Invalid", 500, 100);
        assert!(!subclip.is_valid());
    }
}
