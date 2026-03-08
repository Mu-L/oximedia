#![allow(dead_code)]
//! Signal presence and health monitoring for routed signals.
//!
//! Provides [`SignalStatus`], [`SignalMonitor`], and [`MonitorReport`] for
//! tracking whether routed signals are active and within spec.

use std::collections::HashMap;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Signal status
// ---------------------------------------------------------------------------

/// The health status of a monitored signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalStatus {
    /// Signal is present and within specification.
    Ok,
    /// Signal is present but a metric is out of tolerance.
    Warning,
    /// Signal loss detected.
    Lost,
    /// Monitoring has not yet produced a result for this signal.
    Unknown,
}

impl SignalStatus {
    /// Returns a short human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warning => "warning",
            Self::Lost => "lost",
            Self::Unknown => "unknown",
        }
    }

    /// Returns `true` if the signal requires operator attention.
    pub fn needs_attention(&self) -> bool {
        matches!(self, Self::Warning | Self::Lost)
    }

    /// Returns `true` if the signal is operational.
    pub fn is_operational(&self) -> bool {
        matches!(self, Self::Ok)
    }
}

// ---------------------------------------------------------------------------
// Signal sample
// ---------------------------------------------------------------------------

/// A single measurement sample for a monitored signal.
#[derive(Debug, Clone)]
pub struct SignalSample {
    /// Signal level in dBFS (audio) or dBmV (video).
    pub level_db: f32,
    /// Signal-to-noise ratio in dB.
    pub snr_db: f32,
    /// Timestamp of the sample.
    pub timestamp: Instant,
}

impl SignalSample {
    /// Creates a new sample taken at the current instant.
    pub fn now(level_db: f32, snr_db: f32) -> Self {
        Self {
            level_db,
            snr_db,
            timestamp: Instant::now(),
        }
    }

    /// Returns the age of this sample.
    pub fn age(&self) -> Duration {
        self.timestamp.elapsed()
    }
}

// ---------------------------------------------------------------------------
// Monitor entry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct MonitorEntry {
    port_name: String,
    status: SignalStatus,
    last_sample: Option<SignalSample>,
    fault_count: u32,
    min_level_db: f32,
    max_snr_db: f32,
}

impl MonitorEntry {
    fn new(port_name: impl Into<String>, min_level_db: f32, max_snr_db: f32) -> Self {
        Self {
            port_name: port_name.into(),
            status: SignalStatus::Unknown,
            last_sample: None,
            fault_count: 0,
            min_level_db,
            max_snr_db,
        }
    }

    fn update(&mut self, sample: SignalSample) {
        let level_ok = sample.level_db >= self.min_level_db;
        let snr_ok = sample.snr_db <= self.max_snr_db;

        self.status = match (level_ok, snr_ok) {
            (true, true) => SignalStatus::Ok,
            (false, _) => {
                self.fault_count += 1;
                SignalStatus::Lost
            }
            (true, false) => SignalStatus::Warning,
        };
        self.last_sample = Some(sample);
    }
}

// ---------------------------------------------------------------------------
// SignalMonitor
// ---------------------------------------------------------------------------

/// Monitors the health of a set of named signal ports.
#[derive(Debug, Default)]
pub struct SignalMonitor {
    entries: HashMap<String, MonitorEntry>,
}

impl SignalMonitor {
    /// Creates an empty monitor.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a port for monitoring.
    ///
    /// `min_level_db` — minimum acceptable level (e.g., -60 dBFS).
    /// `max_snr_threshold_db` — maximum SNR value considered a warning
    ///   (i.e., SNR above this value is considered noisy/reversed — set
    ///   very high to effectively disable, e.g., 100.0).
    pub fn register_port(
        &mut self,
        port_name: impl Into<String>,
        min_level_db: f32,
        max_snr_threshold_db: f32,
    ) {
        let name = port_name.into();
        self.entries.insert(
            name.clone(),
            MonitorEntry::new(name, min_level_db, max_snr_threshold_db),
        );
    }

    /// Submits a measurement sample for the named port.
    ///
    /// Returns `None` if the port is not registered.
    pub fn submit(&mut self, port_name: &str, sample: SignalSample) -> Option<SignalStatus> {
        let entry = self.entries.get_mut(port_name)?;
        entry.update(sample);
        Some(entry.status)
    }

    /// Returns the current status of the named port.
    pub fn status(&self, port_name: &str) -> SignalStatus {
        self.entries
            .get(port_name)
            .map(|e| e.status)
            .unwrap_or(SignalStatus::Unknown)
    }

    /// Returns the number of faults recorded for a port.
    pub fn fault_count(&self, port_name: &str) -> u32 {
        self.entries
            .get(port_name)
            .map(|e| e.fault_count)
            .unwrap_or(0)
    }

    /// Returns the last sample for a port, if any.
    pub fn last_sample(&self, port_name: &str) -> Option<&SignalSample> {
        self.entries.get(port_name)?.last_sample.as_ref()
    }

    /// Returns the number of registered ports.
    pub fn port_count(&self) -> usize {
        self.entries.len()
    }

    /// Generates a summary report.
    pub fn report(&self) -> MonitorReport {
        let mut ok = 0usize;
        let mut warning = 0usize;
        let mut lost = 0usize;
        let mut unknown = 0usize;

        for entry in self.entries.values() {
            match entry.status {
                SignalStatus::Ok => ok += 1,
                SignalStatus::Warning => warning += 1,
                SignalStatus::Lost => lost += 1,
                SignalStatus::Unknown => unknown += 1,
            }
        }

        MonitorReport {
            total: self.entries.len(),
            ok,
            warning,
            lost,
            unknown,
        }
    }
}

