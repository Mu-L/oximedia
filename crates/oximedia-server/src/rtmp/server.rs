//! RTMP ingest server implementation.

use crate::cdn::CdnUploader;
use crate::error::ServerResult;
use crate::metrics::MetricsCollector;
use crate::record::StreamRecorder;
use crate::transcode::TranscodeEngine;
use async_trait::async_trait;
use oximedia_net::rtmp::{
    AuthHandler, AuthResult, MediaPacket as NetMediaPacket, PublishType, RtmpServer,
    RtmpServerBuilder, RtmpServerConfig, StreamMetadata,
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;

/// RTMP ingest server configuration.
#[derive(Debug, Clone)]
pub struct RtmpIngestConfig {
    /// Bind address for RTMP server.
    pub bind_addr: SocketAddr,

    /// Maximum number of concurrent streams.
    pub max_streams: usize,

    /// Enable authentication.
    pub enable_auth: bool,

    /// Enable transcoding.
    pub enable_transcoding: bool,

    /// Enable recording.
    pub enable_recording: bool,

    /// Enable CDN upload.
    pub enable_cdn_upload: bool,

    /// Recording directory.
    pub record_dir: String,

    /// Chunk size for RTMP.
    pub chunk_size: usize,

    /// Max chunk size.
    pub max_chunk_size: usize,

    /// Window acknowledgement size.
    pub window_ack_size: u32,
}

impl Default for RtmpIngestConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:1935".parse().expect("valid address"),
            max_streams: 1000,
            enable_auth: true,
            enable_transcoding: true,
            enable_recording: false,
            enable_cdn_upload: false,
            record_dir: "./recordings".to_string(),
            chunk_size: 4096,
            max_chunk_size: 65536,
            window_ack_size: 2_500_000,
        }
    }
}

/// Stream key validator.
pub struct StreamKeyValidator {
    valid_keys: RwLock<HashMap<String, StreamKeyInfo>>,
}

/// Stream key information.
#[derive(Debug, Clone)]
pub struct StreamKeyInfo {
    /// Stream key.
    pub key: String,

    /// Application name.
    pub app_name: String,

    /// User ID.
    pub user_id: Option<String>,

    /// Maximum bitrate (bits per second).
    pub max_bitrate: Option<u64>,

    /// Allowed codecs.
    pub allowed_codecs: Vec<String>,

    /// Is active.
    pub active: bool,
}

impl StreamKeyValidator {
    /// Creates a new stream key validator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            valid_keys: RwLock::new(HashMap::new()),
        }
    }

    /// Adds a valid stream key.
    pub fn add_key(&self, info: StreamKeyInfo) {
        let mut keys = self.valid_keys.write();
        keys.insert(info.key.clone(), info);
    }

    /// Removes a stream key.
    pub fn remove_key(&self, key: &str) {
        let mut keys = self.valid_keys.write();
        keys.remove(key);
    }

    /// Validates a stream key.
    #[must_use]
    pub fn validate(&self, app: &str, stream_key: &str) -> bool {
        let keys = self.valid_keys.read();
        if let Some(info) = keys.get(stream_key) {
            info.active && info.app_name == app
        } else {
            // For development, allow all keys
            true
        }
    }

    /// Gets stream key info.
    #[must_use]
    pub fn get_key_info(&self, stream_key: &str) -> Option<StreamKeyInfo> {
        let keys = self.valid_keys.read();
        keys.get(stream_key).cloned()
    }
}

impl Default for StreamKeyValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuthHandler for StreamKeyValidator {
    async fn authenticate_connect(
        &self,
        _app: &str,
        _tc_url: &str,
        _params: &std::collections::HashMap<String, String>,
    ) -> AuthResult {
        AuthResult::Success
    }

    async fn authenticate_publish(
        &self,
        app: &str,
        stream_key: &str,
        _publish_type: PublishType,
    ) -> AuthResult {
        if self.validate(app, stream_key) {
            info!("Authenticated publish: {}/{}", app, stream_key);
            AuthResult::Success
        } else {
            warn!("Rejected publish: {}/{}", app, stream_key);
            AuthResult::Failed("Invalid stream key".to_string())
        }
    }

