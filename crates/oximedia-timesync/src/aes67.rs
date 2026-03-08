//! AES67 audio over IP timing implementation.
//!
//! AES67 is a standard for high-performance audio over IP networks,
//! defining timing, synchronization, and transport requirements.

use std::fmt;

/// AES67 stream configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Aes67Config {
    /// Sample rate in Hz (typically 48000 or 96000)
    pub sample_rate: u32,
    /// Number of audio channels
    pub channels: u8,
    /// Bit depth (16, 24, or 32)
    pub bit_depth: u8,
    /// Packet time in microseconds
    pub packet_time_us: u32,
}

impl Aes67Config {
    /// Create an AES67 standard configuration (48kHz, 2ch, 24-bit, 1ms packets).
    #[must_use]
    pub fn standard() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            bit_depth: 24,
            packet_time_us: 1000,
        }
    }

    /// Create a high channel count configuration.
    #[must_use]
    pub fn multichannel(channels: u8) -> Self {
        Self {
            sample_rate: 48000,
            channels,
            bit_depth: 24,
            packet_time_us: 1000,
        }
    }

    /// Get the number of samples per packet.
    #[must_use]
    pub fn samples_per_packet(&self) -> u32 {
        (self.sample_rate as u64 * self.packet_time_us as u64 / 1_000_000) as u32
    }

    /// Get the packet size in bytes (excluding headers).
    #[must_use]
    pub fn packet_payload_bytes(&self) -> u32 {
        self.samples_per_packet() * u32::from(self.channels) * u32::from(self.bit_depth / 8)
    }
}

/// AES67 packet timing options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Aes67PacketTime {
    /// 125 microseconds (ultra-low latency)
    Us125,
    /// 250 microseconds
    Us250,
    /// 333 microseconds (1/3 ms)
    Us333,
    /// 1000 microseconds (1 ms, standard)
    Us1000,
    /// 4000 microseconds (4 ms)
    Us4000,
}

impl Aes67PacketTime {
    /// Get the packet time in microseconds.
    #[must_use]
    pub fn microseconds(&self) -> u32 {
        match self {
            Aes67PacketTime::Us125 => 125,
            Aes67PacketTime::Us250 => 250,
            Aes67PacketTime::Us333 => 333,
            Aes67PacketTime::Us1000 => 1000,
            Aes67PacketTime::Us4000 => 4000,
        }
    }

    /// Get the packet time in milliseconds as a string for SDP.
    #[must_use]
    pub fn sdp_string(&self) -> &'static str {
        match self {
            Aes67PacketTime::Us125 => "0.125",
            Aes67PacketTime::Us250 => "0.25",
            Aes67PacketTime::Us333 => "0.333",
            Aes67PacketTime::Us1000 => "1",
            Aes67PacketTime::Us4000 => "4",
        }
    }
}

impl fmt::Display for Aes67PacketTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} µs", self.microseconds())
    }
}

/// AES67 latency breakdown.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Aes67Latency {
    /// Network transmission latency in microseconds
    pub network_latency_us: u32,
    /// Jitter buffer depth in microseconds
    pub buffer_latency_us: u32,
    /// Processing (DSP) latency in microseconds
    pub processing_latency_us: u32,
}

impl Aes67Latency {
    /// Create a new latency descriptor.
    #[must_use]
    pub fn new(
        network_latency_us: u32,
        buffer_latency_us: u32,
        processing_latency_us: u32,
    ) -> Self {
        Self {
            network_latency_us,
            buffer_latency_us,
            processing_latency_us,
        }
    }

    /// Get the total end-to-end latency in microseconds.
    #[must_use]
    pub fn total_us(&self) -> u32 {
        self.network_latency_us
            .saturating_add(self.buffer_latency_us)
            .saturating_add(self.processing_latency_us)
    }

    /// Get the total latency in milliseconds.
    #[must_use]
    pub fn total_ms(&self) -> f32 {
        self.total_us() as f32 / 1000.0
    }
}

/// RTP timestamp utilities for AES67.
#[allow(dead_code)]
pub struct RtpTimestamp;

impl RtpTimestamp {
    /// Convert a sample count to a wrapping 32-bit RTP timestamp.
    ///
    /// The RTP timestamp wraps at 2^32 as per RFC 3550.
    #[must_use]
    pub fn from_sample_count(samples: u64, _sample_rate: u32) -> u32 {
        // RTP timestamp is based on sample count, wrapping at u32::MAX
        (samples & 0xFFFF_FFFF) as u32
    }

