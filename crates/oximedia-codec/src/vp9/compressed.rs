//! VP9 Compressed header parsing.
//!
//! This module handles parsing of the VP9 compressed header, which contains
//! probability updates, transform mode, reference mode, and other frame-level
//! parameters encoded using arithmetic coding.

#![forbid(unsafe_code)]
#![allow(dead_code)]
#![allow(clippy::struct_excessive_bools)]

use super::loopfilter::LoopFilterInfo;
use super::mv::MvRefType;
use super::partition::{TxMode, TX_SIZES};
use super::probability::{
    FrameContext, Prob, ProbabilityContext, COEF_BANDS, COEF_CONTEXTS, COMP_MODE_CONTEXTS,
    COMP_REF_CONTEXTS, INTER_MODES, INTER_MODE_CONTEXTS, INTRA_MODES, IS_INTER_CONTEXTS, PLANES,
    SINGLE_REF_CONTEXTS, SKIP_CONTEXTS, UNCONSTRAINED_NODES,
};
use super::segmentation::{SegmentFeature, Segmentation, MAX_SEGMENTS};

/// Reference mode enumeration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ReferenceMode {
    /// Single reference mode.
    #[default]
    Single = 0,
    /// Compound reference mode.
    Compound = 1,
    /// Reference mode selected per block.
    Select = 2,
}

impl ReferenceMode {
    /// Converts from u8 value to `ReferenceMode`.
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Single),
            1 => Some(Self::Compound),
            2 => Some(Self::Select),
            _ => None,
        }
    }

    /// Returns true if compound references are allowed.
    #[must_use]
    pub const fn allows_compound(&self) -> bool {
        !matches!(self, Self::Single)
    }

    /// Returns true if reference mode is signaled per block.
    #[must_use]
    pub const fn is_select(&self) -> bool {
        matches!(self, Self::Select)
    }
}

impl From<ReferenceMode> for u8 {
    fn from(value: ReferenceMode) -> Self {
        value as u8
    }
}

/// Quantization parameters.
#[derive(Clone, Copy, Debug, Default)]
pub struct QuantizationParams {
    /// Base quantizer index (0-255).
    pub base_q_idx: u8,
    /// Y DC delta.
    pub delta_q_y_dc: i8,
    /// UV DC delta.
    pub delta_q_uv_dc: i8,
    /// UV AC delta.
    pub delta_q_uv_ac: i8,
    /// Lossless mode flag.
    pub lossless: bool,
}

impl QuantizationParams {
    /// Creates new quantization parameters.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            base_q_idx: 0,
            delta_q_y_dc: 0,
            delta_q_uv_dc: 0,
            delta_q_uv_ac: 0,
            lossless: false,
        }
    }

    /// Returns the effective Y DC quantizer index.
    #[must_use]
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn y_dc_quant(&self) -> u8 {
        (i16::from(self.base_q_idx) + i16::from(self.delta_q_y_dc)).clamp(0, 255) as u8
    }

    /// Returns the effective UV DC quantizer index.
    #[must_use]
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn uv_dc_quant(&self) -> u8 {
        (i16::from(self.base_q_idx) + i16::from(self.delta_q_uv_dc)).clamp(0, 255) as u8
    }

    /// Returns the effective UV AC quantizer index.
    #[must_use]
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn uv_ac_quant(&self) -> u8 {
        (i16::from(self.base_q_idx) + i16::from(self.delta_q_uv_ac)).clamp(0, 255) as u8
    }

    /// Updates lossless mode based on quantizer values.
    pub fn update_lossless(&mut self) {
        self.lossless = self.base_q_idx == 0
            && self.delta_q_y_dc == 0
            && self.delta_q_uv_dc == 0
            && self.delta_q_uv_ac == 0;
    }
}

