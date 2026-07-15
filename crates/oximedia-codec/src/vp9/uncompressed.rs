//! VP9 Uncompressed header parsing.

#![allow(clippy::match_same_arms)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::if_not_else)]

use crate::error::{CodecError, CodecResult};
use oximedia_io::BitReader;

/// VP9 frame types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Vp9FrameType {
    /// Keyframe (intra-only).
    #[default]
    Key = 0,
    /// Inter frame.
    Inter = 1,
}

impl Vp9FrameType {
    /// Returns true if this is a keyframe.
    #[must_use]
    pub const fn is_keyframe(&self) -> bool {
        matches!(self, Self::Key)
    }
}

/// VP9 color space specification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ColorSpace {
    /// Unknown or unspecified.
    #[default]
    Unknown = 0,
    /// ITU-R BT.601.
    Bt601 = 1,
    /// ITU-R BT.709.
    Bt709 = 2,
    /// SMPTE 170M.
    Smpte170 = 3,
    /// SMPTE 240M.
    Smpte240 = 4,
    /// ITU-R BT.2020.
    Bt2020 = 5,
    /// Reserved.
    Reserved = 6,
    /// sRGB.
    Srgb = 7,
}

impl From<u8> for ColorSpace {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Unknown,
            1 => Self::Bt601,
            2 => Self::Bt709,
            3 => Self::Smpte170,
            4 => Self::Smpte240,
            5 => Self::Bt2020,
            6 => Self::Reserved,
            7 => Self::Srgb,
            _ => Self::Unknown,
        }
    }
}

impl ColorSpace {
    /// Returns the color space name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Bt601 => "bt601",
            Self::Bt709 => "bt709",
            Self::Smpte170 => "smpte170",
            Self::Smpte240 => "smpte240",
            Self::Bt2020 => "bt2020",
            Self::Reserved => "reserved",
            Self::Srgb => "srgb",
        }
    }
}

/// VP9 Uncompressed header.
#[derive(Clone, Debug, Default)]
pub struct UncompressedHeader {
    /// Frame marker (should be 0b10).
    pub frame_marker: u8,
    /// Profile (0-3).
    pub profile: u8,
    /// Show existing frame flag.
    pub show_existing_frame: bool,
    /// Frame to show index.
    pub frame_to_show: u8,
    /// Frame type.
    pub frame_type: Vp9FrameType,
    /// Show frame flag.
    pub show_frame: bool,
    /// Error resilient mode.
    pub error_resilient: bool,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Render width.
    pub render_width: u32,
    /// Render height.
    pub render_height: u32,
    /// Intra-only flag.
    pub intra_only: bool,
    /// Reset frame context.
    pub reset_frame_context: u8,
    /// Refresh frame flags bitmask.
    pub refresh_frame_flags: u8,
    /// Reference frame indices for LAST, GOLDEN, ALTREF.
    pub ref_frame_idx: [u8; 3],
    /// Reference frame sign bias.
    pub ref_frame_sign_bias: [bool; 4],
    /// Allow high precision motion vectors.
    pub allow_high_precision_mv: bool,
    /// Interpolation filter type.
    pub interp_filter: u8,
    /// Color space specification.
    pub color_space: ColorSpace,
    /// Full range color values.
    pub color_range: bool,
    /// Chroma subsampling X.
    pub subsampling_x: bool,
    /// Chroma subsampling Y.
    pub subsampling_y: bool,
    /// Bit depth (8, 10, or 12).
    pub bit_depth: u8,
    /// Refresh frame context flag.
    pub refresh_frame_context: bool,
    /// Frame parallel decoding mode.
    pub frame_parallel_decoding: bool,
    /// Frame context index (0..=3).
    pub frame_context_idx: u8,
    /// Loop filter parameters.
    pub loop_filter: LoopFilterHeader,
    /// Quantization parameters.
    pub quant: QuantHeader,
    /// Segmentation parameters.
    pub seg: SegmentationHeader,
    /// log2 of tile columns.
    pub tile_cols_log2: u8,
    /// log2 of tile rows.
    pub tile_rows_log2: u8,
    /// Size of the compressed header in bytes (`header_size_in_bytes`).
    pub compressed_header_size: u16,
    /// Byte size of the uncompressed header (offset of the compressed
    /// header within the frame payload).
    pub uncompressed_header_bytes: usize,
}

