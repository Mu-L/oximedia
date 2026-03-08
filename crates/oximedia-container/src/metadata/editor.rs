//! High-level metadata editing API.
//!
//! Provides a convenient interface for reading and modifying metadata
//! in media files.

use oximedia_core::OxiResult;
use oximedia_io::FileSource;
use std::path::{Path, PathBuf};

use super::reader::{detect_format, FlacMetadataReader, MatroskaMetadataReader, MetadataReader};
use super::tags::{StandardTag, TagMap, TagValue};
use super::util::MediaSourceExt;
use super::writer::{
    FlacMetadataWriter, MatroskaMetadataWriter, MetadataWriter, OggMetadataWriter,
};
use crate::demux::Demuxer;
use crate::demux::MatroskaDemuxer;
use crate::ContainerFormat;

/// Metadata format detection result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataFormat {
    /// FLAC uses Vorbis comments.
    Flac,
    /// Ogg (Vorbis, Opus) uses Vorbis comments.
    Ogg,
    /// Matroska/WebM uses native tags.
    Matroska,
    /// `WebM` (subset of Matroska).
    WebM,
}

impl From<ContainerFormat> for MetadataFormat {
    fn from(format: ContainerFormat) -> Self {
        match format {
            ContainerFormat::Flac => Self::Flac,
            ContainerFormat::Ogg => Self::Ogg,
            ContainerFormat::WebM => Self::WebM,
            _ => Self::Matroska, // Default fallback (includes Matroska)
        }
    }
}

/// A metadata editor for media files.
///
/// Provides high-level operations for reading and writing metadata tags.
///
/// # Example
///
/// ```ignore
/// use oximedia_container::metadata::MetadataEditor;
///
/// let mut editor = MetadataEditor::open("audio.flac").await?;
///
/// // Read existing tags
/// if let Some(title) = editor.get_text("TITLE") {
///     println!("Current title: {}", title);
/// }
///
/// // Modify tags
/// editor.set("TITLE", "New Title");
/// editor.set("ARTIST", "New Artist");
/// editor.remove("COMMENT");
///
/// // Save changes
/// editor.save().await?;
/// ```
pub struct MetadataEditor {
    /// Path to the media file.
    path: PathBuf,
    /// Detected metadata format.
    format: MetadataFormat,
    /// Current tag map.
    tags: TagMap,
    /// Whether tags have been modified.
    modified: bool,
}

impl MetadataEditor {
    /// Opens a media file for metadata editing.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be opened
    /// - The format is not supported
    /// - Reading metadata fails
    pub async fn open(path: impl AsRef<Path>) -> OxiResult<Self> {
        let path = path.as_ref().to_path_buf();

        // Detect format
        let mut magic = [0u8; 8];
        let mut source_clone = FileSource::open(&path).await?;
        source_clone.read_exact(&mut magic).await?;

        let container_format = detect_format(&magic)?;
        let format = MetadataFormat::from(container_format);

        // Read metadata based on format
        let tags = match format {
            MetadataFormat::Flac => {
                let source = FileSource::open(&path).await?;
                FlacMetadataReader::read(source).await?
            }
            MetadataFormat::Ogg => {
                // For Ogg, we would need to use OggDemuxer
                // This is a simplified placeholder
                TagMap::new()
            }
            MetadataFormat::Matroska | MetadataFormat::WebM => {
                let source = FileSource::open(&path).await?;
                let mut demuxer = MatroskaDemuxer::new(source);
                demuxer.probe().await?;

                let tags = demuxer.tags();
                MatroskaMetadataReader::convert_tags(tags)
            }
        };

        Ok(Self {
            path,
            format,
            tags,
            modified: false,
        })
    }

    /// Returns the metadata format of the file.
    #[must_use]
    pub const fn format(&self) -> MetadataFormat {
        self.format
    }

    /// Returns true if tags have been modified.
    #[must_use]
    pub const fn is_modified(&self) -> bool {
        self.modified
    }

