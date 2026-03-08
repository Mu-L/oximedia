//! RPU parser for NAL units and bitstreams.
//!
//! Handles parsing of Dolby Vision RPU from HEVC SEI messages and raw bitstreams.

use crate::{metadata::*, rpu::*, DolbyVisionError, DolbyVisionRpu, Profile, Result};
use bitstream_io::{BigEndian, BitRead, BitReader};
use std::io::Cursor;

/// HEVC NAL unit types for Dolby Vision.
pub mod nal_type {
    /// Dolby Vision RPU NAL unit (unregistered SEI)
    pub const UNREGISTERED_SEI: u8 = 62;

    /// Dolby Vision EL NAL unit
    pub const DV_EL: u8 = 63;

    /// Dolby Vision RPU NAL unit (alternative)
    pub const DV_RPU: u8 = 25;
}

/// Dolby Vision T.35 country code (United States).
const T35_COUNTRY_CODE: u8 = 0xB5;

/// Dolby Vision T.35 terminal provider code.
const T35_TERMINAL_PROVIDER_CODE: u16 = 0x003C;

/// Parse NAL unit containing Dolby Vision RPU.
///
/// # Errors
///
/// Returns error if NAL parsing fails.
#[allow(clippy::too_many_lines)]
pub fn parse_nal_unit(data: &[u8]) -> Result<DolbyVisionRpu> {
    if data.is_empty() {
        return Err(DolbyVisionError::InvalidNalUnit(
            "Empty NAL unit".to_string(),
        ));
    }

    // Check NAL unit type (first byte, top 7 bits after forbidden_zero_bit)
    let nal_type = (data[0] >> 1) & 0x3F;

    let payload = match nal_type {
        nal_type::UNREGISTERED_SEI | nal_type::DV_RPU => {
            // Skip NAL header (2 bytes for HEVC)
            if data.len() < 2 {
                return Err(DolbyVisionError::InvalidNalUnit(
                    "NAL unit too short".to_string(),
                ));
            }
            parse_sei_payload(&data[2..])?
        }
        nal_type::DV_EL => {
            return Err(DolbyVisionError::InvalidNalUnit(
                "Enhancement layer NAL units not yet supported".to_string(),
            ));
        }
        _ => {
            return Err(DolbyVisionError::InvalidNalUnit(format!(
                "Unexpected NAL type: {}",
                nal_type
            )));
        }
    };

    parse_rpu_bitstream(&payload)
}

/// Parse SEI payload to extract RPU data.
fn parse_sei_payload(data: &[u8]) -> Result<Vec<u8>> {
    let mut cursor = Cursor::new(data);
    let mut reader = BitReader::endian(&mut cursor, BigEndian);

    // Parse payload type (variable length)
    let mut payload_type = 0u32;
    loop {
        let byte: u8 = reader
            .read(8)
            .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;
        payload_type += u32::from(byte);
        if byte != 0xFF {
            break;
        }
    }

    // Parse payload size (variable length)
    let mut payload_size = 0u32;
    loop {
        let byte: u8 = reader
            .read(8)
            .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;
        payload_size += u32::from(byte);
        if byte != 0xFF {
            break;
        }
    }

    // For unregistered user data SEI (type 5), check T.35 header
    if payload_type == 5 {
        // Read T.35 country code
        let country_code: u8 = reader
            .read(8)
            .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;
        if country_code != T35_COUNTRY_CODE {
            return Err(DolbyVisionError::InvalidPayload(format!(
                "Invalid T.35 country code: {:#x}",
                country_code
            )));
        }

        // Read T.35 terminal provider code
        let provider_code: u16 = reader
            .read(16)
            .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;
        if provider_code != T35_TERMINAL_PROVIDER_CODE {
            return Err(DolbyVisionError::InvalidPayload(format!(
                "Invalid T.35 provider code: {:#x}",
                provider_code
            )));
        }

        // Remaining data is RPU payload
        let rpu_size = payload_size.saturating_sub(3);
        let mut rpu_data = vec![0u8; rpu_size as usize];
        reader
            .read_bytes(&mut rpu_data)
            .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

        Ok(rpu_data)
    } else {
        // Read raw payload
        let mut payload = vec![0u8; payload_size as usize];
        reader
            .read_bytes(&mut payload)
            .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;
        Ok(payload)
    }
}

