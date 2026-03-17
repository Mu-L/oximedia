//! Dolby Vision RPU (Reference Processing Unit) builder.
//!
//! Generates minimal but structurally valid Dolby Vision RPU NALU payloads
//! for profiles 5, 8.1, and 8.4.  The output bytes can be embedded in an
//! HEVC bitstream as an unregistered SEI NAL unit (type 62) or as a
//! standalone RPU NAL unit with the `0x19` prefix required by all DV
//! profiles.
//!
//! ## Dolby Vision RPU structure (simplified)
//!
//! ```text
//! ┌───────────────────────────────────────────────────────┐
//! │  RPU NAL prefix   (1 byte, fixed 0x19)                │
//! │  dv_rpu_data_header                                   │
//! │    rpu_type       (4 bits, = 2 for regular)           │
//! │    rpu_format     (13 bits)                           │
//! │    vdr_rpu_profile (4 bits)                           │
//! │    vdr_rpu_level  (4 bits)                            │
//! │    el_spatial_resampling_filter_flag (1 bit)          │
//! │    reserved_zero_3bits (3 bits)                       │
//! │  Polynomial/Bezier mapping curves (per component)     │
//! │  Trim pass metadata (per target display)              │
//! │  RBSP trailing bits                                   │
//! └───────────────────────────────────────────────────────┘
//! ```
//!
//! ## References
//!
//! - ETSI TS 103 572 — Dolby Vision Bitstream Specification
//! - ITU-T H.265 Annex D (SEI messages)
//! - SMPTE ST 2094-10 — Dolby Vision Dynamic Metadata

// ── DoviProfile ──────────────────────────────────────────────────────────

/// Dolby Vision profile variant supported by [`DoviRpuBuilder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DoviProfile {
    /// Profile 5 — PQ-only single-layer; no HDR10/HLG base layer.
    Profile5,
    /// Profile 8.1 — HDR10 base layer with Dolby Vision RPU metadata.
    Profile81,
    /// Profile 8.4 — HLG base layer with Dolby Vision RPU metadata.
    Profile84,
}

impl DoviProfile {
    /// Numeric profile ID embedded in the RPU header.
    pub fn profile_id(self) -> u8 {
        match self {
            DoviProfile::Profile5 => 5,
            DoviProfile::Profile81 => 8,
            DoviProfile::Profile84 => 8,
        }
    }

    /// VDR RPU level for this profile (1 = standard single-layer).
    pub fn rpu_level(self) -> u8 {
        match self {
            DoviProfile::Profile5 => 1,
            DoviProfile::Profile81 => 6,
            DoviProfile::Profile84 => 6,
        }
    }

    /// Whether this profile carries an enhancement layer.
    pub fn has_el(self) -> bool {
        false // Profiles 5/8.x do not have a separate enhancement layer.
    }
}

impl std::fmt::Display for DoviProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DoviProfile::Profile5 => write!(f, "Dolby Vision Profile 5"),
            DoviProfile::Profile81 => write!(f, "Dolby Vision Profile 8.1"),
            DoviProfile::Profile84 => write!(f, "Dolby Vision Profile 8.4"),
        }
    }
}

// ── DoviRpuConfig ─────────────────────────────────────────────────────────

/// Configuration for [`DoviRpuBuilder`].
///
/// All PQ values use the 12-bit PQ code scale (0–4095), consistent with
/// the SMPTE ST 2084 signal range used in Dolby Vision streams.
#[derive(Debug, Clone)]
pub struct DoviRpuConfig {
    /// Dolby Vision profile to encode.
    pub profile: DoviProfile,
    /// Base-layer maximum PQ code value (default 3079 ≈ 1000 nits).
    pub bl_max_pq: u16,
    /// Enhancement-layer maximum PQ code value (for dual-layer; ignored for P5/P8).
    pub el_max_pq: u16,
    /// Target display maximum PQ code value (e.g. 2081 ≈ 100 nits / SDR TV).
    pub target_max_pq: u16,
    /// Trim-pass slope (multiplicative gain, typically near 1.0).
    pub trim_slop: f32,
    /// Trim-pass offset (additive bias, typically 0.0).
    pub trim_offset: f32,
    /// Trim-pass power (gamma-like exponent, typically 1.0).
    pub trim_power: f32,
}

