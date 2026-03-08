//! PTP clock implementations (Ordinary Clock and Boundary Clock).

use super::bmca::{recommend_state, PortState};
use super::dataset::{CurrentDataSet, DefaultDataSet, ParentDataSet, TimePropertiesDataSet};
use super::message::{
    AnnounceMessage, DelayReqMessage, DelayRespMessage, Flags, FollowUpMessage, Header,
    MessageType, SyncMessage,
};
use super::{ClockIdentity, CommunicationMode, Domain, PortIdentity, PtpTimestamp};
use crate::error::{TimeSyncError, TimeSyncResult};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tracing::{debug, info};

/// PTP Ordinary Clock (OC) implementation.
///
/// An ordinary clock has a single PTP port and can operate as master or slave.
pub struct OrdinaryClock {
    /// Default dataset
    default_ds: DefaultDataSet,
    /// Current dataset
    current_ds: CurrentDataSet,
    /// Parent dataset
    parent_ds: ParentDataSet,
    /// Time properties dataset
    time_props_ds: TimePropertiesDataSet,
    /// Port state
    port_state: PortState,
    /// Current master (if slave)
    current_master: Option<PortIdentity>,
    /// Sequence ID counter
    sequence_id: u16,
    /// Socket for communication
    socket: Option<Arc<UdpSocket>>,
    /// Communication mode
    comm_mode: CommunicationMode,
    /// Sync interval (log2 seconds, e.g., 0 = 1s, -1 = 0.5s)
    sync_interval: i8,
    /// Delay request interval
    #[allow(dead_code)]
    delay_req_interval: i8,
    /// Announce interval
    announce_interval: i8,
    /// Received announce messages
    #[allow(dead_code)]
    received_announces: HashMap<PortIdentity, AnnounceMessage>,
}

impl OrdinaryClock {
    /// Create a new ordinary clock.
    #[must_use]
    pub fn new(clock_identity: ClockIdentity, domain: Domain) -> Self {
        let mut default_ds = DefaultDataSet::new(clock_identity);
        default_ds.domain_number = domain.0;

        Self {
            current_ds: CurrentDataSet::default(),
            parent_ds: ParentDataSet::from_local(&default_ds),
            time_props_ds: TimePropertiesDataSet::default(),
            default_ds,
            port_state: PortState::Initializing,
            current_master: None,
            sequence_id: 0,
            socket: None,
            comm_mode: CommunicationMode::Multicast,
            sync_interval: 0, // 1 second
            delay_req_interval: 0,
            announce_interval: 1, // 2 seconds
            received_announces: HashMap::new(),
        }
    }

    /// Set communication mode.
    pub fn set_communication_mode(&mut self, mode: CommunicationMode) {
        self.comm_mode = mode;
    }

    /// Set as slave-only clock.
    pub fn set_slave_only(&mut self) {
        self.default_ds.set_slave_only();
    }

    /// Set as grandmaster-capable clock.
    pub fn set_grandmaster_capable(&mut self, clock_class: u8, accuracy: u8) {
        self.default_ds
            .set_grandmaster_capable(clock_class, accuracy);
    }

    /// Bind to a socket.
    pub async fn bind(&mut self, addr: SocketAddr) -> TimeSyncResult<()> {
        let socket = UdpSocket::bind(addr).await?;
        self.socket = Some(Arc::new(socket));
        info!("PTP clock bound to {}", addr);
        Ok(())
    }

    /// Get current port state.
    #[must_use]
    pub fn port_state(&self) -> PortState {
        self.port_state
    }

    /// Get current clock offset from master (nanoseconds).
    #[must_use]
    pub fn offset_from_master(&self) -> i64 {
        self.current_ds.offset_from_master
    }

    /// Get mean path delay (nanoseconds).
    #[must_use]
    pub fn mean_path_delay(&self) -> i64 {
        self.current_ds.mean_path_delay
    }

    /// Handle received PTP message.
    pub async fn handle_message(&mut self, data: &[u8], src: SocketAddr) -> TimeSyncResult<()> {
        if data.len() < 34 {
            return Err(TimeSyncError::InvalidPacket("Packet too short".to_string()));
        }

        let mut buf = data;
        let header = Header::deserialize(&mut buf)?;

        // Check domain
        if header.domain.0 != self.default_ds.domain_number {
            debug!("Ignoring message from different domain");
            return Ok(());
        }

        match header.message_type {
            MessageType::Sync => {
                let sync = SyncMessage::deserialize(data)?;
                self.handle_sync(sync, src).await?;
            }
            MessageType::FollowUp => {
                let follow_up = FollowUpMessage::deserialize(data)?;
                self.handle_follow_up(follow_up).await?;
            }
            MessageType::DelayReq => {
                let delay_req = DelayReqMessage::deserialize(data)?;
                self.handle_delay_req(delay_req, src).await?;
            }
            MessageType::DelayResp => {
                let delay_resp = DelayRespMessage::deserialize(data)?;
                self.handle_delay_resp(delay_resp).await?;
            }
            MessageType::Announce => {
                let announce = AnnounceMessage::deserialize(data)?;
                self.handle_announce(announce).await?;
            }
            _ => {
                debug!("Unhandled message type: {:?}", header.message_type);
            }
        }

        Ok(())
    }

