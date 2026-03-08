//! Forward Error Correction (FEC) using Reed-Solomon codes.

use crate::error::{VideoIpError, VideoIpResult};
use crate::packet::{Packet, PacketBuilder, PacketFlags};
use crate::types::StreamType;
use bytes::Bytes;
use reed_solomon_erasure::galois_8::ReedSolomon;
use std::collections::HashMap;

/// FEC encoder for creating parity packets.
#[allow(dead_code)]
pub struct FecEncoder {
    /// Reed-Solomon encoder.
    encoder: ReedSolomon,
    /// Number of data packets in each FEC group.
    data_shards: usize,
    /// Number of parity packets in each FEC group.
    parity_shards: usize,
    /// Maximum packet size.
    max_packet_size: usize,
}

impl FecEncoder {
    /// Creates a new FEC encoder.
    ///
    /// # Arguments
    ///
    /// * `data_shards` - Number of data packets in each FEC group (typically 20-50)
    /// * `parity_shards` - Number of parity packets in each FEC group (typically 1-10)
    ///
    /// # Errors
    ///
    /// Returns an error if the shard configuration is invalid.
    pub fn new(data_shards: usize, parity_shards: usize) -> VideoIpResult<Self> {
        let encoder = ReedSolomon::new(data_shards, parity_shards)
            .map_err(|e| VideoIpError::Fec(format!("failed to create encoder: {e}")))?;

        Ok(Self {
            encoder,
            data_shards,
            parity_shards,
            max_packet_size: 8192,
        })
    }

    /// Creates an FEC encoder with a specific FEC ratio.
    ///
    /// # Arguments
    ///
    /// * `ratio` - FEC ratio (0.05 = 5%, 0.10 = 10%, etc.)
    ///
    /// # Errors
    ///
    /// Returns an error if the ratio is invalid.
    pub fn with_ratio(ratio: f32) -> VideoIpResult<Self> {
        if !(0.01..=0.5).contains(&ratio) {
            return Err(VideoIpError::Fec(format!(
                "invalid FEC ratio: {ratio} (must be between 0.01 and 0.5)"
            )));
        }

        let data_shards = 20;
        let parity_shards = ((data_shards as f32) * ratio).ceil() as usize;
        Self::new(data_shards, parity_shards)
    }

    /// Encodes a group of data packets and generates parity packets.
    ///
    /// # Errors
    ///
    /// Returns an error if encoding fails.
    pub fn encode(
        &self,
        packets: &[Packet],
        base_sequence: u16,
        timestamp: u64,
        stream_type: StreamType,
    ) -> VideoIpResult<Vec<Packet>> {
        if packets.is_empty() || packets.len() > self.data_shards {
            return Err(VideoIpError::Fec(format!(
                "invalid packet count: {} (expected 1-{})",
                packets.len(),
                self.data_shards
            )));
        }

        // Find the maximum packet size in this group
        let max_size = packets.iter().map(|p| p.payload.len()).max().unwrap_or(0);

        // Pad all packets to the same size
        let mut shards: Vec<Vec<u8>> = packets
            .iter()
            .map(|p| {
                let mut data = p.payload.to_vec();
                data.resize(max_size, 0);
                data
            })
            .collect();

        // Add empty shards up to data_shards count
        while shards.len() < self.data_shards {
            shards.push(vec![0u8; max_size]);
        }

        // Add parity shards
        for _ in 0..self.parity_shards {
            shards.push(vec![0u8; max_size]);
        }

        // Encode
        self.encoder
            .encode(&mut shards)
            .map_err(|e| VideoIpError::Fec(format!("encoding failed: {e}")))?;

        // Create parity packets from the parity shards
        let mut parity_packets = Vec::with_capacity(self.parity_shards);
        for (i, shard) in shards[self.data_shards..].iter().enumerate() {
            let sequence = base_sequence.wrapping_add(i as u16);
            let payload = Bytes::from(shard.clone());

            let packet = PacketBuilder::new(sequence)
                .fec()
                .with_timestamp(timestamp)
                .with_stream_type(stream_type)
                .build(payload)?;

            parity_packets.push(packet);
        }

        Ok(parity_packets)
    }

    /// Returns the number of data shards.
    #[must_use]
    pub const fn data_shards(&self) -> usize {
        self.data_shards
    }

    /// Returns the number of parity shards.
    #[must_use]
    pub const fn parity_shards(&self) -> usize {
        self.parity_shards
    }
}

/// FEC decoder for recovering lost packets.
pub struct FecDecoder {
    /// Reed-Solomon decoder.
    decoder: ReedSolomon,
    /// Number of data packets in each FEC group.
    data_shards: usize,
    /// Number of parity packets in each FEC group.
    parity_shards: usize,
    /// Pending FEC groups waiting for completion.
    pending_groups: HashMap<u16, FecGroup>,
}

