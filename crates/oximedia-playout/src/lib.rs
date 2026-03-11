//! # OxiMedia Playout Server
//!
//! Professional broadcast playout server with frame-accurate timing,
//! 24/7 reliability, and support for multiple broadcast outputs.
//!
//! ## Features
//!
//! - Frame-accurate timing (no dropped frames)
//! - 24/7 reliability with emergency fallback
//! - Genlock/sync support for professional broadcast
//! - Multiple simultaneous outputs (SDI, NDI, RTMP, SRT, IP multicast)
//! - Graphics overlay (logos, lower thirds, tickers)
//! - Comprehensive monitoring and alerting
//! - SCTE-35 marker insertion for ad breaks
//! - Dynamic playlist management
//!
//! ## Example
//!
//! ```no_run
//! use oximedia_playout::{PlayoutServer, PlayoutConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = PlayoutConfig::default();
//!     let server = PlayoutServer::new(config).await?;
//!     server.start().await?;
//!     Ok(())
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;
use thiserror::Error;
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::{mpsc, RwLock};

/// SCTE-35 ad insertion and splice point management.
pub mod ad_insertion;
#[cfg(not(target_arch = "wasm32"))]
pub mod api;
#[cfg(not(target_arch = "wasm32"))]
pub mod asrun;
#[cfg(not(target_arch = "wasm32"))]
pub mod automation;
pub mod branding;
#[cfg(not(target_arch = "wasm32"))]
pub mod bxf;
pub mod catchup;
pub mod cg;
#[cfg(not(target_arch = "wasm32"))]
pub mod channel;
/// Channel format and configuration registry (SD/HD/UHD, frame rate, audio).
pub mod channel_config;
pub mod clip_store;
pub mod compliance_ingest;
#[cfg(not(target_arch = "wasm32"))]
pub mod content;
#[cfg(not(target_arch = "wasm32"))]
pub mod device;
pub mod event_log;
#[cfg(not(target_arch = "wasm32"))]
pub mod failover;
/// Frame ring buffer with pre-roll gating and overflow/underrun detection.
pub mod frame_buffer;
/// Automatic gap detection and filler content insertion.
pub mod gap_filler;
pub mod graphics;
pub mod highlight_automation;
pub mod ingest;
/// Signal routing from programme sources to SDI/IP/RTMP/file targets.
pub mod media_router_playout;
pub mod monitoring;
#[cfg(not(target_arch = "wasm32"))]
pub mod output;
pub mod output_router;
#[cfg(not(target_arch = "wasm32"))]
pub mod playback;
pub mod playlist;
/// Playlist ingest session: format detection, item validation, clip trimming.
pub mod playlist_ingest;
/// Detailed playout logging and audit trail.
pub mod playout_log;
/// 24-hour playout schedule grid with conflict detection and gap finding.
pub mod playout_schedule;
pub mod rundown;
/// Time-blocked schedule management for broadcast playout.
pub mod schedule_block;
/// Time-slot schedule grid with booking, availability, and overlap queries.
pub mod schedule_slot;
pub mod scheduler;
pub mod secondary_events;
/// Ordered processing chain (input -> process -> output) with bypass support.
pub mod signal_chain;
pub mod tally_system;
/// Timecode burn-in overlay for monitoring outputs.

/// Result type for playout operations
pub type Result<T> = std::result::Result<T, PlayoutError>;

/// Errors that can occur during playout operations
#[derive(Error, Debug)]
pub enum PlayoutError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Scheduler error: {0}")]
    Scheduler(String),

    #[error("Playlist error: {0}")]
    Playlist(String),

    #[error("Playback error: {0}")]
    Playback(String),

    #[error("Output error: {0}")]
    Output(String),

    #[error("Graphics error: {0}")]
    Graphics(String),

    #[error("Monitoring error: {0}")]
    Monitoring(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Synchronization error: {0}")]
    Sync(String),

    #[error("Timing error: {0}")]
    Timing(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Emergency fallback activated: {0}")]
    EmergencyFallback(String),
}

/// Video format configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VideoFormat {
    /// 1920x1080 progressive at 23.976 fps
    HD1080p2398,
    /// 1920x1080 progressive at 24 fps
    HD1080p24,
    /// 1920x1080 progressive at 25 fps
    HD1080p25,
    /// 1920x1080 progressive at 29.97 fps
    HD1080p2997,
    /// 1920x1080 progressive at 30 fps
    HD1080p30,
    /// 1920x1080 progressive at 50 fps
    HD1080p50,
    /// 1920x1080 progressive at 59.94 fps
    HD1080p5994,
    /// 1920x1080 progressive at 60 fps
    HD1080p60,
    /// 1920x1080 interlaced at 50 Hz
    HD1080i50,
    /// 1920x1080 interlaced at 59.94 Hz
    HD1080i5994,
    /// 3840x2160 progressive at 25 fps
    UHD2160p25,
    /// 3840x2160 progressive at 29.97 fps
    UHD2160p2997,
    /// 3840x2160 progressive at 50 fps
    UHD2160p50,
    /// 3840x2160 progressive at 59.94 fps
    UHD2160p5994,
}

