//! VP8 key-frame macroblock decoding and reconstruction (RFC 6386 §11-§15).
//!
//! This module orchestrates the per-macroblock decode loop for a VP8 key frame:
//! 1. parse the macroblock prediction modes (16x16/B_PRED luma + 8x8 chroma),
//! 2. decode the DCT coefficient tokens for the 25 (Y2 + 16 Y + 4 U + 4 V)
//!    sub-blocks,
//! 3. dequantise, inverse-transform, intra-predict and reconstruct each plane,
//! 4. run the in-loop deblocking filter,
//! 5. upsample chroma 4:2:0 and convert YCbCr -> RGBA.

use super::bool_decoder::BoolDecoder;
use super::frame_header::FrameHeader;
use super::loopfilter::{
    compute_filter_params, normal_mbedge_filter_edge, normal_subblock_filter_edge,
    simple_filter_edge, FilterParams,
};
use super::predict::{predict_block, predict_subblock, SubBlockEdge};
use super::tables::{
    clamp_qindex, AC_QUANT, BMODE_TREE, B_DC_PRED, B_HE_PRED, B_PRED, B_TM_PRED, B_VE_PRED,
    CAT_BASE, COEFF_BANDS, COEFF_TREE, DCT_0, DCT_1, DCT_2, DCT_3, DCT_4, DCT_CAT1, DCT_CAT2,
    DCT_CAT3, DCT_CAT4, DCT_CAT5, DCT_CAT6, DCT_EOB, DC_PRED, DC_QUANT, H_PRED, KF_BMODE_PROB,
    KF_UV_MODE_PROB, KF_YMODE_PROB, KF_YMODE_TREE, PCAT1, PCAT2, PCAT3, PCAT4, PCAT5, PCAT6,
    TM_PRED, UV_MODE_TREE, V_PRED, ZIGZAG,
};
use super::transform::{add_residual, idct4x4, iwht4x4};
use crate::error::{ImageError, ImageResult};

/// Number of pixels of border padding kept around each plane.
///
/// VP8 prediction reads up to one pixel left / above and (for sub-block modes)
/// up to four pixels above-right; a one-macroblock guard keeps all reads in
/// bounds without per-pixel branching beyond the explicit availability flags.
const BORDER: usize = 32;

/// Per-macroblock decoded state retained for the loop filter.
#[derive(Clone, Copy)]
struct MbInfo {
    /// Whole-block luma mode (`DC/V/H/TM/B_PRED`).
    y_mode: usize,
    /// `true` if the macroblock carried no non-zero coefficients.
    skip_coeff: bool,
    /// Effective loop-filter level for this macroblock.
    filter_level: i32,
}

/// Dequantisation factors for one segment (RFC 6386 §14.1).
#[derive(Clone, Copy, Default)]
struct DequantFactors {
    /// Luma DC factor.
    y_dc: i32,
    /// Luma AC factor.
    y_ac: i32,
    /// Y2 (WHT) DC factor.
    y2_dc: i32,
    /// Y2 (WHT) AC factor.
    y2_ac: i32,
    /// Chroma DC factor.
    uv_dc: i32,
    /// Chroma AC factor.
    uv_ac: i32,
}

/// Plane reconstruction buffers with padding borders.
struct Planes {
    /// Luma plane.
    y: Vec<u8>,
    /// Chroma-blue plane (subsampled 2x2).
    u: Vec<u8>,
    /// Chroma-red plane (subsampled 2x2).
    v: Vec<u8>,
    /// Stride (row length, including border) of the luma plane.
    y_stride: usize,
    /// Stride of each chroma plane.
    uv_stride: usize,
    /// Offset of luma pixel (0,0).
    y_origin: usize,
    /// Offset of chroma pixel (0,0).
    uv_origin: usize,
}

/// The fully-decoded VP8 image (RGBA8888).
pub struct Vp8Image {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Tightly-packed RGBA pixels, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// Decodes a complete VP8 key-frame `VP8 ` chunk payload into RGBA.
///
/// # Errors
/// Fails on malformed headers, truncated partitions, or unsupported parameters.
pub fn decode_vp8_keyframe(data: &[u8]) -> ImageResult<Vp8Image> {
    let (header, header_bd) = FrameHeader::parse(data)?;
    let mut decoder = Decoder::new(&header, header_bd, data)?;
    decoder.decode_all();
    Ok(decoder.finish())
}

/// Internal decoder holding all per-frame mutable state.
struct Decoder<'a> {
    header: &'a FrameHeader,
    /// Boolean decoder over the first (header) partition — mode/segment data.
    bd: BoolDecoder<'a>,
    /// Boolean decoders, one per DCT-token partition.
    token_bd: Vec<BoolDecoder<'a>>,
    /// Macroblock columns.
    mb_cols: usize,
    /// Macroblock rows.
    mb_rows: usize,
    /// Reconstruction planes.
    planes: Planes,
    /// Per-MB info kept for the loop filter (row-major).
    mb_info: Vec<MbInfo>,
    /// Per-segment dequantisation factors.
    dequant: [DequantFactors; 4],
    /// 4x4 sub-block modes for the current macroblock row, used as the "above"
    /// context for the next row: `above_bmode[mb_col * 4 + col]`.
    above_bmode: Vec<u8>,
    /// "Above" non-zero-coefficient context for the entropy decoder.
    /// 9 entries per MB column: 4 Y + 2 U + 2 V + 1 Y2.
    above_nz: Vec<bool>,
    #[cfg(test)]
    debug_first_mb: String,
    #[cfg(test)]
    debug_tok_state: String,
    #[cfg(test)]
    debug_mb_modes: String,
}