    /// Gets a tag value by key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&TagValue> {
        self.tags.get(key)
    }

    /// Gets a text tag value by key.
    #[must_use]
    pub fn get_text(&self, key: &str) -> Option<&str> {
        self.tags.get_text(key)
    }

    /// Gets all values for a tag key.
    #[must_use]
    pub fn get_all(&self, key: &str) -> &[TagValue] {
        self.tags.get_all(key)
    }

    /// Gets a standard tag value.
    #[must_use]
    pub fn get_standard(&self, tag: StandardTag) -> Option<&TagValue> {
        self.tags.get_standard(tag)
    }

    /// Sets a tag value, replacing any existing values.
    pub fn set(&mut self, key: impl AsRef<str>, value: impl Into<TagValue>) {
        self.tags.set(key, value);
        self.modified = true;
    }

    /// Adds a tag value without removing existing values.
    pub fn add(&mut self, key: impl AsRef<str>, value: impl Into<TagValue>) {
        self.tags.add(key, value);
        self.modified = true;
    }

    /// Sets a standard tag value.
    pub fn set_standard(&mut self, tag: StandardTag, value: impl Into<TagValue>) {
        self.tags.set_standard(tag, value);
        self.modified = true;
    }

    /// Removes a tag and all its values.
    ///
    /// Returns true if the tag existed.
    pub fn remove(&mut self, key: &str) -> bool {
        let removed = self.tags.remove(key);
        if removed {
            self.modified = true;
        }
        removed
    }

    /// Clears all tags.
    pub fn clear(&mut self) {
        if !self.tags.is_empty() {
            self.tags.clear();
            self.modified = true;
        }
    }

    /// Returns an iterator over all tag keys.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.tags.keys()
    }

    /// Returns an iterator over all tag entries.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &TagValue)> {
        self.tags.iter()
    }

    /// Returns a reference to the tag map.
    #[must_use]
    pub const fn tags(&self) -> &TagMap {
        &self.tags
    }

    /// Returns a mutable reference to the tag map.
    pub fn tags_mut(&mut self) -> &mut TagMap {
        self.modified = true;
        &mut self.tags
    }

    /// Saves metadata changes to the file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be opened for writing
    /// - Writing fails
    /// - The format doesn't support metadata writing
    pub async fn save(&mut self) -> OxiResult<()> {
        if !self.modified {
            return Ok(());
        }

        let mut source = FileSource::open(&self.path).await?;

        match self.format {
            MetadataFormat::Flac => {
                FlacMetadataWriter::write(&mut source, &self.tags).await?;
            }
            MetadataFormat::Ogg => {
                OggMetadataWriter::write(&mut source, &self.tags).await?;
            }
            MetadataFormat::Matroska | MetadataFormat::WebM => {
                MatroskaMetadataWriter::write(&mut source, &self.tags).await?;
            }
        }

        self.modified = false;
        Ok(())
    }

    /// Discards any unsaved changes and reloads metadata from the file.
    ///
    /// # Errors
    ///
    /// Returns an error if reading fails.
    pub async fn reload(&mut self) -> OxiResult<()> {
        let new_editor = Self::open(&self.path).await?;
        self.tags = new_editor.tags;
        self.modified = false;
        Ok(())
    }
}

/// Reads metadata from a file without creating an editor.
///
/// This is a convenience function for read-only metadata access.
///
/// # Errors
///
/// Returns an error if reading or parsing fails.
pub async fn read_metadata(path: impl AsRef<Path>) -> OxiResult<TagMap> {
    let editor = MetadataEditor::open(path).await?;
    Ok(editor.tags)
}

