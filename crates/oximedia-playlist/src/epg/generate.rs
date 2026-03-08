//! EPG generation from playlists.

use crate::Playlist;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A single program entry in the EPG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramEntry {
    /// Program title.
    pub title: String,

    /// Program description.
    pub description: Option<String>,

    /// Start time.
    pub start_time: DateTime<Utc>,

    /// End time.
    pub end_time: DateTime<Utc>,

    /// Duration.
    pub duration: Duration,

    /// Channel ID.
    pub channel_id: String,

    /// Episode number.
    pub episode: Option<u32>,

    /// Season number.
    pub season: Option<u32>,

    /// Content rating.
    pub rating: Option<String>,

    /// Genre tags.
    pub genres: Vec<String>,

    /// Whether this is a live program.
    pub is_live: bool,

    /// Whether this is a premiere.
    pub is_premiere: bool,

    /// Whether this is a repeat.
    pub is_repeat: bool,
}

impl ProgramEntry {
    /// Creates a new program entry.
    #[must_use]
    pub fn new<S: Into<String>>(
        title: S,
        channel_id: S,
        start_time: DateTime<Utc>,
        duration: Duration,
    ) -> Self {
        let start_time_copy = start_time;
        let end_time = start_time
            + chrono::Duration::from_std(duration).unwrap_or_else(|_| chrono::Duration::zero());

        Self {
            title: title.into(),
            description: None,
            start_time: start_time_copy,
            end_time,
            duration,
            channel_id: channel_id.into(),
            episode: None,
            season: None,
            rating: None,
            genres: Vec::new(),
            is_live: false,
            is_premiere: false,
            is_repeat: false,
        }
    }

    /// Sets the description.
    #[must_use]
    pub fn with_description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets episode and season numbers.
    #[must_use]
    pub const fn with_episode(mut self, season: u32, episode: u32) -> Self {
        self.season = Some(season);
        self.episode = Some(episode);
        self
    }

    /// Sets the content rating.
    #[must_use]
    pub fn with_rating<S: Into<String>>(mut self, rating: S) -> Self {
        self.rating = Some(rating.into());
        self
    }

    /// Adds a genre tag.
    #[must_use]
    pub fn with_genre<S: Into<String>>(mut self, genre: S) -> Self {
        self.genres.push(genre.into());
        self
    }

    /// Marks this as a live program.
    #[must_use]
    pub const fn as_live(mut self) -> Self {
        self.is_live = true;
        self
    }

    /// Marks this as a premiere.
    #[must_use]
    pub const fn as_premiere(mut self) -> Self {
        self.is_premiere = true;
        self
    }

    /// Marks this as a repeat.
    #[must_use]
    pub const fn as_repeat(mut self) -> Self {
        self.is_repeat = true;
        self
    }
}

/// EPG generator.
#[derive(Debug)]
pub struct EpgGenerator {
    programs: Vec<ProgramEntry>,
}

impl EpgGenerator {
    /// Creates a new EPG generator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            programs: Vec::new(),
        }
    }

    /// Generates EPG entries from a playlist.
    pub fn generate_from_playlist(
        &mut self,
        playlist: &Playlist,
        channel_id: &str,
        start_time: DateTime<Utc>,
    ) {
        let mut current_time = start_time;

        for item in &playlist.items {
            if !item.is_enabled() {
                continue;
            }

            let title = item
                .metadata
                .title
                .clone()
                .unwrap_or_else(|| item.name.clone());

            let mut entry =
                ProgramEntry::new(title, channel_id.to_string(), current_time, item.duration);

            if let Some(desc) = &item.metadata.description {
                entry = entry.with_description(desc);
            }

            if let Some(rating) = &item.metadata.rating {
                entry = entry.with_rating(rating);
            }

            if let (Some(season), Some(episode)) = (item.metadata.season, item.metadata.episode) {
                entry = entry.with_episode(season, episode);
            }

            for genre in &item.metadata.genre {
                entry = entry.with_genre(genre);
            }

            self.programs.push(entry);

            current_time += chrono::Duration::from_std(item.duration)
                .unwrap_or_else(|_| chrono::Duration::zero());
        }
    }

    /// Adds a program entry manually.
    pub fn add_program(&mut self, entry: ProgramEntry) {
        self.programs.push(entry);
        self.sort_programs();
    }

    /// Gets all programs for a specific channel.
    #[must_use]
    pub fn get_programs_for_channel(&self, channel_id: &str) -> Vec<&ProgramEntry> {
        self.programs
            .iter()
            .filter(|p| p.channel_id == channel_id)
            .collect()
    }

    /// Gets programs in a time range.
    #[must_use]
    pub fn get_programs_in_range(
        &self,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> Vec<&ProgramEntry> {
        self.programs
            .iter()
            .filter(|p| p.start_time < *end && p.end_time > *start)
            .collect()
    }

    /// Gets all programs.
    #[must_use]
    pub fn get_all_programs(&self) -> &[ProgramEntry] {
        &self.programs
    }

    /// Clears all programs.
    pub fn clear(&mut self) {
        self.programs.clear();
    }

    /// Sorts programs by start time.
    fn sort_programs(&mut self) {
        self.programs
            .sort_by(|a, b| a.start_time.cmp(&b.start_time));
    }

    /// Returns the number of programs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.programs.len()
    }

    /// Returns true if there are no programs.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.programs.is_empty()
    }
}

impl Default for EpgGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playlist::{PlaylistItem, PlaylistType};

    #[test]
    fn test_program_entry() {
        let entry = ProgramEntry::new(
            "Test Program",
            "channel1",
            Utc::now(),
            Duration::from_secs(3600),
        )
        .with_description("A test program")
        .with_episode(1, 5)
        .with_rating("TV-PG")
        .with_genre("Drama");

        assert_eq!(entry.title, "Test Program");
        assert_eq!(entry.season, Some(1));
        assert_eq!(entry.episode, Some(5));
    }

    #[test]
    fn test_epg_generator() {
        let mut generator = EpgGenerator::new();
        let mut playlist = Playlist::new("test", PlaylistType::Linear);

        let item = PlaylistItem::new("show.mxf")
            .with_duration(Duration::from_secs(1800))
            .with_title("Test Show");

        playlist.add_item(item);

        generator.generate_from_playlist(&playlist, "channel1", Utc::now());

        assert_eq!(generator.len(), 1);
        let programs = generator.get_programs_for_channel("channel1");
        assert_eq!(programs.len(), 1);
    }
}
