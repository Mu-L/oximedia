//! VP9 intra-frame (keyframe / intra-only) reconstruction driver.
//!
//! Exact port of the intra paths of libvpx `vp9/decoder/vp9_decodeframe.c`
//! (`decode_tiles` / `decode_partition` / `decode_block` /
//! `predict_and_reconstruct_intra_block`), `vp9_decodemv.c`
//! (`read_intra_frame_mode_info`) and `vp9_detokenize.c` (`decode_coefs`),
//! for 8-bit profile-0 4:2:0 frames.

#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_possible_truncation)]

use super::booldec::BoolReader;
use super::hdr::{parse_compressed_header_intra, FrameProbs, TxMode};
use super::itx::{inverse_transform_add, TxKind};
use super::lf::{loop_filter_frame, LoopFilterInfo};
use super::pred::{predict_intra, PredMode};
use super::scan;
use super::tables;
use crate::error::{CodecError, CodecResult};
use crate::vp9::uncompressed::UncompressedHeader;

/// One reconstruction plane with MI-aligned dimensions.
pub struct PlaneBuf {
    /// Pixel data, `stride * height` bytes.
    pub data: Vec<u8>,
    /// Row stride (== aligned width).
    pub stride: usize,
    /// Aligned width in pixels.
    pub width: usize,
    /// Aligned height in pixels.
    pub height: usize,
}

/// Per-8x8 mode info (replicated over all covered grid cells like libvpx's
/// `mi_grid_visible` pointers).
#[derive(Clone, Copy, Default)]
pub struct MiInfo {
    /// Block size (VP9 `BLOCK_SIZE` index).
    pub sb_type: u8,
    /// Skip flag (no residual).
    pub skip: bool,
    /// Transform size (0..=3).
    pub tx_size: u8,
    /// Segment id (0..=7).
    pub segment_id: u8,
    /// Luma prediction mode (`mi->mode`).
    pub mode: u8,
    /// Chroma prediction mode.
    pub uv_mode: u8,
    /// Sub-8x8 luma modes (`bmi[i].as_mode`).
    pub bmi: [u8; 4],
}

/// Frame-wide mode-info grid.
pub struct FrameMi {
    /// Grid height in MI units.
    pub rows: usize,
    /// Grid width in MI units.
    pub cols: usize,
    data: Vec<MiInfo>,
}

impl FrameMi {
    fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            data: vec![MiInfo::default(); rows * cols],
        }
    }

    /// Mode info at `(row, col)`.
    #[must_use]
    pub fn get(&self, row: usize, col: usize) -> MiInfo {
        self.data[row * self.cols + col]
    }

    fn set(&mut self, row: usize, col: usize, mi: MiInfo) {
        self.data[row * self.cols + col] = mi;
    }
}

/// Decoded intra frame: planes + display dimensions.
pub struct DecodedIntraFrame {
    /// Y, U, V planes (MI-aligned).
    pub planes: [PlaneBuf; 3],
    /// Display width.
    pub width: usize,
    /// Display height.
    pub height: usize,
}

/// Per-segment dequant pairs `[dc, ac]` for Y and UV.
struct Dequant {
    y: [[i64; 2]; 8],
    uv: [[i64; 2]; 8],
}

/// `vp9_dc_quant` / `vp9_ac_quant` (8-bit).
fn dc_q(qindex: i32) -> i64 {
    i64::from(tables::DC_QLOOKUP[qindex.clamp(0, 255) as usize])
}
fn ac_q(qindex: i32) -> i64 {
    i64::from(tables::AC_QLOOKUP[qindex.clamp(0, 255) as usize])
}

/// `vp9_get_qindex`.
fn seg_qindex(hdr: &UncompressedHeader, seg_id: usize) -> i32 {
    let base = i32::from(hdr.quant.base_q_idx);
    if hdr.seg.enabled && hdr.seg.feature_enabled[seg_id][0] {
        let data = i32::from(hdr.seg.feature_data[seg_id][0]);
        let q = if hdr.seg.abs_delta { data } else { base + data };
        q.clamp(0, 255)
    } else {
        base
    }
}

/// Whole-frame decode state.
struct FrameState<'h> {
    hdr: &'h UncompressedHeader,
    probs: FrameProbs,
    tx_mode: TxMode,
    lossless: bool,
    dequant: Dequant,
    mi: FrameMi,
    planes: [PlaneBuf; 3],
    /// Entropy contexts per plane, `2 * aligned_mi_cols` bytes each.
    above_ctx: [Vec<u8>; 3],
    /// Per-plane left entropy contexts (one superblock tall).
    left_ctx: [[u8; 16]; 3],
    /// Partition contexts.
    above_seg_ctx: Vec<u8>,
    left_seg_ctx: [u8; 8],
    /// Scratch dequantized-coefficient block (one 32x32 max).
    dqcoeff: Vec<i64>,
    ss_x: usize,
    ss_y: usize,
}

