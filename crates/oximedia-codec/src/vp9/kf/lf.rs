//! VP9 loop filter — exact port of libvpx `vp9/common/vp9_loopfilter.c`
//! (`vp9_filter_block_plane_non420`, the general per-MI reference path,
//! bit-identical to the ss00/ss11 mask fast paths) and the
//! `vpx_dsp/loopfilter.c` kernels (8-bit build).
//!
//! Frame application order matches `vp9_loop_filter_frame` /
//! `loop_filter_rows`: superblocks in raster order; per superblock and per
//! plane, all vertical edges first, then all horizontal edges.

#![allow(clippy::too_many_arguments)]
#![allow(clippy::many_single_char_names)]
#![allow(clippy::similar_names)]

use super::recon::{FrameMi, PlaneBuf};
use super::tables;

/// Maximum loop filter level.
const MAX_LOOP_FILTER: i32 = 63;

/// Per-level thresholds (`loop_filter_thresh`): `mblim`, `lim`, `hev_thr`.
#[derive(Clone, Copy, Default)]
struct LfThresh {
    mblim: u8,
    lim: u8,
    hev_thr: u8,
}

/// Frame-level loop filter state: thresholds per level and the per-segment
/// intra filter level (`lfi_n->lvl[seg][INTRA_FRAME][0]`).
pub struct LoopFilterInfo {
    thr: [LfThresh; MAX_LOOP_FILTER as usize + 1],
    /// Filter level per segment for intra blocks.
    lvl_intra: [u8; 8],
}

impl LoopFilterInfo {
    /// Builds thresholds and per-segment levels
    /// (`update_sharpness` + `vp9_loop_filter_frame_init`, intra rows only —
    /// every block of a keyframe has `ref_frame = INTRA_FRAME`, `mode_lf_lut
    /// = 0`).
    #[must_use]
    pub fn new(
        default_filt_lvl: u8,
        sharpness: u8,
        delta_enabled: bool,
        intra_ref_delta: i8,
        seg_enabled: bool,
        seg_abs_delta: bool,
        seg_lf_feature: &[(bool, i16); 8],
    ) -> Self {
        let mut thr = [LfThresh::default(); MAX_LOOP_FILTER as usize + 1];
        for (lvl, t) in thr.iter_mut().enumerate() {
            let lvl = lvl as i32;
            // update_sharpness()
            let mut block_inside_limit =
                lvl >> (i32::from(sharpness > 0) + i32::from(sharpness > 4));
            if sharpness > 0 && block_inside_limit > 9 - i32::from(sharpness) {
                block_inside_limit = 9 - i32::from(sharpness);
            }
            if block_inside_limit < 1 {
                block_inside_limit = 1;
            }
            t.lim = block_inside_limit as u8;
            t.mblim = (2 * (lvl + 2) + block_inside_limit) as u8;
            // vp9_loop_filter_init(): hev_thr = lvl >> 4
            t.hev_thr = (lvl >> 4) as u8;
        }

        // vp9_loop_filter_frame_init(), intra entries.
        let scale = 1i32 << (default_filt_lvl >> 5);
        let mut lvl_intra = [0u8; 8];
        for (seg_id, out) in lvl_intra.iter_mut().enumerate() {
            let mut lvl_seg = i32::from(default_filt_lvl);
            if seg_enabled && seg_lf_feature[seg_id].0 {
                let data = i32::from(seg_lf_feature[seg_id].1);
                lvl_seg = if seg_abs_delta {
                    data
                } else {
                    i32::from(default_filt_lvl) + data
                }
                .clamp(0, MAX_LOOP_FILTER);
            }
            *out = if delta_enabled {
                (lvl_seg + i32::from(intra_ref_delta) * scale).clamp(0, MAX_LOOP_FILTER) as u8
            } else {
                lvl_seg as u8
            };
        }

        Self { thr, lvl_intra }
    }
}

#[inline]
fn schar_clamp(t: i32) -> i32 {
    t.clamp(-128, 127)
}

#[inline]
fn round2(v: u32, n: u32) -> u8 {
    ((v + (1 << (n - 1))) >> n) as u8
}

