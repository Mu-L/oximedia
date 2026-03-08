//! Codec optimization and tuning suite for `OxiMedia`.
//!
//! `oximedia-optimize` provides advanced optimization techniques for video encoders:
//!
//! - **Rate-Distortion Optimization (RDO)** - Advanced mode decision based on rate-distortion curves
//! - **Psychovisual Optimization** - Perceptual quality tuning using visual masking models
//! - **Motion Search Tuning** - Advanced algorithms (`TZSearch`, EPZS, UMH) for motion estimation
//! - **Intra Prediction Optimization** - RDO-based mode selection for intra frames
//! - **Transform Optimization** - Adaptive transform selection (DCT/ADST) and quantization
//! - **Loop Filter Tuning** - Deblocking and Sample Adaptive Offset (SAO) optimization
//! - **Partition Selection** - Complexity-based block size selection
//! - **Reference Frame Management** - Optimal reference frame selection and DPB management
//! - **Adaptive Quantization** - Variance and psychovisual-based AQ modes
//! - **Entropy Coding Optimization** - Context modeling for CABAC/CAVLC
//!
//! # Architecture
//!
//! The optimization suite is organized into several modules:
//!
//! - [`rdo`] - Rate-distortion optimization engine and cost functions
//! - [`psycho`] - Psychovisual optimization and masking models
//! - [`motion`] - Advanced motion search algorithms
//! - [`intra`] - Intra mode selection and directional prediction
//! - [`transform`] - Transform type selection and quantization
//! - [`filter`] - Loop filter strength tuning
//! - [`partition`] - Partition decision trees
//! - [`mod@reference`] - Reference frame management
//! - [`aq`] - Adaptive quantization strategies
//! - [`entropy`] - Entropy coding context optimization
//!
//! # Optimization Levels
//!
//! Different preset levels balance encoding speed vs. quality:
//!
//! - **Fast**: Simple SAD-based decisions, limited search patterns
//! - **Medium**: SATD-based with moderate RDO
//! - **Slow**: Full RDO with extended search patterns
//! - **Placebo**: Exhaustive search for maximum quality
//!
//! # Example
//!
//! ```ignore
//! use oximedia_optimize::{OptimizerConfig, OptimizationLevel, Optimizer};
//!
//! let config = OptimizerConfig {
//!     level: OptimizationLevel::Slow,
//!     enable_psychovisual: true,
//!     enable_aq: true,
//!     lookahead_frames: 40,
//!     ..Default::default()
//! };
//!
//! let optimizer = Optimizer::new(config)?;
//! let decision = optimizer.optimize_block(&frame_data, block_info)?;
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::module_name_repetitions)]

pub mod adaptive_ladder;
pub mod aq;
pub mod benchmark;
pub mod bitrate_controller;
pub mod bitrate_optimizer;
pub mod cache_opt;
pub mod cache_optimizer;
pub mod cache_strategy;
pub mod complexity_analysis;
pub mod crf_sweep;
pub mod decision;
pub mod encode_preset;
pub mod encode_stats;
pub mod entropy;
pub mod examples;
pub mod filter;
pub mod frame_budget;
pub mod gop_optimizer;
pub mod intra;
pub mod lookahead;
pub mod media_optimize;
pub mod motion;
pub mod parallel_strategy;
pub mod partition;
pub mod perceptual_optimization;
pub mod prefetch;
pub mod presets;
pub mod psycho;
pub mod quality_ladder;
pub mod quality_metric;
pub mod quantizer_curve;
pub mod rdo;
pub mod reference;
pub mod scene_encode;
pub mod strategies;
pub mod transcode_optimizer;
pub mod transform;
pub mod two_pass;
pub mod utils;

use oximedia_core::OxiResult;

/// Optimization level presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptimizationLevel {
    /// Fast encoding with simple SAD-based decisions.
    Fast,
    /// Medium quality with SATD and moderate RDO.
    #[default]
    Medium,
    /// Slow encoding with full RDO.
    Slow,
    /// Exhaustive search for maximum quality.
    Placebo,
}

/// Content type hints for adaptive optimization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentType {
    /// Animation content (sharp edges, flat areas).
    Animation,
    /// Film/camera content (grain, natural textures).
    Film,
    /// Screen content (text, graphics).
    Screen,
    /// Generic mixed content.
    #[default]
    Generic,
}

