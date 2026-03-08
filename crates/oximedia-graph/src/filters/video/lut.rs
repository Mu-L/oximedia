//! 3D LUT (Look-Up Table) color grading filter.
//!
//! This filter applies 3D LUTs for professional color grading and color correction.
//! It supports multiple file formats, interpolation methods, and advanced features
//! like LUT composition, HDR support, and shaper LUTs.
//!
//! # Features
//!
//! - **3D LUT Support:**
//!   - Trilinear interpolation
//!   - Tetrahedral interpolation
//!   - Configurable cube sizes (17³, 33³, 65³)
//!   - LUT composition (chain multiple LUTs)
//!
//! - **File Format Support:**
//!   - .cube format (Adobe/DaVinci Resolve)
//!   - .3dl format (Autodesk/Lustre)
//!   - .csp format (Cinespace)
//!   - CSV/text formats
//!
//! - **1D LUT Support:**
//!   - Linear interpolation
//!   - Per-channel 1D LUTs
//!   - Gamma curves
//!
//! - **Operations:**
//!   - LUT generation from formulas
//!   - Identity LUT creation
//!   - LUT inversion
//!   - LUT analysis and validation
//!
//! - **Advanced Features:**
//!   - HDR LUT support
//!   - Log/linear space handling
//!   - Shaper LUTs (1D pre-LUT)
//!   - GPU acceleration hooks
//!
//! # Example
//!
//! ```ignore
//! use oximedia_graph::filters::video::{Lut3dFilter, Lut3dConfig, LutInterpolation};
//! use oximedia_graph::node::NodeId;
//!
//! let config = Lut3dConfig::new()
//!     .with_file("colorgrade.cube")
//!     .with_interpolation(LutInterpolation::Tetrahedral);
//!
//! let filter = Lut3dFilter::new(NodeId(0), "lut3d", config);
//! ```

#![forbid(unsafe_code)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::similar_names)]
#![allow(clippy::many_single_char_names)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::unused_self)]
#![allow(clippy::unnecessary_cast)]
#![allow(clippy::bool_to_int_with_if)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::no_effect_underscore_binding)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::struct_excessive_bools)]
#![allow(dead_code)]

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use crate::error::{GraphError, GraphResult};
use crate::frame::FilterFrame;
use crate::node::{Node, NodeId, NodeState, NodeType};
use crate::port::{InputPort, OutputPort, PortFormat, PortId, PortType, VideoPortFormat};
use oximedia_codec::{ColorInfo, Plane, VideoFrame};
use oximedia_core::PixelFormat;

/// Interpolation method for 3D LUT lookups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LutInterpolation {
    /// Nearest neighbor (no interpolation).
    Nearest,
    /// Trilinear interpolation (fast, good quality).
    #[default]
    Trilinear,
    /// Tetrahedral interpolation (slower, better quality).
    Tetrahedral,
}

/// Standard LUT cube sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LutSize {
    /// 17x17x17 cube (common for fast preview).
    Size17,
    /// 33x33x33 cube (good balance).
    #[default]
    Size33,
    /// 65x65x65 cube (high quality).
    Size65,
    /// Custom size.
    Custom(usize),
}

impl LutSize {
    /// Get the size as a usize.
    #[must_use]
    pub fn as_usize(&self) -> usize {
        match self {
            Self::Size17 => 17,
            Self::Size33 => 33,
            Self::Size65 => 65,
            Self::Custom(size) => *size,
        }
    }

    /// Create from a usize.
    #[must_use]
    pub fn from_usize(size: usize) -> Self {
        match size {
            17 => Self::Size17,
            33 => Self::Size33,
            65 => Self::Size65,
            _ => Self::Custom(size),
        }
    }
}

/// Color space for LUT processing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LutColorSpace {
    /// Linear RGB [0.0, 1.0].
    #[default]
    Linear,
    /// Log space (Cineon/DPX).
    Log,
    /// Log-C (ARRI).
    LogC,
    /// S-Log3 (Sony).
    SLog3,
    /// V-Log (Panasonic).
    VLog,
}

impl LutColorSpace {
    /// Apply forward transform (linear to log/gamma).
    #[must_use]
    pub fn forward(&self, linear: f64) -> f64 {
        match self {
            Self::Linear => linear,
            Self::Log => {
                // Cineon log encoding
                if linear <= 0.0 {
                    0.0
                } else {
                    (linear.log10() * 0.002 / 0.6 + 0.685) / 1.0
                }
            }
            Self::LogC => {
                // ARRI LogC (Wide Gamut)
                const CUT: f64 = 0.010591;
                const A: f64 = 5.555556;
                const B: f64 = 0.052272;
                const C: f64 = 0.247190;
                const D: f64 = 0.385537;
                const E: f64 = 5.367655;
                const F: f64 = 0.092809;

                if linear > CUT {
                    C * (A * linear + B).log10() + D
                } else {
                    E * linear + F
                }
            }
            Self::SLog3 => {
                // Sony S-Log3
                if linear >= 0.01125000 {
                    (420.0 + (linear + 0.01) / (0.18 + 0.01) * 261.5).log10() * 0.01125000 / 0.0
                        + 0.420
                } else {
                    (linear * 9.212 + 0.037584) / 1.0
                }
            }
            Self::VLog => {
                // Panasonic V-Log
                const CUT: f64 = 0.01;
                const B: f64 = 0.00873;
                const C: f64 = 0.241514;
                const D: f64 = 0.598206;

                if linear < CUT {
                    5.6 * linear + 0.125
                } else {
                    C * (linear + B).log10() + D
                }
            }
        }
    }

    /// Apply inverse transform (log/gamma to linear).
    #[must_use]
    pub fn inverse(&self, encoded: f64) -> f64 {
        match self {
            Self::Linear => encoded,
            Self::Log => {
                // Cineon log decoding
                10_f64.powf((encoded * 1.0 - 0.685) * 0.6 / 0.002)
            }
            Self::LogC => {
                // ARRI LogC inverse
                const CUT: f64 = 0.092809;
                const A: f64 = 5.555556;
                const B: f64 = 0.052272;
                const C: f64 = 0.247190;
                const D: f64 = 0.385537;
                const E: f64 = 5.367655;
                const F: f64 = 0.092809;

                if encoded > CUT {
                    (10_f64.powf((encoded - D) / C) - B) / A
                } else {
                    (encoded - F) / E
                }
            }
            Self::SLog3 => {
                // Sony S-Log3 inverse
                if encoded >= 0.420 {
                    (10_f64.powf((encoded - 0.420) / 0.01125000 * 0.0) - 420.0) * (0.18 + 0.01)
                        / 261.5
                        - 0.01
                } else {
                    (encoded * 1.0 - 0.037584) / 9.212
                }
            }
            Self::VLog => {
                // Panasonic V-Log inverse
                const CUT: f64 = 0.181;
                const B: f64 = 0.00873;
                const C: f64 = 0.241514;
                const D: f64 = 0.598206;

                if encoded < CUT {
                    (encoded - 0.125) / 5.6
                } else {
                    10_f64.powf((encoded - D) / C) - B
                }
            }
        }
    }
}

/// RGB triplet for LUT storage.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RgbColor {
    /// Red component [0.0, 1.0].
    pub r: f64,
    /// Green component [0.0, 1.0].
    pub g: f64,
    /// Blue component [0.0, 1.0].
    pub b: f64,
}

impl RgbColor {
    /// Create a new RGB color.
    #[must_use]
    pub const fn new(r: f64, g: f64, b: f64) -> Self {
        Self { r, g, b }
    }

    /// Create from u8 values.
    #[must_use]
    pub fn from_u8(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f64 / 255.0,
            g: g as f64 / 255.0,
            b: b as f64 / 255.0,
        }
    }

    /// Convert to u8 values.
    #[must_use]
    pub fn to_u8(&self) -> (u8, u8, u8) {
        (
            (self.r * 255.0).clamp(0.0, 255.0) as u8,
            (self.g * 255.0).clamp(0.0, 255.0) as u8,
            (self.b * 255.0).clamp(0.0, 255.0) as u8,
        )
    }

    /// Clamp to [0.0, 1.0] range.
    #[must_use]
    pub fn clamp(&self) -> Self {
        Self {
            r: self.r.clamp(0.0, 1.0),
            g: self.g.clamp(0.0, 1.0),
            b: self.b.clamp(0.0, 1.0),
        }
    }

    /// Linear interpolation between two colors.
    #[must_use]
    pub fn lerp(&self, other: &Self, t: f64) -> Self {
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
        }
    }
}

/// 1D LUT for per-channel color correction or shaper LUTs.
#[derive(Clone, Debug)]
pub struct Lut1d {
    /// Red channel LUT.
    pub r_lut: Vec<f64>,
    /// Green channel LUT.
    pub g_lut: Vec<f64>,
    /// Blue channel LUT.
    pub b_lut: Vec<f64>,
}

