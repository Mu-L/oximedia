//! Transcoding engine for real-time stream processing.

use crate::error::{ServerError, ServerResult};
use crate::transcode::AbrLadder;
use oximedia_net::rtmp::MediaPacket;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;
use uuid::Uuid;

/// Transcode job state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscodeJobState {
    /// Initializing.
    Initializing,
    /// Running.
    Running,
    /// Paused.
    Paused,
    /// Completed.
    Completed,
    /// Failed.
    Failed,
}

/// Transcode job.
pub struct TranscodeJob {
    /// Job ID.
    pub id: Uuid,

    /// Stream key.
    pub stream_key: String,

    /// ABR ladder.
    pub ladder: AbrLadder,

    /// Job state.
    pub state: RwLock<TranscodeJobState>,

    /// Input packet sender.
    pub input_tx: mpsc::UnboundedSender<MediaPacket>,

    /// Output packet receivers (one per quality level).
    pub output_rxs: HashMap<String, mpsc::UnboundedReceiver<MediaPacket>>,

    /// Frames processed.
    pub frames_processed: RwLock<u64>,

    /// Start time.
    pub start_time: std::time::Instant,
}

impl TranscodeJob {
    /// Creates a new transcode job.
    pub fn new(stream_key: impl Into<String>, ladder: AbrLadder) -> Self {
        let (input_tx, _input_rx) = mpsc::unbounded_channel();
        let output_rxs = HashMap::new();

        Self {
            id: Uuid::new_v4(),
            stream_key: stream_key.into(),
            ladder,
            state: RwLock::new(TranscodeJobState::Initializing),
            input_tx,
            output_rxs,
            frames_processed: RwLock::new(0),
            start_time: std::time::Instant::now(),
        }
    }

    /// Gets the current state.
    #[must_use]
    pub fn state(&self) -> TranscodeJobState {
        *self.state.read()
    }

    /// Sets the state.
    pub fn set_state(&self, state: TranscodeJobState) {
        *self.state.write() = state;
    }

    /// Gets frames processed.
    #[must_use]
    pub fn frames_processed(&self) -> u64 {
        *self.frames_processed.read()
    }

    /// Increments frames processed.
    pub fn increment_frames(&self) {
        *self.frames_processed.write() += 1;
    }

    /// Gets processing duration.
    #[must_use]
    pub fn duration(&self) -> std::time::Duration {
        std::time::Instant::now().duration_since(self.start_time)
    }
}

/// Transcoding engine.
#[allow(dead_code)]
pub struct TranscodeEngine {
    /// Active jobs.
    jobs: Arc<RwLock<HashMap<String, Arc<TranscodeJob>>>>,

    /// Default ABR ladder.
    default_ladder: AbrLadder,

    /// Worker pool size.
    workers: usize,
}

impl TranscodeEngine {
    /// Creates a new transcoding engine.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    pub async fn new() -> ServerResult<Self> {
        let default_ladder = AbrLadder::standard();
        let workers = num_cpus::get();

        Ok(Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            default_ladder,
            workers,
        })
    }

    /// Creates a new transcode job.
    pub fn create_job(&self, stream_key: impl Into<String>) -> Arc<TranscodeJob> {
        self.create_job_with_ladder(stream_key, self.default_ladder.clone())
    }

    /// Creates a new transcode job with custom ladder.
    pub fn create_job_with_ladder(
        &self,
        stream_key: impl Into<String>,
        ladder: AbrLadder,
    ) -> Arc<TranscodeJob> {
        let stream_key = stream_key.into();
        let job = Arc::new(TranscodeJob::new(stream_key.clone(), ladder));

        let mut jobs = self.jobs.write();
        jobs.insert(stream_key.clone(), Arc::clone(&job));

        info!("Created transcode job for stream: {}", stream_key);
        job
    }

    /// Gets a transcode job.
    #[must_use]
    pub fn get_job(&self, stream_key: &str) -> Option<Arc<TranscodeJob>> {
        let jobs = self.jobs.read();
        jobs.get(stream_key).cloned()
    }

    /// Removes a transcode job.
    pub fn remove_job(&self, stream_key: &str) {
        let mut jobs = self.jobs.write();
        jobs.remove(stream_key);
        info!("Removed transcode job for stream: {}", stream_key);
    }

    /// Lists all jobs.
    #[must_use]
    pub fn list_jobs(&self) -> Vec<Arc<TranscodeJob>> {
        let jobs = self.jobs.read();
        jobs.values().cloned().collect()
    }

    /// Processes a media packet through the transcode pipeline.
    ///
    /// # Errors
    ///
    /// Real-time transcoding on ingest requires a persistent, per-stream
    /// decode -> scale -> encode graph that carries codec state across
    /// packets (keyed by stream, one encoder per ABR rung). That graph is
    /// not yet wired here, so — rather than silently returning `Ok(())` and
    /// fabricating a "transcode happened" — this returns an honest error.
    /// The ingest caller is expected to degrade to stream-copy (pass-through)
    /// when it sees this error, so no transcode is ever falsely claimed.
    // TODO(0.2.x): implement a real per-stream transcode pipeline — resolve
    // the `TranscodeJob` for the packet's stream, feed the packet into a live
    // decode -> ABR-scale -> encode graph (reuse oximedia-transcode), and emit
    // the encoded output on each quality level's `output_rxs` channel. Until
    // then this path is honestly unimplemented.
    pub async fn process_packet(&self, _packet: &MediaPacket) -> ServerResult<()> {
        Err(ServerError::TranscodingFailed(
            "real-time ingest transcoding is not implemented; caller must \
             stream-copy (pass-through) this packet"
                .to_string(),
        ))
    }

    /// Starts transcoding for a stream.
    pub async fn start_transcoding(&self, stream_key: &str) -> ServerResult<()> {
        let job = self.get_job(stream_key).ok_or_else(|| {
            ServerError::NotFound(format!("Transcode job not found: {}", stream_key))
        })?;

        job.set_state(TranscodeJobState::Running);
        info!("Started transcoding for stream: {}", stream_key);

        Ok(())
    }

    /// Stops transcoding for a stream.
    pub async fn stop_transcoding(&self, stream_key: &str) -> ServerResult<()> {
        let job = self.get_job(stream_key).ok_or_else(|| {
            ServerError::NotFound(format!("Transcode job not found: {}", stream_key))
        })?;

        job.set_state(TranscodeJobState::Completed);
        info!("Stopped transcoding for stream: {}", stream_key);

        Ok(())
    }

    /// Gets the number of active jobs.
    #[must_use]
    pub fn active_jobs(&self) -> usize {
        let jobs = self.jobs.read();
        jobs.values()
            .filter(|j| j.state() == TranscodeJobState::Running)
            .count()
    }
}

// Stub for num_cpus (in real implementation, would use num_cpus crate)
mod num_cpus {
    #[must_use]
    #[allow(dead_code)]
    pub fn get() -> usize {
        std::thread::available_parallelism()
            .map(std::num::NonZeroUsize::get)
            .unwrap_or(4)
    }
}
