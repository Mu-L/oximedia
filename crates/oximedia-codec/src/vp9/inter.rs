//! VP9 Inter prediction types and structures.
//!
//! This module provides inter-prediction modes, compound prediction types,
//! and context structures for motion-compensated prediction in VP9 decoding.
//!
//! Inter prediction uses motion vectors to reference blocks from previously
//! decoded frames (LAST, GOLDEN, or ALTREF references).

#![forbid(unsafe_code)]
#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::similar_names)]
#![allow(clippy::fn_params_excessive_bools)]
#![allow(clippy::if_not_else)]
#![allow(clippy::bool_to_int_with_if)]

use super::mv::{MotionVector, MvPair, MvRefType, RefPair};
use super::partition::BlockSize;

/// Number of inter prediction modes.
pub const INTER_MODES: usize = 4;

/// Number of compound prediction modes.
pub const COMPOUND_MODES: usize = 8;

/// Number of reference frame types (including intra).
pub const REF_FRAMES: usize = 4;

/// Number of inter reference frames (excluding intra).
pub const INTER_REFS: usize = 3;

/// Maximum number of reference motion vectors.
pub const MAX_REF_MV_CANDIDATES: usize = 8;

/// Maximum number of motion vector references per block.
pub const MAX_MV_REF_CANDIDATES: usize = 2;

/// Inter prediction mode.
///
/// These modes describe how motion vectors are derived for inter blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
#[repr(u8)]
pub enum InterMode {
    /// Use the nearest motion vector from spatial neighbors.
    #[default]
    NearestMv = 0,
    /// Use the near motion vector from spatial neighbors.
    NearMv = 1,
    /// Use a zero motion vector.
    ZeroMv = 2,
    /// Use a new motion vector read from the bitstream.
    NewMv = 3,
}

impl InterMode {
    /// All inter modes in order.
    pub const ALL: [InterMode; INTER_MODES] = [
        InterMode::NearestMv,
        InterMode::NearMv,
        InterMode::ZeroMv,
        InterMode::NewMv,
    ];

    /// Converts from u8 value to `InterMode`.
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::NearestMv),
            1 => Some(Self::NearMv),
            2 => Some(Self::ZeroMv),
            3 => Some(Self::NewMv),
            _ => None,
        }
    }

    /// Returns true if this mode requires reading a motion vector delta.
    #[must_use]
    pub const fn requires_mv_delta(&self) -> bool {
        matches!(self, Self::NewMv)
    }

    /// Returns true if this mode uses a zero motion vector.
    #[must_use]
    pub const fn is_zero(&self) -> bool {
        matches!(self, Self::ZeroMv)
    }

    /// Returns true if this mode uses reference motion vectors.
    #[must_use]
    pub const fn uses_ref_mv(&self) -> bool {
        matches!(self, Self::NearestMv | Self::NearMv)
    }

    /// Returns the index of this mode.
    #[must_use]
    pub const fn index(&self) -> usize {
        *self as usize
    }
}

impl From<InterMode> for u8 {
    fn from(value: InterMode) -> Self {
        value as u8
    }
}

/// Compound prediction mode.
///
/// These modes are used for compound prediction with two reference frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
#[repr(u8)]
pub enum CompoundMode {
    /// Both references use NEAREST motion vector.
    #[default]
    NearestNearest = 0,
    /// First reference NEAREST, second NEAR.
    NearestNear = 1,
    /// First reference NEAR, second NEAREST.
    NearNearest = 2,
    /// Both references use NEAR motion vector.
    NearNear = 3,
    /// First reference NEAREST, second NEW.
    NearestNew = 4,
    /// First reference NEW, second NEAREST.
    NewNearest = 5,
    /// First reference NEAR, second NEW.
    NearNew = 6,
    /// First reference NEW, second NEAR.
    NewNear = 7,
}

impl CompoundMode {
    /// All compound modes in order.
    pub const ALL: [CompoundMode; COMPOUND_MODES] = [
        CompoundMode::NearestNearest,
        CompoundMode::NearestNear,
        CompoundMode::NearNearest,
        CompoundMode::NearNear,
        CompoundMode::NearestNew,
        CompoundMode::NewNearest,
        CompoundMode::NearNew,
        CompoundMode::NewNear,
    ];