impl<'a> Decoder<'a> {
    /// Builds a decoder, allocating planes and setting up token partitions.
    fn new(header: &'a FrameHeader, bd: BoolDecoder<'a>, data: &'a [u8]) -> ImageResult<Self> {
        let mb_cols = (header.width as usize + 15) / 16;
        let mb_rows = (header.height as usize + 15) / 16;

        // --- token partition setup (RFC 6386 §9.5) ---
        let num_parts = header.num_token_partitions;
        let mut token_bd = Vec::with_capacity(num_parts);
        let part_table_start = header.partitions_start;
        // For N partitions there are (N-1) 3-byte size entries.
        let size_table_len = 3 * (num_parts.saturating_sub(1));
        let mut part_data_start = part_table_start
            .checked_add(size_table_len)
            .ok_or_else(|| ImageError::invalid_format("VP8: partition table overflow"))?;
        if part_data_start > data.len() {
            return Err(ImageError::invalid_format(
                "VP8: partition size table truncated",
            ));
        }
        for i in 0..num_parts {
            let size = if i + 1 < num_parts {
                let off = part_table_start + 3 * i;
                usize::from(data[off])
                    | (usize::from(data[off + 1]) << 8)
                    | (usize::from(data[off + 2]) << 16)
            } else {
                // Last partition runs to the end of the payload.
                data.len().saturating_sub(part_data_start)
            };
            let end = part_data_start
                .checked_add(size)
                .ok_or_else(|| ImageError::invalid_format("VP8: partition overflow"))?;
            if end > data.len() {
                return Err(ImageError::invalid_format(
                    "VP8: token partition exceeds payload",
                ));
            }
            token_bd.push(BoolDecoder::new(&data[part_data_start..end]));
            part_data_start = end;
        }
        if token_bd.is_empty() {
            return Err(ImageError::invalid_format("VP8: no token partitions"));
        }

        // --- plane allocation with borders ---
        let y_w = mb_cols * 16;
        let y_h = mb_rows * 16;
        let uv_w = mb_cols * 8;
        let uv_h = mb_rows * 8;
        let y_stride = y_w + 2 * BORDER;
        let uv_stride = uv_w + 2 * BORDER;
        let y_origin = BORDER * y_stride + BORDER;
        let uv_origin = BORDER * uv_stride + BORDER;
        let planes = Planes {
            y: vec![129u8; y_stride * (y_h + 2 * BORDER)],
            u: vec![129u8; uv_stride * (uv_h + 2 * BORDER)],
            v: vec![129u8; uv_stride * (uv_h + 2 * BORDER)],
            y_stride,
            uv_stride,
            y_origin,
            uv_origin,
        };

        // --- per-segment dequantisation factors (RFC 6386 §14.1) ---
        let dequant = build_dequant(header);

        Ok(Self {
            header,
            bd,
            token_bd,
            mb_cols,
            mb_rows,
            planes,
            mb_info: vec![
                MbInfo {
                    y_mode: DC_PRED,
                    skip_coeff: false,
                    filter_level: 0,
                };
                mb_cols * mb_rows
            ],
            dequant,
            above_bmode: vec![B_DC_PRED as u8; mb_cols * 4],
            above_nz: vec![false; mb_cols * 9],
            #[cfg(test)]
            debug_first_mb: String::new(),
            #[cfg(test)]
            debug_tok_state: String::new(),
            #[cfg(test)]
            debug_mb_modes: String::new(),
        })
    }

    /// Decodes every macroblock row, then runs the loop filter.
    fn decode_all(&mut self) {
        for mb_y in 0..self.mb_rows {
            // "Left" sub-block mode context resets at the start of each MB row.
            let mut left_bmode = [B_DC_PRED as u8; 4];
            // "Left" non-zero context resets each row: 4 Y + 2 U + 2 V + 1 Y2.
            let mut left_nz = [false; 9];
            for mb_x in 0..self.mb_cols {
                self.decode_macroblock(mb_x, mb_y, &mut left_bmode, &mut left_nz);
            }
        }
        self.apply_loop_filter();
    }

