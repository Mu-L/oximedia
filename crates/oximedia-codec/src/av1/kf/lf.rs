//! AV1 deblocking loop filter (spec 7.14), intra-frame path.
//!
//! Exact port of the loop filter process: per-plane vertical-then-horizontal
//! passes, the edge loop filter process with block/transform edge detection,
//! the filter size and adaptive filter strength processes (with per-block
//! `DeltaLFs` and segment features), the filter mask process, and the
//! narrow (filter4) and wide (8/16-tap) sample filters.
//!
//! On intra frames every block satisfies `isIntra == 1`, `ref ==
//! INTRA_FRAME` and `modeType == 0` (all Y modes are intra), which this
//! implementation relies on (it decodes keyframes/intra-only frames only).

#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]

use super::consts::{FRAME_LF_COUNT, MAX_LOOP_FILTER, SEG_LVL_MAX};
use super::hdr::FrameHdr;
use super::tables_conv::{TX_HEIGHT, TX_WIDTH};

/// Everything the loop filter needs from the decoded frame state.
pub struct LfInput<'a> {
    pub hdr: &'a FrameHdr,
    pub sub_x: bool,
    pub sub_y: bool,
    pub num_planes: usize,
    pub mi_rows: usize,
    pub mi_cols: usize,
    /// Per-MI grids from reconstruction.
    pub mi_sizes: &'a [u8],
    pub skips: &'a [u8],
    pub seg_ids: &'a [u8],
    pub delta_lfs: &'a [[i8; FRAME_LF_COUNT]],
    /// `LoopfilterTxSizes[plane]` at plane-subsampled MI positions (stored
    /// on the shared MI grid).
    pub lf_tx_sizes: &'a [Vec<u8>; 3],
}

/// `SEG_LVL_ALT_LF_Y_V` (spec symbols) — base index of the LF features.
const SEG_LVL_ALT_LF_Y_V: usize = 1;

/// Applies the loop filter to all three planes in place.
pub fn loop_filter_frame(planes: &mut [super::recon::PlaneBuf; 3], input: &LfInput<'_>) {
    for plane in 0..input.num_planes {
        if plane == 0 || input.hdr.lf.level[1 + plane] != 0 {
            for pass in 0..2 {
                let row_step = if plane == 0 {
                    1
                } else {
                    1 << usize::from(input.sub_y)
                };
                let col_step = if plane == 0 {
                    1
                } else {
                    1 << usize::from(input.sub_x)
                };
                let mut row = 0;
                while row < input.mi_rows {
                    let mut col = 0;
                    while col < input.mi_cols {
                        loop_filter_edge(planes, input, plane, pass, row, col);
                        col += col_step;
                    }
                    row += row_step;
                }
            }
        }
    }
}

#[inline]
fn grid(input: &LfInput<'_>, r: usize, c: usize) -> usize {
    r * input.mi_cols + c
}