    /// Compute the difference between two RTP timestamps, handling wrap-around.
    #[must_use]
    pub fn diff(a: u32, b: u32) -> i32 {
        // Signed difference handling 32-bit wrap
        a.wrapping_sub(b) as i32
    }
}

/// AES67 stream descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Aes67StreamDescriptor {
    /// Multicast IP address for the stream
    pub multicast_addr: String,
    /// UDP port number
    pub port: u16,
    /// Audio configuration
    pub config: Aes67Config,
    /// SDP description (Session Description Protocol)
    pub sdp: String,
}

impl Aes67StreamDescriptor {
    /// Create a new stream descriptor.
    #[must_use]
    pub fn new(multicast_addr: String, port: u16, config: Aes67Config) -> Self {
        let sdp = Aes67Sdp::generate_internal(&multicast_addr, port, &config);
        Self {
            multicast_addr,
            port,
            config,
            sdp,
        }
    }
}

/// AES67 SDP (Session Description Protocol) generator.
#[allow(dead_code)]
pub struct Aes67Sdp;

impl Aes67Sdp {
    /// Generate an SDP description for an AES67 stream.
    ///
    /// Produces an RFC 4566 compliant SDP with AES67 specific attributes.
    #[must_use]
    pub fn generate(descriptor: &Aes67StreamDescriptor) -> String {
        Self::generate_internal(
            &descriptor.multicast_addr,
            descriptor.port,
            &descriptor.config,
        )
    }

    fn generate_internal(multicast_addr: &str, port: u16, config: &Aes67Config) -> String {
        let encoding = match config.bit_depth {
            16 => "L16",
            24 => "L24",
            32 => "L32",
            _ => "L24",
        };

        let packet_time_ms = config.packet_time_us as f32 / 1000.0;

        format!(
            "v=0\r\n\
             o=- 0 0 IN IP4 0.0.0.0\r\n\
             s=AES67 Stream\r\n\
             c=IN IP4 {multicast_addr}/32\r\n\
             t=0 0\r\n\
             m=audio {port} RTP/AVP 96\r\n\
             a=rtpmap:96 {encoding}/{sample_rate}/{channels}\r\n\
             a=ptime:{packet_time_ms}\r\n\
             a=ts-refclk:ptp=IEEE1588-2008\r\n\
             a=mediaclk:direct=0\r\n",
            multicast_addr = multicast_addr,
            port = port,
            encoding = encoding,
            sample_rate = config.sample_rate,
            channels = config.channels,
            packet_time_ms = packet_time_ms,
        )
    }
}

/// AES67 jitter buffer for compensating network timing variation.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Aes67JitterBuffer {
    /// Buffer depth in microseconds
    pub depth_us: u32,
    /// Current fill level in microseconds
    pub fill_level_us: u32,
    /// Packet arrival history: (arrival_us, rtp_timestamp)
    packets: Vec<(u64, u32)>,
    /// Sample rate for timestamp calculations
    sample_rate: u32,
}

impl Aes67JitterBuffer {
    /// Create a new jitter buffer with the specified depth.
    #[must_use]
    pub fn new(depth_us: u32, sample_rate: u32) -> Self {
        Self {
            depth_us,
            fill_level_us: 0,
            packets: Vec::new(),
            sample_rate,
        }
    }

    /// Add a packet to the jitter buffer.
    ///
    /// `arrival_us` is the wall-clock arrival time in microseconds.
    /// `timestamp` is the RTP timestamp from the packet header.
    pub fn add_packet(&mut self, arrival_us: u64, timestamp: u32) {
        self.packets.push((arrival_us, timestamp));
        // Keep only recent packets (last 100)
        if self.packets.len() > 100 {
            self.packets.remove(0);
        }
        self.update_fill_level();
    }

    /// Update the estimated fill level based on recent packet arrivals.
    fn update_fill_level(&mut self) {
        if self.packets.len() < 2 {
            self.fill_level_us = self.depth_us / 2;
            return;
        }

        // Estimate jitter from arrival time variation
        let n = self.packets.len();
        let recent = &self.packets[n.saturating_sub(8)..];

        if recent.len() >= 2 {
            let first = recent[0].0;
            let last = recent[recent.len() - 1].0;
            let span_us = last.saturating_sub(first) as u32;

            // Fill level is the difference between expected and actual span
            self.fill_level_us = span_us.min(self.depth_us);
        }
    }