/// Current tile bounds.
#[derive(Clone, Copy)]
struct TileInfo {
    mi_col_start: usize,
    mi_col_end: usize,
}

/// `get_tile_offset` (vp9_tile_common.c).
fn tile_offset(idx: usize, mis: usize, log2: usize) -> usize {
    let sb_units = (mis + 7) >> 3;
    let offset = ((idx * sb_units) >> log2) << 3;
    offset.min(mis)
}

/// Decodes a VP9 keyframe / intra-only frame to planes.
///
/// Scope: 8-bit, 4:2:0 (profile 0). The caller validates profile/bit-depth
/// and frame type before calling.
///
/// # Errors
///
/// Returns [`CodecError::InvalidBitstream`] on malformed data.
pub fn decode_intra_frame(
    hdr: &UncompressedHeader,
    frame_data: &[u8],
) -> CodecResult<DecodedIntraFrame> {
    let width = hdr.width as usize;
    let height = hdr.height as usize;
    let mi_cols = (width + 7) >> 3;
    let mi_rows = (height + 7) >> 3;
    let aligned_mi_cols = (mi_cols + 7) & !7;

    // Compressed header slice.
    let ch_start = hdr.uncompressed_header_bytes;
    let ch_end = ch_start + usize::from(hdr.compressed_header_size);
    if ch_end > frame_data.len() {
        return Err(CodecError::InvalidBitstream(
            "VP9: compressed header extends past frame data".into(),
        ));
    }
    let lossless = hdr.quant.lossless();
    let mut probs = FrameProbs::defaults();
    let tx_mode =
        parse_compressed_header_intra(&frame_data[ch_start..ch_end], lossless, &mut probs)?;

    // Segment dequant tables (setup_segmentation_dequant).
    let mut dequant = Dequant {
        y: [[0; 2]; 8],
        uv: [[0; 2]; 8],
    };
    for seg in 0..8 {
        let q = seg_qindex(hdr, seg);
        dequant.y[seg][0] = dc_q(q + hdr.quant.y_dc_delta);
        dequant.y[seg][1] = ac_q(q);
        dequant.uv[seg][0] = dc_q(q + hdr.quant.uv_dc_delta);
        dequant.uv[seg][1] = ac_q(q + hdr.quant.uv_ac_delta);
    }

    let (ss_x, ss_y) = (
        usize::from(hdr.subsampling_x),
        usize::from(hdr.subsampling_y),
    );
    let y_w = mi_cols * 8;
    let y_h = mi_rows * 8;
    let mk_plane = |w: usize, h: usize| PlaneBuf {
        data: vec![0u8; w * h],
        stride: w,
        width: w,
        height: h,
    };
    let planes = [
        mk_plane(y_w, y_h),
        mk_plane(y_w >> ss_x, y_h >> ss_y),
        mk_plane(y_w >> ss_x, y_h >> ss_y),
    ];

    let mut st = FrameState {
        hdr,
        probs,
        tx_mode,
        lossless,
        dequant,
        mi: FrameMi::new(mi_rows, mi_cols),
        planes,
        above_ctx: [
            vec![0u8; 2 * aligned_mi_cols],
            vec![0u8; 2 * aligned_mi_cols],
            vec![0u8; 2 * aligned_mi_cols],
        ],
        left_ctx: [[0u8; 16]; 3],
        above_seg_ctx: vec![0u8; aligned_mi_cols],
        left_seg_ctx: [0u8; 8],
        dqcoeff: vec![0i64; 32 * 32],
        ss_x,
        ss_y,
    };

    // Tile buffers (get_tile_buffers): 4-byte big-endian sizes, last tile
    // implicit.
    let tile_cols = 1usize << hdr.tile_cols_log2;
    let tile_rows = 1usize << hdr.tile_rows_log2;
    let mut tile_readers: Vec<BoolReader<'_>> = Vec::with_capacity(tile_cols * tile_rows);
    {
        let mut data = &frame_data[ch_end..];
        for tr in 0..tile_rows {
            for tc in 0..tile_cols {
                let is_last = tr == tile_rows - 1 && tc == tile_cols - 1;
                let size = if is_last {
                    data.len()
                } else {
                    if data.len() < 4 {
                        return Err(CodecError::InvalidBitstream(
                            "VP9: truncated tile length".into(),
                        ));
                    }
                    let sz = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
                    data = &data[4..];
                    if sz > data.len() {
                        return Err(CodecError::InvalidBitstream(
                            "VP9: corrupt tile size".into(),
                        ));
                    }
                    sz
                };
                if size == 0 {
                    return Err(CodecError::InvalidBitstream("VP9: empty tile".into()));
                }
                let reader = BoolReader::new(&data[..size]).ok_or_else(|| {
                    CodecError::InvalidBitstream("VP9 tile: invalid marker bit".into())
                })?;
                tile_readers.push(reader);
                data = &data[size..];
            }
        }
    }

    // Tile decode loop (decode_tiles order).
    for tile_row in 0..tile_rows {
        let mi_row_start = tile_offset(tile_row, mi_rows, hdr.tile_rows_log2 as usize);
        let mi_row_end = tile_offset(tile_row + 1, mi_rows, hdr.tile_rows_log2 as usize);
        let mut mi_row = mi_row_start;
        while mi_row < mi_row_end {
            for tile_col in 0..tile_cols {
                let tile = TileInfo {
                    mi_col_start: tile_offset(tile_col, mi_cols, hdr.tile_cols_log2 as usize),
                    mi_col_end: tile_offset(tile_col + 1, mi_cols, hdr.tile_cols_log2 as usize),
                };
                let r = &mut tile_readers[tile_row * tile_cols + tile_col];
                st.left_ctx = [[0u8; 16]; 3];
                st.left_seg_ctx = [0u8; 8];
                let mut mi_col = tile.mi_col_start;
                while mi_col < tile.mi_col_end {
                    st.decode_partition(r, tile, mi_row, mi_col, 12, 4)?;
                    mi_col += 8;
                }
                if r.has_error() {
                    return Err(CodecError::InvalidBitstream(
                        "VP9 tile data overran its partition".into(),
                    ));
                }
            }
            mi_row += 8;
        }
    }

    // Loop filter.
    if hdr.loop_filter.filter_level != 0 {
        let mut seg_lf = [(false, 0i16); 8];
        for s in 0..8 {
            seg_lf[s] = (
                hdr.seg.enabled && hdr.seg.feature_enabled[s][1],
                hdr.seg.feature_data[s][1],
            );
        }
        let lfi = LoopFilterInfo::new(
            hdr.loop_filter.filter_level,
            hdr.loop_filter.sharpness,
            hdr.loop_filter.delta_enabled,
            hdr.loop_filter.ref_deltas[0],
            hdr.seg.enabled,
            hdr.seg.abs_delta,
            &seg_lf,
        );
        loop_filter_frame(&mut st.planes, (ss_x, ss_y), &st.mi, &lfi);
    }

    Ok(DecodedIntraFrame {
        planes: st.planes,
        width,
        height,
    })
}