    /// Converts from u8 value to `CompoundMode`.
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::NearestNearest),
            1 => Some(Self::NearestNear),
            2 => Some(Self::NearNearest),
            3 => Some(Self::NearNear),
            4 => Some(Self::NearestNew),
            5 => Some(Self::NewNearest),
            6 => Some(Self::NearNew),
            7 => Some(Self::NewNear),
            _ => None,
        }
    }

    /// Returns true if the first reference requires reading a new motion vector.
    #[must_use]
    pub const fn first_requires_new_mv(&self) -> bool {
        matches!(self, Self::NewNearest | Self::NewNear)
    }

    /// Returns true if the second reference requires reading a new motion vector.
    #[must_use]
    pub const fn second_requires_new_mv(&self) -> bool {
        matches!(self, Self::NearestNew | Self::NearNew)
    }

    /// Returns true if any reference requires a new motion vector.
    #[must_use]
    pub const fn requires_new_mv(&self) -> bool {
        self.first_requires_new_mv() || self.second_requires_new_mv()
    }

    /// Returns the inter mode for the first reference.
    #[must_use]
    pub const fn first_mode(&self) -> InterMode {
        match self {
            Self::NearestNearest | Self::NearestNear | Self::NearestNew => InterMode::NearestMv,
            Self::NearNearest | Self::NearNear | Self::NearNew => InterMode::NearMv,
            Self::NewNearest | Self::NewNear => InterMode::NewMv,
        }
    }

    /// Returns the inter mode for the second reference.
    #[must_use]
    pub const fn second_mode(&self) -> InterMode {
        match self {
            Self::NearestNearest | Self::NearNearest | Self::NewNearest => InterMode::NearestMv,
            Self::NearestNear | Self::NearNear | Self::NewNear => InterMode::NearMv,
            Self::NearestNew | Self::NearNew => InterMode::NewMv,
        }
    }

    /// Returns the index of this mode.
    #[must_use]
    pub const fn index(&self) -> usize {
        *self as usize
    }
}

impl From<CompoundMode> for u8 {
    fn from(value: CompoundMode) -> Self {
        value as u8
    }
}

/// Reference frame type for VP9.
///
/// VP9 supports three inter reference frames and one intra (no reference) type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
#[repr(u8)]
pub enum RefFrameType {
    /// Intra prediction (no reference frame).
    #[default]
    Intra = 0,
    /// Last decoded frame.
    Last = 1,
    /// Golden reference frame.
    Golden = 2,
    /// Alternate reference frame.
    AltRef = 3,
}

impl RefFrameType {
    /// All reference frame types.
    pub const ALL: [RefFrameType; REF_FRAMES] = [
        RefFrameType::Intra,
        RefFrameType::Last,
        RefFrameType::Golden,
        RefFrameType::AltRef,
    ];

    /// Inter reference frame types only.
    pub const INTER_REFS: [RefFrameType; INTER_REFS] = [
        RefFrameType::Last,
        RefFrameType::Golden,
        RefFrameType::AltRef,
    ];

    /// Converts from u8 value to `RefFrameType`.
    #[must_use]
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Intra),
            1 => Some(Self::Last),
            2 => Some(Self::Golden),
            3 => Some(Self::AltRef),
            _ => None,
        }
    }

    /// Returns true if this is an inter reference type.
    #[must_use]
    pub const fn is_inter(&self) -> bool {
        !matches!(self, Self::Intra)
    }

    /// Returns true if this is an intra reference type.
    #[must_use]
    pub const fn is_intra(&self) -> bool {
        matches!(self, Self::Intra)
    }

    /// Returns the index of this reference type.
    #[must_use]
    pub const fn index(&self) -> usize {
        *self as usize
    }

    /// Returns the inter reference index (0, 1, or 2), or None for intra.
    #[must_use]
    pub const fn inter_index(&self) -> Option<usize> {
        match self {
            Self::Intra => None,
            Self::Last => Some(0),
            Self::Golden => Some(1),
            Self::AltRef => Some(2),
        }
    }

    /// Converts from `MvRefType`.
    #[must_use]
    pub const fn from_mv_ref_type(mv_ref: MvRefType) -> Self {
        match mv_ref {
            MvRefType::Intra => Self::Intra,
            MvRefType::Last => Self::Last,
            MvRefType::Golden => Self::Golden,
            MvRefType::AltRef => Self::AltRef,
        }
    }

    /// Converts to `MvRefType`.
    #[must_use]
    pub const fn to_mv_ref_type(&self) -> MvRefType {
        match self {
            Self::Intra => MvRefType::Intra,
            Self::Last => MvRefType::Last,
            Self::Golden => MvRefType::Golden,
            Self::AltRef => MvRefType::AltRef,
        }
    }
}