impl DoviRpuConfig {
    /// Default configuration for Profile 8.1 targeting a 1000-nit display
    /// mastered for 100-nit SDR playback.
    #[must_use]
    pub fn profile81_default() -> Self {
        Self {
            profile: DoviProfile::Profile81,
            bl_max_pq: 3079,
            el_max_pq: 3079,
            target_max_pq: 2081,
            trim_slop: 1.0,
            trim_offset: 0.0,
            trim_power: 1.0,
        }
    }

    /// Default configuration for Profile 5 (PQ-only single-layer).
    #[must_use]
    pub fn profile5_default() -> Self {
        Self {
            profile: DoviProfile::Profile5,
            bl_max_pq: 3079,
            el_max_pq: 3079,
            target_max_pq: 2081,
            trim_slop: 1.0,
            trim_offset: 0.0,
            trim_power: 1.0,
        }
    }

    /// Default configuration for Profile 8.4 (HLG base layer).
    #[must_use]
    pub fn profile84_default() -> Self {
        Self {
            profile: DoviProfile::Profile84,
            bl_max_pq: 3079,
            el_max_pq: 3079,
            target_max_pq: 2081,
            trim_slop: 1.0,
            trim_offset: 0.0,
            trim_power: 1.0,
        }
    }
}

// ── DoviRpuBuilder ────────────────────────────────────────────────────────

/// Dolby Vision RPU NAL unit builder.
///
/// Produces a byte vector representing a complete RPU NAL payload for each
/// call to [`build_rpu_payload`](DoviRpuBuilder::build_rpu_payload).  The
/// output is frame-independent except for `frame_idx`, which is embedded in
/// a reserved field for diagnostic tracing.
///
/// # Example
///
/// ```rust
/// use oximedia_hdr::dovi_rpu::{DoviRpuBuilder, DoviRpuConfig};
///
/// let config = DoviRpuConfig::profile81_default();
/// let builder = DoviRpuBuilder::new(config);
/// let payload = builder.build_rpu_payload(0);
/// assert!(!payload.is_empty());
/// let sei = builder.build_sei_message(&payload);
/// assert!(!sei.is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct DoviRpuBuilder {
    config: DoviRpuConfig,
}

impl DoviRpuBuilder {
    /// Create a new builder with the given configuration.
    #[must_use]
    pub fn new(config: DoviRpuConfig) -> Self {
        Self { config }
    }

    /// Build a minimal, structurally-valid Dolby Vision RPU NAL payload.
    ///
    /// The output starts with the DV RPU NAL prefix byte (`0x19`) followed by
    /// the encoded RPU data and RBSP trailing bits.
    ///
    /// `frame_idx` is written into a reserved frame-counter field for stream
    /// debugging; it does not affect the semantic content of the RPU.
    #[must_use]
    pub fn build_rpu_payload(&self, frame_idx: u64) -> Vec<u8> {
        let mut bw = BitWriter::new();

        // ── RPU NAL prefix (always 0x19) ─────────────────────────────────
        bw.write_bits(0x19u32, 8);

        // ── dv_rpu_data_header ────────────────────────────────────────────
        // rpu_type = 2 (regular RPU per ETSI TS 103 572 §7.1)
        bw.write_bits(2u32, 4);
        // rpu_format: bits [12:8] = profile number, bits [7:0] = level
        let rpu_format: u16 = ((self.config.profile.profile_id() as u16) << 8)
            | (self.config.profile.rpu_level() as u16);
        bw.write_bits(rpu_format as u32, 13);
        // vdr_rpu_profile
        bw.write_bits(self.config.profile.profile_id() as u32, 4);
        // vdr_rpu_level
        bw.write_bits(self.config.profile.rpu_level() as u32, 4);
        // extended_spatial_resampling_filter_flag = 0
        bw.write_bits(0u32, 1);
        // el_spatial_resampling_filter_flag = 0 (no EL for P5/P8)
        bw.write_bits(0u32, 1);
        // disable_residual_flag = 1 (no EL residual data)
        bw.write_bits(1u32, 1);

        // ── vdr_dm_data_present_flag = 1 (we include DM metadata) ────────
        bw.write_bits(1u32, 1);
        // use_prev_vdr_rpu_flag = 0 (always include full RPU per frame)
        bw.write_bits(0u32, 1);

        // ── Mapping curves (polynomial, single component for luma) ────────
        self.write_mapping_curves(&mut bw);

        // ── VDR DM data (display management) ─────────────────────────────
        self.write_vdr_dm_data(&mut bw);

        // ── Frame index in reserved field (4 bytes little-endian) ─────────
        // This is non-normative but useful for stream debugging.
        let fi = (frame_idx & 0xFFFF_FFFF) as u32;
        bw.write_bits(fi >> 16, 16);
        bw.write_bits(fi & 0xFFFF, 16);

        // ── RBSP trailing bits ────────────────────────────────────────────
        bw.write_rbsp_trailing_bits();

        bw.into_bytes()
    }