/// Compressed header state.
#[derive(Clone, Debug, Default)]
pub struct CompressedHeader {
    /// Transform mode.
    pub tx_mode: TxMode,
    /// Reference mode.
    pub reference_mode: ReferenceMode,
    /// Quantization parameters.
    pub quant: QuantizationParams,
    /// Loop filter info.
    pub loop_filter: LoopFilterInfo,
    /// Segmentation info.
    pub segmentation: Segmentation,
    /// Frame context.
    pub frame_context: FrameContext,
    /// Frame context index to use.
    pub frame_context_idx: u8,
    /// Whether to reset frame context.
    pub reset_context: bool,
    /// Allow high precision MVs.
    pub allow_high_precision_mv: bool,
    /// Compound reference allowed.
    pub compound_reference_allowed: bool,
    /// Fixed reference for compound.
    pub fixed_ref: MvRefType,
    /// Variable reference for compound.
    pub var_ref: [MvRefType; 2],
}

impl CompressedHeader {
    /// Creates a new compressed header with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tx_mode: TxMode::default(),
            reference_mode: ReferenceMode::default(),
            quant: QuantizationParams::new(),
            loop_filter: LoopFilterInfo::new(),
            segmentation: Segmentation::new(),
            frame_context: FrameContext::new(),
            frame_context_idx: 0,
            reset_context: false,
            allow_high_precision_mv: false,
            compound_reference_allowed: false,
            fixed_ref: MvRefType::Last,
            var_ref: [MvRefType::Golden, MvRefType::AltRef],
        }
    }

    /// Resets to default values for a new frame.
    pub fn reset(&mut self) {
        self.tx_mode = TxMode::default();
        self.reference_mode = ReferenceMode::default();
        self.quant = QuantizationParams::new();
        self.loop_filter.reset();
        self.segmentation.reset();
        self.frame_context_idx = 0;
        self.reset_context = false;
        self.allow_high_precision_mv = false;
        self.compound_reference_allowed = false;
    }

    /// Applies probability updates from the compressed header.
    pub fn apply_prob_updates(&mut self, updates: &ProbabilityUpdates) {
        // Apply TX size probability updates
        for (ctx, probs) in updates.tx_8x8.iter().enumerate() {
            if probs.updated {
                // Update would go here when parsing is implemented
                let _ = ctx;
            }
        }

        // Apply partition probability updates
        for (ctx, probs) in updates.partition.iter().enumerate() {
            for (idx, &prob) in probs.iter().enumerate() {
                if prob != 0 {
                    self.frame_context.probs.update_partition(ctx, idx, prob);
                }
            }
        }

        // Apply skip probability updates
        for (ctx, &prob) in updates.skip.iter().enumerate() {
            if prob != 0 {
                self.frame_context.probs.update_skip(ctx, prob);
            }
        }

        // Apply inter mode probability updates
        for (ctx, probs) in updates.inter_mode.iter().enumerate() {
            for (idx, &prob) in probs.iter().enumerate() {
                if prob != 0 {
                    self.frame_context.probs.update_inter_mode(ctx, idx, prob);
                }
            }
        }

        // Apply intra/inter probability updates
        for (ctx, &prob) in updates.intra_inter.iter().enumerate() {
            if prob != 0 {
                self.frame_context.probs.update_intra_inter(ctx, prob);
            }
        }
    }

    /// Returns the probability context.
    #[must_use]
    pub const fn probs(&self) -> &ProbabilityContext {
        &self.frame_context.probs
    }

    /// Returns mutable probability context.
    pub fn probs_mut(&mut self) -> &mut ProbabilityContext {
        &mut self.frame_context.probs
    }

    /// Sets the transform mode.
    pub fn set_tx_mode(&mut self, mode: TxMode) {
        self.tx_mode = mode;
    }

    /// Sets the reference mode.
    pub fn set_reference_mode(&mut self, mode: ReferenceMode) {
        self.reference_mode = mode;
    }

    /// Returns true if compound references are allowed.
    #[must_use]
    pub const fn allows_compound(&self) -> bool {
        self.compound_reference_allowed && self.reference_mode.allows_compound()
    }

    /// Returns true if this is a lossless frame.
    #[must_use]
    pub const fn is_lossless(&self) -> bool {
        self.quant.lossless
    }
}

/// Probability update flags and values.
#[derive(Clone, Debug)]
pub struct TxProbUpdate {
    /// Whether this context was updated.
    pub updated: bool,
    /// New probability values.
    pub probs: [Prob; 3],
}

impl Default for TxProbUpdate {
    fn default() -> Self {
        Self {
            updated: false,
            probs: [128, 128, 128],
        }
    }
}

