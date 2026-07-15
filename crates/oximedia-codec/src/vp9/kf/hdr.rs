//! VP9 compressed-header parsing for intra frames — exact port of
//! `read_compressed_header` (libvpx `vp9/decoder/vp9_decodeframe.c`) and
//! `vp9/decoder/vp9_dsubexp.c`.
//!
//! For keyframes / intra-only frames the compressed header carries only the
//! transform mode (+ tx-size probability updates), coefficient probability
//! updates, and skip probability updates; all inter-related sections are
//! absent (`!frame_is_intra_only`).

use super::booldec::BoolReader;
use super::tables;
use crate::error::{CodecError, CodecResult};

/// VP9 `TX_MODE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxMode {
    /// Only 4x4 transforms.
    Only4x4 = 0,
    /// Up to 8x8.
    Allow8x8 = 1,
    /// Up to 16x16.
    Allow16x16 = 2,
    /// Up to 32x32.
    Allow32x32 = 3,
    /// Per-block selection.
    Select = 4,
}

impl TxMode {
    /// `tx_mode_to_biggest_tx_size` (vp9_common_data.c).
    #[must_use]
    pub fn biggest_tx_size(self) -> usize {
        match self {
            TxMode::Only4x4 => 0,
            TxMode::Allow8x8 => 1,
            TxMode::Allow16x16 => 2,
            TxMode::Allow32x32 | TxMode::Select => 3,
        }
    }
}

/// Coefficient probabilities: `[tx_size][plane_type][ref][band][ctx][node]`.
pub type CoefProbs = [[[[[[u8; 3]; 6]; 6]; 2]; 2]; 4];

/// Entropy state used by the intra-frame decode (defaults + header updates).
#[derive(Clone)]
pub struct FrameProbs {
    /// Coefficient probabilities.
    pub coef: CoefProbs,
    /// Skip flag probabilities per context.
    pub skip: [u8; 3],
    /// tx_probs.p8x8[ctx][node].
    pub tx8: [[u8; 1]; 2],
    /// tx_probs.p16x16[ctx][node].
    pub tx16: [[u8; 2]; 2],
    /// tx_probs.p32x32[ctx][node].
    pub tx32: [[u8; 3]; 2],
}

impl FrameProbs {
    /// Default probabilities (`vp9_default_coef_probs` et al). Keyframes
    /// always reset every frame context to these defaults
    /// (`vp9_setup_past_independence`).
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            coef: [
                tables::DEFAULT_COEF_PROBS_4X4,
                tables::DEFAULT_COEF_PROBS_8X8,
                tables::DEFAULT_COEF_PROBS_16X16,
                tables::DEFAULT_COEF_PROBS_32X32,
            ],
            skip: tables::DEFAULT_SKIP_PROBS,
            tx8: tables::DEFAULT_TX_8X8_PROBS,
            tx16: tables::DEFAULT_TX_16X16_PROBS,
            tx32: tables::DEFAULT_TX_32X32_PROBS,
        }
    }
}

/// libvpx `DIFF_UPDATE_PROB`.
const DIFF_UPDATE_PROB: u8 = 252;

/// libvpx `inv_map_table` (vp9_dsubexp.c) for subexp prob updates.
#[rustfmt::skip]
const INV_MAP_TABLE: [u8; 255] = [7, 20, 33, 46, 59, 72, 85, 98, 111, 124, 137, 150, 163, 176, 189, 202, 215, 228, 241, 254, 1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 253];

/// `inv_recenter_nonneg` (vp9_dsubexp.c).
fn inv_recenter_nonneg(v: i32, m: i32) -> i32 {
    if v > 2 * m {
        return v;
    }
    if v & 1 != 0 {
        m - ((v + 1) >> 1)
    } else {
        m + (v >> 1)
    }
}

/// `decode_uniform` (vp9_dsubexp.c).
fn decode_uniform(r: &mut BoolReader<'_>) -> i32 {
    let m = (1 << 8) - 191; // 65
    let v = r.read_literal(7) as i32;
    if v < m {
        v
    } else {
        (v << 1) - m + i32::from(r.read_bit())
    }
}

/// `inv_remap_prob` (vp9_dsubexp.c). `MAX_PROB` = 255.
fn inv_remap_prob(v: i32, m0: u8) -> u8 {
    let v = i32::from(INV_MAP_TABLE[v as usize]);
    let m = i32::from(m0) - 1;
    if (m << 1) <= 255 {
        (1 + inv_recenter_nonneg(v, m)) as u8
    } else {
        (255 - inv_recenter_nonneg(v, 255 - 1 - m)) as u8
    }
}