impl From<RefFrameType> for u8 {
    fn from(value: RefFrameType) -> Self {
        value as u8
    }
}

impl From<MvRefType> for RefFrameType {
    fn from(value: MvRefType) -> Self {
        Self::from_mv_ref_type(value)
    }
}

/// Prediction mode for a block.
///
/// This enum encapsulates both single and compound inter prediction modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PredictionMode {
    /// Intra prediction (no reference frames).
    #[default]
    Intra,
    /// Single reference inter prediction.
    SingleRef {
        /// The inter prediction mode.
        mode: InterMode,
        /// The reference frame type.
        ref_frame: RefFrameType,
        /// The motion vector.
        mv: MotionVector,
    },
    /// Compound (two reference) inter prediction.
    Compound {
        /// The compound prediction mode.
        mode: CompoundMode,
        /// The reference frame pair.
        ref_frames: RefPair,
        /// The motion vector pair.
        mvs: MvPair,
    },
}

impl PredictionMode {
    /// Creates a new single reference prediction mode.
    #[must_use]
    pub const fn single(mode: InterMode, ref_frame: RefFrameType, mv: MotionVector) -> Self {
        Self::SingleRef {
            mode,
            ref_frame,
            mv,
        }
    }

    /// Creates a new compound prediction mode.
    #[must_use]
    pub const fn compound(mode: CompoundMode, ref_frames: RefPair, mvs: MvPair) -> Self {
        Self::Compound {
            mode,
            ref_frames,
            mvs,
        }
    }

    /// Returns true if this is an intra prediction mode.
    #[must_use]
    pub const fn is_intra(&self) -> bool {
        matches!(self, Self::Intra)
    }

    /// Returns true if this is a single reference inter prediction mode.
    #[must_use]
    pub const fn is_single_ref(&self) -> bool {
        matches!(self, Self::SingleRef { .. })
    }

    /// Returns true if this is a compound (two reference) prediction mode.
    #[must_use]
    pub const fn is_compound(&self) -> bool {
        matches!(self, Self::Compound { .. })
    }

    /// Returns true if this is any inter prediction mode.
    #[must_use]
    pub const fn is_inter(&self) -> bool {
        !self.is_intra()
    }

    /// Returns the first motion vector, if any.
    #[must_use]
    pub const fn mv0(&self) -> Option<MotionVector> {
        match self {
            Self::Intra => None,
            Self::SingleRef { mv, .. } => Some(*mv),
            Self::Compound { mvs, .. } => Some(mvs.mv0),
        }
    }

    /// Returns the second motion vector for compound prediction, if any.
    #[must_use]
    pub const fn mv1(&self) -> Option<MotionVector> {
        match self {
            Self::Intra | Self::SingleRef { .. } => None,
            Self::Compound { mvs, .. } => Some(mvs.mv1),
        }
    }

    /// Returns the first reference frame type, if any.
    #[must_use]
    pub const fn ref0(&self) -> Option<RefFrameType> {
        match self {
            Self::Intra => None,
            Self::SingleRef { ref_frame, .. } => Some(*ref_frame),
            Self::Compound { ref_frames, .. } => {
                Some(RefFrameType::from_mv_ref_type(ref_frames.ref0))
            }
        }
    }

    /// Returns the second reference frame type for compound prediction, if any.
    #[must_use]
    pub const fn ref1(&self) -> Option<RefFrameType> {
        match self {
            Self::Intra | Self::SingleRef { .. } => None,
            Self::Compound { ref_frames, .. } => {
                Some(RefFrameType::from_mv_ref_type(ref_frames.ref1))
            }
        }
    }
}