#[inline]
fn ad(a: u8, b: u8) -> i32 {
    (i32::from(a) - i32::from(b)).abs()
}

/// `filter_mask` (vpx_dsp/loopfilter.c): true = apply filter.
#[inline]
#[allow(clippy::too_many_arguments)]
fn filter_mask(
    limit: u8,
    blimit: u8,
    p3: u8,
    p2: u8,
    p1: u8,
    p0: u8,
    q0: u8,
    q1: u8,
    q2: u8,
    q3: u8,
) -> bool {
    let l = i32::from(limit);
    let mut fail = ad(p3, p2) > l;
    fail |= ad(p2, p1) > l;
    fail |= ad(p1, p0) > l;
    fail |= ad(q1, q0) > l;
    fail |= ad(q2, q1) > l;
    fail |= ad(q3, q2) > l;
    fail |= ad(p0, q0) * 2 + ad(p1, q1) / 2 > i32::from(blimit);
    !fail
}

/// `flat_mask4` with thresh 1.
#[inline]
#[allow(clippy::too_many_arguments)]
fn flat_mask4(p3: u8, p2: u8, p1: u8, p0: u8, q0: u8, q1: u8, q2: u8, q3: u8) -> bool {
    let mut fail = ad(p1, p0) > 1;
    fail |= ad(q1, q0) > 1;
    fail |= ad(p2, p0) > 1;
    fail |= ad(q2, q0) > 1;
    fail |= ad(p3, p0) > 1;
    fail |= ad(q3, q0) > 1;
    !fail
}

/// `hev_mask`.
#[inline]
fn hev_mask(thresh: u8, p1: u8, p0: u8, q0: u8, q1: u8) -> bool {
    let t = i32::from(thresh);
    i32::from(p1).abs_diff(i32::from(p0)) as i32 > t
        || i32::from(q1).abs_diff(i32::from(q0)) as i32 > t
}

/// `filter4` on four pixels addressed via (buf, idx, step).
#[inline]
fn filter4(buf: &mut [u8], pos: usize, step: usize, mask: bool, thresh: u8) {
    if !mask {
        return;
    }
    let ip1 = pos - 2 * step;
    let ip0 = pos - step;
    let iq0 = pos;
    let iq1 = pos + step;

    let hev = hev_mask(thresh, buf[ip1], buf[ip0], buf[iq0], buf[iq1]);
    // The reference XORs with 0x80 to reinterpret pixels as signed chars;
    // subtracting 128 is the same conversion.
    let ps1 = i32::from(buf[ip1]) - 128;
    let ps0 = i32::from(buf[ip0]) - 128;
    let qs0 = i32::from(buf[iq0]) - 128;
    let qs1 = i32::from(buf[iq1]) - 128;

    // add outer taps if we have high edge variance
    let mut filter = if hev { schar_clamp(ps1 - qs1) } else { 0 };
    // inner taps
    filter = schar_clamp(filter + 3 * (qs0 - ps0));

    let filter1 = schar_clamp(filter + 4) >> 3;
    let filter2 = schar_clamp(filter + 3) >> 3;

    buf[iq0] = (schar_clamp(qs0 - filter1) + 128) as u8;
    buf[ip0] = (schar_clamp(ps0 + filter2) + 128) as u8;

    // outer tap adjustments
    let filter = if hev { 0 } else { (filter1 + 1) >> 1 };
    buf[iq1] = (schar_clamp(qs1 - filter) + 128) as u8;
    buf[ip1] = (schar_clamp(ps1 + filter) + 128) as u8;
}

