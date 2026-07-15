//! HLS segment writer for MPEG-TS segments.

use crate::error::{ServerError, ServerResult};
use oximedia_net::rtmp::MediaPacket;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Honest error returned by the segment-muxing methods.
///
/// Producing a standards-compliant MPEG-TS segment from RTMP `MediaPacket`s
/// requires (a) depacketizing the FLV tag bodies into elementary streams and
/// (b) feeding them to a real muxer. `oximedia_container::mux::MpegTsMuxer`
/// exists but only supports patent-free codecs (AV1/VP9/VP8/Opus/FLAC/PCM),
/// whereas typical RTMP ingest carries H.264/AAC, and no codec metadata is
/// available at this layer. Rather than write a byte-concatenation that
/// merely *looks* like a `.ts` file, these methods fail honestly.
// TODO(0.2.x): wire real MPEG-TS muxing — parse FLV codec headers, convert
// AVCC/HEVC NALUs to Annex-B (or route AV1/VP9/Opus/FLAC straight through),
// build `oximedia_container::StreamInfo` + `Packet`s with correct PTS/DTS, and
// mux via `oximedia_container::mux::MpegTsMuxer` into an in-memory sink. For
// H.264/AAC ingest this additionally requires transcoding to a patent-free
// codec first (see the ingest transcode pipeline).
fn ts_mux_unimplemented() -> ServerError {
    ServerError::Internal(
        "MPEG-TS segment muxing is not implemented: refusing to write a \
         non-compliant concatenated segment (see TODO(0.2.x) in hls/segment.rs)"
            .to_string(),
    )
}

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

    /// Writes an MPEG-TS segment for the given packets.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::Internal`] because standards-compliant MPEG-TS
    /// muxing of RTMP `MediaPacket`s is not yet implemented (see
    /// [`ts_mux_unimplemented`]). This method deliberately does **not** write
    /// a byte-concatenated file that would masquerade as a valid `.ts`
    /// segment; callers are expected to degrade honestly.
    pub async fn write_segment(
        &self,
        _filename: &str,
        _packets: &[MediaPacket],
    ) -> ServerResult<()> {
        Err(ts_mux_unimplemented())
    }

    /// Creates an in-memory MPEG-TS segment from packets.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::Internal`] — real MPEG-TS muxing is not
    /// implemented (see [`ts_mux_unimplemented`]).
    #[allow(dead_code)]
    pub fn create_segment(
        &self,
        _filename: &str,
        _packets: &[MediaPacket],
    ) -> ServerResult<TsSegment> {
        Err(ts_mux_unimplemented())
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

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use oximedia_net::rtmp::{MediaPacket, MediaPacketType};

    fn tmp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("oximedia-hls-seg-{name}-{}", std::process::id()))
    }

    fn sample_packets() -> Vec<MediaPacket> {
        vec![MediaPacket {
            packet_type: MediaPacketType::Video,
            timestamp: 0,
            stream_id: 1,
            data: Bytes::from_static(&[0u8; 64]),
        }]
    }

    #[tokio::test]
    async fn write_segment_returns_honest_err_not_fake_ts() {
        let dir = tmp_dir("write");
        let writer = SegmentWriter::new(&dir).expect("create writer");
        let filename = "segment0.ts";

        let result = writer.write_segment(filename, &sample_packets()).await;
        assert!(
            result.is_err(),
            "write_segment must not fabricate a concatenated .ts segment"
        );

        // Crucially, no fake segment file may be left behind.
        let seg_path = dir.join(filename);
        assert!(
            !seg_path.exists(),
            "no fabricated segment file should be written on the honest-error path"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_segment_returns_honest_err() {
        let dir = tmp_dir("create");
        let writer = SegmentWriter::new(&dir).expect("create writer");
        let result = writer.create_segment("segment0.ts", &sample_packets());
        assert!(
            result.is_err(),
            "create_segment must not fabricate an in-memory .ts segment"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
