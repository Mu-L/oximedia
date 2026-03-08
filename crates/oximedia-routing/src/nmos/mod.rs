//! NMOS IS-04/IS-05 network media interface implementation.
//!
//! This module implements the AMWA NMOS (Networked Media Open Specifications)
//! IS-04 Discovery & Registration and IS-05 Device Connection Management.

#![allow(dead_code)]

use std::collections::HashMap;

/// An NMOS node representing a physical or virtual device host.
#[derive(Debug, Clone)]
pub struct NmosNode {
    /// Unique identifier (UUID format)
    pub id: String,
    /// Human-readable label
    pub label: String,
    /// Extended description
    pub description: String,
    /// Arbitrary tags as key -> list of values
    pub tags: HashMap<String, Vec<String>>,
}

impl NmosNode {
    /// Create a new NMOS node.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: String::new(),
            tags: HashMap::new(),
        }
    }
}

/// The type of an NMOS device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NmosDeviceType {
    /// Generic device
    Generic,
    /// Pipeline processor
    Pipeline,
    /// Signal processor
    Processing,
    /// Input device (capture)
    Input,
    /// Output device (playout)
    Output,
}

/// An NMOS device belonging to a node.
#[derive(Debug, Clone)]
pub struct NmosDevice {
    /// Unique identifier
    pub id: String,
    /// Parent node identifier
    pub node_id: String,
    /// Human-readable label
    pub label: String,
    /// Device type classification
    pub device_type: NmosDeviceType,
}

impl NmosDevice {
    /// Create a new NMOS device.
    pub fn new(
        id: impl Into<String>,
        node_id: impl Into<String>,
        label: impl Into<String>,
        device_type: NmosDeviceType,
    ) -> Self {
        Self {
            id: id.into(),
            node_id: node_id.into(),
            label: label.into(),
            device_type,
        }
    }
}

/// Media format carried by a source or flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NmosFormat {
    /// Video essence
    Video,
    /// Audio essence
    Audio,
    /// Generic data
    Data,
    /// Multiplexed essence
    Mux,
}

impl NmosFormat {
    /// Return the IANA media type string for the format.
    #[must_use]
    pub fn media_type(&self) -> &str {
        match self {
            NmosFormat::Video => "video/raw",
            NmosFormat::Audio => "audio/L24",
            NmosFormat::Data => "application/json",
            NmosFormat::Mux => "video/SMPTE2022-6",
        }
    }
}

/// An NMOS source – the logical origin of media content.
#[derive(Debug, Clone)]
pub struct NmosSource {
    /// Unique identifier
    pub id: String,
    /// Parent device identifier
    pub device_id: String,
    /// Human-readable label
    pub label: String,
    /// Format of the essence
    pub format: NmosFormat,
    /// PTP clock reference name
    pub clock_name: String,
}

impl NmosSource {
    /// Create a new NMOS source.
    pub fn new(
        id: impl Into<String>,
        device_id: impl Into<String>,
        label: impl Into<String>,
        format: NmosFormat,
        clock_name: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            device_id: device_id.into(),
            label: label.into(),
            format,
            clock_name: clock_name.into(),
        }
    }
}

/// An NMOS flow – a concrete representation of media from a source.
#[derive(Debug, Clone)]
pub struct NmosFlow {
    /// Unique identifier
    pub id: String,
    /// Parent source identifier
    pub source_id: String,
    /// Human-readable label
    pub label: String,
    /// Format of the essence
    pub format: NmosFormat,
    /// Frame rate as (numerator, denominator)
    pub frame_rate: (u32, u32),
}

impl NmosFlow {
    /// Create a new NMOS flow.
    pub fn new(
        id: impl Into<String>,
        source_id: impl Into<String>,
        label: impl Into<String>,
        format: NmosFormat,
        frame_rate: (u32, u32),
    ) -> Self {
        Self {
            id: id.into(),
            source_id: source_id.into(),
            label: label.into(),
            format,
            frame_rate,
        }
    }
}

