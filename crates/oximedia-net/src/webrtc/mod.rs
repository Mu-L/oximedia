//! WebRTC protocol implementation.
//!
//! This module provides a complete WebRTC implementation including:
//! - ICE (Interactive Connectivity Establishment) for NAT traversal
//! - DTLS for encryption
//! - SCTP over DTLS for data channels
//! - RTP/RTCP for media transport
//! - SDP for session negotiation
//!
//! # Key Types
//!
//! - [`PeerConnection`] - Main WebRTC peer connection
//! - [`DataChannel`] - WebRTC data channel for arbitrary data
//! - [`MediaTrack`] - Media track for audio/video
//! - [`SessionDescription`] - SDP representation
//! - [`IceCandidate`] - ICE candidate for connectivity
//!
//! # Example
//!
//! ```ignore
//! use oximedia_net::webrtc::{PeerConnection, PeerConnectionConfig, IceServer};
//!
//! async fn create_peer_connection() -> Result<PeerConnection, Box<dyn std::error::Error>> {
//!     let config = PeerConnectionConfig::new()
//!         .with_ice_server(IceServer::stun("stun:stun.example.com:3478"));
//!
//!     let pc = PeerConnection::new(config)?;
//!
//!     // Create offer
//!     let offer = pc.create_offer().await?;
//!     pc.set_local_description(offer).await?;
//!
//!     // Create data channel
//!     let dc = pc.create_data_channel("test").await?;
//!     dc.send_text("Hello WebRTC!").await?;
//!
//!     Ok(pc)
//! }
//! ```

mod datachannel;
mod dtls;
mod ice;
mod ice_agent;
mod peer_connection;
mod rtcp;
mod rtp;
mod sctp;
mod sdp;
mod srtp;
mod stun;

// Re-export main API types
pub use datachannel::{DataChannel, DataChannelConfig, DataChannelState, Message, MessageType};
pub use dtls::{DtlsConfig, DtlsFingerprint, DtlsRole};
pub use ice::{CandidateType, IceCandidate, IceServer, TransportProtocol};
pub use ice_agent::{IceConnectionState, IceGatheringState, IceRole};
pub use peer_connection::{
    BundlePolicy, MediaTrack, PeerConnection, PeerConnectionConfig, PeerConnectionState,
    RtcpMuxPolicy, SdpType, SessionDescriptionInit, SignalingState,
};
pub use rtcp::{Packet as RtcpPacket, ReceiverReport, SenderReport};
pub use rtp::{Packet as RtpPacket, Session as RtpSession, Statistics as RtpStatistics};
pub use sdp::{Attribute, Direction, Fingerprint, MediaDescription, MediaType, SessionDescription};