    /// Send a sync message (master only).
    pub async fn send_sync(&mut self) -> TimeSyncResult<()> {
        if self.port_state != PortState::Master {
            return Ok(());
        }

        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| TimeSyncError::InvalidConfig("Socket not bound".to_string()))?;

        let timestamp = PtpTimestamp::now();
        self.sequence_id = self.sequence_id.wrapping_add(1);

        let port_id = PortIdentity::new(self.default_ds.clock_identity, 1);

        let mut flags = Flags::default();
        flags.two_step = self.default_ds.two_step_flag;

        let header = Header {
            message_type: MessageType::Sync,
            version: 2,
            message_length: 44,
            domain: Domain(self.default_ds.domain_number),
            flags,
            correction_field: 0,
            source_port_identity: port_id,
            sequence_id: self.sequence_id,
            control: 0,
            log_message_interval: self.sync_interval,
        };

        let sync = SyncMessage {
            header,
            origin_timestamp: timestamp,
        };

        let data = sync.serialize()?;

        let dest = match self.comm_mode {
            CommunicationMode::Multicast => {
                "224.0.1.129:319".parse().expect("hardcoded regex is valid")
            }
            CommunicationMode::Unicast(addr) => addr,
        };

        socket.send_to(&data, dest).await?;

        // Send follow-up if two-step
        if self.default_ds.two_step_flag {
            self.send_follow_up(timestamp, self.sequence_id).await?;
        }

        Ok(())
    }

    /// Send announce message.
    pub async fn send_announce(&mut self) -> TimeSyncResult<()> {
        if self.port_state != PortState::Master {
            return Ok(());
        }

        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| TimeSyncError::InvalidConfig("Socket not bound".to_string()))?;

        let timestamp = PtpTimestamp::now();
        self.sequence_id = self.sequence_id.wrapping_add(1);

        let port_id = PortIdentity::new(self.default_ds.clock_identity, 1);

        let header = Header {
            message_type: MessageType::Announce,
            version: 2,
            message_length: 64,
            domain: Domain(self.default_ds.domain_number),
            flags: Flags::default(),
            correction_field: 0,
            source_port_identity: port_id,
            sequence_id: self.sequence_id,
            control: 5,
            log_message_interval: self.announce_interval,
        };

        let announce = AnnounceMessage {
            header,
            origin_timestamp: timestamp,
            current_utc_offset: self.time_props_ds.current_utc_offset,
            grandmaster_priority1: self.default_ds.priority1,
            grandmaster_clock_quality: self.default_ds.clock_quality,
            grandmaster_priority2: self.default_ds.priority2,
            grandmaster_identity: self.default_ds.clock_identity,
            steps_removed: 0,
            time_source: self.time_props_ds.time_source as u8,
        };

        let data = announce.serialize()?;

        let dest = match self.comm_mode {
            CommunicationMode::Multicast => {
                "224.0.1.129:319".parse().expect("hardcoded regex is valid")
            }
            CommunicationMode::Unicast(addr) => addr,
        };

        socket.send_to(&data, dest).await?;
        Ok(())
    }

    async fn send_follow_up(&self, timestamp: PtpTimestamp, seq_id: u16) -> TimeSyncResult<()> {
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| TimeSyncError::InvalidConfig("Socket not bound".to_string()))?;

        let port_id = PortIdentity::new(self.default_ds.clock_identity, 1);

        let header = Header {
            message_type: MessageType::FollowUp,
            version: 2,
            message_length: 44,
            domain: Domain(self.default_ds.domain_number),
            flags: Flags::default(),
            correction_field: 0,
            source_port_identity: port_id,
            sequence_id: seq_id,
            control: 2,
            log_message_interval: self.sync_interval,
        };

        let follow_up = FollowUpMessage {
            header,
            precise_origin_timestamp: timestamp,
        };

        let data = follow_up.serialize()?;

        let dest = match self.comm_mode {
            CommunicationMode::Multicast => {
                "224.0.1.129:319".parse().expect("hardcoded regex is valid")
            }
            CommunicationMode::Unicast(addr) => addr,
        };

        socket.send_to(&data, dest).await?;
        Ok(())
    }

    async fn handle_sync(&mut self, _sync: SyncMessage, _src: SocketAddr) -> TimeSyncResult<()> {
        // Slave functionality: record sync reception time
        if self.port_state == PortState::Slave {
            debug!("Received Sync message");
            // In a full implementation, we would:
            // 1. Record the reception timestamp
            // 2. Wait for Follow_Up to get precise origin timestamp
            // 3. Send Delay_Req to measure path delay
            // 4. Calculate offset from master
        }
        Ok(())
    }

    async fn handle_follow_up(&mut self, _follow_up: FollowUpMessage) -> TimeSyncResult<()> {
        if self.port_state == PortState::Slave {
            debug!("Received Follow_Up message");
            // Calculate offset using timestamps
        }
        Ok(())
    }

    async fn handle_delay_req(
        &mut self,
        delay_req: DelayReqMessage,
        src: SocketAddr,
    ) -> TimeSyncResult<()> {
        if self.port_state != PortState::Master {
            return Ok(());
        }

        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| TimeSyncError::InvalidConfig("Socket not bound".to_string()))?;

        let receive_timestamp = PtpTimestamp::now();
        let port_id = PortIdentity::new(self.default_ds.clock_identity, 1);

        let header = Header {
            message_type: MessageType::DelayResp,
            version: 2,
            message_length: 54,
            domain: Domain(self.default_ds.domain_number),
            flags: Flags::default(),
            correction_field: 0,
            source_port_identity: port_id,
            sequence_id: delay_req.header.sequence_id,
            control: 3,
            log_message_interval: 0x7F,
        };

        let delay_resp = DelayRespMessage {
            header,
            receive_timestamp,
            requesting_port_identity: delay_req.header.source_port_identity,
        };

        let data = delay_resp.serialize()?;
        socket.send_to(&data, src).await?;

        Ok(())
    }

    async fn handle_delay_resp(&mut self, _delay_resp: DelayRespMessage) -> TimeSyncResult<()> {
        if self.port_state == PortState::Slave {
            debug!("Received Delay_Resp message");
            // Calculate mean path delay
        }
        Ok(())
    }

    async fn handle_announce(&mut self, announce: AnnounceMessage) -> TimeSyncResult<()> {
        let src_port = announce.header.source_port_identity;
        self.received_announces.insert(src_port, announce.clone());

        // Run BMCA
        let recommendation = recommend_state(&self.default_ds, Some(&announce), self.port_state);

        if recommendation.state != self.port_state {
            info!(
                "State transition: {:?} -> {:?}",
                self.port_state, recommendation.state
            );
            self.port_state = recommendation.state;
            self.current_master = recommendation.best_master;

            if self.port_state == PortState::Slave {
                info!("Became slave to {:?}", self.current_master);
                // Update parent dataset
                self.parent_ds.parent_port_identity = src_port;
                self.parent_ds.grandmaster_identity = announce.grandmaster_identity;
                self.parent_ds.grandmaster_clock_quality = announce.grandmaster_clock_quality;
                self.parent_ds.grandmaster_priority1 = announce.grandmaster_priority1;
                self.parent_ds.grandmaster_priority2 = announce.grandmaster_priority2;
                self.current_ds.steps_removed = announce.steps_removed + 1;
            }
        }

        Ok(())
    }
}

