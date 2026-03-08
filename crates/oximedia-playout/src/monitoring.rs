//! System monitoring and alerting
//!
//! Provides status monitoring, on-air indicators, next-up display,
//! waveform/vectorscope, audio meters, and alert system.

use crate::{PlayoutError, Result};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::Arc;

/// Monitor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    /// HTTP server port for web interface
    pub port: u16,

    /// Enable audio meters
    pub audio_meters: bool,

    /// Enable waveform display
    pub waveform: bool,

    /// Enable vectorscope
    pub vectorscope: bool,

    /// Alert history size
    pub alert_history_size: usize,

    /// Metrics retention period in seconds
    pub metrics_retention_seconds: u64,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            audio_meters: true,
            waveform: false,
            vectorscope: false,
            alert_history_size: 100,
            metrics_retention_seconds: 3600,
        }
    }
}

/// System status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemStatus {
    /// System is offline
    Offline,
    /// System is starting up
    Starting,
    /// System is online and operating normally
    Online,
    /// System has warnings
    Warning,
    /// System has errors
    Error,
    /// System is in emergency fallback mode
    Fallback,
}

/// On-air status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnAirStatus {
    /// Is currently on air
    pub on_air: bool,

    /// Current program name
    pub current_program: Option<String>,

    /// Current item name
    pub current_item: Option<String>,

    /// Time on air (seconds)
    pub time_on_air: u64,

    /// Current timecode
    pub timecode: String,

    /// Frame number
    pub frame_number: u64,
}

impl Default for OnAirStatus {
    fn default() -> Self {
        Self {
            on_air: false,
            current_program: None,
            current_item: None,
            time_on_air: 0,
            timecode: "00:00:00:00".to_string(),
            frame_number: 0,
        }
    }
}

/// Next-up information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextUpInfo {
    /// Next item name
    pub name: String,

    /// Duration in seconds
    pub duration_seconds: u64,

    /// Scheduled start time
    pub scheduled_time: DateTime<Utc>,

    /// Countdown in seconds
    pub countdown_seconds: i64,
}

/// Audio meter levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioMeters {
    /// Peak levels per channel (dBFS)
    pub peak_dbfs: Vec<f32>,

    /// RMS levels per channel (dBFS)
    pub rms_dbfs: Vec<f32>,

    /// True peak flag per channel
    pub true_peak: Vec<bool>,

    /// Loudness (LUFS)
    pub loudness_lufs: f32,

    /// Dynamic range
    pub dynamic_range_db: f32,
}

impl Default for AudioMeters {
    fn default() -> Self {
        Self {
            peak_dbfs: vec![-60.0; 2],
            rms_dbfs: vec![-60.0; 2],
            true_peak: vec![false; 2],
            loudness_lufs: -23.0,
            dynamic_range_db: 20.0,
        }
    }
}

/// Waveform data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveformData {
    /// Luma values (Y)
    pub luma: Vec<u32>,

    /// Chroma Cb values
    pub cb: Vec<u32>,

    /// Chroma Cr values
    pub cr: Vec<u32>,
}

impl Default for WaveformData {
    fn default() -> Self {
        Self {
            luma: vec![0; 256],
            cb: vec![0; 256],
            cr: vec![0; 256],
        }
    }
}

/// Vectorscope data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorscopeData {
    /// U/Cb values
    pub u_values: Vec<i16>,

    /// V/Cr values
    pub v_values: Vec<i16>,

    /// Intensity map
    pub intensity: Vec<Vec<u8>>,
}

impl Default for VectorscopeData {
    fn default() -> Self {
        Self {
            u_values: Vec::new(),
            v_values: Vec::new(),
            intensity: vec![vec![0; 256]; 256],
        }
    }
}

/// Alert severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Alert type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertType {
    /// Video signal lost
    VideoSignalLost,
    /// Audio signal lost
    AudioSignalLost,
    /// Frame drop detected
    FrameDrop,
    /// Buffer underrun
    BufferUnderrun,
    /// Clock drift detected
    ClockDrift,
    /// Output failure
    OutputFailure,
    /// Genlock lost
    GenlockLost,
    /// Disk space low
    DiskSpaceLow,
    /// Emergency fallback activated
    EmergencyFallback,
    /// Network error
    NetworkError,
    /// Custom alert
    Custom(String),
}

/// Alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// Alert ID
    pub id: u64,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Severity
    pub severity: AlertSeverity,

    /// Alert type
    pub alert_type: AlertType,

    /// Message
    pub message: String,

    /// Acknowledged flag
    pub acknowledged: bool,

    /// Cleared flag
    pub cleared: bool,
}

