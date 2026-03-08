//! `OxiMedia` AAF - Advanced Authoring Format support
//!
//! This crate provides SMPTE ST 377-1 compliant AAF (Advanced Authoring Format)
//! reading and writing for professional post-production workflows.
//!
//! # Features
//!
//! - Full SMPTE ST 377-1 (AAF Object Specification) support
//! - SMPTE ST 2001 (AAF Operational Patterns) support
//! - Microsoft Structured Storage (compound file) parsing
//! - Complete object model (Mobs, Segments, Components, Effects)
//! - Dictionary support with extensibility
//! - Essence reference handling (embedded and external)
//! - Timeline and edit rate management
//! - Metadata preservation
//! - Conversion to `OpenTimelineIO` and EDL formats
//! - Read and write capability
//! - No unsafe code
//!
//! # AAF Structure
//!
//! AAF files use Microsoft Structured Storage format (compound files) and contain:
//! - Header: File identification and version
//! - Dictionary: Class, property, and type definitions
//! - Content Storage: Mobs (Master, Source, Composition)
//! - Essence: Media data (embedded or external references)
//!
//! # Example
//!
//! ```rust,no_run
//! use oximedia_aaf::{AafFile, AafReader};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Open an AAF file
//! let mut reader = AafReader::open("timeline.aaf")?;
//! let aaf = reader.read()?;
//!
//! // Access composition mobs
//! for comp_mob in aaf.composition_mobs() {
//!     println!("Composition: {}", comp_mob.name());
//!     for track in comp_mob.tracks() {
//!         println!("  Track: {}", track.name);
//!     }
//! }
//! # Ok(())
//! # }
//! ```

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::cast_possible_wrap,
    dead_code,
    clippy::pedantic
)]

pub mod aaf_export;
pub mod composition;
pub mod composition_mob;
pub mod convert;
pub mod descriptor;
pub mod dictionary;
pub mod effect_def;
pub mod effects;
pub mod essence;
pub mod interchange;
pub mod media_data;
pub mod media_file_ref;
pub mod metadata;
pub mod mob_slot;
pub mod object_model;
pub mod operation_group;
pub mod parameter;
pub mod property_value;
pub mod scope;
pub mod selector;
pub mod source_clip;
pub mod structured_storage;
pub mod timeline;
pub mod timeline_export;
pub mod timeline_mob;
pub mod track_group;
pub mod transition_def;
pub mod writer;

use std::collections::HashMap;
use std::io::{Read, Seek};
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

pub use composition::{
    CompositionMob, Effect, EffectParameter, FadeType, Filler, Sequence, SequenceComponent,
    SourceClip, Track, TrackType, Transition, UsageCode,
};
pub use convert::{
    EdlExporter, Timeline, TimelineClip, TimelineConverter, TimelineTrack, XmlExporter,
};
pub use dictionary::{Auid, DataDefinition, Dictionary};
pub use essence::{EssenceAccess, EssenceDescriptor, EssenceReference};
pub use metadata::{Comment, KlvData, TaggedValue, Timecode as AafTimecode};
pub use object_model::{Component, Header, Mob, MobSlot, Segment};
pub use structured_storage::{StorageReader, StorageWriter};
pub use timeline::{EditRate, Position};
pub use writer::AafWriter;

/// AAF error types
#[derive(Error, Debug)]
pub enum AafError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid AAF file: {0}")]
    InvalidFile(String),

    #[error("Unsupported AAF version: {0}.{1}")]
    UnsupportedVersion(u16, u16),

    #[error("Invalid structured storage: {0}")]
    InvalidStructuredStorage(String),

    #[error("Object not found: {0}")]
    ObjectNotFound(String),

    #[error("Property not found: {0}")]
    PropertyNotFound(String),

    #[error("Invalid class: {0}")]
    InvalidClass(String),

    #[error("Invalid property type: {0}")]
    InvalidPropertyType(String),

    #[error("Reference resolution failed: {0}")]
    ReferenceResolutionFailed(String),

    #[error("Dictionary error: {0}")]
    DictionaryError(String),

    #[error("Essence error: {0}")]
    EssenceError(String),

    #[error("Timeline error: {0}")]
    TimelineError(String),

    #[error("Conversion error: {0}")]
    ConversionError(String),

    #[error("Write error: {0}")]
    WriteError(String),

    #[error("Parse error: {0}")]
    ParseError(String),
}

pub type Result<T> = std::result::Result<T, AafError>;

/// Main AAF file structure
///
/// Represents a complete AAF file with header, dictionary, and content.
#[derive(Debug, Clone)]
pub struct AafFile {
    header: Header,
    dictionary: Dictionary,
    content_storage: ContentStorage,
    essence_data: Vec<EssenceData>,
}

impl AafFile {
    /// Create a new empty AAF file
    #[must_use]
    pub fn new() -> Self {
        Self {
            header: Header::new(),
            dictionary: Dictionary::new(),
            content_storage: ContentStorage::new(),
            essence_data: Vec::new(),
        }
    }

    /// Get the file header
    #[must_use]
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Get the dictionary
    #[must_use]
    pub fn dictionary(&self) -> &Dictionary {
        &self.dictionary
    }

    /// Get the content storage
    #[must_use]
    pub fn content_storage(&self) -> &ContentStorage {
        &self.content_storage
    }

    /// Get all composition mobs
    #[must_use]
    pub fn composition_mobs(&self) -> Vec<&CompositionMob> {
        self.content_storage.composition_mobs()
    }

    /// Get all master mobs
    #[must_use]
    pub fn master_mobs(&self) -> Vec<&Mob> {
        self.content_storage.master_mobs()
    }

