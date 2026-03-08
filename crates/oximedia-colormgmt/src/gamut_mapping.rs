//! Gamut mapping algorithms for color space conversion.
//!
//! Provides methods for mapping colors from a source gamut to a destination gamut,
//! including clipping, compression, soft-clipping, and hue-preserving approaches.

#![allow(dead_code)]

/// Gamut mapping method selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamutMappingMethod {
    /// Hard clip: values outside gamut are clamped to boundary.
    Clip,
    /// Compress: reduce chroma to bring out-of-gamut colors inside.
    Compress,
    /// Soft clip: smooth roll-off near gamut boundary.
    SoftClip,
    /// Reduce-and-lift uniform reduction (RA-LUR style).
    RaLur,
    /// Hue-preserving reduction: reduce chroma while keeping hue.
    HuePreserve,
}

/// Gamut boundary definition using CIE xy chromaticity coordinates.
#[derive(Debug, Clone, Copy)]
pub struct GamutBoundary {
    /// Primary chromaticities [[rx, ry], [gx, gy], [bx, by]].
    pub primaries: [[f64; 2]; 3],
    /// White point chromaticity [x, y].
    pub white_point: [f64; 2],
}

/// Gamut mapper that converts colors from source to destination gamut.
pub struct GamutMapper {
    /// The mapping method to use.
    pub method: GamutMappingMethod,
    /// Source color gamut.
    pub src_gamut: GamutBoundary,
    /// Destination color gamut.
    pub dst_gamut: GamutBoundary,
}

impl GamutMapper {
    /// Create a new gamut mapper.
    #[must_use]
    pub fn new(
        method: GamutMappingMethod,
        src_gamut: GamutBoundary,
        dst_gamut: GamutBoundary,
    ) -> Self {
        Self {
            method,
            src_gamut,
            dst_gamut,
        }
    }

    /// Map an RGB triplet from source to destination gamut.
    ///
    /// Input values are expected in [0.0, 1.0] range for SDR or higher for HDR.
    #[must_use]
    pub fn map_rgb(&self, r: f64, g: f64, b: f64) -> (f64, f64, f64) {
        match self.method {
            GamutMappingMethod::Clip => clip_rgb(r, g, b),
            GamutMappingMethod::Compress => compress_rgb(r, g, b),
            GamutMappingMethod::SoftClip => soft_clip_rgb(r, g, b),
            GamutMappingMethod::RaLur => ra_lur_rgb(r, g, b),
            GamutMappingMethod::HuePreserve => hue_preserve_rgb(r, g, b),
        }
    }

    /// Check whether an RGB value is within the destination gamut (0.0..=1.0 for all channels).
    #[must_use]
    pub fn is_in_gamut(&self, r: f64, g: f64, b: f64) -> bool {
        r >= 0.0 && r <= 1.0 && g >= 0.0 && g <= 1.0 && b >= 0.0 && b <= 1.0
    }

    /// Compute the fraction of pixels that are out of gamut (not in \[0,1\]^3).
    ///
    /// Returns a value in [0.0, 1.0] representing the out-of-gamut ratio.
    #[must_use]
    pub fn compute_out_of_gamut_ratio(pixels: &[(f64, f64, f64)]) -> f64 {
        if pixels.is_empty() {
            return 0.0;
        }
        let out_of_gamut = pixels
            .iter()
            .filter(|(r, g, b)| {
                *r < 0.0 || *r > 1.0 || *g < 0.0 || *g > 1.0 || *b < 0.0 || *b > 1.0
            })
            .count();
        out_of_gamut as f64 / pixels.len() as f64
    }
}

// ── Internal mapping implementations ────────────────────────────────────────

/// Hard clip: clamp each channel to [0.0, 1.0].
#[inline]
fn clip_rgb(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    (r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0))
}

/// Chroma compression: reduce the color toward neutral grey to bring it inside gamut.
fn compress_rgb(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    // Rec.709 luminance weights
    let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let luma = luma.clamp(0.0, 1.0);

    // Find the maximum over-range
    let max_over = r.max(g).max(b).max(1.0);
    if max_over <= 1.0 {
        return (r.max(0.0), g.max(0.0), b.max(0.0));
    }

    // Scale chroma toward luma so the most saturated channel lands at 1.0
    let scale = (1.0 - luma) / (max_over - luma).max(f64::EPSILON);
    let rc = luma + (r - luma) * scale;
    let gc = luma + (g - luma) * scale;
    let bc = luma + (b - luma) * scale;
    (rc.clamp(0.0, 1.0), gc.clamp(0.0, 1.0), bc.clamp(0.0, 1.0))
}

