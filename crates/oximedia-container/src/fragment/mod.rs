//! Fragmented container support.
//!
//! Provides fragmented MP4 and segment writing for adaptive streaming.

#![forbid(unsafe_code)]

pub mod init;
pub mod mp4;
pub mod segment;

pub use init::InitSegmentBuilder;
pub use mp4::{
    FragmentType, FragmentedMp4Builder, FragmentedMp4Config, FragmentedTrack, Mp4Fragment,
};
pub use segment::{DashManifestGenerator, SegmentInfo, SegmentWriter, SegmentWriterConfig};
