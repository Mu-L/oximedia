//! Jitter buffer for packet reordering and delay compensation.

use crate::error::{VideoIpError, VideoIpResult};
use crate::packet::Packet;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::time::{Duration, Instant};

/// Wrapper for packets in the jitter buffer with ordering.
struct JitterPacket {
    packet: Packet,
    arrival_time: Instant,
}

impl PartialEq for JitterPacket {
    fn eq(&self, other: &Self) -> bool {
        self.packet.header.sequence == other.packet.header.sequence
    }
}

impl Eq for JitterPacket {}

impl PartialOrd for JitterPacket {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for JitterPacket {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap (earliest sequence first)
        other
            .packet
            .header
            .sequence
            .cmp(&self.packet.header.sequence)
    }
}

/// Jitter buffer for reordering packets and compensating for network jitter.
pub struct JitterBuffer {
    /// Buffer of packets waiting to be played out.
    buffer: BinaryHeap<JitterPacket>,
    /// Maximum buffer size in packets.
    max_size: usize,
    /// Target buffer delay in milliseconds.
    target_delay_ms: u64,
    /// Expected next sequence number.
    next_sequence: Option<u16>,
    /// Statistics.
    stats: JitterStats,
}

/// Statistics for jitter buffer.
#[derive(Debug, Clone, Default)]
pub struct JitterStats {
    /// Number of packets added.
    pub packets_added: u64,
    /// Number of packets played out.
    pub packets_played: u64,
    /// Number of packets dropped due to buffer overflow.
    pub packets_dropped: u64,
    /// Number of packets played out of order.
    pub packets_out_of_order: u64,
    /// Number of duplicate packets.
    pub packets_duplicate: u64,
    /// Current buffer occupancy.
    pub buffer_occupancy: usize,
}

impl JitterBuffer {
    /// Creates a new jitter buffer.
    ///
    /// # Arguments
    ///
    /// * `max_size` - Maximum number of packets to buffer
    /// * `target_delay_ms` - Target buffering delay in milliseconds
    #[must_use]
    pub fn new(max_size: usize, target_delay_ms: u64) -> Self {
        Self {
            buffer: BinaryHeap::new(),
            max_size,
            target_delay_ms,
            next_sequence: None,
            stats: JitterStats::default(),
        }
    }

    /// Adds a packet to the buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is full.
    pub fn add_packet(&mut self, packet: Packet) -> VideoIpResult<()> {
        // Check for duplicates
        if self
            .buffer
            .iter()
            .any(|jp| jp.packet.header.sequence == packet.header.sequence)
        {
            self.stats.packets_duplicate += 1;
            return Ok(());
        }

        // Check buffer size
        if self.buffer.len() >= self.max_size {
            self.stats.packets_dropped += 1;
            return Err(VideoIpError::BufferOverflow);
        }

        // Initialize next_sequence on first packet
        if self.next_sequence.is_none() {
            self.next_sequence = Some(packet.header.sequence);
        }

        let jitter_packet = JitterPacket {
            packet,
            arrival_time: Instant::now(),
        };

        self.buffer.push(jitter_packet);
        self.stats.packets_added += 1;
        self.stats.buffer_occupancy = self.buffer.len();

        Ok(())
    }

    /// Retrieves the next packet if it's ready to be played out.
    ///
    /// Returns `None` if no packet is ready or if the target delay hasn't been reached.
    #[must_use]
    pub fn get_packet(&mut self) -> Option<Packet> {
        if self.buffer.is_empty() {
            return None;
        }

        // Check if the oldest packet has been buffered long enough
        let oldest = self.buffer.peek()?;
        let buffered_duration = oldest.arrival_time.elapsed();

        if buffered_duration < Duration::from_millis(self.target_delay_ms) {
            return None;
        }

        // Get the packet with the earliest sequence number
        if let Some(jitter_packet) = self.buffer.pop() {
            let packet = jitter_packet.packet;
            let sequence = packet.header.sequence;

            // Check if this is the expected sequence number
            if let Some(expected) = self.next_sequence {
                if sequence != expected {
                    self.stats.packets_out_of_order += 1;
                }
                self.next_sequence = Some(expected.wrapping_add(1));
            } else {
                self.next_sequence = Some(sequence.wrapping_add(1));
            }

            self.stats.packets_played += 1;
            self.stats.buffer_occupancy = self.buffer.len();

            Some(packet)
        } else {
            None
        }
    }

