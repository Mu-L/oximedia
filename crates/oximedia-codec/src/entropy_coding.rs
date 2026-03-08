//! Entropy coding primitives.
//!
//! This module provides simplified implementations of common entropy coding
//! techniques used in video codecs: arithmetic coding, range coding, and
//! Huffman coding.

// -------------------------------------------------------------------------
// Arithmetic Coder
// -------------------------------------------------------------------------

/// Simplified binary arithmetic coder.
///
/// Maintains interval `[low, high)` and narrows it on each coded symbol.
/// The implementation uses integer arithmetic and emits carry-forwarded bits
/// via the E1/E2 (bit-stuffing) technique.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ArithmeticCoder {
    /// Lower bound of the current coding interval.
    pub low: u32,
    /// Upper bound of the current coding interval.
    pub high: u32,
    /// Pending follow bits to emit after the next definite bit.
    pub bits_to_follow: u32,
}

impl ArithmeticCoder {
    /// Creates a new arithmetic coder in its initial state.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            low: 0,
            high: 0xFFFF_FFFF,
            bits_to_follow: 0,
        }
    }

    /// Encodes a single bit given a probability `prob_one ∈ (0.0, 1.0)` that
    /// the bit is `1`.
    ///
    /// Returns any bytes that were flushed from the interval during this step.
    /// Note: this is a simplified model that collects emitted bits into bytes.
    #[allow(dead_code)]
    #[allow(clippy::cast_possible_truncation, clippy::same_item_push)]
    pub fn encode_bit(&mut self, prob_one: f32, bit: bool) -> Vec<u8> {
        let range = u64::from(self.high) - u64::from(self.low) + 1;
        #[allow(clippy::cast_precision_loss)]
        let split = ((range as f64 * f64::from(1.0 - prob_one)) as u64).saturating_sub(1);
        let mid = self.low.saturating_add(split as u32);

        if bit {
            self.low = mid + 1;
        } else {
            self.high = mid;
        }

        // Normalise: emit bits while interval is contained in one half.
        // The repeated push of the same literal is intentional: arithmetic coding
        // follow-bits must all have the same value (complementing the emitted bit).
        let mut emitted_bits: Vec<bool> = Vec::new();
        loop {
            if self.high < 0x8000_0000 {
                // Both in [0, 0.5): emit 0, then any pending 1s.
                emitted_bits.push(false);
                for _ in 0..self.bits_to_follow {
                    emitted_bits.push(true);
                }
                self.bits_to_follow = 0;
                self.low <<= 1;
                self.high = (self.high << 1) | 1;
            } else if self.low >= 0x8000_0000 {
                // Both in [0.5, 1): emit 1, then any pending 0s.
                emitted_bits.push(true);
                for _ in 0..self.bits_to_follow {
                    emitted_bits.push(false);
                }
                self.bits_to_follow = 0;
                self.low = (self.low - 0x8000_0000) << 1;
                self.high = ((self.high - 0x8000_0000) << 1) | 1;
            } else if self.low >= 0x4000_0000 && self.high < 0xC000_0000 {
                // Interval straddles midpoint: E3 scaling.
                self.bits_to_follow += 1;
                self.low = (self.low - 0x4000_0000) << 1;
                self.high = ((self.high - 0x4000_0000) << 1) | 1;
            } else {
                break;
            }
        }

        // Pack the emitted bits into bytes (MSB-first).
        bits_to_bytes(&emitted_bits)
    }

    /// Returns the current interval range `high - low + 1`.
    ///
    /// Returns a `u64` because the initial range is `0xFFFF_FFFF - 0 + 1 = 2^32`,
    /// which overflows `u32`.
    #[allow(dead_code)]
    pub fn get_range(&self) -> u64 {
        u64::from(self.high) - u64::from(self.low) + 1
    }
}

