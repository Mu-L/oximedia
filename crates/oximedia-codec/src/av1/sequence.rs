//! AV1 Sequence Header parsing.
//!
//! The Sequence Header OBU contains codec-level configuration.

use crate::error::{CodecError, CodecResult};
use oximedia_io::BitReader;

/// AV1 Sequence Header OBU.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct SequenceHeader {
    /// Profile (0=Main, 1=High, 2=Professional).
    pub profile: u8,
    /// Still picture mode.
    pub still_picture: bool,
    /// Reduced still picture header.
    pub reduced_still_picture_header: bool,
    /// Maximum frame width minus 1.
    pub max_frame_width_minus_1: u32,
    /// Maximum frame height minus 1.
    pub max_frame_height_minus_1: u32,
    /// Enable order hint.
    pub enable_order_hint: bool,
    /// Order hint bits minus 1.
    pub order_hint_bits: u8,
    /// Enable superres.
    pub enable_superres: bool,
    /// Enable CDEF.
    pub enable_cdef: bool,
    /// Enable restoration.
    pub enable_restoration: bool,
    /// Color configuration.
    pub color_config: ColorConfig,
    /// Film grain params present.
    pub film_grain_params_present: bool,
}

impl SequenceHeader {
    /// Get maximum frame width.
    #[must_use]
    pub const fn max_frame_width(&self) -> u32 {
        self.max_frame_width_minus_1 + 1
    }

    /// Get maximum frame height.
    #[must_use]
    pub const fn max_frame_height(&self) -> u32 {
        self.max_frame_height_minus_1 + 1
    }

    /// Parse sequence header from OBU payload.
    ///
    /// # Errors
    ///
    /// Returns error if the header is malformed.
    #[allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
    pub fn parse(data: &[u8]) -> CodecResult<Self> {
        let mut reader = BitReader::new(data);

        let profile = reader.read_bits(3).map_err(CodecError::Core)? as u8;
        if profile > 2 {
            return Err(CodecError::InvalidBitstream(format!(
                "Invalid profile: {profile}"
            )));
        }

        let still_picture = reader.read_bit().map_err(CodecError::Core)? != 0;
        let reduced_still_picture_header = reader.read_bit().map_err(CodecError::Core)? != 0;

        if reduced_still_picture_header && !still_picture {
            return Err(CodecError::InvalidBitstream(
                "reduced_still_picture_header requires still_picture".to_string(),
            ));
        }

        // Skip timing info and operating points for simplified parsing
        if reduced_still_picture_header {
            reader.read_bits(5).map_err(CodecError::Core)?; // seq_level_idx[0]
        } else {
            // timing_info_present_flag
            let timing_info_present = reader.read_bit().map_err(CodecError::Core)? != 0;
            if timing_info_present {
                reader.skip_bits(64); // Simplified: skip timing info
                let decoder_model_info_present = reader.read_bit().map_err(CodecError::Core)? != 0;
                if decoder_model_info_present {
                    reader.skip_bits(47); // Simplified: skip decoder model info
                }
            }
            // initial_display_delay_present_flag
            reader.read_bit().map_err(CodecError::Core)?;
            // operating_points_cnt_minus_1
            let op_count = reader.read_bits(5).map_err(CodecError::Core)? as usize + 1;
            for _ in 0..op_count {
                reader.skip_bits(12); // operating_point_idc
                let level = reader.read_bits(5).map_err(CodecError::Core)? as u8;
                if level > 7 {
                    reader.skip_bits(1); // seq_tier
                }
            }
        }

        let frame_width_bits = reader.read_bits(4).map_err(CodecError::Core)? as u8 + 1;
        let frame_height_bits = reader.read_bits(4).map_err(CodecError::Core)? as u8 + 1;
        let max_frame_width_minus_1 = reader
            .read_bits(frame_width_bits)
            .map_err(CodecError::Core)? as u32;
        let max_frame_height_minus_1 = reader
            .read_bits(frame_height_bits)
            .map_err(CodecError::Core)? as u32;

        let enable_order_hint;
        let order_hint_bits;
        let enable_superres;
        let enable_cdef;
        let enable_restoration;

        if reduced_still_picture_header {
            enable_order_hint = false;
            order_hint_bits = 0;
            enable_superres = false;
            enable_cdef = false;
            enable_restoration = false;
        } else {
            // frame_id_numbers_present_flag
            let frame_id_present = reader.read_bit().map_err(CodecError::Core)? != 0;
            if frame_id_present {
                reader.skip_bits(7); // delta_frame_id_length_minus_2 + additional_frame_id_length_minus_1
            }

            // Tool enables
            reader.skip_bits(7); // Various tool flags

            enable_order_hint = reader.read_bit().map_err(CodecError::Core)? != 0;

            if enable_order_hint {
                reader.skip_bits(2); // enable_jnt_comp + enable_ref_frame_mvs
            }

            // seq_choose_screen_content_tools
            let seq_choose_screen_content_tools = reader.read_bit().map_err(CodecError::Core)? != 0;
            if !seq_choose_screen_content_tools {
                reader.skip_bits(1);
            }

            // seq_choose_integer_mv
            let seq_choose_integer_mv = reader.read_bit().map_err(CodecError::Core)? != 0;
            if !seq_choose_integer_mv {
                reader.skip_bits(1);
            }

            order_hint_bits = if enable_order_hint {
                reader.read_bits(3).map_err(CodecError::Core)? as u8 + 1
            } else {
                0
            };

            enable_superres = reader.read_bit().map_err(CodecError::Core)? != 0;
            enable_cdef = reader.read_bit().map_err(CodecError::Core)? != 0;
            enable_restoration = reader.read_bit().map_err(CodecError::Core)? != 0;
        }

        let color_config = ColorConfig::parse(&mut reader, profile)?;
        let film_grain_params_present = reader.read_bit().map_err(CodecError::Core)? != 0;

        Ok(Self {
            profile,
            still_picture,
            reduced_still_picture_header,
            max_frame_width_minus_1,
            max_frame_height_minus_1,
            enable_order_hint,
            order_hint_bits,
            enable_superres,
            enable_cdef,
            enable_restoration,
            color_config,
            film_grain_params_present,
        })
    }
}