/// Transport mechanism used by an NMOS sender.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NmosTransport {
    /// RTP multicast (SMPTE ST 2022 / ST 2110)
    RtpMulticast,
    /// RTP unicast
    RtpUnicast,
    /// MPEG-DASH
    Dash,
    /// HTTP Live Streaming
    Hls,
    /// Secure Reliable Transport
    Srt,
}

/// An NMOS sender – transmits a flow over the network.
#[derive(Debug, Clone)]
pub struct NmosSender {
    /// Unique identifier
    pub id: String,
    /// Flow being sent
    pub flow_id: String,
    /// Human-readable label
    pub label: String,
    /// Transport mechanism
    pub transport: NmosTransport,
}

impl NmosSender {
    /// Create a new NMOS sender.
    pub fn new(
        id: impl Into<String>,
        flow_id: impl Into<String>,
        label: impl Into<String>,
        transport: NmosTransport,
    ) -> Self {
        Self {
            id: id.into(),
            flow_id: flow_id.into(),
            label: label.into(),
            transport,
        }
    }
}

/// An NMOS receiver – accepts a flow from a sender.
#[derive(Debug, Clone)]
pub struct NmosReceiver {
    /// Unique identifier
    pub id: String,
    /// Parent device identifier
    pub device_id: String,
    /// Human-readable label
    pub label: String,
    /// Accepted format
    pub format: NmosFormat,
    /// Currently subscribed sender ID (if any)
    pub subscription: Option<String>,
}

impl NmosReceiver {
    /// Create a new NMOS receiver.
    pub fn new(
        id: impl Into<String>,
        device_id: impl Into<String>,
        label: impl Into<String>,
        format: NmosFormat,
    ) -> Self {
        Self {
            id: id.into(),
            device_id: device_id.into(),
            label: label.into(),
            format,
            subscription: None,
        }
    }
}

/// Central NMOS registry holding all registered resources.
#[derive(Debug, Default)]
pub struct NmosRegistry {
    nodes: HashMap<String, NmosNode>,
    devices: HashMap<String, NmosDevice>,
    sources: HashMap<String, NmosSource>,
    flows: HashMap<String, NmosFlow>,
    senders: HashMap<String, NmosSender>,
    receivers: HashMap<String, NmosReceiver>,
}

impl NmosRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a node.
    pub fn add_node(&mut self, node: NmosNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Register a device.
    pub fn add_device(&mut self, device: NmosDevice) {
        self.devices.insert(device.id.clone(), device);
    }

    /// Register a source.
    pub fn add_source(&mut self, source: NmosSource) {
        self.sources.insert(source.id.clone(), source);
    }

    /// Register a flow.
    pub fn add_flow(&mut self, flow: NmosFlow) {
        self.flows.insert(flow.id.clone(), flow);
    }

    /// Register a sender.
    pub fn add_sender(&mut self, sender: NmosSender) {
        self.senders.insert(sender.id.clone(), sender);
    }

    /// Register a receiver.
    pub fn add_receiver(&mut self, receiver: NmosReceiver) {
        self.receivers.insert(receiver.id.clone(), receiver);
    }

    /// Look up a node by ID.
    pub fn get_node(&self, id: &str) -> Option<&NmosNode> {
        self.nodes.get(id)
    }

    /// Look up a sender by ID.
    pub fn get_sender(&self, id: &str) -> Option<&NmosSender> {
        self.senders.get(id)
    }

    /// Look up a flow by ID.
    pub fn get_flow(&self, id: &str) -> Option<&NmosFlow> {
        self.flows.get(id)
    }

    /// Find all receivers compatible with a given sender (same format).
    ///
    /// Compatibility is determined by matching the sender's flow format against
    /// each receiver's accepted format.
    pub fn find_compatible_receivers(&self, sender_id: &str) -> Vec<&NmosReceiver> {
        let Some(sender) = self.senders.get(sender_id) else {
            return Vec::new();
        };
        let Some(flow) = self.flows.get(&sender.flow_id) else {
            return Vec::new();
        };
        self.receivers
            .values()
            .filter(|r| r.format == flow.format)
            .collect()
    }

    /// Return the total number of registered nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Return the total number of registered senders.
    pub fn sender_count(&self) -> usize {
        self.senders.len()
    }

    /// Return the total number of registered receivers.
    pub fn receiver_count(&self) -> usize {
        self.receivers.len()
    }
}

