#![allow(dead_code)]
//! Subtitle and timed-text resource management for IMF packages.
//!
//! IMF packages may include IMSC1 (TTML-based) subtitle tracks as separate
//! essence files. This module provides structures for managing subtitle resources
//! within an IMF composition, including timing, language mapping, and validation.

use std::collections::HashMap;
use std::fmt;

/// Subtitle format type within an IMF package.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SubtitleFormat {
    /// IMSC1 (Internet Media Subtitles and Captions) - SMPTE ST 2067-2.
    Imsc1,
    /// IMSC1 Text profile.
    Imsc1Text,
    /// IMSC1 Image profile.
    Imsc1Image,
    /// SMPTE-TT (SMPTE Timed Text).
    SmpteTt,
}

impl fmt::Display for SubtitleFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Imsc1 => write!(f, "IMSC1"),
            Self::Imsc1Text => write!(f, "IMSC1-Text"),
            Self::Imsc1Image => write!(f, "IMSC1-Image"),
            Self::SmpteTt => write!(f, "SMPTE-TT"),
        }
    }
}

/// Language code and metadata for a subtitle track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubtitleLanguage {
    /// BCP-47 language tag (e.g., "en-US", "ja-JP").
    pub language_tag: String,
    /// Human-readable language name.
    pub display_name: String,
    /// Whether this is a forced narrative subtitle track.
    pub forced: bool,
    /// Whether this track is for hearing-impaired audiences (SDH/CC).
    pub hearing_impaired: bool,
}

impl SubtitleLanguage {
    /// Creates a new subtitle language entry.
    pub fn new(tag: &str, name: &str) -> Self {
        Self {
            language_tag: tag.to_string(),
            display_name: name.to_string(),
            forced: false,
            hearing_impaired: false,
        }
    }

    /// Sets the forced narrative flag.
    pub fn with_forced(mut self, forced: bool) -> Self {
        self.forced = forced;
        self
    }

    /// Sets the hearing-impaired flag.
    pub fn with_hearing_impaired(mut self, hi: bool) -> Self {
        self.hearing_impaired = hi;
        self
    }
}

impl fmt::Display for SubtitleLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.display_name, self.language_tag)?;
        if self.forced {
            write!(f, " [forced]")?;
        }
        if self.hearing_impaired {
            write!(f, " [HI]")?;
        }
        Ok(())
    }
}

/// Time range for a subtitle resource within the CPL timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct SubtitleTimeRange {
    /// Entry point in edit units from the start of the resource.
    pub entry_point: u64,
    /// Duration in edit units.
    pub duration: u64,
    /// Edit rate numerator.
    pub edit_rate_num: u32,
    /// Edit rate denominator.
    pub edit_rate_den: u32,
}

impl SubtitleTimeRange {
    /// Creates a new time range.
    pub fn new(entry: u64, duration: u64, rate_num: u32, rate_den: u32) -> Self {
        Self {
            entry_point: entry,
            duration,
            edit_rate_num: rate_num,
            edit_rate_den: rate_den,
        }
    }

    /// Returns the duration in seconds.
    #[allow(clippy::cast_precision_loss)]
    pub fn duration_seconds(&self) -> f64 {
        if self.edit_rate_num == 0 {
            return 0.0;
        }
        self.duration as f64 * self.edit_rate_den as f64 / self.edit_rate_num as f64
    }

    /// Returns the entry point in seconds.
    #[allow(clippy::cast_precision_loss)]
    pub fn entry_point_seconds(&self) -> f64 {
        if self.edit_rate_num == 0 {
            return 0.0;
        }
        self.entry_point as f64 * self.edit_rate_den as f64 / self.edit_rate_num as f64
    }

    /// Returns the end point in edit units.
    pub fn end_point(&self) -> u64 {
        self.entry_point + self.duration
    }

    /// Checks if this range overlaps with another.
    pub fn overlaps(&self, other: &Self) -> bool {
        self.entry_point < other.end_point() && other.entry_point < self.end_point()
    }
}