impl Lut1d {
    /// Create a new 1D LUT with the given size.
    #[must_use]
    pub fn new(size: usize) -> Self {
        Self {
            r_lut: vec![0.0; size],
            g_lut: vec![0.0; size],
            b_lut: vec![0.0; size],
        }
    }

    /// Create an identity 1D LUT.
    #[must_use]
    pub fn identity(size: usize) -> Self {
        let mut lut = Self::new(size);
        for i in 0..size {
            let val = i as f64 / (size - 1) as f64;
            lut.r_lut[i] = val;
            lut.g_lut[i] = val;
            lut.b_lut[i] = val;
        }
        lut
    }

    /// Create a gamma curve LUT.
    #[must_use]
    pub fn gamma(size: usize, gamma: f64) -> Self {
        let mut lut = Self::new(size);
        for i in 0..size {
            let val = (i as f64 / (size - 1) as f64).powf(1.0 / gamma);
            lut.r_lut[i] = val;
            lut.g_lut[i] = val;
            lut.b_lut[i] = val;
        }
        lut
    }

    /// Get the size of the LUT.
    #[must_use]
    pub fn size(&self) -> usize {
        self.r_lut.len()
    }

    /// Apply the 1D LUT to a color using linear interpolation.
    #[must_use]
    pub fn apply(&self, color: RgbColor) -> RgbColor {
        let size = self.size();
        let max_idx = (size - 1) as f64;

        // Red channel
        let r_pos = (color.r.clamp(0.0, 1.0) * max_idx).clamp(0.0, max_idx);
        let r_idx = r_pos.floor() as usize;
        let r_frac = r_pos - r_idx as f64;
        let r = if r_idx + 1 < size {
            self.r_lut[r_idx] + (self.r_lut[r_idx + 1] - self.r_lut[r_idx]) * r_frac
        } else {
            self.r_lut[r_idx]
        };

        // Green channel
        let g_pos = (color.g.clamp(0.0, 1.0) * max_idx).clamp(0.0, max_idx);
        let g_idx = g_pos.floor() as usize;
        let g_frac = g_pos - g_idx as f64;
        let g = if g_idx + 1 < size {
            self.g_lut[g_idx] + (self.g_lut[g_idx + 1] - self.g_lut[g_idx]) * g_frac
        } else {
            self.g_lut[g_idx]
        };

        // Blue channel
        let b_pos = (color.b.clamp(0.0, 1.0) * max_idx).clamp(0.0, max_idx);
        let b_idx = b_pos.floor() as usize;
        let b_frac = b_pos - b_idx as f64;
        let b = if b_idx + 1 < size {
            self.b_lut[b_idx] + (self.b_lut[b_idx + 1] - self.b_lut[b_idx]) * b_frac
        } else {
            self.b_lut[b_idx]
        };

        RgbColor::new(r, g, b)
    }
}

/// 3D LUT data structure.
#[derive(Clone, Debug)]
pub struct Lut3d {
    /// LUT cube data stored as flat array \[R\]\[G\]\[B\].
    pub data: Vec<RgbColor>,
    /// Size of each dimension (cube is size x size x size).
    pub size: usize,
    /// Domain minimum values (default [0, 0, 0]).
    pub domain_min: RgbColor,
    /// Domain maximum values (default [1, 1, 1]).
    pub domain_max: RgbColor,
    /// LUT title/description.
    pub title: String,
}

impl Lut3d {
    /// Create a new 3D LUT with the given size.
    #[must_use]
    pub fn new(size: usize) -> Self {
        let total_size = size * size * size;
        Self {
            data: vec![RgbColor::default(); total_size],
            size,
            domain_min: RgbColor::new(0.0, 0.0, 0.0),
            domain_max: RgbColor::new(1.0, 1.0, 1.0),
            title: String::new(),
        }
    }