/// A single IS-05 connection between a sender and a receiver.
#[derive(Debug, Clone)]
pub struct NmosConnection {
    /// Sender resource identifier
    pub sender_id: String,
    /// Receiver resource identifier
    pub receiver_id: String,
    /// Whether this connection is currently active
    pub active: bool,
}

impl NmosConnection {
    /// Create a new inactive connection.
    pub fn new(sender_id: impl Into<String>, receiver_id: impl Into<String>) -> Self {
        Self {
            sender_id: sender_id.into(),
            receiver_id: receiver_id.into(),
            active: false,
        }
    }
}

/// IS-05 connection manager: creates and tears down connections.
#[derive(Debug, Default)]
pub struct NmosConnectionManager {
    connections: Vec<NmosConnection>,
}

impl NmosConnectionManager {
    /// Create a new connection manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Establish a connection between sender and receiver.
    ///
    /// If a connection between the same pair already exists it is activated;
    /// otherwise a new active connection is added.
    pub fn connect(&mut self, sender_id: impl Into<String>, receiver_id: impl Into<String>) {
        let sid = sender_id.into();
        let rid = receiver_id.into();

        // Activate existing connection if found.
        for conn in &mut self.connections {
            if conn.sender_id == sid && conn.receiver_id == rid {
                conn.active = true;
                return;
            }
        }

        self.connections.push(NmosConnection {
            sender_id: sid,
            receiver_id: rid,
            active: true,
        });
    }

    /// Tear down the connection between sender and receiver.
    pub fn disconnect(&mut self, sender_id: &str, receiver_id: &str) {
        for conn in &mut self.connections {
            if conn.sender_id == sender_id && conn.receiver_id == receiver_id {
                conn.active = false;
            }
        }
    }

    /// Return references to all currently active connections.
    pub fn active_connections(&self) -> Vec<&NmosConnection> {
        self.connections.iter().filter(|c| c.active).collect()
    }