impl Alert {
    /// Create a new alert
    pub fn new(severity: AlertSeverity, alert_type: AlertType, message: String) -> Self {
        Self {
            id: 0, // Will be set by monitor
            timestamp: Utc::now(),
            severity,
            alert_type,
            message,
            acknowledged: false,
            cleared: false,
        }
    }
}

/// Performance metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// CPU usage percentage
    pub cpu_usage_percent: f32,

    /// Memory usage in MB
    pub memory_usage_mb: u64,

    /// Disk usage percentage
    pub disk_usage_percent: f32,

    /// Network throughput in Mbps
    pub network_throughput_mbps: f32,

    /// Frame rate
    pub frame_rate: f32,

    /// Dropped frames
    pub dropped_frames: u64,

    /// Buffer level percentage
    pub buffer_level_percent: f32,
}

/// Monitoring dashboard data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    /// System status
    pub system_status: SystemStatus,

    /// On-air status
    pub on_air: OnAirStatus,

    /// Next-up information
    pub next_up: Option<NextUpInfo>,

    /// Audio meters
    pub audio_meters: AudioMeters,

    /// Performance metrics
    pub metrics: PerformanceMetrics,

    /// Active alerts count
    pub active_alerts: usize,

    /// Last update timestamp
    pub last_update: DateTime<Utc>,
}

impl Default for DashboardData {
    fn default() -> Self {
        Self {
            system_status: SystemStatus::Offline,
            on_air: OnAirStatus::default(),
            next_up: None,
            audio_meters: AudioMeters::default(),
            metrics: PerformanceMetrics::default(),
            active_alerts: 0,
            last_update: Utc::now(),
        }
    }
}

/// Internal monitoring state
struct MonitorState {
    /// System status
    status: SystemStatus,

    /// On-air status
    on_air: OnAirStatus,

    /// Next-up queue
    next_up_queue: VecDeque<NextUpInfo>,

    /// Audio meters
    audio_meters: AudioMeters,

    /// Waveform data
    waveform: WaveformData,

    /// Vectorscope data
    vectorscope: VectorscopeData,

    /// Performance metrics
    metrics: PerformanceMetrics,

    /// Alert history
    alerts: VecDeque<Alert>,

    /// Next alert ID
    next_alert_id: u64,

    /// Metrics history
    metrics_history: VecDeque<(DateTime<Utc>, PerformanceMetrics)>,
}

/// System monitor
pub struct Monitor {
    config: MonitorConfig,
    state: Arc<RwLock<MonitorState>>,
}

impl Monitor {
    /// Create a new monitor
    pub fn new(config: MonitorConfig) -> Result<Self> {
        let state = MonitorState {
            status: SystemStatus::Offline,
            on_air: OnAirStatus::default(),
            next_up_queue: VecDeque::new(),
            audio_meters: AudioMeters::default(),
            waveform: WaveformData::default(),
            vectorscope: VectorscopeData::default(),
            metrics: PerformanceMetrics::default(),
            alerts: VecDeque::new(),
            next_alert_id: 1,
            metrics_history: VecDeque::new(),
        };

        Ok(Self {
            config,
            state: Arc::new(RwLock::new(state)),
        })
    }

    /// Update system status
    pub fn update_status(&self, status: SystemStatus) {
        self.state.write().status = status;
    }

    /// Get system status
    pub fn get_status(&self) -> SystemStatus {
        self.state.read().status
    }

    /// Update on-air status
    pub fn update_on_air(&self, on_air: OnAirStatus) {
        self.state.write().on_air = on_air;
    }

    /// Get on-air status
    pub fn get_on_air(&self) -> OnAirStatus {
        self.state.read().on_air.clone()
    }

    /// Add next-up item
    pub fn add_next_up(&self, info: NextUpInfo) {
        let mut state = self.state.write();
        state.next_up_queue.push_back(info);

        // Limit queue size
        while state.next_up_queue.len() > 10 {
            state.next_up_queue.pop_front();
        }
    }

    /// Get next-up items
    pub fn get_next_up(&self) -> Vec<NextUpInfo> {
        self.state.read().next_up_queue.iter().cloned().collect()
    }

    /// Clear next-up queue
    pub fn clear_next_up(&self) {
        self.state.write().next_up_queue.clear();
    }

    /// Update audio meters
    pub fn update_audio_meters(&self, meters: AudioMeters) {
        self.state.write().audio_meters = meters;
    }

    /// Get audio meters
    pub fn get_audio_meters(&self) -> AudioMeters {
        self.state.read().audio_meters.clone()
    }