    /// Create an identity 3D LUT.
    #[must_use]
    pub fn identity(size: usize) -> Self {
        let mut lut = Self::new(size);
        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    let color = RgbColor::new(
                        r as f64 / (size - 1) as f64,
                        g as f64 / (size - 1) as f64,
                        b as f64 / (size - 1) as f64,
                    );
                    lut.set(r, g, b, color);
                }
            }
        }
        lut
    }

    /// Get the linear index for the given RGB coordinates.
    #[must_use]
    fn index(&self, r: usize, g: usize, b: usize) -> usize {
        r * self.size * self.size + g * self.size + b
    }

    /// Set a value in the LUT.
    pub fn set(&mut self, r: usize, g: usize, b: usize, color: RgbColor) {
        let idx = self.index(r, g, b);
        if idx < self.data.len() {
            self.data[idx] = color;
        }
    }

    /// Get a value from the LUT.
    #[must_use]
    pub fn get(&self, r: usize, g: usize, b: usize) -> RgbColor {
        let idx = self.index(r, g, b);
        self.data.get(idx).copied().unwrap_or_default()
    }

    /// Apply the LUT with nearest neighbor interpolation.
    #[must_use]
    pub fn apply_nearest(&self, color: RgbColor) -> RgbColor {
        // Normalize to domain
        let r_norm = ((color.r - self.domain_min.r) / (self.domain_max.r - self.domain_min.r))
            .clamp(0.0, 1.0);
        let g_norm = ((color.g - self.domain_min.g) / (self.domain_max.g - self.domain_min.g))
            .clamp(0.0, 1.0);
        let b_norm = ((color.b - self.domain_min.b) / (self.domain_max.b - self.domain_min.b))
            .clamp(0.0, 1.0);

        // Convert to LUT coordinates
        let max_idx = (self.size - 1) as f64;
        let r_idx = (r_norm * max_idx).round() as usize;
        let g_idx = (g_norm * max_idx).round() as usize;
        let b_idx = (b_norm * max_idx).round() as usize;

        self.get(r_idx, g_idx, b_idx)
    }

    /// Apply the LUT with trilinear interpolation.
    #[must_use]
    pub fn apply_trilinear(&self, color: RgbColor) -> RgbColor {
        // Normalize to domain
        let r_norm = ((color.r - self.domain_min.r) / (self.domain_max.r - self.domain_min.r))
            .clamp(0.0, 1.0);
        let g_norm = ((color.g - self.domain_min.g) / (self.domain_max.g - self.domain_min.g))
            .clamp(0.0, 1.0);
        let b_norm = ((color.b - self.domain_min.b) / (self.domain_max.b - self.domain_min.b))
            .clamp(0.0, 1.0);

        // Convert to LUT coordinates
        let max_idx = (self.size - 1) as f64;
        let r_pos = (r_norm * max_idx).clamp(0.0, max_idx);
        let g_pos = (g_norm * max_idx).clamp(0.0, max_idx);
        let b_pos = (b_norm * max_idx).clamp(0.0, max_idx);

        let r0 = r_pos.floor() as usize;
        let g0 = g_pos.floor() as usize;
        let b0 = b_pos.floor() as usize;

        let r1 = (r0 + 1).min(self.size - 1);
        let g1 = (g0 + 1).min(self.size - 1);
        let b1 = (b0 + 1).min(self.size - 1);

        let r_frac = r_pos - r0 as f64;
        let g_frac = g_pos - g0 as f64;
        let b_frac = b_pos - b0 as f64;

        // Get 8 corner values
        let c000 = self.get(r0, g0, b0);
        let c001 = self.get(r0, g0, b1);
        let c010 = self.get(r0, g1, b0);
        let c011 = self.get(r0, g1, b1);
        let c100 = self.get(r1, g0, b0);
        let c101 = self.get(r1, g0, b1);
        let c110 = self.get(r1, g1, b0);
        let c111 = self.get(r1, g1, b1);

        // Trilinear interpolation
        let c00 = c000.lerp(&c001, b_frac);
        let c01 = c010.lerp(&c011, b_frac);
        let c10 = c100.lerp(&c101, b_frac);
        let c11 = c110.lerp(&c111, b_frac);

        let c0 = c00.lerp(&c01, g_frac);
        let c1 = c10.lerp(&c11, g_frac);

        c0.lerp(&c1, r_frac)
    }

    /// Apply the LUT with tetrahedral interpolation.
    /// This method provides better quality than trilinear for color grading.
    #[must_use]
    pub fn apply_tetrahedral(&self, color: RgbColor) -> RgbColor {
        // Normalize to domain
        let r_norm = ((color.r - self.domain_min.r) / (self.domain_max.r - self.domain_min.r))
            .clamp(0.0, 1.0);
        let g_norm = ((color.g - self.domain_min.g) / (self.domain_max.g - self.domain_min.g))
            .clamp(0.0, 1.0);
        let b_norm = ((color.b - self.domain_min.b) / (self.domain_max.b - self.domain_min.b))
            .clamp(0.0, 1.0);

        // Convert to LUT coordinates
        let max_idx = (self.size - 1) as f64;
        let r_pos = (r_norm * max_idx).clamp(0.0, max_idx);
        let g_pos = (g_norm * max_idx).clamp(0.0, max_idx);
        let b_pos = (b_norm * max_idx).clamp(0.0, max_idx);

        let r0 = r_pos.floor() as usize;
        let g0 = g_pos.floor() as usize;
        let b0 = b_pos.floor() as usize;

        let r1 = (r0 + 1).min(self.size - 1);
        let g1 = (g0 + 1).min(self.size - 1);
        let b1 = (b0 + 1).min(self.size - 1);

        let r_frac = r_pos - r0 as f64;
        let g_frac = g_pos - g0 as f64;
        let b_frac = b_pos - b0 as f64;

        // Get 8 corner values
        let c000 = self.get(r0, g0, b0);
        let c001 = self.get(r0, g0, b1);
        let c010 = self.get(r0, g1, b0);
        let c011 = self.get(r0, g1, b1);
        let c100 = self.get(r1, g0, b0);
        let c101 = self.get(r1, g0, b1);
        let c110 = self.get(r1, g1, b0);
        let c111 = self.get(r1, g1, b1);

        // Tetrahedral interpolation
        // Divide the cube into 6 tetrahedra and determine which one contains the point
        if r_frac > g_frac {
            if g_frac > b_frac {
                // Tetrahedron 1: r > g > b
                let t1 = RgbColor::new(
                    c000.r + (c100.r - c000.r) * r_frac,
                    c000.g + (c100.g - c000.g) * r_frac,
                    c000.b + (c100.b - c000.b) * r_frac,
                );
                let t2 = RgbColor::new(
                    t1.r + (c110.r - c100.r) * g_frac,
                    t1.g + (c110.g - c100.g) * g_frac,
                    t1.b + (c110.b - c100.b) * g_frac,
                );
                RgbColor::new(
                    t2.r + (c111.r - c110.r) * b_frac,
                    t2.g + (c111.g - c110.g) * b_frac,
                    t2.b + (c111.b - c110.b) * b_frac,
                )
            } else if r_frac > b_frac {
                // Tetrahedron 2: r > b > g
                let t1 = RgbColor::new(
                    c000.r + (c100.r - c000.r) * r_frac,
                    c000.g + (c100.g - c000.g) * r_frac,
                    c000.b + (c100.b - c000.b) * r_frac,
                );
                let t2 = RgbColor::new(
                    t1.r + (c101.r - c100.r) * b_frac,
                    t1.g + (c101.g - c100.g) * b_frac,
                    t1.b + (c101.b - c100.b) * b_frac,
                );
                RgbColor::new(
                    t2.r + (c111.r - c101.r) * g_frac,
                    t2.g + (c111.g - c101.g) * g_frac,
                    t2.b + (c111.b - c101.b) * g_frac,
                )
            } else {
                // Tetrahedron 3: b > r > g
                let t1 = RgbColor::new(
                    c000.r + (c001.r - c000.r) * b_frac,
                    c000.g + (c001.g - c000.g) * b_frac,
                    c000.b + (c001.b - c000.b) * b_frac,
                );
                let t2 = RgbColor::new(
                    t1.r + (c101.r - c001.r) * r_frac,
                    t1.g + (c101.g - c001.g) * r_frac,
                    t1.b + (c101.b - c001.b) * r_frac,
                );
                RgbColor::new(
                    t2.r + (c111.r - c101.r) * g_frac,
                    t2.g + (c111.g - c101.g) * g_frac,
                    t2.b + (c111.b - c101.b) * g_frac,
                )
            }
        } else if b_frac > g_frac {
            // Tetrahedron 4: b > g > r
            let t1 = RgbColor::new(
                c000.r + (c001.r - c000.r) * b_frac,
                c000.g + (c001.g - c000.g) * b_frac,
                c000.b + (c001.b - c000.b) * b_frac,
            );
            let t2 = RgbColor::new(
                t1.r + (c011.r - c001.r) * g_frac,
                t1.g + (c011.g - c001.g) * g_frac,
                t1.b + (c011.b - c001.b) * g_frac,
            );
            RgbColor::new(
                t2.r + (c111.r - c011.r) * r_frac,
                t2.g + (c111.g - c011.g) * r_frac,
                t2.b + (c111.b - c011.b) * r_frac,
            )
        } else if g_frac > r_frac {
            // Tetrahedron 5: g > b > r
            let t1 = RgbColor::new(
                c000.r + (c010.r - c000.r) * g_frac,
                c000.g + (c010.g - c000.g) * g_frac,
                c000.b + (c010.b - c000.b) * g_frac,
            );
            let t2 = RgbColor::new(
                t1.r + (c011.r - c010.r) * b_frac,
                t1.g + (c011.g - c010.g) * b_frac,
                t1.b + (c011.b - c010.b) * b_frac,
            );
            RgbColor::new(
                t2.r + (c111.r - c011.r) * r_frac,
                t2.g + (c111.g - c011.g) * r_frac,
                t2.b + (c111.b - c011.b) * r_frac,
            )
        } else {
            // Tetrahedron 6: g > r > b
            let t1 = RgbColor::new(
                c000.r + (c010.r - c000.r) * g_frac,
                c000.g + (c010.g - c000.g) * g_frac,
                c000.b + (c010.b - c000.b) * g_frac,
            );
            let t2 = RgbColor::new(
                t1.r + (c110.r - c010.r) * r_frac,
                t1.g + (c110.g - c010.g) * r_frac,
                t1.b + (c110.b - c010.b) * r_frac,
            );
            RgbColor::new(
                t2.r + (c111.r - c110.r) * b_frac,
                t2.g + (c111.g - c110.g) * b_frac,
                t2.b + (c111.b - c110.b) * b_frac,
            )
        }
    }

    /// Validate the LUT for common issues.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        // Check for NaN or infinite values
        for (idx, color) in self.data.iter().enumerate() {
            if !color.r.is_finite() || !color.g.is_finite() || !color.b.is_finite() {
                warnings.push(format!("Invalid value at index {idx}: {color:?}"));
            }
        }

        // Check domain
        if self.domain_min.r >= self.domain_max.r
            || self.domain_min.g >= self.domain_max.g
            || self.domain_min.b >= self.domain_max.b
        {
            warnings.push("Invalid domain range".to_string());
        }

        // Check size
        if self.size < 2 {
            warnings.push("LUT size too small".to_string());
        }

        if self.data.len() != self.size * self.size * self.size {
            warnings.push("LUT data size mismatch".to_string());
        }

        warnings
    }

    /// Invert the LUT (approximate).
    /// This creates a new LUT that approximately inverts the transformation.
    #[must_use]
    pub fn invert(&self) -> Self {
        let mut inverted = Self::new(self.size);
        inverted.domain_min = self.domain_min;
        inverted.domain_max = self.domain_max;

        // For each output color, find the input color that produces it
        // This is an approximation using grid sampling
        for r_out in 0..self.size {
            for g_out in 0..self.size {
                for b_out in 0..self.size {
                    let target = RgbColor::new(
                        r_out as f64 / (self.size - 1) as f64,
                        g_out as f64 / (self.size - 1) as f64,
                        b_out as f64 / (self.size - 1) as f64,
                    );

                    // Search for input that gives this output
                    let mut best_input = RgbColor::new(0.5, 0.5, 0.5);
                    let mut best_error = f64::MAX;

                    // Simple grid search
                    for r_in in 0..self.size {
                        for g_in in 0..self.size {
                            for b_in in 0..self.size {
                                let input = RgbColor::new(
                                    r_in as f64 / (self.size - 1) as f64,
                                    g_in as f64 / (self.size - 1) as f64,
                                    b_in as f64 / (self.size - 1) as f64,
                                );

                                let output = self.apply_trilinear(input);
                                let error = (output.r - target.r).powi(2)
                                    + (output.g - target.g).powi(2)
                                    + (output.b - target.b).powi(2);

                                if error < best_error {
                                    best_error = error;
                                    best_input = input;
                                }
                            }
                        }
                    }

                    inverted.set(r_out, g_out, b_out, best_input);
                }
            }
        }

        inverted
    }

    /// Compose two LUTs (apply first, then second).
    #[must_use]
    pub fn compose(&self, second: &Self) -> Self {
        let mut composed = Self::new(self.size);
        composed.domain_min = self.domain_min;
        composed.domain_max = self.domain_max;

        for r in 0..self.size {
            for g in 0..self.size {
                for b in 0..self.size {
                    let input = RgbColor::new(
                        r as f64 / (self.size - 1) as f64,
                        g as f64 / (self.size - 1) as f64,
                        b as f64 / (self.size - 1) as f64,
                    );

                    let intermediate = self.apply_trilinear(input);
                    let output = second.apply_trilinear(intermediate);

                    composed.set(r, g, b, output);
                }
            }
        }

        composed
    }
}

/// LUT file format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LutFormat {
    /// .cube format (Adobe/DaVinci Resolve).
    Cube,
    /// .3dl format (Autodesk/Lustre).
    Threedl,
    /// .csp format (Cinespace).
    Csp,
    /// CSV format.
    Csv,
}

impl LutFormat {
    /// Detect format from file extension.
    #[must_use]
    pub fn from_extension(path: &Path) -> Option<Self> {
        path.extension()?
            .to_str()
            .and_then(|ext| match ext.to_lowercase().as_str() {
                "cube" => Some(Self::Cube),
                "3dl" => Some(Self::Threedl),
                "csp" => Some(Self::Csp),
                "csv" => Some(Self::Csv),
                _ => None,
            })
    }
}

