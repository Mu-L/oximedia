//! SMPTE ST 2110 media-over-IP support.
//!
//! Implements SMPTE ST 2110 standards for professional video transport over IP networks.
//! Covers ST 2110-20 (video), ST 2110-30 (audio), and ancillary data.

#![allow(dead_code)]

/// SMPTE ST 2110-20 video stream configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct St2110_20Config {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Frame rate as (numerator, denominator) rational.
    pub fps: (u32, u32),
    /// Whether the video is interlaced.
    pub interlaced: bool,
    /// Colorspace standard.
    pub colorspace: St2110Colorspace,
    /// Sampling format.
    pub sampling: St2110Sampling,
}

impl St2110_20Config {
    /// Creates a new ST 2110-20 configuration.
    #[must_use]
    pub const fn new(
        width: u32,
        height: u32,
        fps: (u32, u32),
        interlaced: bool,
        colorspace: St2110Colorspace,
        sampling: St2110Sampling,
    ) -> Self {
        Self {
            width,
            height,
            fps,
            interlaced,
            colorspace,
            sampling,
        }
    }

    /// Returns the total number of active pixels per frame.
    #[must_use]
    pub fn pixel_count(&self) -> u32 {
        self.width * self.height
    }

    /// Returns the frame rate as a floating-point value.
    #[must_use]
    pub fn fps_f64(&self) -> f64 {
        f64::from(self.fps.0) / f64::from(self.fps.1)
    }
}

impl Default for St2110_20Config {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: (30, 1),
            interlaced: false,
            colorspace: St2110Colorspace::BT709,
            sampling: St2110Sampling::Yuv422_10bit,
        }
    }
}

/// SMPTE ST 2110 colorspace standards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum St2110Colorspace {
    /// ITU-R BT.709 (HDTV standard).
    BT709,
    /// ITU-R BT.2020 (UHDTV/HDR standard).
    BT2020,
    /// ITU-R BT.601 (SDTV standard).
    BT601,
}

impl St2110Colorspace {
    /// Returns the numeric code for the colorspace as used in SDP.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            St2110Colorspace::BT709 => 1,
            St2110Colorspace::BT2020 => 9,
            St2110Colorspace::BT601 => 5,
        }
    }
}

/// SMPTE ST 2110 pixel sampling formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum St2110Sampling {
    /// YCbCr 4:2:2, 10 bits per component.
    Yuv422_10bit,
    /// YCbCr 4:2:2, 12 bits per component.
    Yuv422_12bit,
    /// RGB 4:4:4, 10 bits per component.
    Rgb444_10bit,
    /// RGB 4:4:4, 12 bits per component.
    Rgb444_12bit,
}

impl St2110Sampling {
    /// Returns the number of bits per sample for a single component.
    #[must_use]
    pub const fn bits_per_sample(self) -> u8 {
        match self {
            St2110Sampling::Yuv422_10bit | St2110Sampling::Rgb444_10bit => 10,
            St2110Sampling::Yuv422_12bit | St2110Sampling::Rgb444_12bit => 12,
        }
    }

    /// Returns true if the sampling is chroma-subsampled (4:2:2).
    #[must_use]
    pub const fn is_subsampled(self) -> bool {
        matches!(
            self,
            St2110Sampling::Yuv422_10bit | St2110Sampling::Yuv422_12bit
        )
    }
}

/// SMPTE ST 2110-30 audio stream configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct St2110_30Config {
    /// Audio sample rate in Hz.
    pub sample_rate: u32,
    /// Number of audio channels.
    pub channels: u8,
    /// Bit depth per sample (16, 24, or 32).
    pub bit_depth: u8,
    /// Packet time in microseconds (125, 250, 333, 1000, 4000).
    pub packet_time_us: u32,
}

impl St2110_30Config {
    /// Creates a new ST 2110-30 configuration.
    #[must_use]
    pub const fn new(sample_rate: u32, channels: u8, bit_depth: u8, packet_time_us: u32) -> Self {
        Self {
            sample_rate,
            channels,
            bit_depth,
            packet_time_us,
        }
    }

    /// Returns samples per packet based on packet time and sample rate.
    #[must_use]
    pub fn samples_per_packet(&self) -> u32 {
        (u64::from(self.sample_rate) * u64::from(self.packet_time_us) / 1_000_000) as u32
    }