/// A group of packets for FEC decoding.
struct FecGroup {
    /// Data packets (Some if received, None if missing).
    data_packets: Vec<Option<Packet>>,
    /// Parity packets (Some if received, None if missing).
    parity_packets: Vec<Option<Packet>>,
    /// Maximum packet size in this group.
    max_packet_size: usize,
    /// Timestamp of the group.
    timestamp: u64,
}

impl FecDecoder {
    /// Creates a new FEC decoder.
    ///
    /// # Errors
    ///
    /// Returns an error if the shard configuration is invalid.
    pub fn new(data_shards: usize, parity_shards: usize) -> VideoIpResult<Self> {
        let decoder = ReedSolomon::new(data_shards, parity_shards)
            .map_err(|e| VideoIpError::Fec(format!("failed to create decoder: {e}")))?;

        Ok(Self {
            decoder,
            data_shards,
            parity_shards,
            pending_groups: HashMap::new(),
        })
    }

    /// Adds a packet to the decoder.
    ///
    /// Returns recovered packets if FEC decoding was successful.
    ///
    /// # Errors
    ///
    /// Returns an error if decoding fails.
    pub fn add_packet(&mut self, packet: Packet) -> VideoIpResult<Vec<Packet>> {
        let group_id = self.get_group_id(packet.header.sequence);

        // Calculate indices before acquiring mutable borrow
        let is_parity = packet.header.flags.contains(PacketFlags::FEC);
        let parity_idx = if is_parity {
            Some(self.get_parity_index(packet.header.sequence))
        } else {
            None
        };
        let data_idx = if is_parity {
            None
        } else {
            Some(self.get_data_index(packet.header.sequence))
        };

        let group = self
            .pending_groups
            .entry(group_id)
            .or_insert_with(|| FecGroup {
                data_packets: vec![None; self.data_shards],
                parity_packets: vec![None; self.parity_shards],
                max_packet_size: 0,
                timestamp: packet.header.timestamp,
            });

        group.max_packet_size = group.max_packet_size.max(packet.payload.len());

        if is_parity {
            // This is a parity packet
            if let Some(idx) = parity_idx {
                if idx < self.parity_shards {
                    group.parity_packets[idx] = Some(packet);
                }
            }
        } else {
            // This is a data packet
            if let Some(idx) = data_idx {
                if idx < self.data_shards {
                    group.data_packets[idx] = Some(packet);
                }
            }
        }

        // Try to recover if we have enough packets
        self.try_recover(group_id)
    }
    /// Attempts to recover missing packets in a group.
    fn try_recover(&mut self, group_id: u16) -> VideoIpResult<Vec<Packet>> {
        let group = match self.pending_groups.get_mut(&group_id) {
            Some(g) => g,
            None => return Ok(Vec::new()),
        };

        let data_count = group.data_packets.iter().filter(|p| p.is_some()).count();
        let parity_count = group.parity_packets.iter().filter(|p| p.is_some()).count();
        let total_count = data_count + parity_count;

        // We need at least data_shards packets to recover
        if total_count < self.data_shards {
            return Ok(Vec::new());
        }

        // Build shards for decoding
        let mut shards: Vec<Option<Vec<u8>>> = Vec::new();
        for packet in &group.data_packets {
            shards.push(packet.as_ref().map(|p| {
                let mut data = p.payload.to_vec();
                data.resize(group.max_packet_size, 0);
                data
            }));
        }
        for packet in &group.parity_packets {
            shards.push(packet.as_ref().map(|p| {
                let mut data = p.payload.to_vec();
                data.resize(group.max_packet_size, 0);
                data
            }));
        }

        // Decode
        self.decoder
            .reconstruct(&mut shards)
            .map_err(|e| VideoIpError::Fec(format!("reconstruction failed: {e}")))?;

        // Extract recovered packets
        let mut recovered = Vec::new();
        for (i, shard) in shards[..self.data_shards].iter().enumerate() {
            if group.data_packets[i].is_none() {
                if let Some(data) = shard {
                    let sequence = group_id.wrapping_add(i as u16);
                    let payload = Bytes::from(data.clone());

                    let packet = PacketBuilder::new(sequence)
                        .video() // Assume video for now
                        .with_timestamp(group.timestamp)
                        .build(payload)?;

                    recovered.push(packet);
                }
            }
        }

        // Clean up the group
        self.pending_groups.remove(&group_id);

        Ok(recovered)
    }

    /// Gets the group ID for a sequence number.
    fn get_group_id(&self, sequence: u16) -> u16 {
        let total_shards = (self.data_shards + self.parity_shards) as u16;
        (sequence / total_shards) * total_shards
    }

    /// Gets the data packet index within a group.
    fn get_data_index(&self, sequence: u16) -> usize {
        let group_id = self.get_group_id(sequence);
        (sequence.wrapping_sub(group_id)) as usize
    }