/// Probability updates parsed from compressed header.
#[derive(Clone, Debug, Default)]
pub struct ProbabilityUpdates {
    /// TX 8x8 probability updates.
    pub tx_8x8: [TxProbUpdate; 2],
    /// TX 16x16 probability updates.
    pub tx_16x16: [TxProbUpdate; 2],
    /// TX 32x32 probability updates.
    pub tx_32x32: [TxProbUpdate; 2],
    /// Partition probability updates.
    pub partition: [[Prob; 3]; 16],
    /// Skip probability updates.
    pub skip: [Prob; SKIP_CONTEXTS],
    /// Intra/inter probability updates.
    pub intra_inter: [Prob; IS_INTER_CONTEXTS],
    /// Compound mode probability updates.
    pub comp_mode: [Prob; COMP_MODE_CONTEXTS],
    /// Single reference probability updates.
    pub single_ref: [[Prob; 2]; SINGLE_REF_CONTEXTS],
    /// Compound reference probability updates.
    pub comp_ref: [Prob; COMP_REF_CONTEXTS],
    /// Inter mode probability updates.
    pub inter_mode: [[Prob; INTER_MODES - 1]; INTER_MODE_CONTEXTS],
    /// Y mode probability updates.
    pub y_mode: [[Prob; INTRA_MODES - 1]; 4],
    /// UV mode probability updates.
    pub uv_mode: [[Prob; INTRA_MODES - 1]; INTRA_MODES],
    /// Motion vector probability updates.
    pub mv: MvProbUpdates,
    /// Coefficient probability updates.
    pub coef: CoefProbUpdates,
}

impl ProbabilityUpdates {
    /// Creates new empty probability updates.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears all updates.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Returns true if any updates are present.
    #[must_use]
    pub fn has_updates(&self) -> bool {
        self.tx_8x8.iter().any(|u| u.updated)
            || self.tx_16x16.iter().any(|u| u.updated)
            || self.tx_32x32.iter().any(|u| u.updated)
            || self.skip.iter().any(|&p| p != 0)
            || self.intra_inter.iter().any(|&p| p != 0)
    }
}

/// Motion vector probability updates.
#[derive(Clone, Debug, Default)]
pub struct MvProbUpdates {
    /// Joint probability updates.
    pub joints: [Prob; 3],
    /// Component probability updates.
    pub comps: [MvComponentProbUpdates; 2],
}

impl MvProbUpdates {
    /// Creates new empty MV probability updates.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Motion vector component probability updates.
#[derive(Clone, Debug, Default)]
pub struct MvComponentProbUpdates {
    /// Sign probability update.
    pub sign: Prob,
    /// Class probability updates.
    pub classes: [Prob; 10],
    /// Class 0 probability update.
    pub class0: Prob,
    /// Bits probability updates.
    pub bits: [Prob; 10],
    /// Class 0 FP probability updates.
    pub class0_fp: [[Prob; 3]; 2],
    /// FP probability updates.
    pub fp: [Prob; 3],
    /// Class 0 HP probability update.
    pub class0_hp: Prob,
    /// HP probability update.
    pub hp: Prob,
}

/// Coefficient probability updates.
#[derive(Clone, Debug)]
pub struct CoefProbUpdates {
    /// Updates per TX size, plane, band, context.
    pub updates: [[[[CoefNodeUpdates; COEF_CONTEXTS]; COEF_BANDS]; PLANES]; TX_SIZES],
}

impl Default for CoefProbUpdates {
    fn default() -> Self {
        let default_node = CoefNodeUpdates::default();
        let default_ctx: [CoefNodeUpdates; COEF_CONTEXTS] = [default_node; COEF_CONTEXTS];
        let default_band: [[CoefNodeUpdates; COEF_CONTEXTS]; COEF_BANDS] =
            [default_ctx; COEF_BANDS];
        let default_plane: [[[CoefNodeUpdates; COEF_CONTEXTS]; COEF_BANDS]; PLANES] =
            [default_band; PLANES];
        Self {
            updates: [default_plane; TX_SIZES],
        }
    }
}

/// Coefficient node probability updates.
#[derive(Clone, Copy, Debug, Default)]
pub struct CoefNodeUpdates {
    /// Updated node probabilities.
    pub nodes: [Prob; UNCONSTRAINED_NODES],
    /// Whether nodes were updated.
    pub updated: [bool; UNCONSTRAINED_NODES],
}

/// Compressed header parser state.
#[derive(Clone, Debug, Default)]
pub struct CompressedHeaderParser {
    /// Accumulated probability updates.
    pub updates: ProbabilityUpdates,
    /// Whether TX mode was read.
    pub tx_mode_read: bool,
    /// Whether reference mode was read.
    pub ref_mode_read: bool,
    /// Current parsing position.
    pub position: usize,
}

impl CompressedHeaderParser {
    /// Creates a new parser.
    #[must_use]
    pub fn new() -> Self {
        Self {
            updates: ProbabilityUpdates::new(),
            tx_mode_read: false,
            ref_mode_read: false,
            position: 0,
        }
    }

