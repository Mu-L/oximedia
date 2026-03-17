//! Smart collection with auto-updating rules.

use super::{Collection, CollectionId};
use crate::clip::Clip;
use crate::logging::Rating;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

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

    /// Polling interval for auto-refresh in seconds. `None` means no polling.
    #[serde(default)]
    pub poll_interval_secs: Option<u64>,

    /// Cached result: clip IDs that currently match the rules.
    #[serde(default)]
    pub cached_clip_ids: Vec<crate::clip::ClipId>,

    /// Whether the cache is considered valid.
    #[serde(default)]
    pub cache_valid: bool,
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
            poll_interval_secs: None,
            cached_clip_ids: Vec::new(),
            cache_valid: false,
        }
    }

    /// Sets the polling interval for auto-refresh.
    ///
    /// When set, `needs_refresh()` will return `true` if more than `interval`
    /// has elapsed since the last update.
    pub fn set_poll_interval(&mut self, interval: Duration) {
        self.poll_interval_secs = Some(interval.as_secs().max(1));
    }

    /// Clears the polling interval (disables timed auto-refresh).
    pub fn clear_poll_interval(&mut self) {
        self.poll_interval_secs = None;
    }

    /// Returns the configured poll interval, if any.
    #[must_use]
    pub fn poll_interval(&self) -> Option<Duration> {
        self.poll_interval_secs.map(Duration::from_secs)
    }

    /// Returns `true` if the collection should be refreshed.
    ///
    /// Criteria:
    /// - `auto_update` is enabled, AND
    /// - either the cache is marked invalid, OR the poll interval has elapsed.
    #[must_use]
    pub fn needs_refresh(&self) -> bool {
        if !self.auto_update {
            return false;
        }
        if !self.cache_valid {
            return true;
        }
        if let Some(interval_secs) = self.poll_interval_secs {
            let elapsed = Utc::now()
                .signed_duration_since(self.last_updated)
                .num_seconds();
            return elapsed >= interval_secs as i64;
        }
        false
    }

    /// Invalidates the cached results, forcing the next call to `needs_refresh`
    /// (when `auto_update` is enabled) to return `true`.
    pub fn invalidate_cache(&mut self) {
        self.cache_valid = false;
        self.cached_clip_ids.clear();
    }

    /// Returns the cached clip IDs if the cache is valid.
    #[must_use]
    pub fn cached_clip_ids(&self) -> Option<&[crate::clip::ClipId]> {
        if self.cache_valid {
            Some(&self.cached_clip_ids)
        } else {
            None
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
    ///
    /// Also refreshes the internal cache so that `cached_clip_ids()` returns
    /// the up-to-date list.
    pub fn update(&mut self, clips: &[Clip]) {
        self.collection.clear();
        self.cached_clip_ids.clear();

        for clip in clips {
            if self.matches(clip) {
                self.collection.add_clip(clip.id);
                self.cached_clip_ids.push(clip.id);
            }
        }

        self.last_updated = Utc::now();
        self.cache_valid = true;
    }

    /// Updates the collection only if `needs_refresh()` returns `true`.
    ///
    /// Returns `true` if a refresh was performed.
    pub fn refresh_if_needed(&mut self, clips: &[Clip]) -> bool {
        if self.needs_refresh() {
            self.update(clips);
            true
        } else {
            false
        }
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

    #[test]
    fn test_smart_collection_auto_refresh_needs_refresh_when_cache_invalid() {
        let rule = SmartRule::Keyword {
            keyword: "interview".to_string(),
        };
        let mut smart = SmartCollection::new("Interviews", vec![rule], MatchMode::All);

        // Fresh collection with no cache should need refresh (cache_valid = false).
        assert!(smart.needs_refresh());

        // After update the cache is valid, no polling interval set → no refresh needed.
        let clips: Vec<Clip> = Vec::new();
        smart.update(&clips);
        assert!(!smart.needs_refresh());
    }

    #[test]
    fn test_smart_collection_invalidate_cache() {
        let rule = SmartRule::Keyword {
            keyword: "outdoor".to_string(),
        };
        let mut smart = SmartCollection::new("Outdoor", vec![rule], MatchMode::All);

        let clips: Vec<Clip> = Vec::new();
        smart.update(&clips);
        assert!(!smart.needs_refresh());

        // Invalidate the cache explicitly.
        smart.invalidate_cache();
        assert!(smart.needs_refresh());
        assert!(smart.cached_clip_ids().is_none());
    }

    #[test]
    fn test_smart_collection_poll_interval_accessors() {
        let rule = SmartRule::HasMarkers;
        let mut smart = SmartCollection::new("Marked", vec![rule], MatchMode::All);

        assert!(smart.poll_interval().is_none());

        smart.set_poll_interval(Duration::from_secs(60));
        assert_eq!(smart.poll_interval(), Some(Duration::from_secs(60)));

        smart.clear_poll_interval();
        assert!(smart.poll_interval().is_none());
    }

    #[test]
    fn test_smart_collection_cache_populated_after_update() {
        let rule = SmartRule::Keyword {
            keyword: "interview".to_string(),
        };
        let mut smart = SmartCollection::new("Interviews", vec![rule], MatchMode::All);

        let mut clip = Clip::new(PathBuf::from("/test.mov"));
        clip.add_keyword("interview");

        smart.update(&[clip.clone()]);

        let cached = smart.cached_clip_ids().expect("cache should be valid");
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0], clip.id);
    }

    #[test]
    fn test_smart_collection_auto_update_new_clips() {
        let rule = SmartRule::Keyword {
            keyword: "broll".to_string(),
        };
        let mut smart = SmartCollection::new("B-Roll", vec![rule], MatchMode::All);

        // No clips initially.
        smart.update(&[]);
        assert_eq!(smart.collection.count(), 0);

        // Add a matching clip and re-update.
        let mut clip = Clip::new(PathBuf::from("/broll.mov"));
        clip.add_keyword("broll");
        smart.update(&[clip]);
        assert_eq!(smart.collection.count(), 1);
    }

    #[test]
    fn test_refresh_if_needed_skips_when_not_needed() {
        let rule = SmartRule::Keyword {
            keyword: "action".to_string(),
        };
        let mut smart = SmartCollection::new("Action", vec![rule], MatchMode::All);
        let clips: Vec<Clip> = Vec::new();

        // First refresh always runs (cache invalid).
        let refreshed = smart.refresh_if_needed(&clips);
        assert!(refreshed);

        // Second call: cache is valid, no polling interval → skip.
        let refreshed = smart.refresh_if_needed(&clips);
        assert!(!refreshed);
    }
}
