//! RTMP (Real-Time Messaging Protocol) implementation.
//!
//! This module provides types and utilities for working with RTMP streaming,
//! including handshaking, chunking, and message handling.
//!
//! # Key Types
//!
//! - [`Handshake`] - RTMP handshake handler
//! - [`ChunkHeader`] - Chunk stream header
//! - [`ChunkStream`] - Chunk stream multiplexer
//! - [`RtmpMessage`] - RTMP message types
//! - [`AmfValue`] - AMF0 data type
//!
//! # Protocol Overview
//!
//! RTMP uses a handshake followed by chunk-based messaging:
//! 1. Client sends C0+C1, server responds with S0+S1+S2
//! 2. Client sends C2, connection established
//! 3. Messages are split into chunks for multiplexing
//!
//! # Example
//!
//! ```ignore
//! use oximedia_net::rtmp::{Handshake, ChunkStream, RtmpMessage};
//!
//! async fn handle_connection(stream: TcpStream) -> NetResult<()> {
//!     let mut handshake = Handshake::new();
//!     handshake.perform(&mut stream).await?;
//!     // ...
//!     Ok(())
//! }
//! ```

mod amf;
mod chunk;
mod client;
mod handshake;
mod message;
mod server;

pub use amf::{AmfDecoder, AmfEncoder, AmfValue};
pub use chunk::{
    Amf0Value, AssembledMessage, ChunkDecoder, ChunkEncoder, ChunkHeader, ChunkHeaderType,
    ChunkStream, MessageHeader,
};
pub use client::{
    ConnectionState, RtmpClient, RtmpClientBuilder, RtmpClientConfig, RtmpUrl, SessionInfo,
};
pub use handshake::{Handshake, HandshakeState};
pub use message::{
    CommandMessage, ControlMessage, DataMessage, MessageType, RtmpMessage, UserControlEvent,
};
pub use server::{
    ActiveStream, AllowAllAuth, AuthHandler, AuthResult, ConnectionInfo, MediaPacket,
    MediaPacketType, OutgoingMessage, PublishType, RtmpServer, RtmpServerBuilder, RtmpServerConfig,
    ServerConnection, ServerConnectionState, StreamMetadata, StreamRegistry,
};
