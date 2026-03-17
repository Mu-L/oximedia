//! Video deinterlacing algorithms.
//!
//! Supports Weave, Bob (double-rate with linear interpolation), Blend,
//! Yadif spatial-temporal, and adaptive edge-directed deinterlacing on
//! grayscale (1 byte per pixel) frames.

/// Algorithm used to reconstruct a progressive frame from interlaced fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeinterlaceMethod {
    /// Combine fields as-is (simple weave).
    Weave,
    /// Double-rate: upsample each field with linear interpolation to full height.
    Bob,
    /// Blend top and bottom fields.
    Blend,
    /// Spatial-temporal filter (YADIF-inspired) with edge-directed interpolation
    /// and proper temporal consistency checks.
    Yadif,
    /// Linear-interpolation Bob: interpolates missing lines from adjacent kept lines.
    /// Unlike standard Bob, it produces a single output frame.
    LinearBob,
    /// Motion-adaptive deinterlacing: uses weave in static areas and bob in
    /// areas with detected inter-field motion.
    MotionAdaptive,
}

/// Field dominance / order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldOrder {
    /// Top (even) field is displayed first.
    TopFieldFirst,
    /// Bottom (odd) field is displayed first.
    BottomFieldFirst,
}

/// Configuration for a deinterlacer.
pub struct Deinterlacer {
    /// Deinterlacing algorithm.
    pub method: DeinterlaceMethod,
    /// Field order / dominance.
    pub field_order: FieldOrder,
    /// If `true`, Bob mode outputs two frames per input frame (2× rate).
    pub output_double_rate: bool,
}

impl Deinterlacer {
    /// Create a new `Deinterlacer`.  `output_double_rate` is automatically
    /// set to `true` when `method` is `Bob`.
    pub fn new(method: DeinterlaceMethod, field_order: FieldOrder) -> Self {
        let output_double_rate = matches!(method, DeinterlaceMethod::Bob);
        Self {
            method,
            field_order,
            output_double_rate,
        }
    }

    /// Deinterlace a single frame, optionally using temporal neighbours.
    ///
    /// Returns a `Vec` of one or two progressive frames (two only for Bob
    /// with `output_double_rate`).
    ///
    /// All frames are grayscale: `width × height` bytes.
    pub fn deinterlace_frame(
        &self,
        prev: Option<&[u8]>,
        cur: &[u8],
        next: Option<&[u8]>,
        width: u32,
        height: u32,
    ) -> Vec<Vec<u8>> {
        let keep_even = matches!(self.field_order, FieldOrder::TopFieldFirst);
        match self.method {
            DeinterlaceMethod::Weave => vec![weave(cur)],
            DeinterlaceMethod::Bob => bob(cur, width, height),
            DeinterlaceMethod::Blend => vec![blend_fields(cur, width, height)],
            DeinterlaceMethod::Yadif => {
                vec![yadif_adaptive(prev, cur, next, width, height, keep_even)]
            }
            DeinterlaceMethod::LinearBob => vec![linear_bob(cur, width, height, keep_even)],
            DeinterlaceMethod::MotionAdaptive => {
                vec![motion_adaptive(prev, cur, next, width, height, keep_even)]
            }
        }
    }

    /// Process a full sequence of interlaced frames.
    pub fn process_sequence(&self, frames: &[Vec<u8>], width: u32, height: u32) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        for (i, cur) in frames.iter().enumerate() {
            let prev = if i > 0 {
                Some(frames[i - 1].as_slice())
            } else {
                None
            };
            let next = frames.get(i + 1).map(|f| f.as_slice());
            let deinterlaced = self.deinterlace_frame(prev, cur, next, width, height);
            out.extend(deinterlaced);
        }
        out
    }
}

/// Separate a frame into its top (even) and bottom (odd) fields.
///
/// Returns `(top_field, bottom_field)`.  Each field is `width × (height/2)`
/// bytes.  If `height` is odd the last row is ignored.
pub fn split_fields(frame: &[u8], width: u32, height: u32) -> (Vec<u8>, Vec<u8>) {
    let w = width as usize;
    let h = (height as usize) & !1; // round down to even
    let field_height = h / 2;

    let mut top = Vec::with_capacity(w * field_height);
    let mut bottom = Vec::with_capacity(w * field_height);

    for row in 0..h {
        let start = row * w;
        let end = start + w;
        let slice = frame.get(start..end).unwrap_or(&[]);
        if row % 2 == 0 {
            top.extend_from_slice(slice);
        } else {
            bottom.extend_from_slice(slice);
        }
    }

    (top, bottom)
}