/// Writes metadata to a file.
///
/// This is a convenience function for updating metadata without
/// reading existing tags first.
///
/// # Errors
///
/// Returns an error if writing fails.
pub async fn write_metadata(path: impl AsRef<Path>, tags: &TagMap) -> OxiResult<()> {
    let mut editor = MetadataEditor::open(path).await?;
    editor.tags = tags.clone();
    editor.modified = true;
    editor.save().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_format_from_container_format() {
        assert_eq!(
            MetadataFormat::from(ContainerFormat::Flac),
            MetadataFormat::Flac
        );
        assert_eq!(
            MetadataFormat::from(ContainerFormat::Ogg),
            MetadataFormat::Ogg
        );
        assert_eq!(
            MetadataFormat::from(ContainerFormat::Matroska),
            MetadataFormat::Matroska
        );
        assert_eq!(
            MetadataFormat::from(ContainerFormat::WebM),
            MetadataFormat::WebM
        );
    }

    #[test]
    fn test_metadata_editor_modification_tracking() {
        let editor = MetadataEditor {
            path: PathBuf::from("test.flac"),
            format: MetadataFormat::Flac,
            tags: TagMap::new(),
            modified: false,
        };

        assert!(!editor.is_modified());
    }

    #[test]
    fn test_metadata_editor_set() {
        let mut editor = MetadataEditor {
            path: PathBuf::from("test.flac"),
            format: MetadataFormat::Flac,
            tags: TagMap::new(),
            modified: false,
        };

        editor.set("TITLE", "Test");
        assert!(editor.is_modified());
        assert_eq!(editor.get_text("TITLE"), Some("Test"));
    }

    #[test]
    fn test_metadata_editor_add() {
        let mut editor = MetadataEditor {
            path: PathBuf::from("test.flac"),
            format: MetadataFormat::Flac,
            tags: TagMap::new(),
            modified: false,
        };

        editor.add("ARTIST", "Artist 1");
        editor.add("ARTIST", "Artist 2");

        assert!(editor.is_modified());
        let artists = editor.get_all("ARTIST");
        assert_eq!(artists.len(), 2);
    }

    #[test]
    fn test_metadata_editor_remove() {
        let mut editor = MetadataEditor {
            path: PathBuf::from("test.flac"),
            format: MetadataFormat::Flac,
            tags: TagMap::new(),
            modified: false,
        };

        editor.set("TITLE", "Test");
        editor.modified = false; // Reset flag

        assert!(editor.remove("TITLE"));
        assert!(editor.is_modified());
        assert!(!editor.remove("TITLE"));
    }

    #[test]
    fn test_metadata_editor_clear() {
        let mut editor = MetadataEditor {
            path: PathBuf::from("test.flac"),
            format: MetadataFormat::Flac,
            tags: TagMap::new(),
            modified: false,
        };

        editor.set("TITLE", "Test");
        editor.set("ARTIST", "Test");
        editor.modified = false;

        editor.clear();
        assert!(editor.is_modified());
        assert!(editor.tags.is_empty());
    }

    #[test]
    fn test_metadata_editor_standard_tags() {
        let mut editor = MetadataEditor {
            path: PathBuf::from("test.flac"),
            format: MetadataFormat::Flac,
            tags: TagMap::new(),
            modified: false,
        };

        editor.set_standard(StandardTag::Title, "Test Title");
        assert_eq!(
            editor
                .get_standard(StandardTag::Title)
                .and_then(|v| v.as_text()),
            Some("Test Title")
        );
    }

    #[test]
    fn test_metadata_editor_iter() {
        let mut editor = MetadataEditor {
            path: PathBuf::from("test.flac"),
            format: MetadataFormat::Flac,
            tags: TagMap::new(),
            modified: false,
        };

        editor.set("TITLE", "Title");
        editor.set("ARTIST", "Artist");

        let entries: Vec<_> = editor.iter().collect();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_metadata_editor_keys() {
        let mut editor = MetadataEditor {
            path: PathBuf::from("test.flac"),
            format: MetadataFormat::Flac,
            tags: TagMap::new(),
            modified: false,
        };

        editor.set("TITLE", "Title");
        editor.set("ARTIST", "Artist");

        let keys: Vec<_> = editor.keys().collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"TITLE"));
        assert!(keys.contains(&"ARTIST"));
    }
}