/// A subtitle resource entry within an IMF CPL.
#[derive(Clone, Debug)]
pub struct SubtitleResource {
    /// Unique resource identifier (UUID).
    pub id: String,
    /// Track file identifier referencing the MXF essence.
    pub track_file_id: String,
    /// Subtitle format.
    pub format: SubtitleFormat,
    /// Language metadata.
    pub language: SubtitleLanguage,
    /// Time range within the CPL.
    pub time_range: SubtitleTimeRange,
    /// Intrinsic duration of the source file in edit units.
    pub intrinsic_duration: u64,
    /// Hash of the track file (hex string).
    pub hash: Option<String>,
}

impl SubtitleResource {
    /// Creates a new subtitle resource.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: &str,
        track_file_id: &str,
        format: SubtitleFormat,
        language: SubtitleLanguage,
        time_range: SubtitleTimeRange,
        intrinsic_duration: u64,
    ) -> Self {
        Self {
            id: id.to_string(),
            track_file_id: track_file_id.to_string(),
            format,
            language,
            time_range,
            intrinsic_duration,
            hash: None,
        }
    }

    /// Sets the hash for this resource.
    pub fn with_hash(mut self, hash: &str) -> Self {
        self.hash = Some(hash.to_string());
        self
    }

    /// Validates that the time range is within the intrinsic duration.
    pub fn is_time_range_valid(&self) -> bool {
        self.time_range.end_point() <= self.intrinsic_duration
    }
}

/// Manages a collection of subtitle resources for an IMF composition.
#[derive(Clone, Debug)]
pub struct SubtitleResourceManager {
    /// All subtitle resources keyed by resource ID.
    resources: HashMap<String, SubtitleResource>,
}

impl SubtitleResourceManager {
    /// Creates a new empty manager.
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
        }
    }

    /// Adds a subtitle resource.
    pub fn add(&mut self, resource: SubtitleResource) {
        self.resources.insert(resource.id.clone(), resource);
    }

    /// Gets a resource by ID.
    pub fn get(&self, id: &str) -> Option<&SubtitleResource> {
        self.resources.get(id)
    }

    /// Removes a resource by ID.
    pub fn remove(&mut self, id: &str) -> Option<SubtitleResource> {
        self.resources.remove(id)
    }

    /// Returns the number of subtitle resources.
    pub fn len(&self) -> usize {
        self.resources.len()
    }

    /// Returns true if there are no subtitle resources.
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    /// Returns all unique language tags.
    pub fn languages(&self) -> Vec<String> {
        let mut langs: Vec<String> = self
            .resources
            .values()
            .map(|r| r.language.language_tag.clone())
            .collect();
        langs.sort();
        langs.dedup();
        langs
    }

    /// Returns resources for a specific language tag.
    pub fn by_language(&self, lang_tag: &str) -> Vec<&SubtitleResource> {
        self.resources
            .values()
            .filter(|r| r.language.language_tag == lang_tag)
            .collect()
    }

    /// Returns resources of a specific format.
    pub fn by_format(&self, format: SubtitleFormat) -> Vec<&SubtitleResource> {
        self.resources
            .values()
            .filter(|r| r.format == format)
            .collect()
    }

    /// Validates all resources and returns a list of issues.
    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();
        for r in self.resources.values() {
            if !r.is_time_range_valid() {
                issues.push(format!(
                    "Resource {}: time range exceeds intrinsic duration ({} > {})",
                    r.id,
                    r.time_range.end_point(),
                    r.intrinsic_duration
                ));
            }
            if r.language.language_tag.is_empty() {
                issues.push(format!("Resource {}: missing language tag", r.id));
            }
        }
        issues
    }
}