/// Inter prediction context.
///
/// This structure holds context information needed for inter prediction
/// of a block, including reference frames and motion vectors.
#[derive(Clone, Debug, Default)]
pub struct InterPredContext {
    /// Block size for this context.
    pub block_size: BlockSize,
    /// Block row position in 4x4 units.
    pub mi_row: usize,
    /// Block column position in 4x4 units.
    pub mi_col: usize,
    /// The prediction mode.
    pub mode: PredictionMode,
    /// Reference frame sign bias (for motion vector sign adjustment).
    pub ref_sign_bias: [bool; REF_FRAMES],
    /// Whether the block is compound prediction.
    pub is_compound: bool,
    /// Reference motion vector candidates for first reference.
    pub ref_mv_candidates_0: [MotionVector; MAX_MV_REF_CANDIDATES],
    /// Reference motion vector candidates for second reference (compound).
    pub ref_mv_candidates_1: [MotionVector; MAX_MV_REF_CANDIDATES],
    /// Number of valid candidates for first reference.
    pub ref_mv_count_0: usize,
    /// Number of valid candidates for second reference.
    pub ref_mv_count_1: usize,
}

impl InterPredContext {
    /// Creates a new inter prediction context.
    #[must_use]
    pub const fn new(block_size: BlockSize, mi_row: usize, mi_col: usize) -> Self {
        Self {
            block_size,
            mi_row,
            mi_col,
            mode: PredictionMode::Intra,
            ref_sign_bias: [false; REF_FRAMES],
            is_compound: false,
            ref_mv_candidates_0: [MotionVector::zero(); MAX_MV_REF_CANDIDATES],
            ref_mv_candidates_1: [MotionVector::zero(); MAX_MV_REF_CANDIDATES],
            ref_mv_count_0: 0,
            ref_mv_count_1: 0,
        }
    }

    /// Sets the prediction mode.
    pub fn set_mode(&mut self, mode: PredictionMode) {
        self.mode = mode;
        self.is_compound = mode.is_compound();
    }

    /// Sets the reference sign bias.
    pub fn set_sign_bias(&mut self, ref_type: RefFrameType, bias: bool) {
        self.ref_sign_bias[ref_type.index()] = bias;
    }

    /// Returns the sign bias for a reference frame.
    #[must_use]
    pub const fn sign_bias(&self, ref_type: RefFrameType) -> bool {
        self.ref_sign_bias[ref_type.index()]
    }

    /// Adds a reference motion vector candidate for the first reference.
    pub fn add_mv_candidate_0(&mut self, mv: MotionVector) {
        if self.ref_mv_count_0 < MAX_MV_REF_CANDIDATES {
            self.ref_mv_candidates_0[self.ref_mv_count_0] = mv;
            self.ref_mv_count_0 += 1;
        }
    }

    /// Adds a reference motion vector candidate for the second reference.
    pub fn add_mv_candidate_1(&mut self, mv: MotionVector) {
        if self.ref_mv_count_1 < MAX_MV_REF_CANDIDATES {
            self.ref_mv_candidates_1[self.ref_mv_count_1] = mv;
            self.ref_mv_count_1 += 1;
        }
    }

    /// Returns the nearest motion vector for the first reference.
    #[must_use]
    pub const fn nearest_mv_0(&self) -> MotionVector {
        if self.ref_mv_count_0 > 0 {
            self.ref_mv_candidates_0[0]
        } else {
            MotionVector::zero()
        }
    }

    /// Returns the near motion vector for the first reference.
    #[must_use]
    pub const fn near_mv_0(&self) -> MotionVector {
        if self.ref_mv_count_0 > 1 {
            self.ref_mv_candidates_0[1]
        } else {
            MotionVector::zero()
        }
    }

    /// Returns the nearest motion vector for the second reference.
    #[must_use]
    pub const fn nearest_mv_1(&self) -> MotionVector {
        if self.ref_mv_count_1 > 0 {
            self.ref_mv_candidates_1[0]
        } else {
            MotionVector::zero()
        }
    }

    /// Returns the near motion vector for the second reference.
    #[must_use]
    pub const fn near_mv_1(&self) -> MotionVector {
        if self.ref_mv_count_1 > 1 {
            self.ref_mv_candidates_1[1]
        } else {
            MotionVector::zero()
        }
    }