impl FrameState<'_> {
    /// `decode_partition` (n4x4_l2 is the block width log2 in 4x4 units).
    fn decode_partition(
        &mut self,
        r: &mut BoolReader<'_>,
        tile: TileInfo,
        mi_row: usize,
        mi_col: usize,
        bsize: u8,
        n4x4_l2: usize,
    ) -> CodecResult<()> {
        if mi_row >= self.mi.rows || mi_col >= self.mi.cols {
            return Ok(());
        }
        let n8x8_l2 = n4x4_l2 - 1;
        let num_8x8 = 1usize << n8x8_l2;
        let hbs = num_8x8 >> 1;
        let has_rows = (mi_row + hbs) < self.mi.rows;
        let has_cols = (mi_col + hbs) < self.mi.cols;

        // read_partition with keyframe probabilities.
        let ctx = {
            let above = (self.above_seg_ctx[mi_col] >> n8x8_l2) & 1;
            let left = (self.left_seg_ctx[mi_row & 7] >> n8x8_l2) & 1;
            usize::from(left) * 2 + usize::from(above) + n8x8_l2 * 4
        };
        let probs = &tables::KF_PARTITION_PROBS[ctx];
        let partition: u8 = if has_rows && has_cols {
            r.read_tree(&tables::PARTITION_TREE, probs)
        } else if !has_rows && has_cols {
            if r.read_bool(probs[1]) {
                3
            } else {
                1
            }
        } else if has_rows {
            if r.read_bool(probs[2]) {
                3
            } else {
                2
            }
        } else {
            3
        };

        let subsize = tables::SUBSIZE_LOOKUP[partition as usize][bsize as usize];
        if subsize < 0 {
            return Err(CodecError::InvalidBitstream(
                "VP9: invalid partition subsize".into(),
            ));
        }
        let subsize = subsize as u8;

        if hbs == 0 {
            // 8x8 splits into sub-8x8 block types.
            self.decode_block(r, tile, mi_row, mi_col, subsize, 1, 1)?;
        } else {
            match partition {
                0 => self.decode_block(r, tile, mi_row, mi_col, subsize, n4x4_l2, n4x4_l2)?,
                1 => {
                    self.decode_block(r, tile, mi_row, mi_col, subsize, n4x4_l2, n8x8_l2)?;
                    if has_rows {
                        self.decode_block(
                            r,
                            tile,
                            mi_row + hbs,
                            mi_col,
                            subsize,
                            n4x4_l2,
                            n8x8_l2,
                        )?;
                    }
                }
                2 => {
                    self.decode_block(r, tile, mi_row, mi_col, subsize, n8x8_l2, n4x4_l2)?;
                    if has_cols {
                        self.decode_block(
                            r,
                            tile,
                            mi_row,
                            mi_col + hbs,
                            subsize,
                            n8x8_l2,
                            n4x4_l2,
                        )?;
                    }
                }
                _ => {
                    self.decode_partition(r, tile, mi_row, mi_col, subsize, n8x8_l2)?;
                    self.decode_partition(r, tile, mi_row, mi_col + hbs, subsize, n8x8_l2)?;
                    self.decode_partition(r, tile, mi_row + hbs, mi_col, subsize, n8x8_l2)?;
                    self.decode_partition(r, tile, mi_row + hbs, mi_col + hbs, subsize, n8x8_l2)?;
                }
            }
        }

        // dec_update_partition_context
        if bsize >= 3 && (bsize == 3 || partition != 3) {
            let pc = tables::PARTITION_CONTEXT_LOOKUP[subsize as usize];
            let above_end = (mi_col + num_8x8).min(self.above_seg_ctx.len());
            for v in &mut self.above_seg_ctx[mi_col..above_end] {
                *v = pc[0];
            }
            let left_start = mi_row & 7;
            for v in &mut self.left_seg_ctx[left_start..(left_start + num_8x8).min(8)] {
                *v = pc[1];
            }
        }
        Ok(())
    }

    /// `decode_block` intra path.
    fn decode_block(
        &mut self,
        r: &mut BoolReader<'_>,
        tile: TileInfo,
        mi_row: usize,
        mi_col: usize,
        bsize: u8,
        bwl: usize,
        bhl: usize,
    ) -> CodecResult<()> {
        let bw = 1usize << (bwl - 1);
        let bh = 1usize << (bhl - 1);
        let x_mis = bw.min(self.mi.cols - mi_col);
        let y_mis = bh.min(self.mi.rows - mi_row);

        let above_mi = if mi_row > 0 {
            Some(self.mi.get(mi_row - 1, mi_col))
        } else {
            None
        };
        let left_mi = if mi_col > tile.mi_col_start {
            Some(self.mi.get(mi_row, mi_col - 1))
        } else {
            None
        };

        if bsize >= 3 && (self.ss_x != 0 || self.ss_y != 0) {
            let uv_subsize = tables::SS_SIZE_LOOKUP[bsize as usize][self.ss_x][self.ss_y];
            if uv_subsize < 0 {
                return Err(CodecError::InvalidBitstream(
                    "VP9: invalid uv block size".into(),
                ));
            }
        }

        // --- read_intra_frame_mode_info ---
        let seg = &self.hdr.seg;
        let segment_id: u8 = if seg.enabled && seg.update_map {
            r.read_tree(&tables::SEGMENT_TREE, &seg.tree_probs)
        } else {
            0
        };

        // read_skip
        let skip = if seg.enabled && seg.feature_enabled[segment_id as usize][3] {
            true
        } else {
            let ctx = usize::from(above_mi.is_some_and(|m| m.skip))
                + usize::from(left_mi.is_some_and(|m| m.skip));
            r.read_bool(self.probs.skip[ctx])
        };

        // read_tx_size (allow_select = 1)
        let max_tx_size = tables::MAX_TXSIZE_LOOKUP[bsize as usize];
        let tx_size: u8 = if self.tx_mode == TxMode::Select && bsize >= 3 {
            let has_above = above_mi.is_some();
            let has_left = left_mi.is_some();
            let mut above_ctx = above_mi
                .filter(|m| !m.skip)
                .map_or(max_tx_size, |m| m.tx_size);
            let mut left_ctx = left_mi
                .filter(|m| !m.skip)
                .map_or(max_tx_size, |m| m.tx_size);
            if !has_left {
                left_ctx = above_ctx;
            }
            if !has_above {
                above_ctx = left_ctx;
            }
            let ctx = usize::from(above_ctx + left_ctx > max_tx_size);
            let p: &[u8] = match max_tx_size {
                1 => &self.probs.tx8[ctx],
                2 => &self.probs.tx16[ctx],
                _ => &self.probs.tx32[ctx],
            };
            let mut tx = u8::from(r.read_bool(p[0]));
            if tx != 0 && max_tx_size >= 2 {
                tx += u8::from(r.read_bool(p[1]));
                if tx != 1 && max_tx_size >= 3 {
                    tx += u8::from(r.read_bool(p[2]));
                }
            }
            tx
        } else {
            max_tx_size.min(self.tx_mode.biggest_tx_size() as u8)
        };

        // Keyframe Y modes with above/left block-mode contexts.
        let above_block_mode = |cur: &MiInfo, b: usize| -> u8 {
            if b == 0 || b == 1 {
                match above_mi {
                    Some(m) => {
                        if m.sb_type < 3 {
                            m.bmi[b + 2]
                        } else {
                            m.mode
                        }
                    }
                    None => 0,
                }
            } else {
                cur.bmi[b - 2]
            }
        };
        let left_block_mode = |cur: &MiInfo, b: usize| -> u8 {
            if b == 0 || b == 2 {
                match left_mi {
                    Some(m) => {
                        if m.sb_type < 3 {
                            m.bmi[b + 1]
                        } else {
                            m.mode
                        }
                    }
                    None => 0,
                }
            } else {
                cur.bmi[b - 1]
            }
        };
        let mut mi = MiInfo {
            sb_type: bsize,
            skip,
            tx_size,
            segment_id,
            ..MiInfo::default()
        };
        let read_mode = |r: &mut BoolReader<'_>, cur: &MiInfo, b: usize| -> u8 {
            let a = above_block_mode(cur, b) as usize;
            let l = left_block_mode(cur, b) as usize;
            r.read_tree(&tables::INTRA_MODE_TREE, &tables::KF_Y_MODE_PROBS[a][l])
        };
        match bsize {
            0 => {
                // BLOCK_4X4
                for i in 0..4 {
                    mi.bmi[i] = read_mode(r, &mi, i);
                }
                mi.mode = mi.bmi[3];
            }
            1 => {
                // BLOCK_4X8
                let m0 = read_mode(r, &mi, 0);
                mi.bmi[0] = m0;
                mi.bmi[2] = m0;
                let m1 = read_mode(r, &mi, 1);
                mi.bmi[1] = m1;
                mi.bmi[3] = m1;
                mi.mode = m1;
            }
            2 => {
                // BLOCK_8X4
                let m0 = read_mode(r, &mi, 0);
                mi.bmi[0] = m0;
                mi.bmi[1] = m0;
                let m2 = read_mode(r, &mi, 2);
                mi.bmi[2] = m2;
                mi.bmi[3] = m2;
                mi.mode = m2;
            }
            _ => {
                mi.mode = read_mode(r, &mi, 0);
            }
        }
        mi.uv_mode = r.read_tree(
            &tables::INTRA_MODE_TREE,
            &tables::KF_UV_MODE_PROBS[mi.mode as usize],
        );

        // Replicate MI over covered cells (set_offsets).
        for y in 0..y_mis {
            for x in 0..x_mis {
                self.mi.set(mi_row + y, mi_col + x, mi);
            }
        }

        // dec_reset_skip_context
        if mi.skip {
            let above_len = 2 * self.mi_cols_al();
            for plane in 0..3 {
                let (px, py) = self.plane_ss(plane);
                let n4_w = (bw << 1) >> px;
                let n4_h = (bh << 1) >> py;
                let a0 = (mi_col * 2) >> px;
                let l0 = ((mi_row * 2) & 15) >> py;
                for v in &mut self.above_ctx[plane][a0..(a0 + n4_w).min(above_len)] {
                    *v = 0;
                }
                for v in &mut self.left_ctx[plane][l0..(l0 + n4_h).min(16)] {
                    *v = 0;
                }
            }
        }

        // Reconstruction: per plane, per tx block.
        let mb_to_right_edge = ((self.mi.cols as i64) - (bw as i64) - (mi_col as i64)) * 64;
        let mb_to_bottom_edge = ((self.mi.rows as i64) - (bh as i64) - (mi_row as i64)) * 64;
        for plane in 0..3 {
            let (px, py) = self.plane_ss(plane);
            let n4_w = (bw << 1) >> px;
            let n4_h = (bh << 1) >> py;
            let n4_wl = bwl - px;
            let tx = if plane == 0 {
                mi.tx_size
            } else {
                tables::UV_TXSIZE_LOOKUP[bsize as usize][mi.tx_size as usize][px][py]
            };
            let step = 1usize << tx;
            let max_blocks_wide = if mb_to_right_edge >= 0 {
                n4_w
            } else {
                (n4_w as i64 + (mb_to_right_edge >> (5 + px as i64))) as usize
            };
            let max_blocks_high = if mb_to_bottom_edge >= 0 {
                n4_h
            } else {
                (n4_h as i64 + (mb_to_bottom_edge >> (5 + py as i64))) as usize
            };
            // xd->max_blocks_wide is 0 unless the block crosses the edge.
            let limit_w = if mb_to_right_edge >= 0 {
                0
            } else {
                max_blocks_wide
            };
            let limit_h = if mb_to_bottom_edge >= 0 {
                0
            } else {
                max_blocks_high
            };

            let mut row = 0usize;
            while row < max_blocks_high {
                let mut col = 0usize;
                while col < max_blocks_wide {
                    self.predict_and_reconstruct(
                        r,
                        &mi,
                        tile,
                        plane,
                        mi_row,
                        mi_col,
                        row,
                        col,
                        tx,
                        n4_wl,
                        limit_w,
                        limit_h,
                        mb_to_right_edge < 0,
                        mb_to_bottom_edge < 0,
                        above_mi.is_some(),
                        left_mi.is_some(),
                    )?;
                    col += step;
                }
                row += step;
            }
        }
        Ok(())
    }

    fn plane_ss(&self, plane: usize) -> (usize, usize) {
        if plane == 0 {
            (0, 0)
        } else {
            (self.ss_x, self.ss_y)
        }
    }

    fn mi_cols_al(&self) -> usize {
        (self.mi.cols + 7) & !7
    }

    /// `predict_and_reconstruct_intra_block`.
    fn predict_and_reconstruct(
        &mut self,
        r: &mut BoolReader<'_>,
        mi: &MiInfo,
        _tile: TileInfo,
        plane: usize,
        mi_row: usize,
        mi_col: usize,
        row: usize,
        col: usize,
        tx: u8,
        n4_wl: usize,
        limit_w: usize,
        limit_h: usize,
        edge_slow_x: bool,
        edge_slow_y: bool,
        has_above_mi: bool,
        has_left_mi: bool,
    ) -> CodecResult<()> {
        let (px, py) = self.plane_ss(plane);
        let mut mode_idx = if plane == 0 { mi.mode } else { mi.uv_mode };
        if mi.sb_type < 3 && plane == 0 {
            mode_idx = mi.bmi[(row << 1) + col];
        }
        let mode = PredMode::from_index(mode_idx);

        let bs = 4usize << tx;
        let x0 = ((mi_col * 8) >> px) + 4 * col;
        let y0 = ((mi_row * 8) >> py) + 4 * row;

        // vp9_predict_intra_block availability.
        let have_top = row > 0 || has_above_mi;
        let have_left = col > 0 || has_left_mi;
        let txw = 1usize << tx;
        let have_right = (col + txw) < (1usize << n4_wl);

        {
            let plane_buf = &mut self.planes[plane];
            predict_intra(
                &mut plane_buf.data,
                plane_buf.stride,
                x0,
                y0,
                bs,
                mode,
                have_top,
                have_left,
                have_right,
                edge_slow_x,
                edge_slow_y,
                plane_buf.width,
                plane_buf.height,
            );
        }

        if !mi.skip {
            let tx_type = if plane > 0 || self.lossless {
                TxKind::DctDct
            } else {
                mode_to_tx_type(mode_idx)
            };
            let (scan_tbl, nb_tbl) = select_scan(tx, tx_type);
            let eob = self.decode_block_tokens(
                r,
                plane,
                mi.segment_id as usize,
                mi_row,
                mi_col,
                col,
                row,
                tx,
                scan_tbl,
                nb_tbl,
                limit_w,
                limit_h,
            );
            if eob > 0 {
                let plane_buf = &mut self.planes[plane];
                let off = y0 * plane_buf.stride + x0;
                inverse_transform_add(
                    tx as usize,
                    tx_type,
                    self.lossless,
                    &self.dqcoeff,
                    &mut plane_buf.data,
                    off,
                    plane_buf.stride,
                );
                // Clear the used coefficients (libvpx zeroes eob-dependent
                // spans; clearing the whole tx block is equivalent).
                let n = (4usize << tx) * (4usize << tx);
                self.dqcoeff[..n].fill(0);
            }
        }
        Ok(())
    }

    /// `vp9_decode_block_tokens`.
    fn decode_block_tokens(
        &mut self,
        r: &mut BoolReader<'_>,
        plane: usize,
        seg_id: usize,
        mi_row: usize,
        mi_col: usize,
        x: usize,
        y: usize,
        tx: u8,
        scan_tbl: &[i16],
        nb_tbl: &[i16],
        limit_w: usize,
        limit_h: usize,
    ) -> usize {
        let (px, py) = self.plane_ss(plane);
        let a0 = ((mi_col * 2) >> px) + x;
        let l0 = (((mi_row * 2) & 15) >> py) + y;
        let n = 1usize << tx; // tx size in 4x4 units

        let ctx = {
            let a_any = self.above_ctx[plane][a0..a0 + n].iter().any(|&v| v != 0);
            let l_any = self.left_ctx[plane][l0..l0 + n].iter().any(|&v| v != 0);
            usize::from(a_any) + usize::from(l_any)
        };

        let dequant = if plane == 0 {
            self.dequant.y[seg_id]
        } else {
            self.dequant.uv[seg_id]
        };
        let plane_type = usize::from(plane > 0);

        let eob = decode_coefs(
            r,
            &self.probs,
            plane_type,
            tx as usize,
            dequant,
            ctx,
            scan_tbl,
            nb_tbl,
            &mut self.dqcoeff,
        );

        // Context update with edge truncation (get_ctx_shift semantics: the
        // `((eob > 0) * 0x0101...) >> ctx_shift` little-endian store sets the
        // first `limit - x` bytes and zeroes the rest).
        let a_valid = if limit_w != 0 && n + x > limit_w {
            limit_w - x
        } else {
            n
        };
        let l_valid = if limit_h != 0 && n + y > limit_h {
            limit_h - y
        } else {
            n
        };
        let v = u8::from(eob > 0);
        for i in 0..n {
            self.above_ctx[plane][a0 + i] = if i < a_valid { v } else { 0 };
        }
        for i in 0..n {
            self.left_ctx[plane][l0 + i] = if i < l_valid { v } else { 0 };
        }
        eob
    }
}