    /// Returns the audio payload size in bytes per packet.
    #[must_use]
    pub fn payload_size(&self) -> u32 {
        self.samples_per_packet() * u32::from(self.channels) * (u32::from(self.bit_depth) / 8)
    }
}

impl Default for St2110_30Config {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            channels: 2,
            bit_depth: 24,
            packet_time_us: 1_000,
        }
    }
}

/// Stream type for ST 2110 packets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamType {
    /// ST 2110-20 video stream.
    Video,
    /// ST 2110-30 audio stream.
    Audio,
    /// ST 2110-40 ancillary data stream.
    Ancillary,
}

impl StreamType {
    /// Returns the RTP payload type for this stream type.
    #[must_use]
    pub const fn default_payload_type(self) -> u8 {
        match self {
            StreamType::Video => 96,
            StreamType::Audio => 97,
            StreamType::Ancillary => 100,
        }
    }
}

/// A SMPTE ST 2110 packet.
#[derive(Debug, Clone)]
pub struct St2110Packet {
    /// Type of stream this packet belongs to.
    pub stream_type: StreamType,
    /// RTP sequence number.
    pub sequence: u32,
    /// RTP timestamp.
    pub timestamp: u32,
    /// Packet payload data.
    pub payload: Vec<u8>,
}

impl St2110Packet {
    /// Creates a new ST 2110 packet.
    #[must_use]
    pub fn new(stream_type: StreamType, sequence: u32, timestamp: u32, payload: Vec<u8>) -> Self {
        Self {
            stream_type,
            sequence,
            timestamp,
            payload,
        }
    }

    /// Returns the total packet size including RTP header.
    #[must_use]
    pub fn total_size(&self) -> usize {
        St2110RtpHeader::SIZE + self.payload.len()
    }
}

/// SMPTE ST 2110 RTP header (12 bytes, fixed size per RFC 3550).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct St2110RtpHeader {
    /// RTP version (must be 2).
    pub version: u8,
    /// Padding flag.
    pub padding: bool,
    /// Extension flag.
    pub extension: bool,
    /// CSRC count.
    pub csrc_count: u8,
    /// Marker bit (end of frame in video).
    pub marker: bool,
    /// Payload type.
    pub payload_type: u8,
    /// Sequence number.
    pub sequence: u16,
    /// RTP timestamp.
    pub timestamp: u32,
    /// Synchronization source identifier.
    pub ssrc: u32,
}

impl St2110RtpHeader {
    /// Size of the fixed RTP header in bytes.
    pub const SIZE: usize = 12;

    /// Creates a new RTP header.
    #[must_use]
    pub const fn new(
        payload_type: u8,
        sequence: u16,
        timestamp: u32,
        ssrc: u32,
        marker: bool,
    ) -> Self {
        Self {
            version: 2,
            padding: false,
            extension: false,
            csrc_count: 0,
            marker,
            payload_type,
            sequence,
            timestamp,
            ssrc,
        }
    }

    /// Encodes the RTP header into a 12-byte vector.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::SIZE);

        // Byte 0: V(2) P(1) X(1) CC(4)
        let byte0 = (self.version & 0x03) << 6
            | u8::from(self.padding) << 5
            | u8::from(self.extension) << 4
            | (self.csrc_count & 0x0F);
        buf.push(byte0);

        // Byte 1: M(1) PT(7)
        let byte1 = u8::from(self.marker) << 7 | (self.payload_type & 0x7F);
        buf.push(byte1);

        // Bytes 2-3: Sequence number (big-endian)
        buf.push((self.sequence >> 8) as u8);
        buf.push(self.sequence as u8);

        // Bytes 4-7: Timestamp (big-endian)
        buf.push((self.timestamp >> 24) as u8);
        buf.push((self.timestamp >> 16) as u8);
        buf.push((self.timestamp >> 8) as u8);
        buf.push(self.timestamp as u8);

        // Bytes 8-11: SSRC (big-endian)
        buf.push((self.ssrc >> 24) as u8);
        buf.push((self.ssrc >> 16) as u8);
        buf.push((self.ssrc >> 8) as u8);
        buf.push(self.ssrc as u8);

