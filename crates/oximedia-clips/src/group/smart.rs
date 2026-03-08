//! Smart collection with auto-updating rules.

use super::{Collection, CollectionId};
use crate::clip::Clip;
use crate::logging::Rating;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A smart collection that auto-updates based on rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartCollection {
    /// Base collection.
    pub collection: Collection,

    /// Rules for matching clips.
    pub rules: Vec<SmartRule>,

    /// Match mode.
    pub match_mode: MatchMode,

    /// Auto-update enabled.
    pub auto_update: bool,

    /// Last update timestamp.
    pub last_updated: DateTime<Utc>,
}

/// Rule match mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchMode {
    /// Match all rules (AND).
    All,
    /// Match any rule (OR).
    Any,
}

/// A rule for smart collection matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SmartRule {
    /// Match by keyword.
    Keyword {
        /// Keyword to match.
        keyword: String,
    },

    /// Match by rating.
    Rating {
        /// Comparison operator.
        operator: Comparison,
        /// Rating value.
        value: Rating,
    },

    /// Match by favorite status.
    IsFavorite {
        /// Whether the clip is favorite.
        is_favorite: bool,
    },

    /// Match by rejected status.
    IsRejected {
        /// Whether the clip is rejected.
        is_rejected: bool,
    },

    /// Match by file name pattern.
    FileName {
        /// File name pattern.
        pattern: String,
    },

    /// Match by duration.
    Duration {
        /// Comparison operator.
        operator: Comparison,
        /// Duration in frames.
        frames: i64,
    },

    /// Match by creation date.
    CreatedDate {
        /// Comparison operator.
        operator: Comparison,
        /// Creation date.
        date: DateTime<Utc>,
    },

    /// Match by modification date.
    ModifiedDate {
        /// Comparison operator.
        operator: Comparison,
        /// Modification date.
        date: DateTime<Utc>,
    },

    /// Match clips with markers.
    HasMarkers,

    /// Match clips with notes.
    HasNotes,

    /// Match by custom metadata field.
    CustomMetadata {
        /// Metadata key.
        key: String,
        /// Metadata value.
        value: String,
    },
}

/// Comparison operator for rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Comparison {
    /// Equal to.
    Equal,
    /// Not equal to.
    NotEqual,
    /// Greater than.
    GreaterThan,
    /// Greater than or equal.
    GreaterThanOrEqual,
    /// Less than.
    LessThan,
    /// Less than or equal.
    LessThanOrEqual,
}

impl SmartCollection {
    /// Creates a new smart collection.
    #[must_use]
    pub fn new(name: impl Into<String>, rules: Vec<SmartRule>, match_mode: MatchMode) -> Self {
        Self {
            collection: Collection::new(name),
            rules,
            match_mode,
            auto_update: true,
            last_updated: Utc::now(),
        }
    }

    /// Returns the collection ID.
    #[must_use]
    pub const fn id(&self) -> CollectionId {
        self.collection.id
    }

    /// Checks if a clip matches the smart collection rules.
    #[must_use]
    pub fn matches(&self, clip: &Clip) -> bool {
        if self.rules.is_empty() {
            return false;
        }

        match self.match_mode {
            MatchMode::All => self.rules.iter().all(|rule| rule.matches(clip)),
            MatchMode::Any => self.rules.iter().any(|rule| rule.matches(clip)),
        }
    }

    /// Updates the collection by evaluating all clips.
    pub fn update(&mut self, clips: &[Clip]) {
        self.collection.clear();

        for clip in clips {
            if self.matches(clip) {
                self.collection.add_clip(clip.id);
            }
        }

        self.last_updated = Utc::now();
    }

    /// Adds a rule.
    pub fn add_rule(&mut self, rule: SmartRule) {
        self.rules.push(rule);
    }

    /// Removes a rule at index.
    pub fn remove_rule(&mut self, index: usize) -> Option<SmartRule> {
        if index < self.rules.len() {
            Some(self.rules.remove(index))
        } else {
            None
        }
    }

    /// Sets the match mode.
    pub fn set_match_mode(&mut self, mode: MatchMode) {
        self.match_mode = mode;
    }
}

