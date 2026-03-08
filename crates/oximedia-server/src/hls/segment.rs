//! HLS segment writer for MPEG-TS segments.

use crate::error::ServerResult;
use oximedia_net::rtmp::MediaPacket;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// MPEG-TS segment.
pub struct TsSegment {
    /// Segment filename.
    pub filename: String,

    /// Segment duration.
    pub duration: f64,

    /// Segment data.
    pub data: Vec<u8>,
}

/// Segment writer.
pub struct SegmentWriter {
    /// Output directory.
    output_dir: PathBuf,
}

impl SegmentWriter {
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

    /// Writes a segment.
    pub async fn write_segment(&self, filename: &str, packets: &[MediaPacket]) -> ServerResult<()> {
        let path = self.output_dir.join(filename);

        // Create parent directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // For now, just concatenate packet data
        // In a real implementation, this would properly mux into MPEG-TS format
        let mut data = Vec::new();
        for packet in packets {
            data.extend_from_slice(&packet.data);
        }

        let mut file = fs::File::create(&path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        Ok(())
    }

    /// Creates a MPEG-TS segment from packets.
    ///
    /// # Errors
    ///
    /// Returns an error if segment creation fails.
    #[allow(dead_code)]
    pub fn create_segment(
        &self,
        filename: &str,
        packets: &[MediaPacket],
    ) -> ServerResult<TsSegment> {
        // In a real implementation, this would properly mux packets into MPEG-TS
        let mut data = Vec::new();
        let duration = 0.0;

        for packet in packets {
            data.extend_from_slice(&packet.data);
            // Calculate duration from timestamps
        }

        Ok(TsSegment {
            filename: filename.to_string(),
            duration,
            data,
        })
    }

    /// Deletes old segments.
    pub async fn cleanup_old_segments(&self, keep_count: usize) -> ServerResult<()> {
        let mut entries = fs::read_dir(&self.output_dir).await?;
        let mut files = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension().and_then(|s| s.to_str()) == Some("ts") {
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