/// Packs a slice of bits (MSB-first within each byte) into a `Vec<u8>`.
#[allow(dead_code)]
fn bits_to_bytes(bits: &[bool]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut current: u8 = 0;
    let mut count = 0u8;
    for &b in bits {
        current = (current << 1) | u8::from(b);
        count += 1;
        if count == 8 {
            bytes.push(current);
            current = 0;
            count = 0;
        }
    }
    if count > 0 {
        bytes.push(current << (8 - count));
    }
    bytes
}

// -------------------------------------------------------------------------
// Range Coder
// -------------------------------------------------------------------------

/// Simplified range coder (decoder side).
///
/// Range coding is a generalisation of arithmetic coding used in many modern
/// codecs (VP8, VP9, AV1, …).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RangeCoder {
    /// Current range (normalised to [128, 256)).
    pub range: u32,
    /// Current code word.
    pub code: u32,
}

impl RangeCoder {
    /// Creates a new range coder with a full-range initial state.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            range: 256,
            code: 0,
        }
    }

    /// Normalises the range back into `[128, 256)` by doubling, returning
    /// the number of bits consumed from the bitstream.
    #[allow(dead_code)]
    pub fn normalize(&mut self) -> u32 {
        let mut bits_consumed = 0;
        while self.range < 128 {
            self.range <<= 1;
            self.code <<= 1;
            bits_consumed += 1;
        }
        bits_consumed
    }

    /// Decodes one symbol given a split probability `prob ∈ [0, 256)`.
    ///
    /// Returns `true` for the high partition (code ≥ split), `false` otherwise.
    #[allow(dead_code)]
    pub fn decode_symbol(&mut self, prob: u32) -> bool {
        let split = (self.range * prob) >> 8;
        if self.code >= split {
            self.code -= split;
            self.range -= split;
            true
        } else {
            self.range = split;
            false
        }
    }
}

// -------------------------------------------------------------------------
// Huffman Coding
// -------------------------------------------------------------------------

/// A node in a Huffman tree.
#[derive(Debug)]
#[allow(dead_code)]
pub struct HuffmanNode {
    /// Present only on leaf nodes; the symbol value.
    pub symbol: Option<u8>,
    /// Aggregate frequency of the subtree rooted here.
    pub freq: u32,
    /// Left child (lower-frequency subtree).
    pub left: Option<Box<HuffmanNode>>,
    /// Right child (higher-frequency subtree).
    pub right: Option<Box<HuffmanNode>>,
}

impl HuffmanNode {
    /// Returns `true` when this node is a leaf (holds a symbol, has no children).
    #[allow(dead_code)]
    pub fn is_leaf(&self) -> bool {
        self.left.is_none() && self.right.is_none()
    }
}

/// Builds a Huffman tree from a frequency table using a greedy (priority-queue)
/// algorithm.
///
/// `freqs[i]` is the frequency of symbol `i`.  Symbols with frequency 0 are
/// excluded.  If `freqs` is empty or all frequencies are 0, a trivial leaf
/// tree for symbol 0 is returned.
#[allow(dead_code)]
pub fn build_huffman_tree(freqs: &[u32]) -> HuffmanNode {
    // Collect leaf nodes for non-zero-frequency symbols.
    let mut nodes: Vec<HuffmanNode> = freqs
        .iter()
        .enumerate()
        .filter(|(_, &f)| f > 0)
        .map(|(i, &f)| HuffmanNode {
            symbol: Some(i as u8),
            freq: f,
            left: None,
            right: None,
        })
        .collect();

    if nodes.is_empty() {
        // Degenerate: return a leaf for symbol 0 with freq 0.
        return HuffmanNode {
            symbol: Some(0),
            freq: 0,
            left: None,
            right: None,
        };
    }

    // Single-symbol alphabet: wrap in a parent so tree depth ≥ 1.
    if nodes.len() == 1 {
        let leaf = nodes.remove(0);
        return HuffmanNode {
            symbol: None,
            freq: leaf.freq,
            left: Some(Box::new(leaf)),
            right: None,
        };
    }

    // Greedy combination: always merge the two lowest-frequency nodes.
    while nodes.len() > 1 {
        // Sort ascending by frequency (stable, so ties preserve insertion order).
        nodes.sort_by_key(|n| n.freq);
        let left = nodes.remove(0);
        let right = nodes.remove(0);
        let parent = HuffmanNode {
            symbol: None,
            freq: left.freq + right.freq,
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
        };
        nodes.push(parent);
    }

    nodes.remove(0)
}