    /// Decodes and reconstructs one macroblock.
    fn decode_macroblock(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        left_bmode: &mut [u8; 4],
        left_nz: &mut [bool; 9],
    ) {
        let mb_idx = mb_y * self.mb_cols + mb_x;

        // --- segment id (RFC 6386 §10) ---
        let segment_id = self.read_segment_id();

        // --- mb_skip_coeff (RFC 6386 §11.1) ---
        let skip_coeff = if self.header.mb_no_skip_coeff {
            self.bd.get_bool(self.header.prob_skip_false)
        } else {
            false
        };

        // --- prediction modes (RFC 6386 §11.2-§11.4) ---
        // 4x4 sub-block modes for this MB, raster order (16 entries).
        let mut bmodes = [DC_PRED as u8; 16];
        let y_mode = self.read_kf_ymode();
        if y_mode == B_PRED {
            // Per-4x4 submodes with above/left context.
            for r in 0..4 {
                for c in 0..4 {
                    let above = if r == 0 {
                        usize::from(self.above_bmode[mb_x * 4 + c])
                    } else {
                        usize::from(bmodes[(r - 1) * 4 + c])
                    };
                    let left = if c == 0 {
                        usize::from(left_bmode[r])
                    } else {
                        usize::from(bmodes[r * 4 + c - 1])
                    };
                    let probs = &KF_BMODE_PROB[above][left];
                    let m = self.bd.read_tree(&BMODE_TREE, probs) as u8;
                    bmodes[r * 4 + c] = m;
                }
            }
        } else {
            // A whole-block luma mode implies a fixed equivalent submode for
            // the purposes of neighbouring B_PRED context (RFC 6386 §11.3).
            let implied = match y_mode {
                V_PRED => B_VE_PRED,
                H_PRED => B_HE_PRED,
                TM_PRED => B_TM_PRED,
                _ => B_DC_PRED,
            } as u8;
            bmodes = [implied; 16];
        }
        // Update above/left submode context for the next MB / row.
        for c in 0..4 {
            self.above_bmode[mb_x * 4 + c] = bmodes[12 + c];
        }
        for r in 0..4 {
            left_bmode[r] = bmodes[r * 4 + 3];
        }

        let uv_mode = self.read_kf_uvmode();

        // --- coefficient decode (RFC 6386 §13) ---
        // 25 sub-blocks: index 24 = Y2, 0..16 = Y, 16..20 = U, 20..24 = V.
        let mut coeffs = [[0i32; 16]; 25];
        let has_y2;
        let mut any_nonzero = false;
        if skip_coeff {
            // Skipped MB: clear the non-zero context for all sub-blocks.
            // The Y2 context is preserved across a skipped non-B_PRED MB.
            for c in 0..8 {
                left_nz[c] = false;
                self.above_nz[mb_x * 9 + c] = false;
            }
            if y_mode == B_PRED {
                left_nz[8] = false;
                self.above_nz[mb_x * 9 + 8] = false;
            }
            has_y2 = y_mode != B_PRED;
        } else {
            let dq = self.dequant[segment_id as usize];
            let part = mb_y % self.token_bd.len();
            let (y2_present, nonzero) =
                self.decode_residuals(mb_x, y_mode, &dq, &mut coeffs, part, left_nz);
            has_y2 = y2_present;
            any_nonzero = nonzero;
        }

        // --- reconstruction ---
        #[cfg(test)]
        if mb_x == 0 && mb_y == 0 {
            self.debug_first_mb = format!(
                "MB(0,0): y_mode={y_mode} uv_mode={uv_mode} skip={skip_coeff} seg={segment_id} bmodes={bmodes:?} y2[0..4]={:?} Y0[0..6]={:?} U0[0..6]={:?} {}",
                &coeffs[24][0..4],
                &coeffs[0][0..6],
                &coeffs[16][0..6],
                self.debug_tok_state,
            );
        }
        if y_mode == B_PRED {
            self.reconstruct_bpred(mb_x, mb_y, &bmodes, &coeffs);
        } else {
            self.reconstruct_y16(mb_x, mb_y, y_mode, has_y2, &mut coeffs);
        }
        self.reconstruct_chroma(mb_x, mb_y, uv_mode, &coeffs);

        // --- record loop-filter info ---
        let filter_level = self.compute_mb_filter_level(segment_id, y_mode);
        self.mb_info[mb_idx] = MbInfo {
            y_mode,
            skip_coeff: skip_coeff || !any_nonzero,
            filter_level,
        };
        #[cfg(test)]
        self.debug_mb_modes
            .push_str(&format!("{y_mode}/{uv_mode} "));
    }

    /// Reads the per-MB segment id from the segment-id tree.
    fn read_segment_id(&mut self) -> u8 {
        if !self.header.segment.enabled || !self.header.segment.update_map {
            return 0;
        }
        let probs = &self.header.segment.tree_probs;
        // 2-level binary tree over 4 segments.
        if self.bd.get_bool(probs[0]) {
            if self.bd.get_bool(probs[2]) {
                3
            } else {
                2
            }
        } else if self.bd.get_bool(probs[1]) {
            1
        } else {
            0
        }
    }

    /// Reads the 16x16 luma mode for a key-frame macroblock.
    fn read_kf_ymode(&mut self) -> usize {
        self.bd.read_tree(&KF_YMODE_TREE, &KF_YMODE_PROB) as usize
    }

    /// Reads the 8x8 chroma mode for a key-frame macroblock.
    fn read_kf_uvmode(&mut self) -> usize {
        self.bd.read_tree(&UV_MODE_TREE, &KF_UV_MODE_PROB) as usize
    }