impl VideoFormat {
    /// Get frame rate in frames per second
    pub fn fps(&self) -> f64 {
        match self {
            Self::HD1080p2398 => 23.976,
            Self::HD1080p24 => 24.0,
            Self::HD1080p25 | Self::UHD2160p25 => 25.0,
            Self::HD1080p2997 | Self::UHD2160p2997 => 29.97,
            Self::HD1080p30 => 30.0,
            Self::HD1080p50 | Self::HD1080i50 | Self::UHD2160p50 => 50.0,
            Self::HD1080p5994 | Self::HD1080i5994 | Self::UHD2160p5994 => 59.94,
            Self::HD1080p60 => 60.0,
        }
    }

    /// Get width in pixels
    pub fn width(&self) -> u32 {
        match self {
            Self::HD1080p2398
            | Self::HD1080p24
            | Self::HD1080p25
            | Self::HD1080p2997
            | Self::HD1080p30
            | Self::HD1080p50
            | Self::HD1080p5994
            | Self::HD1080p60
            | Self::HD1080i50
            | Self::HD1080i5994 => 1920,
            Self::UHD2160p25 | Self::UHD2160p2997 | Self::UHD2160p50 | Self::UHD2160p5994 => 3840,
        }
    }

    /// Get height in pixels
    pub fn height(&self) -> u32 {
        match self {
            Self::HD1080p2398
            | Self::HD1080p24
            | Self::HD1080p25
            | Self::HD1080p2997
            | Self::HD1080p30
            | Self::HD1080p50
            | Self::HD1080p5994
            | Self::HD1080p60
            | Self::HD1080i50
            | Self::HD1080i5994 => 1080,
            Self::UHD2160p25 | Self::UHD2160p2997 | Self::UHD2160p50 | Self::UHD2160p5994 => 2160,
        }
    }
}

/// Audio format configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub bit_depth: u16,
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            bit_depth: 24,
        }
    }
}

/// Playout server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayoutConfig {
    /// Video format for output
    pub video_format: VideoFormat,

    /// Audio format for output
    pub audio_format: AudioFormat,

    /// Enable genlock synchronization
    pub genlock_enabled: bool,

    /// Reference clock source (e.g., "internal", "sdi", "ptp")
    pub clock_source: String,

    /// Buffer size in frames
    pub buffer_size: usize,

    /// Emergency fallback content path
    pub fallback_content: PathBuf,

    /// Maximum allowed latency in milliseconds
    pub max_latency_ms: u64,

    /// Enable frame drop detection
    pub detect_frame_drops: bool,

    /// Playlist directory
    pub playlist_dir: PathBuf,

    /// Content root directory
    pub content_root: PathBuf,

    /// Enable monitoring
    pub monitoring_enabled: bool,

    /// Monitoring port
    pub monitoring_port: u16,
}

impl Default for PlayoutConfig {
    fn default() -> Self {
        Self {
            video_format: VideoFormat::HD1080p25,
            audio_format: AudioFormat::default(),
            genlock_enabled: false,
            clock_source: "internal".to_string(),
            buffer_size: 10,
            fallback_content: PathBuf::from("/var/oximedia/fallback.mxf"),
            max_latency_ms: 100,
            detect_frame_drops: true,
            playlist_dir: PathBuf::from("/var/oximedia/playlists"),
            content_root: PathBuf::from("/var/oximedia/content"),
            monitoring_enabled: true,
            monitoring_port: 8080,
        }
    }
}

/// Playout server state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayoutState {
    /// Server is stopped
    Stopped,
    /// Server is starting up
    Starting,
    /// Server is running normally
    Running,
    /// Server is paused
    Paused,
    /// Server is in emergency fallback mode
    Fallback,
    /// Server is stopping
    Stopping,
}

/// Internal server state
#[cfg(not(target_arch = "wasm32"))]
struct ServerState {
    state: PlayoutState,
    scheduler: Option<Arc<scheduler::Scheduler>>,
    playback: Option<Arc<playback::PlaybackEngine>>,
    outputs: Vec<Arc<output::Output>>,
    graphics: Option<Arc<graphics::GraphicsEngine>>,
    monitor: Option<Arc<monitoring::Monitor>>,
}

/// Professional broadcast playout server
#[cfg(not(target_arch = "wasm32"))]
pub struct PlayoutServer {
    config: PlayoutConfig,
    state: Arc<RwLock<ServerState>>,
    #[allow(dead_code)]
    control_tx: mpsc::Sender<ControlCommand>,
    #[allow(dead_code)]
    control_rx: Arc<RwLock<mpsc::Receiver<ControlCommand>>>,
}