/// Soft clip: smooth knee function near the [0,1] boundary.
fn soft_clip_rgb(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    (
        soft_clip_channel(r),
        soft_clip_channel(g),
        soft_clip_channel(b),
    )
}

/// Sigmoid-like soft clip for a single channel.
#[inline]
fn soft_clip_channel(v: f64) -> f64 {
    // Knee at 0.8; above that transition to 1.0 with a smooth curve
    const KNEE: f64 = 0.8;
    if v <= 0.0 {
        return 0.0;
    }
    if v <= KNEE {
        return v;
    }
    if v >= 2.0 {
        return 1.0;
    }
    // Smooth interpolation between knee and hard limit
    let t = (v - KNEE) / (2.0 - KNEE);
    let t = t.clamp(0.0, 1.0);
    KNEE + (1.0 - KNEE) * (3.0 * t * t - 2.0 * t * t * t)
}

/// RA-LUR style: uniform scale down all channels to bring the peak within range.
fn ra_lur_rgb(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let peak = r.max(g).max(b);
    if peak <= 1.0 {
        return (r.max(0.0), g.max(0.0), b.max(0.0));
    }
    (
        (r / peak).clamp(0.0, 1.0),
        (g / peak).clamp(0.0, 1.0),
        (b / peak).clamp(0.0, 1.0),
    )
}

/// Hue-preserving compression: reduce chroma while maintaining hue angle.
fn hue_preserve_rgb(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    // Rec.709 luminance
    let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;

    // Chroma vectors
    let cr = r - luma;
    let cg = g - luma;
    let cb = b - luma;

    // Maximum chroma scale that keeps all channels in [0,1]
    // We need luma + t * cr in [0,1], same for g,b
    let max_scale = chroma_scale_limit(luma, cr)
        .min(chroma_scale_limit(luma, cg))
        .min(chroma_scale_limit(luma, cb));

    let scale = max_scale.clamp(0.0, 1.0);
    let ro = (luma + cr * scale).clamp(0.0, 1.0);
    let go = (luma + cg * scale).clamp(0.0, 1.0);
    let bo = (luma + cb * scale).clamp(0.0, 1.0);
    (ro, go, bo)
}

/// Compute the maximum scale `t` such that `luma + t * c` stays in [0, 1].
#[inline]
fn chroma_scale_limit(luma: f64, c: f64) -> f64 {
    if c.abs() < f64::EPSILON {
        return 1.0;
    }
    if c > 0.0 {
        // luma + t*c <= 1  =>  t <= (1 - luma)/c
        (1.0 - luma) / c
    } else {
        // luma + t*c >= 0  =>  t <= luma / (-c)
        luma / (-c)
    }
    .max(0.0)
}

// ── Standard gamut definitions ───────────────────────────────────────────────

/// Standard sRGB / Rec.709 gamut (D65 white point).
#[must_use]
pub fn srgb_gamut() -> GamutBoundary {
    GamutBoundary {
        primaries: [
            [0.640, 0.330], // R
            [0.300, 0.600], // G
            [0.150, 0.060], // B
        ],
        white_point: [0.3127, 0.3290], // D65
    }
}

/// DCI-P3 D65 gamut.
#[must_use]
pub fn p3_d65_gamut() -> GamutBoundary {
    GamutBoundary {
        primaries: [
            [0.680, 0.320], // R
            [0.265, 0.690], // G
            [0.150, 0.060], // B
        ],
        white_point: [0.3127, 0.3290], // D65
    }
}