    /// Get all source mobs
    #[must_use]
    pub fn source_mobs(&self) -> Vec<&Mob> {
        self.content_storage.source_mobs()
    }

    /// Find a mob by ID
    #[must_use]
    pub fn find_mob(&self, mob_id: &Uuid) -> Option<&Mob> {
        self.content_storage.find_mob(mob_id)
    }

    /// Get all essence data
    #[must_use]
    pub fn essence_data(&self) -> &[EssenceData] {
        &self.essence_data
    }

    /// Get the file's edit rate (from first composition mob)
    #[must_use]
    pub fn edit_rate(&self) -> Option<EditRate> {
        self.composition_mobs()
            .first()
            .and_then(|mob| mob.edit_rate())
    }

    /// Get file duration in edit units
    #[must_use]
    pub fn duration(&self) -> Option<i64> {
        self.composition_mobs()
            .first()
            .and_then(|mob| mob.duration())
    }
}

impl Default for AafFile {
    fn default() -> Self {
        Self::new()
    }
}

/// Content storage containing all mobs
#[derive(Debug, Clone)]
pub struct ContentStorage {
    mobs: HashMap<Uuid, Mob>,
    composition_mobs: HashMap<Uuid, CompositionMob>,
}

impl ContentStorage {
    /// Create new empty content storage
    #[must_use]
    pub fn new() -> Self {
        Self {
            mobs: HashMap::new(),
            composition_mobs: HashMap::new(),
        }
    }

    /// Add a mob
    pub fn add_mob(&mut self, mob: Mob) {
        let id = mob.mob_id();
        self.mobs.insert(id, mob);
    }

    /// Add a composition mob
    pub fn add_composition_mob(&mut self, comp_mob: CompositionMob) {
        let id = comp_mob.mob_id();
        self.composition_mobs.insert(id, comp_mob);
    }

    /// Get all composition mobs
    #[must_use]
    pub fn composition_mobs(&self) -> Vec<&CompositionMob> {
        self.composition_mobs.values().collect()
    }

    /// Get all master mobs
    #[must_use]
    pub fn master_mobs(&self) -> Vec<&Mob> {
        self.mobs.values().filter(|m| m.is_master_mob()).collect()
    }

    /// Get all source mobs
    #[must_use]
    pub fn source_mobs(&self) -> Vec<&Mob> {
        self.mobs.values().filter(|m| m.is_source_mob()).collect()
    }

    /// Find a mob by ID
    #[must_use]
    pub fn find_mob(&self, mob_id: &Uuid) -> Option<&Mob> {
        self.mobs.get(mob_id)
    }

    /// Find a composition mob by ID
    #[must_use]
    pub fn find_composition_mob(&self, mob_id: &Uuid) -> Option<&CompositionMob> {
        self.composition_mobs.get(mob_id)
    }
}

impl Default for ContentStorage {
    fn default() -> Self {
        Self::new()
    }
}

/// Essence data stored in the AAF file
#[derive(Debug, Clone)]
pub struct EssenceData {
    mob_id: Uuid,
    data: Vec<u8>,
}

impl EssenceData {
    /// Create new essence data
    #[must_use]
    pub fn new(mob_id: Uuid, data: Vec<u8>) -> Self {
        Self { mob_id, data }
    }

    /// Get the mob ID
    #[must_use]
    pub fn mob_id(&self) -> Uuid {
        self.mob_id
    }

    /// Get the essence data
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

/// AAF file reader
pub struct AafReader<R: Read + Seek> {
    storage: StorageReader<R>,
}

impl<R: Read + Seek> AafReader<R> {
    /// Create a new AAF reader from a readable source
    pub fn new(reader: R) -> Result<Self> {
        let storage = StorageReader::new(reader)?;
        Ok(Self { storage })
    }

    /// Read the complete AAF file
    pub fn read(&mut self) -> Result<AafFile> {
        // Read header from root entry
        let header = self.read_header()?;

        // Read dictionary
        let dictionary = self.read_dictionary()?;

        // Read content storage
        let content_storage = self.read_content_storage(&dictionary)?;

        // Read essence data
        let essence_data = self.read_essence_data()?;

        Ok(AafFile {
            header,
            dictionary,
            content_storage,
            essence_data,
        })
    }

    fn read_header(&mut self) -> Result<Header> {
        // Implementation in object_model module
        object_model::read_header(&mut self.storage)
    }

    fn read_dictionary(&mut self) -> Result<Dictionary> {
        // Implementation in dictionary module
        dictionary::read_dictionary(&mut self.storage)
    }

    fn read_content_storage(&mut self, dictionary: &Dictionary) -> Result<ContentStorage> {
        // Implementation in object_model module
        object_model::read_content_storage(&mut self.storage, dictionary)
    }

    fn read_essence_data(&mut self) -> Result<Vec<EssenceData>> {
        // Implementation in essence module
        essence::read_essence_data(&mut self.storage)
    }
}

impl AafReader<std::fs::File> {
    /// Open an AAF file from a path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        Self::new(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aaf_file_creation() {
        let aaf = AafFile::new();
        assert!(aaf.composition_mobs().is_empty());
        assert!(aaf.master_mobs().is_empty());
        assert!(aaf.source_mobs().is_empty());
    }

    #[test]
    fn test_content_storage() {
        let storage = ContentStorage::new();
        assert!(storage.composition_mobs().is_empty());
        assert!(storage.master_mobs().is_empty());
    }

    #[test]
    fn test_essence_data() {
        let mob_id = Uuid::new_v4();
        let data = vec![1, 2, 3, 4];
        let essence = EssenceData::new(mob_id, data.clone());
        assert_eq!(essence.mob_id(), mob_id);
        assert_eq!(essence.data(), &data);
    }
}
