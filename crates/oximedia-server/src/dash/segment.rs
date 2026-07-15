//! DASH segment writer for CMAF/fMP4 segments.

use crate::error::{ServerError, ServerResult};
use oximedia_net::rtmp::MediaPacket;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Honest error returned by the fMP4 segment-muxing methods.
///
/// A valid CMAF/fMP4 segment (or `moov`/`ftyp` init segment) cannot be built
/// by concatenating RTMP FLV payloads: it needs a real ISOBMFF writer fed with
/// depacketized elementary samples plus the codec's decoder-configuration
/// record. `oximedia_container::mux::mp4::Mp4Muxer` exists, but the RTMP
/// `MediaPacket`s arriving here are opaque FLV tag bodies with no codec
/// metadata. Rather than emit an empty `init.mp4` or a byte-concatenated
/// `.m4s` that only *looks* like a fragment, these methods fail honestly.
// TODO(0.2.x): wire real fMP4 muxing — depacketize FLV into elementary
// samples, derive the ISOBMFF sample-entry / decoder-config from the RTMP
// sequence headers, and produce the init + media fragments via
// `oximedia_container::mux::mp4::Mp4Muxer` (fragmented mode) / `build_sidx`.
fn fmp4_mux_unimplemented(kind: &str) -> ServerError {
    ServerError::Internal(format!(
        "fMP4 {kind} muxing is not implemented: refusing to write a \
         non-compliant segment (see TODO(0.2.x) in dash/segment.rs)"
    ))
}

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

    /// Writes an fMP4 initialization segment.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::Internal`] — a real `ftyp`/`moov` init segment
    /// cannot be produced here yet (see [`fmp4_mux_unimplemented`]). This
    /// method refuses to write an empty/placeholder `init.mp4` that would
    /// masquerade as a valid initialization segment.
    pub async fn write_init_segment(&self, _filename: &str) -> ServerResult<()> {
        Err(fmp4_mux_unimplemented("init"))
    }

    /// Writes an fMP4 media segment for the given packets.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::Internal`] — standards-compliant fMP4 muxing of
    /// RTMP `MediaPacket`s is not implemented (see
    /// [`fmp4_mux_unimplemented`]). This method deliberately does **not**
    /// write a byte-concatenated file that would masquerade as a `.m4s`
    /// fragment; callers are expected to degrade honestly.
    pub async fn write_media_segment(
        &self,
        _filename: &str,
        _packets: &[MediaPacket],
    ) -> ServerResult<()> {
        Err(fmp4_mux_unimplemented("media"))
    }

    /// Creates an in-memory fMP4 initialization segment.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::Internal`] — real fMP4 init muxing is not
    /// implemented (see [`fmp4_mux_unimplemented`]).
    #[allow(dead_code)]
    fn create_init_segment(&self) -> ServerResult<InitSegment> {
        Err(fmp4_mux_unimplemented("init"))
    }

    /// Creates an in-memory fMP4 media segment from packets.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError::Internal`] — real fMP4 muxing is not
    /// implemented (see [`fmp4_mux_unimplemented`]).
    #[allow(dead_code)]
    fn create_media_segment(
        &self,
        _number: u64,
        _packets: &[MediaPacket],
    ) -> ServerResult<MediaSegment> {
        Err(fmp4_mux_unimplemented("media"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use oximedia_net::rtmp::{MediaPacket, MediaPacketType};

    fn tmp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("oximedia-dash-seg-{name}-{}", std::process::id()))
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
    async fn write_init_segment_returns_honest_err_not_empty_file() {
        let dir = tmp_dir("init");
        let writer = DashSegmentWriter::new(&dir).expect("create writer");
        let filename = "init.mp4";

        let result = writer.write_init_segment(filename).await;
        assert!(
            result.is_err(),
            "write_init_segment must not fabricate an empty init.mp4"
        );
        assert!(
            !dir.join(filename).exists(),
            "no placeholder init segment should be written on the honest-error path"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn write_media_segment_returns_honest_err_not_fake_m4s() {
        let dir = tmp_dir("media");
        let writer = DashSegmentWriter::new(&dir).expect("create writer");
        let filename = "segment1.m4s";

        let result = writer
            .write_media_segment(filename, &sample_packets())
            .await;
        assert!(
            result.is_err(),
            "write_media_segment must not fabricate a concatenated .m4s segment"
        );
        assert!(
            !dir.join(filename).exists(),
            "no fabricated fragment file should be written on the honest-error path"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