impl SmartRule {
    /// Checks if a clip matches this rule.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn matches(&self, clip: &Clip) -> bool {
        match self {
            Self::Keyword { keyword } => clip.keywords.contains(keyword),

            Self::Rating { operator, value } => match operator {
                Comparison::Equal => clip.rating == *value,
                Comparison::NotEqual => clip.rating != *value,
                Comparison::GreaterThan => clip.rating > *value,
                Comparison::GreaterThanOrEqual => clip.rating >= *value,
                Comparison::LessThan => clip.rating < *value,
                Comparison::LessThanOrEqual => clip.rating <= *value,
            },

            Self::IsFavorite { is_favorite } => clip.is_favorite == *is_favorite,

            Self::IsRejected { is_rejected } => clip.is_rejected == *is_rejected,

            Self::FileName { pattern } => clip
                .file_path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| name.contains(pattern)),

            Self::Duration { operator, frames } => {
                if let Some(duration) = clip.effective_duration() {
                    match operator {
                        Comparison::Equal => duration == *frames,
                        Comparison::NotEqual => duration != *frames,
                        Comparison::GreaterThan => duration > *frames,
                        Comparison::GreaterThanOrEqual => duration >= *frames,
                        Comparison::LessThan => duration < *frames,
                        Comparison::LessThanOrEqual => duration <= *frames,
                    }
                } else {
                    false
                }
            }

            Self::CreatedDate { operator, date } => match operator {
                Comparison::Equal => clip.created_at == *date,
                Comparison::NotEqual => clip.created_at != *date,
                Comparison::GreaterThan => clip.created_at > *date,
                Comparison::GreaterThanOrEqual => clip.created_at >= *date,
                Comparison::LessThan => clip.created_at < *date,
                Comparison::LessThanOrEqual => clip.created_at <= *date,
            },

            Self::ModifiedDate { operator, date } => match operator {
                Comparison::Equal => clip.modified_at == *date,
                Comparison::NotEqual => clip.modified_at != *date,
                Comparison::GreaterThan => clip.modified_at > *date,
                Comparison::GreaterThanOrEqual => clip.modified_at >= *date,
                Comparison::LessThan => clip.modified_at < *date,
                Comparison::LessThanOrEqual => clip.modified_at <= *date,
            },

            Self::HasMarkers => !clip.markers.is_empty(),

            Self::HasNotes => false, // Would need access to note database

            Self::CustomMetadata { key, value } => clip
                .custom_metadata
                .as_ref()
                .and_then(|json| {
                    serde_json::from_str::<serde_json::Value>(json)
                        .ok()
                        .and_then(|v| v.get(key).and_then(|val| val.as_str().map(String::from)))
                })
                .is_some_and(|v| &v == value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_smart_collection_keyword() {
        let rule = SmartRule::Keyword {
            keyword: "interview".to_string(),
        };
        let rules = vec![rule];
        let smart = SmartCollection::new("Interviews", rules, MatchMode::All);

        let mut clip = Clip::new(PathBuf::from("/test.mov"));
        clip.add_keyword("interview");

        assert!(smart.matches(&clip));
    }

    #[test]
    fn test_smart_collection_rating() {
        let rule = SmartRule::Rating {
            operator: Comparison::GreaterThanOrEqual,
            value: Rating::FourStars,
        };
        let smart = SmartCollection::new("High Rated", vec![rule], MatchMode::All);

        let mut clip = Clip::new(PathBuf::from("/test.mov"));
        clip.set_rating(Rating::FiveStars);

        assert!(smart.matches(&clip));
    }

    #[test]
    fn test_smart_collection_match_modes() {
        let rules = vec![
            SmartRule::IsFavorite { is_favorite: true },
            SmartRule::Rating {
                operator: Comparison::GreaterThanOrEqual,
                value: Rating::FourStars,
            },
        ];

        let smart_all = SmartCollection::new("Test All", rules.clone(), MatchMode::All);
        let smart_any = SmartCollection::new("Test Any", rules, MatchMode::Any);

        let mut clip = Clip::new(PathBuf::from("/test.mov"));
        clip.set_favorite(true);

        // Matches ANY but not ALL
        assert!(smart_any.matches(&clip));
        assert!(!smart_all.matches(&clip));

        clip.set_rating(Rating::FourStars);

        // Now matches ALL
        assert!(smart_all.matches(&clip));
    }
}