    /// Decodes the DCT-token residuals for one macroblock.
    ///
    /// Returns `(has_y2, any_nonzero)`. Coefficients are written dequantised
    /// into `coeffs` in raster order per sub-block.
    fn decode_residuals(
        &mut self,
        mb_x: usize,
        y_mode: usize,
        dq: &DequantFactors,
        coeffs: &mut [[i32; 16]; 25],
        part: usize,
        left_nz: &mut [bool; 9],
    ) -> (bool, bool) {
        let has_y2 = y_mode != B_PRED;
        let mut any_nonzero = false;
        let coeff_probs = &self.header.coeff_probs;

        #[cfg(test)]
        if mb_x == 0 {
            let (po, rg, va, bc) = self.token_bd[part].debug_state();
            self.debug_tok_state = format!(
                "[tok-state pos={po} range={rg} value={va} bc={bc} cp[3][0][0][0]={}]",
                coeff_probs[3][0][0][0],
            );
        }

        // --- Y2 block (block type 1) ---
        if has_y2 {
            let ctx = usize::from(left_nz[8]) + usize::from(self.above_nz[mb_x * 9 + 8]);
            let nz = decode_block(
                &mut self.token_bd[part],
                coeff_probs,
                1,
                ctx,
                0,
                dq.y2_dc,
                dq.y2_ac,
                &mut coeffs[24],
            );
            left_nz[8] = nz;
            self.above_nz[mb_x * 9 + 8] = nz;
            any_nonzero |= nz;
        }

        // Luma block type: 0 when Y2 carries DC, else 3 (coeff 0 included).
        let y_block_type = if has_y2 { 0 } else { 3 };
        let first_coeff = if has_y2 { 1 } else { 0 };

        // --- 16 luma sub-blocks ---
        for r in 0..4 {
            for col in 0..4 {
                let sb = r * 4 + col;
                let ctx = usize::from(self.above_nz[mb_x * 9 + col]) + usize::from(left_nz[r]);
                let nz = decode_block(
                    &mut self.token_bd[part],
                    coeff_probs,
                    y_block_type,
                    ctx,
                    first_coeff,
                    dq.y_dc,
                    dq.y_ac,
                    &mut coeffs[sb],
                );
                self.above_nz[mb_x * 9 + col] = nz;
                left_nz[r] = nz;
                any_nonzero |= nz;
            }
        }

        #[cfg(test)]
        if mb_x == 0 {
            let (po, _, va, _) = self.token_bd[part].debug_state();
            self.debug_tok_state
                .push_str(&format!(" [afterY pos={po} val={va}]"));
        }

        // --- 4 U + 4 V chroma sub-blocks (block type 2) ---
        for (plane, base) in [(0usize, 16usize), (1usize, 20usize)] {
            // U uses above/left context slots 4,5; V uses 6,7.
            let ctx_base = 4 + plane * 2;
            for r in 0..2 {
                for col in 0..2 {
                    let sb = base + r * 2 + col;
                    let a_idx = mb_x * 9 + ctx_base + col;
                    let l_idx = ctx_base + r;
                    let ctx = usize::from(self.above_nz[a_idx]) + usize::from(left_nz[l_idx]);
                    let nz = decode_block(
                        &mut self.token_bd[part],
                        coeff_probs,
                        2,
                        ctx,
                        0,
                        dq.uv_dc,
                        dq.uv_ac,
                        &mut coeffs[sb],
                    );
                    self.above_nz[a_idx] = nz;
                    left_nz[l_idx] = nz;
                    any_nonzero |= nz;
                }
            }
        }

        (has_y2, any_nonzero)
    }

    /// Reconstructs a whole-16x16-predicted luma macroblock.
    fn reconstruct_y16(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        y_mode: usize,
        has_y2: bool,
        coeffs: &mut [[i32; 16]; 25],
    ) {
        let stride = self.planes.y_stride;
        let off = self.planes.y_origin + mb_y * 16 * stride + mb_x * 16;
        let have_up = mb_y > 0;
        let have_left = mb_x > 0;
        // Whole-block prediction over the 16x16 luma region.
        predict_block(
            &mut self.planes.y,
            off,
            stride,
            16,
            y_mode,
            have_up,
            have_left,
        );

        // If a Y2 block was decoded, inverse-WHT it and scatter DCs.
        if has_y2 {
            let mut y2 = coeffs[24];
            iwht4x4(&mut y2);
            for sb in 0..16 {
                coeffs[sb][0] = y2[sb];
            }
        }

        // Inverse-transform and add each 4x4 luma sub-block.
        for r in 0..4 {
            for col in 0..4 {
                let sb = r * 4 + col;
                let mut blk = coeffs[sb];
                idct4x4(&mut blk);
                let sb_off = off + r * 4 * stride + col * 4;
                add_residual(&mut self.planes.y, sb_off, stride, &blk);
            }
        }
    }

    /// Reconstructs a B_PRED (per-4x4) luma macroblock.
    fn reconstruct_bpred(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        bmodes: &[u8; 16],
        coeffs: &[[i32; 16]; 25],
    ) {
        let stride = self.planes.y_stride;
        let mb_off = self.planes.y_origin + mb_y * 16 * stride + mb_x * 16;

        // Each 4x4 sub-block is predicted then immediately reconstructed so
        // later sub-blocks see the correct neighbours.
        for r in 0..4 {
            for col in 0..4 {
                let sb = r * 4 + col;
                let sb_off = mb_off + r * 4 * stride + col * 4;
                let have_up = mb_y > 0 || r > 0;
                let have_left = mb_x > 0 || col > 0;
                let have_up_right = self.subblock_has_up_right(mb_x, mb_y, r, col);
                let edge =
                    self.gather_subblock_edge(sb_off, stride, have_up, have_left, have_up_right);
                predict_subblock(
                    &mut self.planes.y,
                    sb_off,
                    stride,
                    usize::from(bmodes[sb]),
                    &edge,
                );
                let mut blk = coeffs[sb];
                idct4x4(&mut blk);
                add_residual(&mut self.planes.y, sb_off, stride, &blk);
            }
        }
    }

    /// Determines whether a 4x4 sub-block has valid above-right samples.
    fn subblock_has_up_right(&self, mb_x: usize, mb_y: usize, r: usize, col: usize) -> bool {
        if r == 0 {
            // Top sub-block row: above-right is in the macroblock above-right.
            if col < 3 {
                mb_y > 0
            } else {
                // Rightmost column: needs the MB to the upper-right.
                mb_y > 0 && mb_x + 1 < self.mb_cols
            }
        } else {
            // Interior rows: above-right belongs to a sub-block in this MB,
            // which has not yet been reconstructed for col == 3.
            col < 3
        }
    }