// -----------------------------------------------------------------------
// Algorithm implementations
// -----------------------------------------------------------------------

/// Weave: return the frame as-is (already interleaved).
fn weave(cur: &[u8]) -> Vec<u8> {
    cur.to_vec()
}

/// Bob: upsample each field to full height by line doubling.
/// Returns `[top_frame, bottom_frame]`.
fn bob(cur: &[u8], width: u32, height: u32) -> Vec<Vec<u8>> {
    let w = width as usize;
    let h = height as usize;
    let mut top_out = Vec::with_capacity(w * h);
    let mut bot_out = Vec::with_capacity(w * h);

    for row in 0..h {
        // Top field comes from even rows; bottom from odd rows.
        let top_src_row = (row / 2) * 2; // nearest even row
        let bot_src_row = (row / 2) * 2 + 1; // nearest odd row, clamped below

        let top_src = {
            let r = top_src_row.min(h.saturating_sub(1));
            let start = r * w;
            cur.get(start..start + w).unwrap_or(&[])
        };
        let bot_src = {
            let r = bot_src_row.min(h.saturating_sub(1));
            let start = r * w;
            cur.get(start..start + w).unwrap_or(&[])
        };

        top_out.extend_from_slice(top_src);
        bot_out.extend_from_slice(bot_src);
    }

    vec![top_out, bot_out]
}

/// Blend: average adjacent field lines.
fn blend_fields(cur: &[u8], width: u32, height: u32) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut out = Vec::with_capacity(w * h);

    for row in 0..h {
        let other_row = if row % 2 == 0 {
            (row + 1).min(h - 1)
        } else {
            row.saturating_sub(1)
        };

        for col in 0..w {
            let a = cur.get(row * w + col).copied().unwrap_or(0);
            let b = cur.get(other_row * w + col).copied().unwrap_or(0);
            out.push(((a as u16 + b as u16) / 2) as u8);
        }
    }

    out
}

/// Yadif-inspired spatial-temporal deinterlacing (legacy, keeps even rows).
#[allow(dead_code)]
fn yadif(prev: Option<&[u8]>, cur: &[u8], next: Option<&[u8]>, width: u32, height: u32) -> Vec<u8> {
    yadif_adaptive(prev, cur, next, width, height, true)
}

/// Full Yadif-style adaptive deinterlacing with edge-directed spatial interpolation,
/// temporal consistency checking, and spatial-temporal score selection.
///
/// `keep_even`: if true, even rows (top field) are kept; otherwise odd rows.
fn yadif_adaptive(
    prev: Option<&[u8]>,
    cur: &[u8],
    next: Option<&[u8]>,
    width: u32,
    height: u32,
    keep_even: bool,
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut out = vec![0u8; w * h];

    for row in 0..h {
        let is_kept_row = if keep_even {
            row % 2 == 0
        } else {
            row % 2 == 1
        };

        for col in 0..w {
            let idx = row * w + col;

            if is_kept_row {
                out[idx] = cur.get(idx).copied().unwrap_or(0);
            } else {
                // -- Spatial prediction with edge-direction detection --
                let above = if row > 0 {
                    cur.get((row - 1) * w + col).copied().unwrap_or(0) as i32
                } else {
                    cur.get(idx).copied().unwrap_or(0) as i32
                };
                let below = if row + 1 < h {
                    cur.get((row + 1) * w + col).copied().unwrap_or(0) as i32
                } else {
                    cur.get(idx).copied().unwrap_or(0) as i32
                };

                // Edge-directed: check diagonal neighbours for edge orientation
                let spatial_pred = yadif_spatial_pred(cur, w, h, row, col, above, below);

                // -- Temporal prediction --
                // Yadif uses temporal diff to compute min/max bounds
                let cur_val = cur.get(idx).copied().unwrap_or(0) as i32;
                let (temporal_pred, temporal_diff) = yadif_temporal(prev, next, idx, cur_val);

                // Spatial diff
                let spatial_diff = (above - below).unsigned_abs() as i32;

                // Choose: if spatial is smooth (small diff), use spatial pred.
                // If temporal evidence is strong (large motion), use temporal pred
                // with spatial clamping.
                let score = spatial_diff * 2 - temporal_diff;
                let result = if score < 10 {
                    spatial_pred
                } else {
                    // Clamp temporal prediction to spatial bounds
                    let lo = above.min(below);
                    let hi = above.max(below);
                    temporal_pred.clamp(lo, hi)
                };

                out[idx] = result.clamp(0, 255) as u8;
            }
        }
    }

    out
}