/// `filter8`.
#[inline]
fn filter8(buf: &mut [u8], pos: usize, step: usize, mask: bool, thresh: u8, flat: bool) {
    if flat && mask {
        let g = |i: i32| u32::from(buf[(pos as i64 + i64::from(i) * step as i64) as usize]);
        let (p3, p2, p1, p0) = (g(-4), g(-3), g(-2), g(-1));
        let (q0, q1, q2, q3) = (g(0), g(1), g(2), g(3));
        let mut s = |i: i32, v: u8| {
            buf[(pos as i64 + i64::from(i) * step as i64) as usize] = v;
        };
        s(-3, round2(p3 + p3 + p3 + 2 * p2 + p1 + p0 + q0, 3));
        s(-2, round2(p3 + p3 + p2 + 2 * p1 + p0 + q0 + q1, 3));
        s(-1, round2(p3 + p2 + p1 + 2 * p0 + q0 + q1 + q2, 3));
        s(0, round2(p2 + p1 + p0 + 2 * q0 + q1 + q2 + q3, 3));
        s(1, round2(p1 + p0 + q0 + 2 * q1 + q2 + q3 + q3, 3));
        s(2, round2(p0 + q0 + q1 + 2 * q2 + q3 + q3 + q3, 3));
    } else {
        filter4(buf, pos, step, mask, thresh);
    }
}

/// `filter16` (15-tap wide filter).
#[inline]
fn filter16(
    buf: &mut [u8],
    pos: usize,
    step: usize,
    mask: bool,
    thresh: u8,
    flat: bool,
    flat2: bool,
) {
    if flat2 && flat && mask {
        let g = |i: i32| u32::from(buf[(pos as i64 + i64::from(i) * step as i64) as usize]);
        let (p7, p6, p5, p4) = (g(-8), g(-7), g(-6), g(-5));
        let (p3, p2, p1, p0) = (g(-4), g(-3), g(-2), g(-1));
        let (q0, q1, q2, q3) = (g(0), g(1), g(2), g(3));
        let (q4, q5, q6, q7) = (g(4), g(5), g(6), g(7));
        let mut out = [0u8; 14];
        out[0] = round2(p7 * 7 + p6 * 2 + p5 + p4 + p3 + p2 + p1 + p0 + q0, 4);
        out[1] = round2(p7 * 6 + p6 + p5 * 2 + p4 + p3 + p2 + p1 + p0 + q0 + q1, 4);
        out[2] = round2(
            p7 * 5 + p6 + p5 + p4 * 2 + p3 + p2 + p1 + p0 + q0 + q1 + q2,
            4,
        );
        out[3] = round2(
            p7 * 4 + p6 + p5 + p4 + p3 * 2 + p2 + p1 + p0 + q0 + q1 + q2 + q3,
            4,
        );
        out[4] = round2(
            p7 * 3 + p6 + p5 + p4 + p3 + p2 * 2 + p1 + p0 + q0 + q1 + q2 + q3 + q4,
            4,
        );
        out[5] = round2(
            p7 * 2 + p6 + p5 + p4 + p3 + p2 + p1 * 2 + p0 + q0 + q1 + q2 + q3 + q4 + q5,
            4,
        );
        out[6] = round2(
            p7 + p6 + p5 + p4 + p3 + p2 + p1 + p0 * 2 + q0 + q1 + q2 + q3 + q4 + q5 + q6,
            4,
        );
        out[7] = round2(
            p6 + p5 + p4 + p3 + p2 + p1 + p0 + q0 * 2 + q1 + q2 + q3 + q4 + q5 + q6 + q7,
            4,
        );
        out[8] = round2(
            p5 + p4 + p3 + p2 + p1 + p0 + q0 + q1 * 2 + q2 + q3 + q4 + q5 + q6 + q7 * 2,
            4,
        );
        out[9] = round2(
            p4 + p3 + p2 + p1 + p0 + q0 + q1 + q2 * 2 + q3 + q4 + q5 + q6 + q7 * 3,
            4,
        );
        out[10] = round2(
            p3 + p2 + p1 + p0 + q0 + q1 + q2 + q3 * 2 + q4 + q5 + q6 + q7 * 4,
            4,
        );
        out[11] = round2(
            p2 + p1 + p0 + q0 + q1 + q2 + q3 + q4 * 2 + q5 + q6 + q7 * 5,
            4,
        );
        out[12] = round2(p1 + p0 + q0 + q1 + q2 + q3 + q4 + q5 * 2 + q6 + q7 * 6, 4);
        out[13] = round2(p0 + q0 + q1 + q2 + q3 + q4 + q5 + q6 * 2 + q7 * 7, 4);
        let s = |b: &mut [u8], i: i32, v: u8| {
            b[(pos as i64 + i64::from(i) * step as i64) as usize] = v;
        };
        for (k, i) in (-7..=6).enumerate() {
            s(buf, i, out[k]);
        }
    } else {
        filter8(buf, pos, step, mask, thresh, flat);
    }
}