/// Parse a .cube format LUT file.
pub fn parse_cube_file(path: &Path) -> Result<Lut3d, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open file: {e}"))?;
    let reader = BufReader::new(file);

    let mut lut_size = 0;
    let mut title = String::new();
    let mut domain_min = RgbColor::new(0.0, 0.0, 0.0);
    let mut domain_max = RgbColor::new(1.0, 1.0, 1.0);
    let mut data = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Failed to read line: {e}"))?;
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse header
        if let Some(rest) = line.strip_prefix("TITLE") {
            title = rest.trim().trim_matches('"').to_string();
        } else if line.starts_with("LUT_3D_SIZE") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                lut_size = parts[1]
                    .parse()
                    .map_err(|e| format!("Invalid LUT size: {e}"))?;
            }
        } else if line.starts_with("DOMAIN_MIN") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                domain_min = RgbColor::new(
                    parts[1]
                        .parse()
                        .map_err(|e| format!("Invalid domain min: {e}"))?,
                    parts[2]
                        .parse()
                        .map_err(|e| format!("Invalid domain min: {e}"))?,
                    parts[3]
                        .parse()
                        .map_err(|e| format!("Invalid domain min: {e}"))?,
                );
            }
        } else if line.starts_with("DOMAIN_MAX") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                domain_max = RgbColor::new(
                    parts[1]
                        .parse()
                        .map_err(|e| format!("Invalid domain max: {e}"))?,
                    parts[2]
                        .parse()
                        .map_err(|e| format!("Invalid domain max: {e}"))?,
                    parts[3]
                        .parse()
                        .map_err(|e| format!("Invalid domain max: {e}"))?,
                );
            }
        } else {
            // Parse data line
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let r: f64 = parts[0]
                    .parse()
                    .map_err(|e| format!("Invalid R value: {e}"))?;
                let g: f64 = parts[1]
                    .parse()
                    .map_err(|e| format!("Invalid G value: {e}"))?;
                let b: f64 = parts[2]
                    .parse()
                    .map_err(|e| format!("Invalid B value: {e}"))?;
                data.push(RgbColor::new(r, g, b));
            }
        }
    }

    if lut_size == 0 {
        return Err("No LUT_3D_SIZE found in file".to_string());
    }

    let expected_size = lut_size * lut_size * lut_size;
    if data.len() != expected_size {
        return Err(format!(
            "Data size mismatch: expected {expected_size}, got {}",
            data.len()
        ));
    }

    let mut lut = Lut3d::new(lut_size);
    lut.data = data;
    lut.domain_min = domain_min;
    lut.domain_max = domain_max;
    lut.title = title;

    Ok(lut)
}

/// Parse a .3dl format LUT file (Autodesk/Lustre).
pub fn parse_3dl_file(path: &Path) -> Result<Lut3d, String> {
    let mut file = File::open(path).map_err(|e| format!("Failed to open file: {e}"))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| format!("Failed to read file: {e}"))?;

    // .3dl files are typically 33x33x33 and use integer values 0-4095
    const SIZE: usize = 33;
    let mut lut = Lut3d::new(SIZE);

    let mut data_count = 0;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let r: u32 = parts[0]
                .parse()
                .map_err(|e| format!("Invalid R value: {e}"))?;
            let g: u32 = parts[1]
                .parse()
                .map_err(|e| format!("Invalid G value: {e}"))?;
            let b: u32 = parts[2]
                .parse()
                .map_err(|e| format!("Invalid B value: {e}"))?;

            // Convert from 0-4095 to 0.0-1.0
            let color = RgbColor::new(r as f64 / 4095.0, g as f64 / 4095.0, b as f64 / 4095.0);

            if data_count < lut.data.len() {
                lut.data[data_count] = color;
                data_count += 1;
            }
        }
    }

    if data_count != SIZE * SIZE * SIZE {
        return Err(format!("Incomplete .3dl data: got {data_count} entries"));
    }

    Ok(lut)
}

/// Parse a CSV format LUT file.
pub fn parse_csv_file(path: &Path) -> Result<Lut3d, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open file: {e}"))?;
    let reader = BufReader::new(file);

    let mut data = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Failed to read line: {e}"))?;
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 3 {
            let r: f64 = parts[0]
                .trim()
                .parse()
                .map_err(|e| format!("Invalid R: {e}"))?;
            let g: f64 = parts[1]
                .trim()
                .parse()
                .map_err(|e| format!("Invalid G: {e}"))?;
            let b: f64 = parts[2]
                .trim()
                .parse()
                .map_err(|e| format!("Invalid B: {e}"))?;
            data.push(RgbColor::new(r, g, b));
        }
    }

    // Infer size from data length
    let total = data.len();
    let size = (total as f64).cbrt().round() as usize;

    if size * size * size != total {
        return Err(format!(
            "Invalid CSV data size: {total} is not a perfect cube"
        ));
    }

    let mut lut = Lut3d::new(size);
    lut.data = data;

    Ok(lut)
}

/// Load a LUT from a file.
pub fn load_lut_file(path: &Path) -> Result<Lut3d, String> {
    let format = LutFormat::from_extension(path)
        .ok_or_else(|| format!("Unknown LUT file format: {}", path.display()))?;

    match format {
        LutFormat::Cube => parse_cube_file(path),
        LutFormat::Threedl => parse_3dl_file(path),
        LutFormat::Csv => parse_csv_file(path),
        LutFormat::Csp => Err("CSP format not yet implemented".to_string()),
    }
}

/// Configuration for 3D LUT filter.
#[derive(Clone, Debug)]
pub struct Lut3dConfig {
    /// Path to LUT file (optional, can use programmatic LUT).
    pub lut_file: Option<String>,
    /// Interpolation method.
    pub interpolation: LutInterpolation,
    /// Color space for LUT processing.
    pub color_space: LutColorSpace,
    /// Optional shaper LUT (1D pre-LUT).
    pub shaper_lut: Option<Lut1d>,
    /// LUT strength/mix (0.0 = bypass, 1.0 = full strength).
    pub strength: f64,
    /// Target output format.
    pub target_format: PixelFormat,
}

impl Lut3dConfig {
    /// Create a new LUT configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            lut_file: None,
            interpolation: LutInterpolation::default(),
            color_space: LutColorSpace::default(),
            shaper_lut: None,
            strength: 1.0,
            target_format: PixelFormat::Rgb24,
        }
    }

    /// Set the LUT file path.
    #[must_use]
    pub fn with_file(mut self, path: impl Into<String>) -> Self {
        self.lut_file = Some(path.into());
        self
    }

    /// Set the interpolation method.
    #[must_use]
    pub fn with_interpolation(mut self, interpolation: LutInterpolation) -> Self {
        self.interpolation = interpolation;
        self
    }

    /// Set the color space.
    #[must_use]
    pub fn with_color_space(mut self, color_space: LutColorSpace) -> Self {
        self.color_space = color_space;
        self
    }

    /// Set the shaper LUT.
    #[must_use]
    pub fn with_shaper(mut self, shaper: Lut1d) -> Self {
        self.shaper_lut = Some(shaper);
        self
    }

    /// Set the LUT strength.
    #[must_use]
    pub fn with_strength(mut self, strength: f64) -> Self {
        self.strength = strength.clamp(0.0, 1.0);
        self
    }

    /// Set the target format.
    #[must_use]
    pub fn with_target_format(mut self, format: PixelFormat) -> Self {
        self.target_format = format;
        self
    }
}

impl Default for Lut3dConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// 3D LUT filter for color grading.
pub struct Lut3dFilter {
    id: NodeId,
    name: String,
    state: NodeState,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
    config: Lut3dConfig,
    lut: Lut3d,
}

impl Lut3dFilter {
    /// Create a new 3D LUT filter.
    pub fn new(id: NodeId, name: impl Into<String>, config: Lut3dConfig) -> Result<Self, String> {
        // Load or create LUT
        let lut = if let Some(ref path) = config.lut_file {
            load_lut_file(Path::new(path))?
        } else {
            // Create identity LUT if no file specified
            Lut3d::identity(33)
        };

        // Validate LUT
        let warnings = lut.validate();
        if !warnings.is_empty() {
            return Err(format!("LUT validation failed: {}", warnings.join(", ")));
        }

        let output_format = PortFormat::Video(VideoPortFormat::new(config.target_format));

        Ok(Self {
            id,
            name: name.into(),
            state: NodeState::Idle,
            inputs: vec![InputPort::new(PortId(0), "input", PortType::Video)
                .with_format(PortFormat::Video(VideoPortFormat::any()))],
            outputs: vec![
                OutputPort::new(PortId(0), "output", PortType::Video).with_format(output_format)
            ],
            config,
            lut,
        })
    }