/// Loop filter fields of the uncompressed header (spec `loop_filter_params`).
///
/// For keyframes the mode/ref deltas start from the
/// `vp9_setup_past_independence` defaults (`set_default_lf_deltas`:
/// ref `[1, 0, -1, -1]`, mode `[0, 0]`) and are then optionally updated.
#[derive(Clone, Debug)]
pub struct LoopFilterHeader {
    /// Base filter level (0..=63).
    pub filter_level: u8,
    /// Sharpness level (0..=7).
    pub sharpness: u8,
    /// Mode/ref delta enabled flag.
    pub delta_enabled: bool,
    /// Reference deltas (INTRA, LAST, GOLDEN, ALTREF).
    pub ref_deltas: [i8; 4],
    /// Mode deltas.
    pub mode_deltas: [i8; 2],
}

impl Default for LoopFilterHeader {
    fn default() -> Self {
        Self {
            filter_level: 0,
            sharpness: 0,
            delta_enabled: true,
            ref_deltas: [1, 0, -1, -1],
            mode_deltas: [0, 0],
        }
    }
}

/// Quantization fields of the uncompressed header.
#[derive(Clone, Debug, Default)]
pub struct QuantHeader {
    /// Base quantizer index.
    pub base_q_idx: u8,
    /// Luma DC delta.
    pub y_dc_delta: i32,
    /// Chroma DC delta.
    pub uv_dc_delta: i32,
    /// Chroma AC delta.
    pub uv_ac_delta: i32,
}

impl QuantHeader {
    /// True when the frame is lossless (all-zero quantizer state).
    #[must_use]
    pub fn lossless(&self) -> bool {
        self.base_q_idx == 0
            && self.y_dc_delta == 0
            && self.uv_dc_delta == 0
            && self.uv_ac_delta == 0
    }
}

/// Segmentation fields of the uncompressed header.
#[derive(Clone, Debug)]
pub struct SegmentationHeader {
    /// Segmentation enabled.
    pub enabled: bool,
    /// Segment map update flag.
    pub update_map: bool,
    /// Segment tree probabilities.
    pub tree_probs: [u8; 7],
    /// Temporal update flag.
    pub temporal_update: bool,
    /// Prediction probabilities (temporal updates).
    pub pred_probs: [u8; 3],
    /// Absolute (vs delta) feature values.
    pub abs_delta: bool,
    /// Per-segment feature enable flags: `[segment][feature]` with features
    /// ALT_Q, ALT_LF, REF_FRAME, SKIP.
    pub feature_enabled: [[bool; 4]; 8],
    /// Per-segment feature data.
    pub feature_data: [[i16; 4]; 8],
}

impl Default for SegmentationHeader {
    fn default() -> Self {
        Self {
            enabled: false,
            update_map: false,
            tree_probs: [255; 7],
            temporal_update: false,
            pred_probs: [255; 3],
            abs_delta: false,
            feature_enabled: [[false; 4]; 8],
            feature_data: [[0; 4]; 8],
        }
    }
}

impl UncompressedHeader {
    const SYNC_BYTES: [u8; 3] = [0x49, 0x83, 0x42];

