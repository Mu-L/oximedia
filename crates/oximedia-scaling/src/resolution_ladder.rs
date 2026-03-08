//! Resolution ladder management for adaptive bitrate streaming.
//!
//! Provides standard resolution constants, ladder construction, and normalization utilities.

use crate::aspect_ratio::AspectRatio;

/// A video resolution expressed as width × height.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Resolution {
    /// Total pixel count.
    #[must_use]
    #[allow(dead_code)]
    pub fn pixels(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    /// Pixel count in megapixels.
    #[must_use]
    #[allow(dead_code)]
    pub fn megapixels(&self) -> f32 {
        self.pixels() as f32 / 1_000_000.0
    }

    /// Reduced aspect ratio.
    #[must_use]
    #[allow(dead_code)]
    pub fn aspect_ratio(&self) -> AspectRatio {
        AspectRatio::new(self.width, self.height)
    }
}

impl PartialOrd for Resolution {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Resolution {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.pixels().cmp(&other.pixels())
    }
}

// Common resolution constants
/// 240p (426×240).
#[allow(dead_code)]
pub const R240P: Resolution = Resolution {
    width: 426,
    height: 240,
};
/// 360p (640×360).
#[allow(dead_code)]
pub const R360P: Resolution = Resolution {
    width: 640,
    height: 360,
};
/// 480p (854×480).
#[allow(dead_code)]
pub const R480P: Resolution = Resolution {
    width: 854,
    height: 480,
};
/// 720p HD (1280×720).
#[allow(dead_code)]
pub const R720P: Resolution = Resolution {
    width: 1280,
    height: 720,
};
/// 1080p Full HD (1920×1080).
#[allow(dead_code)]
pub const R1080P: Resolution = Resolution {
    width: 1920,
    height: 1080,
};
/// 1440p Quad HD (2560×1440).
#[allow(dead_code)]
pub const R1440P: Resolution = Resolution {
    width: 2560,
    height: 1440,
};
/// 2160p 4K UHD (3840×2160).
#[allow(dead_code)]
pub const R2160P: Resolution = Resolution {
    width: 3840,
    height: 2160,
};
/// 4320p 8K UHD (7680×4320).
#[allow(dead_code)]
pub const R4320P: Resolution = Resolution {
    width: 7680,
    height: 4320,
};

/// A resolution ladder — an ordered set of quality rungs for ABR streaming.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct ResolutionLadder {
    /// Rungs in ascending order (smallest first).
    pub rungs: Vec<Resolution>,
}

impl ResolutionLadder {
    /// Create an empty ladder.
    #[must_use]
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self { rungs: Vec::new() }
    }

    /// Add a resolution rung (inserted in sorted order).
    #[allow(dead_code)]
    pub fn add(&mut self, r: Resolution) {
        let pos = self
            .rungs
            .partition_point(|existing| existing.pixels() <= r.pixels());
        self.rungs.insert(pos, r);
    }

    /// Return all rungs with fewer pixels than `r`.
    #[must_use]
    #[allow(dead_code)]
    pub fn below(&self, r: &Resolution) -> Vec<&Resolution> {
        self.rungs
            .iter()
            .filter(|&rung| rung.pixels() < r.pixels())
            .collect()
    }

    /// Return all rungs with more pixels than `r`.
    #[must_use]
    #[allow(dead_code)]
    pub fn above(&self, r: &Resolution) -> Vec<&Resolution> {
        self.rungs
            .iter()
            .filter(|&rung| rung.pixels() > r.pixels())
            .collect()
    }

    /// Return the rung nearest in pixel count to `(w, h)`.
    ///
    /// Panics if the ladder is empty.
    #[must_use]
    #[allow(dead_code)]
    pub fn nearest(&self, w: u32, h: u32) -> &Resolution {
        let target = u64::from(w) * u64::from(h);
        self.rungs
            .iter()
            .min_by_key(|r| {
                let p = r.pixels();
                if p >= target {
                    p - target
                } else {
                    target - p
                }
            })
            .expect("ResolutionLadder must not be empty")
    }
}

/// Generates standard ABR resolution ladders for common codecs.
pub struct LadderGenerator;

impl LadderGenerator {
    /// Build a standard ABR ladder for the given codec up to `max` resolution.
    ///
    /// - `"h264"` / `"avc"`: 240p, 360p, 480p, 720p, 1080p
    /// - `"h265"` / `"hevc"`: 360p, 720p, 1080p, 2160p
    /// - `"av1"`: 240p, 480p, 720p, 1080p, 1440p, 2160p
    /// - Any other: full standard ladder
    #[must_use]
    #[allow(dead_code)]
    pub fn abr_ladder(max: Resolution, codec: &str) -> ResolutionLadder {
        let candidates: &[Resolution] = match codec.to_lowercase().as_str() {
            "h264" | "avc" => &[R240P, R360P, R480P, R720P, R1080P],
            "h265" | "hevc" => &[R360P, R720P, R1080P, R2160P],
            "av1" => &[R240P, R480P, R720P, R1080P, R1440P, R2160P],
            _ => &[R240P, R360P, R480P, R720P, R1080P, R1440P, R2160P],
        };

        let mut ladder = ResolutionLadder::new();
        for &r in candidates {
            if r.pixels() <= max.pixels() {
                ladder.add(r);
            }
        }
        // Always include max if not already present
        if !ladder.rungs.contains(&max) {
            ladder.add(max);
        }
        ladder
    }
}