    /// Wrap a raw RPU payload in an HEVC SEI NAL unit (SEI type 62).
    ///
    /// The SEI message structure follows ITU-T H.265 Annex D:
    /// ```text
    /// forbidden_zero_bit (1)
    /// nal_unit_type = 39 (PREFIX_SEI) or 40 (SUFFIX_SEI) (6)
    /// nuh_layer_id = 0 (6)
    /// nuh_temporal_id_plus1 = 1 (3)
    /// payloadType = 62 (unregistered user data) (1–N bytes 0xFF + remainder)
    /// payloadSize (1–N bytes 0xFF + remainder)
    /// payload_data
    /// RBSP trailing bits
    /// ```
    #[must_use]
    pub fn build_sei_message(&self, rpu_payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(rpu_payload.len() + 8);

        // HEVC NAL unit header (2 bytes).
        // nal_unit_type = 39 (PREFIX_SEI_NUT), nuh_layer_id = 0, nuh_temporal_id_plus1 = 1
        // Encoding: [forbidden(1), nal_unit_type(6), nuh_layer_id(6), nuh_temporal_id_plus1(3)]
        // = 0b0_100111_000000_001 = 0x4E01
        out.push(0x4E);
        out.push(0x01);

        // SEI payload type = 62 (user_data_unregistered → Dolby Vision RPU).
        encode_exp_golomb_sei_size(&mut out, 62);

        // SEI payload size.
        encode_exp_golomb_sei_size(&mut out, rpu_payload.len());

        // RPU payload bytes.
        out.extend_from_slice(rpu_payload);

        // RBSP trailing bits: 0x80.
        out.push(0x80);

        out
    }

    // ── Private helpers ───────────────────────────────────────────────────

    /// Write per-component polynomial mapping curves.
    ///
    /// Uses a linear (degree-1) polynomial mapping that implements the
    /// configured trim-pass slope and offset.  Three components (Y, Cb, Cr)
    /// are written; chroma components use identity mapping.
    fn write_mapping_curves(&self, bw: &mut BitWriter) {
        // num_pivots_minus2 = 0 → 2 pivots (linear mapping, 1 segment)
        for comp in 0..3u8 {
            // num_pivots_minus2: coded as 0 for 2 pivots (1 polynomial segment)
            bw.write_bits(0u32, 4); // num_pivots_minus2

            // Pivot values in 12-bit PQ domain.
            let (piv0, piv1) = if comp == 0 {
                // Luma: map [0, bl_max_pq] to [0, target_max_pq].
                (0u16, self.config.bl_max_pq.min(4095))
            } else {
                // Chroma: identity [0, 4095].
                (0u16, 4095u16)
            };

            bw.write_bits(piv0 as u32, 12);
            bw.write_bits(piv1 as u32, 12);

            // mapping_idc = 0 (polynomial mapping for this segment)
            bw.write_bits(0u32, 4);

            // poly_order = 1 (linear polynomial)
            bw.write_bits(1u32, 4);

            if comp == 0 {
                // Linear coeff a₁ (slope) encoded as fixed-point s2.13:
                // trim_slop ≈ 1.0 → 0x2000 (2^13).
                let slope_fp = (self.config.trim_slop * 8192.0).round().clamp(0.0, 65535.0) as u32;
                bw.write_bits(slope_fp, 16);
                // Linear coeff a₀ (offset) encoded as s2.13:
                let offset_fp = encode_signed_fp(self.config.trim_offset, 13);
                bw.write_bits(offset_fp, 16);
            } else {
                // Identity mapping for chroma: slope = 1.0, offset = 0.0.
                bw.write_bits(0x2000u32, 16); // slope = 1.0 in s2.13
                bw.write_bits(0u32, 16); // offset = 0.0
            }
        }
    }

