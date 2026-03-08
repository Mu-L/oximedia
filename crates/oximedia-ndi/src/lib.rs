//! NDI (Network Device Interface) implementation for OxiMedia
//!
//! This is a clean-room implementation of the NDI protocol that doesn't rely on
//! the official NDI SDK. It provides mDNS-based discovery, low-latency streaming,
//! and support for tally lights and PTZ control.
//!
//! # Features
//!
//! - mDNS-based source discovery
//! - Low-latency video streaming (<1 frame)
//! - Full HD and 4K support
//! - Tally light support (program/preview indicators)
//! - PTZ (Pan-Tilt-Zoom) control
//! - Audio/video synchronization
//! - Bandwidth adaptation
//!
//! # Example
//!
//! ```no_run
//! use oximedia_ndi::{NdiSource, NdiSender, SenderConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create an NDI sender
//! let sender = NdiSender::new(SenderConfig::default()).await?;
//!
//! // Send a video frame
//! // sender.send_video_frame(frame).await?;
//!
//! // Discover NDI sources
//! let sources = NdiSource::discover_sources(std::time::Duration::from_secs(5)).await?;
//! for source in sources {
//!     println!("Found: {} at {}", source.name(), source.address());
//! }
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]

pub mod audio_config;
pub mod audio_format;
pub mod av_buffer;
pub mod bandwidth;
pub mod channel_map;
pub mod clock_sync;
pub mod connection_config;
pub mod connection_state;
pub mod failover;
pub mod frame_buffer;
pub mod frame_sync;
pub mod genlock;
pub mod group;
pub mod metadata;
pub mod metadata_frame;
pub mod ndi_stats;
pub mod ptz;
pub mod quality;
pub mod routing;
pub mod sender_config;
pub mod source_filter;
pub mod source_registry;
pub mod statistics;
pub mod stream_info;
pub mod tally_bus;
pub mod tally_manager;
pub mod transport;
pub mod video_format;

mod codec;
mod discovery;
mod protocol;
mod receiver;
mod sender;
mod tally;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

pub use codec::{SpeedHqCodec, YuvFormat};
pub use discovery::{DiscoveryService, NdiSourceInfo};
pub use protocol::{NdiFrame, NdiFrameType, NdiMetadata};
pub use receiver::{NdiReceiver, ReceiverConfig};
pub use sender::{NdiSender, SenderConfig};
pub use tally::{TallyServer, TallyState};

use oximedia_core::OxiError;
use thiserror::Error;