    /// Create with an existing LUT.
    #[must_use]
    pub fn with_lut(id: NodeId, name: impl Into<String>, config: Lut3dConfig, lut: Lut3d) -> Self {
        let output_format = PortFormat::Video(VideoPortFormat::new(config.target_format));

        Self {
            id,
            name: name.into(),
            state: NodeState::Idle,
            inputs: vec![InputPort::new(PortId(0), "input", PortType::Video)
                .with_format(PortFormat::Video(VideoPortFormat::any()))],
            outputs: vec![
                OutputPort::new(PortId(0), "output", PortType::Video).with_format(output_format)
            ],
            config,
            lut,
        }
    }

    /// Get the current configuration.
    #[must_use]
    pub fn config(&self) -> &Lut3dConfig {
        &self.config
    }

    /// Get the current LUT.
    #[must_use]
    pub fn lut(&self) -> &Lut3d {
        &self.lut
    }

    /// Apply LUT to a single pixel.
    fn apply_lut_to_pixel(&self, r: u8, g: u8, b: u8) -> (u8, u8, u8) {
        // Convert to normalized RGB
        let mut color = RgbColor::from_u8(r, g, b);

        // Apply color space transform to linear/log
        color = RgbColor::new(
            self.config.color_space.inverse(color.r),
            self.config.color_space.inverse(color.g),
            self.config.color_space.inverse(color.b),
        );

        // Apply shaper LUT if present
        if let Some(ref shaper) = self.config.shaper_lut {
            color = shaper.apply(color);
        }

        // Apply 3D LUT with selected interpolation
        let lut_output = match self.config.interpolation {
            LutInterpolation::Nearest => self.lut.apply_nearest(color),
            LutInterpolation::Trilinear => self.lut.apply_trilinear(color),
            LutInterpolation::Tetrahedral => self.lut.apply_tetrahedral(color),
        };

        // Mix with original based on strength
        let mixed = if (self.config.strength - 1.0).abs() < 0.001 {
            lut_output
        } else {
            color.lerp(&lut_output, self.config.strength)
        };

        // Apply color space transform back
        let output = RgbColor::new(
            self.config.color_space.forward(mixed.r),
            self.config.color_space.forward(mixed.g),
            self.config.color_space.forward(mixed.b),
        );

        output.clamp().to_u8()
    }

    /// Convert YUV to RGB.
    fn yuv_to_rgb(&self, frame: &VideoFrame) -> Vec<u8> {
        let width = frame.width as usize;
        let height = frame.height as usize;

        let y_plane = frame.planes.first();
        let u_plane = frame.planes.get(1);
        let v_plane = frame.planes.get(2);

        let (h_sub, v_sub) = frame.format.chroma_subsampling();
        let mut rgb_data = vec![0u8; width * height * 3];

        // BT.709 matrix for YUV to RGB
        const KR: f64 = 0.2126;
        const KB: f64 = 0.0722;

        for y in 0..height {
            for x in 0..width {
                let y_val = y_plane
                    .map(|p| p.row(y).get(x).copied().unwrap_or(16))
                    .unwrap_or(16) as f64;

                let chroma_x = x / h_sub as usize;
                let chroma_y = y / v_sub as usize;

                let u_val = u_plane
                    .map(|p| p.row(chroma_y).get(chroma_x).copied().unwrap_or(128))
                    .unwrap_or(128) as f64;
                let v_val = v_plane
                    .map(|p| p.row(chroma_y).get(chroma_x).copied().unwrap_or(128))
                    .unwrap_or(128) as f64;

                // YUV to RGB conversion (limited range)
                let y_norm = (y_val - 16.0) * 255.0 / 219.0;
                let cb = (u_val - 128.0) * 255.0 / 224.0;
                let cr = (v_val - 128.0) * 255.0 / 224.0;

                let r = y_norm + 1.5748 * cr;
                let g = y_norm - 0.1873 * cb - 0.4681 * cr;
                let b = y_norm + 1.8556 * cb;

                let offset = (y * width + x) * 3;
                rgb_data[offset] = r.clamp(0.0, 255.0) as u8;
                rgb_data[offset + 1] = g.clamp(0.0, 255.0) as u8;
                rgb_data[offset + 2] = b.clamp(0.0, 255.0) as u8;
            }
        }

        rgb_data
    }

    /// Convert RGB to YUV.
    fn rgb_to_yuv(
        &self,
        rgb_data: &[u8],
        width: usize,
        height: usize,
    ) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        // BT.709 matrix for RGB to YUV
        const KR: f64 = 0.2126;
        const KB: f64 = 0.0722;

        let mut y_data = vec![0u8; width * height];
        let chroma_width = width / 2;
        let chroma_height = height / 2;
        let mut u_data = vec![128u8; chroma_width * chroma_height];
        let mut v_data = vec![128u8; chroma_width * chroma_height];

        for y in 0..height {
            for x in 0..width {
                let offset = (y * width + x) * 3;
                let r = rgb_data[offset] as f64;
                let g = rgb_data[offset + 1] as f64;
                let b = rgb_data[offset + 2] as f64;

                // RGB to YUV conversion (limited range)
                let y_val = KR * r + (1.0 - KR - KB) * g + KB * b;
                let cb = (b - y_val) / 1.8556;
                let cr = (r - y_val) / 1.5748;

                let y_out = y_val * 219.0 / 255.0 + 16.0;
                let cb_out = cb * 224.0 / 255.0 + 128.0;
                let cr_out = cr * 224.0 / 255.0 + 128.0;

                y_data[y * width + x] = y_out.clamp(16.0, 235.0) as u8;

                // Subsample chroma (4:2:0)
                if x % 2 == 0 && y % 2 == 0 {
                    let chroma_x = x / 2;
                    let chroma_y = y / 2;
                    u_data[chroma_y * chroma_width + chroma_x] = cb_out.clamp(16.0, 240.0) as u8;
                    v_data[chroma_y * chroma_width + chroma_x] = cr_out.clamp(16.0, 240.0) as u8;
                }
            }
        }

        (y_data, u_data, v_data)
    }

    /// Process RGB frame.
    fn process_rgb(&self, input: &VideoFrame) -> GraphResult<VideoFrame> {
        let width = input.width as usize;
        let height = input.height as usize;

        let src_plane = input
            .planes
            .first()
            .ok_or_else(|| GraphError::ConfigurationError("Missing RGB plane".to_string()))?;

        let src_bpp = if input.format == PixelFormat::Rgba32 {
            4
        } else {
            3
        };

        let mut output_rgb = vec![0u8; width * height * 3];

        // Process each pixel
        for y in 0..height {
            for x in 0..width {
                let row = src_plane.row(y);
                let offset = x * src_bpp;

                let r = row.get(offset).copied().unwrap_or(0);
                let g = row.get(offset + 1).copied().unwrap_or(0);
                let b = row.get(offset + 2).copied().unwrap_or(0);

                let (r_out, g_out, b_out) = self.apply_lut_to_pixel(r, g, b);

                let out_offset = (y * width + x) * 3;
                output_rgb[out_offset] = r_out;
                output_rgb[out_offset + 1] = g_out;
                output_rgb[out_offset + 2] = b_out;
            }
        }

        // Convert to target format
        if self.config.target_format.is_yuv() {
            let (y_data, u_data, v_data) = self.rgb_to_yuv(&output_rgb, width, height);

            let mut output = VideoFrame::new(self.config.target_format, input.width, input.height);
            output.timestamp = input.timestamp;
            output.frame_type = input.frame_type;
            output.color_info = ColorInfo {
                full_range: false,
                ..input.color_info
            };

            let chroma_width = width / 2;
            output.planes.push(Plane::new(y_data, width));
            output.planes.push(Plane::new(u_data, chroma_width));
            output.planes.push(Plane::new(v_data, chroma_width));

            Ok(output)
        } else {
            let mut output = VideoFrame::new(self.config.target_format, input.width, input.height);
            output.timestamp = input.timestamp;
            output.frame_type = input.frame_type;
            output.color_info = input.color_info;
            output.planes.push(Plane::new(output_rgb, width * 3));

            Ok(output)
        }
    }

    /// Process YUV frame.
    fn process_yuv(&self, input: &VideoFrame) -> GraphResult<VideoFrame> {
        let width = input.width as usize;
        let height = input.height as usize;

        // Convert to RGB
        let rgb_data = self.yuv_to_rgb(input);

        // Process each pixel
        let mut output_rgb = vec![0u8; width * height * 3];

        for y in 0..height {
            for x in 0..width {
                let offset = (y * width + x) * 3;
                let r = rgb_data[offset];
                let g = rgb_data[offset + 1];
                let b = rgb_data[offset + 2];

                let (r_out, g_out, b_out) = self.apply_lut_to_pixel(r, g, b);

                output_rgb[offset] = r_out;
                output_rgb[offset + 1] = g_out;
                output_rgb[offset + 2] = b_out;
            }
        }

        // Convert to target format
        if self.config.target_format.is_yuv() {
            let (y_data, u_data, v_data) = self.rgb_to_yuv(&output_rgb, width, height);

            let mut output = VideoFrame::new(self.config.target_format, input.width, input.height);
            output.timestamp = input.timestamp;
            output.frame_type = input.frame_type;
            output.color_info = ColorInfo {
                full_range: false,
                ..input.color_info
            };

            let chroma_width = width / 2;
            output.planes.push(Plane::new(y_data, width));
            output.planes.push(Plane::new(u_data, chroma_width));
            output.planes.push(Plane::new(v_data, chroma_width));

            Ok(output)
        } else {
            let mut output = VideoFrame::new(self.config.target_format, input.width, input.height);
            output.timestamp = input.timestamp;
            output.frame_type = input.frame_type;
            output.color_info = input.color_info;
            output.planes.push(Plane::new(output_rgb, width * 3));

            Ok(output)
        }
    }

    /// Apply LUT to a frame.
    fn apply_lut(&self, input: &VideoFrame) -> GraphResult<VideoFrame> {
        if input.format.is_yuv() {
            self.process_yuv(input)
        } else {
            self.process_rgb(input)
        }
    }
}