/// `intra_mode_to_tx_type_lookup` (vp9_reconintra.c).
fn mode_to_tx_type(mode: u8) -> TxKind {
    match mode {
        1 | 5 | 8 => TxKind::AdstDct, // V, D117, D63
        2 | 6 | 7 => TxKind::DctAdst, // H, D153, D207
        4 | 9 => TxKind::AdstAdst,    // D135, TM
        _ => TxKind::DctDct,          // DC, D45
    }
}

/// `vp9_scan_orders[tx_size][tx_type]` (scan, neighbors).
fn select_scan(tx: u8, kind: TxKind) -> (&'static [i16], &'static [i16]) {
    match (tx, kind) {
        (0, TxKind::AdstDct) => (&scan::ROW_SCAN_4X4, &scan::ROW_SCAN_4X4_NB),
        (0, TxKind::DctAdst) => (&scan::COL_SCAN_4X4, &scan::COL_SCAN_4X4_NB),
        (0, _) => (&scan::DEFAULT_SCAN_4X4, &scan::DEFAULT_SCAN_4X4_NB),
        (1, TxKind::AdstDct) => (&scan::ROW_SCAN_8X8, &scan::ROW_SCAN_8X8_NB),
        (1, TxKind::DctAdst) => (&scan::COL_SCAN_8X8, &scan::COL_SCAN_8X8_NB),
        (1, _) => (&scan::DEFAULT_SCAN_8X8, &scan::DEFAULT_SCAN_8X8_NB),
        (2, TxKind::AdstDct) => (&scan::ROW_SCAN_16X16, &scan::ROW_SCAN_16X16_NB),
        (2, TxKind::DctAdst) => (&scan::COL_SCAN_16X16, &scan::COL_SCAN_16X16_NB),
        (2, _) => (&scan::DEFAULT_SCAN_16X16, &scan::DEFAULT_SCAN_16X16_NB),
        _ => (&scan::DEFAULT_SCAN_32X32, &scan::DEFAULT_SCAN_32X32_NB),
    }
}