/// Parse RPU from raw bitstream.
///
/// # Errors
///
/// Returns error if parsing fails.
#[allow(clippy::too_many_lines)]
pub fn parse_rpu_bitstream(data: &[u8]) -> Result<DolbyVisionRpu> {
    let mut cursor = Cursor::new(data);
    let mut reader = BitReader::endian(&mut cursor, BigEndian);

    // Parse RPU header
    let header = parse_rpu_header(&mut reader)?;

    // Determine profile from header
    let profile = if header.vdr_seq_info_present {
        if let Some(ref seq_info) = header.vdr_seq_info {
            // Infer profile from characteristics
            if seq_info.ycbcr_to_rgb_flag {
                if seq_info.bl_bit_depth == 10 {
                    Profile::Profile8
                } else {
                    Profile::Profile7
                }
            } else {
                Profile::Profile5
            }
        } else {
            Profile::Profile8
        }
    } else {
        Profile::Profile8
    };

    let mut rpu = DolbyVisionRpu::new(profile);
    rpu.header = header;

    // Parse VDR DM data if present
    if rpu.header.change_flags.contains(ChangeFlags::VDR_CHANGED) {
        rpu.vdr_dm_data = Some(parse_vdr_dm_data(&mut reader, profile)?);
    }

    // Parse metadata levels
    rpu.level1 = parse_level1_metadata(&mut reader)?;
    rpu.level2 = parse_level2_metadata(&mut reader)?;
    rpu.level5 = parse_level5_metadata(&mut reader)?;
    rpu.level6 = parse_level6_metadata(&mut reader)?;
    rpu.level8 = parse_level8_metadata(&mut reader)?;
    rpu.level9 = parse_level9_metadata(&mut reader)?;
    rpu.level11 = parse_level11_metadata(&mut reader)?;

    Ok(rpu)
}

/// Parse RPU header.
fn parse_rpu_header<R: std::io::Read>(reader: &mut BitReader<R, BigEndian>) -> Result<RpuHeader> {
    let rpu_type: u8 = reader
        .read(6)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let rpu_format: u16 = reader
        .read(11)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let vdr_seq_info_present: bool = reader
        .read_bit()
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let vdr_seq_info = if vdr_seq_info_present {
        Some(parse_vdr_seq_info(reader)?)
    } else {
        None
    };

    let picture_index: u16 = reader
        .read(10)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let change_flags_bits: u16 = reader
        .read(4)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;
    let change_flags = ChangeFlags::from_bits_truncate(change_flags_bits);

    let nlq_param_pred_flag: bool = reader
        .read_bit()
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let num_nlq_param_predictors: u8 = if nlq_param_pred_flag {
        reader
            .read(4)
            .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?
    } else {
        0
    };

    let component_order: u8 = reader
        .read(2)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let coef_data_type: u8 = reader
        .read(1)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let coef_log2_denom: u8 = reader
        .read(4)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let mapping_color_space: u8 = reader
        .read(2)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let mapping_chroma_format: u8 = reader
        .read(2)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let num_pivots_minus_2: u8 = reader
        .read(3)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let pred_pivot_value: u16 = reader
        .read(12)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    Ok(RpuHeader {
        rpu_type,
        rpu_format,
        vdr_seq_info_present,
        vdr_seq_info,
        picture_index,
        change_flags,
        nlq_param_pred_flag,
        num_nlq_param_predictors,
        component_order,
        coef_data_type,
        coef_log2_denom,
        mapping_color_space,
        mapping_chroma_format,
        num_pivots_minus_2,
        pred_pivot_value,
    })
}

/// Parse VDR sequence info.
fn parse_vdr_seq_info<R: std::io::Read>(
    reader: &mut BitReader<R, BigEndian>,
) -> Result<VdrSeqInfo> {
    let vdr_dm_metadata_id: u8 = reader
        .read(8)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let scene_refresh_flag: u8 = reader
        .read(2)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let ycbcr_to_rgb_flag: bool = reader
        .read_bit()
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let coef_data_type: u8 = reader
        .read(1)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let coef_log2_denom: u8 = reader
        .read(4)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let vdr_bit_depth: u8 = reader
        .read(4)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let bl_bit_depth: u8 = reader
        .read(4)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let el_bit_depth: u8 = reader
        .read(4)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    let source_bit_depth: u8 = reader
        .read(4)
        .map_err(|e| DolbyVisionError::InvalidHeader(e.to_string()))?;

    Ok(VdrSeqInfo {
        vdr_dm_metadata_id,
        scene_refresh_flag,
        ycbcr_to_rgb_flag,
        coef_data_type,
        coef_log2_denom,
        vdr_bit_depth,
        bl_bit_depth,
        el_bit_depth,
        source_bit_depth,
    })
}