    /// Parses the uncompressed header from bitstream data.
    ///
    /// # Errors
    ///
    /// Returns error if the header is invalid.
    #[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
    pub fn parse(data: &[u8]) -> CodecResult<Self> {
        let mut reader = BitReader::new(data);
        let mut header = Self::default();

        header.frame_marker = reader.read_bits(2).map_err(CodecError::Core)? as u8;
        if header.frame_marker != 0b10 {
            return Err(CodecError::InvalidBitstream(
                "Invalid VP9 frame marker".into(),
            ));
        }

        let profile_low = reader.read_bit().map_err(CodecError::Core)?;
        let profile_high = reader.read_bit().map_err(CodecError::Core)?;
        header.profile = (profile_high << 1) | profile_low;

        if header.profile == 3 {
            let reserved = reader.read_bit().map_err(CodecError::Core)?;
            if reserved != 0 {
                return Err(CodecError::InvalidBitstream("Reserved bit not zero".into()));
            }
        }

        header.show_existing_frame = reader.read_bit().map_err(CodecError::Core)? != 0;

        if header.show_existing_frame {
            header.frame_to_show = reader.read_bits(3).map_err(CodecError::Core)? as u8;
            return Ok(header);
        }

        header.frame_type = if reader.read_bit().map_err(CodecError::Core)? != 0 {
            Vp9FrameType::Inter
        } else {
            Vp9FrameType::Key
        };

        header.show_frame = reader.read_bit().map_err(CodecError::Core)? != 0;
        header.error_resilient = reader.read_bit().map_err(CodecError::Core)? != 0;

        if header.frame_type == Vp9FrameType::Key {
            Self::parse_sync_bytes(&mut reader)?;
            Self::parse_color_config(&mut reader, &mut header)?;
            Self::parse_frame_size(&mut reader, &mut header)?;
            Self::parse_render_size(&mut reader, &mut header)?;
            header.refresh_frame_flags = 0xFF;
        } else {
            if !header.show_frame {
                header.intra_only = reader.read_bit().map_err(CodecError::Core)? != 0;
            }

            if !header.error_resilient {
                header.reset_frame_context = reader.read_bits(2).map_err(CodecError::Core)? as u8;
            }

            if header.intra_only {
                Self::parse_sync_bytes(&mut reader)?;
                if header.profile > 0 {
                    Self::parse_color_config(&mut reader, &mut header)?;
                } else {
                    header.color_space = ColorSpace::Bt601;
                    header.subsampling_x = true;
                    header.subsampling_y = true;
                    header.bit_depth = 8;
                }
                header.refresh_frame_flags = reader.read_bits(8).map_err(CodecError::Core)? as u8;
                Self::parse_frame_size(&mut reader, &mut header)?;
                Self::parse_render_size(&mut reader, &mut header)?;
            } else {
                header.refresh_frame_flags = reader.read_bits(8).map_err(CodecError::Core)? as u8;
                for i in 0..3 {
                    header.ref_frame_idx[i] = reader.read_bits(3).map_err(CodecError::Core)? as u8;
                    header.ref_frame_sign_bias[i + 1] =
                        reader.read_bit().map_err(CodecError::Core)? != 0;
                }
                let found_ref = Self::parse_frame_size_with_refs(&mut reader, &mut header)?;
                if !found_ref {
                    Self::parse_frame_size(&mut reader, &mut header)?;
                }
                Self::parse_render_size(&mut reader, &mut header)?;
                header.allow_high_precision_mv = reader.read_bit().map_err(CodecError::Core)? != 0;
                Self::parse_interp_filter(&mut reader, &mut header)?;
            }
        }

        if header.error_resilient {
            header.refresh_frame_context = false;
            header.frame_parallel_decoding = true;
        } else {
            header.refresh_frame_context = reader.read_bit().map_err(CodecError::Core)? != 0;
            header.frame_parallel_decoding = reader.read_bit().map_err(CodecError::Core)? != 0;
        }
        header.frame_context_idx = reader.read_bits(2).map_err(CodecError::Core)? as u8;

        Self::parse_loop_filter(&mut reader, &mut header)?;
        Self::parse_quantization(&mut reader, &mut header)?;
        Self::parse_segmentation(&mut reader, &mut header)?;
        Self::parse_tile_info(&mut reader, &mut header)?;

        header.compressed_header_size = reader.read_bits(16).map_err(CodecError::Core)? as u16;
        if header.compressed_header_size == 0 {
            return Err(CodecError::InvalidBitstream(
                "VP9: zero compressed header size".into(),
            ));
        }

        // trailing_bits(): the uncompressed header is padded to a byte
        // boundary; the compressed header starts at the next byte.
        header.uncompressed_header_bytes = reader.bits_read().div_ceil(8);

        Ok(header)
    }

    /// Reads `su(bits)`: magnitude then sign (libvpx
    /// `vpx_rb_read_signed_literal`).
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    fn read_signed_literal(reader: &mut BitReader<'_>, bits: u8) -> CodecResult<i32> {
        let value = reader.read_bits(bits).map_err(CodecError::Core)? as i32;
        Ok(if reader.read_bit().map_err(CodecError::Core)? != 0 {
            -value
        } else {
            value
        })
    }

    /// Spec `loop_filter_params()` (libvpx `setup_loopfilter`).
    #[allow(clippy::cast_possible_truncation)]
    fn parse_loop_filter(reader: &mut BitReader<'_>, header: &mut Self) -> CodecResult<()> {
        let lf = &mut header.loop_filter;
        lf.filter_level = reader.read_bits(6).map_err(CodecError::Core)? as u8;
        lf.sharpness = reader.read_bits(3).map_err(CodecError::Core)? as u8;
        lf.delta_enabled = reader.read_bit().map_err(CodecError::Core)? != 0;
        if lf.delta_enabled {
            let delta_update = reader.read_bit().map_err(CodecError::Core)? != 0;
            if delta_update {
                for i in 0..4 {
                    if reader.read_bit().map_err(CodecError::Core)? != 0 {
                        lf.ref_deltas[i] = Self::read_signed_literal(reader, 6)? as i8;
                    }
                }
                for i in 0..2 {
                    if reader.read_bit().map_err(CodecError::Core)? != 0 {
                        lf.mode_deltas[i] = Self::read_signed_literal(reader, 6)? as i8;
                    }
                }
            }
        }
        Ok(())
    }