/// One 8-pixel-wide horizontal edge with the given kernel size.
fn lpf_horizontal(buf: &mut [u8], pos: usize, pitch: usize, t: &LfThresh, size: u8, count: usize) {
    for i in 0..8 * count {
        let s = pos + i;
        let (p3, p2, p1, p0) = (
            buf[s - 4 * pitch],
            buf[s - 3 * pitch],
            buf[s - 2 * pitch],
            buf[s - pitch],
        );
        let (q0, q1, q2, q3) = (
            buf[s],
            buf[s + pitch],
            buf[s + 2 * pitch],
            buf[s + 3 * pitch],
        );
        let mask = filter_mask(t.lim, t.mblim, p3, p2, p1, p0, q0, q1, q2, q3);
        match size {
            4 => filter4(buf, s, pitch, mask, t.hev_thr),
            8 => {
                let flat = flat_mask4(p3, p2, p1, p0, q0, q1, q2, q3);
                filter8(buf, s, pitch, mask, t.hev_thr, flat);
            }
            _ => {
                let flat = flat_mask4(p3, p2, p1, p0, q0, q1, q2, q3);
                let flat2 = flat_mask5(
                    buf[s - 8 * pitch],
                    buf[s - 7 * pitch],
                    buf[s - 6 * pitch],
                    buf[s - 5 * pitch],
                    p0,
                    q0,
                    buf[s + 4 * pitch],
                    buf[s + 5 * pitch],
                    buf[s + 6 * pitch],
                    buf[s + 7 * pitch],
                );
                filter16(buf, s, pitch, mask, t.hev_thr, flat, flat2);
            }
        }
    }
}

/// One 8-pixel-tall vertical edge (`count` rows for the 16-wide kernel).
fn lpf_vertical(buf: &mut [u8], pos: usize, pitch: usize, t: &LfThresh, size: u8, count: usize) {
    let rows = if size == 16 { count } else { 8 };
    for i in 0..rows {
        let s = pos + i * pitch;
        let (p3, p2, p1, p0) = (buf[s - 4], buf[s - 3], buf[s - 2], buf[s - 1]);
        let (q0, q1, q2, q3) = (buf[s], buf[s + 1], buf[s + 2], buf[s + 3]);
        let mask = filter_mask(t.lim, t.mblim, p3, p2, p1, p0, q0, q1, q2, q3);
        match size {
            4 => filter4(buf, s, 1, mask, t.hev_thr),
            8 => {
                let flat = flat_mask4(p3, p2, p1, p0, q0, q1, q2, q3);
                filter8(buf, s, 1, mask, t.hev_thr, flat);
            }
            _ => {
                let flat = flat_mask4(p3, p2, p1, p0, q0, q1, q2, q3);
                let flat2 = flat_mask5(
                    buf[s - 8],
                    buf[s - 7],
                    buf[s - 6],
                    buf[s - 5],
                    p0,
                    q0,
                    buf[s + 4],
                    buf[s + 5],
                    buf[s + 6],
                    buf[s + 7],
                );
                filter16(buf, s, 1, mask, t.hev_thr, flat, flat2);
            }
        }
    }
}

/// `flat_mask5` with thresh 1 (as called by `mb_lpf_*_edge_w`: the outer
/// ring p7..p4 / q4..q7 plays the role of p4..p1 / q1..q4).
#[inline]
#[allow(clippy::too_many_arguments)]
fn flat_mask5(
    p4: u8,
    p3: u8,
    p2: u8,
    p1: u8,
    p0: u8,
    q0: u8,
    q1: u8,
    q2: u8,
    q3: u8,
    q4: u8,
) -> bool {
    let flat4 = flat_mask4(p3, p2, p1, p0, q0, q1, q2, q3);
    flat4 && ad(p4, p0) <= 1 && ad(q4, q0) <= 1
}