    /// Clears all motion vector candidates.
    pub fn clear_candidates(&mut self) {
        self.ref_mv_count_0 = 0;
        self.ref_mv_count_1 = 0;
        self.ref_mv_candidates_0 = [MotionVector::zero(); MAX_MV_REF_CANDIDATES];
        self.ref_mv_candidates_1 = [MotionVector::zero(); MAX_MV_REF_CANDIDATES];
    }

    /// Returns the pixel x coordinate.
    #[must_use]
    pub const fn pixel_x(&self) -> usize {
        self.mi_col * 4
    }

    /// Returns the pixel y coordinate.
    #[must_use]
    pub const fn pixel_y(&self) -> usize {
        self.mi_row * 4
    }

    /// Returns the block width in pixels.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.block_size.width()
    }

    /// Returns the block height in pixels.
    #[must_use]
    pub const fn height(&self) -> usize {
        self.block_size.height()
    }
}

/// Inter mode context for probability selection.
#[derive(Clone, Copy, Debug, Default)]
pub struct InterModeContext {
    /// Context for inter mode selection.
    pub mode_context: u8,
    /// Context for new motion vector mode.
    pub new_mv_context: u8,
    /// Context for zero motion vector mode.
    pub zero_mv_context: u8,
    /// Context for reference motion vector mode.
    pub ref_mv_context: u8,
}

impl InterModeContext {
    /// Creates a new inter mode context.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode_context: 0,
            new_mv_context: 0,
            zero_mv_context: 0,
            ref_mv_context: 0,
        }
    }

    /// Creates context from neighbor counts.
    #[must_use]
    pub fn from_neighbors(
        same_ref_count: u8,
        diff_ref_count: u8,
        new_mv_count: u8,
        zero_mv_count: u8,
    ) -> Self {
        // Context calculation based on VP9 specification
        let mode_context = match (same_ref_count, diff_ref_count) {
            (0, 0) => 0,
            (1, 0) | (0, 1) => 1,
            (1, 1) => 2,
            (2, 0) | (0, 2) => 3,
            (2, 1) | (1, 2) => 4,
            _ => 5,
        };

        Self {
            mode_context,
            new_mv_context: new_mv_count.min(2),
            zero_mv_context: zero_mv_count.min(2),
            ref_mv_context: same_ref_count.min(2),
        }
    }

    /// Returns the combined mode context index.
    #[must_use]
    pub const fn mode_index(&self) -> usize {
        self.mode_context as usize
    }
}

/// Reference frame context for probability selection.
#[derive(Clone, Copy, Debug, Default)]
pub struct RefFrameContext {
    /// Context for single vs compound reference.
    pub comp_mode_context: u8,
    /// Context for LAST vs GOLDEN reference.
    pub single_ref_context_0: u8,
    /// Context for GOLDEN vs ALTREF reference.
    pub single_ref_context_1: u8,
    /// Context for compound reference selection.
    pub comp_ref_context: u8,
}