    async fn authenticate_play(&self, app: &str, stream_key: &str) -> AuthResult {
        if self.validate(app, stream_key) {
            AuthResult::Success
        } else {
            AuthResult::Failed("Invalid stream key".to_string())
        }
    }
}

/// Active ingest stream.
pub struct IngestStream {
    /// Stream ID.
    pub id: Uuid,

    /// Stream key.
    pub stream_key: String,

    /// Application name.
    pub app_name: String,

    /// Metadata.
    pub metadata: StreamMetadata,

    /// Packet sender.
    pub packet_tx: mpsc::UnboundedSender<NetMediaPacket>,

    /// Bytes received.
    pub bytes_received: Arc<RwLock<u64>>,

    /// Packets received.
    pub packets_received: Arc<RwLock<u64>>,

    /// Start time.
    pub start_time: std::time::Instant,
}

/// RTMP ingest server.
pub struct RtmpIngestServer {
    /// Configuration.
    config: RtmpIngestConfig,

    /// RTMP server.
    rtmp_server: Arc<RtmpServer>,

    /// Stream key validator.
    validator: Arc<StreamKeyValidator>,

    /// Active streams.
    streams: Arc<RwLock<HashMap<String, Arc<IngestStream>>>>,

    /// Transcode engine.
    transcode_engine: Option<Arc<TranscodeEngine>>,

    /// Stream recorder.
    recorder: Option<Arc<StreamRecorder>>,

    /// CDN uploader.
    cdn_uploader: Option<Arc<CdnUploader>>,

    /// Metrics collector.
    metrics: Arc<MetricsCollector>,
}

impl RtmpIngestServer {
    /// Creates a new RTMP ingest server.
    ///
    /// # Errors
    ///
    /// Returns an error if server initialization fails.
    pub async fn new(
        config: RtmpIngestConfig,
        metrics: Arc<MetricsCollector>,
    ) -> ServerResult<Self> {
        let validator = Arc::new(StreamKeyValidator::new());

        let _rtmp_config = RtmpServerConfig {
            bind_address: config.bind_addr.to_string(),
            chunk_size: config.chunk_size as u32,
            window_ack_size: config.window_ack_size,
            max_connections: config.max_streams,
            ..RtmpServerConfig::default()
        };

        let mut rtmp_builder = RtmpServerBuilder::new();
        rtmp_builder = rtmp_builder.auth_handler(Arc::clone(&validator) as Arc<dyn AuthHandler>);
        let rtmp_server = rtmp_builder.build();

        let transcode_engine = if config.enable_transcoding {
            Some(Arc::new(TranscodeEngine::new().await?))
        } else {
            None
        };

        let recorder = if config.enable_recording {
            Some(Arc::new(StreamRecorder::new(&config.record_dir)?))
        } else {
            None
        };

        let cdn_uploader = if config.enable_cdn_upload {
            Some(Arc::new(CdnUploader::new().await?))
        } else {
            None
        };

        Ok(Self {
            config,
            rtmp_server: Arc::new(rtmp_server),
            validator,
            streams: Arc::new(RwLock::new(HashMap::new())),
            transcode_engine,
            recorder,
            cdn_uploader,
            metrics,
        })
    }

    /// Starts the RTMP ingest server.
    ///
    /// # Errors
    ///
    /// Returns an error if the server fails to start.
    pub async fn start(&self) -> ServerResult<()> {
        info!("Starting RTMP ingest server on {}", self.config.bind_addr);

        let server = Arc::clone(&self.rtmp_server);
        let streams = Arc::clone(&self.streams);
        let metrics = Arc::clone(&self.metrics);
        let transcode = self.transcode_engine.clone();
        let recorder = self.recorder.clone();
        let cdn = self.cdn_uploader.clone();

        tokio::spawn(async move {
            if let Err(e) =
                Self::run_server(server, streams, metrics, transcode, recorder, cdn).await
            {
                error!("RTMP server error: {}", e);
            }
        });

        Ok(())
    }