impl Node for Lut3dFilter {
    fn id(&self) -> NodeId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn node_type(&self) -> NodeType {
        NodeType::Filter
    }

    fn state(&self) -> NodeState {
        self.state
    }

    fn set_state(&mut self, state: NodeState) -> GraphResult<()> {
        if !self.state.can_transition_to(state) {
            return Err(GraphError::InvalidStateTransition {
                node: self.id,
                from: self.state.to_string(),
                to: state.to_string(),
            });
        }
        self.state = state;
        Ok(())
    }

    fn inputs(&self) -> &[InputPort] {
        &self.inputs
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn process(&mut self, input: Option<FilterFrame>) -> GraphResult<Option<FilterFrame>> {
        match input {
            Some(FilterFrame::Video(frame)) => {
                let processed = self.apply_lut(&frame)?;
                Ok(Some(FilterFrame::Video(processed)))
            }
            Some(_) => Err(GraphError::PortTypeMismatch {
                expected: "Video".to_string(),
                actual: "Audio".to_string(),
            }),
            None => Ok(None),
        }
    }
}

/// GPU acceleration hints for 3D LUT processing.
/// These can be used by GPU backends to optimize LUT application.
#[derive(Clone, Debug)]
pub struct GpuLutHints {
    /// Whether to use GPU texture for LUT storage.
    pub use_texture_3d: bool,
    /// Prefer compute shader over fragment shader.
    pub prefer_compute: bool,
    /// Cache LUT on GPU.
    pub cache_on_gpu: bool,
}

impl Default for GpuLutHints {
    fn default() -> Self {
        Self {
            use_texture_3d: true,
            prefer_compute: false,
            cache_on_gpu: true,
        }
    }
}

/// Analyze a LUT for statistics and characteristics.
#[derive(Clone, Debug, Default)]
pub struct LutAnalysis {
    /// Average change magnitude.
    pub avg_change: f64,
    /// Maximum change magnitude.
    pub max_change: f64,
    /// Is approximately identity.
    pub is_identity: bool,
    /// Dynamic range min.
    pub range_min: RgbColor,
    /// Dynamic range max.
    pub range_max: RgbColor,
}

impl LutAnalysis {
    /// Analyze a 3D LUT.
    #[must_use]
    pub fn analyze(lut: &Lut3d) -> Self {
        let mut total_change = 0.0;
        let mut max_change: f64 = 0.0;
        let mut min_vals = RgbColor::new(f64::MAX, f64::MAX, f64::MAX);
        let mut max_vals = RgbColor::new(f64::MIN, f64::MIN, f64::MIN);
        let mut is_identity = true;

        let size = lut.size;
        let count = size * size * size;

        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    let input = RgbColor::new(
                        r as f64 / (size - 1) as f64,
                        g as f64 / (size - 1) as f64,
                        b as f64 / (size - 1) as f64,
                    );

                    let output = lut.get(r, g, b);

                    // Calculate change
                    let change = ((output.r - input.r).powi(2)
                        + (output.g - input.g).powi(2)
                        + (output.b - input.b).powi(2))
                    .sqrt();

                    total_change += change;
                    max_change = max_change.max(change);

                    if change > 0.01 {
                        is_identity = false;
                    }

                    // Track range
                    min_vals.r = min_vals.r.min(output.r);
                    min_vals.g = min_vals.g.min(output.g);
                    min_vals.b = min_vals.b.min(output.b);

                    max_vals.r = max_vals.r.max(output.r);
                    max_vals.g = max_vals.g.max(output.g);
                    max_vals.b = max_vals.b.max(output.b);
                }
            }
        }

        Self {
            avg_change: total_change / count as f64,
            max_change,
            is_identity,
            range_min: min_vals,
            range_max: max_vals,
        }
    }
}

/// LUT blending and mixing operations.
impl Lut3d {
    /// Blend two LUTs with a given weight.
    /// weight = 0.0 returns self, weight = 1.0 returns other.
    #[must_use]
    pub fn blend(&self, other: &Self, weight: f64) -> Self {
        assert_eq!(self.size, other.size, "LUT sizes must match for blending");

        let mut result = Self::new(self.size);
        result.domain_min = self.domain_min.lerp(&other.domain_min, weight);
        result.domain_max = self.domain_max.lerp(&other.domain_max, weight);

        for i in 0..self.data.len() {
            result.data[i] = self.data[i].lerp(&other.data[i], weight);
        }

        result
    }

    /// Layer two LUTs (apply first, then second).
    /// This is similar to compose but preserves the original LUT sizes.
    #[must_use]
    pub fn layer(&self, top: &Self) -> Self {
        self.compose(top)
    }

    /// Mix two LUTs using different blend modes.
    #[must_use]
    pub fn mix(&self, other: &Self, mode: LutBlendMode, opacity: f64) -> Self {
        assert_eq!(self.size, other.size, "LUT sizes must match for mixing");

        let mut result = Self::new(self.size);
        result.domain_min = self.domain_min;
        result.domain_max = self.domain_max;

        let opacity = opacity.clamp(0.0, 1.0);

        for i in 0..self.data.len() {
            let base = self.data[i];
            let blend = other.data[i];

            let mixed = match mode {
                LutBlendMode::Normal => blend,
                LutBlendMode::Multiply => {
                    RgbColor::new(base.r * blend.r, base.g * blend.g, base.b * blend.b)
                }
                LutBlendMode::Screen => RgbColor::new(
                    1.0 - (1.0 - base.r) * (1.0 - blend.r),
                    1.0 - (1.0 - base.g) * (1.0 - blend.g),
                    1.0 - (1.0 - base.b) * (1.0 - blend.b),
                ),
                LutBlendMode::Overlay => RgbColor::new(
                    overlay_blend(base.r, blend.r),
                    overlay_blend(base.g, blend.g),
                    overlay_blend(base.b, blend.b),
                ),
                LutBlendMode::Add => RgbColor::new(
                    (base.r + blend.r).min(1.0),
                    (base.g + blend.g).min(1.0),
                    (base.b + blend.b).min(1.0),
                ),
                LutBlendMode::Subtract => RgbColor::new(
                    (base.r - blend.r).max(0.0),
                    (base.g - blend.g).max(0.0),
                    (base.b - blend.b).max(0.0),
                ),
            };

            result.data[i] = base.lerp(&mixed, opacity);
        }

        result
    }

    /// Adjust the strength/intensity of the LUT.
    #[must_use]
    pub fn adjust_strength(&self, strength: f64) -> Self {
        let identity = Self::identity(self.size);
        self.blend(&identity, 1.0 - strength.clamp(0.0, 1.0))
    }

    /// Resize the LUT to a different cube size.
    #[must_use]
    pub fn resize(&self, new_size: usize) -> Self {
        let mut result = Self::new(new_size);
        result.domain_min = self.domain_min;
        result.domain_max = self.domain_max;

        for r in 0..new_size {
            for g in 0..new_size {
                for b in 0..new_size {
                    let input = RgbColor::new(
                        r as f64 / (new_size - 1) as f64,
                        g as f64 / (new_size - 1) as f64,
                        b as f64 / (new_size - 1) as f64,
                    );

                    let output = self.apply_trilinear(input);
                    result.set(r, g, b, output);
                }
            }
        }

        result
    }
}

/// Blend modes for LUT mixing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LutBlendMode {
    /// Normal blending.
    Normal,
    /// Multiply blending.
    Multiply,
    /// Screen blending.
    Screen,
    /// Overlay blending.
    Overlay,
    /// Additive blending.
    Add,
    /// Subtractive blending.
    Subtract,
}

/// Helper function for overlay blend mode.
fn overlay_blend(base: f64, blend: f64) -> f64 {
    if base < 0.5 {
        2.0 * base * blend
    } else {
        1.0 - 2.0 * (1.0 - base) * (1.0 - blend)
    }
}