/// `loop_filter_edge` / edge loop filter process (spec 7.14.2).
fn loop_filter_edge(
    planes: &mut [super::recon::PlaneBuf; 3],
    input: &LfInput<'_>,
    plane: usize,
    pass: usize,
    row: usize,
    col: usize,
) {
    let (sub_x, sub_y) = if plane == 0 {
        (0usize, 0usize)
    } else {
        (usize::from(input.sub_x), usize::from(input.sub_y))
    };
    let (dx, dy) = if pass == 0 { (1usize, 0usize) } else { (0, 1) };
    let x = col * 4;
    let y = row * 4;
    let row = row | sub_y;
    let col = col | sub_x;

    // onScreen determination (frame dimensions, not MI-aligned).
    let on_screen = x < input.hdr.frame_width as usize
        && y < input.hdr.frame_height as usize
        && !(pass == 0 && x == 0)
        && !(pass == 1 && y == 0);
    if !on_screen {
        return;
    }

    let xp = x >> sub_x;
    let yp = y >> sub_y;
    let prev_row = row - (dy << sub_y);
    let prev_col = col - (dx << sub_x);

    let mi_size = usize::from(input.mi_sizes[grid(input, row, col)]);
    let tx_sz = usize::from(input.lf_tx_sizes[plane][grid(input, row >> sub_y, col >> sub_x)]);
    let plane_size = usize::from(
        super::tables_conv::SUBSAMPLED_SIZE[mi_size][if plane > 0 { sub_x } else { 0 }]
            [if plane > 0 { sub_y } else { 0 }],
    );
    let skip = input.skips[grid(input, row, col)] != 0;
    // isIntra == 1 always on intra frames.
    let prev_tx_sz =
        usize::from(input.lf_tx_sizes[plane][grid(input, prev_row >> sub_y, prev_col >> sub_x)]);

    // The spec's applyFilter condition is
    //   isTxEdge && (isBlockEdge || skip == 0 || isIntra == 1).
    // On intra frames the isIntra term is always 1, so applyFilter
    // degenerates to isTxEdge and the isBlockEdge/skip terms need not be
    // evaluated (plane_size and the skip grid feed only that condition).
    let _ = (plane_size, skip);
    let is_tx_edge = if pass == 0 {
        xp % usize::from(TX_WIDTH[tx_sz]) == 0
    } else {
        yp % usize::from(TX_HEIGHT[tx_sz]) == 0
    };
    if !is_tx_edge {
        return;
    }

    // Filter size process (spec 7.14.3).
    let base_size = if pass == 0 {
        core::cmp::min(
            usize::from(TX_WIDTH[prev_tx_sz]),
            usize::from(TX_WIDTH[tx_sz]),
        )
    } else {
        core::cmp::min(
            usize::from(TX_HEIGHT[prev_tx_sz]),
            usize::from(TX_HEIGHT[tx_sz]),
        )
    };
    let filter_size = if plane == 0 {
        core::cmp::min(16, base_size)
    } else {
        core::cmp::min(8, base_size)
    };

    // Adaptive filter strength (spec 7.14.4), falling back to the previous
    // block when lvl == 0.
    let (mut lvl, mut limit, mut blimit, mut thresh) =
        adaptive_filter_strength(input, row, col, plane, pass);
    if lvl == 0 {
        let (l2, li2, b2, t2) = adaptive_filter_strength(input, prev_row, prev_col, plane, pass);
        lvl = l2;
        limit = li2;
        blimit = b2;
        thresh = t2;
    }

    if lvl > 0 {
        let p = &mut planes[plane];
        for i in 0..4usize {
            let sx = xp + dy * i;
            let sy = yp + dx * i;
            sample_filtering(p, sx, sy, limit, blimit, thresh, dx, dy, filter_size, plane);
        }
    }
}

/// Adaptive filter strength process (spec 7.14.4) + selection (7.14.5).
fn adaptive_filter_strength(
    input: &LfInput<'_>,
    row: usize,
    col: usize,
    plane: usize,
    pass: usize,
) -> (i32, i32, i32, i32) {
    let segment = usize::from(input.seg_ids[grid(input, row, col)]);
    // ref == INTRA_FRAME (0), modeType == 0 on intra frames.
    let i = if plane == 0 { pass } else { plane + 1 };
    let delta_lf = if input.hdr.delta_lf_multi {
        i32::from(input.delta_lfs[grid(input, row, col)][i])
    } else {
        i32::from(input.delta_lfs[grid(input, row, col)][0])
    };

    // Selection process.
    let base_filter_level =
        (delta_lf + input.hdr.lf.level[i] as i32).clamp(0, MAX_LOOP_FILTER as i32);
    let mut lvl_seg = base_filter_level;
    let feature = SEG_LVL_ALT_LF_Y_V + i;
    debug_assert!(feature < SEG_LVL_MAX);
    if input.hdr.seg.enabled && input.hdr.seg.feature_enabled[segment][feature] {
        lvl_seg = (input.hdr.seg.feature_data[segment][feature] + lvl_seg)
            .clamp(0, MAX_LOOP_FILTER as i32);
    }
    if input.hdr.lf.delta_enabled {
        // ref == INTRA_FRAME on intra frames.
        let n_shift = lvl_seg >> 5;
        lvl_seg += input.hdr.lf.ref_deltas[0] << n_shift;
        lvl_seg = lvl_seg.clamp(0, MAX_LOOP_FILTER as i32);
    }
    let lvl = lvl_seg;

    let sharpness = input.hdr.lf.sharpness as i32;
    let shift = if sharpness > 4 {
        2
    } else if sharpness > 0 {
        1
    } else {
        0
    };
    let limit = if sharpness > 0 {
        (lvl >> shift).clamp(1, 9 - sharpness)
    } else {
        core::cmp::max(1, lvl >> shift)
    };
    let blimit = 2 * (lvl + 2) + limit;
    let thresh = lvl >> 4;
    (lvl, limit, blimit, thresh)
}