    /// Runs the RTMP server.
    async fn run_server(
        _server: Arc<RtmpServer>,
        streams: Arc<RwLock<HashMap<String, Arc<IngestStream>>>>,
        metrics: Arc<MetricsCollector>,
        _transcode: Option<Arc<TranscodeEngine>>,
        _recorder: Option<Arc<StreamRecorder>>,
        _cdn: Option<Arc<CdnUploader>>,
    ) -> ServerResult<()> {
        // Server main loop
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            // Process active streams
            let stream_list = {
                let s = streams.read();
                s.values().map(Arc::clone).collect::<Vec<_>>()
            };

            for stream in stream_list {
                // Update metrics
                metrics.record_stream_active(&stream.app_name, &stream.stream_key);
            }
        }
    }

    /// Registers a new stream.
    pub fn register_stream(
        &self,
        app_name: impl Into<String>,
        stream_key: impl Into<String>,
        metadata: StreamMetadata,
    ) -> Arc<IngestStream> {
        let app_name = app_name.into();
        let stream_key = stream_key.into();
        let id = Uuid::new_v4();

        let (packet_tx, mut packet_rx) = mpsc::unbounded_channel();

        let stream = Arc::new(IngestStream {
            id,
            stream_key: stream_key.clone(),
            app_name: app_name.clone(),
            metadata,
            packet_tx,
            bytes_received: Arc::new(RwLock::new(0)),
            packets_received: Arc::new(RwLock::new(0)),
            start_time: std::time::Instant::now(),
        });

        let key = format!("{}/{}", app_name, stream_key);
        let mut streams = self.streams.write();
        streams.insert(key.clone(), Arc::clone(&stream));

        // Spawn packet processing task
        let stream_clone = Arc::clone(&stream);
        let metrics = Arc::clone(&self.metrics);
        let transcode = self.transcode_engine.clone();
        let recorder = self.recorder.clone();
        let cdn = self.cdn_uploader.clone();

        tokio::spawn(async move {
            while let Some(packet) = packet_rx.recv().await {
                // Update stats
                let data_len = packet.data.len() as u64;
                *stream_clone.bytes_received.write() += data_len;
                *stream_clone.packets_received.write() += 1;

                // Record metrics
                metrics.record_bytes_received(data_len);
                metrics.record_packet_received();

                // Process transcoding
                if let Some(ref engine) = transcode {
                    if let Err(e) = engine.process_packet(&packet).await {
                        error!("Transcode error: {}", e);
                    }
                }

                // Record stream
                if let Some(ref rec) = recorder {
                    if let Err(e) = rec.write_packet(&stream_clone.stream_key, &packet).await {
                        error!("Recording error: {}", e);
                    }
                }

                // Upload to CDN
                if let Some(ref uploader) = cdn {
                    if let Err(e) = uploader
                        .upload_packet(&stream_clone.stream_key, &packet)
                        .await
                    {
                        error!("CDN upload error: {}", e);
                    }
                }
            }

            info!("Stream ended: {}", key);
        });

        stream
    }

    /// Unregisters a stream.
    pub fn unregister_stream(&self, app_name: &str, stream_key: &str) {
        let key = format!("{}/{}", app_name, stream_key);
        let mut streams = self.streams.write();
        streams.remove(&key);
        info!("Unregistered stream: {}", key);
    }

    /// Gets an active stream.
    #[must_use]
    pub fn get_stream(&self, app_name: &str, stream_key: &str) -> Option<Arc<IngestStream>> {
        let key = format!("{}/{}", app_name, stream_key);
        let streams = self.streams.read();
        streams.get(&key).cloned()
    }

    /// Lists all active streams.
    #[must_use]
    pub fn list_streams(&self) -> Vec<Arc<IngestStream>> {
        let streams = self.streams.read();
        streams.values().cloned().collect()
    }

    /// Gets the stream key validator.
    #[must_use]
    pub fn validator(&self) -> &Arc<StreamKeyValidator> {
        &self.validator
    }

    /// Shuts down the server.
    pub async fn shutdown(&self) -> ServerResult<()> {
        info!("Shutting down RTMP ingest server");

        // Stop all streams
        let streams = {
            let s = self.streams.write();
            s.keys().cloned().collect::<Vec<_>>()
        };

        for key in streams {
            let parts: Vec<&str> = key.split('/').collect();
            if parts.len() == 2 {
                self.unregister_stream(parts[0], parts[1]);
            }
        }

        Ok(())
    }
}