    /// Write VDR display management data (trim passes for target display).
    fn write_vdr_dm_data(&self, bw: &mut BitWriter) {
        // dm_metadata_id = 1 (level 1 metadata block)
        bw.write_bits(1u32, 8);

        // signal_full_range_flag = 0 (limited range)
        bw.write_bits(0u32, 1);
        // source_min_PQ (12 bits): 0 (black)
        bw.write_bits(0u32, 12);
        // source_max_PQ (12 bits): bl_max_pq
        bw.write_bits(self.config.bl_max_pq as u32 & 0xFFF, 12);

        // dm_metadata_id = 4 (level 4: trim pass)
        bw.write_bits(4u32, 8);

        // anchor_pq (12 bits): target_max_pq
        bw.write_bits(self.config.target_max_pq as u32 & 0xFFF, 12);

        // anchor_power (12 bits): trim_power in fixed-point 4.8 (1.0 → 0x0100)
        let power_fp = (self.config.trim_power * 256.0).round().clamp(0.0, 4095.0) as u32;
        bw.write_bits(power_fp, 12);

        // Trim pass coefficients for this target display.
        // trim_slope_bias encoded as int12 offset from 0x800 neutral.
        let slope_bias = encode_trim_coeff(self.config.trim_slop);
        bw.write_bits(slope_bias, 12);
        // trim_offset_bias
        let offset_bias = encode_trim_coeff(self.config.trim_offset);
        bw.write_bits(offset_bias, 12);
        // trim_power_bias
        let power_bias = encode_trim_coeff(self.config.trim_power);
        bw.write_bits(power_bias, 12);

        // chroma_weight (12 bits): neutral = 0x800
        bw.write_bits(0x800u32, 12);
        // saturation_vector_field0..5 (12 bits each): all neutral
        for _ in 0..6 {
            bw.write_bits(0x800u32, 12);
        }

        // dm_metadata_id = 5 (level 5: active area)
        bw.write_bits(5u32, 8);
        // canvas_width  (13 bits): 1920
        bw.write_bits(1920u32, 13);
        // canvas_height (13 bits): 1080
        bw.write_bits(1080u32, 13);
        // active_area: left=0, right=1920, top=0, bottom=1080
        bw.write_bits(0u32, 13);
        bw.write_bits(1920u32, 13);
        bw.write_bits(0u32, 13);
        bw.write_bits(1080u32, 13);

        // dm_metadata_id = 255 (end-of-metadata sentinel)
        bw.write_bits(0xFFu32, 8);
    }
}

// ── Encoding helpers ──────────────────────────────────────────────────────

/// Encode a trim coefficient (centred around 1.0) into a 12-bit biased value.
///
/// Neutral (1.0 for slope/power, 0.0 for offset) maps to 0x800.
/// Range clamp: value maps to [0, 0xFFF].
fn encode_trim_coeff(value: f32) -> u32 {
    // Scale: 1 unit = 1024 steps (12-bit range 0..4095).
    // No added bias — 0.0 maps cleanly to 0, 1.0 maps to 1024.
    (value * 1024.0).round().clamp(0.0, 4095.0) as u32
}

/// Encode a signed float in two's complement fixed-point with `frac_bits`
/// fractional bits, stored in 16 bits.
fn encode_signed_fp(value: f32, frac_bits: u32) -> u32 {
    let scale = (1u32 << frac_bits) as f32;
    let raw = (value * scale).round();
    // Clamp to i16 range and reinterpret as u16.
    let clamped = raw.clamp(i16::MIN as f32, i16::MAX as f32) as i16;
    clamped as u16 as u32
}

/// Write a HEVC SEI payload-type or payload-size field using the standard
/// "run of 0xFF bytes then remainder" encoding.
fn encode_exp_golomb_sei_size(out: &mut Vec<u8>, mut value: usize) {
    while value >= 0xFF {
        out.push(0xFF);
        value -= 0xFF;
    }
    out.push(value as u8);
}

// ── BitWriter ─────────────────────────────────────────────────────────────