/// Parse VDR DM data.
#[allow(clippy::too_many_lines)]
fn parse_vdr_dm_data<R: std::io::Read>(
    reader: &mut BitReader<R, BigEndian>,
    _profile: Profile,
) -> Result<VdrDmData> {
    let affected_dm_metadata_id: u8 = reader
        .read(8)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let current_dm_metadata_id: u8 = reader
        .read(8)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let scene_refresh_flag: u8 = reader
        .read(2)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let ycbcr_to_rgb_present: bool = reader
        .read_bit()
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let ycbcr_to_rgb_matrix = if ycbcr_to_rgb_present {
        Some(parse_color_matrix(reader)?)
    } else {
        None
    };

    let rgb_to_lms_present: bool = reader
        .read_bit()
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let rgb_to_lms_matrix = if rgb_to_lms_present {
        Some(parse_color_matrix(reader)?)
    } else {
        None
    };

    let signal_eotf: u16 = reader
        .read(16)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let signal_eotf_param0: u16 = reader
        .read(16)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let signal_eotf_param1: u16 = reader
        .read(16)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let signal_eotf_param2: u32 = reader
        .read(32)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let signal_bit_depth: u8 = reader
        .read(5)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let signal_color_space: u8 = reader
        .read(2)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let signal_chroma_format: u8 = reader
        .read(2)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let signal_full_range_flag: u8 = reader
        .read(2)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let source_min_pq: u16 = reader
        .read(12)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let source_max_pq: u16 = reader
        .read(12)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let source_diagonal: u16 = reader
        .read(10)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    // Parse reshaping curves (simplified - usually 3 curves for RGB)
    let num_curves: u8 = reader
        .read(2)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let mut reshaping_curves = Vec::new();
    for _ in 0..=num_curves {
        reshaping_curves.push(parse_reshaping_curve(reader)?);
    }

    // Parse NLQ parameters
    let num_nlq_params: u8 = reader
        .read(2)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let mut nlq_params = Vec::new();
    for _ in 0..=num_nlq_params {
        nlq_params.push(parse_nlq_params(reader)?);
    }

    Ok(VdrDmData {
        affected_dm_metadata_id,
        current_dm_metadata_id,
        scene_refresh_flag,
        ycbcr_to_rgb_matrix,
        rgb_to_lms_matrix,
        signal_eotf,
        signal_eotf_param0,
        signal_eotf_param1,
        signal_eotf_param2,
        signal_bit_depth,
        signal_color_space,
        signal_chroma_format,
        signal_full_range_flag,
        source_min_pq,
        source_max_pq,
        source_diagonal,
        reshaping_curves,
        nlq_params,
    })
}

/// Parse color matrix.
fn parse_color_matrix<R: std::io::Read>(
    reader: &mut BitReader<R, BigEndian>,
) -> Result<ColorMatrix> {
    let mut matrix = [[0i32; 3]; 3];
    for row in &mut matrix {
        for col in row {
            *col = reader
                .read::<i32>(16)
                .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;
        }
    }
    Ok(ColorMatrix { matrix })
}

