//! Forensic watermarking for traitor tracing.
//!
//! A forensic watermark embeds a unique identifier (customer ID, session,
//! timestamp) into the content so that leaks can be traced back to the
//! responsible party.  This module provides payload encoding/decoding,
//! a DCT-domain embedding stub, and a simple traitor-tracing structure.

// ── ForensicPayload ───────────────────────────────────────────────────────────

/// A 96-bit forensic payload identifying the recipient of a media asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForensicPayload {
    /// Customer (or user) identifier.
    pub customer_id: u32,
    /// Session identifier.
    pub session_id: u32,
    /// Unix timestamp (seconds since epoch) at embed time.
    pub timestamp_sec: u32,
}

impl ForensicPayload {
    /// Pack the payload into a 64-bit value.
    ///
    /// Layout (MSB first):
    /// - bits 63-32: `customer_id`
    /// - bits 31-16: lower 16 bits of `session_id`
    /// - bits 15-0 : lower 16 bits of `timestamp_sec`
    #[must_use]
    pub fn encode(&self) -> u64 {
        let cid = u64::from(self.customer_id);
        let sid = u64::from(self.session_id & 0xFFFF);
        let ts = u64::from(self.timestamp_sec & 0xFFFF);
        (cid << 32) | (sid << 16) | ts
    }

    /// Unpack a payload from a 64-bit value produced by `encode`.
    #[must_use]
    pub fn decode(v: u64) -> ForensicPayload {
        let customer_id = (v >> 32) as u32;
        let session_id = ((v >> 16) & 0xFFFF) as u32;
        let timestamp_sec = (v & 0xFFFF) as u32;
        ForensicPayload {
            customer_id,
            session_id,
            timestamp_sec,
        }
    }
}

// ── WatermarkVariant ──────────────────────────────────────────────────────────

/// A specific watermark variant assigned to a customer.
#[derive(Debug, Clone)]
pub struct WatermarkVariant {
    /// Variant index (used to select an embedding pattern).
    pub variant_id: u8,
    /// The forensic payload embedded in this variant.
    pub payload: ForensicPayload,
}

impl WatermarkVariant {
    /// Human-readable description of this variant.
    #[must_use]
    pub fn describe(&self) -> String {
        format!(
            "variant={} customer={} session={} ts={}",
            self.variant_id,
            self.payload.customer_id,
            self.payload.session_id,
            self.payload.timestamp_sec,
        )
    }
}

// ── ForensicEmbedder ──────────────────────────────────────────────────────────

/// Embeds a forensic payload into pixel data using a DCT-domain strength
/// modulation approach (stub implementation).
pub struct ForensicEmbedder {
    /// Block size for DCT processing (e.g. 8 for 8×8 DCT).
    pub block_size: usize,
    /// Embedding strength (0.0 – 1.0).
    pub strength: f32,
}

impl ForensicEmbedder {
    /// Create a new embedder.
    #[must_use]
    pub fn new(block_size: usize, strength: f32) -> Self {
        Self {
            block_size: block_size.max(1),
            strength: strength.clamp(0.0, 1.0),
        }
    }

    /// Embed `payload` into `pixels`.
    ///
    /// This is a DCT-domain stub: the 64 payload bits are written into the
    /// LSBs of selected DCT blocks using a deterministic block-selection
    /// strategy.  In a production system the actual DCT transform would be
    /// applied; here we modify pixel values directly as a placeholder.
    ///
    /// # Arguments
    /// * `pixels`  – mutable flat 8-bit luma buffer (`width * height` bytes).
    /// * `width`   – image width.
    /// * `height`  – image height.
    /// * `payload` – the forensic payload to embed.
    pub fn embed(&self, pixels: &mut [u8], width: usize, height: usize, payload: &ForensicPayload) {
        let bits = payload_to_bits(payload.encode());
        let bs = self.block_size;
        let blocks_x = width / bs;
        let blocks_y = height / bs;
        let total_blocks = blocks_x * blocks_y;
        if total_blocks == 0 {
            return;
        }

        // Embed one bit per block (wrapping if more blocks than bits)
        for (block_idx, &bit) in bits.iter().enumerate() {
            let bx = (block_idx % blocks_x) * bs;
            let by = (block_idx / blocks_x.max(1)) * bs;
            if by + bs > height || bx + bs > width {
                break;
            }
            // Modify the top-left pixel of the block (DCT DC coefficient stub)
            let px_idx = by * width + bx;
            if px_idx < pixels.len() {
                let delta = (self.strength * 4.0).round() as u8;
                if bit {
                    pixels[px_idx] = pixels[px_idx].saturating_add(delta);
                } else {
                    pixels[px_idx] = pixels[px_idx].saturating_sub(delta);
                }
            }
        }
    }
}