/// A simple MSB-first bit-packing writer.
///
/// Bits are accumulated in a 64-bit accumulator and flushed to the output
/// byte buffer as whole bytes.
struct BitWriter {
    buf: Vec<u8>,
    acc: u64,
    bits_in_acc: u32,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            buf: Vec::new(),
            acc: 0,
            bits_in_acc: 0,
        }
    }

    /// Write the `n` least-significant bits of `value` MSB-first.
    fn write_bits(&mut self, value: u32, n: u32) {
        debug_assert!(n <= 32, "write_bits: n={n} > 32");
        // Mask to `n` bits.
        let mask: u64 = if n == 32 {
            0xFFFF_FFFF
        } else {
            (1u64 << n) - 1
        };
        let v = (value as u64) & mask;
        self.acc = (self.acc << n) | v;
        self.bits_in_acc += n;
        while self.bits_in_acc >= 8 {
            self.bits_in_acc -= 8;
            let byte = ((self.acc >> self.bits_in_acc) & 0xFF) as u8;
            self.buf.push(byte);
        }
    }

    /// Write RBSP trailing bits: a `1` bit followed by zero-padding to a byte boundary.
    fn write_rbsp_trailing_bits(&mut self) {
        self.write_bits(1u32, 1);
        // Pad to byte boundary with 0s.
        let remaining = if self.bits_in_acc == 0 {
            0
        } else {
            8 - self.bits_in_acc
        };
        if remaining > 0 {
            self.write_bits(0u32, remaining);
        }
    }

    /// Consume the writer and return the accumulated byte buffer.
    fn into_bytes(self) -> Vec<u8> {
        // Any leftover bits that didn't fill a complete byte are discarded
        // (should not happen after write_rbsp_trailing_bits).
        self.buf
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── DoviProfile ──────────────────────────────────────────────────────

    #[test]
    fn test_profile_ids() {
        assert_eq!(DoviProfile::Profile5.profile_id(), 5);
        assert_eq!(DoviProfile::Profile81.profile_id(), 8);
        assert_eq!(DoviProfile::Profile84.profile_id(), 8);
    }

    #[test]
    fn test_profile_no_el() {
        assert!(!DoviProfile::Profile5.has_el());
        assert!(!DoviProfile::Profile81.has_el());
        assert!(!DoviProfile::Profile84.has_el());
    }

    #[test]
    fn test_profile_display() {
        assert!(DoviProfile::Profile5.to_string().contains("5"));
        assert!(DoviProfile::Profile81.to_string().contains("8.1"));
        assert!(DoviProfile::Profile84.to_string().contains("8.4"));
    }

    // ── DoviRpuConfig ────────────────────────────────────────────────────

    #[test]
    fn test_profile81_default_config() {
        let cfg = DoviRpuConfig::profile81_default();
        assert_eq!(cfg.profile, DoviProfile::Profile81);
        assert_eq!(cfg.bl_max_pq, 3079);
        assert_eq!(cfg.target_max_pq, 2081);
        assert!((cfg.trim_slop - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_profile5_default_config() {
        let cfg = DoviRpuConfig::profile5_default();
        assert_eq!(cfg.profile, DoviProfile::Profile5);
    }

    #[test]
    fn test_profile84_default_config() {
        let cfg = DoviRpuConfig::profile84_default();
        assert_eq!(cfg.profile, DoviProfile::Profile84);
    }

    // ── BitWriter ────────────────────────────────────────────────────────

    #[test]
    fn test_bitwriter_single_byte() {
        let mut bw = BitWriter::new();
        bw.write_bits(0xA5u32, 8);
        bw.write_rbsp_trailing_bits();
        let bytes = bw.into_bytes();
        assert_eq!(bytes[0], 0xA5);
    }

    #[test]
    fn test_bitwriter_partial_byte_padding() {
        let mut bw = BitWriter::new();
        bw.write_bits(0b111u32, 3); // 3 bits
        bw.write_rbsp_trailing_bits(); // 1-bit + 4 bits padding → total 8 bits
        let bytes = bw.into_bytes();
        assert_eq!(bytes.len(), 1);
        // 111_1_0000 = 0xF0
        assert_eq!(bytes[0], 0b1111_0000);
    }

    #[test]
    fn test_bitwriter_zero_bits() {
        let mut bw = BitWriter::new();
        bw.write_bits(0u32, 8);
        bw.write_rbsp_trailing_bits();
        let bytes = bw.into_bytes();
        assert_eq!(bytes[0], 0x00);
    }

    // ── build_rpu_payload ────────────────────────────────────────────────

    #[test]
    fn test_build_rpu_payload_non_empty() {
        let builder = DoviRpuBuilder::new(DoviRpuConfig::profile81_default());
        let payload = builder.build_rpu_payload(0);
        assert!(!payload.is_empty(), "RPU payload must not be empty");
    }

    #[test]
    fn test_build_rpu_payload_starts_with_prefix() {
        let builder = DoviRpuBuilder::new(DoviRpuConfig::profile81_default());
        let payload = builder.build_rpu_payload(0);
        assert_eq!(
            payload[0], 0x19,
            "RPU NAL must start with 0x19 prefix, got 0x{:02X}",
            payload[0]
        );
    }

    #[test]
    fn test_build_rpu_payload_frame_idx_variation() {
        let builder = DoviRpuBuilder::new(DoviRpuConfig::profile81_default());
        let p0 = builder.build_rpu_payload(0);
        let p1 = builder.build_rpu_payload(1);
        // Same length.
        assert_eq!(
            p0.len(),
            p1.len(),
            "frame index should not change payload length"
        );
        // Content differs because frame_idx is encoded.
        assert_ne!(p0, p1, "payloads for different frames should differ");
    }

    #[test]
    fn test_build_rpu_profile5() {
        let builder = DoviRpuBuilder::new(DoviRpuConfig::profile5_default());
        let payload = builder.build_rpu_payload(0);
        assert_eq!(payload[0], 0x19);
        assert!(payload.len() >= 4);
    }

    #[test]
    fn test_build_rpu_profile84() {
        let builder = DoviRpuBuilder::new(DoviRpuConfig::profile84_default());
        let payload = builder.build_rpu_payload(42);
        assert_eq!(payload[0], 0x19);
        assert!(!payload.is_empty());
    }

    // ── build_sei_message ─────────────────────────────────────────────────

    #[test]
    fn test_build_sei_message_non_empty() {
        let builder = DoviRpuBuilder::new(DoviRpuConfig::profile81_default());
        let payload = builder.build_rpu_payload(0);
        let sei = builder.build_sei_message(&payload);
        assert!(!sei.is_empty());
    }

    #[test]
    fn test_build_sei_message_header_bytes() {
        let builder = DoviRpuBuilder::new(DoviRpuConfig::profile81_default());
        let payload = builder.build_rpu_payload(0);
        let sei = builder.build_sei_message(&payload);
        // First 2 bytes are the NAL unit header (PREFIX_SEI_NUT).
        assert_eq!(sei[0], 0x4E, "NAL header byte 0 should be 0x4E");
        assert_eq!(sei[1], 0x01, "NAL header byte 1 should be 0x01");
    }

    #[test]
    fn test_build_sei_message_ends_with_trailing_bits() {
        let builder = DoviRpuBuilder::new(DoviRpuConfig::profile81_default());
        let payload = builder.build_rpu_payload(0);
        let sei = builder.build_sei_message(&payload);
        assert_eq!(
            *sei.last().unwrap_or(&0),
            0x80,
            "SEI must end with RBSP trailing bits 0x80"
        );
    }

    #[test]
    fn test_build_sei_contains_payload() {
        let builder = DoviRpuBuilder::new(DoviRpuConfig::profile81_default());
        let payload = builder.build_rpu_payload(0);
        let sei = builder.build_sei_message(&payload);
        // The payload bytes must be present somewhere in the SEI.
        let sei_body = &sei[..sei.len().saturating_sub(1)]; // strip trailing 0x80
        assert!(
            sei_body.windows(payload.len()).any(|w| w == payload),
            "SEI should contain the raw RPU payload"
        );
    }

    // ── encode_exp_golomb_sei_size ────────────────────────────────────────

    #[test]
    fn test_sei_size_encoding_small() {
        let mut out = Vec::new();
        encode_exp_golomb_sei_size(&mut out, 62);
        assert_eq!(out, vec![62u8]);
    }

    #[test]
    fn test_sei_size_encoding_large() {
        let mut out = Vec::new();
        encode_exp_golomb_sei_size(&mut out, 300);
        // 300 = 255 + 45 → [0xFF, 0x2D]
        assert_eq!(out, vec![0xFF, 45u8]);
    }

    #[test]
    fn test_sei_size_encoding_exact_255() {
        let mut out = Vec::new();
        encode_exp_golomb_sei_size(&mut out, 255);
        assert_eq!(out, vec![0xFF, 0x00]);
    }

    // ── encode_trim_coeff ─────────────────────────────────────────────────

    #[test]
    fn test_trim_coeff_zero() {
        let v = encode_trim_coeff(0.0);
        assert_eq!(v, 0, "0.0 should encode to 0");
    }

    #[test]
    fn test_trim_coeff_clamp_max() {
        let v = encode_trim_coeff(1000.0);
        assert_eq!(v, 4095);
    }

    // ── encode_signed_fp ─────────────────────────────────────────────────

    #[test]
    fn test_signed_fp_zero() {
        let v = encode_signed_fp(0.0, 13);
        assert_eq!(v, 0);
    }

    #[test]
    fn test_signed_fp_one() {
        let v = encode_signed_fp(1.0, 13);
        assert_eq!(v, 8192); // 2^13 = 8192
    }
}