impl Default for SubtitleResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_resource(id: &str, lang: &str, entry: u64, dur: u64) -> SubtitleResource {
        SubtitleResource::new(
            id,
            &format!("track-{id}"),
            SubtitleFormat::Imsc1Text,
            SubtitleLanguage::new(lang, lang),
            SubtitleTimeRange::new(entry, dur, 24, 1),
            1000,
        )
    }

    #[test]
    fn test_subtitle_format_display() {
        assert_eq!(format!("{}", SubtitleFormat::Imsc1), "IMSC1");
        assert_eq!(format!("{}", SubtitleFormat::Imsc1Text), "IMSC1-Text");
        assert_eq!(format!("{}", SubtitleFormat::SmpteTt), "SMPTE-TT");
    }

    #[test]
    fn test_subtitle_language_basic() {
        let lang = SubtitleLanguage::new("en-US", "English");
        assert_eq!(lang.language_tag, "en-US");
        assert!(!lang.forced);
        assert!(!lang.hearing_impaired);
    }

    #[test]
    fn test_subtitle_language_flags() {
        let lang = SubtitleLanguage::new("en-US", "English")
            .with_forced(true)
            .with_hearing_impaired(true);
        assert!(lang.forced);
        assert!(lang.hearing_impaired);
        let display = format!("{lang}");
        assert!(display.contains("[forced]"));
        assert!(display.contains("[HI]"));
    }

    #[test]
    fn test_time_range_duration_seconds() {
        let tr = SubtitleTimeRange::new(0, 240, 24, 1);
        assert!((tr.duration_seconds() - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_time_range_entry_point_seconds() {
        let tr = SubtitleTimeRange::new(48, 240, 24, 1);
        assert!((tr.entry_point_seconds() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_time_range_end_point() {
        let tr = SubtitleTimeRange::new(100, 200, 24, 1);
        assert_eq!(tr.end_point(), 300);
    }

    #[test]
    fn test_time_range_overlaps() {
        let a = SubtitleTimeRange::new(0, 100, 24, 1);
        let b = SubtitleTimeRange::new(50, 100, 24, 1);
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
    }

    #[test]
    fn test_time_range_no_overlap() {
        let a = SubtitleTimeRange::new(0, 100, 24, 1);
        let b = SubtitleTimeRange::new(100, 50, 24, 1);
        assert!(!a.overlaps(&b));
    }

    #[test]
    fn test_resource_time_range_valid() {
        let r = make_resource("r1", "en", 0, 500);
        assert!(r.is_time_range_valid());
    }

    #[test]
    fn test_resource_time_range_invalid() {
        let r = make_resource("r1", "en", 900, 200); // 900+200=1100 > 1000
        assert!(!r.is_time_range_valid());
    }

    #[test]
    fn test_manager_add_get() {
        let mut mgr = SubtitleResourceManager::new();
        mgr.add(make_resource("r1", "en", 0, 100));
        assert_eq!(mgr.len(), 1);
        assert!(mgr.get("r1").is_some());
        assert!(mgr.get("r2").is_none());
    }

    #[test]
    fn test_manager_languages() {
        let mut mgr = SubtitleResourceManager::new();
        mgr.add(make_resource("r1", "en", 0, 100));
        mgr.add(make_resource("r2", "ja", 0, 100));
        mgr.add(make_resource("r3", "en", 100, 100));
        let langs = mgr.languages();
        assert_eq!(langs.len(), 2);
        assert!(langs.contains(&"en".to_string()));
        assert!(langs.contains(&"ja".to_string()));
    }

    #[test]
    fn test_manager_by_language() {
        let mut mgr = SubtitleResourceManager::new();
        mgr.add(make_resource("r1", "en", 0, 100));
        mgr.add(make_resource("r2", "ja", 0, 100));
        let en_resources = mgr.by_language("en");
        assert_eq!(en_resources.len(), 1);
    }

    #[test]
    fn test_manager_validate() {
        let mut mgr = SubtitleResourceManager::new();
        mgr.add(make_resource("ok", "en", 0, 500));
        mgr.add(make_resource("bad", "ja", 900, 200));
        let issues = mgr.validate();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("bad"));
    }

    #[test]
    fn test_manager_remove() {
        let mut mgr = SubtitleResourceManager::new();
        mgr.add(make_resource("r1", "en", 0, 100));
        assert!(mgr.remove("r1").is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn test_time_range_zero_rate() {
        let tr = SubtitleTimeRange::new(0, 100, 0, 1);
        assert_eq!(tr.duration_seconds(), 0.0);
        assert_eq!(tr.entry_point_seconds(), 0.0);
    }
}