/// `filter_selectively_vert` (non-dual path used by the non420 filter).
fn filter_selectively_vert(
    buf: &mut [u8],
    row_pos: usize,
    pitch: usize,
    mut mask_16x16: u32,
    mut mask_8x8: u32,
    mut mask_4x4: u32,
    mut mask_4x4_int: u32,
    lfi: &LoopFilterInfo,
    lfl: &[u8],
) {
    let mut mask = mask_16x16 | mask_8x8 | mask_4x4 | mask_4x4_int;
    let mut s = row_pos;
    let mut li = 0usize;
    while mask != 0 {
        let t = &lfi.thr[lfl[li] as usize];
        if mask & 1 != 0 {
            if mask_16x16 & 1 != 0 {
                lpf_vertical(buf, s, pitch, t, 16, 8);
            } else if mask_8x8 & 1 != 0 {
                lpf_vertical(buf, s, pitch, t, 8, 8);
            } else if mask_4x4 & 1 != 0 {
                lpf_vertical(buf, s, pitch, t, 4, 8);
            }
        }
        if mask_4x4_int & 1 != 0 {
            lpf_vertical(buf, s + 4, pitch, t, 4, 8);
        }
        s += 8;
        li += 1;
        mask >>= 1;
        mask_16x16 >>= 1;
        mask_8x8 >>= 1;
        mask_4x4 >>= 1;
        mask_4x4_int >>= 1;
    }
}

/// `filter_selectively_horiz`, including the shared-threshold 16-wide dual.
fn filter_selectively_horiz(
    buf: &mut [u8],
    row_pos: usize,
    pitch: usize,
    mut mask_16x16: u32,
    mut mask_8x8: u32,
    mut mask_4x4: u32,
    mut mask_4x4_int: u32,
    lfi_info: &LoopFilterInfo,
    lfl: &[u8],
) {
    let mut mask = mask_16x16 | mask_8x8 | mask_4x4 | mask_4x4_int;
    let mut s = row_pos;
    let mut li = 0usize;
    while mask != 0 {
        let mut count = 1usize;
        if mask & 1 != 0 {
            let lfi = &lfi_info.thr[lfl[li] as usize];
            if mask_16x16 & 1 != 0 {
                if mask_16x16 & 3 == 3 {
                    // 16-wide with the FIRST block's thresholds (dual).
                    lpf_horizontal(buf, s, pitch, lfi, 16, 2);
                    count = 2;
                } else {
                    lpf_horizontal(buf, s, pitch, lfi, 16, 1);
                }
            } else if mask_8x8 & 1 != 0 {
                if mask_8x8 & 3 == 3 {
                    let lfin = &lfi_info.thr[lfl[li + 1] as usize];
                    lpf_horizontal(buf, s, pitch, lfi, 8, 1);
                    lpf_horizontal(buf, s + 8, pitch, lfin, 8, 1);
                    if mask_4x4_int & 3 == 3 {
                        lpf_horizontal(buf, s + 4 * pitch, pitch, lfi, 4, 1);
                        lpf_horizontal(buf, s + 8 + 4 * pitch, pitch, lfin, 4, 1);
                    } else {
                        if mask_4x4_int & 1 != 0 {
                            lpf_horizontal(buf, s + 4 * pitch, pitch, lfi, 4, 1);
                        } else if mask_4x4_int & 2 != 0 {
                            lpf_horizontal(buf, s + 8 + 4 * pitch, pitch, lfin, 4, 1);
                        }
                    }
                    count = 2;
                } else {
                    lpf_horizontal(buf, s, pitch, lfi, 8, 1);
                    if mask_4x4_int & 1 != 0 {
                        lpf_horizontal(buf, s + 4 * pitch, pitch, lfi, 4, 1);
                    }
                }
            } else if mask_4x4 & 1 != 0 {
                if mask_4x4 & 3 == 3 {
                    let lfin = &lfi_info.thr[lfl[li + 1] as usize];
                    lpf_horizontal(buf, s, pitch, lfi, 4, 1);
                    lpf_horizontal(buf, s + 8, pitch, lfin, 4, 1);
                    if mask_4x4_int & 3 == 3 {
                        lpf_horizontal(buf, s + 4 * pitch, pitch, lfi, 4, 1);
                        lpf_horizontal(buf, s + 8 + 4 * pitch, pitch, lfin, 4, 1);
                    } else {
                        if mask_4x4_int & 1 != 0 {
                            lpf_horizontal(buf, s + 4 * pitch, pitch, lfi, 4, 1);
                        } else if mask_4x4_int & 2 != 0 {
                            lpf_horizontal(buf, s + 8 + 4 * pitch, pitch, lfin, 4, 1);
                        }
                    }
                    count = 2;
                } else {
                    lpf_horizontal(buf, s, pitch, lfi, 4, 1);
                    if mask_4x4_int & 1 != 0 {
                        lpf_horizontal(buf, s + 4 * pitch, pitch, lfi, 4, 1);
                    }
                }
            } else {
                lpf_horizontal(buf, s + 4 * pitch, pitch, lfi, 4, 1);
            }
        }
        s += 8 * count;
        li += count;
        mask >>= count;
        mask_16x16 >>= count;
        mask_8x8 >>= count;
        mask_4x4 >>= count;
        mask_4x4_int >>= count;
    }
}