    /// Resets the parser for a new frame.
    pub fn reset(&mut self) {
        self.updates.clear();
        self.tx_mode_read = false;
        self.ref_mode_read = false;
        self.position = 0;
    }

    /// Parses TX mode from boolean decoder.
    ///
    /// Returns the parsed TX mode.
    #[must_use]
    pub fn parse_tx_mode(&mut self, lossless: bool, allow_select: bool) -> TxMode {
        if lossless {
            self.tx_mode_read = true;
            return TxMode::Only4x4;
        }

        // In a real implementation, this would read from the boolean decoder
        // For now, return a default based on parameters
        self.tx_mode_read = true;
        if allow_select {
            TxMode::Select
        } else {
            TxMode::Allow32x32
        }
    }

    /// Parses reference mode from boolean decoder.
    ///
    /// Returns the parsed reference mode.
    #[must_use]
    pub fn parse_reference_mode(&mut self, compound_allowed: bool) -> ReferenceMode {
        if !compound_allowed {
            self.ref_mode_read = true;
            return ReferenceMode::Single;
        }

        // In a real implementation, this would read from the boolean decoder
        self.ref_mode_read = true;
        ReferenceMode::Select
    }

    /// Parses loop filter parameters.
    pub fn parse_loop_filter(&mut self, lf: &mut LoopFilterInfo) {
        // Reset to defaults - actual parsing would read from boolean decoder
        lf.delta_update = false;

        // In a real implementation, this would parse:
        // - loop_filter_level (6 bits)
        // - loop_filter_sharpness (3 bits)
        // - loop_filter_delta_enabled (1 bit)
        // - If delta_enabled:
        //   - loop_filter_delta_update (1 bit)
        //   - If delta_update:
        //     - ref_deltas (4 values, signed 6-bit)
        //     - mode_deltas (2 values, signed 6-bit)
    }

    /// Parses segmentation parameters.
    pub fn parse_segmentation(&mut self, seg: &mut Segmentation) {
        // In a real implementation, this would parse:
        // - segmentation_enabled (1 bit)
        // - If enabled:
        //   - segmentation_update_map (1 bit)
        //   - If update_map:
        //     - segmentation_tree_probs (7 probabilities)
        //     - segmentation_temporal_update (1 bit)
        //     - If temporal_update:
        //       - segmentation_pred_probs (3 probabilities)
        //   - segmentation_update_data (1 bit)
        //   - If update_data:
        //     - segmentation_abs_or_delta_update (1 bit)
        //     - For each segment and feature:
        //       - feature_enabled (1 bit)
        //       - If enabled: feature_value

        seg.enabled = false;
    }

    /// Parses quantization parameters.
    pub fn parse_quantization(&mut self, quant: &mut QuantizationParams) {
        // In a real implementation, this would parse:
        // - base_q_idx (8 bits literal)
        // - delta_q_y_dc (if present, signed)
        // - delta_q_uv_dc (if present, signed)
        // - delta_q_uv_ac (if present, signed)

        quant.update_lossless();
    }

    /// Parses coefficient probability updates.
    pub fn parse_coef_probs(&mut self, _tx_mode: TxMode) {
        // In a real implementation, this would parse coefficient
        // probability updates for each TX size up to the maximum
        // allowed by tx_mode
    }

    /// Parses skip probability updates.
    pub fn parse_skip_probs(&mut self) {
        // In a real implementation, this would parse skip
        // probability updates for each context
    }