/// `decode_term_subexp` (vp9_dsubexp.c).
fn decode_term_subexp(r: &mut BoolReader<'_>) -> i32 {
    if !r.read_bit() {
        return r.read_literal(4) as i32;
    }
    if !r.read_bit() {
        return r.read_literal(4) as i32 + 16;
    }
    if !r.read_bit() {
        return r.read_literal(5) as i32 + 32;
    }
    decode_uniform(r) + 64
}

/// `vp9_diff_update_prob`.
fn diff_update_prob(r: &mut BoolReader<'_>, p: &mut u8) {
    if r.read_bool(DIFF_UPDATE_PROB) {
        let delp = decode_term_subexp(r);
        *p = inv_remap_prob(delp, *p);
    }
}

/// `BAND_COEFF_CONTEXTS(band)`: band 0 has 3 contexts, others 6.
fn band_coeff_contexts(band: usize) -> usize {
    if band == 0 {
        3
    } else {
        6
    }
}

/// Parses the intra-frame compressed header, updating `probs` in place.
/// Returns the parsed `TxMode`.
///
/// # Errors
///
/// Returns [`CodecError::InvalidBitstream`] on a bad marker bit or when the
/// bool decoder consumed more data than the partition holds.
pub fn parse_compressed_header_intra(
    data: &[u8],
    lossless: bool,
    probs: &mut FrameProbs,
) -> CodecResult<TxMode> {
    let mut r = BoolReader::new(data).ok_or_else(|| {
        CodecError::InvalidBitstream("VP9 compressed header: invalid marker bit".into())
    })?;

    // read_tx_mode()
    let tx_mode = if lossless {
        TxMode::Only4x4
    } else {
        let mut v = r.read_literal(2);
        if v == 3 {
            v += u32::from(r.read_bit());
        }
        match v {
            0 => TxMode::Only4x4,
            1 => TxMode::Allow8x8,
            2 => TxMode::Allow16x16,
            3 => TxMode::Allow32x32,
            _ => TxMode::Select,
        }
    };

    // read_tx_mode_probs()
    if tx_mode == TxMode::Select {
        for i in 0..2 {
            for j in 0..1 {
                diff_update_prob(&mut r, &mut probs.tx8[i][j]);
            }
        }
        for i in 0..2 {
            for j in 0..2 {
                diff_update_prob(&mut r, &mut probs.tx16[i][j]);
            }
        }
        for i in 0..2 {
            for j in 0..3 {
                diff_update_prob(&mut r, &mut probs.tx32[i][j]);
            }
        }
    }

    // read_coef_probs(): one update pass per coded tx size.
    let max_tx_size = tx_mode.biggest_tx_size();
    for tx_size in 0..=max_tx_size {
        if r.read_bit() {
            for plane in 0..2 {
                for is_inter in 0..2 {
                    for band in 0..6 {
                        for ctx in 0..band_coeff_contexts(band) {
                            for node in 0..3 {
                                diff_update_prob(
                                    &mut r,
                                    &mut probs.coef[tx_size][plane][is_inter][band][ctx][node],
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // skip probs
    for k in 0..3 {
        diff_update_prob(&mut r, &mut probs.skip[k]);
    }

    // Inter sections are absent for intra-only frames.

    if r.has_error() {
        return Err(CodecError::InvalidBitstream(
            "VP9 compressed header overran its partition".into(),
        ));
    }
    Ok(tx_mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_real_coef_probs_not_uniform() {
        let p = FrameProbs::defaults();
        // libvpx default_coef_probs_4x4[0][0][0][0] = { 195, 29, 183 }
        assert_eq!(p.coef[0][0][0][0][0], [195, 29, 183]);
        assert_eq!(p.skip, [192, 128, 64]);
    }

    #[test]
    fn inv_remap_matches_reference_examples() {
        // v=0 -> inv_map_table[0] = 7; m = 128-1 = 127; (127<<1) <= 255
        // -> 1 + inv_recenter_nonneg(7, 127); 7 is odd -> 127 - 4 = 123
        // -> 124 (hand-traced against vp9_dsubexp.c).
        assert_eq!(inv_remap_prob(0, 128), 124);
        // v=20 -> inv_map_table[20] = 1; 1 is odd -> 127 - 1 = 126 -> 127.
        assert_eq!(inv_remap_prob(20, 128), 127);
    }

    #[test]
    fn bad_marker_bit_is_error() {
        let mut probs = FrameProbs::defaults();
        let e = parse_compressed_header_intra(&[0xFF, 0, 0, 0], false, &mut probs);
        assert!(e.is_err());
    }
}
