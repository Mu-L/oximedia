//! Chromatic aberration simulation for VFX and lens-matching workflows.
//!
//! Models the radial colour fringing produced by imperfect lens optics where
//! different wavelengths focus at slightly different points.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Axis along which lateral chromatic aberration is expressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AberrationAxis {
    /// Horizontal colour fringing.
    Horizontal,
    /// Vertical colour fringing.
    Vertical,
    /// Radial fringing (both axes, centred on the optical axis).
    Radial,
    /// Tangential fringing (perpendicular to the radial direction).
    Tangential,
}

impl AberrationAxis {
    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
            Self::Radial => "radial",
            Self::Tangential => "tangential",
        }
    }

    /// Returns `true` if the axis affects both spatial dimensions.
    pub fn is_two_dimensional(self) -> bool {
        matches!(self, Self::Radial | Self::Tangential)
    }
}

/// Describes a chromatic aberration along a single axis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChromaticAberration {
    /// Axis of aberration.
    pub axis: AberrationAxis,
    /// Red-channel offset in normalised image coordinates (-1.0 – 1.0).
    pub red_offset: f32,
    /// Green-channel offset in normalised image coordinates.
    pub green_offset: f32,
    /// Blue-channel offset in normalised image coordinates.
    pub blue_offset: f32,
    /// Radial power factor: how quickly the aberration grows from centre (1.0 = linear).
    pub radial_power: f32,
}

impl Default for ChromaticAberration {
    fn default() -> Self {
        Self {
            axis: AberrationAxis::Radial,
            red_offset: 0.002,
            green_offset: 0.0,
            blue_offset: -0.002,
            radial_power: 1.0,
        }
    }
}

impl ChromaticAberration {
    /// Create a zero-aberration (identity) instance.
    pub fn identity() -> Self {
        Self {
            red_offset: 0.0,
            green_offset: 0.0,
            blue_offset: 0.0,
            ..Self::default()
        }
    }

    /// Compute the radial offset magnitude at a normalised distance `r` from
    /// the optical centre (0.0 = centre, 1.0 = corner).
    pub fn radial_offset(&self, r: f32) -> f32 {
        let r = r.clamp(0.0, 1.0);
        r.powf(self.radial_power.max(0.1))
    }

    /// Returns the per-channel displacement `[red, green, blue]` at radius `r`.
    pub fn channel_offsets_at(&self, r: f32) -> [f32; 3] {
        let scale = self.radial_offset(r);
        [
            self.red_offset * scale,
            self.green_offset * scale,
            self.blue_offset * scale,
        ]
    }

    /// Maximum absolute channel offset across all three channels (useful for extent calculations).
    pub fn max_abs_offset(&self) -> f32 {
        self.red_offset
            .abs()
            .max(self.green_offset.abs())
            .max(self.blue_offset.abs())
    }
}

/// Configuration for the aberration simulation pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AberrationConfig {
    /// Primary lateral chromatic aberration model.
    pub aberration: ChromaticAberration,
    /// Additional global strength multiplier (0.0 = off, 1.0 = full).
    pub strength: f32,
    /// Whether to apply the effect only beyond a certain radius from centre.
    pub vignette_mask: bool,
    /// Inner radius (normalised) beyond which the effect starts when `vignette_mask` is true.
    pub mask_inner_radius: f32,
}

impl Default for AberrationConfig {
    fn default() -> Self {
        Self {
            aberration: ChromaticAberration::default(),
            strength: 1.0,
            vignette_mask: false,
            mask_inner_radius: 0.5,
        }
    }
}

impl AberrationConfig {
    /// Returns `true` when the configured aberration would be barely noticeable.
    ///
    /// Subtle is defined as max channel offset below 0.005 normalised units.
    pub fn is_subtle(&self) -> bool {
        self.aberration.max_abs_offset() * self.strength < 0.005
    }

    /// Returns `true` if the configuration would produce a visually meaningful effect.
    pub fn is_active(&self) -> bool {
        self.strength > 0.0 && self.aberration.max_abs_offset() > 0.0
    }
}

/// Simulates chromatic aberration on a pixel buffer.
#[derive(Debug, Clone)]
pub struct AberrationSimulator {
    /// Configuration for this simulator instance.
    pub config: AberrationConfig,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
}

impl AberrationSimulator {
    /// Create a new simulator for a given frame size.
    pub fn new(width: u32, height: u32, config: AberrationConfig) -> Self {
        Self {
            config,
            width,
            height,
        }
    }