    /// Spec `quantization_params()` (libvpx `setup_quantization`).
    #[allow(clippy::cast_possible_truncation)]
    fn parse_quantization(reader: &mut BitReader<'_>, header: &mut Self) -> CodecResult<()> {
        header.quant.base_q_idx = reader.read_bits(8).map_err(CodecError::Core)? as u8;
        header.quant.y_dc_delta = Self::read_delta_q(reader)?;
        header.quant.uv_dc_delta = Self::read_delta_q(reader)?;
        header.quant.uv_ac_delta = Self::read_delta_q(reader)?;
        Ok(())
    }

    fn read_delta_q(reader: &mut BitReader<'_>) -> CodecResult<i32> {
        if reader.read_bit().map_err(CodecError::Core)? != 0 {
            Self::read_signed_literal(reader, 4)
        } else {
            Ok(0)
        }
    }

    /// Spec `segmentation_params()` (libvpx `setup_segmentation`).
    #[allow(clippy::cast_possible_truncation)]
    fn parse_segmentation(reader: &mut BitReader<'_>, header: &mut Self) -> CodecResult<()> {
        /// Bits per feature value (`vp9_seg_feature_data_max` bit widths for
        /// ALT_Q=255, ALT_LF=63, REF_FRAME=3, SKIP=0).
        const FEATURE_BITS: [u8; 4] = [8, 6, 2, 0];
        /// Maximum feature values.
        const FEATURE_MAX: [i32; 4] = [255, 63, 3, 0];
        /// Signedness per feature.
        const FEATURE_SIGNED: [bool; 4] = [true, true, false, false];

        let seg = &mut header.seg;
        seg.enabled = reader.read_bit().map_err(CodecError::Core)? != 0;
        if !seg.enabled {
            return Ok(());
        }

        seg.update_map = reader.read_bit().map_err(CodecError::Core)? != 0;
        if seg.update_map {
            for p in &mut seg.tree_probs {
                *p = if reader.read_bit().map_err(CodecError::Core)? != 0 {
                    reader.read_bits(8).map_err(CodecError::Core)? as u8
                } else {
                    255
                };
            }
            seg.temporal_update = reader.read_bit().map_err(CodecError::Core)? != 0;
            for p in &mut seg.pred_probs {
                *p = if seg.temporal_update && reader.read_bit().map_err(CodecError::Core)? != 0 {
                    reader.read_bits(8).map_err(CodecError::Core)? as u8
                } else {
                    255
                };
            }
        }

        if reader.read_bit().map_err(CodecError::Core)? != 0 {
            // update_data
            seg.abs_delta = reader.read_bit().map_err(CodecError::Core)? != 0;
            for s in 0..8 {
                for f in 0..4 {
                    let enabled = reader.read_bit().map_err(CodecError::Core)? != 0;
                    seg.feature_enabled[s][f] = enabled;
                    let mut data = 0i32;
                    if enabled {
                        if FEATURE_BITS[f] > 0 {
                            data = reader
                                .read_bits(FEATURE_BITS[f])
                                .map_err(CodecError::Core)?
                                as i32;
                            if data > FEATURE_MAX[f] {
                                data = FEATURE_MAX[f];
                            }
                        }
                        if FEATURE_SIGNED[f] && reader.read_bit().map_err(CodecError::Core)? != 0 {
                            data = -data;
                        }
                    }
                    seg.feature_data[s][f] = data as i16;
                }
            }
        }
        Ok(())
    }