/// PTP Boundary Clock (BC) implementation.
///
/// A boundary clock has multiple PTP ports and can forward timing information.
pub struct BoundaryClock {
    /// Default dataset
    #[allow(dead_code)]
    default_ds: DefaultDataSet,
    /// Port states (indexed by port number)
    port_states: HashMap<u16, PortState>,
    /// Number of ports
    #[allow(dead_code)]
    num_ports: u16,
}

impl BoundaryClock {
    /// Create a new boundary clock.
    #[must_use]
    pub fn new(clock_identity: ClockIdentity, num_ports: u16) -> Self {
        let mut default_ds = DefaultDataSet::new(clock_identity);
        default_ds.number_ports = num_ports;

        let mut port_states = HashMap::new();
        for port_num in 1..=num_ports {
            port_states.insert(port_num, PortState::Initializing);
        }

        Self {
            default_ds,
            port_states,
            num_ports,
        }
    }

    /// Get port state.
    #[must_use]
    pub fn get_port_state(&self, port: u16) -> Option<PortState> {
        self.port_states.get(&port).copied()
    }

    /// Set port state.
    pub fn set_port_state(&mut self, port: u16, state: PortState) -> TimeSyncResult<()> {
        if port == 0 || port > self.num_ports {
            return Err(TimeSyncError::InvalidConfig(
                "Invalid port number".to_string(),
            ));
        }
        self.port_states.insert(port, state);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ordinary_clock_creation() {
        let clock_id = ClockIdentity::random();
        let clock = OrdinaryClock::new(clock_id, Domain::DEFAULT);

        assert_eq!(clock.port_state(), PortState::Initializing);
        assert_eq!(clock.offset_from_master(), 0);
    }

    #[test]
    fn test_boundary_clock_creation() {
        let clock_id = ClockIdentity::random();
        let clock = BoundaryClock::new(clock_id, 4);

        assert_eq!(clock.get_port_state(1), Some(PortState::Initializing));
        assert_eq!(clock.get_port_state(4), Some(PortState::Initializing));
        assert_eq!(clock.get_port_state(5), None);
    }
}
