//! Professional clip management and logging system for `OxiMedia`.
//!
//! This crate provides comprehensive clip management functionality for professional
//! video editing and logging workflows, including:
//!
//! - **Clip Database**: Store and manage video clips with metadata
//! - **Subclip Creation**: Create subclips with in/out points
//! - **Clip Grouping**: Organize clips into bins, folders, and collections
//! - **Professional Logging**: Keywords, markers, ratings, and notes
//! - **Take Management**: Track multiple takes of the same shot
//! - **Proxy Association**: Link clips to proxy versions
//! - **Smart Collections**: Auto-updating collections based on criteria
//! - **Search and Filter**: Advanced search and filtering capabilities
//! - **Import/Export**: EDL, XML, CSV, JSON export
//!
//! # Example
//!
//! ```rust
//! use oximedia_clips::{ClipManager, Clip, Rating};
//! use oximedia_core::types::Rational;
//! use std::path::PathBuf;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a clip manager
//! let manager = ClipManager::new(":memory:").await?;
//!
//! // Create a new clip
//! let mut clip = Clip::new(PathBuf::from("/path/to/video.mov"));
//! clip.set_name("Interview Take 1");
//! clip.set_rating(Rating::FourStars);
//! clip.add_keyword("interview");
//! clip.add_keyword("john-doe");
//!
//! // Save the clip
//! let clip_id = manager.add_clip(clip).await?;
//!
//! // Search for clips
//! let results = manager.search("interview").await?;
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]
#![allow(clippy::format_push_string)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_lossless)]

pub mod bin_organizer;
pub mod clip;
pub mod clip_audit;
pub mod clip_bin;
pub mod clip_compare;
pub mod clip_export;
pub mod clip_history;
pub mod clip_metadata;
pub mod clip_relations;
pub mod clip_search;
pub mod clip_tag;
pub mod clip_timeline;
#[cfg(not(target_arch = "wasm32"))]
pub mod database;
pub mod export;
pub mod group;
#[cfg(not(target_arch = "wasm32"))]
pub mod import;
pub mod logging;
pub mod marker;
pub mod note;
pub mod proxy;
pub mod proxy_link;
pub mod rating;
pub mod search;
pub mod storyboard;
pub mod subclip;
pub mod sync;
pub mod take;
pub mod trim;
pub mod version;

mod error;
#[cfg(not(target_arch = "wasm32"))]
mod manager;

pub use clip::{Clip, ClipId, ClipMetadata, SubClip};
pub use error::{ClipError, ClipResult};
pub use group::{Bin, BinId, Collection, CollectionId, Folder, FolderId, SmartCollection};
pub use logging::{Favorite, Keyword, Rating};
#[cfg(not(target_arch = "wasm32"))]
pub use manager::ClipManager;
pub use marker::{Marker, MarkerId, MarkerType};
pub use note::{Annotation, Note, NoteId};
pub use proxy::{ProxyLink, ProxyQuality};
pub use take::{Take, TakeId, TakeSelector};
