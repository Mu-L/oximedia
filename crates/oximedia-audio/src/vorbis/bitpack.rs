//! Bitstream packing for Vorbis encoding.
//!
//! Vorbis uses LSB-first bit packing for its bitstream format.

#![forbid(unsafe_code)]

/// Bitstream packer for Vorbis encoding.
#[derive(Debug, Clone)]
pub struct BitPacker {
    /// Accumulated bytes.
    bytes: Vec<u8>,
    /// Current byte being packed.
    current_byte: u8,
    /// Number of bits used in current byte.
    bit_position: u8,
}

impl BitPacker {
    /// Create a new bit packer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            current_byte: 0,
            bit_position: 0,
        }
    }

    /// Write a single bit.
    pub fn write_bit(&mut self, bit: bool) {
        if bit {
            self.current_byte |= 1 << self.bit_position;
        }
        self.bit_position += 1;

        if self.bit_position == 8 {
            self.flush_byte();
        }
    }

    /// Write multiple bits (LSB first).
    pub fn write_bits(&mut self, value: u32, bits: u8) {
        for i in 0..bits {
            let bit = (value >> i) & 1;
            self.write_bit(bit != 0);
        }
    }

    /// Write a byte directly.
    pub fn write_byte(&mut self, byte: u8) {
        self.write_bits(u32::from(byte), 8);
    }

    /// Write multiple bytes.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_byte(byte);
        }
    }

    /// Write a signed integer.
    #[allow(clippy::cast_sign_loss)]
    pub fn write_signed(&mut self, value: i32, bits: u8) {
        let unsigned = value as u32;
        self.write_bits(unsigned, bits);
    }

    /// Flush current byte to buffer.
    fn flush_byte(&mut self) {
        self.bytes.push(self.current_byte);
        self.current_byte = 0;
        self.bit_position = 0;
    }

    /// Get the current bit position in the stream.
    #[must_use]
    pub fn position(&self) -> usize {
        self.bytes.len() * 8 + self.bit_position as usize
    }

    /// Finish packing and return the byte buffer.
    #[must_use]
    pub fn finish(mut self) -> Vec<u8> {
        if self.bit_position > 0 {
            self.flush_byte();
        }
        self.bytes
    }

    /// Get a reference to the current bytes (without consuming).
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Get the current size in bytes (including partial byte).
    #[must_use]
    pub fn size(&self) -> usize {
        let mut size = self.bytes.len();
        if self.bit_position > 0 {
            size += 1;
        }
        size
    }
}

impl Default for BitPacker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_bit() {
        let mut packer = BitPacker::new();
        packer.write_bit(true);
        packer.write_bit(false);
        packer.write_bit(true);
        packer.write_bit(true);
        packer.write_bit(false);
        packer.write_bit(false);
        packer.write_bit(false);
        packer.write_bit(false);

        let bytes = packer.finish();
        assert_eq!(bytes.len(), 1);
        assert_eq!(bytes[0], 0b0000_1101); // LSB first
    }

    #[test]
    fn test_write_bits() {
        let mut packer = BitPacker::new();
        packer.write_bits(0b1101, 4);
        packer.write_bits(0b0010, 4);

        let bytes = packer.finish();
        assert_eq!(bytes.len(), 1);
        assert_eq!(bytes[0], 0b0010_1101);
    }

    #[test]
    fn test_write_byte() {
        let mut packer = BitPacker::new();
        packer.write_byte(0xAB);
        packer.write_byte(0xCD);

        let bytes = packer.finish();
        assert_eq!(bytes, vec![0xAB, 0xCD]);
    }

    #[test]
    fn test_write_bytes() {
        let mut packer = BitPacker::new();
        packer.write_bytes(&[0x01, 0x02, 0x03]);

        let bytes = packer.finish();
        assert_eq!(bytes, vec![0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_write_across_bytes() {
        let mut packer = BitPacker::new();
        packer.write_bits(0xFF, 6); // 6 bits of 1s: 0b111111
        packer.write_bits(0xFF, 6); // 6 more bits of 1s: 0b111111

        // Total: 12 bits of 1s = 0b111111111111
        // Byte 0 (bits 0-7): 0b11111111 = 0xFF = 255
        // Byte 1 (bits 8-11): 0b00001111 = 0x0F = 15
        let bytes = packer.finish();
        assert_eq!(bytes.len(), 2);
        assert_eq!(bytes[0], 0xFF); // All 8 bits set
        assert_eq!(bytes[1], 0x0F); // 4 bits set (bits 0-3 since LSB)
    }

    #[test]
    fn test_position() {
        let mut packer = BitPacker::new();
        assert_eq!(packer.position(), 0);

        packer.write_bits(0, 3);
        assert_eq!(packer.position(), 3);

        packer.write_bits(0, 5);
        assert_eq!(packer.position(), 8);

        packer.write_bits(0, 4);
        assert_eq!(packer.position(), 12);
    }

    #[test]
    fn test_size() {
        let mut packer = BitPacker::new();
        assert_eq!(packer.size(), 0);

        packer.write_bits(0, 3);
        assert_eq!(packer.size(), 1); // Partial byte

        packer.write_bits(0, 5);
        assert_eq!(packer.size(), 1); // Complete byte

        packer.write_bits(0, 4);
        assert_eq!(packer.size(), 2); // One complete, one partial
    }

    #[test]
    fn test_signed() {
        let mut packer = BitPacker::new();
        packer.write_signed(-1, 8);

        let bytes = packer.finish();
        assert_eq!(bytes[0], 0xFF);
    }
}