    /// Parses inter mode probability updates.
    pub fn parse_inter_mode_probs(&mut self) {
        // In a real implementation, this would parse inter mode
        // probability updates for each context
    }

    /// Parses intra mode probability updates.
    pub fn parse_intra_mode_probs(&mut self) {
        // In a real implementation, this would parse intra mode
        // probability updates
    }

    /// Parses motion vector probability updates.
    pub fn parse_mv_probs(&mut self, _allow_hp: bool) {
        // In a real implementation, this would parse:
        // - MV joint probabilities
        // - For each component (row, col):
        //   - sign probability
        //   - class probabilities
        //   - class0 probabilities
        //   - bits probabilities
        //   - fractional precision probabilities
        //   - If allow_hp: high precision probabilities
    }
}

/// Segment feature parsing helper.
#[derive(Clone, Debug, Default)]
pub struct SegmentFeatureParser {
    /// Current segment index.
    pub segment_id: u8,
    /// Current feature index.
    pub feature_id: u8,
}

impl SegmentFeatureParser {
    /// Creates a new segment feature parser.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            segment_id: 0,
            feature_id: 0,
        }
    }

    /// Parses all segment features.
    #[allow(clippy::cast_possible_truncation)]
    pub fn parse_all(&mut self, seg: &mut Segmentation, abs_delta: bool) {
        seg.abs_delta = abs_delta;

        for segment_id in 0..MAX_SEGMENTS {
            self.segment_id = segment_id as u8;
            for feature in SegmentFeature::ALL {
                self.parse_feature(seg, feature);
            }
        }
    }

    /// Parses a single segment feature.
    pub fn parse_feature(&mut self, seg: &mut Segmentation, feature: SegmentFeature) {
        // In a real implementation, this would:
        // 1. Read feature_enabled bit
        // 2. If enabled, read feature_data based on feature type
        // 3. Update the segment data

        self.feature_id = feature as u8;

        // Placeholder - actual implementation would read from boolean decoder
        let _ = (seg, feature);
    }

    /// Returns the number of bits for a feature's data.
    #[must_use]
    pub const fn feature_bits(feature: SegmentFeature) -> u8 {
        feature.data_bits()
    }

    /// Returns whether a feature uses signed data.
    #[must_use]
    pub const fn feature_signed(feature: SegmentFeature) -> bool {
        feature.is_signed()
    }
}

/// TX size probability update context.
#[derive(Clone, Copy, Debug, Default)]
pub struct TxProbContext {
    /// Context index (0-1).
    pub ctx: u8,
    /// TX size being updated.
    pub tx_size: u8,
}

impl TxProbContext {
    /// Creates a new TX probability context.
    #[must_use]
    pub const fn new(ctx: u8, tx_size: u8) -> Self {
        Self { ctx, tx_size }
    }