/// Sample filtering process (spec 7.14.6.1).
#[allow(clippy::too_many_arguments)]
fn sample_filtering(
    p: &mut super::recon::PlaneBuf,
    x: usize,
    y: usize,
    limit: i32,
    blimit: i32,
    thresh: i32,
    dx: usize,
    dy: usize,
    filter_size: usize,
    plane: usize,
) {
    let stride = p.stride;
    let at = |px: i32, py: i32| -> i32 { i32::from(p.data[(py as usize) * stride + px as usize]) };
    let xi = x as i32;
    let yi = y as i32;
    let dxi = dx as i32;
    let dyi = dy as i32;

    // Filter mask process (spec 7.14.6.2), BitDepth = 8.
    let q = |k: i32| at(xi + dxi * k, yi + dyi * k);
    let pn = |k: i32| at(xi - dxi * (k + 1), yi - dyi * (k + 1));
    let q0 = q(0);
    let q1 = q(1);
    let q2 = q(2);
    let q3 = q(3);
    let p0 = pn(0);
    let p1 = pn(1);
    let p2 = pn(2);
    let p3 = pn(3);

    let mut hev_mask = false;
    hev_mask |= (p1 - p0).abs() > thresh;
    hev_mask |= (q1 - q0).abs() > thresh;

    let filter_len = if filter_size == 4 {
        4
    } else if plane != 0 {
        6
    } else if filter_size == 8 {
        8
    } else {
        16
    };

    let mut mask = false;
    mask |= (p1 - p0).abs() > limit;
    mask |= (q1 - q0).abs() > limit;
    mask |= (p0 - q0).abs() * 2 + (p1 - q1).abs() / 2 > blimit;
    if filter_len >= 6 {
        mask |= (p2 - p1).abs() > limit;
        mask |= (q2 - q1).abs() > limit;
    }
    if filter_len >= 8 {
        mask |= (p3 - p2).abs() > limit;
        mask |= (q3 - q2).abs() > limit;
    }
    let filter_mask = !mask;
    if !filter_mask {
        return;
    }

    let threshold_bd = 1i32;
    let flat_mask = if filter_size >= 8 {
        let mut m = false;
        m |= (p1 - p0).abs() > threshold_bd;
        m |= (q1 - q0).abs() > threshold_bd;
        m |= (p2 - p0).abs() > threshold_bd;
        m |= (q2 - q0).abs() > threshold_bd;
        if filter_len >= 8 {
            m |= (p3 - p0).abs() > threshold_bd;
            m |= (q3 - q0).abs() > threshold_bd;
        }
        !m
    } else {
        false
    };
    let flat_mask2 = if filter_size >= 16 {
        let q4 = q(4);
        let q5 = q(5);
        let q6 = q(6);
        let p4 = pn(4);
        let p5 = pn(5);
        let p6 = pn(6);
        let mut m = false;
        m |= (p6 - p0).abs() > threshold_bd;
        m |= (q6 - q0).abs() > threshold_bd;
        m |= (p5 - p0).abs() > threshold_bd;
        m |= (q5 - q0).abs() > threshold_bd;
        m |= (p4 - p0).abs() > threshold_bd;
        m |= (q4 - q0).abs() > threshold_bd;
        !m
    } else {
        false
    };

    if filter_size == 4 || !flat_mask {
        narrow_filter(p, xi, yi, dxi, dyi, hev_mask);
    } else if filter_size == 8 || !flat_mask2 {
        wide_filter(p, xi, yi, dxi, dyi, 3, plane);
    } else {
        wide_filter(p, xi, yi, dxi, dyi, 4, plane);
    }
}