    /// Gathers the 8 above + 4 left + corner edge samples for a 4x4 block.
    fn gather_subblock_edge(
        &self,
        sb_off: usize,
        stride: usize,
        have_up: bool,
        have_left: bool,
        have_up_right: bool,
    ) -> SubBlockEdge {
        let plane = &self.planes.y;
        let mut above = [127i32; 8];
        let mut left = [129i32; 4];
        let mut corner = if have_up && have_left {
            i32::from(plane[sb_off - stride - 1])
        } else if have_up {
            129
        } else {
            127
        };
        if have_up {
            for (c, a) in above.iter_mut().take(4).enumerate() {
                *a = i32::from(plane[sb_off - stride + c]);
            }
            if have_up_right {
                for c in 4..8 {
                    above[c] = i32::from(plane[sb_off - stride + c]);
                }
            } else {
                // Replicate the last above sample when the above-right block
                // is unavailable (RFC 6386 §12.3 edge behaviour).
                for c in 4..8 {
                    above[c] = above[3];
                }
            }
        } else {
            corner = 127;
        }
        if have_left {
            for (r, l) in left.iter_mut().enumerate() {
                *l = i32::from(plane[sb_off + r * stride - 1]);
            }
        }
        SubBlockEdge {
            above,
            left,
            corner,
        }
    }

    /// Reconstructs both chroma planes of a macroblock.
    fn reconstruct_chroma(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        uv_mode: usize,
        coeffs: &[[i32; 16]; 25],
    ) {
        let stride = self.planes.uv_stride;
        let off = self.planes.uv_origin + mb_y * 8 * stride + mb_x * 8;
        let have_up = mb_y > 0;
        let have_left = mb_x > 0;

        for (plane_sel, base) in [(0usize, 16usize), (1usize, 20usize)] {
            let plane = if plane_sel == 0 {
                &mut self.planes.u
            } else {
                &mut self.planes.v
            };
            predict_block(plane, off, stride, 8, uv_mode, have_up, have_left);
            for r in 0..2 {
                for col in 0..2 {
                    let sb = base + r * 2 + col;
                    let mut blk = coeffs[sb];
                    idct4x4(&mut blk);
                    let sb_off = off + r * 4 * stride + col * 4;
                    add_residual(plane, sb_off, stride, &blk);
                }
            }
        }
    }

    /// Computes the effective loop-filter level for a macroblock.
    ///
    /// Combines the frame base level with the per-segment level and any per-MB
    /// reference/mode deltas (RFC 6386 §15.2).
    fn compute_mb_filter_level(&self, segment_id: u8, y_mode: usize) -> i32 {
        let lf = &self.header.loop_filter;
        let mut level = lf.level;

        // Segment adjustment.
        if self.header.segment.enabled {
            let seg = self.header.segment.filter_strength[segment_id as usize];
            level = if self.header.segment.abs_delta {
                seg
            } else {
                level + seg
            };
        }
        level = level.clamp(0, 63);

        // Per-MB loop-filter deltas (RFC 6386 §15.2).
        if lf.delta_enabled {
            // Key-frame macroblocks all use the "intra" reference-frame slot 0.
            level += lf.ref_deltas[0];
            // Mode delta slot 0 applies to B_PRED macroblocks.
            if y_mode == B_PRED {
                level += lf.mode_deltas[0];
            }
        }
        level.clamp(0, 63)
    }

    /// Runs the in-loop deblocking filter over the whole frame.
    ///
    /// Filters macroblock edges and (for the normal filter) the three interior
    /// 4-pixel sub-block edges of each macroblock. Edge order is the standard
    /// "all vertical edges of the MB left-to-right, then all horizontal edges
    /// top-to-bottom" sweep (RFC 6386 §15.1).
    fn apply_loop_filter(&mut self) {
        // Diagnostic short-circuit (test builds only): if the env var is set,
        // disable the loop filter to isolate filter-vs-reconstruction errors.
        #[cfg(test)]
        if std::env::var_os("VP8_SKIP_LF").is_some() {
            return;
        }
        if self.header.loop_filter.level == 0 {
            return;
        }
        let sharpness = self.header.loop_filter.sharpness;
        let simple = self.header.loop_filter.simple;

        for mb_y in 0..self.mb_rows {
            for mb_x in 0..self.mb_cols {
                let info = self.mb_info[mb_y * self.mb_cols + mb_x];
                if info.filter_level == 0 {
                    continue;
                }
                let params = compute_filter_params(info.filter_level, sharpness);
                // Inner (sub-block) edges are skipped when the MB is fully
                // skipped and is not B_PRED (no residual => no blocking).
                let filter_inner = !(info.skip_coeff && info.y_mode != B_PRED);

                if simple {
                    self.filter_mb_simple(mb_x, mb_y, &params, filter_inner);
                } else {
                    self.filter_mb_normal(mb_x, mb_y, &params, filter_inner);
                }
            }
        }
    }

    /// Applies the simple loop filter to one macroblock (luma only).
    fn filter_mb_simple(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        params: &FilterParams,
        filter_inner: bool,
    ) {
        let stride = self.planes.y_stride;
        let off = self.planes.y_origin + mb_y * 16 * stride + mb_x * 16;

        // Left macroblock edge (vertical), 16 rows.
        if mb_x > 0 {
            for row in 0..16 {
                let p = off + row * stride;
                simple_filter_edge(&mut self.planes.y, p, 1, params.mbedge_limit);
            }
        }
        // Interior vertical sub-block edges at columns 4, 8, 12.
        if filter_inner {
            for col in [4usize, 8, 12] {
                for row in 0..16 {
                    let p = off + row * stride + col;
                    simple_filter_edge(&mut self.planes.y, p, 1, params.sub_bedge_limit);
                }
            }
        }
        // Top macroblock edge (horizontal), 16 columns.
        if mb_y > 0 {
            for col in 0..16 {
                let p = off + col;
                simple_filter_edge(&mut self.planes.y, p, stride, params.mbedge_limit);
            }
        }
        // Interior horizontal sub-block edges at rows 4, 8, 12.
        if filter_inner {
            for row in [4usize, 8, 12] {
                for col in 0..16 {
                    let p = off + row * stride + col;
                    simple_filter_edge(&mut self.planes.y, p, stride, params.sub_bedge_limit);
                }
            }
        }
    }