/// Export a LUT to .cube format.
pub fn export_cube_file(lut: &Lut3d, path: &Path, title: Option<&str>) -> Result<(), String> {
    use std::io::Write;

    let mut file = File::create(path).map_err(|e| format!("Failed to create file: {e}"))?;

    // Write header
    if let Some(title_str) = title {
        writeln!(file, "TITLE \"{title_str}\"").map_err(|e| format!("Write failed: {e}"))?;
    } else if !lut.title.is_empty() {
        writeln!(file, "TITLE \"{}\"", lut.title).map_err(|e| format!("Write failed: {e}"))?;
    }

    writeln!(file, "LUT_3D_SIZE {}", lut.size).map_err(|e| format!("Write failed: {e}"))?;

    // Write domain if not default
    if lut.domain_min != RgbColor::new(0.0, 0.0, 0.0)
        || lut.domain_max != RgbColor::new(1.0, 1.0, 1.0)
    {
        writeln!(
            file,
            "DOMAIN_MIN {:.6} {:.6} {:.6}",
            lut.domain_min.r, lut.domain_min.g, lut.domain_min.b
        )
        .map_err(|e| format!("Write failed: {e}"))?;

        writeln!(
            file,
            "DOMAIN_MAX {:.6} {:.6} {:.6}",
            lut.domain_max.r, lut.domain_max.g, lut.domain_max.b
        )
        .map_err(|e| format!("Write failed: {e}"))?;
    }

    // Write data
    for color in &lut.data {
        writeln!(file, "{:.6} {:.6} {:.6}", color.r, color.g, color.b)
            .map_err(|e| format!("Write failed: {e}"))?;
    }

    Ok(())
}

/// Export a LUT to .3dl format.
pub fn export_3dl_file(lut: &Lut3d, path: &Path) -> Result<(), String> {
    use std::io::Write;

    // .3dl format is typically 33x33x33
    let export_lut = if lut.size != 33 {
        lut.resize(33)
    } else {
        lut.clone()
    };

    let mut file = File::create(path).map_err(|e| format!("Failed to create file: {e}"))?;

    // Write data (integer values 0-4095)
    for color in &export_lut.data {
        let r = (color.r.clamp(0.0, 1.0) * 4095.0).round() as u32;
        let g = (color.g.clamp(0.0, 1.0) * 4095.0).round() as u32;
        let b = (color.b.clamp(0.0, 1.0) * 4095.0).round() as u32;
        writeln!(file, "{r} {g} {b}").map_err(|e| format!("Write failed: {e}"))?;
    }

    Ok(())
}

/// Export a LUT to CSV format.
pub fn export_csv_file(lut: &Lut3d, path: &Path) -> Result<(), String> {
    use std::io::Write;

    let mut file = File::create(path).map_err(|e| format!("Failed to create file: {e}"))?;

    // Write header
    writeln!(file, "R,G,B").map_err(|e| format!("Write failed: {e}"))?;

    // Write data
    for color in &lut.data {
        writeln!(file, "{:.6},{:.6},{:.6}", color.r, color.g, color.b)
            .map_err(|e| format!("Write failed: {e}"))?;
    }

    Ok(())
}

/// Procedural LUT generation functions.
pub mod procedural {
    use super::*;

    /// Generate a contrast adjustment LUT.
    #[must_use]
    pub fn contrast_lut(size: usize, contrast: f64) -> Lut3d {
        let mut lut = Lut3d::new(size);

        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    let input = RgbColor::new(
                        r as f64 / (size - 1) as f64,
                        g as f64 / (size - 1) as f64,
                        b as f64 / (size - 1) as f64,
                    );

                    let output = RgbColor::new(
                        apply_contrast(input.r, contrast),
                        apply_contrast(input.g, contrast),
                        apply_contrast(input.b, contrast),
                    );

                    lut.set(r, g, b, output);
                }
            }
        }

        lut
    }

    /// Generate a saturation adjustment LUT.
    #[must_use]
    pub fn saturation_lut(size: usize, saturation: f64) -> Lut3d {
        let mut lut = Lut3d::new(size);

        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    let input = RgbColor::new(
                        r as f64 / (size - 1) as f64,
                        g as f64 / (size - 1) as f64,
                        b as f64 / (size - 1) as f64,
                    );

                    // Calculate luminance
                    let luma = 0.2126 * input.r + 0.7152 * input.g + 0.0722 * input.b;

                    let output = RgbColor::new(
                        luma + (input.r - luma) * saturation,
                        luma + (input.g - luma) * saturation,
                        luma + (input.b - luma) * saturation,
                    );

                    lut.set(r, g, b, output.clamp());
                }
            }
        }

        lut
    }

    /// Generate a hue shift LUT.
    #[must_use]
    pub fn hue_shift_lut(size: usize, hue_shift: f64) -> Lut3d {
        let mut lut = Lut3d::new(size);

        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    let input = RgbColor::new(
                        r as f64 / (size - 1) as f64,
                        g as f64 / (size - 1) as f64,
                        b as f64 / (size - 1) as f64,
                    );

                    let output = shift_hue(input, hue_shift);
                    lut.set(r, g, b, output.clamp());
                }
            }
        }

        lut
    }

    /// Generate a temperature adjustment LUT.
    #[must_use]
    pub fn temperature_lut(size: usize, temperature: f64) -> Lut3d {
        let mut lut = Lut3d::new(size);

        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    let input = RgbColor::new(
                        r as f64 / (size - 1) as f64,
                        g as f64 / (size - 1) as f64,
                        b as f64 / (size - 1) as f64,
                    );

                    let output = if temperature > 0.0 {
                        // Warm
                        RgbColor::new(
                            input.r * (1.0 + temperature * 0.1),
                            input.g,
                            input.b * (1.0 - temperature * 0.1),
                        )
                    } else {
                        // Cool
                        RgbColor::new(
                            input.r * (1.0 + temperature * 0.1),
                            input.g,
                            input.b * (1.0 - temperature * 0.1),
                        )
                    };

                    lut.set(r, g, b, output.clamp());
                }
            }
        }

        lut
    }

    /// Generate a vibrance adjustment LUT.
    #[must_use]
    pub fn vibrance_lut(size: usize, vibrance: f64) -> Lut3d {
        let mut lut = Lut3d::new(size);

        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    let input = RgbColor::new(
                        r as f64 / (size - 1) as f64,
                        g as f64 / (size - 1) as f64,
                        b as f64 / (size - 1) as f64,
                    );

                    let output = apply_vibrance(input, vibrance);
                    lut.set(r, g, b, output.clamp());
                }
            }
        }

        lut
    }

    /// Generate an exposure adjustment LUT.
    #[must_use]
    pub fn exposure_lut(size: usize, exposure: f64) -> Lut3d {
        let mut lut = Lut3d::new(size);
        let multiplier = 2_f64.powf(exposure);

        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    let input = RgbColor::new(
                        r as f64 / (size - 1) as f64,
                        g as f64 / (size - 1) as f64,
                        b as f64 / (size - 1) as f64,
                    );

                    let output = RgbColor::new(
                        input.r * multiplier,
                        input.g * multiplier,
                        input.b * multiplier,
                    );

                    lut.set(r, g, b, output.clamp());
                }
            }
        }

        lut
    }

    /// Generate a sepia tone LUT.
    #[must_use]
    pub fn sepia_lut(size: usize, strength: f64) -> Lut3d {
        let mut lut = Lut3d::new(size);

        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    let input = RgbColor::new(
                        r as f64 / (size - 1) as f64,
                        g as f64 / (size - 1) as f64,
                        b as f64 / (size - 1) as f64,
                    );

                    let sepia_r = input.r * 0.393 + input.g * 0.769 + input.b * 0.189;
                    let sepia_g = input.r * 0.349 + input.g * 0.686 + input.b * 0.168;
                    let sepia_b = input.r * 0.272 + input.g * 0.534 + input.b * 0.131;

                    let sepia = RgbColor::new(sepia_r, sepia_g, sepia_b);
                    let output = input.lerp(&sepia, strength);

                    lut.set(r, g, b, output.clamp());
                }
            }
        }

        lut
    }

    /// Generate a bleach bypass LUT.
    #[must_use]
    pub fn bleach_bypass_lut(size: usize, strength: f64) -> Lut3d {
        let mut lut = Lut3d::new(size);

        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    let input = RgbColor::new(
                        r as f64 / (size - 1) as f64,
                        g as f64 / (size - 1) as f64,
                        b as f64 / (size - 1) as f64,
                    );

                    let luma = 0.2126 * input.r + 0.7152 * input.g + 0.0722 * input.b;

                    // Bleach bypass blends the image with its luminance
                    let bleach = RgbColor::new(
                        overlay_blend(input.r, luma),
                        overlay_blend(input.g, luma),
                        overlay_blend(input.b, luma),
                    );

                    let output = input.lerp(&bleach, strength);
                    lut.set(r, g, b, output.clamp());
                }
            }
        }

        lut
    }

    /// Generate a channel mixer LUT.
    #[must_use]
    pub fn channel_mixer_lut(
        size: usize,
        rr: f64,
        rg: f64,
        rb: f64,
        gr: f64,
        gg: f64,
        gb: f64,
        br: f64,
        bg: f64,
        bb: f64,
    ) -> Lut3d {
        let mut lut = Lut3d::new(size);

        for r in 0..size {
            for g in 0..size {
                for b in 0..size {
                    let input = RgbColor::new(
                        r as f64 / (size - 1) as f64,
                        g as f64 / (size - 1) as f64,
                        b as f64 / (size - 1) as f64,
                    );

                    let output = RgbColor::new(
                        input.r * rr + input.g * rg + input.b * rb,
                        input.r * gr + input.g * gg + input.b * gb,
                        input.r * br + input.g * bg + input.b * bb,
                    );

                    lut.set(r, g, b, output.clamp());
                }
            }
        }

        lut
    }

    fn apply_contrast(value: f64, contrast: f64) -> f64 {
        ((value - 0.5) * contrast + 0.5).clamp(0.0, 1.0)
    }

    fn shift_hue(color: RgbColor, hue_shift: f64) -> RgbColor {
        let (h, s, v) = rgb_to_hsv(color);
        let new_h = (h + hue_shift).rem_euclid(360.0);
        hsv_to_rgb(new_h, s, v)
    }

    fn apply_vibrance(color: RgbColor, vibrance: f64) -> RgbColor {
        let max_val = color.r.max(color.g).max(color.b);
        let min_val = color.r.min(color.g).min(color.b);
        let saturation = if max_val > 0.0 {
            (max_val - min_val) / max_val
        } else {
            0.0
        };

        // Apply vibrance more to less saturated colors
        let adjustment = vibrance * (1.0 - saturation);
        let luma = 0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b;

        RgbColor::new(
            luma + (color.r - luma) * (1.0 + adjustment),
            luma + (color.g - luma) * (1.0 + adjustment),
            luma + (color.b - luma) * (1.0 + adjustment),
        )
    }

    fn rgb_to_hsv(color: RgbColor) -> (f64, f64, f64) {
        let max_val = color.r.max(color.g).max(color.b);
        let min_val = color.r.min(color.g).min(color.b);
        let delta = max_val - min_val;

        let v = max_val;
        let s = if max_val > 0.0 { delta / max_val } else { 0.0 };

        let h = if delta == 0.0 {
            0.0
        } else if (max_val - color.r).abs() < f64::EPSILON {
            60.0 * (((color.g - color.b) / delta).rem_euclid(6.0))
        } else if (max_val - color.g).abs() < f64::EPSILON {
            60.0 * (((color.b - color.r) / delta) + 2.0)
        } else {
            60.0 * (((color.r - color.g) / delta) + 4.0)
        };

        (h, s, v)
    }

    fn hsv_to_rgb(h: f64, s: f64, v: f64) -> RgbColor {
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
        let m = v - c;

        let (r, g, b) = if h < 60.0 {
            (c, x, 0.0)
        } else if h < 120.0 {
            (x, c, 0.0)
        } else if h < 180.0 {
            (0.0, c, x)
        } else if h < 240.0 {
            (0.0, x, c)
        } else if h < 300.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        RgbColor::new(r + m, g + m, b + m)
    }
}