        buf
    }

    /// Decodes an RTP header from a byte slice.
    ///
    /// Returns `None` if the data is too short or the version is invalid.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE {
            return None;
        }

        let byte0 = data[0];
        let version = (byte0 >> 6) & 0x03;
        if version != 2 {
            return None;
        }

        let padding = (byte0 >> 5) & 0x01 != 0;
        let extension = (byte0 >> 4) & 0x01 != 0;
        let csrc_count = byte0 & 0x0F;

        let byte1 = data[1];
        let marker = (byte1 >> 7) != 0;
        let payload_type = byte1 & 0x7F;

        let sequence = u16::from_be_bytes([data[2], data[3]]);
        let timestamp = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let ssrc = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

        Some(Self {
            version,
            padding,
            extension,
            csrc_count,
            marker,
            payload_type,
            sequence,
            timestamp,
            ssrc,
        })
    }
}

/// ST 2110-20 line header for video payload.
///
/// Each line in a video packet is preceded by a 6-byte line header
/// per the ST 2110-20 specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineHeader {
    /// Video line number (0-based from the top of the frame).
    pub line_no: u16,
    /// Horizontal pixel offset within the line.
    pub offset: u16,
    /// Continuation flag: true if more lines follow in this packet.
    pub continuation: bool,
}

impl LineHeader {
    /// Size of the line header in bytes.
    pub const SIZE: usize = 6;

    /// Creates a new line header.
    #[must_use]
    pub const fn new(line_no: u16, offset: u16, continuation: bool) -> Self {
        Self {
            line_no,
            offset,
            continuation,
        }
    }

