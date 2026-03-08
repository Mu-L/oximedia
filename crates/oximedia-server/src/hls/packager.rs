//! HLS packager for creating HLS streams.

use crate::error::ServerResult;
use crate::hls::{PlaylistGenerator, SegmentWriter};
use oximedia_net::rtmp::MediaPacket;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

/// HLS configuration.
#[derive(Debug, Clone)]
pub struct HlsConfig {
    /// Output directory.
    pub output_dir: PathBuf,

    /// Segment duration.
    pub segment_duration: Duration,

    /// Playlist length (number of segments).
    pub playlist_length: usize,

    /// Enable low-latency HLS.
    pub low_latency: bool,

    /// Part duration for LL-HLS.
    pub part_duration: Duration,

    /// DVR window duration.
    pub dvr_window: Option<Duration>,
}

impl Default for HlsConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("./hls"),
            segment_duration: Duration::from_secs(2),
            playlist_length: 6,
            low_latency: false,
            part_duration: Duration::from_millis(500),
            dvr_window: None,
        }
    }
}

/// HLS stream packager.
#[allow(dead_code)]
struct StreamPackager {
    /// Stream key.
    stream_key: String,

    /// Configuration.
    config: HlsConfig,

    /// Playlist generator.
    playlist_gen: PlaylistGenerator,

    /// Segment writer.
    segment_writer: SegmentWriter,

    /// Current segment index.
    segment_index: RwLock<u64>,

    /// Packets buffered for current segment.
    packet_buffer: RwLock<Vec<MediaPacket>>,
}

impl StreamPackager {
    /// Creates a new stream packager.
    fn new(stream_key: String, config: HlsConfig) -> ServerResult<Self> {
        let playlist_gen = PlaylistGenerator::new(config.clone());
        let segment_writer = SegmentWriter::new(&config.output_dir)?;

        Ok(Self {
            stream_key,
            config,
            playlist_gen,
            segment_writer,
            segment_index: RwLock::new(0),
            packet_buffer: RwLock::new(Vec::new()),
        })
    }

    /// Processes a media packet.
    async fn process_packet(&self, packet: MediaPacket) -> ServerResult<()> {
        let should_finalize = {
            let mut buffer = self.packet_buffer.write();
            buffer.push(packet);
            // Check if we should finalize the segment
            // (simplified logic - in real implementation, check timestamp and keyframes)
            buffer.len() >= 100
        };

        if should_finalize {
            self.finalize_segment().await?;
        }

        Ok(())
    }

    /// Finalizes the current segment.
    async fn finalize_segment(&self) -> ServerResult<()> {
        let packets = {
            let mut buffer = self.packet_buffer.write();
            std::mem::take(&mut *buffer)
        };

        if packets.is_empty() {
            return Ok(());
        }

        let (index_val, segment_name) = {
            let mut index = self.segment_index.write();
            let idx = *index;
            *index += 1;
            (idx, format!("segment{}.ts", idx))
        };

        // Write segment
        self.segment_writer
            .write_segment(&segment_name, &packets)
            .await?;

        // Update playlist
        self.playlist_gen
            .add_segment(&segment_name, self.config.segment_duration.as_secs_f64())?;

        let _ = index_val; // used above

        info!("Finalized HLS segment: {}", segment_name);

        Ok(())
    }
}

/// HLS packager.
pub struct HlsPackager {
    /// Configuration.
    config: HlsConfig,

    /// Active stream packagers.
    packagers: Arc<RwLock<HashMap<String, Arc<StreamPackager>>>>,
}

impl HlsPackager {
    /// Creates a new HLS packager.
    pub fn new(config: HlsConfig) -> Self {
        Self {
            config,
            packagers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Starts packaging for a stream.
    pub fn start_stream(&self, stream_key: impl Into<String>) -> ServerResult<()> {
        let stream_key = stream_key.into();

        let packager = Arc::new(StreamPackager::new(
            stream_key.clone(),
            self.config.clone(),
        )?);

        let mut packagers = self.packagers.write();
        packagers.insert(stream_key.clone(), packager);

        info!("Started HLS packaging for stream: {}", stream_key);

        Ok(())
    }

    /// Stops packaging for a stream.
    pub async fn stop_stream(&self, stream_key: &str) -> ServerResult<()> {
        let packager = {
            let mut packagers = self.packagers.write();
            packagers.remove(stream_key)
        };

        if let Some(packager) = packager {
            // Finalize any remaining segment
            packager.finalize_segment().await?;
        }

        info!("Stopped HLS packaging for stream: {}", stream_key);

        Ok(())
    }

    /// Processes a media packet.
    pub async fn process_packet(&self, stream_key: &str, packet: MediaPacket) -> ServerResult<()> {
        let packager = {
            let packagers = self.packagers.read();
            packagers.get(stream_key).map(Arc::clone)
        };

        if let Some(packager) = packager {
            packager.process_packet(packet).await?;
        }

        Ok(())
    }

    /// Gets the playlist path for a stream.
    #[must_use]
    pub fn get_playlist_path(&self, stream_key: &str) -> PathBuf {
        self.config
            .output_dir
            .join(stream_key)
            .join("playlist.m3u8")
    }
}