    /// Returns the number of probabilities for this TX size.
    #[must_use]
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub const fn num_probs(&self) -> usize {
        match self.tx_size {
            1 => 1, // TX_8X8: 1 probability
            2 => 2, // TX_16X16: 2 probabilities
            3 => 3, // TX_32X32: 3 probabilities
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_mode() {
        assert!(!ReferenceMode::Single.allows_compound());
        assert!(ReferenceMode::Compound.allows_compound());
        assert!(ReferenceMode::Select.allows_compound());
        assert!(ReferenceMode::Select.is_select());
    }

    #[test]
    fn test_reference_mode_from_u8() {
        assert_eq!(ReferenceMode::from_u8(0), Some(ReferenceMode::Single));
        assert_eq!(ReferenceMode::from_u8(1), Some(ReferenceMode::Compound));
        assert_eq!(ReferenceMode::from_u8(2), Some(ReferenceMode::Select));
        assert_eq!(ReferenceMode::from_u8(3), None);
    }

    #[test]
    fn test_quantization_params() {
        let mut quant = QuantizationParams::new();
        quant.base_q_idx = 100;
        quant.delta_q_y_dc = -10;
        quant.delta_q_uv_dc = 5;
        quant.delta_q_uv_ac = 0;

        assert_eq!(quant.y_dc_quant(), 90);
        assert_eq!(quant.uv_dc_quant(), 105);
        assert_eq!(quant.uv_ac_quant(), 100);

        quant.update_lossless();
        assert!(!quant.lossless);
    }

    #[test]
    fn test_quantization_lossless() {
        let mut quant = QuantizationParams::new();
        quant.base_q_idx = 0;
        quant.delta_q_y_dc = 0;
        quant.delta_q_uv_dc = 0;
        quant.delta_q_uv_ac = 0;
        quant.update_lossless();

        assert!(quant.lossless);
    }

    #[test]
    fn test_compressed_header_new() {
        let header = CompressedHeader::new();
        assert_eq!(header.tx_mode, TxMode::Only4x4);
        assert_eq!(header.reference_mode, ReferenceMode::Single);
        assert!(!header.compound_reference_allowed);
    }

    #[test]
    fn test_compressed_header_allows_compound() {
        let mut header = CompressedHeader::new();
        assert!(!header.allows_compound());

        header.compound_reference_allowed = true;
        header.reference_mode = ReferenceMode::Compound;
        assert!(header.allows_compound());
    }

    #[test]
    fn test_compressed_header_reset() {
        let mut header = CompressedHeader::new();
        header.tx_mode = TxMode::Select;
        header.compound_reference_allowed = true;

        header.reset();

        assert_eq!(header.tx_mode, TxMode::Only4x4);
        assert!(!header.compound_reference_allowed);
    }

    #[test]
    fn test_probability_updates() {
        let mut updates = ProbabilityUpdates::new();
        assert!(!updates.has_updates());

        updates.tx_8x8[0].updated = true;
        assert!(updates.has_updates());

        updates.clear();
        assert!(!updates.has_updates());
    }

    #[test]
    fn test_compressed_header_parser() {
        let mut parser = CompressedHeaderParser::new();

        let tx_mode = parser.parse_tx_mode(false, true);
        assert_eq!(tx_mode, TxMode::Select);
        assert!(parser.tx_mode_read);

        let ref_mode = parser.parse_reference_mode(true);
        assert_eq!(ref_mode, ReferenceMode::Select);
        assert!(parser.ref_mode_read);
    }

    #[test]
    fn test_parser_lossless_tx_mode() {
        let mut parser = CompressedHeaderParser::new();
        let tx_mode = parser.parse_tx_mode(true, true);
        assert_eq!(tx_mode, TxMode::Only4x4);
    }

    #[test]
    fn test_parser_no_compound_ref_mode() {
        let mut parser = CompressedHeaderParser::new();
        let ref_mode = parser.parse_reference_mode(false);
        assert_eq!(ref_mode, ReferenceMode::Single);
    }

    #[test]
    fn test_segment_feature_parser() {
        let parser = SegmentFeatureParser::new();
        assert_eq!(parser.segment_id, 0);
        assert_eq!(parser.feature_id, 0);

        assert_eq!(SegmentFeatureParser::feature_bits(SegmentFeature::AltQ), 8);
        assert!(SegmentFeatureParser::feature_signed(SegmentFeature::AltQ));
        assert!(!SegmentFeatureParser::feature_signed(SegmentFeature::Skip));
    }

    #[test]
    fn test_tx_prob_context() {
        let ctx = TxProbContext::new(1, 2);
        assert_eq!(ctx.ctx, 1);
        assert_eq!(ctx.tx_size, 2);
        assert_eq!(ctx.num_probs(), 2);

        let ctx32 = TxProbContext::new(0, 3);
        assert_eq!(ctx32.num_probs(), 3);
    }

    #[test]
    fn test_apply_prob_updates() {
        let mut header = CompressedHeader::new();
        let mut updates = ProbabilityUpdates::new();

        updates.skip[0] = 200;
        updates.partition[0][0] = 150;

        header.apply_prob_updates(&updates);

        assert_eq!(header.probs().skip[0], 200);
        assert_eq!(header.probs().partition[0][0], 150);
    }

    #[test]
    fn test_parser_reset() {
        let mut parser = CompressedHeaderParser::new();
        parser.tx_mode_read = true;
        parser.position = 100;
        parser.updates.skip[0] = 200;

        parser.reset();

        assert!(!parser.tx_mode_read);
        assert_eq!(parser.position, 0);
        assert_eq!(parser.updates.skip[0], 0);
    }
}