    /// Encodes the line header into bytes.
    ///
    /// Format (per ST 2110-20):
    /// - Bits 15:0: Line number
    /// - Bits 31:16: Offset (with C-bit in MSB)
    /// - Bytes 4-5: Length (set to 0 here, filled by caller)
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::SIZE);

        // Bytes 0-1: Line number
        buf.push((self.line_no >> 8) as u8);
        buf.push(self.line_no as u8);

        // Bytes 2-3: C-bit | Offset
        let c_bit: u16 = if self.continuation { 0x8000 } else { 0 };
        let offset_field = c_bit | (self.offset & 0x7FFF);
        buf.push((offset_field >> 8) as u8);
        buf.push(offset_field as u8);

        // Bytes 4-5: Length (placeholder, 0)
        buf.push(0);
        buf.push(0);

        buf
    }

    /// Decodes a line header from bytes.
    ///
    /// Returns `None` if data is too short.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < Self::SIZE {
            return None;
        }

        let line_no = u16::from_be_bytes([data[0], data[1]]);
        let offset_field = u16::from_be_bytes([data[2], data[3]]);
        let continuation = (offset_field & 0x8000) != 0;
        let offset = offset_field & 0x7FFF;

        Some(Self {
            line_no,
            offset,
            continuation,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_st2110_20_config_default() {
        let config = St2110_20Config::default();
        assert_eq!(config.width, 1920);
        assert_eq!(config.height, 1080);
        assert_eq!(config.fps, (30, 1));
        assert!(!config.interlaced);
    }

    #[test]
    fn test_st2110_20_pixel_count() {
        let config = St2110_20Config::default();
        assert_eq!(config.pixel_count(), 1920 * 1080);
    }

    #[test]
    fn test_st2110_20_fps_f64() {
        let config = St2110_20Config {
            fps: (30000, 1001),
            ..St2110_20Config::default()
        };
        assert!((config.fps_f64() - 29.97).abs() < 0.01);
    }

    #[test]
    fn test_colorspace_codes() {
        assert_eq!(St2110Colorspace::BT709.code(), 1);
        assert_eq!(St2110Colorspace::BT2020.code(), 9);
        assert_eq!(St2110Colorspace::BT601.code(), 5);
    }

    #[test]
    fn test_sampling_bits_per_sample() {
        assert_eq!(St2110Sampling::Yuv422_10bit.bits_per_sample(), 10);
        assert_eq!(St2110Sampling::Yuv422_12bit.bits_per_sample(), 12);
        assert_eq!(St2110Sampling::Rgb444_10bit.bits_per_sample(), 10);
        assert_eq!(St2110Sampling::Rgb444_12bit.bits_per_sample(), 12);
    }

    #[test]
    fn test_sampling_is_subsampled() {
        assert!(St2110Sampling::Yuv422_10bit.is_subsampled());
        assert!(St2110Sampling::Yuv422_12bit.is_subsampled());
        assert!(!St2110Sampling::Rgb444_10bit.is_subsampled());
        assert!(!St2110Sampling::Rgb444_12bit.is_subsampled());
    }

    #[test]
    fn test_st2110_30_config_default() {
        let config = St2110_30Config::default();
        assert_eq!(config.sample_rate, 48_000);
        assert_eq!(config.channels, 2);
        assert_eq!(config.bit_depth, 24);
        assert_eq!(config.packet_time_us, 1_000);
    }

    #[test]
    fn test_st2110_30_samples_per_packet() {
        let config = St2110_30Config::new(48_000, 2, 24, 1_000);
        assert_eq!(config.samples_per_packet(), 48);
    }

    #[test]
    fn test_st2110_30_payload_size() {
        let config = St2110_30Config::new(48_000, 2, 24, 1_000);
        // 48 samples * 2 channels * 3 bytes = 288 bytes
        assert_eq!(config.payload_size(), 288);
    }

    #[test]
    fn test_rtp_header_encode_decode_roundtrip() {
        let header = St2110RtpHeader::new(96, 1234, 90_000, 0xDEAD_BEEF, true);
        let encoded = header.encode();
        assert_eq!(encoded.len(), St2110RtpHeader::SIZE);

        let decoded = St2110RtpHeader::decode(&encoded).expect("should succeed in test");
        assert_eq!(decoded.version, 2);
        assert_eq!(decoded.payload_type, 96);
        assert_eq!(decoded.sequence, 1234);
        assert_eq!(decoded.timestamp, 90_000);
        assert_eq!(decoded.ssrc, 0xDEAD_BEEF);
        assert!(decoded.marker);
        assert!(!decoded.padding);
        assert!(!decoded.extension);
    }

    #[test]
    fn test_rtp_header_size() {
        let header = St2110RtpHeader::new(96, 0, 0, 0, false);
        assert_eq!(header.encode().len(), 12);
    }

    #[test]
    fn test_rtp_header_decode_too_short() {
        let data = [0u8; 5];
        assert!(St2110RtpHeader::decode(&data).is_none());
    }

    #[test]
    fn test_rtp_header_invalid_version() {
        let mut data = [0u8; 12];
        // version = 1 (not 2)
        data[0] = 0x40;
        assert!(St2110RtpHeader::decode(&data).is_none());
    }

    #[test]
    fn test_line_header_encode_decode_roundtrip() {
        let lh = LineHeader::new(540, 0, true);
        let encoded = lh.encode();
        assert_eq!(encoded.len(), LineHeader::SIZE);

        let decoded = LineHeader::decode(&encoded).expect("should succeed in test");
        assert_eq!(decoded.line_no, 540);
        assert_eq!(decoded.offset, 0);
        assert!(decoded.continuation);
    }

    #[test]
    fn test_line_header_no_continuation() {
        let lh = LineHeader::new(1079, 100, false);
        let encoded = lh.encode();
        let decoded = LineHeader::decode(&encoded).expect("should succeed in test");
        assert_eq!(decoded.line_no, 1079);
        assert_eq!(decoded.offset, 100);
        assert!(!decoded.continuation);
    }

    #[test]
    fn test_line_header_decode_too_short() {
        let data = [0u8; 3];
        assert!(LineHeader::decode(&data).is_none());
    }

    #[test]
    fn test_stream_type_payload_types() {
        assert_eq!(StreamType::Video.default_payload_type(), 96);
        assert_eq!(StreamType::Audio.default_payload_type(), 97);
        assert_eq!(StreamType::Ancillary.default_payload_type(), 100);
    }

    #[test]
    fn test_st2110_packet_total_size() {
        let pkt = St2110Packet::new(StreamType::Video, 1, 90_000, vec![0u8; 100]);
        assert_eq!(pkt.total_size(), St2110RtpHeader::SIZE + 100);
    }
}
