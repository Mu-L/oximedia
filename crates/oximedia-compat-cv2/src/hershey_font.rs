//! Hershey-style stroke-based vector font renderer.
//!
//! Provides `put_text_hershey` which draws text using simplified stroke outlines
//! derived from the public-domain Hershey font data.  Digits `'0'`–`'9'` and
//! uppercase letters `'A'`–`'N'` are supported; all other characters advance the
//! cursor without drawing.
//!
//! The coordinate system places the baseline at `y = 12` within a 12×16 glyph
//! box.  The `org` argument to `put_text_hershey` is the *bottom-left* origin
//! of the first character, matching the OpenCV `putText` convention.

use crate::{
    drawing::line,
    error::{Cv2Error, Cv2Result},
    mat::{Mat, Point, Scalar},
};

// ── Glyph data ────────────────────────────────────────────────────────────────

/// A single stroke-font glyph: character advance width and a slice of strokes.
///
/// Each stroke is a sequence of `(x, y)` pairs to be connected with line
/// segments.  The glyph coordinate space has x in `0..=12` and y in `0..=12`,
/// with `y = 12` at the baseline.
struct HersheyGlyph {
    advance: i8,
    strokes: &'static [&'static [(i8, i8)]],
}

// ── Glyph stroke data ─────────────────────────────────────────────────────────

static GLYPH_0: &[&[(i8, i8)]] = &[&[
    (5, 0),
    (3, 0),
    (1, 2),
    (0, 5),
    (0, 7),
    (1, 10),
    (3, 12),
    (5, 12),
    (7, 12),
    (9, 10),
    (10, 7),
    (10, 5),
    (9, 2),
    (7, 0),
    (5, 0),
]];

static GLYPH_1: &[&[(i8, i8)]] = &[&[(2, 3), (4, 1), (4, 12)], &[(1, 12), (7, 12)]];

static GLYPH_2: &[&[(i8, i8)]] = &[&[
    (1, 3),
    (2, 1),
    (4, 0),
    (6, 0),
    (8, 1),
    (9, 3),
    (9, 4),
    (8, 6),
    (1, 11),
    (1, 12),
    (9, 12),
]];

static GLYPH_3: &[&[(i8, i8)]] = &[&[
    (1, 0),
    (9, 0),
    (5, 5),
    (7, 5),
    (9, 7),
    (9, 10),
    (8, 12),
    (5, 12),
    (3, 12),
    (1, 11),
]];

static GLYPH_4: &[&[(i8, i8)]] = &[&[(7, 0), (0, 8), (9, 8)], &[(7, 0), (7, 12)]];

static GLYPH_5: &[&[(i8, i8)]] = &[&[
    (8, 0),
    (1, 0),
    (1, 5),
    (5, 5),
    (8, 6),
    (9, 8),
    (9, 10),
    (7, 12),
    (4, 12),
    (2, 11),
    (1, 9),
]];

static GLYPH_6: &[&[(i8, i8)]] = &[&[
    (8, 1),
    (6, 0),
    (3, 0),
    (1, 3),
    (0, 6),
    (0, 9),
    (1, 11),
    (3, 12),
    (6, 12),
    (8, 11),
    (9, 9),
    (9, 8),
    (8, 6),
    (6, 5),
    (3, 5),
    (1, 6),
    (0, 8),
]];

static GLYPH_7: &[&[(i8, i8)]] = &[&[(0, 0), (9, 0), (3, 12)]];

static GLYPH_8: &[&[(i8, i8)]] = &[&[
    (4, 0),
    (2, 1),
    (1, 3),
    (1, 5),
    (2, 6),
    (5, 7),
    (7, 8),
    (8, 10),
    (8, 11),
    (6, 12),
    (3, 12),
    (1, 11),
    (1, 10),
    (2, 8),
    (5, 7),
    (7, 6),
    (8, 4),
    (8, 2),
    (7, 1),
    (5, 0),
    (4, 0),
]];

static GLYPH_9: &[&[(i8, i8)]] = &[&[
    (1, 11),
    (3, 12),
    (6, 12),
    (8, 11),
    (9, 9),
    (9, 6),
    (8, 4),
    (6, 3),
    (3, 3),
    (1, 4),
    (0, 6),
    (0, 7),
    (1, 9),
    (3, 10),
    (6, 10),
    (8, 9),
    (9, 7),
]];

static GLYPH_A: &[&[(i8, i8)]] = &[&[(0, 12), (4, 0), (8, 12)], &[(1, 8), (7, 8)]];

