//! HDR (High Dynamic Range) video processing for OxiMedia.
//!
//! Provides SMPTE ST 2084 (PQ), HLG, and SDR transfer functions,
//! Rec. 2020 / Rec. 2100 gamut handling, HDR10 and HDR10+ metadata,
//! and a suite of tone-mapping operators for HDR-to-SDR conversion.

pub mod color_volume;
pub mod color_volume_transform;
pub mod cuva_metadata;
pub mod display_model;
pub mod dolby_vision_profile;
pub mod dovi_rpu;
pub mod dynamic_metadata;
pub mod gamut;
pub mod hdr_histogram;
pub mod hdr_metadata_extractor;
pub mod hdr_scopes;
pub mod hlg_advanced;
pub mod luminance_stats;
pub mod metadata;
pub mod metadata_passthrough;
pub mod pq_hlg_convert;
pub mod scene_grading;
pub mod st2094;
pub mod tone_mapping;
pub mod tone_mapping_ext;
pub mod transfer_function;
pub mod vivid_hdr;

pub use color_volume::{
    encode_cll_sei, encode_hdr10_sei, luminance_from_primaries, parse_cll_sei, parse_hdr10_sei,
    ContentLightLevel, Hdr10PlusMetadata, MasteringDisplayColorVolume, MaxRgbAnalyzer,
};
pub use color_volume_transform::{ictcp_to_rgb, rgb_to_ictcp, ICtCpFrame};
pub use dolby_vision_profile::{
    detect_profile, BlSignalCompatibility, CrossVersionDvMeta, DolbyVisionProfile, DvMetadata,
    DvRpuData,
};
pub use dynamic_metadata::{DynamicMetadataFrame, Hdr10PlusDynamicMetadata};
pub use gamut::{ColorGamut, GamutConversionMatrix};
pub use hdr_histogram::{
    HdrHistogram, HdrHistogramAnalyzer, HdrHistogramConfig, HistogramScale, LuminanceHistogram,
};
pub use hdr_scopes::{LumaStatistics, Vectorscope, WaveformAxis, WaveformMonitor};
pub use hlg_advanced::{
    hlg_adapted_system_gamma, hlg_ootf, hlg_system_for_display, hlg_system_gamma,
    hlg_to_pq_with_gamma, hlg_to_sdr_adaptive, HlgHdr10Convert, HlgSdrConvert, HlgSystem,
};
pub use st2094::{ExtBlock, ExtBlockType, St2094_10Metadata, St2094_40Metadata};
// Note: metadata::ContentLightLevel is superseded by color_volume::ContentLightLevel;
// use the metadata module's struct via its full path if needed.
pub use cuva_metadata::{CuvaMetadata, CuvaPictureType, CuvaToneMapParams, CuvaWhitepoint};
pub use display_model::{
    DisplayGamma, DisplayPrimaries, FullDisplayModel, HdrFormatDisplay, ToneMapDisplayParams,
    WhitePoint,
};
pub use hdr_metadata_extractor::{ContentLightLevelInfo, HdrMetadataExtractor};
pub use luminance_stats::{ContentLuminanceAnalyzer, FrameLuminanceStats};
pub use metadata::{HdrFormat, HdrMasteringMetadata};
pub use metadata_passthrough::{
    HdrMetadataPassthrough, HdrSeiInjector, HdrSeiPayload, SeiNaluExtractor,
};
pub use pq_hlg_convert::{
    effective_system_gamma, hlg_to_pq_convert, pq_to_hlg_convert, PqHlgConverterConfig,
};
pub use scene_grading::{ColorGrade, SceneGradingDatabase, SceneGradingMetadata, TrimPass};
pub use tone_mapping::{
    map_frame_parallel, tone_map_frame_rayon, BT2446MethodAToneMapper, BT2446MethodCToneMapper,
    FrameLuminanceAnalysis, InverseToneMapper, InverseToneMappingOperator, SceneReferredToneMapper,
    ToneMapper, ToneMappingConfig, ToneMappingOperator,
};
pub use tone_mapping_ext::{
    Bt2446MethodAForwardMapper, SdrToHdrConfig, SdrToHdrMapper, UpliftAlgorithm,
};
pub use transfer_function::{
    hlg_eotf, hlg_oetf, pq_eotf, pq_eotf_batch, pq_oetf, pq_oetf_batch, sdr_gamma, HlgEotfLut,
    PqEotfLut, PqOetfLut, TransferFunction,
};
pub use vivid_hdr::{VividEotf, VividHdrMetadata, VividToneMapParams};

#[derive(Debug, Clone, thiserror::Error)]
pub enum HdrError {
    #[error("invalid luminance value: {0}")]
    InvalidLuminance(f32),
    #[error("unsupported transfer function: {0}")]
    UnsupportedTransferFunction(String),
    #[error("gamut conversion error: {0}")]
    GamutConversionError(String),
    #[error("metadata parse error: {0}")]
    MetadataParseError(String),
    #[error("tone mapping error: {0}")]
    ToneMappingError(String),
}

pub type Result<T> = std::result::Result<T, HdrError>;
