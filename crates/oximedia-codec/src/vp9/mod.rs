//! VP9 codec implementation.
//!
//! Pure Rust VP9 decoder based on the VP9 bitstream specification.
//! VP9 is a royalty-free video codec developed by Google.
//!
//! # Modules
//!
//! - `bitstream` - Boolean decoder for entropy coding
//! - `compressed` - Compressed header parsing
//! - `decoder` - Main VP9 decoder implementation
//! - `frame` - Frame types and structures
//! - `inter` - Inter prediction modes and context
//! - `intra` - Intra prediction modes and functions
//! - `loopfilter` - Loop filter parameters
//! - `mv` - Motion vector types
//! - `mvref` - Motion vector reference building
//! - `partition` - Partition types and block sizes
//! - `prediction` - Prediction buffers and interpolation
//! - `probability` - Probability tables for entropy coding
//! - `reference` - Reference frame management
//! - `segmentation` - Segmentation handling
//! - `superframe` - Superframe container parsing
//! - `transform` - Transform types and inverse transforms
//! - `uncompressed` - Uncompressed header parsing

mod bitstream;
mod coeff_decode;
mod compressed;
mod decoder;
mod frame;
mod inter;
mod intra;
mod loopfilter;
mod mv;
mod mvref;
mod partition;
mod prediction;
mod probability;
mod reference;
mod segmentation;
mod superframe;
mod symbols;
mod transform;
mod uncompressed;

// Primary exports
pub use decoder::Vp9Decoder;
pub use frame::{FrameType as Vp9FrameType, Vp9Frame};
pub use superframe::{Superframe, SuperframeIndex};
pub use uncompressed::{ColorSpace, UncompressedHeader, Vp9FrameType as HeaderFrameType};

// Compressed header exports
pub use compressed::{
    CompressedHeader, CompressedHeaderParser, ProbabilityUpdates, QuantizationParams, ReferenceMode,
};

// Loop filter exports
pub use loopfilter::{LoopFilterInfo, LoopFilterMask, LoopFilterParams, LoopFilterState};

// Motion vector exports
pub use mv::{MotionVector, MvCandidate, MvClass, MvContext, MvJoint, MvPair, MvRefType, RefPair};

// Partition exports
pub use partition::{
    BlockPosition, BlockSize, Partition, PartitionContext, Superblock, TxMode, TxSize,
};

// Probability exports
pub use probability::{FrameContext, FrameCounts, MvProbs, Prob, ProbabilityContext};

// Segmentation exports
pub use segmentation::{SegmentData, SegmentFeature, SegmentMap, Segmentation};

// Inter prediction exports
pub use inter::{
    CompoundMode, InterMode, InterModeContext, InterPredContext, PredictionMode, RefFrameContext,
    RefFrameType, ScalingFactors,
};

// Intra prediction exports
pub use intra::{
    apply_intra_prediction, predict_dc, predict_horizontal, predict_tm, predict_vertical,
    IntraMode, IntraModeContext, IntraPredContext, SubBlockModes,
};

// Prediction exports
pub use prediction::{
    apply_inter_prediction, blend_predictions, blend_weighted, subpel_interp_2d, InterPrediction,
    InterpFilter, PredBuffer,
};

// Reference frame exports
pub use reference::{
    RefFrameBuffer, RefUpdateFlags, ReferenceFrame, ReferenceFramePool, SignBiasInfo,
};

// MV reference exports
pub use mvref::{
    clamp_mv, find_best_ref_mvs, find_mv_refs, round_mv, BlockModeInfo, ModeInfoGrid,
    MvPredContext, MvRefCandidate, MvRefContext, MvRefStack,
};

// Transform exports
pub use transform::{apply_inverse_transform, dequantize, CoeffBuffer, DequantContext, TxType};

// Coefficient decoding exports
pub use coeff_decode::{CoeffContext, CoeffDecoder, CoeffToken, QuantTables, ScanOrder};

// Symbol decoding exports
pub use symbols::SymbolDecoder;