static GLYPH_B: &[&[(i8, i8)]] = &[
    &[(1, 0), (1, 12)],
    &[(1, 0), (6, 0), (8, 1), (8, 3), (7, 5), (5, 6), (1, 6)],
    &[(1, 6), (6, 6), (8, 7), (8, 10), (7, 11), (5, 12), (1, 12)],
];

static GLYPH_C: &[&[(i8, i8)]] = &[&[
    (8, 2),
    (6, 0),
    (4, 0),
    (2, 1),
    (0, 4),
    (0, 8),
    (2, 11),
    (4, 12),
    (6, 12),
    (8, 10),
]];

static GLYPH_D: &[&[(i8, i8)]] = &[
    &[(1, 0), (1, 12)],
    &[
        (1, 0),
        (5, 0),
        (7, 1),
        (9, 4),
        (9, 8),
        (7, 11),
        (5, 12),
        (1, 12),
    ],
];

static GLYPH_E: &[&[(i8, i8)]] = &[
    &[(1, 0), (1, 12)],
    &[(1, 0), (8, 0)],
    &[(1, 6), (6, 6)],
    &[(1, 12), (8, 12)],
];

static GLYPH_F: &[&[(i8, i8)]] = &[&[(1, 0), (1, 12)], &[(1, 0), (8, 0)], &[(1, 6), (6, 6)]];

static GLYPH_G: &[&[(i8, i8)]] = &[&[
    (8, 2),
    (6, 0),
    (4, 0),
    (2, 1),
    (0, 4),
    (0, 8),
    (2, 11),
    (4, 12),
    (6, 12),
    (8, 10),
    (8, 7),
    (5, 7),
]];

static GLYPH_H: &[&[(i8, i8)]] = &[&[(1, 0), (1, 12)], &[(8, 0), (8, 12)], &[(1, 6), (8, 6)]];

static GLYPH_I: &[&[(i8, i8)]] = &[&[(2, 0), (6, 0)], &[(4, 0), (4, 12)], &[(2, 12), (6, 12)]];

static GLYPH_J: &[&[(i8, i8)]] = &[
    &[(2, 0), (7, 0)],
    &[(5, 0), (5, 10), (4, 12), (2, 12), (1, 10)],
];

static GLYPH_K: &[&[(i8, i8)]] = &[&[(1, 0), (1, 12)], &[(8, 0), (1, 6)], &[(1, 6), (8, 12)]];

static GLYPH_L: &[&[(i8, i8)]] = &[&[(1, 0), (1, 12), (8, 12)]];

static GLYPH_M: &[&[(i8, i8)]] = &[&[(0, 12), (0, 0), (5, 8), (9, 0), (9, 12)]];

static GLYPH_N: &[&[(i8, i8)]] = &[&[(0, 12), (0, 0), (8, 12), (8, 0)]];

// ── Glyph lookup ──────────────────────────────────────────────────────────────