impl RefFrameContext {
    /// Creates a new reference frame context.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            comp_mode_context: 0,
            single_ref_context_0: 0,
            single_ref_context_1: 0,
            comp_ref_context: 0,
        }
    }

    /// Creates context from above and left neighbors.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_neighbors(
        above_ref: Option<RefFrameType>,
        left_ref: Option<RefFrameType>,
        above_compound: bool,
        left_compound: bool,
        above_intra: bool,
        left_intra: bool,
        has_above: bool,
        has_left: bool,
    ) -> Self {
        let mut ctx = Self::new();

        // Compound mode context
        if has_above && has_left {
            if above_compound && left_compound {
                ctx.comp_mode_context = 4;
            } else if above_compound || left_compound {
                ctx.comp_mode_context = 2;
            } else if above_intra || left_intra {
                ctx.comp_mode_context = 1;
            } else {
                ctx.comp_mode_context = 0;
            }
        } else if has_above {
            ctx.comp_mode_context = if above_compound { 3 } else { 0 };
        } else if has_left {
            ctx.comp_mode_context = if left_compound { 3 } else { 0 };
        }

        // Single reference context
        let above_is_last = matches!(above_ref, Some(RefFrameType::Last));
        let left_is_last = matches!(left_ref, Some(RefFrameType::Last));
        let above_is_golden = matches!(above_ref, Some(RefFrameType::Golden));
        let left_is_golden = matches!(left_ref, Some(RefFrameType::Golden));

        if has_above && has_left {
            if above_intra && left_intra {
                ctx.single_ref_context_0 = 2;
                ctx.single_ref_context_1 = 2;
            } else if above_intra || left_intra {
                let ref_frame = if above_intra { left_ref } else { above_ref };
                let is_last = matches!(ref_frame, Some(RefFrameType::Last));
                let is_golden = matches!(ref_frame, Some(RefFrameType::Golden));
                ctx.single_ref_context_0 = if is_last { 2 } else { 3 };
                ctx.single_ref_context_1 = if is_golden { 2 } else { 3 };
            } else {
                ctx.single_ref_context_0 = if above_is_last && left_is_last {
                    3
                } else if above_is_last || left_is_last {
                    1
                } else {
                    0
                };
                ctx.single_ref_context_1 = if above_is_golden && left_is_golden {
                    3
                } else if above_is_golden || left_is_golden {
                    1
                } else {
                    0
                };
            }
        } else if has_above {
            ctx.single_ref_context_0 = if above_intra {
                2
            } else if above_is_last {
                3
            } else {
                0
            };
            ctx.single_ref_context_1 = if above_intra {
                2
            } else if above_is_golden {
                3
            } else {
                0
            };
        } else if has_left {
            ctx.single_ref_context_0 = if left_intra {
                2
            } else if left_is_last {
                3
            } else {
                0
            };
            ctx.single_ref_context_1 = if left_intra {
                2
            } else if left_is_golden {
                3
            } else {
                0
            };
        } else {
            ctx.single_ref_context_0 = 2;
            ctx.single_ref_context_1 = 2;
        }

        ctx
    }
}

/// Scaling factors for reference frame upscaling/downscaling.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScalingFactors {
    /// Horizontal scaling factor (fixed-point 14-bit fraction).
    pub x_scale: i32,
    /// Vertical scaling factor (fixed-point 14-bit fraction).
    pub y_scale: i32,
    /// True if scaling is needed.
    pub is_scaled: bool,
}

impl ScalingFactors {
    /// Fixed-point scale factor for 1:1 (no scaling).
    pub const SCALE_ONE: i32 = 1 << 14;

