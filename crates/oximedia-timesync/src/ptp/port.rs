//! PTP port management.

use super::bmca::PortState as BmcaPortState;
use super::{DelayMechanism, PortIdentity};
use std::time::Duration;

/// PTP port configuration.
#[derive(Debug, Clone)]
pub struct PortConfig {
    /// Port number (1-based)
    pub port_number: u16,
    /// Delay mechanism
    pub delay_mechanism: DelayMechanism,
    /// Announce receipt timeout (number of intervals)
    pub announce_receipt_timeout: u8,
    /// Sync receipt timeout
    pub sync_receipt_timeout: u8,
    /// Delay request interval (log2 seconds)
    pub delay_request_interval: i8,
    /// Announce interval (log2 seconds)
    pub announce_interval: i8,
    /// Sync interval (log2 seconds)
    pub sync_interval: i8,
}

impl Default for PortConfig {
    fn default() -> Self {
        Self {
            port_number: 1,
            delay_mechanism: DelayMechanism::E2E,
            announce_receipt_timeout: 3,
            sync_receipt_timeout: 3,
            delay_request_interval: 0, // 1 second
            announce_interval: 1,      // 2 seconds
            sync_interval: 0,          // 1 second
        }
    }
}

impl PortConfig {
    /// Get announce interval as duration.
    #[must_use]
    pub fn announce_interval_duration(&self) -> Duration {
        interval_to_duration(self.announce_interval)
    }

    /// Get sync interval as duration.
    #[must_use]
    pub fn sync_interval_duration(&self) -> Duration {
        interval_to_duration(self.sync_interval)
    }

    /// Get delay request interval as duration.
    #[must_use]
    pub fn delay_request_interval_duration(&self) -> Duration {
        interval_to_duration(self.delay_request_interval)
    }

    /// Get announce receipt timeout as duration.
    #[must_use]
    pub fn announce_timeout_duration(&self) -> Duration {
        self.announce_interval_duration()
            .mul_f64(f64::from(self.announce_receipt_timeout))
    }
}

/// Convert log2 interval to duration.
fn interval_to_duration(log_interval: i8) -> Duration {
    if log_interval >= 0 {
        Duration::from_secs(1 << log_interval)
    } else {
        Duration::from_millis(1000 >> (-log_interval))
    }
}

/// PTP port runtime state.
#[derive(Debug)]
pub struct PtpPortState {
    /// Port identity
    pub identity: PortIdentity,
    /// Current state
    pub state: BmcaPortState,
    /// Configuration
    pub config: PortConfig,
    /// Last sync sequence ID
    pub last_sync_seq: Option<u16>,
    /// Last announce sequence ID
    pub last_announce_seq: Option<u16>,
}

impl PtpPortState {
    /// Create a new port state.
    #[must_use]
    pub fn new(identity: PortIdentity, config: PortConfig) -> Self {
        Self {
            identity,
            state: BmcaPortState::Initializing,
            config,
            last_sync_seq: None,
            last_announce_seq: None,
        }
    }

    /// Update state.
    pub fn set_state(&mut self, state: BmcaPortState) {
        self.state = state;
    }

    /// Check if port is master.
    #[must_use]
    pub fn is_master(&self) -> bool {
        self.state == BmcaPortState::Master
    }

    /// Check if port is slave.
    #[must_use]
    pub fn is_slave(&self) -> bool {
        self.state == BmcaPortState::Slave
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ptp::ClockIdentity;

    #[test]
    fn test_interval_to_duration() {
        assert_eq!(interval_to_duration(0), Duration::from_secs(1));
        assert_eq!(interval_to_duration(1), Duration::from_secs(2));
        assert_eq!(interval_to_duration(2), Duration::from_secs(4));
        assert_eq!(interval_to_duration(-1), Duration::from_millis(500));
        assert_eq!(interval_to_duration(-2), Duration::from_millis(250));
    }

    #[test]
    fn test_port_config_default() {
        let config = PortConfig::default();
        assert_eq!(config.port_number, 1);
        assert_eq!(config.delay_mechanism, DelayMechanism::E2E);
        assert_eq!(config.sync_interval_duration(), Duration::from_secs(1));
    }

    #[test]
    fn test_port_state_creation() {
        let clock_id = ClockIdentity::random();
        let port_id = PortIdentity::new(clock_id, 1);
        let config = PortConfig::default();
        let state = PtpPortState::new(port_id, config);

        assert_eq!(state.identity, port_id);
        assert!(!state.is_master());
        assert!(!state.is_slave());
    }
}