// ---------------------------------------------------------------------------
// MonitorReport
// ---------------------------------------------------------------------------

/// Summary report from a [`SignalMonitor`] poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorReport {
    /// Total number of monitored ports.
    pub total: usize,
    /// Ports with `Ok` status.
    pub ok: usize,
    /// Ports with `Warning` status.
    pub warning: usize,
    /// Ports with `Lost` status.
    pub lost: usize,
    /// Ports with `Unknown` status.
    pub unknown: usize,
}

impl MonitorReport {
    /// Returns `true` if all ports are `Ok`.
    pub fn all_ok(&self) -> bool {
        self.total > 0 && self.ok == self.total
    }

    /// Returns `true` if any port needs operator attention.
    pub fn has_alerts(&self) -> bool {
        self.warning > 0 || self.lost > 0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_monitor() -> SignalMonitor {
        let mut m = SignalMonitor::new();
        // min_level = -60 dBFS, max_snr = 100.0 (effectively disabled)
        m.register_port("cam1", -60.0, 100.0);
        m.register_port("cam2", -60.0, 100.0);
        m
    }

    #[test]
    fn test_initial_status_unknown() {
        let m = make_monitor();
        assert_eq!(m.status("cam1"), SignalStatus::Unknown);
    }

    #[test]
    fn test_unregistered_port_unknown() {
        let m = make_monitor();
        assert_eq!(m.status("no_such_port"), SignalStatus::Unknown);
    }

    #[test]
    fn test_submit_ok_sample() {
        let mut m = make_monitor();
        let s = m.submit("cam1", SignalSample::now(-20.0, 50.0));
        assert_eq!(s, Some(SignalStatus::Ok));
        assert_eq!(m.status("cam1"), SignalStatus::Ok);
    }

    #[test]
    fn test_submit_lost_below_min_level() {
        let mut m = make_monitor();
        // Level below -60 dBFS → Lost
        let s = m.submit("cam1", SignalSample::now(-80.0, 50.0));
        assert_eq!(s, Some(SignalStatus::Lost));
    }

    #[test]
    fn test_submit_warning_snr_exceeded() {
        let mut m = SignalMonitor::new();
        // max_snr = 30 → any SNR above 30 triggers Warning
        m.register_port("mic", -60.0, 30.0);
        let s = m.submit("mic", SignalSample::now(-10.0, 35.0));
        assert_eq!(s, Some(SignalStatus::Warning));
    }

    #[test]
    fn test_submit_unregistered_returns_none() {
        let mut m = SignalMonitor::new();
        let result = m.submit("ghost", SignalSample::now(0.0, 60.0));
        assert!(result.is_none());
    }

    #[test]
    fn test_fault_count_increments_on_loss() {
        let mut m = make_monitor();
        m.submit("cam1", SignalSample::now(-80.0, 50.0));
        m.submit("cam1", SignalSample::now(-90.0, 50.0));
        assert_eq!(m.fault_count("cam1"), 2);
    }

    #[test]
    fn test_fault_count_zero_on_ok() {
        let mut m = make_monitor();
        m.submit("cam1", SignalSample::now(-10.0, 50.0));
        assert_eq!(m.fault_count("cam1"), 0);
    }

    #[test]
    fn test_last_sample_stored() {
        let mut m = make_monitor();
        m.submit("cam1", SignalSample::now(-15.0, 40.0));
        let sample = m.last_sample("cam1");
        assert!(sample.is_some());
        assert!((sample.expect("should succeed in test").level_db - (-15.0)).abs() < 0.001);
    }

    #[test]
    fn test_port_count() {
        let m = make_monitor();
        assert_eq!(m.port_count(), 2);
    }

    #[test]
    fn test_report_all_ok() {
        let mut m = make_monitor();
        m.submit("cam1", SignalSample::now(-10.0, 40.0));
        m.submit("cam2", SignalSample::now(-5.0, 35.0));
        let report = m.report();
        assert!(report.all_ok());
        assert!(!report.has_alerts());
    }

    #[test]
    fn test_report_has_alerts_on_loss() {
        let mut m = make_monitor();
        m.submit("cam1", SignalSample::now(-80.0, 40.0));
        let report = m.report();
        assert!(report.has_alerts());
        assert_eq!(report.lost, 1);
    }

    #[test]
    fn test_signal_status_label() {
        assert_eq!(SignalStatus::Ok.label(), "ok");
        assert_eq!(SignalStatus::Warning.label(), "warning");
        assert_eq!(SignalStatus::Lost.label(), "lost");
        assert_eq!(SignalStatus::Unknown.label(), "unknown");
    }

    #[test]
    fn test_signal_status_needs_attention() {
        assert!(!SignalStatus::Ok.needs_attention());
        assert!(SignalStatus::Warning.needs_attention());
        assert!(SignalStatus::Lost.needs_attention());
        assert!(!SignalStatus::Unknown.needs_attention());
    }

    #[test]
    fn test_signal_status_is_operational() {
        assert!(SignalStatus::Ok.is_operational());
        assert!(!SignalStatus::Warning.is_operational());
        assert!(!SignalStatus::Lost.is_operational());
    }

    #[test]
    fn test_report_unknown_before_submission() {
        let m = make_monitor();
        let report = m.report();
        assert_eq!(report.unknown, 2);
        assert!(!report.all_ok());
    }
}