/// Yadif spatial prediction with edge-direction detection.
///
/// Checks three diagonal directions and picks the one with minimum
/// gradient, falling back to vertical average.
fn yadif_spatial_pred(
    cur: &[u8],
    w: usize,
    h: usize,
    row: usize,
    col: usize,
    above: i32,
    below: i32,
) -> i32 {
    let vertical = (above + below + 1) / 2;

    // Check diagonals: -1, 0, +1 pixel offset
    let mut best_pred = vertical;
    let mut best_score = (above - below).unsigned_abs();

    for &diag in &[-1i32, 1] {
        let dcol = (col as i32 + diag).clamp(0, w as i32 - 1) as usize;
        let dcol_neg = (col as i32 - diag).clamp(0, w as i32 - 1) as usize;

        let above_diag = if row > 0 {
            cur.get((row - 1) * w + dcol).copied().unwrap_or(0) as i32
        } else {
            above
        };
        let below_diag = if row + 1 < h {
            cur.get((row + 1) * w + dcol_neg).copied().unwrap_or(0) as i32
        } else {
            below
        };

        let score = (above_diag - below_diag).unsigned_abs();
        if score < best_score {
            best_score = score;
            best_pred = (above_diag + below_diag + 1) / 2;
        }
    }

    best_pred
}

/// Yadif temporal prediction: average of prev and next at same position,
/// plus the temporal diff magnitude.
fn yadif_temporal(
    prev: Option<&[u8]>,
    next: Option<&[u8]>,
    idx: usize,
    cur_val: i32,
) -> (i32, i32) {
    let pv = prev
        .and_then(|p| p.get(idx).copied())
        .unwrap_or(cur_val as u8) as i32;
    let nv = next
        .and_then(|n| n.get(idx).copied())
        .unwrap_or(cur_val as u8) as i32;
    let temporal_pred = (pv + nv + 1) / 2;
    let temporal_diff = ((pv - cur_val).abs() + (nv - cur_val).abs()) / 2;
    (temporal_pred, temporal_diff)
}

/// Linear-interpolation Bob: keeps one field and interpolates missing lines
/// using linear interpolation from adjacent kept lines. Single output frame.
fn linear_bob(cur: &[u8], width: u32, height: u32, keep_even: bool) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut out = vec![0u8; w * h];

    for row in 0..h {
        let is_kept = if keep_even {
            row % 2 == 0
        } else {
            row % 2 == 1
        };

        if is_kept {
            // Copy the kept field line directly
            let start = row * w;
            for col in 0..w {
                out[start + col] = cur.get(start + col).copied().unwrap_or(0);
            }
        } else {
            // Interpolate from the two nearest kept lines
            // The kept lines are 1 row above and 1 row below (both same parity)
            let above_row = if row > 0 { row - 1 } else { 0 };
            let below_row = if row + 1 < h {
                row + 1
            } else {
                h.saturating_sub(1)
            };

            for col in 0..w {
                let a = cur.get(above_row * w + col).copied().unwrap_or(0) as u16;
                let b = cur.get(below_row * w + col).copied().unwrap_or(0) as u16;
                out[row * w + col] = ((a + b + 1) / 2) as u8;
            }
        }
    }

    out
}