/// Control commands for the playout server
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum ControlCommand {
    Start,
    Stop,
    Pause,
    Resume,
    LoadPlaylist(PathBuf),
    EmergencyFallback,
    Shutdown,
}

#[cfg(not(target_arch = "wasm32"))]
impl PlayoutServer {
    /// Create a new playout server with the given configuration
    pub async fn new(config: PlayoutConfig) -> Result<Self> {
        let (control_tx, control_rx) = mpsc::channel(100);

        let state = ServerState {
            state: PlayoutState::Stopped,
            scheduler: None,
            playback: None,
            outputs: Vec::new(),
            graphics: None,
            monitor: None,
        };

        Ok(Self {
            config,
            state: Arc::new(RwLock::new(state)),
            control_tx,
            control_rx: Arc::new(RwLock::new(control_rx)),
        })
    }

    /// Start the playout server
    pub async fn start(&self) -> Result<()> {
        let mut state = self.state.write().await;

        if state.state != PlayoutState::Stopped {
            return Err(PlayoutError::Config(
                "Server is already running".to_string(),
            ));
        }

        state.state = PlayoutState::Starting;

        // Initialize scheduler
        let scheduler_config = scheduler::SchedulerConfig::default();
        state.scheduler = Some(Arc::new(scheduler::Scheduler::new(scheduler_config)));

        // Initialize playback engine
        let playback_config = playback::PlaybackConfig::from_playout_config(&self.config);
        state.playback = Some(Arc::new(playback::PlaybackEngine::new(playback_config)?));

        // Initialize graphics engine
        let graphics_config = graphics::GraphicsConfig::default();
        state.graphics = Some(Arc::new(graphics::GraphicsEngine::new(graphics_config)?));

        // Initialize monitor
        if self.config.monitoring_enabled {
            let monitor_config = monitoring::MonitorConfig {
                port: self.config.monitoring_port,
                audio_meters: true,
                waveform: false,
                vectorscope: false,
                alert_history_size: 100,
                metrics_retention_seconds: 3600,
            };
            state.monitor = Some(Arc::new(monitoring::Monitor::new(monitor_config)?));
        }

        state.state = PlayoutState::Running;

        Ok(())
    }

    /// Stop the playout server
    pub async fn stop(&self) -> Result<()> {
        let mut state = self.state.write().await;
        state.state = PlayoutState::Stopping;

        // Clean up resources
        state.monitor = None;
        state.graphics = None;
        state.playback = None;
        state.scheduler = None;
        state.outputs.clear();

        state.state = PlayoutState::Stopped;

        Ok(())
    }

    /// Pause playout
    pub async fn pause(&self) -> Result<()> {
        let mut state = self.state.write().await;
        if state.state == PlayoutState::Running {
            state.state = PlayoutState::Paused;
        }
        Ok(())
    }

    /// Resume playout
    pub async fn resume(&self) -> Result<()> {
        let mut state = self.state.write().await;
        if state.state == PlayoutState::Paused {
            state.state = PlayoutState::Running;
        }
        Ok(())
    }

    /// Get current server state
    pub async fn state(&self) -> PlayoutState {
        self.state.read().await.state
    }

    /// Load a new playlist
    pub async fn load_playlist(&self, path: PathBuf) -> Result<()> {
        let state = self.state.read().await;
        if let Some(scheduler) = &state.scheduler {
            scheduler.load_playlist(path).await?;
        }
        Ok(())
    }

    /// Activate emergency fallback
    pub async fn emergency_fallback(&self) -> Result<()> {
        let mut state = self.state.write().await;
        state.state = PlayoutState::Fallback;
        Ok(())
    }

    /// Get playout configuration
    pub fn config(&self) -> &PlayoutConfig {
        &self.config
    }

    /// Wait for server to finish (blocks until shutdown)
    pub async fn wait(&self) -> Result<()> {
        loop {
            let state = self.state().await;
            if state == PlayoutState::Stopped {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_format_properties() {
        let format = VideoFormat::HD1080p25;
        assert_eq!(format.fps(), 25.0);
        assert_eq!(format.width(), 1920);
        assert_eq!(format.height(), 1080);
    }

    #[test]
    fn test_default_config() {
        let config = PlayoutConfig::default();
        assert_eq!(config.video_format, VideoFormat::HD1080p25);
        assert_eq!(config.audio_format.sample_rate, 48000);
        assert_eq!(config.buffer_size, 10);
    }

    #[tokio::test]
    async fn test_server_lifecycle() {
        let config = PlayoutConfig::default();
        let server = PlayoutServer::new(config)
            .await
            .expect("should succeed in test");
        assert_eq!(server.state().await, PlayoutState::Stopped);
    }
}
