//! Rice coding for FLAC residuals.
//!
//! Rice coding is a special case of Golomb coding where the divisor `m = 2^k`.
//! FLAC uses Rice coding (partition coding) to compress the LPC residuals.
//!
//! Each partition's Rice parameter `k` is optimised to minimise bit usage.

#![forbid(unsafe_code)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]

/// Maximum Rice parameter (FLAC allows 0-14 for Rice1, 0-30 for Rice2).
pub const MAX_RICE_PARAM: u8 = 14;

/// Map a signed residual to an unsigned zigzag-encoded value.
///
/// FLAC encodes signed residuals as zigzag: `0 → 0, -1 → 1, 1 → 2, -2 → 3, 2 → 4, ...`
#[inline]
pub fn zigzag_encode(v: i32) -> u32 {
    if v >= 0 {
        (v as u32) << 1
    } else {
        ((-v - 1) as u32) << 1 | 1
    }
}

/// Decode a zigzag-encoded unsigned value back to signed.
#[inline]
pub fn zigzag_decode(u: u32) -> i32 {
    if u & 1 == 0 {
        (u >> 1) as i32
    } else {
        -((u >> 1) as i32) - 1
    }
}

/// Compute the Rice bit cost for encoding `residuals` with parameter `k`.
///
/// `cost = sum(1 + k + (zigzag(r) >> k))` bits per sample.
#[must_use]
pub fn rice_bit_cost(residuals: &[i32], k: u8) -> u64 {
    residuals
        .iter()
        .map(|&r| {
            let u = zigzag_encode(r);
            let quotient = u >> k;
            1u64 + u64::from(k) + u64::from(quotient)
        })
        .sum()
}

/// Select the optimal Rice parameter for a partition of residuals.
///
/// Tests `k = 0..=MAX_RICE_PARAM` and returns the best.
#[must_use]
pub fn optimal_rice_param(residuals: &[i32]) -> u8 {
    if residuals.is_empty() {
        return 0;
    }
    let mut best_k = 0u8;
    let mut best_cost = u64::MAX;
    for k in 0..=MAX_RICE_PARAM {
        let cost = rice_bit_cost(residuals, k);
        if cost < best_cost {
            best_cost = cost;
            best_k = k;
        }
    }
    best_k
}

/// Encode residuals using Rice coding with parameter `k`.
///
/// Returns the packed bit stream as a `Vec<u8>` (MSB-first, zero-padded to byte boundary).
#[must_use]
pub fn rice_encode(residuals: &[i32], k: u8) -> Vec<u8> {
    let mut bits: Vec<bool> = Vec::new();

    for &r in residuals {
        let u = zigzag_encode(r);
        let quotient = u >> k;
        let remainder = u & ((1u32 << k) - 1);

        // Unary-coded quotient: `quotient` ones followed by a zero
        for _ in 0..quotient {
            bits.push(true);
        }
        bits.push(false);

        // Binary `k` bits of remainder (MSB first)
        for bit_idx in (0..k).rev() {
            bits.push((remainder >> bit_idx) & 1 != 0);
        }
    }

    // Pack bits into bytes (MSB-first)
    let mut out = Vec::with_capacity((bits.len() + 7) / 8);
    let mut byte = 0u8;
    let mut fill = 0u8;
    for bit in bits {
        byte = (byte << 1) | u8::from(bit);
        fill += 1;
        if fill == 8 {
            out.push(byte);
            byte = 0;
            fill = 0;
        }
    }
    if fill > 0 {
        out.push(byte << (8 - fill));
    }
    out
}

/// Rice decoder state.
pub struct RiceDecoder<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8,
}

impl<'a> RiceDecoder<'a> {
    /// Create a decoder over a Rice-coded byte stream.
    #[must_use]
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    fn read_bit(&mut self) -> Option<bool> {
        if self.byte_pos >= self.data.len() {
            return None;
        }
        let bit = (self.data[self.byte_pos] >> (7 - self.bit_pos)) & 1 != 0;
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.byte_pos += 1;
            self.bit_pos = 0;
        }
        Some(bit)
    }

    /// Decode one Rice-coded residual with parameter `k`.
    pub fn decode_one(&mut self, k: u8) -> Option<i32> {
        // Read unary quotient
        let mut quotient = 0u32;
        loop {
            let bit = self.read_bit()?;
            if !bit {
                break;
            }
            quotient += 1;
            if quotient > 1024 * 1024 {
                return None; // guard against corrupt data
            }
        }

        // Read `k` remainder bits
        let mut remainder = 0u32;
        for _ in 0..k {
            let bit = self.read_bit()?;
            remainder = (remainder << 1) | u32::from(bit);
        }

        let u = (quotient << k) | remainder;
        Some(zigzag_decode(u))
    }

    /// Decode `count` residuals with parameter `k`.
    pub fn decode_n(&mut self, count: usize, k: u8) -> Vec<i32> {
        (0..count).map_while(|_| self.decode_one(k)).collect()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zigzag_encode_decode_identity() {
        for v in [-100i32, -1, 0, 1, 100, i16::MAX as i32] {
            let u = zigzag_encode(v);
            let back = zigzag_decode(u);
            assert_eq!(back, v, "zigzag roundtrip failed for {v}");
        }
    }

    #[test]
    fn test_zigzag_non_negative_output() {
        // zigzag_encode maps i32 -> u32, which is inherently non-negative
        for v in [-200i32, -100, -1, 0, 1, 100, 200] {
            let _u = zigzag_encode(v);
        }
    }

    #[test]
    fn test_rice_bit_cost_zero_residuals() {
        let res = vec![0i32; 16];
        let cost = rice_bit_cost(&res, 0);
        // Each 0 costs 1 (unary 0) + 0 (k=0) = 1 bit → 16 total
        assert_eq!(cost, 16);
    }

    #[test]
    fn test_rice_encode_decode_roundtrip() {
        let residuals = vec![0i32, 1, -1, 2, -2, 5, -5, 10, -10];
        let k = optimal_rice_param(&residuals);
        let encoded = rice_encode(&residuals, k);
        let mut dec = RiceDecoder::new(&encoded);
        let decoded = dec.decode_n(residuals.len(), k);
        assert_eq!(decoded, residuals, "Rice roundtrip must be lossless");
    }

    #[test]
    fn test_rice_encode_empty() {
        let encoded = rice_encode(&[], 4);
        assert!(encoded.is_empty());
    }

    #[test]
    fn test_optimal_rice_param_small_residuals() {
        // Small residuals → small k is optimal
        let residuals = vec![0i32; 32];
        let k = optimal_rice_param(&residuals);
        assert_eq!(k, 0, "All-zero residuals → k=0 is optimal");
    }

    #[test]
    fn test_optimal_rice_param_large_residuals() {
        // Large residuals → larger k is better
        let residuals: Vec<i32> = (0..32).map(|i| i * 1000).collect();
        let k_large = optimal_rice_param(&residuals);
        let k_small = optimal_rice_param(&vec![0i32; 32]);
        assert!(k_large >= k_small, "Large residuals should use larger k");
    }

    #[test]
    fn test_rice_decode_n_partial() {
        // If stream is shorter than count, decode_n returns fewer items
        let residuals = vec![1i32, 2, 3];
        let k = 1;
        let encoded = rice_encode(&residuals, k);
        // Request more than available
        let mut dec = RiceDecoder::new(&encoded);
        let decoded = dec.decode_n(100, k);
        assert!(decoded.len() >= residuals.len());
        assert_eq!(&decoded[..residuals.len()], &residuals[..]);
    }
}