/// Motion-adaptive deinterlacing: detect inter-field motion per pixel and
/// use weave (direct copy) for static areas and bob (interpolation) for
/// areas with detected motion.
fn motion_adaptive(
    prev: Option<&[u8]>,
    cur: &[u8],
    _next: Option<&[u8]>,
    width: u32,
    height: u32,
    keep_even: bool,
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut out = vec![0u8; w * h];

    // Motion threshold: if difference between current and previous frame
    // at the same position exceeds this, we consider it "moving".
    const MOTION_THRESHOLD: u8 = 15;

    for row in 0..h {
        let is_kept = if keep_even {
            row % 2 == 0
        } else {
            row % 2 == 1
        };

        if is_kept {
            let start = row * w;
            for col in 0..w {
                out[start + col] = cur.get(start + col).copied().unwrap_or(0);
            }
        } else {
            for col in 0..w {
                let idx = row * w + col;
                let cur_val = cur.get(idx).copied().unwrap_or(0);

                // Detect motion using previous frame
                let has_motion = if let Some(p) = prev {
                    let prev_val = p.get(idx).copied().unwrap_or(0);
                    (cur_val as i16 - prev_val as i16).unsigned_abs() as u8 > MOTION_THRESHOLD
                } else {
                    true // No prev frame, assume motion (use bob)
                };

                if has_motion {
                    // Bob: interpolate from adjacent kept lines
                    let above_row = if row > 0 { row - 1 } else { 0 };
                    let below_row = if row + 1 < h {
                        row + 1
                    } else {
                        h.saturating_sub(1)
                    };
                    let a = cur.get(above_row * w + col).copied().unwrap_or(0) as u16;
                    let b = cur.get(below_row * w + col).copied().unwrap_or(0) as u16;
                    out[idx] = ((a + b + 1) / 2) as u8;
                } else {
                    // Weave: use the current interlaced line directly
                    out[idx] = cur_val;
                }
            }
        }
    }

    out
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(width: usize, height: usize, fill: u8) -> Vec<u8> {
        vec![fill; width * height]
    }

    /// Build a test frame where each row has a constant value equal to its row index.
    fn row_indexed_frame(width: usize, height: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(width * height);
        for row in 0..height {
            for _ in 0..width {
                v.push(row as u8);
            }
        }
        v
    }

    // 1. Deinterlacer::new sets fields correctly (Blend)
    #[test]
    fn test_new_blend_sets_fields() {
        let d = Deinterlacer::new(DeinterlaceMethod::Blend, FieldOrder::TopFieldFirst);
        assert_eq!(d.method, DeinterlaceMethod::Blend);
        assert_eq!(d.field_order, FieldOrder::TopFieldFirst);
        assert!(!d.output_double_rate);
    }

    // 2. Bob method sets output_double_rate=true
    #[test]
    fn test_bob_sets_double_rate() {
        let d = Deinterlacer::new(DeinterlaceMethod::Bob, FieldOrder::TopFieldFirst);
        assert!(d.output_double_rate);
    }

    // 3. Weave method sets output_double_rate=false
    #[test]
    fn test_weave_sets_single_rate() {
        let d = Deinterlacer::new(DeinterlaceMethod::Weave, FieldOrder::TopFieldFirst);
        assert!(!d.output_double_rate);
    }

    // 4. split_fields returns correct top field (even rows)
    #[test]
    fn test_split_fields_top() {
        let width = 4usize;
        let height = 4usize;
        let frame = row_indexed_frame(width, height);
        let (top, _) = split_fields(&frame, width as u32, height as u32);
        // Top field: rows 0 and 2
        assert_eq!(top.len(), width * (height / 2));
        // First 4 bytes = row 0 = all 0
        assert_eq!(&top[0..4], &[0u8, 0, 0, 0]);
        // Next 4 bytes = row 2 = all 2
        assert_eq!(&top[4..8], &[2u8, 2, 2, 2]);
    }

    // 5. split_fields returns correct bottom field (odd rows)
    #[test]
    fn test_split_fields_bottom() {
        let width = 4usize;
        let height = 4usize;
        let frame = row_indexed_frame(width, height);
        let (_, bottom) = split_fields(&frame, width as u32, height as u32);
        // Bottom field: rows 1 and 3
        assert_eq!(bottom.len(), width * (height / 2));
        assert_eq!(&bottom[0..4], &[1u8, 1, 1, 1]);
        assert_eq!(&bottom[4..8], &[3u8, 3, 3, 3]);
    }

    // 6. deinterlace_frame Weave returns 1 frame
    #[test]
    fn test_weave_returns_one_frame() {
        let d = Deinterlacer::new(DeinterlaceMethod::Weave, FieldOrder::TopFieldFirst);
        let frame = make_frame(8, 8, 100);
        let out = d.deinterlace_frame(None, &frame, None, 8, 8);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], frame);
    }

    // 7. deinterlace_frame Bob returns 2 frames
    #[test]
    fn test_bob_returns_two_frames() {
        let d = Deinterlacer::new(DeinterlaceMethod::Bob, FieldOrder::TopFieldFirst);
        let frame = make_frame(8, 8, 50);
        let out = d.deinterlace_frame(None, &frame, None, 8, 8);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].len(), 8 * 8);
        assert_eq!(out[1].len(), 8 * 8);
    }

    // 8. deinterlace_frame Blend returns 1 frame
    #[test]
    fn test_blend_returns_one_frame() {
        let d = Deinterlacer::new(DeinterlaceMethod::Blend, FieldOrder::TopFieldFirst);
        let frame = make_frame(8, 8, 80);
        let out = d.deinterlace_frame(None, &frame, None, 8, 8);
        assert_eq!(out.len(), 1);
    }

    // 9. deinterlace_frame Yadif returns 1 frame
    #[test]
    fn test_yadif_returns_one_frame() {
        let d = Deinterlacer::new(DeinterlaceMethod::Yadif, FieldOrder::TopFieldFirst);
        let frame = make_frame(8, 8, 120);
        let out = d.deinterlace_frame(None, &frame, None, 8, 8);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), 8 * 8);
    }

    // 10. Blend output has correct averaged values
    #[test]
    fn test_blend_averaged_values() {
        let width = 4u32;
        let height = 4u32;
        // Even rows = 100, odd rows = 200
        let mut frame = vec![0u8; 16];
        for row in 0..4usize {
            let fill = if row % 2 == 0 { 100u8 } else { 200u8 };
            for col in 0..4usize {
                frame[row * 4 + col] = fill;
            }
        }
        let d = Deinterlacer::new(DeinterlaceMethod::Blend, FieldOrder::TopFieldFirst);
        let out = d.deinterlace_frame(None, &frame, None, width, height);
        // Row 0 (even): blended with row 1 → (100+200)/2 = 150
        for col in 0..4usize {
            assert_eq!(out[0][col], 150u8);
        }
        // Row 1 (odd): blended with row 0 → (200+100)/2 = 150
        for col in 0..4usize {
            assert_eq!(out[0][4 + col], 150u8);
        }
    }

    // 11. process_sequence Bob doubles frame count
    #[test]
    fn test_process_sequence_bob_doubles_frames() {
        let d = Deinterlacer::new(DeinterlaceMethod::Bob, FieldOrder::TopFieldFirst);
        let frames: Vec<Vec<u8>> = (0..4).map(|_| make_frame(8, 8, 60)).collect();
        let out = d.process_sequence(&frames, 8, 8);
        assert_eq!(out.len(), frames.len() * 2);
    }

    // 12. process_sequence Weave preserves frame count
    #[test]
    fn test_process_sequence_weave_preserves_count() {
        let d = Deinterlacer::new(DeinterlaceMethod::Weave, FieldOrder::TopFieldFirst);
        let frames: Vec<Vec<u8>> = (0..5).map(|_| make_frame(8, 8, 90)).collect();
        let out = d.process_sequence(&frames, 8, 8);
        assert_eq!(out.len(), frames.len());
    }

    // ---- Enhanced deinterlace algorithm tests ----

    // 13. LinearBob returns 1 frame
    #[test]
    fn test_linear_bob_returns_one_frame() {
        let d = Deinterlacer::new(DeinterlaceMethod::LinearBob, FieldOrder::TopFieldFirst);
        let frame = make_frame(8, 8, 100);
        let out = d.deinterlace_frame(None, &frame, None, 8, 8);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), 8 * 8);
    }

    // 14. LinearBob interpolates missing lines correctly
    #[test]
    fn test_linear_bob_interpolation() {
        let width = 4u32;
        let height = 4u32;
        // Even rows = 100, odd rows = 200
        let mut frame = vec![0u8; 16];
        for row in 0..4usize {
            let fill = if row % 2 == 0 { 100u8 } else { 200u8 };
            for col in 0..4usize {
                frame[row * 4 + col] = fill;
            }
        }
        let d = Deinterlacer::new(DeinterlaceMethod::LinearBob, FieldOrder::TopFieldFirst);
        let out = d.deinterlace_frame(None, &frame, None, width, height);
        // Row 0 (kept): 100
        assert_eq!(out[0][0], 100);
        // Row 1 (interpolated from row 0=100, row 2=100): 100
        assert_eq!(out[0][4], 100);
        // Row 2 (kept): 100
        assert_eq!(out[0][8], 100);
    }

    // 15. MotionAdaptive returns 1 frame
    #[test]
    fn test_motion_adaptive_returns_one_frame() {
        let d = Deinterlacer::new(DeinterlaceMethod::MotionAdaptive, FieldOrder::TopFieldFirst);
        let frame = make_frame(8, 8, 80);
        let out = d.deinterlace_frame(None, &frame, None, 8, 8);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), 8 * 8);
    }

    // 16. MotionAdaptive uses weave for static content (with prev frame)
    #[test]
    fn test_motion_adaptive_static_weaves() {
        let d = Deinterlacer::new(DeinterlaceMethod::MotionAdaptive, FieldOrder::TopFieldFirst);
        let frame = make_frame(4, 4, 128);
        // prev = same as cur => no motion => weave
        let out = d.deinterlace_frame(Some(&frame), &frame, None, 4, 4);
        // All pixels should remain 128 (weave copies directly)
        for &px in &out[0] {
            assert_eq!(px, 128);
        }
    }

    // 17. MotionAdaptive uses bob for moving content
    #[test]
    fn test_motion_adaptive_moving_bobs() {
        let d = Deinterlacer::new(DeinterlaceMethod::MotionAdaptive, FieldOrder::TopFieldFirst);
        let width = 4usize;
        let height = 4usize;
        let prev_frame = make_frame(width, height, 50);
        // Current frame is very different from prev => motion detected
        let cur_frame = make_frame(width, height, 200);
        let out = d.deinterlace_frame(Some(&prev_frame), &cur_frame, None, 4, 4);
        // Row 0 (even, kept): 200
        assert_eq!(out[0][0], 200);
        // Row 1 (odd, motion detected => bob interpolation from rows 0 and 2)
        // Both row 0 and row 2 are 200, so interpolated = 200
        assert_eq!(out[0][4], 200);
    }

    // 18. Yadif adaptive with field order respects keep_even=true
    #[test]
    fn test_yadif_adaptive_top_field_first() {
        let d = Deinterlacer::new(DeinterlaceMethod::Yadif, FieldOrder::TopFieldFirst);
        let frame = row_indexed_frame(4, 4);
        let out = d.deinterlace_frame(None, &frame, None, 4, 4);
        assert_eq!(out.len(), 1);
        // Even rows (0,2) should be preserved exactly
        assert_eq!(out[0][0], 0); // row 0
        assert_eq!(out[0][8], 2); // row 2
    }

    // 19. Yadif adaptive with BFF keeps odd rows
    #[test]
    fn test_yadif_adaptive_bottom_field_first() {
        let d = Deinterlacer::new(DeinterlaceMethod::Yadif, FieldOrder::BottomFieldFirst);
        let frame = row_indexed_frame(4, 4);
        let out = d.deinterlace_frame(None, &frame, None, 4, 4);
        assert_eq!(out.len(), 1);
        // Odd rows (1,3) should be preserved exactly
        assert_eq!(out[0][4], 1); // row 1
        assert_eq!(out[0][12], 3); // row 3
    }

    // 20. Yadif with temporal neighbours produces reasonable output
    #[test]
    fn test_yadif_with_temporal_neighbours() {
        let width = 8u32;
        let height = 8u32;
        let prev = make_frame(width as usize, height as usize, 100);
        let cur = make_frame(width as usize, height as usize, 100);
        let next = make_frame(width as usize, height as usize, 100);
        let d = Deinterlacer::new(DeinterlaceMethod::Yadif, FieldOrder::TopFieldFirst);
        let out = d.deinterlace_frame(Some(&prev), &cur, Some(&next), width, height);
        // Uniform frames: all pixels should be ~100
        for &px in &out[0] {
            assert!(
                (px as i16 - 100).unsigned_abs() <= 1,
                "pixel {} should be ~100",
                px
            );
        }
    }

    // 21. LinearBob with BFF keeps odd rows
    #[test]
    fn test_linear_bob_bottom_field_first() {
        let d = Deinterlacer::new(DeinterlaceMethod::LinearBob, FieldOrder::BottomFieldFirst);
        let frame = row_indexed_frame(4, 4);
        let out = d.deinterlace_frame(None, &frame, None, 4, 4);
        // Odd rows (1,3) should be kept
        assert_eq!(out[0][4], 1); // row 1
        assert_eq!(out[0][12], 3); // row 3
    }

    // 22. process_sequence with MotionAdaptive preserves frame count
    #[test]
    fn test_process_sequence_motion_adaptive() {
        let d = Deinterlacer::new(DeinterlaceMethod::MotionAdaptive, FieldOrder::TopFieldFirst);
        let frames: Vec<Vec<u8>> = (0..5).map(|_| make_frame(8, 8, 90)).collect();
        let out = d.process_sequence(&frames, 8, 8);
        assert_eq!(out.len(), 5);
    }

    // 23. DeinterlaceMethod enum variants are distinguishable
    #[test]
    fn test_deinterlace_method_variants() {
        assert_ne!(DeinterlaceMethod::Weave, DeinterlaceMethod::Bob);
        assert_ne!(DeinterlaceMethod::Yadif, DeinterlaceMethod::LinearBob);
        assert_ne!(DeinterlaceMethod::MotionAdaptive, DeinterlaceMethod::Blend);
    }
}