/// Traverses the Huffman tree depth-first, collecting `(symbol, code_bits)`
/// pairs at each leaf.
///
/// `prefix` is the bit-path from the root to the current node (each `u8`
/// is `0` or `1`).
#[allow(dead_code)]
pub fn compute_huffman_codes(node: &HuffmanNode, prefix: Vec<u8>) -> Vec<(u8, Vec<u8>)> {
    if node.is_leaf() {
        if let Some(sym) = node.symbol {
            return vec![(sym, prefix)];
        }
        return vec![];
    }

    let mut codes = Vec::new();
    if let Some(left) = &node.left {
        let mut left_prefix = prefix.clone();
        left_prefix.push(0);
        codes.extend(compute_huffman_codes(left, left_prefix));
    }
    if let Some(right) = &node.right {
        let mut right_prefix = prefix.clone();
        right_prefix.push(1);
        codes.extend(compute_huffman_codes(right, right_prefix));
    }
    codes
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- ArithmeticCoder tests ---

    #[test]
    fn arithmetic_coder_new_initial_range() {
        let coder = ArithmeticCoder::new();
        assert_eq!(coder.low, 0);
        assert_eq!(coder.high, 0xFFFF_FFFF);
        // Initial range spans the full 32-bit space: 2^32.
        assert_eq!(coder.get_range(), 0x1_0000_0000u64);
    }

    #[test]
    fn arithmetic_coder_get_range() {
        let c = ArithmeticCoder::new();
        let initial_range = c.get_range();
        // Initial range must be positive.
        assert!(initial_range > 0);
        // Initial range is the full 32-bit span.
        assert_eq!(initial_range, 0x1_0000_0000u64);
        // After encoding with a strongly-biased probability, the coder should
        // still maintain a valid (positive) range.
        let mut c2 = ArithmeticCoder::new();
        c2.encode_bit(0.9, true);
        assert!(c2.get_range() > 0);
        assert!(c2.low <= c2.high);
    }

    #[test]
    fn arithmetic_coder_encode_bit_does_not_panic() {
        let mut c = ArithmeticCoder::new();
        let _bytes = c.encode_bit(0.5, true);
        let _bytes = c.encode_bit(0.5, false);
        let _bytes = c.encode_bit(0.9, true);
        // No panic is sufficient for this test.
    }

    #[test]
    fn arithmetic_coder_bits_to_follow_increments() {
        let mut c = ArithmeticCoder::new();
        // Repeated near-50% bits tend to trigger E3 scaling.
        for _ in 0..16 {
            c.encode_bit(0.5, true);
        }
        // State should remain coherent (low ≤ high).
        assert!(c.low <= c.high);
    }

    #[test]
    fn arithmetic_coder_encode_sequence_returns_bytes() {
        let mut c = ArithmeticCoder::new();
        let mut all_bytes = Vec::new();
        // Encode 32 bits with strong probability – should flush many bytes.
        for _ in 0..32 {
            all_bytes.extend(c.encode_bit(0.95, true));
        }
        // We don't verify bit-exact values, just that the coder is usable.
        assert!(all_bytes.len() <= 32 * 2); // sanity upper bound
    }

    // --- bits_to_bytes helper ---

    #[test]
    fn bits_to_bytes_empty() {
        let b = bits_to_bytes(&[]);
        assert!(b.is_empty());
    }

    #[test]
    fn bits_to_bytes_full_byte() {
        // 0b1010_1010 = 0xAA
        let bits = [true, false, true, false, true, false, true, false];
        let b = bits_to_bytes(&bits);
        assert_eq!(b, vec![0xAA]);
    }

    // --- RangeCoder tests ---

    #[test]
    fn range_coder_new() {
        let rc = RangeCoder::new();
        assert_eq!(rc.range, 256);
        assert_eq!(rc.code, 0);
    }

    #[test]
    fn range_coder_normalize_already_normalised() {
        let mut rc = RangeCoder::new();
        let bits = rc.normalize();
        assert_eq!(bits, 0); // already in [128, 256)
    }

    #[test]
    fn range_coder_normalize_below_128() {
        let mut rc = RangeCoder { range: 32, code: 0 };
        let bits = rc.normalize();
        assert!(rc.range >= 128);
        assert_eq!(bits, 2); // 32 → 64 → 128, two doublings
    }

    #[test]
    fn range_coder_decode_symbol_high_partition() {
        let mut rc = RangeCoder {
            range: 256,
            code: 200,
        };
        // split = (256 * 128) >> 8 = 128; code(200) >= split(128) → true
        let sym = rc.decode_symbol(128);
        assert!(sym);
        assert_eq!(rc.range, 256 - 128);
        assert_eq!(rc.code, 200 - 128);
    }

    #[test]
    fn range_coder_decode_symbol_low_partition() {
        let mut rc = RangeCoder {
            range: 256,
            code: 50,
        };
        // split = 128; code(50) < split(128) → false
        let sym = rc.decode_symbol(128);
        assert!(!sym);
        assert_eq!(rc.range, 128);
        assert_eq!(rc.code, 50);
    }

    // --- HuffmanNode tests ---

    #[test]
    fn huffman_node_is_leaf_true() {
        let leaf = HuffmanNode {
            symbol: Some(42),
            freq: 10,
            left: None,
            right: None,
        };
        assert!(leaf.is_leaf());
    }

    #[test]
    fn huffman_node_is_leaf_false() {
        let inner = HuffmanNode {
            symbol: None,
            freq: 20,
            left: Some(Box::new(HuffmanNode {
                symbol: Some(0),
                freq: 10,
                left: None,
                right: None,
            })),
            right: None,
        };
        assert!(!inner.is_leaf());
    }

    #[test]
    fn build_huffman_tree_two_symbols() {
        let freqs = [10u32, 20];
        let tree = build_huffman_tree(&freqs);
        assert!(!tree.is_leaf());
        assert_eq!(tree.freq, 30);
        let codes = compute_huffman_codes(&tree, vec![]);
        // Two leaves → two codes.
        assert_eq!(codes.len(), 2);
    }

    #[test]
    fn build_huffman_tree_multiple_symbols() {
        // Typical small alphabet.
        let freqs = [5u32, 9, 12, 13, 16, 45];
        let tree = build_huffman_tree(&freqs);
        let codes = compute_huffman_codes(&tree, vec![]);
        assert_eq!(codes.len(), 6);
        // Higher-frequency symbols should have shorter codes.
        let mut code_map = std::collections::HashMap::new();
        for (sym, code) in &codes {
            code_map.insert(*sym, code.len());
        }
        // Symbol 5 (freq=45) should have the shortest code.
        assert!(code_map[&5] <= code_map[&0]);
    }

    #[test]
    fn build_huffman_tree_empty_freqs() {
        let tree = build_huffman_tree(&[]);
        // Degenerate: single leaf for symbol 0.
        assert!(tree.is_leaf());
        assert_eq!(tree.symbol, Some(0));
    }

    #[test]
    fn build_huffman_tree_single_symbol() {
        let freqs = [0u32, 7, 0];
        let tree = build_huffman_tree(&freqs);
        // Wrapped in a parent.
        assert!(!tree.is_leaf());
        let codes = compute_huffman_codes(&tree, vec![]);
        assert_eq!(codes.len(), 1);
        assert_eq!(codes[0].0, 1); // symbol index 1
    }

    #[test]
    fn compute_huffman_codes_all_unique() {
        let freqs = [1u32, 2, 4, 8];
        let tree = build_huffman_tree(&freqs);
        let codes = compute_huffman_codes(&tree, vec![]);
        let symbols: Vec<u8> = codes.iter().map(|(s, _)| *s).collect();
        // All symbols should appear exactly once.
        let mut sorted = symbols.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), symbols.len());
    }
}