/// Main optimizer configuration.
#[derive(Debug, Clone)]
pub struct OptimizerConfig {
    /// Optimization level preset.
    pub level: OptimizationLevel,
    /// Enable psychovisual optimizations.
    pub enable_psychovisual: bool,
    /// Enable adaptive quantization.
    pub enable_aq: bool,
    /// Number of lookahead frames for temporal optimization.
    pub lookahead_frames: usize,
    /// Content type hint.
    pub content_type: ContentType,
    /// Enable parallel RDO evaluation.
    pub parallel_rdo: bool,
    /// Lambda multiplier for rate-distortion tradeoff.
    pub lambda_multiplier: f64,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            level: OptimizationLevel::default(),
            enable_psychovisual: true,
            enable_aq: true,
            lookahead_frames: 20,
            content_type: ContentType::default(),
            parallel_rdo: true,
            lambda_multiplier: 1.0,
        }
    }
}

/// Main optimization engine.
pub struct Optimizer {
    config: OptimizerConfig,
    rdo_engine: rdo::RdoEngine,
    psycho_analyzer: psycho::PsychoAnalyzer,
    motion_optimizer: motion::MotionOptimizer,
    aq_engine: aq::AqEngine,
}

impl Optimizer {
    /// Creates a new optimizer with the given configuration.
    pub fn new(config: OptimizerConfig) -> OxiResult<Self> {
        let rdo_engine = rdo::RdoEngine::new(&config)?;
        let psycho_analyzer = psycho::PsychoAnalyzer::new(&config)?;
        let motion_optimizer = motion::MotionOptimizer::new(&config)?;
        let aq_engine = aq::AqEngine::new(&config)?;

        Ok(Self {
            config,
            rdo_engine,
            psycho_analyzer,
            motion_optimizer,
            aq_engine,
        })
    }

    /// Gets the optimizer configuration.
    #[must_use]
    pub fn config(&self) -> &OptimizerConfig {
        &self.config
    }

    /// Gets the RDO engine.
    #[must_use]
    pub fn rdo_engine(&self) -> &rdo::RdoEngine {
        &self.rdo_engine
    }

    /// Gets the psychovisual analyzer.
    #[must_use]
    pub fn psycho_analyzer(&self) -> &psycho::PsychoAnalyzer {
        &self.psycho_analyzer
    }

    /// Gets the motion optimizer.
    #[must_use]
    pub fn motion_optimizer(&self) -> &motion::MotionOptimizer {
        &self.motion_optimizer
    }

    /// Gets the adaptive quantization engine.
    #[must_use]
    pub fn aq_engine(&self) -> &aq::AqEngine {
        &self.aq_engine
    }
}

// Re-export commonly used types
pub use aq::{AqEngine, AqMode, AqResult};
pub use benchmark::{BenchmarkConfig, BenchmarkResult, BenchmarkRunner, Profiler};
pub use decision::{
    DecisionContext, DecisionStrategy, ModeDecision, ReferenceDecision, SplitDecision,
};
pub use entropy::{ContextModel, ContextOptimizer, EntropyStats};
pub use filter::{DeblockOptimizer, FilterDecision, SaoOptimizer};
pub use intra::{AngleOptimizer, IntraModeDecision, ModeOptimizer};
pub use lookahead::{GopStructure, LookaheadAnalyzer, LookaheadFrame};
pub use motion::{
    BidirectionalOptimizer, MotionOptimizer, MotionSearchResult, MotionVector, MvPredictor,
    SubpelOptimizer,
};
pub use partition::{ComplexityAnalyzer, PartitionDecision, SplitOptimizer};
pub use presets::{OptimizationPresets, TunePresets};
pub use psycho::{ContrastSensitivity, PsychoAnalyzer, VisualMasking};
pub use rdo::{CostEstimate, LambdaCalculator, RdoEngine, RdoResult, RdoqOptimizer};
pub use reference::{DpbOptimizer, ReferenceSelection};
pub use strategies::{
    BitrateAllocator, ContentAdaptiveOptimizer, OptimizationStrategy, StrategySelector,
    TemporalOptimizer,
};
pub use transform::{QuantizationOptimizer, TransformSelection};
pub use utils::{BlockMetrics, FrameMetrics, OptimizationStats};