    /// Creates scaling factors with no scaling.
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            x_scale: Self::SCALE_ONE,
            y_scale: Self::SCALE_ONE,
            is_scaled: false,
        }
    }

    /// Creates scaling factors from source and destination dimensions.
    #[must_use]
    pub fn from_dimensions(
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
    ) -> Self {
        let x_scale = ((i64::from(src_width) << 14) / i64::from(dst_width)) as i32;
        let y_scale = ((i64::from(src_height) << 14) / i64::from(dst_height)) as i32;
        let is_scaled = x_scale != Self::SCALE_ONE || y_scale != Self::SCALE_ONE;

        Self {
            x_scale,
            y_scale,
            is_scaled,
        }
    }

    /// Scales a horizontal position.
    #[must_use]
    pub const fn scale_x(&self, x: i32) -> i32 {
        if self.is_scaled {
            (x * self.x_scale) >> 14
        } else {
            x
        }
    }

    /// Scales a vertical position.
    #[must_use]
    pub const fn scale_y(&self, y: i32) -> i32 {
        if self.is_scaled {
            (y * self.y_scale) >> 14
        } else {
            y
        }
    }

    /// Scales a motion vector.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn scale_mv(&self, mv: MotionVector) -> MotionVector {
        if self.is_scaled {
            MotionVector::new(
                ((i32::from(mv.row) * self.y_scale) >> 14) as i16,
                ((i32::from(mv.col) * self.x_scale) >> 14) as i16,
            )
        } else {
            mv
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inter_mode() {
        assert_eq!(InterMode::NearestMv.index(), 0);
        assert_eq!(InterMode::NearMv.index(), 1);
        assert_eq!(InterMode::ZeroMv.index(), 2);
        assert_eq!(InterMode::NewMv.index(), 3);

        assert!(InterMode::NewMv.requires_mv_delta());
        assert!(!InterMode::ZeroMv.requires_mv_delta());

        assert!(InterMode::ZeroMv.is_zero());
        assert!(!InterMode::NearestMv.is_zero());

        assert!(InterMode::NearestMv.uses_ref_mv());
        assert!(InterMode::NearMv.uses_ref_mv());
        assert!(!InterMode::NewMv.uses_ref_mv());
    }

    #[test]
    fn test_inter_mode_from_u8() {
        assert_eq!(InterMode::from_u8(0), Some(InterMode::NearestMv));
        assert_eq!(InterMode::from_u8(3), Some(InterMode::NewMv));
        assert_eq!(InterMode::from_u8(4), None);
    }

    #[test]
    fn test_compound_mode() {
        assert!(CompoundMode::NewNearest.first_requires_new_mv());
        assert!(!CompoundMode::NewNearest.second_requires_new_mv());
        assert!(CompoundMode::NearestNew.second_requires_new_mv());
        assert!(!CompoundMode::NearestNew.first_requires_new_mv());

        assert_eq!(
            CompoundMode::NearestNearest.first_mode(),
            InterMode::NearestMv
        );
        assert_eq!(
            CompoundMode::NearestNearest.second_mode(),
            InterMode::NearestMv
        );
        assert_eq!(CompoundMode::NewNear.first_mode(), InterMode::NewMv);
        assert_eq!(CompoundMode::NewNear.second_mode(), InterMode::NearMv);
    }

    #[test]
    fn test_compound_mode_from_u8() {
        assert_eq!(CompoundMode::from_u8(0), Some(CompoundMode::NearestNearest));
        assert_eq!(CompoundMode::from_u8(7), Some(CompoundMode::NewNear));
        assert_eq!(CompoundMode::from_u8(8), None);
    }

    #[test]
    fn test_ref_frame_type() {
        assert!(RefFrameType::Intra.is_intra());
        assert!(!RefFrameType::Intra.is_inter());
        assert!(RefFrameType::Last.is_inter());
        assert!(RefFrameType::Golden.is_inter());
        assert!(RefFrameType::AltRef.is_inter());

        assert_eq!(RefFrameType::Intra.index(), 0);
        assert_eq!(RefFrameType::Last.index(), 1);
        assert_eq!(RefFrameType::Golden.index(), 2);
        assert_eq!(RefFrameType::AltRef.index(), 3);

        assert_eq!(RefFrameType::Intra.inter_index(), None);
        assert_eq!(RefFrameType::Last.inter_index(), Some(0));
        assert_eq!(RefFrameType::Golden.inter_index(), Some(1));
        assert_eq!(RefFrameType::AltRef.inter_index(), Some(2));
    }

    #[test]
    fn test_ref_frame_type_conversion() {
        assert_eq!(
            RefFrameType::from_mv_ref_type(MvRefType::Last),
            RefFrameType::Last
        );
        assert_eq!(RefFrameType::Last.to_mv_ref_type(), MvRefType::Last);

        let ref_type: RefFrameType = MvRefType::Golden.into();
        assert_eq!(ref_type, RefFrameType::Golden);
    }

    #[test]
    fn test_prediction_mode_intra() {
        let mode = PredictionMode::Intra;
        assert!(mode.is_intra());
        assert!(!mode.is_inter());
        assert!(!mode.is_single_ref());
        assert!(!mode.is_compound());
        assert!(mode.mv0().is_none());
        assert!(mode.ref0().is_none());
    }

    #[test]
    fn test_prediction_mode_single_ref() {
        let mv = MotionVector::new(10, 20);
        let mode = PredictionMode::single(InterMode::NearestMv, RefFrameType::Last, mv);

        assert!(!mode.is_intra());
        assert!(mode.is_inter());
        assert!(mode.is_single_ref());
        assert!(!mode.is_compound());
        assert_eq!(mode.mv0(), Some(mv));
        assert!(mode.mv1().is_none());
        assert_eq!(mode.ref0(), Some(RefFrameType::Last));
        assert!(mode.ref1().is_none());
    }

    #[test]
    fn test_prediction_mode_compound() {
        let mvs = MvPair::new(MotionVector::new(10, 20), MotionVector::new(30, 40));
        let ref_frames = RefPair::compound(MvRefType::Last, MvRefType::Golden);
        let mode = PredictionMode::compound(CompoundMode::NearestNearest, ref_frames, mvs);

        assert!(!mode.is_intra());
        assert!(mode.is_inter());
        assert!(!mode.is_single_ref());
        assert!(mode.is_compound());
        assert_eq!(mode.mv0(), Some(mvs.mv0));
        assert_eq!(mode.mv1(), Some(mvs.mv1));
        assert_eq!(mode.ref0(), Some(RefFrameType::Last));
        assert_eq!(mode.ref1(), Some(RefFrameType::Golden));
    }

    #[test]
    fn test_inter_pred_context() {
        let mut ctx = InterPredContext::new(BlockSize::Block16x16, 4, 8);

        assert_eq!(ctx.block_size, BlockSize::Block16x16);
        assert_eq!(ctx.mi_row, 4);
        assert_eq!(ctx.mi_col, 8);
        assert_eq!(ctx.pixel_x(), 32);
        assert_eq!(ctx.pixel_y(), 16);
        assert_eq!(ctx.width(), 16);
        assert_eq!(ctx.height(), 16);

        ctx.add_mv_candidate_0(MotionVector::new(10, 20));
        ctx.add_mv_candidate_0(MotionVector::new(30, 40));

        assert_eq!(ctx.ref_mv_count_0, 2);
        assert_eq!(ctx.nearest_mv_0(), MotionVector::new(10, 20));
        assert_eq!(ctx.near_mv_0(), MotionVector::new(30, 40));

        ctx.clear_candidates();
        assert_eq!(ctx.ref_mv_count_0, 0);
        assert_eq!(ctx.nearest_mv_0(), MotionVector::zero());
    }

    #[test]
    fn test_inter_pred_context_sign_bias() {
        let mut ctx = InterPredContext::new(BlockSize::Block8x8, 0, 0);

        ctx.set_sign_bias(RefFrameType::Golden, true);
        ctx.set_sign_bias(RefFrameType::AltRef, true);

        assert!(!ctx.sign_bias(RefFrameType::Intra));
        assert!(!ctx.sign_bias(RefFrameType::Last));
        assert!(ctx.sign_bias(RefFrameType::Golden));
        assert!(ctx.sign_bias(RefFrameType::AltRef));
    }

    #[test]
    fn test_inter_mode_context() {
        let ctx = InterModeContext::from_neighbors(2, 0, 1, 0);
        assert_eq!(ctx.mode_context, 3);
        assert_eq!(ctx.new_mv_context, 1);
        assert_eq!(ctx.zero_mv_context, 0);
        assert_eq!(ctx.ref_mv_context, 2);
    }

    #[test]
    fn test_ref_frame_context() {
        let ctx = RefFrameContext::from_neighbors(
            Some(RefFrameType::Last),
            Some(RefFrameType::Last),
            false,
            false,
            false,
            false,
            true,
            true,
        );

        assert_eq!(ctx.comp_mode_context, 0);
        assert_eq!(ctx.single_ref_context_0, 3);
    }

    #[test]
    fn test_scaling_factors_identity() {
        let sf = ScalingFactors::identity();
        assert!(!sf.is_scaled);
        assert_eq!(sf.x_scale, ScalingFactors::SCALE_ONE);
        assert_eq!(sf.y_scale, ScalingFactors::SCALE_ONE);
    }

    #[test]
    fn test_scaling_factors_scaled() {
        let sf = ScalingFactors::from_dimensions(1920, 1080, 960, 540);
        assert!(sf.is_scaled);

        // 2:1 scaling
        assert_eq!(sf.scale_x(100), 200);
        assert_eq!(sf.scale_y(100), 200);
    }

    #[test]
    fn test_scaling_factors_mv() {
        let sf = ScalingFactors::from_dimensions(1920, 1080, 960, 540);
        let mv = MotionVector::new(10, 20);
        let scaled = sf.scale_mv(mv);

        assert_eq!(scaled.row, 20);
        assert_eq!(scaled.col, 40);
    }

    #[test]
    fn test_scaling_factors_no_scale() {
        let sf = ScalingFactors::from_dimensions(1920, 1080, 1920, 1080);
        assert!(!sf.is_scaled);
    }
}