/// `get_coef_context` (vp9_scan.h).
#[inline]
fn coef_context(nb: &[i16], token_cache: &[u8; 1024], c: usize) -> usize {
    ((1 + u32::from(token_cache[nb[2 * c] as usize])
        + u32::from(token_cache[nb[2 * c + 1] as usize]))
        >> 1) as usize
}

/// `decode_coefs` (vp9_detokenize.c), intra (`ref = 0`), 8-bit.
fn decode_coefs(
    r: &mut BoolReader<'_>,
    probs: &FrameProbs,
    plane_type: usize,
    tx: usize,
    dq: [i64; 2],
    mut ctx: usize,
    scan_tbl: &[i16],
    nb_tbl: &[i16],
    dqcoeff: &mut [i64],
) -> usize {
    let max_eob = 16usize << (tx << 1);
    let coef_probs = &probs.coef[tx][plane_type][0];
    let band_translate: &[u8] = if tx == 0 {
        &tables::COEFBAND_TRANS_4X4
    } else {
        &tables::COEFBAND_TRANS_8X8PLUS
    };
    let dq_shift = i64::from(tx == 3);
    let mut token_cache = [0u8; 1024];
    let mut dqv = dq[0];
    let mut c = 0usize;

    while c < max_eob {
        let mut band = band_translate[c] as usize;
        let mut prob = &coef_probs[band][ctx];

        // EOB_CONTEXT_NODE
        if !r.read_bool(prob[0]) {
            break;
        }

        // ZERO_CONTEXT_NODE run
        while !r.read_bool(prob[1]) {
            dqv = dq[1];
            token_cache[scan_tbl[c] as usize] = 0;
            c += 1;
            if c >= max_eob {
                return c; // zero tokens at the end (no eob token)
            }
            ctx = coef_context(nb_tbl, &token_cache, c);
            band = band_translate[c] as usize;
            prob = &coef_probs[band][ctx];
        }

        // ONE_CONTEXT_NODE
        let v: i64;
        if r.read_bool(prob[2]) {
            // Probabilities are always >= 1 on valid streams (defaults and
            // inv_remap_prob both guarantee it); max(1) guards the index.
            let p = &tables::PARETO8_FULL[usize::from(prob[2].max(1)) - 1];
            if r.read_bool(p[0]) {
                if r.read_bool(p[3]) {
                    token_cache[scan_tbl[c] as usize] = 5;
                    let val: i64 = if r.read_bool(p[5]) {
                        if r.read_bool(p[7]) {
                            67 + read_coeff(r, &tables::CAT6_PROB)
                        } else {
                            35 + read_coeff(r, &tables::CAT5_PROB)
                        }
                    } else if r.read_bool(p[6]) {
                        19 + read_coeff(r, &tables::CAT4_PROB)
                    } else {
                        11 + read_coeff(r, &tables::CAT3_PROB)
                    };
                    v = (val * dqv) >> dq_shift;
                } else {
                    token_cache[scan_tbl[c] as usize] = 4;
                    let val: i64 = if r.read_bool(p[4]) {
                        7 + read_coeff(r, &tables::CAT2_PROB)
                    } else {
                        5 + read_coeff(r, &tables::CAT1_PROB)
                    };
                    v = (val * dqv) >> dq_shift;
                }
            } else if r.read_bool(p[1]) {
                token_cache[scan_tbl[c] as usize] = 3;
                v = ((3 + i64::from(r.read_bool(p[2]))) * dqv) >> dq_shift;
            } else {
                token_cache[scan_tbl[c] as usize] = 2;
                v = (2 * dqv) >> dq_shift;
            }
        } else {
            token_cache[scan_tbl[c] as usize] = 1;
            v = dqv >> dq_shift;
        }

        // Sign; store with libvpx's (tran_low_t) int16 truncation.
        let signed = if r.read_bool(128) { -v } else { v };
        dqcoeff[scan_tbl[c] as usize] = i64::from(signed as i16);
        c += 1;
        ctx = coef_context(nb_tbl, &token_cache, c);
        dqv = dq[1];
    }

    c
}

/// `read_coeff` (extra-bit categories, MSB first).
fn read_coeff(r: &mut BoolReader<'_>, cat_probs: &[u8]) -> i64 {
    let mut val: i64 = 0;
    for &p in cat_probs {
        val = (val << 1) | i64::from(r.read_bool(p));
    }
    val
}