/// Normalizes resolutions to be divisible by a given modulus.
pub struct ResolutionNormalizer;

impl ResolutionNormalizer {
    /// Round `(w, h)` to the nearest multiple of `modulus`.
    ///
    /// This is required by many codecs (e.g. H.264 requires mod-2, HEVC may require mod-8).
    #[must_use]
    #[allow(dead_code)]
    pub fn normalize_to_mod(w: u32, h: u32, modulus: u32) -> (u32, u32) {
        if modulus == 0 {
            return (w, h);
        }
        let round_to_mod = |v: u32| {
            let rem = v % modulus;
            if rem == 0 {
                v
            } else if rem < modulus / 2 + modulus % 2 {
                v - rem
            } else {
                v + (modulus - rem)
            }
        };
        (round_to_mod(w).max(modulus), round_to_mod(h).max(modulus))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolution_pixels() {
        assert_eq!(R1080P.pixels(), 1920 * 1080);
    }

    #[test]
    fn test_resolution_megapixels() {
        let mp = R1080P.megapixels();
        assert!((mp - 2.0736).abs() < 0.001);
    }

    #[test]
    fn test_resolution_aspect_ratio() {
        let ar = R1080P.aspect_ratio();
        assert_eq!(ar.width, 16);
        assert_eq!(ar.height, 9);
    }

    #[test]
    fn test_resolution_ordering() {
        assert!(R720P < R1080P);
        assert!(R2160P > R1080P);
    }

    #[test]
    fn test_ladder_add_sorted() {
        let mut ladder = ResolutionLadder::new();
        ladder.add(R1080P);
        ladder.add(R480P);
        ladder.add(R720P);
        assert_eq!(ladder.rungs[0], R480P);
        assert_eq!(ladder.rungs[1], R720P);
        assert_eq!(ladder.rungs[2], R1080P);
    }

    #[test]
    fn test_ladder_below() {
        let mut ladder = ResolutionLadder::new();
        ladder.add(R480P);
        ladder.add(R720P);
        ladder.add(R1080P);
        let below = ladder.below(&R720P);
        assert_eq!(below.len(), 1);
        assert_eq!(*below[0], R480P);
    }

    #[test]
    fn test_ladder_above() {
        let mut ladder = ResolutionLadder::new();
        ladder.add(R480P);
        ladder.add(R720P);
        ladder.add(R1080P);
        let above = ladder.above(&R720P);
        assert_eq!(above.len(), 1);
        assert_eq!(*above[0], R1080P);
    }

    #[test]
    fn test_ladder_nearest() {
        let mut ladder = ResolutionLadder::new();
        ladder.add(R480P);
        ladder.add(R720P);
        ladder.add(R1080P);
        let nearest = ladder.nearest(1280, 720);
        assert_eq!(*nearest, R720P);
    }

    #[test]
    fn test_abr_ladder_h264() {
        let ladder = LadderGenerator::abr_ladder(R1080P, "h264");
        assert!(!ladder.rungs.is_empty());
        // Should not exceed max
        for r in &ladder.rungs {
            assert!(r.pixels() <= R1080P.pixels());
        }
    }

    #[test]
    fn test_abr_ladder_av1() {
        let ladder = LadderGenerator::abr_ladder(R2160P, "av1");
        assert!(ladder.rungs.len() >= 4);
    }

    #[test]
    fn test_normalize_to_mod_exact() {
        // 1920 is divisible by 16, 1088 is the nearest multiple of 16 to 1080
        let (w, h) = ResolutionNormalizer::normalize_to_mod(1920, 1080, 16);
        assert_eq!(w, 1920);
        // 1080 % 16 = 8; since 8 >= 16/2=8, rounds up to 1080+8=1088
        assert_eq!(h, 1088);
    }

    #[test]
    fn test_normalize_to_mod_rounding() {
        let (w, h) = ResolutionNormalizer::normalize_to_mod(1921, 1081, 2);
        assert_eq!(w % 2, 0);
        assert_eq!(h % 2, 0);
    }

    #[test]
    fn test_normalize_to_mod_zero_modulus() {
        let (w, h) = ResolutionNormalizer::normalize_to_mod(1920, 1080, 0);
        assert_eq!(w, 1920);
        assert_eq!(h, 1080);
    }
}