/// Rec.2020 / BT.2020 gamut (D65 white point).
#[must_use]
pub fn bt2020_gamut() -> GamutBoundary {
    GamutBoundary {
        primaries: [
            [0.708, 0.292], // R
            [0.170, 0.797], // G
            [0.131, 0.046], // B
        ],
        white_point: [0.3127, 0.3290], // D65
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clip_in_gamut() {
        let mapper = GamutMapper::new(GamutMappingMethod::Clip, srgb_gamut(), srgb_gamut());
        let (r, g, b) = mapper.map_rgb(0.5, 0.3, 0.2);
        assert!((r - 0.5).abs() < 1e-10);
        assert!((g - 0.3).abs() < 1e-10);
        assert!((b - 0.2).abs() < 1e-10);
    }

    #[test]
    fn test_clip_out_of_gamut() {
        let mapper = GamutMapper::new(GamutMappingMethod::Clip, bt2020_gamut(), srgb_gamut());
        let (r, g, b) = mapper.map_rgb(1.5, -0.1, 0.8);
        assert_eq!(r, 1.0);
        assert_eq!(g, 0.0);
        assert!((b - 0.8).abs() < 1e-10);
    }

    #[test]
    fn test_compress_in_gamut() {
        let mapper = GamutMapper::new(GamutMappingMethod::Compress, srgb_gamut(), srgb_gamut());
        let (r, g, b) = mapper.map_rgb(0.5, 0.5, 0.5);
        // All in gamut, should not move much
        assert!(r >= 0.0 && r <= 1.0);
        assert!(g >= 0.0 && g <= 1.0);
        assert!(b >= 0.0 && b <= 1.0);
    }

    #[test]
    fn test_compress_out_of_gamut() {
        let mapper = GamutMapper::new(GamutMappingMethod::Compress, bt2020_gamut(), srgb_gamut());
        let (r, g, b) = mapper.map_rgb(1.8, 0.5, 0.1);
        assert!(r <= 1.0 && r >= 0.0, "r={r}");
        assert!(g <= 1.0 && g >= 0.0, "g={g}");
        assert!(b <= 1.0 && b >= 0.0, "b={b}");
    }

    #[test]
    fn test_soft_clip_channel_below_knee() {
        let v = soft_clip_channel(0.5);
        assert!((v - 0.5).abs() < 1e-10, "Below knee should be identity");
    }

    #[test]
    fn test_soft_clip_channel_above_one() {
        let v = soft_clip_channel(1.5);
        assert!(
            v < 1.0 && v > 0.8,
            "Above 1.0 should be soft-clipped toward 1.0"
        );
    }

    #[test]
    fn test_soft_clip_channel_at_zero() {
        let v = soft_clip_channel(0.0);
        assert_eq!(v, 0.0);
    }

    #[test]
    fn test_soft_clip_channel_at_two() {
        let v = soft_clip_channel(2.0);
        assert_eq!(v, 1.0);
    }

    #[test]
    fn test_ra_lur_preserves_hue_ratio() {
        let (r, g, b) = ra_lur_rgb(2.0, 1.0, 0.5);
        // peak is 2.0; all channels divided by 2
        assert!((r - 1.0).abs() < 1e-10);
        assert!((g - 0.5).abs() < 1e-10);
        assert!((b - 0.25).abs() < 1e-10);
    }

    #[test]
    fn test_ra_lur_in_gamut() {
        let (r, g, b) = ra_lur_rgb(0.8, 0.5, 0.2);
        assert!((r - 0.8).abs() < 1e-10);
        assert!((g - 0.5).abs() < 1e-10);
        assert!((b - 0.2).abs() < 1e-10);
    }

    #[test]
    fn test_hue_preserve_in_gamut() {
        let (r, g, b) = hue_preserve_rgb(0.8, 0.5, 0.2);
        assert!(r <= 1.0 && r >= 0.0);
        assert!(g <= 1.0 && g >= 0.0);
        assert!(b <= 1.0 && b >= 0.0);
    }

    #[test]
    fn test_hue_preserve_out_of_gamut() {
        let (r, g, b) = hue_preserve_rgb(1.5, 0.3, 0.1);
        assert!(r <= 1.0 && r >= 0.0, "r={r}");
        assert!(g <= 1.0 && g >= 0.0, "g={g}");
        assert!(b <= 1.0 && b >= 0.0, "b={b}");
    }

    #[test]
    fn test_is_in_gamut() {
        let mapper = GamutMapper::new(GamutMappingMethod::Clip, srgb_gamut(), srgb_gamut());
        assert!(mapper.is_in_gamut(0.5, 0.5, 0.5));
        assert!(!mapper.is_in_gamut(1.1, 0.5, 0.5));
        assert!(!mapper.is_in_gamut(0.5, -0.1, 0.5));
    }

    #[test]
    fn test_compute_out_of_gamut_ratio_empty() {
        let ratio = GamutMapper::compute_out_of_gamut_ratio(&[]);
        assert_eq!(ratio, 0.0);
    }

    #[test]
    fn test_compute_out_of_gamut_ratio_all_in() {
        let pixels = vec![(0.5, 0.5, 0.5), (0.1, 0.9, 0.3)];
        let ratio = GamutMapper::compute_out_of_gamut_ratio(&pixels);
        assert_eq!(ratio, 0.0);
    }

    #[test]
    fn test_compute_out_of_gamut_ratio_half_out() {
        let pixels = vec![(0.5, 0.5, 0.5), (1.5, 0.5, 0.5)];
        let ratio = GamutMapper::compute_out_of_gamut_ratio(&pixels);
        assert!((ratio - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_gamut_boundary_primaries() {
        let g = srgb_gamut();
        assert!((g.primaries[0][0] - 0.640).abs() < 1e-6);
        let p3 = p3_d65_gamut();
        assert!((p3.primaries[0][0] - 0.680).abs() < 1e-6);
        let bt = bt2020_gamut();
        assert!((bt.primaries[0][0] - 0.708).abs() < 1e-6);
    }
}