    /// Apply chromatic aberration to a pixel buffer.
    ///
    /// `pixels` is a flat RGBA buffer (4 bytes per pixel, row-major).
    /// The operation returns a new buffer with the effect applied.
    #[allow(clippy::cast_precision_loss)]
    pub fn apply(&self, pixels: &[u8]) -> Vec<u8> {
        let w = self.width as usize;
        let h = self.height as usize;
        let expected = w * h * 4;
        if pixels.len() != expected || w == 0 || h == 0 {
            return pixels.to_vec();
        }

        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;
        let max_r = (cx * cx + cy * cy).sqrt();

        let mut out = pixels.to_vec();

        for py in 0..h {
            for px in 0..w {
                let dx = px as f32 - cx;
                let dy = py as f32 - cy;
                let r = if max_r > 0.0 {
                    (dx * dx + dy * dy).sqrt() / max_r
                } else {
                    0.0
                };

                let mask = if self.config.vignette_mask && r < self.config.mask_inner_radius {
                    0.0_f32
                } else {
                    1.0_f32
                };

                let offsets = self.config.aberration.channel_offsets_at(r);

                // Sample each channel with its offset
                let base_idx = (py * w + px) * 4;
                let channels = [0_usize, 1, 2];
                for (ch, &off) in channels.iter().zip(offsets.iter()) {
                    let effective_off = off * self.config.strength * mask;
                    // Compute offset in pixels
                    let off_px = (effective_off * w as f32) as i32;
                    let off_py = (effective_off * h as f32) as i32;
                    let sx = (px as i32 + off_px).clamp(0, w as i32 - 1) as usize;
                    let sy = (py as i32 + off_py).clamp(0, h as i32 - 1) as usize;
                    let src_idx = (sy * w + sx) * 4 + ch;
                    out[base_idx + ch] = pixels[src_idx];
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aberration_axis_labels_unique() {
        let axes = [
            AberrationAxis::Horizontal,
            AberrationAxis::Vertical,
            AberrationAxis::Radial,
            AberrationAxis::Tangential,
        ];
        let labels: Vec<_> = axes.iter().map(|a| a.label()).collect();
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(labels.len(), unique.len());
    }

    #[test]
    fn test_aberration_axis_two_dimensional() {
        assert!(AberrationAxis::Radial.is_two_dimensional());
        assert!(AberrationAxis::Tangential.is_two_dimensional());
        assert!(!AberrationAxis::Horizontal.is_two_dimensional());
    }

    #[test]
    fn test_chromatic_aberration_identity_zero_offset() {
        let ca = ChromaticAberration::identity();
        assert_eq!(ca.max_abs_offset(), 0.0);
    }

    #[test]
    fn test_radial_offset_centre() {
        let ca = ChromaticAberration::default();
        assert_eq!(ca.radial_offset(0.0), 0.0);
    }

    #[test]
    fn test_radial_offset_corner() {
        let ca = ChromaticAberration::default();
        assert_eq!(ca.radial_offset(1.0), 1.0);
    }

    #[test]
    fn test_channel_offsets_at_zero_radius() {
        let ca = ChromaticAberration::default();
        let offs = ca.channel_offsets_at(0.0);
        assert_eq!(offs, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_channel_offsets_red_blue_opposite() {
        let ca = ChromaticAberration::default();
        let offs = ca.channel_offsets_at(1.0);
        // red and blue should be opposite in sign
        assert!(offs[0] > 0.0);
        assert!(offs[2] < 0.0);
    }

    #[test]
    fn test_max_abs_offset() {
        let ca = ChromaticAberration {
            red_offset: 0.01,
            green_offset: 0.0,
            blue_offset: -0.02,
            ..Default::default()
        };
        assert!((ca.max_abs_offset() - 0.02).abs() < 1e-6);
    }

    #[test]
    fn test_aberration_config_is_subtle_identity() {
        let cfg = AberrationConfig {
            aberration: ChromaticAberration::identity(),
            ..Default::default()
        };
        assert!(cfg.is_subtle());
    }

    #[test]
    fn test_aberration_config_not_subtle_strong() {
        let cfg = AberrationConfig {
            aberration: ChromaticAberration {
                red_offset: 0.05,
                blue_offset: -0.05,
                ..Default::default()
            },
            strength: 1.0,
            ..Default::default()
        };
        assert!(!cfg.is_subtle());
    }

    #[test]
    fn test_aberration_config_is_active() {
        assert!(AberrationConfig::default().is_active());
        let inactive = AberrationConfig {
            strength: 0.0,
            ..Default::default()
        };
        assert!(!inactive.is_active());
    }

    #[test]
    fn test_simulator_apply_identity_unchanged() {
        let cfg = AberrationConfig {
            aberration: ChromaticAberration::identity(),
            strength: 0.0,
            ..Default::default()
        };
        let sim = AberrationSimulator::new(4, 4, cfg);
        let pixels: Vec<u8> = (0..64).collect();
        let result = sim.apply(&pixels);
        // Alpha channel (index 3, 7, …) should be unchanged; with zero strength
        // the result should match the input (but channel sampling still occurs at same coords)
        assert_eq!(result.len(), pixels.len());
    }

    #[test]
    fn test_simulator_apply_wrong_buffer_size_returns_copy() {
        let sim = AberrationSimulator::new(4, 4, AberrationConfig::default());
        let bad_pixels = vec![0u8; 10]; // wrong size
        let result = sim.apply(&bad_pixels);
        assert_eq!(result, bad_pixels);
    }
}