/// LUT caching for performance optimization.
#[derive(Clone, Debug)]
pub struct LutCache {
    /// Cached LUTs by path.
    cache: std::collections::HashMap<String, Lut3d>,
    /// Maximum cache size.
    max_size: usize,
}

impl LutCache {
    /// Create a new LUT cache.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: std::collections::HashMap::new(),
            max_size,
        }
    }

    /// Get a LUT from cache or load it.
    pub fn get_or_load(&mut self, path: &str) -> Result<Lut3d, String> {
        if let Some(lut) = self.cache.get(path) {
            Ok(lut.clone())
        } else {
            let lut = load_lut_file(Path::new(path))?;

            // Evict oldest if cache is full
            if self.cache.len() >= self.max_size {
                if let Some(key) = self.cache.keys().next().cloned() {
                    self.cache.remove(&key);
                }
            }

            self.cache.insert(path.to_string(), lut.clone());
            Ok(lut)
        }
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Get cache statistics.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            entries: self.cache.len(),
            max_size: self.max_size,
        }
    }
}

impl Default for LutCache {
    fn default() -> Self {
        Self::new(10)
    }
}

/// Cache statistics.
#[derive(Clone, Copy, Debug)]
pub struct CacheStats {
    /// Number of cached entries.
    pub entries: usize,
    /// Maximum cache size.
    pub max_size: usize,
}

/// Advanced LUT manipulation utilities.
pub mod utils {
    use super::*;

    /// Extract a 1D LUT from a 3D LUT along a specific axis.
    #[must_use]
    pub fn extract_1d_lut(lut: &Lut3d, channel: ColorChannel) -> Lut1d {
        let size = lut.size;
        let mut lut_1d = Lut1d::new(size);

        for i in 0..size {
            let t = i as f64 / (size - 1) as f64;
            let color = match channel {
                ColorChannel::Red => RgbColor::new(t, 0.0, 0.0),
                ColorChannel::Green => RgbColor::new(0.0, t, 0.0),
                ColorChannel::Blue => RgbColor::new(0.0, 0.0, t),
                ColorChannel::Luminance => RgbColor::new(t, t, t),
            };

            let result = lut.apply_trilinear(color);

            lut_1d.r_lut[i] = result.r;
            lut_1d.g_lut[i] = result.g;
            lut_1d.b_lut[i] = result.b;
        }

        lut_1d
    }

    /// Smooth a LUT by applying a simple averaging filter.
    #[must_use]
    pub fn smooth_lut(lut: &Lut3d, iterations: usize) -> Lut3d {
        let mut result = lut.clone();

        for _ in 0..iterations {
            let mut new_data = result.data.clone();

            for r in 1..(result.size - 1) {
                for g in 1..(result.size - 1) {
                    for b in 1..(result.size - 1) {
                        let mut sum = RgbColor::new(0.0, 0.0, 0.0);
                        let mut count = 0.0;

                        // Average with neighbors
                        for dr in -1_i32..=1 {
                            for dg in -1_i32..=1 {
                                for db in -1_i32..=1 {
                                    let nr = (r as i32 + dr) as usize;
                                    let ng = (g as i32 + dg) as usize;
                                    let nb = (b as i32 + db) as usize;

                                    let color = result.get(nr, ng, nb);
                                    sum.r += color.r;
                                    sum.g += color.g;
                                    sum.b += color.b;
                                    count += 1.0;
                                }
                            }
                        }

                        let idx = result.index(r, g, b);
                        new_data[idx] = RgbColor::new(sum.r / count, sum.g / count, sum.b / count);
                    }
                }
            }

            result.data = new_data;
        }

        result
    }

    /// Detect if a LUT is monotonic (values always increase).
    #[must_use]
    pub fn is_monotonic(lut: &Lut3d) -> bool {
        let size = lut.size;

        for r in 0..(size - 1) {
            for g in 0..(size - 1) {
                for b in 0..(size - 1) {
                    let curr = lut.get(r, g, b);
                    let next_r = lut.get(r + 1, g, b);
                    let next_g = lut.get(r, g + 1, b);
                    let next_b = lut.get(r, g, b + 1);

                    if next_r.r < curr.r || next_g.g < curr.g || next_b.b < curr.b {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Clamp all LUT values to a specific range.
    #[must_use]
    pub fn clamp_lut(lut: &Lut3d, min: f64, max: f64) -> Lut3d {
        let mut result = lut.clone();

        for color in &mut result.data {
            color.r = color.r.clamp(min, max);
            color.g = color.g.clamp(min, max);
            color.b = color.b.clamp(min, max);
        }

        result
    }

    /// Normalize a LUT to use the full [0, 1] range.
    #[must_use]
    pub fn normalize_lut(lut: &Lut3d) -> Lut3d {
        let mut result = lut.clone();

        let mut min_vals = RgbColor::new(f64::MAX, f64::MAX, f64::MAX);
        let mut max_vals = RgbColor::new(f64::MIN, f64::MIN, f64::MIN);

        // Find range
        for color in &lut.data {
            min_vals.r = min_vals.r.min(color.r);
            min_vals.g = min_vals.g.min(color.g);
            min_vals.b = min_vals.b.min(color.b);

            max_vals.r = max_vals.r.max(color.r);
            max_vals.g = max_vals.g.max(color.g);
            max_vals.b = max_vals.b.max(color.b);
        }

        // Normalize
        for color in &mut result.data {
            if max_vals.r > min_vals.r {
                color.r = (color.r - min_vals.r) / (max_vals.r - min_vals.r);
            }
            if max_vals.g > min_vals.g {
                color.g = (color.g - min_vals.g) / (max_vals.g - min_vals.g);
            }
            if max_vals.b > min_vals.b {
                color.b = (color.b - min_vals.b) / (max_vals.b - min_vals.b);
            }
        }

        result
    }
}

/// Color channel identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorChannel {
    /// Red channel.
    Red,
    /// Green channel.
    Green,
    /// Blue channel.
    Blue,
    /// Luminance (grayscale).
    Luminance,
}