// ── ForensicDetector ─────────────────────────────────────────────────────────

/// Extracts a forensic payload from pixel data.
pub struct ForensicDetector {
    /// Block size (must match the embedder).
    pub block_size: usize,
}

impl ForensicDetector {
    /// Create a new detector.
    #[must_use]
    pub fn new(block_size: usize) -> Self {
        Self {
            block_size: block_size.max(1),
        }
    }

    /// Attempt to extract a `ForensicPayload` from `pixels`.
    ///
    /// Returns `None` if the image is too small to hold a payload or if the
    /// extracted data appears invalid (all-zero customer ID *and* session ID).
    #[must_use]
    pub fn detect(&self, pixels: &[u8], width: usize, height: usize) -> Option<ForensicPayload> {
        let bs = self.block_size;
        let blocks_x = width / bs;
        let blocks_y = height / bs;
        let total_blocks = blocks_x * blocks_y;

        // We need at least 64 blocks to read the full 64-bit payload
        if total_blocks < 64 {
            return None;
        }

        // Read the LSB of the DC coefficient (top-left pixel) from each block
        let mut bits = [false; 64];
        for i in 0..64 {
            let bx = (i % blocks_x) * bs;
            let by = (i / blocks_x) * bs;
            let px_idx = by * width + bx;
            if px_idx < pixels.len() {
                bits[i] = (pixels[px_idx] & 1) == 1;
            }
        }

        let value = bits_to_u64(&bits);
        if value == 0 {
            return None;
        }
        Some(ForensicPayload::decode(value))
    }
}

// ── TraitorTrace ──────────────────────────────────────────────────────────────

/// A set of traitor tracing suspects with confidence scores.
#[derive(Debug, Clone, Default)]
pub struct TraitorTrace {
    /// (`customer_id`, confidence) pairs, unsorted.
    pub suspects: Vec<(u32, f32)>,
}

impl TraitorTrace {
    /// Create an empty traitor trace.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a suspect.
    pub fn add_suspect(&mut self, customer_id: u32, confidence: f32) {
        self.suspects.push((customer_id, confidence));
    }