/// NDI-specific errors
#[derive(Error, Debug)]
pub enum NdiError {
    #[error("Network error: {0}")]
    Network(#[from] std::io::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Discovery error: {0}")]
    Discovery(String),

    #[error("Codec error: {0}")]
    Codec(String),

    #[error("Timeout")]
    Timeout,

    #[error("Source not found: {0}")]
    SourceNotFound(String),

    #[error("Invalid frame format")]
    InvalidFrameFormat,

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Core error: {0}")]
    Core(#[from] OxiError),
}

/// Result type for NDI operations
pub type Result<T> = std::result::Result<T, NdiError>;

/// NDI configuration
#[derive(Debug, Clone)]
pub struct NdiConfig {
    /// Source name
    pub name: String,

    /// Source groups (for filtering)
    pub groups: Vec<String>,

    /// Enable low bandwidth mode
    pub low_bandwidth: bool,

    /// Buffer size in frames
    pub buffer_size: usize,

    /// Enable tally support
    pub enable_tally: bool,

    /// Enable PTZ support
    pub enable_ptz: bool,

    /// Discovery timeout
    pub discovery_timeout: Duration,

    /// Connection timeout
    pub connection_timeout: Duration,
}

impl Default for NdiConfig {
    fn default() -> Self {
        Self {
            name: "OxiMedia NDI Source".to_string(),
            groups: vec!["public".to_string()],
            low_bandwidth: false,
            buffer_size: 16,
            enable_tally: true,
            enable_ptz: true,
            discovery_timeout: Duration::from_secs(5),
            connection_timeout: Duration::from_secs(10),
        }
    }
}

/// Represents an NDI source that can be received from
#[derive(Debug, Clone)]
pub struct NdiSource {
    info: Arc<NdiSourceInfo>,
    receiver: Option<Arc<NdiReceiver>>,
}

impl NdiSource {
    /// Create a new NDI source from source information
    pub fn new(info: NdiSourceInfo) -> Self {
        Self {
            info: Arc::new(info),
            receiver: None,
        }
    }

    /// Get the source name
    pub fn name(&self) -> &str {
        &self.info.name
    }

    /// Get the source address
    pub fn address(&self) -> SocketAddr {
        self.info.address
    }

    /// Get the source groups
    pub fn groups(&self) -> &[String] {
        &self.info.groups
    }

    /// Discover available NDI sources on the network
    ///
    /// # Arguments
    ///
    /// * `timeout` - How long to wait for discovery responses
    ///
    /// # Returns
    ///
    /// A list of discovered NDI sources
    pub async fn discover_sources(timeout: Duration) -> Result<Vec<NdiSource>> {
        let discovery = DiscoveryService::new()?;
        let sources = discovery.discover(timeout).await?;
        Ok(sources.into_iter().map(NdiSource::new).collect())
    }

    /// Connect to this NDI source for receiving
    ///
    /// # Arguments
    ///
    /// * `config` - Receiver configuration
    ///
    /// # Returns
    ///
    /// A receiver that can be used to receive frames from this source
    pub async fn connect(&mut self, config: ReceiverConfig) -> Result<Arc<NdiReceiver>> {
        let receiver = Arc::new(NdiReceiver::new(self.info.clone(), config).await?);
        self.receiver = Some(receiver.clone());
        Ok(receiver)
    }

    /// Get the connected receiver, if any
    pub fn receiver(&self) -> Option<Arc<NdiReceiver>> {
        self.receiver.clone()
    }

    /// Disconnect from the source
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(receiver) = self.receiver.take() {
            receiver.disconnect().await?;
        }
        Ok(())
    }
}

/// PTZ (Pan-Tilt-Zoom) command
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PtzCommand {
    /// Pan left at specified speed (0.0 to 1.0)
    PanLeft(f32),

    /// Pan right at specified speed (0.0 to 1.0)
    PanRight(f32),

    /// Tilt up at specified speed (0.0 to 1.0)
    TiltUp(f32),

    /// Tilt down at specified speed (0.0 to 1.0)
    TiltDown(f32),

    /// Zoom in at specified speed (0.0 to 1.0)
    ZoomIn(f32),

    /// Zoom out at specified speed (0.0 to 1.0)
    ZoomOut(f32),

    /// Focus near at specified speed (0.0 to 1.0)
    FocusNear(f32),

    /// Focus far at specified speed (0.0 to 1.0)
    FocusFar(f32),

    /// Auto focus
    AutoFocus,

    /// Store preset at index
    StorePreset(u8),

    /// Recall preset at index
    RecallPreset(u8),

    /// Stop all motion
    Stop,
}

/// Video frame format information
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoFormat {
    /// Width in pixels
    pub width: u32,

    /// Height in pixels
    pub height: u32,

    /// Frame rate numerator
    pub fps_num: u32,

    /// Frame rate denominator
    pub fps_den: u32,

    /// Progressive (true) or interlaced (false)
    pub progressive: bool,

    /// Aspect ratio (width/height)
    pub aspect_ratio: f32,
}

impl VideoFormat {
    /// Create a new video format
    pub fn new(width: u32, height: u32, fps_num: u32, fps_den: u32) -> Self {
        Self {
            width,
            height,
            fps_num,
            fps_den,
            progressive: true,
            aspect_ratio: width as f32 / height as f32,
        }
    }

    /// Create a Full HD 1080p30 format
    pub fn full_hd_30p() -> Self {
        Self::new(1920, 1080, 30, 1)
    }

    /// Create a Full HD 1080p60 format
    pub fn full_hd_60p() -> Self {
        Self::new(1920, 1080, 60, 1)
    }

    /// Create a 4K UHD 30p format
    pub fn uhd_4k_30p() -> Self {
        Self::new(3840, 2160, 30, 1)
    }

    /// Create a 4K UHD 60p format
    pub fn uhd_4k_60p() -> Self {
        Self::new(3840, 2160, 60, 1)
    }

    /// Get the frame rate as a floating point number
    pub fn frame_rate(&self) -> f64 {
        f64::from(self.fps_num) / f64::from(self.fps_den)
    }
}

/// Audio format information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormat {
    /// Sample rate in Hz
    pub sample_rate: u32,

    /// Number of channels
    pub channels: u16,

    /// Bits per sample
    pub bits_per_sample: u16,
}

impl AudioFormat {
    /// Create a new audio format
    pub fn new(sample_rate: u32, channels: u16, bits_per_sample: u16) -> Self {
        Self {
            sample_rate,
            channels,
            bits_per_sample,
        }
    }

    /// Create a standard 48kHz stereo 16-bit format
    pub fn stereo_48k() -> Self {
        Self::new(48000, 2, 16)
    }

    /// Create a standard 48kHz mono 16-bit format
    pub fn mono_48k() -> Self {
        Self::new(48000, 1, 16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_format() {
        let format = VideoFormat::full_hd_60p();
        assert_eq!(format.width, 1920);
        assert_eq!(format.height, 1080);
        assert_eq!(format.fps_num, 60);
        assert_eq!(format.fps_den, 1);
        assert!((format.frame_rate() - 60.0).abs() < 0.001);
    }

    #[test]
    fn test_audio_format() {
        let format = AudioFormat::stereo_48k();
        assert_eq!(format.sample_rate, 48000);
        assert_eq!(format.channels, 2);
        assert_eq!(format.bits_per_sample, 16);
    }

    #[test]
    fn test_ndi_config_default() {
        let config = NdiConfig::default();
        assert!(config.enable_tally);
        assert!(config.enable_ptz);
        assert_eq!(config.buffer_size, 16);
    }
}
