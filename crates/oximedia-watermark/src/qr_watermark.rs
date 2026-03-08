//! QR code based visual watermarking.
//!
//! Provides structures and functions for embedding QR-code-style patterns
//! into video frames as a watermarking strategy.

/// Error-correction level for QR payload encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcLevel {
    /// Low error correction (~7% redundancy).
    L,
    /// Medium error correction (~15% redundancy).
    M,
    /// Quartile error correction (~25% redundancy).
    Q,
    /// High error correction (~30% redundancy).
    H,
}

impl EcLevel {
    /// Return the approximate redundancy percentage for this EC level.
    #[must_use]
    pub fn redundancy_pct(self) -> u32 {
        match self {
            EcLevel::L => 7,
            EcLevel::M => 15,
            EcLevel::Q => 25,
            EcLevel::H => 30,
        }
    }
}

/// Payload carried by a QR watermark.
#[derive(Debug, Clone)]
pub struct QrPayload {
    /// QR version (1–40), determines module count.
    pub version: u8,
    /// Error-correction level.
    pub error_correction: EcLevel,
    /// Raw data bytes to encode.
    pub data: Vec<u8>,
}

impl QrPayload {
    /// Create a new payload.
    #[must_use]
    pub fn new(version: u8, error_correction: EcLevel, data: Vec<u8>) -> Self {
        Self {
            version,
            error_correction,
            data,
        }
    }

    /// Calculate the encoded size in bytes, accounting for error-correction overhead.
    ///
    /// Returns `data.len()` inflated by the EC-level redundancy percentage.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn encoded_size(&self) -> usize {
        let pct = self.error_correction.redundancy_pct() as usize;
        let overhead = (self.data.len() * pct).div_ceil(100);
        self.data.len() + overhead
    }
}

/// QR watermark embedder.
#[derive(Debug, Clone)]
pub struct QrWatermark {
    /// Size of each QR module in pixels.
    pub module_size_px: u32,
    /// Quiet zone width in modules.
    pub quiet_zone: u32,
    /// Payload to embed.
    pub payload: QrPayload,
}

impl QrWatermark {
    /// Create a new QR watermark.
    #[must_use]
    pub fn new(module_size_px: u32, quiet_zone: u32, payload: QrPayload) -> Self {
        Self {
            module_size_px,
            quiet_zone,
            payload,
        }
    }

    /// Calculate the total image size in pixels for this QR watermark.
    ///
    /// QR version V has `(17 + 4*V)` modules plus 2 × `quiet_zone` on each side.
    #[must_use]
    pub fn image_size_px(&self) -> u32 {
        let v = u32::from(self.payload.version.max(1).min(40));
        let modules = 17 + 4 * v + 2 * self.quiet_zone;
        modules * self.module_size_px
    }

    /// Embed the QR watermark into a raw RGB frame at position `(x, y)`.
    ///
    /// `frame` must be a flat `width * height * 3` byte buffer (RGB).
    /// Returns `true` if the watermark fits within the frame, `false` otherwise.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn embed_in_frame(&self, frame: &mut [u8], width: usize, x: u32, y: u32) -> bool {
        let size = self.image_size_px();
        let height = if width == 0 {
            0
        } else {
            frame.len() / (width * 3)
        };

        if x + size > width as u32 || y + size > height as u32 {
            return false;
        }

        // Render a checkerboard pattern as a placeholder for the QR modules.
        let v = u32::from(self.payload.version.max(1).min(40));
        let total_modules = 17 + 4 * v + 2 * self.quiet_zone;

        for row in 0..total_modules {
            for col in 0..total_modules {
                // Quiet zone is white; interior follows checkerboard.
                let in_quiet = row < self.quiet_zone
                    || row >= total_modules - self.quiet_zone
                    || col < self.quiet_zone
                    || col >= total_modules - self.quiet_zone;

                let dark = if in_quiet {
                    false
                } else {
                    (row + col) % 2 == 0
                };
                let color: u8 = if dark { 0 } else { 255 };

                for py in 0..self.module_size_px {
                    for px in 0..self.module_size_px {
                        let fx = (x + col * self.module_size_px + px) as usize;
                        let fy = (y + row * self.module_size_px + py) as usize;
                        let base = (fy * width + fx) * 3;
                        if base + 2 < frame.len() {
                            frame[base] = color;
                            frame[base + 1] = color;
                            frame[base + 2] = color;
                        }
                    }
                }
            }
        }
        true
    }
}