    /// Gets the parity packet index within a group.
    fn get_parity_index(&self, sequence: u16) -> usize {
        let group_id = self.get_group_id(sequence);
        let offset = sequence.wrapping_sub(group_id) as usize;
        offset.saturating_sub(self.data_shards)
    }

    /// Cleans up old pending groups.
    pub fn cleanup_old_groups(&mut self, max_age_ms: u64) {
        let now = crate::packet::current_timestamp_micros();
        self.pending_groups
            .retain(|_, group| now - group.timestamp < max_age_ms * 1000);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fec_encoder_creation() {
        let encoder = FecEncoder::new(20, 2).expect("should succeed in test");
        assert_eq!(encoder.data_shards(), 20);
        assert_eq!(encoder.parity_shards(), 2);
    }

    #[test]
    fn test_fec_encoder_with_ratio() {
        let encoder = FecEncoder::with_ratio(0.1).expect("should succeed in test");
        assert_eq!(encoder.data_shards(), 20);
        assert_eq!(encoder.parity_shards(), 2);
    }

    #[test]
    fn test_fec_invalid_ratio() {
        assert!(FecEncoder::with_ratio(0.0).is_err());
        assert!(FecEncoder::with_ratio(0.6).is_err());
    }

    #[test]
    fn test_fec_encode() {
        let encoder = FecEncoder::new(10, 2).expect("should succeed in test");

        let packets: Vec<Packet> = (0..10)
            .map(|i| {
                PacketBuilder::new(i)
                    .video()
                    .with_timestamp(12345)
                    .build(Bytes::from(vec![i as u8; 100]))
                    .expect("should succeed in test")
            })
            .collect();

        let parity = encoder
            .encode(&packets, 100, 12345, StreamType::Program)
            .expect("should succeed in test");

        assert_eq!(parity.len(), 2);
        assert!(parity[0].header.flags.contains(PacketFlags::FEC));
    }

    #[test]
    fn test_fec_decoder_creation() {
        let decoder = FecDecoder::new(20, 2).expect("should succeed in test");
        assert_eq!(decoder.data_shards, 20);
        assert_eq!(decoder.parity_shards, 2);
    }

    #[test]
    fn test_fec_recovery() {
        let encoder = FecEncoder::new(5, 2).expect("should succeed in test");

        // Create 5 data packets
        let packets: Vec<Packet> = (0..5)
            .map(|i| {
                PacketBuilder::new(i)
                    .video()
                    .with_timestamp(12345)
                    .build(Bytes::from(vec![i as u8; 50]))
                    .expect("should succeed in test")
            })
            .collect();

        // Generate parity packets
        let parity = encoder
            .encode(&packets, 5, 12345, StreamType::Program)
            .expect("should succeed in test");

        // Create decoder and add 4 data packets + 2 parity packets (missing packet 2)
        let mut decoder = FecDecoder::new(5, 2).expect("should succeed in test");
        let mut all_recovered = Vec::new();
        all_recovered.extend(
            decoder
                .add_packet(packets[0].clone())
                .expect("should succeed in test"),
        );
        all_recovered.extend(
            decoder
                .add_packet(packets[1].clone())
                .expect("should succeed in test"),
        );
        // Skip packet 2
        all_recovered.extend(
            decoder
                .add_packet(packets[3].clone())
                .expect("should succeed in test"),
        );
        all_recovered.extend(
            decoder
                .add_packet(packets[4].clone())
                .expect("should succeed in test"),
        );
        all_recovered.extend(
            decoder
                .add_packet(parity[0].clone())
                .expect("should succeed in test"),
        );
        all_recovered.extend(
            decoder
                .add_packet(parity[1].clone())
                .expect("should succeed in test"),
        );

        // Should recover the missing packet
        assert!(!all_recovered.is_empty());
    }

    #[test]
    fn test_group_id_calculation() {
        let decoder = FecDecoder::new(20, 2).expect("should succeed in test");
        assert_eq!(decoder.get_group_id(0), 0);
        assert_eq!(decoder.get_group_id(21), 0);
        assert_eq!(decoder.get_group_id(22), 22);
        assert_eq!(decoder.get_group_id(43), 22);
    }

    #[test]
    fn test_cleanup_old_groups() {
        let mut decoder = FecDecoder::new(10, 2).expect("should succeed in test");

        let packet = PacketBuilder::new(0)
            .video()
            .with_timestamp(0)
            .build(Bytes::from_static(b"test"))
            .expect("should succeed in test");

        decoder.add_packet(packet).expect("should succeed in test");
        assert_eq!(decoder.pending_groups.len(), 1);

        decoder.cleanup_old_groups(0);
        assert_eq!(decoder.pending_groups.len(), 0);
    }
}