    /// Update waveform data
    pub fn update_waveform(&self, waveform: WaveformData) {
        if self.config.waveform {
            self.state.write().waveform = waveform;
        }
    }

    /// Get waveform data
    pub fn get_waveform(&self) -> WaveformData {
        self.state.read().waveform.clone()
    }

    /// Update vectorscope data
    pub fn update_vectorscope(&self, vectorscope: VectorscopeData) {
        if self.config.vectorscope {
            self.state.write().vectorscope = vectorscope;
        }
    }

    /// Get vectorscope data
    pub fn get_vectorscope(&self) -> VectorscopeData {
        self.state.read().vectorscope.clone()
    }

    /// Update performance metrics
    pub fn update_metrics(&self, metrics: PerformanceMetrics) {
        let mut state = self.state.write();
        state.metrics = metrics.clone();

        // Add to history
        state.metrics_history.push_back((Utc::now(), metrics));

        // Trim old metrics
        let retention = chrono::Duration::seconds(self.config.metrics_retention_seconds as i64);
        let cutoff = Utc::now() - retention;

        while let Some((timestamp, _)) = state.metrics_history.front() {
            if *timestamp < cutoff {
                state.metrics_history.pop_front();
            } else {
                break;
            }
        }
    }

    /// Get performance metrics
    pub fn get_metrics(&self) -> PerformanceMetrics {
        self.state.read().metrics.clone()
    }

    /// Get metrics history
    pub fn get_metrics_history(&self) -> Vec<(DateTime<Utc>, PerformanceMetrics)> {
        self.state.read().metrics_history.iter().cloned().collect()
    }

    /// Raise an alert
    pub fn raise_alert(&self, mut alert: Alert) -> u64 {
        let mut state = self.state.write();
        alert.id = state.next_alert_id;
        state.next_alert_id += 1;

        let alert_id = alert.id;
        state.alerts.push_back(alert);

        // Limit alert history
        while state.alerts.len() > self.config.alert_history_size {
            state.alerts.pop_front();
        }

        // Update system status based on alert severity
        match state
            .alerts
            .back()
            .expect("invariant: alerts non-empty (just pushed above)")
            .severity
        {
            AlertSeverity::Critical | AlertSeverity::Error => {
                if state.status != SystemStatus::Fallback {
                    state.status = SystemStatus::Error;
                }
            }
            AlertSeverity::Warning => {
                if state.status == SystemStatus::Online {
                    state.status = SystemStatus::Warning;
                }
            }
            _ => {}
        }

        alert_id
    }