fn glyph_for_char(c: char) -> Option<HersheyGlyph> {
    let (advance, strokes): (i8, &'static [&'static [(i8, i8)]]) = match c {
        '0' => (11, GLYPH_0),
        '1' => (8, GLYPH_1),
        '2' => (11, GLYPH_2),
        '3' => (11, GLYPH_3),
        '4' => (11, GLYPH_4),
        '5' => (11, GLYPH_5),
        '6' => (11, GLYPH_6),
        '7' => (11, GLYPH_7),
        '8' => (11, GLYPH_8),
        '9' => (11, GLYPH_9),
        'A' => (11, GLYPH_A),
        'B' => (11, GLYPH_B),
        'C' => (11, GLYPH_C),
        'D' => (11, GLYPH_D),
        'E' => (11, GLYPH_E),
        'F' => (11, GLYPH_F),
        'G' => (11, GLYPH_G),
        'H' => (11, GLYPH_H),
        'I' => (9, GLYPH_I),
        'J' => (11, GLYPH_J),
        'K' => (11, GLYPH_K),
        'L' => (11, GLYPH_L),
        'M' => (11, GLYPH_M),
        'N' => (11, GLYPH_N),
        _ => return None,
    };
    Some(HersheyGlyph { advance, strokes })
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Render text using Hershey stroke vector outlines.
///
/// Only [`crate::constants::FONT_HERSHEY_SIMPLEX`] (value `0`) is currently
/// supported; other `font_face` values return
/// [`Cv2Error::UnsupportedFlag`].
///
/// `org` is the bottom-left origin of the text baseline (matching the OpenCV
/// `putText` convention).  Glyphs are scaled by `font_scale`; `thickness`
/// controls the line width of each stroke.  Digits `'0'`–`'9'` and uppercase
/// `'A'`–`'N'` produce visible strokes; all other characters advance the
/// cursor without drawing.
///
/// # Errors
/// Returns [`Cv2Error::UnsupportedFlag`] when `font_face` is not
/// `FONT_HERSHEY_SIMPLEX`.
pub fn put_text_hershey(
    img: &mut Mat,
    text: &str,
    org: Point,
    font_face: i32,
    font_scale: f64,
    color: Scalar,
    thickness: i32,
) -> Cv2Result<()> {
    if font_face != crate::constants::FONT_HERSHEY_SIMPLEX {
        return Err(Cv2Error::UnsupportedFlag {
            name: "put_text_hershey: unsupported font_face",
            value: font_face,
        });
    }

    let scale = font_scale;
    let mut x_offset = 0.0f64;

    for c in text.chars() {
        if c == ' ' {
            x_offset += 6.0 * scale;
            continue;
        }

        let Some(glyph) = glyph_for_char(c) else {
            // Unknown char: advance by a default width without drawing.
            x_offset += 11.0 * scale;
            continue;
        };

        for stroke in glyph.strokes {
            let n = stroke.len();
            if n < 2 {
                continue;
            }
            for i in 0..n - 1 {
                let (sx1, sy1) = stroke[i];
                let (sx2, sy2) = stroke[i + 1];
                // Transform glyph coordinates to image coordinates.
                // y=12 is the baseline in glyph space; org.y is the baseline
                // in image space, so glyph rows above the baseline map to
                // smaller image y values.
                let x1 = (org.x as f64 + x_offset + sx1 as f64 * scale) as i32;
                let y1 = (org.y as f64 - (12.0 - sy1 as f64) * scale) as i32;
                let x2 = (org.x as f64 + x_offset + sx2 as f64 * scale) as i32;
                let y2 = (org.y as f64 - (12.0 - sy2 as f64) * scale) as i32;
                line(
                    img,
                    Point { x: x1, y: y1 },
                    Point { x: x2, y: y2 },
                    color,
                    thickness,
                )?;
            }
        }

        x_offset += glyph.advance as f64 * scale;
    }

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        constants::FONT_HERSHEY_SIMPLEX,
        mat::{Mat, MatType, Point, Scalar},
    };

    fn blank(w: usize, h: usize) -> Mat {
        Mat::new(h, w, MatType::CV_8UC3)
    }

    fn nonzero(m: &Mat) -> usize {
        m.data.iter().filter(|&&v| v > 0).count()
    }

    #[test]
    fn test_digit_produces_pixels() {
        let mut img = blank(200, 60);
        put_text_hershey(
            &mut img,
            "0",
            Point { x: 10, y: 50 },
            FONT_HERSHEY_SIMPLEX,
            2.0,
            Scalar(255.0, 255.0, 255.0, 255.0),
            1,
        )
        .unwrap();
        assert!(
            nonzero(&img) > 0,
            "digit '0' should produce non-zero pixels"
        );
    }

    #[test]
    fn test_letter_produces_pixels() {
        let mut img = blank(200, 60);
        put_text_hershey(
            &mut img,
            "A",
            Point { x: 10, y: 50 },
            FONT_HERSHEY_SIMPLEX,
            2.0,
            Scalar(255.0, 255.0, 255.0, 255.0),
            1,
        )
        .unwrap();
        assert!(
            nonzero(&img) > 0,
            "letter 'A' should produce non-zero pixels"
        );
    }

    #[test]
    fn test_two_chars_more_pixels_than_one() {
        let mut img1 = blank(300, 60);
        let mut img2 = blank(300, 60);
        put_text_hershey(
            &mut img1,
            "A",
            Point { x: 0, y: 50 },
            FONT_HERSHEY_SIMPLEX,
            2.0,
            Scalar(255.0, 255.0, 255.0, 255.0),
            1,
        )
        .unwrap();
        put_text_hershey(
            &mut img2,
            "AN",
            Point { x: 0, y: 50 },
            FONT_HERSHEY_SIMPLEX,
            2.0,
            Scalar(255.0, 255.0, 255.0, 255.0),
            1,
        )
        .unwrap();
        assert!(
            nonzero(&img2) > nonzero(&img1),
            "two chars should produce more pixels than one"
        );
    }

    #[test]
    fn test_unsupported_font_errors() {
        let mut img = blank(100, 50);
        let res = put_text_hershey(
            &mut img,
            "A",
            Point { x: 0, y: 20 },
            99,
            1.0,
            Scalar(255.0, 255.0, 255.0, 255.0),
            1,
        );
        assert!(res.is_err());
    }
}
