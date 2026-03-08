//! DASH packager for creating DASH streams.

use crate::dash::{DashSegmentWriter, MpdGenerator};
use crate::error::ServerResult;
use oximedia_net::rtmp::MediaPacket;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

/// DASH configuration.
#[derive(Debug, Clone)]
pub struct DashConfig {
    /// Output directory.
    pub output_dir: PathBuf,

    /// Segment duration.
    pub segment_duration: Duration,

    /// Minimum buffer time.
    pub min_buffer_time: Duration,

    /// Time shift buffer depth (for live DVR).
    pub time_shift_buffer_depth: Option<Duration>,

    /// Enable low-latency DASH.
    pub low_latency: bool,

    /// Suggested presentation delay.
    pub suggested_presentation_delay: Duration,
}

impl Default for DashConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("./dash"),
            segment_duration: Duration::from_secs(2),
            min_buffer_time: Duration::from_secs(2),
            time_shift_buffer_depth: Some(Duration::from_secs(60)),
            low_latency: false,
            suggested_presentation_delay: Duration::from_secs(6),
        }
    }
}

/// DASH stream packager.
#[allow(dead_code)]
struct StreamPackager {
    /// Stream key.
    stream_key: String,

    /// Configuration.
    config: DashConfig,

    /// MPD generator.
    mpd_gen: MpdGenerator,

    /// Segment writer.
    segment_writer: DashSegmentWriter,

    /// Current segment index.
    segment_index: RwLock<u64>,

    /// Packets buffered for current segment.
    packet_buffer: RwLock<Vec<MediaPacket>>,

    /// Initialization segment written.
    init_written: RwLock<bool>,
}

impl StreamPackager {
    /// Creates a new stream packager.
    fn new(stream_key: String, config: DashConfig) -> ServerResult<Self> {
        let mpd_gen = MpdGenerator::new(config.clone());
        let segment_writer = DashSegmentWriter::new(&config.output_dir)?;

        Ok(Self {
            stream_key,
            config,
            mpd_gen,
            segment_writer,
            segment_index: RwLock::new(1),
            packet_buffer: RwLock::new(Vec::new()),
            init_written: RwLock::new(false),
        })
    }

    /// Processes a media packet.
    async fn process_packet(&self, packet: MediaPacket) -> ServerResult<()> {
        // Write initialization segment if not done yet
        if !*self.init_written.read() {
            self.write_init_segment().await?;
            *self.init_written.write() = true;
        }

        let should_finalize = {
            let mut buffer = self.packet_buffer.write();
            buffer.push(packet);
            // Check if we should finalize the segment
            buffer.len() >= 100
        };

        if should_finalize {
            self.finalize_segment().await?;
        }

        Ok(())
    }

    /// Writes the initialization segment.
    async fn write_init_segment(&self) -> ServerResult<()> {
        let init_name = "init.mp4";
        self.segment_writer.write_init_segment(init_name).await?;
        info!("Wrote DASH initialization segment: {}", init_name);
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
            (idx, format!("segment{}.m4s", idx))
        };

        // Write segment
        self.segment_writer
            .write_media_segment(&segment_name, &packets)
            .await?;

        // Update MPD
        self.mpd_gen
            .add_segment(index_val, self.config.segment_duration.as_secs_f64())?;

        info!("Finalized DASH segment: {}", segment_name);

        Ok(())
    }
}

/// DASH packager.
pub struct DashPackager {
    /// Configuration.
    config: DashConfig,

    /// Active stream packagers.
    packagers: Arc<RwLock<HashMap<String, Arc<StreamPackager>>>>,
}

impl DashPackager {
    /// Creates a new DASH packager.
    #[must_use]
    pub fn new(config: DashConfig) -> Self {
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

        info!("Started DASH packaging for stream: {}", stream_key);

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

        info!("Stopped DASH packaging for stream: {}", stream_key);

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

    /// Gets the MPD path for a stream.
    #[must_use]
    pub fn get_mpd_path(&self, stream_key: &str) -> PathBuf {
        self.config.output_dir.join(stream_key).join("manifest.mpd")
    }
}