/// Encode data using FNV-1a hashing as a simple content fingerprint.
///
/// The input is split into 4-byte blocks; each block's FNV-1a hash is appended
/// to the output as 4 bytes (little-endian).
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn fnv_encode(data: &[u8]) -> Vec<u8> {
    const FNV_PRIME: u32 = 0x0100_0193;
    const FNV_OFFSET: u32 = 0x811c_9dc5;

    let mut out = Vec::with_capacity(data.len() + (data.len() / 4 + 1) * 4);
    out.extend_from_slice(data);

    // Append hash of each 4-byte chunk.
    for chunk in data.chunks(4) {
        let mut hash = FNV_OFFSET;
        for &b in chunk {
            hash ^= u32::from(b);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        out.extend_from_slice(&hash.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ec_level_l_redundancy() {
        assert_eq!(EcLevel::L.redundancy_pct(), 7);
    }

    #[test]
    fn test_ec_level_m_redundancy() {
        assert_eq!(EcLevel::M.redundancy_pct(), 15);
    }

    #[test]
    fn test_ec_level_q_redundancy() {
        assert_eq!(EcLevel::Q.redundancy_pct(), 25);
    }

    #[test]
    fn test_ec_level_h_redundancy() {
        assert_eq!(EcLevel::H.redundancy_pct(), 30);
    }

    #[test]
    fn test_payload_encoded_size_larger_than_data() {
        let p = QrPayload::new(1, EcLevel::M, vec![0u8; 100]);
        assert!(p.encoded_size() > 100);
    }

    #[test]
    fn test_payload_encoded_size_empty() {
        let p = QrPayload::new(1, EcLevel::L, vec![]);
        assert_eq!(p.encoded_size(), 0);
    }

    #[test]
    fn test_payload_encoded_size_h_level() {
        let data = vec![0u8; 10];
        let p = QrPayload::new(1, EcLevel::H, data);
        // 30% overhead → 3 bytes overhead → 13 total
        assert_eq!(p.encoded_size(), 13);
    }

    #[test]
    fn test_image_size_px_version1() {
        let payload = QrPayload::new(1, EcLevel::M, vec![0]);
        let qr = QrWatermark::new(2, 4, payload);
        // modules = 17 + 4*1 + 2*4 = 29; px = 29 * 2 = 58
        assert_eq!(qr.image_size_px(), 58);
    }

    #[test]
    fn test_image_size_px_scales_with_module_size() {
        let p1 = QrPayload::new(1, EcLevel::L, vec![0]);
        let p2 = QrPayload::new(1, EcLevel::L, vec![0]);
        let qr1 = QrWatermark::new(1, 0, p1);
        let qr2 = QrWatermark::new(2, 0, p2);
        assert_eq!(qr2.image_size_px(), qr1.image_size_px() * 2);
    }

    #[test]
    fn test_embed_in_frame_success() {
        let payload = QrPayload::new(1, EcLevel::L, vec![42]);
        let qr = QrWatermark::new(1, 0, payload);
        let size = qr.image_size_px() as usize;
        let mut frame = vec![128u8; size * size * 3];
        let result = qr.embed_in_frame(&mut frame, size, 0, 0);
        assert!(result);
    }

    #[test]
    fn test_embed_in_frame_out_of_bounds() {
        let payload = QrPayload::new(1, EcLevel::L, vec![0]);
        let qr = QrWatermark::new(2, 2, payload);
        // Frame too small
        let mut frame = vec![0u8; 10 * 10 * 3];
        let result = qr.embed_in_frame(&mut frame, 10, 8, 8);
        assert!(!result);
    }

    #[test]
    fn test_embed_modifies_frame() {
        let payload = QrPayload::new(1, EcLevel::L, vec![1]);
        let qr = QrWatermark::new(1, 0, payload);
        let size = qr.image_size_px() as usize;
        let mut frame = vec![128u8; size * size * 3];
        let _ = qr.embed_in_frame(&mut frame, size, 0, 0);
        // Some pixels should have changed to 0 or 255
        let has_black = frame.iter().any(|&b| b == 0);
        let has_white = frame.iter().any(|&b| b == 255);
        assert!(has_black || has_white);
    }

    #[test]
    fn test_fnv_encode_non_empty() {
        let data = b"hello";
        let encoded = fnv_encode(data);
        assert!(encoded.len() > data.len());
    }

    #[test]
    fn test_fnv_encode_empty() {
        let encoded = fnv_encode(&[]);
        assert!(encoded.is_empty());
    }

    #[test]
    fn test_fnv_encode_deterministic() {
        let data = b"watermark";
        let a = fnv_encode(data);
        let b = fnv_encode(data);
        assert_eq!(a, b);
    }

    #[test]
    fn test_fnv_encode_different_inputs_differ() {
        let a = fnv_encode(b"abc");
        let b = fnv_encode(b"xyz");
        assert_ne!(a, b);
    }
}