/// Narrow filter process (spec 7.14.6.3), BitDepth = 8.
fn narrow_filter(p: &mut super::recon::PlaneBuf, x: i32, y: i32, dx: i32, dy: i32, hev: bool) {
    let stride = p.stride;
    let at = |px: i32, py: i32| -> i32 { i32::from(p.data[(py as usize) * stride + px as usize]) };
    let put = |p: &mut super::recon::PlaneBuf, px: i32, py: i32, v: i32| {
        p.data[(py as usize) * stride + px as usize] = v as u8;
    };
    #[inline]
    fn clamp4(v: i32) -> i32 {
        v.clamp(-128, 127)
    }
    let q0 = at(x, y);
    let q1 = at(x + dx, y + dy);
    let p0 = at(x - dx, y - dy);
    let p1 = at(x - dx * 2, y - dy * 2);
    let ps1 = p1 - 0x80;
    let ps0 = p0 - 0x80;
    let qs0 = q0 - 0x80;
    let qs1 = q1 - 0x80;
    let mut filter = if hev { clamp4(ps1 - qs1) } else { 0 };
    filter = clamp4(filter + 3 * (qs0 - ps0));
    let filter1 = clamp4(filter + 4) >> 3;
    let filter2 = clamp4(filter + 3) >> 3;
    let oq0 = clamp4(qs0 - filter1) + 0x80;
    let op0 = clamp4(ps0 + filter2) + 0x80;
    put(p, x, y, oq0);
    put(p, x - dx, y - dy, op0);
    if !hev {
        let f = (filter1 + 1) >> 1;
        let oq1 = clamp4(qs1 - f) + 0x80;
        let op1 = clamp4(ps1 + f) + 0x80;
        put(p, x + dx, y + dy, oq1);
        put(p, x - dx * 2, y - dy * 2, op1);
    }
}

/// Wide filter process (spec 7.14.6.4).
fn wide_filter(
    p: &mut super::recon::PlaneBuf,
    x: i32,
    y: i32,
    dx: i32,
    dy: i32,
    log2_size: u32,
    plane: usize,
) {
    let n: i32 = if log2_size == 4 {
        6
    } else if plane == 0 {
        3
    } else {
        2
    };
    let n2: i32 = if log2_size == 3 && plane == 0 { 0 } else { 1 };
    let stride = p.stride;
    let at = |px: i32, py: i32| -> i32 { i32::from(p.data[(py as usize) * stride + px as usize]) };
    let mut f = [0i32; 12];
    for i in -n..n {
        let mut t = 0i32;
        for j in -n..=n {
            let pos = (i + j).clamp(-(n + 1), n);
            let tap = if j.abs() <= n2 { 2 } else { 1 };
            t += at(x + pos * dx, y + pos * dy) * tap;
        }
        f[(i + n) as usize] = (t + (1 << (log2_size - 1))) >> log2_size;
    }
    for i in -n..n {
        let v = f[(i + n) as usize];
        p.data[((y + i * dy) as usize) * stride + (x + i * dx) as usize] = v as u8;
    }
}
