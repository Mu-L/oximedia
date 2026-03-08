//! Matching strategies for conforming media files to clips.

pub mod content;
pub mod filename;
pub mod strategies;
pub mod timecode;

pub use strategies::MatchStrategy;