/// Parse reshaping curve.
fn parse_reshaping_curve<R: std::io::Read>(
    reader: &mut BitReader<R, BigEndian>,
) -> Result<ReshapingCurve> {
    let num_pivots: u8 = reader
        .read(4)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let mut pivots = Vec::new();
    for _ in 0..=num_pivots {
        let pivot: u16 = reader
            .read(12)
            .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;
        pivots.push(pivot);
    }

    let mut mapping_idc = Vec::new();
    let mut poly_order_minus1 = Vec::new();
    let mut poly_coef = Vec::new();

    for _ in 0..num_pivots {
        let idc: u8 = reader
            .read(2)
            .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;
        mapping_idc.push(idc);

        if idc == 0 {
            // Polynomial mapping
            let order: u8 = reader
                .read(2)
                .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;
            poly_order_minus1.push(order);

            let mut coefs = Vec::new();
            for _ in 0..=(order + 1) {
                let coef: i64 = reader
                    .read(16)
                    .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;
                coefs.push(coef);
            }
            poly_coef.push(coefs);
        }
    }

    let mmr_order_minus1: u8 = reader
        .read(2)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let mut mmr_coef = Vec::new();
    for _ in 0..=(mmr_order_minus1 + 1) {
        let coef: i64 = reader
            .read(16)
            .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;
        mmr_coef.push(coef);
    }

    Ok(ReshapingCurve {
        pivots,
        mapping_idc,
        poly_order_minus1,
        poly_coef,
        mmr_order_minus1,
        mmr_coef,
    })
}

/// Parse NLQ parameters.
fn parse_nlq_params<R: std::io::Read>(reader: &mut BitReader<R, BigEndian>) -> Result<NlqParams> {
    let nlq_offset: u16 = reader
        .read(10)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let vdr_in_max: u64 = reader
        .read(27)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let linear_deadzone_slope: u64 = reader
        .read(26)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let linear_deadzone_threshold: u64 = reader
        .read(26)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    Ok(NlqParams {
        nlq_offset,
        vdr_in_max,
        linear_deadzone_slope,
        linear_deadzone_threshold,
    })
}

/// Parse Level 1 metadata.
fn parse_level1_metadata<R: std::io::Read>(
    reader: &mut BitReader<R, BigEndian>,
) -> Result<Option<Level1Metadata>> {
    let present: bool = reader.read_bit().unwrap_or(false);

    if !present {
        return Ok(None);
    }

    let min_pq: u16 = reader
        .read(12)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let max_pq: u16 = reader
        .read(12)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    let avg_pq: u16 = reader
        .read(12)
        .map_err(|e| DolbyVisionError::InvalidPayload(e.to_string()))?;

    Ok(Some(Level1Metadata {
        min_pq,
        max_pq,
        avg_pq,
    }))
}

/// Parse Level 2 metadata.
fn parse_level2_metadata<R: std::io::Read>(
    _reader: &mut BitReader<R, BigEndian>,
) -> Result<Option<Level2Metadata>> {
    // Level 2 parsing is complex and optional
    Ok(None)
}

/// Parse Level 5 metadata.
fn parse_level5_metadata<R: std::io::Read>(
    _reader: &mut BitReader<R, BigEndian>,
) -> Result<Option<Level5Metadata>> {
    Ok(None)
}

/// Parse Level 6 metadata.
fn parse_level6_metadata<R: std::io::Read>(
    _reader: &mut BitReader<R, BigEndian>,
) -> Result<Option<Level6Metadata>> {
    Ok(None)
}

/// Parse Level 8 metadata.
fn parse_level8_metadata<R: std::io::Read>(
    _reader: &mut BitReader<R, BigEndian>,
) -> Result<Option<Level8Metadata>> {
    Ok(None)
}

/// Parse Level 9 metadata.
fn parse_level9_metadata<R: std::io::Read>(
    _reader: &mut BitReader<R, BigEndian>,
) -> Result<Option<Level9Metadata>> {
    Ok(None)
}

/// Parse Level 11 metadata.
fn parse_level11_metadata<R: std::io::Read>(
    _reader: &mut BitReader<R, BigEndian>,
) -> Result<Option<Level11Metadata>> {
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nal_type_constants() {
        assert_eq!(nal_type::UNREGISTERED_SEI, 62);
        assert_eq!(nal_type::DV_EL, 63);
        assert_eq!(nal_type::DV_RPU, 25);
    }

    #[test]
    fn test_t35_constants() {
        assert_eq!(T35_COUNTRY_CODE, 0xB5);
        assert_eq!(T35_TERMINAL_PROVIDER_CODE, 0x003C);
    }

    #[test]
    fn test_parse_empty_nal() {
        let result = parse_nal_unit(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_nal_type() {
        let nal = vec![0x00, 0x00]; // Invalid NAL type
        let result = parse_nal_unit(&nal);
        assert!(result.is_err());
    }
}
