//! DASH segment writer for CMAF/fMP4 segments.

use crate::error::ServerResult;
use oximedia_net::rtmp::MediaPacket;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Initialization segment.
pub struct InitSegment {
    /// Segment data.
    pub data: Vec<u8>,
}

/// Media segment.
pub struct MediaSegment {
    /// Segment number.
    pub number: u64,

    /// Duration.
    pub duration: f64,

    /// Segment data.
    pub data: Vec<u8>,
}

/// DASH segment writer.
pub struct DashSegmentWriter {
    /// Output directory.
    output_dir: PathBuf,
}

impl DashSegmentWriter {
    /// Creates a new segment writer.
    ///
    /// # Errors
    ///
    /// Returns an error if directory creation fails.
    pub fn new(output_dir: impl AsRef<Path>) -> ServerResult<Self> {
        let output_dir = output_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&output_dir)?;

        Ok(Self { output_dir })
    }

    /// Writes an initialization segment.
    pub async fn write_init_segment(&self, filename: &str) -> ServerResult<()> {
        let path = self.output_dir.join(filename);

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Generate initialization segment
        // In a real implementation, this would create a proper fMP4 init segment
        let init_data = self.create_init_segment()?;

        let mut file = fs::File::create(&path).await?;
        file.write_all(&init_data.data).await?;
        file.flush().await?;

        Ok(())
    }

    /// Writes a media segment.
    pub async fn write_media_segment(
        &self,
        filename: &str,
        packets: &[MediaPacket],
    ) -> ServerResult<()> {
        let path = self.output_dir.join(filename);

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // For now, just concatenate packet data
        // In a real implementation, this would properly mux into fMP4 format
        let mut data = Vec::new();
        for packet in packets {
            data.extend_from_slice(&packet.data);
        }

        let mut file = fs::File::create(&path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        Ok(())
    }

    /// Creates an initialization segment.
    fn create_init_segment(&self) -> ServerResult<InitSegment> {
        // In a real implementation, this would create a proper fMP4 initialization segment
        // with ftyp, moov boxes, etc.
        Ok(InitSegment { data: Vec::new() })
    }

    /// Creates a media segment from packets.
    #[allow(dead_code)]
    fn create_media_segment(
        &self,
        number: u64,
        packets: &[MediaPacket],
    ) -> ServerResult<MediaSegment> {
        // In a real implementation, this would properly mux packets into fMP4
        let mut data = Vec::new();
        let duration = 0.0;

        for packet in packets {
            data.extend_from_slice(&packet.data);
            // Calculate duration from timestamps
        }

        Ok(MediaSegment {
            number,
            duration,
            data,
        })
    }

    /// Deletes old segments.
    pub async fn cleanup_old_segments(&self, keep_count: usize) -> ServerResult<()> {
        let mut entries = fs::read_dir(&self.output_dir).await?;
        let mut files = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension().and_then(|s| s.to_str()) == Some("m4s") {
                files.push(entry.path());
            }
        }

        // Sort by modification time
        files.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());

        // Delete oldest files if we have too many
        if files.len() > keep_count {
            for file in &files[..files.len() - keep_count] {
                fs::remove_file(file).await?;
            }
        }

        Ok(())
    }
}