    /// Applies the normal loop filter to one macroblock (luma + chroma).
    fn filter_mb_normal(
        &mut self,
        mb_x: usize,
        mb_y: usize,
        params: &FilterParams,
        filter_inner: bool,
    ) {
        let y_stride = self.planes.y_stride;
        let uv_stride = self.planes.uv_stride;
        let y_off = self.planes.y_origin + mb_y * 16 * y_stride + mb_x * 16;
        let uv_off = self.planes.uv_origin + mb_y * 8 * uv_stride + mb_x * 8;

        // --- left macroblock edge ---
        if mb_x > 0 {
            for row in 0..16 {
                let p = y_off + row * y_stride;
                normal_mbedge_filter_edge(&mut self.planes.y, p, 1, params);
            }
            for row in 0..8 {
                let p = uv_off + row * uv_stride;
                normal_mbedge_filter_edge(&mut self.planes.u, p, 1, params);
                normal_mbedge_filter_edge(&mut self.planes.v, p, 1, params);
            }
        }
        // --- interior vertical sub-block edges ---
        if filter_inner {
            for col in [4usize, 8, 12] {
                for row in 0..16 {
                    let p = y_off + row * y_stride + col;
                    normal_subblock_filter_edge(&mut self.planes.y, p, 1, params);
                }
            }
            // Chroma has a single interior vertical edge at column 4.
            for row in 0..8 {
                let p = uv_off + row * uv_stride + 4;
                normal_subblock_filter_edge(&mut self.planes.u, p, 1, params);
                normal_subblock_filter_edge(&mut self.planes.v, p, 1, params);
            }
        }
        // --- top macroblock edge ---
        if mb_y > 0 {
            for col in 0..16 {
                let p = y_off + col;
                normal_mbedge_filter_edge(&mut self.planes.y, p, y_stride, params);
            }
            for col in 0..8 {
                let p = uv_off + col;
                normal_mbedge_filter_edge(&mut self.planes.u, p, uv_stride, params);
                normal_mbedge_filter_edge(&mut self.planes.v, p, uv_stride, params);
            }
        }
        // --- interior horizontal sub-block edges ---
        if filter_inner {
            for row in [4usize, 8, 12] {
                for col in 0..16 {
                    let p = y_off + row * y_stride + col;
                    normal_subblock_filter_edge(&mut self.planes.y, p, y_stride, params);
                }
            }
            for col in 0..8 {
                let p = uv_off + 4 * uv_stride + col;
                normal_subblock_filter_edge(&mut self.planes.u, p, uv_stride, params);
                normal_subblock_filter_edge(&mut self.planes.v, p, uv_stride, params);
            }
        }
    }

    /// Converts the reconstructed YCbCr planes into a tight RGBA buffer.
    fn finish(self) -> Vp8Image {
        let w = self.header.width as usize;
        let h = self.header.height as usize;
        let mut rgba = vec![0u8; w * h * 4];

        for y in 0..h {
            let y_row = self.planes.y_origin + y * self.planes.y_stride;
            let uv_row = self.planes.uv_origin + (y / 2) * self.planes.uv_stride;
            for x in 0..w {
                let yy = i32::from(self.planes.y[y_row + x]);
                let uu = i32::from(self.planes.u[uv_row + x / 2]);
                let vv = i32::from(self.planes.v[uv_row + x / 2]);
                let (r, g, b) = ycbcr_to_rgb(yy, uu, vv);
                let o = (y * w + x) * 4;
                rgba[o] = r;
                rgba[o + 1] = g;
                rgba[o + 2] = b;
                rgba[o + 3] = 255;
            }
        }
        Vp8Image {
            width: self.header.width,
            height: self.header.height,
            rgba,
        }
    }
}

/// Builds the per-segment dequantisation factor table (RFC 6386 §14.1).
fn build_dequant(header: &FrameHeader) -> [DequantFactors; 4] {
    let q = &header.quant;
    let seg = &header.segment;
    let mut out = [DequantFactors::default(); 4];
    for (s, df) in out.iter_mut().enumerate() {
        // Base AC quantiser index, optionally adjusted per segment.
        let base = if seg.enabled {
            if seg.abs_delta {
                seg.quantizer[s]
            } else {
                q.y_ac_qi + seg.quantizer[s]
            }
        } else {
            q.y_ac_qi
        };

        let y_ac_idx = clamp_qindex(base);
        let y_dc_idx = clamp_qindex(base + q.y_dc_delta);
        let y2_dc_idx = clamp_qindex(base + q.y2_dc_delta);
        let y2_ac_idx = clamp_qindex(base + q.y2_ac_delta);
        let uv_dc_idx = clamp_qindex(base + q.uv_dc_delta);
        let uv_ac_idx = clamp_qindex(base + q.uv_ac_delta);

        // Y2 scaling factors per RFC 6386 §14.1: DC x2, AC x155/100 (min 8).
        let y2_dc = DC_QUANT[y2_dc_idx] * 2;
        let mut y2_ac = AC_QUANT[y2_ac_idx] * 155 / 100;
        if y2_ac < 8 {
            y2_ac = 8;
        }
        // Chroma DC factor is capped at 132.
        let mut uv_dc = DC_QUANT[uv_dc_idx];
        if uv_dc > 132 {
            uv_dc = 132;
        }

        *df = DequantFactors {
            y_dc: DC_QUANT[y_dc_idx],
            y_ac: AC_QUANT[y_ac_idx],
            y2_dc,
            y2_ac,
            uv_dc,
            uv_ac: AC_QUANT[uv_ac_idx],
        };
    }
    out
}