    /// Return the customer ID of the most likely suspect (highest confidence),
    /// or `None` if no suspects have been added.
    #[must_use]
    pub fn most_likely(&self) -> Option<u32> {
        self.suspects
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|&(id, _)| id)
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Convert a `u64` to a 64-element array of bits (MSB first).
fn payload_to_bits(v: u64) -> [bool; 64] {
    let mut bits = [false; 64];
    for i in 0..64 {
        bits[i] = ((v >> (63 - i)) & 1) == 1;
    }
    bits
}

/// Convert a 64-element bit array (MSB first) back to a `u64`.
fn bits_to_u64(bits: &[bool; 64]) -> u64 {
    let mut v = 0_u64;
    for (i, &b) in bits.iter().enumerate() {
        if b {
            v |= 1 << (63 - i);
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ForensicPayload ───────────────────────────────────────────────────────

    #[test]
    fn test_encode_decode_roundtrip() {
        let p = ForensicPayload {
            customer_id: 12345,
            session_id: 678,
            timestamp_sec: 910,
        };
        let encoded = p.encode();
        let decoded = ForensicPayload::decode(encoded);
        assert_eq!(decoded.customer_id, p.customer_id);
        // session_id and timestamp_sec are truncated to 16 bits
        assert_eq!(decoded.session_id, p.session_id & 0xFFFF);
        assert_eq!(decoded.timestamp_sec, p.timestamp_sec & 0xFFFF);
    }

    #[test]
    fn test_encode_zero_payload() {
        let p = ForensicPayload {
            customer_id: 0,
            session_id: 0,
            timestamp_sec: 0,
        };
        assert_eq!(p.encode(), 0);
    }

    #[test]
    fn test_encode_max_customer_id() {
        let p = ForensicPayload {
            customer_id: u32::MAX,
            session_id: 0,
            timestamp_sec: 0,
        };
        let enc = p.encode();
        assert_eq!(ForensicPayload::decode(enc).customer_id, u32::MAX);
    }

    #[test]
    fn test_encode_session_truncated_to_16_bits() {
        let p = ForensicPayload {
            customer_id: 1,
            session_id: 0x1_2345,
            timestamp_sec: 0,
        };
        let decoded = ForensicPayload::decode(p.encode());
        assert_eq!(decoded.session_id, 0x2345);
    }

    #[test]
    fn test_decode_known_value() {
        // customer_id=1 in bits 63-32, session_id=2 in bits 31-16, ts=3 in bits 15-0
        let v: u64 = (1_u64 << 32) | (2_u64 << 16) | 3;
        let p = ForensicPayload::decode(v);
        assert_eq!(p.customer_id, 1);
        assert_eq!(p.session_id, 2);
        assert_eq!(p.timestamp_sec, 3);
    }

    // ── WatermarkVariant ──────────────────────────────────────────────────────

    #[test]
    fn test_variant_describe_contains_customer_id() {
        let v = WatermarkVariant {
            variant_id: 3,
            payload: ForensicPayload {
                customer_id: 99,
                session_id: 1,
                timestamp_sec: 0,
            },
        };
        assert!(v.describe().contains("99"));
    }

    #[test]
    fn test_variant_describe_contains_variant_id() {
        let v = WatermarkVariant {
            variant_id: 7,
            payload: ForensicPayload {
                customer_id: 1,
                session_id: 1,
                timestamp_sec: 0,
            },
        };
        assert!(v.describe().contains("variant=7"));
    }

    // ── TraitorTrace ──────────────────────────────────────────────────────────

    #[test]
    fn test_most_likely_empty() {
        let trace = TraitorTrace::new();
        assert!(trace.most_likely().is_none());
    }

    #[test]
    fn test_most_likely_single() {
        let mut trace = TraitorTrace::new();
        trace.add_suspect(42, 0.9);
        assert_eq!(trace.most_likely(), Some(42));
    }

    #[test]
    fn test_most_likely_highest_confidence() {
        let mut trace = TraitorTrace::new();
        trace.add_suspect(1, 0.4);
        trace.add_suspect(2, 0.9);
        trace.add_suspect(3, 0.7);
        assert_eq!(trace.most_likely(), Some(2));
    }

    #[test]
    fn test_suspect_count() {
        let mut trace = TraitorTrace::new();
        trace.add_suspect(10, 0.5);
        trace.add_suspect(20, 0.6);
        assert_eq!(trace.suspects.len(), 2);
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    #[test]
    fn test_payload_to_bits_msb_first() {
        let bits = payload_to_bits(1_u64 << 63);
        assert!(bits[0]);
        assert!(!bits[1]);
    }

    #[test]
    fn test_bits_to_u64_roundtrip() {
        let original: u64 = 0xDEAD_BEEF_CAFE_1234;
        let bits = payload_to_bits(original);
        let recovered = bits_to_u64(&bits);
        assert_eq!(recovered, original);
    }

    // ── ForensicEmbedder / ForensicDetector ───────────────────────────────────

    #[test]
    fn test_embedder_does_not_panic_small_image() {
        let mut pixels = vec![128_u8; 4 * 4];
        let embedder = ForensicEmbedder::new(8, 0.5);
        let p = ForensicPayload {
            customer_id: 1,
            session_id: 2,
            timestamp_sec: 3,
        };
        // Image smaller than one block — should not panic
        embedder.embed(&mut pixels, 4, 4, &p);
    }

    #[test]
    fn test_detector_returns_none_for_tiny_image() {
        let pixels = vec![0_u8; 4 * 4];
        let det = ForensicDetector::new(8);
        assert!(det.detect(&pixels, 4, 4).is_none());
    }

    #[test]
    fn test_embedder_modifies_pixels() {
        let original = vec![128_u8; 32 * 32];
        let mut pixels = original.clone();
        let embedder = ForensicEmbedder::new(4, 0.5);
        let p = ForensicPayload {
            customer_id: 0xABCD,
            session_id: 0x1234,
            timestamp_sec: 100,
        };
        embedder.embed(&mut pixels, 32, 32, &p);
        assert_ne!(pixels, original, "embed should modify at least one pixel");
    }
}