/// Color configuration.
#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct ColorConfig {
    /// Bit depth (8, 10, or 12).
    pub bit_depth: u8,
    /// Monochrome mode.
    pub mono_chrome: bool,
    /// Number of planes.
    pub num_planes: u8,
    /// Color primaries.
    pub color_primaries: u8,
    /// Transfer characteristics.
    pub transfer_characteristics: u8,
    /// Matrix coefficients.
    pub matrix_coefficients: u8,
    /// Full color range.
    pub color_range: bool,
    /// Subsampling X.
    pub subsampling_x: bool,
    /// Subsampling Y.
    pub subsampling_y: bool,
    /// Separate UV delta Q.
    pub separate_uv_delta_q: bool,
}

impl ColorConfig {
    /// Check if this is 4:2:0 subsampling.
    #[must_use]
    pub const fn is_420(&self) -> bool {
        self.subsampling_x && self.subsampling_y
    }

    /// Parse color config from bitstream.
    #[allow(clippy::cast_possible_truncation)]
    fn parse(reader: &mut BitReader<'_>, profile: u8) -> CodecResult<Self> {
        let high_bitdepth = reader.read_bit().map_err(CodecError::Core)? != 0;

        let twelve_bit = if profile == 2 && high_bitdepth {
            reader.read_bit().map_err(CodecError::Core)? != 0
        } else {
            false
        };

        let bit_depth = if profile == 2 && twelve_bit {
            12
        } else if high_bitdepth {
            10
        } else {
            8
        };

        let mono_chrome = if profile == 1 {
            false
        } else {
            reader.read_bit().map_err(CodecError::Core)? != 0
        };

        let num_planes = if mono_chrome { 1 } else { 3 };

        let color_description_present = reader.read_bit().map_err(CodecError::Core)? != 0;

        let (color_primaries, transfer_characteristics, matrix_coefficients) =
            if color_description_present {
                let cp = reader.read_bits(8).map_err(CodecError::Core)? as u8;
                let tc = reader.read_bits(8).map_err(CodecError::Core)? as u8;
                let mc = reader.read_bits(8).map_err(CodecError::Core)? as u8;
                (cp, tc, mc)
            } else {
                (2, 2, 2)
            };

        let color_range;
        let subsampling_x;
        let subsampling_y;

        if mono_chrome {
            color_range = reader.read_bit().map_err(CodecError::Core)? != 0;
            subsampling_x = true;
            subsampling_y = true;
        } else if color_primaries == 1 && transfer_characteristics == 13 && matrix_coefficients == 0
        {
            color_range = true;
            subsampling_x = false;
            subsampling_y = false;
        } else {
            color_range = reader.read_bit().map_err(CodecError::Core)? != 0;

            if profile == 0 {
                subsampling_x = true;
                subsampling_y = true;
            } else if profile == 1 {
                subsampling_x = false;
                subsampling_y = false;
            } else if bit_depth == 12 {
                subsampling_x = reader.read_bit().map_err(CodecError::Core)? != 0;
                subsampling_y = if subsampling_x {
                    reader.read_bit().map_err(CodecError::Core)? != 0
                } else {
                    false
                };
            } else {
                subsampling_x = true;
                subsampling_y = false;
            }

            if subsampling_x && subsampling_y {
                reader.skip_bits(2); // chroma_sample_position
            }
        }

        let separate_uv_delta_q = if mono_chrome {
            false
        } else {
            reader.read_bit().map_err(CodecError::Core)? != 0
        };

        Ok(Self {
            bit_depth,
            mono_chrome,
            num_planes,
            color_primaries,
            transfer_characteristics,
            matrix_coefficients,
            color_range,
            subsampling_x,
            subsampling_y,
            separate_uv_delta_q,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequence_header_dimensions() {
        let header = SequenceHeader {
            profile: 0,
            still_picture: false,
            reduced_still_picture_header: false,
            max_frame_width_minus_1: 1919,
            max_frame_height_minus_1: 1079,
            enable_order_hint: false,
            order_hint_bits: 0,
            enable_superres: false,
            enable_cdef: false,
            enable_restoration: false,
            color_config: ColorConfig::default(),
            film_grain_params_present: false,
        };

        assert_eq!(header.max_frame_width(), 1920);
        assert_eq!(header.max_frame_height(), 1080);
    }

    #[test]
    fn test_color_config_subsampling() {
        let config_420 = ColorConfig {
            subsampling_x: true,
            subsampling_y: true,
            ..Default::default()
        };
        assert!(config_420.is_420());
    }
}