/// Decodes one sub-block of DCT coefficient tokens (RFC 6386 §13).
///
/// `block_type` selects the probability plane; `ctx` is the initial token
/// context (0..2); `first_coeff` is the starting zig-zag position (1 for luma
/// when a Y2 block carries DC). Coefficients are written dequantised into
/// `out` in raster order. Returns `true` if any coefficient is non-zero.
#[allow(clippy::too_many_arguments)]
fn decode_block(
    bd: &mut BoolDecoder<'_>,
    coeff_probs: &[[[[u8; 11]; 3]; 8]; 4],
    block_type: usize,
    ctx: usize,
    first_coeff: usize,
    dc_factor: i32,
    ac_factor: i32,
    out: &mut [i32; 16],
) -> bool {
    let mut nonzero = false;
    let mut prev_ctx = ctx;
    let mut i = first_coeff;
    // `skip_eob` mirrors the libvpx semantics: after a literal 0 token the EOB
    // branch is skipped for the immediately-following token.
    let mut skip_eob = false;

    while i < 16 {
        let band = COEFF_BANDS[i];
        let probs = &coeff_probs[block_type][band][prev_ctx];

        // Enter the token tree either at the root (node 0) or — if the previous
        // token was a literal zero — past the EOB branch at node 2.
        let token = if skip_eob {
            bd.read_tree_from(&COEFF_TREE, probs, 2)
        } else {
            bd.read_tree(&COEFF_TREE, probs)
        };

        if token == DCT_EOB {
            break;
        }

        // Decode the magnitude of the token.
        let abs_value = match token {
            DCT_0 => 0,
            DCT_1 => 1,
            DCT_2 => 2,
            DCT_3 => 3,
            DCT_4 => 4,
            DCT_CAT1 => decode_category(bd, &PCAT1, CAT_BASE[0]),
            DCT_CAT2 => decode_category(bd, &PCAT2, CAT_BASE[1]),
            DCT_CAT3 => decode_category(bd, &PCAT3, CAT_BASE[2]),
            DCT_CAT4 => decode_category(bd, &PCAT4, CAT_BASE[3]),
            DCT_CAT5 => decode_category(bd, &PCAT5, CAT_BASE[4]),
            DCT_CAT6 => decode_category(bd, &PCAT6, CAT_BASE[5]),
            _ => 0,
        };

        if abs_value == 0 {
            // Literal zero: next token context is 0, EOB is skipped.
            prev_ctx = 0;
            skip_eob = true;
        } else {
            // Non-zero coefficient: read the sign and dequantise.
            let sign = bd.get_flag();
            let signed = if sign { -abs_value } else { abs_value };
            // Coefficient 0 uses the DC factor, all others the AC factor.
            let factor = if i == 0 { dc_factor } else { ac_factor };
            let zz = ZIGZAG[i];
            out[zz] = signed * factor;
            nonzero = true;
            // Token context for the next coefficient: 1 for |v|==1, else 2.
            prev_ctx = if abs_value == 1 { 1 } else { 2 };
            skip_eob = false;
        }
        i += 1;
    }
    nonzero
}

/// Decodes a category token's extra bits and adds the category base.
///
/// RFC 6386 §13.2: each category reads `probs.len()` extra bits MSB-first,
/// each against its own probability, then adds `base`.
fn decode_category(bd: &mut BoolDecoder<'_>, probs: &[u8], base: i32) -> i32 {
    let mut extra = 0i32;
    for &p in probs {
        extra = (extra << 1) | i32::from(bd.get_bool(p));
    }
    base + extra
}

