//! DASH packager for creating DASH streams.

use crate::dash::{DashSegmentWriter, MpdGenerator};
use crate::error::ServerResult;
use oximedia_net::rtmp::MediaPacket;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

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

    /// Initialization segment written (or honestly skipped as unsupported).
    init_written: RwLock<bool>,

    /// Set once we have logged that real segment muxing is unavailable, so we
    /// warn a single time per stream instead of once per segment.
    mux_unsupported_logged: AtomicBool,
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
            mux_unsupported_logged: AtomicBool::new(false),
        })
    }

    /// Processes a media packet.
    async fn process_packet(&self, packet: MediaPacket) -> ServerResult<()> {
        // Attempt the initialization segment once. `write_init_segment`
        // degrades honestly (never fabricates an empty init.mp4); we mark the
        // attempt done regardless so we do not retry on every packet.
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

    /// Logs the "segment muxing unavailable" warning at most once per stream.
    fn warn_mux_unsupported_once(&self, err: &crate::error::ServerError) {
        if !self.mux_unsupported_logged.swap(true, Ordering::Relaxed) {
            warn!(
                "DASH segment muxing unavailable for stream '{}': {}; \
                 dropping segments (no fabricated output produced)",
                self.stream_key, err
            );
        }
    }

    /// Attempts to write the fMP4 initialization segment.
    ///
    /// Real fMP4 init muxing is not implemented yet (see dash/segment.rs), so
    /// this degrades honestly: it neither writes a placeholder `init.mp4` nor
    /// claims success — it logs once and moves on so ingest is not aborted.
    async fn write_init_segment(&self) -> ServerResult<()> {
        let init_name = "init.mp4";
        match self.segment_writer.write_init_segment(init_name).await {
            Ok(()) => {
                info!("Wrote DASH initialization segment: {}", init_name);
            }
            Err(e) => {
                self.warn_mux_unsupported_once(&e);
            }
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
            (idx, format!("segment{}.m4s", idx))
        };

        // Attempt to write a real fMP4 fragment. Real muxing is not yet
        // implemented (see dash/segment.rs), so this fails honestly. We degrade
        // by dropping the segment rather than writing a fabricated file or
        // advertising a segment that does not exist in the MPD.
        match self
            .segment_writer
            .write_media_segment(&segment_name, &packets)
            .await
        {
            Ok(()) => {
                // Only advertise the segment once it was genuinely produced.
                self.mpd_gen
                    .add_segment(index_val, self.config.segment_duration.as_secs_f64())?;
                info!("Finalized DASH segment: {}", segment_name);
            }
            Err(e) => {
                self.warn_mux_unsupported_once(&e);
            }
        }

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