    /// Acknowledge an alert
    pub fn acknowledge_alert(&self, alert_id: u64) -> Result<()> {
        let mut state = self.state.write();
        if let Some(alert) = state.alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.acknowledged = true;
            Ok(())
        } else {
            Err(PlayoutError::Monitoring(format!(
                "Alert not found: {alert_id}"
            )))
        }
    }

    /// Clear an alert
    pub fn clear_alert(&self, alert_id: u64) -> Result<()> {
        let mut state = self.state.write();
        if let Some(alert) = state.alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.cleared = true;
            Ok(())
        } else {
            Err(PlayoutError::Monitoring(format!(
                "Alert not found: {alert_id}"
            )))
        }
    }

    /// Get active alerts
    pub fn get_active_alerts(&self) -> Vec<Alert> {
        self.state
            .read()
            .alerts
            .iter()
            .filter(|alert| !alert.cleared)
            .cloned()
            .collect()
    }

    /// Get all alerts
    pub fn get_all_alerts(&self) -> Vec<Alert> {
        self.state.read().alerts.iter().cloned().collect()
    }

    /// Clear all alerts
    pub fn clear_all_alerts(&self) {
        let mut state = self.state.write();
        for alert in &mut state.alerts {
            alert.cleared = true;
        }
    }

    /// Get dashboard data
    pub fn get_dashboard_data(&self) -> DashboardData {
        let state = self.state.read();

        DashboardData {
            system_status: state.status,
            on_air: state.on_air.clone(),
            next_up: state.next_up_queue.front().cloned(),
            audio_meters: state.audio_meters.clone(),
            metrics: state.metrics.clone(),
            active_alerts: state.alerts.iter().filter(|a| !a.cleared).count(),
            last_update: Utc::now(),
        }
    }

    /// Start HTTP monitoring server.
    ///
    /// Binds a `TcpListener` on `0.0.0.0:<port>` and spawns a blocking
    /// thread to handle connections.  Three endpoints are served:
    ///
    /// - `GET /health`  → `{"status":"ok"}` (JSON)
    /// - `GET /status`  → JSON dashboard snapshot
    /// - `GET /metrics` → Prometheus text-format metrics
    ///
    /// The server runs until the process exits (no graceful shutdown handle
    /// is stored, keeping the implementation dependency-free).
    pub async fn start_server(&self) -> Result<()> {
        let port = self.config.port;
        tracing::info!("Monitoring server starting on port {}", port);

        let addr = format!("0.0.0.0:{port}");
        let listener = TcpListener::bind(&addr)
            .map_err(|e| PlayoutError::Monitoring(format!("Failed to bind {addr}:{e}")))?;

        // Clone the shared state so the background thread can read it.
        let state = Arc::clone(&self.state);

        std::thread::Builder::new()
            .name("oximedia-monitor-http".to_string())
            .spawn(move || {
                tracing::info!("HTTP monitoring server listening on {}", addr);
                for stream in listener.incoming() {
                    match stream {
                        Ok(mut stream) => {
                            // Read the first request line (e.g. "GET /health HTTP/1.1")
                            let request_line = {
                                let mut reader = BufReader::new(&stream);
                                let mut line = String::new();
                                let _ = reader.read_line(&mut line);
                                line
                            };

                            let path = request_line
                                .split_whitespace()
                                .nth(1)
                                .unwrap_or("/")
                                .to_string();

                            // Build response body
                            let (content_type, body) = match path.as_str() {
                                "/health" => (
                                    "application/json",
                                    r#"{"status":"ok"}"#.to_string(),
                                ),
                                "/status" => {
                                    let st = state.read();
                                    let dashboard = DashboardData {
                                        system_status: st.status,
                                        on_air: st.on_air.clone(),
                                        next_up: st.next_up_queue.front().cloned(),
                                        audio_meters: st.audio_meters.clone(),
                                        metrics: st.metrics.clone(),
                                        active_alerts: st.alerts.iter().filter(|a| !a.cleared).count(),
                                        last_update: Utc::now(),
                                    };
                                    drop(st);
                                    let json = serde_json::to_string_pretty(&dashboard)
                                        .unwrap_or_else(|_| r#"{"error":"serialization failed"}"#.to_string());
                                    ("application/json", json)
                                }
                                "/metrics" => {
                                    let st = state.read();
                                    let metrics_text = format!(
                                        "# HELP oximedia_cpu_usage CPU usage percentage\n\
                                         # TYPE oximedia_cpu_usage gauge\n\
                                         oximedia_cpu_usage {:.2}\n\
                                         # HELP oximedia_memory_usage_mb Memory usage in MiB\n\
                                         # TYPE oximedia_memory_usage_mb gauge\n\
                                         oximedia_memory_usage_mb {}\n\
                                         # HELP oximedia_dropped_frames Total dropped frames\n\
                                         # TYPE oximedia_dropped_frames counter\n\
                                         oximedia_dropped_frames {}\n\
                                         # HELP oximedia_buffer_level_percent Buffer level percentage\n\
                                         # TYPE oximedia_buffer_level_percent gauge\n\
                                         oximedia_buffer_level_percent {:.2}\n\
                                         # HELP oximedia_frame_rate Current frame rate\n\
                                         # TYPE oximedia_frame_rate gauge\n\
                                         oximedia_frame_rate {:.2}\n\
                                         # HELP oximedia_active_alerts Number of active (uncleared) alerts\n\
                                         # TYPE oximedia_active_alerts gauge\n\
                                         oximedia_active_alerts {}\n",
                                        st.metrics.cpu_usage_percent,
                                        st.metrics.memory_usage_mb,
                                        st.metrics.dropped_frames,
                                        st.metrics.buffer_level_percent,
                                        st.metrics.frame_rate,
                                        st.alerts.iter().filter(|a| !a.cleared).count(),
                                    );
                                    drop(st);
                                    ("text/plain; version=0.0.4", metrics_text)
                                }
                                _ => (
                                    "application/json",
                                    r#"{"error":"not found"}"#.to_string(),
                                ),
                            };

                            let status_line = if path == "/health"
                                || path == "/status"
                                || path == "/metrics"
                            {
                                "HTTP/1.1 200 OK"
                            } else {
                                "HTTP/1.1 404 Not Found"
                            };

                            let response = format!(
                                "{}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                status_line,
                                content_type,
                                body.len(),
                                body
                            );

                            let _ = stream.write_all(response.as_bytes());
                        }
                        Err(e) => {
                            tracing::warn!("Monitoring HTTP accept error: {}", e);
                        }
                    }
                }
            })
            .map_err(|e| PlayoutError::Monitoring(format!("Failed to spawn HTTP thread: {e}")))?;

        Ok(())
    }

    /// Stop monitoring server
    pub async fn stop_server(&self) -> Result<()> {
        tracing::info!("Monitoring server stopping");
        Ok(())
    }

    /// Export metrics to JSON
    pub fn export_metrics(&self) -> Result<String> {
        let data = self.get_dashboard_data();
        serde_json::to_string_pretty(&data)
            .map_err(|e| PlayoutError::Monitoring(format!("Export failed: {e}")))
    }

    /// Health check
    pub fn health_check(&self) -> HashMap<String, String> {
        let state = self.state.read();
        let mut health = HashMap::new();

        health.insert("status".to_string(), format!("{:?}", state.status));
        health.insert("on_air".to_string(), state.on_air.on_air.to_string());
        health.insert(
            "active_alerts".to_string(),
            state
                .alerts
                .iter()
                .filter(|a| !a.cleared)
                .count()
                .to_string(),
        );
        health.insert(
            "cpu_usage".to_string(),
            format!("{}%", state.metrics.cpu_usage_percent),
        );
        health.insert(
            "memory_usage".to_string(),
            format!("{}MB", state.metrics.memory_usage_mb),
        );

        health
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_creation() {
        let config = MonitorConfig::default();
        let monitor = Monitor::new(config).expect("should succeed in test");
        assert_eq!(monitor.get_status(), SystemStatus::Offline);
    }

    #[test]
    fn test_status_update() {
        let monitor = Monitor::new(MonitorConfig::default()).expect("should succeed in test");
        monitor.update_status(SystemStatus::Online);
        assert_eq!(monitor.get_status(), SystemStatus::Online);
    }

    #[test]
    fn test_alert_system() {
        let monitor = Monitor::new(MonitorConfig::default()).expect("should succeed in test");

        let alert = Alert::new(
            AlertSeverity::Warning,
            AlertType::FrameDrop,
            "Frame dropped".to_string(),
        );

        let alert_id = monitor.raise_alert(alert);
        assert!(alert_id > 0);

        let active_alerts = monitor.get_active_alerts();
        assert_eq!(active_alerts.len(), 1);

        monitor
            .acknowledge_alert(alert_id)
            .expect("should succeed in test");
        monitor
            .clear_alert(alert_id)
            .expect("should succeed in test");
    }

    #[test]
    fn test_next_up_queue() {
        let monitor = Monitor::new(MonitorConfig::default()).expect("should succeed in test");

        let next_up = NextUpInfo {
            name: "Test Item".to_string(),
            duration_seconds: 120,
            scheduled_time: Utc::now(),
            countdown_seconds: 60,
        };

        monitor.add_next_up(next_up);
        let queue = monitor.get_next_up();
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_audio_meters() {
        let monitor = Monitor::new(MonitorConfig::default()).expect("should succeed in test");

        let mut meters = AudioMeters::default();
        meters.peak_dbfs = vec![-12.0, -12.0];
        meters.loudness_lufs = -23.0;

        monitor.update_audio_meters(meters.clone());
        let retrieved = monitor.get_audio_meters();

        assert_eq!(retrieved.peak_dbfs, meters.peak_dbfs);
    }

    #[test]
    fn test_dashboard_data() {
        let monitor = Monitor::new(MonitorConfig::default()).expect("should succeed in test");
        monitor.update_status(SystemStatus::Online);

        let dashboard = monitor.get_dashboard_data();
        assert_eq!(dashboard.system_status, SystemStatus::Online);
    }

    #[test]
    fn test_metrics_history() {
        let monitor = Monitor::new(MonitorConfig::default()).expect("should succeed in test");

        let metrics = PerformanceMetrics {
            cpu_usage_percent: 45.0,
            memory_usage_mb: 2048,
            ..Default::default()
        };

        monitor.update_metrics(metrics);
        let history = monitor.get_metrics_history();
        assert_eq!(history.len(), 1);
    }

    #[test]
    fn test_health_check() {
        let monitor = Monitor::new(MonitorConfig::default()).expect("should succeed in test");
        let health = monitor.health_check();

        assert!(health.contains_key("status"));
        assert!(health.contains_key("on_air"));
    }

    #[test]
    fn test_alert_severity_ordering() {
        assert!(AlertSeverity::Critical > AlertSeverity::Error);
        assert!(AlertSeverity::Error > AlertSeverity::Warning);
        assert!(AlertSeverity::Warning > AlertSeverity::Info);
    }
}