/// Converts a single YCbCr triple to RGB using the exact fixed-point matrix
/// of libwebp's `VP8YUVToRGB` (`yuv.h`).
///
/// VP8/WebP reconstruct studio-range BT.601 luma/chroma. The conversion is
/// `channel = clip8( (Y * 19077 >> 8) ± (chroma * coeff >> 8) + cst )` where
/// `clip8(v)` right-shifts `v` by 6 and clamps to `[0, 255]`. Using these exact
/// constants (rather than the textbook full-range matrix) is required for
/// bit-accurate output against libwebp / `dwebp`.
fn ycbcr_to_rgb(y: i32, u: i32, v: i32) -> (u8, u8, u8) {
    /// Emulates libwebp `MultHi`: a 16-bit fixed-point multiply, `>> 8`.
    #[inline]
    fn mult_hi(v: i32, coeff: i32) -> i32 {
        (v * coeff) >> 8
    }
    /// Emulates libwebp `VP8Clip8`: `>> 6` then clamp to `[0, 255]`.
    #[inline]
    fn clip8(v: i32) -> u8 {
        (v >> 6).clamp(0, 255) as u8
    }
    let yv = mult_hi(y, 19077);
    let r = yv + mult_hi(v, 26149) - 14234;
    let g = yv - mult_hi(u, 6419) - mult_hi(v, 13320) + 8708;
    let b = yv + mult_hi(u, 33050) - 17685;
    (clip8(r), clip8(g), clip8(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ycbcr_to_rgb_neutral_chroma_is_grey() {
        // With neutral chroma (128) the three channels are equal: a grey.
        let (r, g, b) = ycbcr_to_rgb(150, 128, 128);
        assert_eq!(r, g, "neutral chroma must give grey");
        assert_eq!(g, b, "neutral chroma must give grey");
    }

    #[test]
    fn test_ycbcr_to_rgb_matches_libwebp_constants() {
        // Verified against libwebp `VP8YUVToRGB`: Y=102,U=128,V=190 -> (199,50,100).
        assert_eq!(ycbcr_to_rgb(102, 128, 190), (199, 50, 100));
    }

    #[test]
    fn test_ycbcr_to_rgb_red() {
        // High V boosts red relative to blue.
        let (r, _g, b) = ycbcr_to_rgb(128, 128, 240);
        assert!(r > b, "high V should make red exceed blue");
    }

    #[test]
    fn test_decode_category_base_only() {
        // With zero input every extra bit is 0, so the result is the base.
        let data = [0u8; 16];
        let mut bd = BoolDecoder::new(&data);
        assert_eq!(decode_category(&mut bd, &PCAT1, CAT_BASE[0]), CAT_BASE[0]);
    }

    #[test]
    fn test_build_dequant_no_segments() {
        let mut header = make_minimal_header();
        header.quant.y_ac_qi = 20;
        let dq = build_dequant(&header);
        assert_eq!(dq[0].y_ac, AC_QUANT[20]);
        assert_eq!(dq[0].y_dc, DC_QUANT[20]);
        // Y2 DC is 2x the DC table.
        assert_eq!(dq[0].y2_dc, DC_QUANT[20] * 2);
    }

    #[test]
    fn test_dequant_uv_dc_capped() {
        let mut header = make_minimal_header();
        header.quant.y_ac_qi = 127; // max index
        let dq = build_dequant(&header);
        assert!(dq[0].uv_dc <= 132, "chroma DC must be capped at 132");
    }

    #[test]
    #[ignore]
    fn probe_decode_fixtures() {
        let mut report = String::new();
        for (name, w, h) in [
            ("flat", 32usize, 32usize),
            ("small", 16, 16),
            ("textured", 48, 48),
        ] {
            let path = format!("/tmp/vp8_{name}.webp");
            let Ok(data) = std::fs::read(&path) else {
                report.push_str(&format!("probe: {path} missing\n"));
                continue;
            };
            // Strip RIFF to get the VP8 payload (offset 20, 12 RIFF + 8 chunk).
            if data.len() < 20 {
                continue;
            }
            let payload = &data[20..];
            // Dump header fields for diagnosis.
            if let Ok((hdr, _)) = FrameHeader::parse(payload) {
                report.push_str(&format!(
                    "  hdr {name}: q.y_ac={} dc_d={} y2dc_d={} y2ac_d={} uvdc_d={} uvac_d={} parts={} seg={} lf.level={} lf.simple={} noskip={}\n",
                    hdr.quant.y_ac_qi,
                    hdr.quant.y_dc_delta,
                    hdr.quant.y2_dc_delta,
                    hdr.quant.y2_ac_delta,
                    hdr.quant.uv_dc_delta,
                    hdr.quant.uv_ac_delta,
                    hdr.num_token_partitions,
                    hdr.segment.enabled,
                    hdr.loop_filter.level,
                    hdr.loop_filter.simple,
                    hdr.mb_no_skip_coeff,
                ));
            }
            // Run the decoder directly so the first-MB diagnostic is captured.
            if let Ok((hdr, bd)) = FrameHeader::parse(payload) {
                report.push_str(&format!(
                    "  seg.absdelta={} segq={:?} segf={:?}\n",
                    hdr.segment.abs_delta, hdr.segment.quantizer, hdr.segment.filter_strength,
                ));
                if let Ok(mut dec) = Decoder::new(&hdr, bd, payload) {
                    dec.decode_all();
                    report.push_str(&format!("  {}\n", dec.debug_first_mb));
                    report.push_str(&format!("  modes(y/uv): {}\n", dec.debug_mb_modes));
                    report.push_str(&format!(
                        "  dequant seg3: y_dc={} y_ac={} y2_dc={} y2_ac={} uv_dc={} uv_ac={}\n",
                        dec.dequant[3].y_dc,
                        dec.dequant[3].y_ac,
                        dec.dequant[3].y2_dc,
                        dec.dequant[3].y2_ac,
                        dec.dequant[3].uv_dc,
                        dec.dequant[3].uv_ac,
                    ));
                }
            }
            match decode_vp8_keyframe(payload) {
                Ok(img) => {
                    let n = w * h;
                    let mut sum = [0i64; 3];
                    for px in img.rgba.chunks_exact(4).take(n) {
                        for i in 0..3 {
                            sum[i] += i64::from(px[i]);
                        }
                    }
                    report.push_str(&format!(
                        "probe {name}: {}x{} avg=[{},{},{}] corner=[{},{},{}]\n",
                        img.width,
                        img.height,
                        sum[0] / n as i64,
                        sum[1] / n as i64,
                        sum[2] / n as i64,
                        img.rgba[0],
                        img.rgba[1],
                        img.rgba[2],
                    ));
                    let _ = std::fs::write(format!("/tmp/{name}_mine.rgba"), &img.rgba);
                }
                Err(e) => report.push_str(&format!("probe {name}: ERROR {e}\n")),
            }
        }
        let _ = std::fs::write("/tmp/vp8_probe_out.txt", &report);
    }

    /// Builds a minimal header for dequant tests.
    fn make_minimal_header() -> FrameHeader {
        FrameHeader {
            width: 16,
            height: 16,
            horizontal_scale: 0,
            vertical_scale: 0,
            color_space: 0,
            clamping_required: true,
            segment: super::super::frame_header::SegmentHeader::default(),
            loop_filter: super::super::frame_header::LoopFilterHeader::default(),
            quant: super::super::frame_header::QuantHeader::default(),
            coeff_probs: super::super::tables::DEFAULT_COEFF_PROBS,
            mb_no_skip_coeff: false,
            prob_skip_false: 0,
            y_mode_probs: KF_YMODE_PROB,
            uv_mode_probs: KF_UV_MODE_PROB,
            partitions_start: 10,
            first_partition_size: 1,
            num_token_partitions: 1,
        }
    }
}