/// `vp9_filter_block_plane_non420` for one 64x64 superblock of one plane.
fn filter_block_plane(
    plane: &mut PlaneBuf,
    ss_x: usize,
    ss_y: usize,
    mi: &FrameMi,
    mi_row: usize,
    mi_col: usize,
    lfi: &LoopFilterInfo,
) {
    let row_step = 1 << ss_y;
    let col_step = 1 << ss_x;
    let stride = plane.stride;
    // dst offset of this SB in the plane
    let sb_off = ((mi_row * 8) >> ss_y) * stride + ((mi_col * 8) >> ss_x);

    let mut mask_16x16 = [0u32; 8];
    let mut mask_8x8 = [0u32; 8];
    let mut mask_4x4 = [0u32; 8];
    let mut mask_4x4_int = [0u32; 8];
    let mut lfl = [0u8; 64];

    // Vertical pass with mask construction.
    let mut r = 0usize;
    while r < 8 && mi_row + r < mi.rows {
        let mut mask_16x16_c: u32 = 0;
        let mut mask_8x8_c: u32 = 0;
        let mut mask_4x4_c: u32 = 0;

        let mut c = 0usize;
        while c < 8 && mi_col + c < mi.cols {
            let info = mi.get(mi_row + r, mi_col + c);
            let sb_type = info.sb_type as usize;
            // Keyframe blocks are all intra: skip_this is always false
            // (`mi->skip && is_inter_block(mi)`), kept for fidelity.
            let skip_this = false;
            let block_edge_left = if tables::NUM_4X4_BLOCKS_WIDE[sb_type] > 1 {
                (c & (tables::NUM_8X8_BLOCKS_WIDE[sb_type] as usize - 1)) == 0
            } else {
                true
            };
            let skip_this_c = skip_this && !block_edge_left;
            let block_edge_above = if tables::NUM_4X4_BLOCKS_HIGH[sb_type] > 1 {
                (r & (tables::NUM_8X8_BLOCKS_HIGH[sb_type] as usize - 1)) == 0
            } else {
                true
            };
            let skip_this_r = skip_this && !block_edge_above;
            let tx_size = usize::from(
                tables::UV_TXSIZE_LOOKUP[sb_type][info.tx_size as usize][usize::from(ss_x != 0)]
                    [usize::from(ss_y != 0)],
            );
            let tx_size = if ss_x == 0 && ss_y == 0 {
                info.tx_size as usize
            } else {
                tx_size
            };
            let skip_border_4x4_c = ss_x != 0 && mi_col + c == mi.cols - 1;
            let skip_border_4x4_r = ss_y != 0 && mi_row + r == mi.rows - 1;

            let level = lfi.lvl_intra[info.segment_id as usize];
            lfl[(r << 3) + (c >> ss_x)] = level;
            if level == 0 {
                c += col_step;
                continue;
            }

            let cb = c >> ss_x;
            if tx_size == 3 {
                // TX_32X32
                if !skip_this_c && (cb & 3) == 0 {
                    if !skip_border_4x4_c {
                        mask_16x16_c |= 1 << cb;
                    } else {
                        mask_8x8_c |= 1 << cb;
                    }
                }
                if !skip_this_r && ((r >> ss_y) & 3) == 0 {
                    if !skip_border_4x4_r {
                        mask_16x16[r] |= 1 << cb;
                    } else {
                        mask_8x8[r] |= 1 << cb;
                    }
                }
            } else if tx_size == 2 {
                // TX_16X16
                if !skip_this_c && (cb & 1) == 0 {
                    if !skip_border_4x4_c {
                        mask_16x16_c |= 1 << cb;
                    } else {
                        mask_8x8_c |= 1 << cb;
                    }
                }
                if !skip_this_r && ((r >> ss_y) & 1) == 0 {
                    if !skip_border_4x4_r {
                        mask_16x16[r] |= 1 << cb;
                    } else {
                        mask_8x8[r] |= 1 << cb;
                    }
                }
            } else {
                // force 8x8 filtering on 32x32 boundaries
                if !skip_this_c {
                    if tx_size == 1 || (cb & 3) == 0 {
                        mask_8x8_c |= 1 << cb;
                    } else {
                        mask_4x4_c |= 1 << cb;
                    }
                }
                if !skip_this_r {
                    if tx_size == 1 || ((r >> ss_y) & 3) == 0 {
                        mask_8x8[r] |= 1 << cb;
                    } else {
                        mask_4x4[r] |= 1 << cb;
                    }
                }
                if !skip_this && tx_size < 1 && !skip_border_4x4_c {
                    mask_4x4_int[r] |= 1 << cb;
                }
            }
            c += col_step;
        }

        // Disable filtering on the leftmost column.
        let border_mask: u32 = if mi_col == 0 { !1u32 } else { !0u32 };
        let row_pos = sb_off + (r >> ss_y) * 8 * stride;
        filter_selectively_vert(
            &mut plane.data,
            row_pos,
            stride,
            mask_16x16_c & border_mask,
            mask_8x8_c & border_mask,
            mask_4x4_c & border_mask,
            mask_4x4_int[r],
            lfi,
            &lfl[r << 3..],
        );
        r += row_step;
    }

    // Horizontal pass.
    let mut r = 0usize;
    while r < 8 && mi_row + r < mi.rows {
        let skip_border_4x4_r = ss_y != 0 && mi_row + r == mi.rows - 1;
        let mask_4x4_int_r = if skip_border_4x4_r {
            0
        } else {
            mask_4x4_int[r]
        };
        let (m16, m8, m4);
        if mi_row + r == 0 {
            m16 = 0;
            m8 = 0;
            m4 = 0;
        } else {
            m16 = mask_16x16[r];
            m8 = mask_8x8[r];
            m4 = mask_4x4[r];
        }
        let row_pos = sb_off + (r >> ss_y) * 8 * stride;
        filter_selectively_horiz(
            &mut plane.data,
            row_pos,
            stride,
            m16,
            m8,
            m4,
            mask_4x4_int_r,
            lfi,
            &lfl[r << 3..],
        );
        r += row_step;
    }
}

/// Applies the loop filter to the whole frame (`vp9_loop_filter_frame`,
/// `loop_filter_rows` order: raster superblocks, per SB vertical then
/// horizontal, per plane).
pub fn loop_filter_frame(
    planes: &mut [PlaneBuf; 3],
    subsampling: (usize, usize),
    mi: &FrameMi,
    lfi: &LoopFilterInfo,
) {
    let (ss_x, ss_y) = subsampling;
    let mut mi_row = 0;
    while mi_row < mi.rows {
        let mut mi_col = 0;
        while mi_col < mi.cols {
            for (p, plane) in planes.iter_mut().enumerate() {
                let (px, py) = if p == 0 { (0, 0) } else { (ss_x, ss_y) };
                filter_block_plane(plane, px, py, mi, mi_row, mi_col, lfi);
            }
            mi_col += 8;
        }
        mi_row += 8;
    }
}
