//! Professional Archive and Digital Preservation System for `OxiMedia`
//!
//! `oximedia-archive-pro` provides comprehensive tools for long-term digital preservation
//! of media files, including:
//!
//! - **Preservation Packaging**: `BagIt`, OAIS (SIP/AIP/DIP), TAR, and ZIP
//! - **Format Migration**: Planning, execution, and validation of format migrations
//! - **Checksum Management**: Multi-algorithm checksums (MD5, SHA-256, SHA-512, xxHash, BLAKE3)
//! - **Metadata Preservation**: PREMIS, METS, Dublin Core metadata
//! - **Version Control**: Track versions and changes over time
//! - **Fixity Checking**: Periodic integrity verification
//! - **Risk Assessment**: Format obsolescence monitoring
//! - **Emulation Support**: Prepare for future emulation needs
//! - **Documentation**: Auto-generate preservation documentation
//!
//! # Example
//!
//! ```rust,no_run
//! use oximedia_archive_pro::{
//!     package::bagit::BagItBuilder,
//!     checksum::{ChecksumAlgorithm, ChecksumGenerator},
//!     metadata::premis::PremisMetadata,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a BagIt package
//! let bag = BagItBuilder::new(std::path::PathBuf::from("/path/to/bag"))
//!     .with_algorithm(ChecksumAlgorithm::Sha256)
//!     .with_metadata("Contact-Name", "Archivist")
//!     .add_file(std::path::Path::new("/path/to/media.mkv"))?
//!     .build()?;
//!
//! // Generate preservation metadata
//! let premis = PremisMetadata::for_file(std::path::Path::new("/path/to/media.mkv"))?;
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::too_many_arguments)]

pub mod access_policy;
pub mod archive_report;
pub mod archive_stats;
pub mod audit_trail;
pub mod bit_rot_detection;
pub mod checksum;
pub mod cold_storage;
pub mod deaccession;
pub mod deep_archive;
pub mod disaster_recovery;
pub mod docs;
pub mod emulation;
pub mod emulation_planning;
pub mod fixity;
pub mod format_migration;
pub mod format_registry;
pub mod format_validator;
pub mod ingest;
pub mod integrity_check;
pub mod metadata;
pub mod metadata_crosswalk;
pub mod migrate;
pub mod migration_plan;
pub mod oais_model;
pub mod package;
pub mod policy;
pub mod provenance_chain;
pub mod replication_verify;
pub mod restore_workflow;
pub mod retention;
pub mod risk;
pub mod storage_quota;
pub mod version;
pub mod workflow_state;

use thiserror::Error;

/// Result type for archive preservation operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for archive preservation operations
#[derive(Error, Debug)]
pub enum Error {
    /// I/O error occurred
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Checksum verification failed
    #[error("Checksum verification failed: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// Expected checksum value
        expected: String,
        /// Actual checksum value
        actual: String,
    },

    /// Invalid `BagIt` package
    #[error("Invalid BagIt package: {0}")]
    InvalidBag(String),

    /// Invalid OAIS package
    #[error("Invalid OAIS package: {0}")]
    InvalidOais(String),

    /// Format migration error
    #[error("Format migration error: {0}")]
    Migration(String),

    /// Metadata error
    #[error("Metadata error: {0}")]
    Metadata(String),

    /// XML parsing/serialization error
    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),

    /// Archive creation error
    #[error("Archive error: {0}")]
    Archive(String),

    /// Policy violation
    #[error("Policy violation: {0}")]
    PolicyViolation(String),

    /// Format at risk
    #[error("Format obsolescence risk: {0}")]
    FormatRisk(String),

    /// Unsupported operation
    #[error("Unsupported operation: {0}")]
    Unsupported(String),
}

/// Preservation format recommendations
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PreservationFormat {
    /// Video: FFV1 in Matroska
    VideoFfv1Mkv,
    /// Video: UT Video in AVI
    VideoUtVideo,
    /// Audio: FLAC
    AudioFlac,
    /// Audio: WAV PCM
    AudioWav,
    /// Image: TIFF (uncompressed)
    ImageTiff,
    /// Image: PNG
    ImagePng,
    /// Image: JPEG 2000 (lossless)
    ImageJpeg2000,
    /// Document: PDF/A
    DocumentPdfA,
    /// Document: Plain text (UTF-8)
    DocumentText,
}

impl PreservationFormat {
    /// Returns the recommended file extension
    #[must_use]
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::VideoFfv1Mkv => "mkv",
            Self::VideoUtVideo => "avi",
            Self::AudioFlac => "flac",
            Self::AudioWav => "wav",
            Self::ImageTiff => "tiff",
            Self::ImagePng => "png",
            Self::ImageJpeg2000 => "jp2",
            Self::DocumentPdfA => "pdf",
            Self::DocumentText => "txt",
        }
    }

    /// Returns the MIME type
    #[must_use]
    pub const fn mime_type(&self) -> &'static str {
        match self {
            Self::VideoFfv1Mkv => "video/x-matroska",
            Self::VideoUtVideo => "video/x-msvideo",
            Self::AudioFlac => "audio/flac",
            Self::AudioWav => "audio/wav",
            Self::ImageTiff => "image/tiff",
            Self::ImagePng => "image/png",
            Self::ImageJpeg2000 => "image/jp2",
            Self::DocumentPdfA => "application/pdf",
            Self::DocumentText => "text/plain",
        }
    }

    /// Returns a description of the format
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::VideoFfv1Mkv => "FFV1 lossless video in Matroska container",
            Self::VideoUtVideo => "UT Video lossless codec in AVI container",
            Self::AudioFlac => "Free Lossless Audio Codec",
            Self::AudioWav => "WAV PCM uncompressed audio",
            Self::ImageTiff => "TIFF uncompressed image",
            Self::ImagePng => "PNG lossless image",
            Self::ImageJpeg2000 => "JPEG 2000 lossless image",
            Self::DocumentPdfA => "PDF/A archival document format",
            Self::DocumentText => "Plain text UTF-8",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preservation_format_extensions() {
        assert_eq!(PreservationFormat::VideoFfv1Mkv.extension(), "mkv");
        assert_eq!(PreservationFormat::AudioFlac.extension(), "flac");
        assert_eq!(PreservationFormat::ImageTiff.extension(), "tiff");
    }

    #[test]
    fn test_preservation_format_mime_types() {
        assert_eq!(
            PreservationFormat::VideoFfv1Mkv.mime_type(),
            "video/x-matroska"
        );
        assert_eq!(PreservationFormat::AudioFlac.mime_type(), "audio/flac");
    }
}