    /// Return the total number of connections (active and inactive).
    pub fn total_connections(&self) -> usize {
        self.connections.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registry() -> NmosRegistry {
        let mut reg = NmosRegistry::new();

        let node = NmosNode::new("node-1", "Test Node");
        reg.add_node(node);

        let device = NmosDevice::new("dev-1", "node-1", "Camera", NmosDeviceType::Output);
        reg.add_device(device);

        let source = NmosSource::new("src-1", "dev-1", "Cam Source", NmosFormat::Video, "clk-0");
        reg.add_source(source);

        let flow = NmosFlow::new("flow-1", "src-1", "Cam Flow", NmosFormat::Video, (25, 1));
        reg.add_flow(flow);

        let sender = NmosSender::new(
            "sender-1",
            "flow-1",
            "Cam Sender",
            NmosTransport::RtpMulticast,
        );
        reg.add_sender(sender);

        let rx1 = NmosReceiver::new("rx-1", "dev-1", "Monitor A", NmosFormat::Video);
        let rx2 = NmosReceiver::new("rx-2", "dev-1", "Monitor B", NmosFormat::Video);
        let rx3 = NmosReceiver::new("rx-3", "dev-1", "Audio In", NmosFormat::Audio);
        reg.add_receiver(rx1);
        reg.add_receiver(rx2);
        reg.add_receiver(rx3);

        reg
    }

    #[test]
    fn test_nmos_node_creation() {
        let node = NmosNode::new("n-1", "My Node");
        assert_eq!(node.id, "n-1");
        assert_eq!(node.label, "My Node");
        assert!(node.description.is_empty());
    }

    #[test]
    fn test_nmos_device_types() {
        assert_ne!(NmosDeviceType::Generic, NmosDeviceType::Pipeline);
        assert_ne!(NmosDeviceType::Processing, NmosDeviceType::Input);
        assert_ne!(NmosDeviceType::Input, NmosDeviceType::Output);
        // Five distinct variants exist
        let types = [
            NmosDeviceType::Generic,
            NmosDeviceType::Pipeline,
            NmosDeviceType::Processing,
            NmosDeviceType::Input,
            NmosDeviceType::Output,
        ];
        assert_eq!(types.len(), 5);
    }

    #[test]
    fn test_nmos_format_media_type() {
        assert_eq!(NmosFormat::Video.media_type(), "video/raw");
        assert_eq!(NmosFormat::Audio.media_type(), "audio/L24");
        assert_eq!(NmosFormat::Data.media_type(), "application/json");
        assert_eq!(NmosFormat::Mux.media_type(), "video/SMPTE2022-6");
    }

    #[test]
    fn test_registry_add_and_get_node() {
        let mut reg = NmosRegistry::new();
        let node = NmosNode::new("n-42", "Node 42");
        reg.add_node(node);
        assert_eq!(reg.node_count(), 1);
        assert_eq!(
            reg.get_node("n-42").expect("should succeed in test").label,
            "Node 42"
        );
    }

    #[test]
    fn test_registry_add_sender_receiver() {
        let reg = make_registry();
        assert_eq!(reg.sender_count(), 1);
        assert_eq!(reg.receiver_count(), 3);
    }

    #[test]
    fn test_find_compatible_receivers_video() {
        let reg = make_registry();
        let compatible = reg.find_compatible_receivers("sender-1");
        // rx-1 and rx-2 accept Video; rx-3 accepts Audio
        assert_eq!(compatible.len(), 2);
        for r in &compatible {
            assert_eq!(r.format, NmosFormat::Video);
        }
    }

    #[test]
    fn test_find_compatible_receivers_unknown_sender() {
        let reg = make_registry();
        let result = reg.find_compatible_receivers("nonexistent");
        assert!(result.is_empty());
    }

    #[test]
    fn test_connection_manager_connect() {
        let mut mgr = NmosConnectionManager::new();
        mgr.connect("sender-1", "rx-1");
        assert_eq!(mgr.active_connections().len(), 1);
        assert_eq!(mgr.total_connections(), 1);
    }

    #[test]
    fn test_connection_manager_disconnect() {
        let mut mgr = NmosConnectionManager::new();
        mgr.connect("sender-1", "rx-1");
        mgr.disconnect("sender-1", "rx-1");
        assert_eq!(mgr.active_connections().len(), 0);
        // The record still exists, just inactive
        assert_eq!(mgr.total_connections(), 1);
    }

    #[test]
    fn test_connection_manager_reconnect() {
        let mut mgr = NmosConnectionManager::new();
        mgr.connect("sender-1", "rx-1");
        mgr.disconnect("sender-1", "rx-1");
        mgr.connect("sender-1", "rx-1");
        // Should reuse the existing slot
        assert_eq!(mgr.total_connections(), 1);
        assert_eq!(mgr.active_connections().len(), 1);
    }

    #[test]
    fn test_multiple_connections() {
        let mut mgr = NmosConnectionManager::new();
        mgr.connect("sender-1", "rx-1");
        mgr.connect("sender-1", "rx-2");
        assert_eq!(mgr.active_connections().len(), 2);
    }

    #[test]
    fn test_nmos_flow_frame_rate() {
        let flow = NmosFlow::new("f", "s", "Flow", NmosFormat::Video, (30000, 1001));
        assert_eq!(flow.frame_rate, (30000, 1001));
    }

    #[test]
    fn test_nmos_transport_variants() {
        let transports = [
            NmosTransport::RtpMulticast,
            NmosTransport::RtpUnicast,
            NmosTransport::Dash,
            NmosTransport::Hls,
            NmosTransport::Srt,
        ];
        assert_eq!(transports.len(), 5);
    }

    #[test]
    fn test_receiver_subscription_default_none() {
        let rx = NmosReceiver::new("r-1", "d-1", "Rec", NmosFormat::Audio);
        assert!(rx.subscription.is_none());
    }
}