    /// Spec `tile_info()` (libvpx `setup_tile_info` / `vp9_get_tile_n_bits`).
    fn parse_tile_info(reader: &mut BitReader<'_>, header: &mut Self) -> CodecResult<()> {
        let mi_cols = (header.width as usize + 7) >> 3;
        let sb64_cols = (mi_cols + 7) >> 3;

        let mut min_log2 = 0u8;
        while (64usize << min_log2) < sb64_cols {
            min_log2 += 1;
        }
        let mut max_log2 = 1u8;
        while (sb64_cols >> max_log2) >= 4 {
            max_log2 += 1;
        }
        max_log2 -= 1;

        header.tile_cols_log2 = min_log2;
        let mut max_ones = max_log2.saturating_sub(min_log2);
        while max_ones > 0 && reader.read_bit().map_err(CodecError::Core)? != 0 {
            header.tile_cols_log2 += 1;
            max_ones -= 1;
        }
        if header.tile_cols_log2 > 6 {
            return Err(CodecError::InvalidBitstream(
                "VP9: invalid number of tile columns".into(),
            ));
        }

        header.tile_rows_log2 = if reader.read_bit().map_err(CodecError::Core)? != 0 {
            1 + reader.read_bit().map_err(CodecError::Core)? as u8
        } else {
            0
        };
        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    fn parse_sync_bytes(reader: &mut BitReader<'_>) -> CodecResult<()> {
        for expected in Self::SYNC_BYTES {
            let byte = reader.read_bits(8).map_err(CodecError::Core)? as u8;
            if byte != expected {
                return Err(CodecError::InvalidBitstream(
                    "Invalid VP9 sync bytes".into(),
                ));
            }
        }
        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    fn parse_color_config(reader: &mut BitReader<'_>, header: &mut Self) -> CodecResult<()> {
        if header.profile >= 2 {
            header.bit_depth = if reader.read_bit().map_err(CodecError::Core)? != 0 {
                12
            } else {
                10
            };
        } else {
            header.bit_depth = 8;
        }

        header.color_space = ColorSpace::from(reader.read_bits(3).map_err(CodecError::Core)? as u8);

        if header.color_space != ColorSpace::Srgb {
            header.color_range = reader.read_bit().map_err(CodecError::Core)? != 0;
            if header.profile == 1 || header.profile == 3 {
                header.subsampling_x = reader.read_bit().map_err(CodecError::Core)? != 0;
                header.subsampling_y = reader.read_bit().map_err(CodecError::Core)? != 0;
                reader.read_bit().map_err(CodecError::Core)?;
            } else {
                header.subsampling_x = true;
                header.subsampling_y = true;
            }
        } else {
            header.color_range = true;
            if header.profile == 1 || header.profile == 3 {
                header.subsampling_x = false;
                header.subsampling_y = false;
                reader.read_bit().map_err(CodecError::Core)?;
            }
        }

        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    fn parse_frame_size(reader: &mut BitReader<'_>, header: &mut Self) -> CodecResult<()> {
        header.width = reader.read_bits(16).map_err(CodecError::Core)? as u32 + 1;
        header.height = reader.read_bits(16).map_err(CodecError::Core)? as u32 + 1;
        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    fn parse_render_size(reader: &mut BitReader<'_>, header: &mut Self) -> CodecResult<()> {
        let different = reader.read_bit().map_err(CodecError::Core)? != 0;
        if different {
            header.render_width = reader.read_bits(16).map_err(CodecError::Core)? as u32 + 1;
            header.render_height = reader.read_bits(16).map_err(CodecError::Core)? as u32 + 1;
        } else {
            header.render_width = header.width;
            header.render_height = header.height;
        }
        Ok(())
    }

    fn parse_frame_size_with_refs(
        reader: &mut BitReader<'_>,
        _header: &mut Self,
    ) -> CodecResult<bool> {
        for _ in 0..3 {
            if reader.read_bit().map_err(CodecError::Core)? != 0 {
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn parse_interp_filter(reader: &mut BitReader<'_>, header: &mut Self) -> CodecResult<()> {
        let switchable = reader.read_bit().map_err(CodecError::Core)? != 0;
        header.interp_filter = if switchable {
            4
        } else {
            reader.read_bits(2).map_err(CodecError::Core)? as u8
        };
        Ok(())
    }

    /// Returns true if this is a keyframe.
    #[must_use]
    pub fn is_keyframe(&self) -> bool {
        self.frame_type == Vp9FrameType::Key
    }

    /// Returns true if this is an intra-only frame.
    #[must_use]
    pub fn is_intra_only(&self) -> bool {
        self.frame_type == Vp9FrameType::Key || self.intra_only
    }

    /// Returns the chroma subsampling as (x, y).
    #[must_use]
    pub const fn chroma_subsampling(&self) -> (u8, u8) {
        let x = if self.subsampling_x { 2 } else { 1 };
        let y = if self.subsampling_y { 2 } else { 1 };
        (x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_space_from() {
        assert_eq!(ColorSpace::from(2), ColorSpace::Bt709);
        assert_eq!(ColorSpace::from(7), ColorSpace::Srgb);
    }

    #[test]
    fn test_vp9_frame_type() {
        assert!(Vp9FrameType::Key.is_keyframe());
        assert!(!Vp9FrameType::Inter.is_keyframe());
    }

    #[test]
    fn test_invalid_frame_marker() {
        let data = [0x00];
        assert!(UncompressedHeader::parse(&data).is_err());
    }

    #[test]
    fn test_chroma_subsampling() {
        let mut header = UncompressedHeader::default();
        header.subsampling_x = true;
        header.subsampling_y = true;
        assert_eq!(header.chroma_subsampling(), (2, 2));
    }
}