    /// Check if the buffer is in underrun (empty).
    #[must_use]
    pub fn is_underrun(&self) -> bool {
        self.fill_level_us == 0 && !self.packets.is_empty()
    }

    /// Check if the buffer is in overrun (full).
    #[must_use]
    pub fn is_overrun(&self) -> bool {
        self.fill_level_us >= self.depth_us
    }

    /// Get the number of buffered packets.
    #[must_use]
    pub fn packet_count(&self) -> usize {
        self.packets.len()
    }

    /// Reset the jitter buffer.
    pub fn reset(&mut self) {
        self.packets.clear();
        self.fill_level_us = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aes67_config_standard() {
        let cfg = Aes67Config::standard();
        assert_eq!(cfg.sample_rate, 48000);
        assert_eq!(cfg.channels, 2);
        assert_eq!(cfg.bit_depth, 24);
        assert_eq!(cfg.packet_time_us, 1000);
    }

    #[test]
    fn test_aes67_config_samples_per_packet() {
        let cfg = Aes67Config::standard(); // 48kHz, 1ms
        assert_eq!(cfg.samples_per_packet(), 48);
    }

    #[test]
    fn test_aes67_config_payload_bytes() {
        let cfg = Aes67Config::standard(); // 48 samples, 2ch, 3 bytes each
        assert_eq!(cfg.packet_payload_bytes(), 48 * 2 * 3);
    }

    #[test]
    fn test_aes67_packet_time_microseconds() {
        assert_eq!(Aes67PacketTime::Us125.microseconds(), 125);
        assert_eq!(Aes67PacketTime::Us250.microseconds(), 250);
        assert_eq!(Aes67PacketTime::Us333.microseconds(), 333);
        assert_eq!(Aes67PacketTime::Us1000.microseconds(), 1000);
        assert_eq!(Aes67PacketTime::Us4000.microseconds(), 4000);
    }

    #[test]
    fn test_aes67_latency_total() {
        let latency = Aes67Latency::new(500, 1000, 250);
        assert_eq!(latency.total_us(), 1750);
    }

    #[test]
    fn test_aes67_latency_total_ms() {
        let latency = Aes67Latency::new(500, 1000, 500);
        assert!((latency.total_ms() - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_rtp_timestamp_from_sample_count() {
        // Normal case: no wrap
        let ts = RtpTimestamp::from_sample_count(48000, 48000);
        assert_eq!(ts, 48000);

        // Wrap case
        let large_count = u64::from(u32::MAX) + 100;
        let ts_wrap = RtpTimestamp::from_sample_count(large_count, 48000);
        assert_eq!(ts_wrap, 99); // wraps around
    }

    #[test]
    fn test_rtp_timestamp_diff() {
        let diff = RtpTimestamp::diff(1000, 500);
        assert_eq!(diff, 500);

        // Wrap-around difference
        let diff_wrap = RtpTimestamp::diff(100, u32::MAX - 100);
        assert_eq!(diff_wrap, 201);
    }

    #[test]
    fn test_aes67_sdp_generate() {
        let descriptor =
            Aes67StreamDescriptor::new("239.69.0.1".to_string(), 5004, Aes67Config::standard());
        let sdp = Aes67Sdp::generate(&descriptor);
        assert!(sdp.contains("a=rtpmap:96 L24/48000/2"));
        assert!(sdp.contains("239.69.0.1"));
        assert!(sdp.contains("5004"));
    }

    #[test]
    fn test_aes67_jitter_buffer() {
        let mut buf = Aes67JitterBuffer::new(2000, 48000);
        assert!(!buf.is_underrun());
        assert_eq!(buf.packet_count(), 0);

        buf.add_packet(1_000_000, 0);
        buf.add_packet(1_001_000, 48);
        buf.add_packet(1_002_000, 96);
        assert_eq!(buf.packet_count(), 3);
    }

    #[test]
    fn test_aes67_jitter_buffer_reset() {
        let mut buf = Aes67JitterBuffer::new(2000, 48000);
        buf.add_packet(1_000_000, 0);
        buf.reset();
        assert_eq!(buf.packet_count(), 0);
        assert_eq!(buf.fill_level_us, 0);
    }
}