    /// Tries to get a packet immediately, bypassing the delay check.
    ///
    /// This is useful when the buffer is getting too full.
    #[must_use]
    pub fn get_packet_immediate(&mut self) -> Option<Packet> {
        if let Some(jitter_packet) = self.buffer.pop() {
            let packet = jitter_packet.packet;
            self.stats.packets_played += 1;
            self.stats.buffer_occupancy = self.buffer.len();
            Some(packet)
        } else {
            None
        }
    }

    /// Returns the number of packets currently in the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns true if the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Returns the buffer statistics.
    #[must_use]
    pub const fn stats(&self) -> &JitterStats {
        &self.stats
    }

    /// Clears the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.next_sequence = None;
        self.stats.buffer_occupancy = 0;
    }

    /// Sets the target delay.
    pub fn set_target_delay(&mut self, delay_ms: u64) {
        self.target_delay_ms = delay_ms;
    }

    /// Returns the current target delay in milliseconds.
    #[must_use]
    pub const fn target_delay(&self) -> u64 {
        self.target_delay_ms
    }

    /// Adjusts the buffer delay dynamically based on network conditions.
    ///
    /// This implements a simple adaptive algorithm that increases delay when
    /// packets are arriving out of order and decreases it when the buffer is stable.
    pub fn adjust_delay(&mut self) {
        const MIN_DELAY_MS: u64 = 5;
        const MAX_DELAY_MS: u64 = 100;
        const ADJUSTMENT_STEP: u64 = 5;

        // Increase delay if we're seeing lots of out-of-order packets
        let out_of_order_ratio = if self.stats.packets_played > 0 {
            self.stats.packets_out_of_order as f64 / self.stats.packets_played as f64
        } else {
            0.0
        };

        if out_of_order_ratio > 0.1 && self.target_delay_ms < MAX_DELAY_MS {
            self.target_delay_ms = (self.target_delay_ms + ADJUSTMENT_STEP).min(MAX_DELAY_MS);
        } else if out_of_order_ratio < 0.01 && self.target_delay_ms > MIN_DELAY_MS {
            self.target_delay_ms = (self.target_delay_ms - ADJUSTMENT_STEP).max(MIN_DELAY_MS);
        }
    }

    /// Removes packets older than the specified age.
    pub fn cleanup_old_packets(&mut self, max_age: Duration) {
        let now = Instant::now();
        let mut new_buffer = BinaryHeap::new();
        let mut dropped = 0;

        while let Some(jitter_packet) = self.buffer.pop() {
            if now.duration_since(jitter_packet.arrival_time) <= max_age {
                new_buffer.push(jitter_packet);
            } else {
                dropped += 1;
            }
        }

        self.buffer = new_buffer;
        self.stats.packets_dropped += dropped;
        self.stats.buffer_occupancy = self.buffer.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::PacketBuilder;
    use bytes::Bytes;
    use std::thread;

    #[test]
    fn test_jitter_buffer_creation() {
        let buffer = JitterBuffer::new(100, 20);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert_eq!(buffer.target_delay(), 20);
    }

    #[test]
    fn test_add_packet() {
        let mut buffer = JitterBuffer::new(100, 20);
        let packet = PacketBuilder::new(0)
            .video()
            .build(Bytes::from_static(b"test"))
            .expect("should succeed in test");

        buffer.add_packet(packet).expect("should succeed in test");
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn test_buffer_overflow() {
        let mut buffer = JitterBuffer::new(2, 20);

        for i in 0..3 {
            let packet = PacketBuilder::new(i)
                .video()
                .build(Bytes::from_static(b"test"))
                .expect("should succeed in test");

            if i < 2 {
                buffer.add_packet(packet).expect("should succeed in test");
            } else {
                assert!(buffer.add_packet(packet).is_err());
            }
        }
    }

    #[test]
    fn test_duplicate_detection() {
        let mut buffer = JitterBuffer::new(100, 20);
        let packet = PacketBuilder::new(0)
            .video()
            .build(Bytes::from_static(b"test"))
            .expect("should succeed in test");

        buffer
            .add_packet(packet.clone())
            .expect("should succeed in test");
        buffer.add_packet(packet).expect("should succeed in test"); // Duplicate

        assert_eq!(buffer.stats().packets_duplicate, 1);
        assert_eq!(buffer.len(), 1); // Only one packet in buffer
    }

    #[test]
    fn test_get_packet_with_delay() {
        let mut buffer = JitterBuffer::new(100, 10);
        let packet = PacketBuilder::new(0)
            .video()
            .build(Bytes::from_static(b"test"))
            .expect("should succeed in test");

        buffer.add_packet(packet).expect("should succeed in test");

        // Should not be available immediately
        assert!(buffer.get_packet().is_none());

        // Wait for the delay
        thread::sleep(Duration::from_millis(15));

        // Should now be available
        assert!(buffer.get_packet().is_some());
    }

    #[test]
    fn test_get_packet_immediate() {
        let mut buffer = JitterBuffer::new(100, 100);
        let packet = PacketBuilder::new(0)
            .video()
            .build(Bytes::from_static(b"test"))
            .expect("should succeed in test");

        buffer.add_packet(packet).expect("should succeed in test");

        // Should be available immediately
        assert!(buffer.get_packet_immediate().is_some());
    }

    #[test]
    fn test_packet_ordering() {
        let mut buffer = JitterBuffer::new(100, 0);

        // Add packets out of order
        for seq in [2u16, 0, 1, 4, 3] {
            let packet = PacketBuilder::new(seq)
                .video()
                .build(Bytes::from_static(b"test"))
                .expect("should succeed in test");
            buffer.add_packet(packet).expect("should succeed in test");
        }

        // Should come out in order
        for expected in 0..5 {
            let packet = buffer
                .get_packet_immediate()
                .expect("should succeed in test");
            assert_eq!(packet.header.sequence, expected);
        }
    }

    #[test]
    fn test_statistics() {
        let mut buffer = JitterBuffer::new(100, 0);

        let packet = PacketBuilder::new(0)
            .video()
            .build(Bytes::from_static(b"test"))
            .expect("should succeed in test");
        buffer.add_packet(packet).expect("should succeed in test");

        assert_eq!(buffer.stats().packets_added, 1);
        assert_eq!(buffer.stats().buffer_occupancy, 1);

        let _ = buffer.get_packet_immediate();
        assert_eq!(buffer.stats().packets_played, 1);
    }

    #[test]
    fn test_clear() {
        let mut buffer = JitterBuffer::new(100, 20);
        let packet = PacketBuilder::new(0)
            .video()
            .build(Bytes::from_static(b"test"))
            .expect("should succeed in test");

        buffer.add_packet(packet).expect("should succeed in test");
        assert!(!buffer.is_empty());

        buffer.clear();
        assert!(buffer.is_empty());
    }

    #[test]
    fn test_set_target_delay() {
        let mut buffer = JitterBuffer::new(100, 20);
        buffer.set_target_delay(50);
        assert_eq!(buffer.target_delay(), 50);
    }

    #[test]
    fn test_cleanup_old_packets() {
        let mut buffer = JitterBuffer::new(100, 0);

        let packet = PacketBuilder::new(0)
            .video()
            .build(Bytes::from_static(b"test"))
            .expect("should succeed in test");
        buffer.add_packet(packet).expect("should succeed in test");

        thread::sleep(Duration::from_millis(10));

        buffer.cleanup_old_packets(Duration::from_millis(5));
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.stats().packets_dropped, 1);
    }
}
