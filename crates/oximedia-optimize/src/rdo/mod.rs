//! Rate-distortion optimization module.
//!
//! Provides the core RDO engine for encoder optimization.

pub mod cost;
pub mod engine;
pub mod lambda;
pub mod trellis;

pub use cost::{BitCost, CostEstimate, CostMetric};
pub use engine::{ModeCandidate, RdoEngine, RdoResult};
pub use lambda::{LambdaCalculator, LambdaParams};
pub use trellis::{RdoqOptimizer, RdoqResult, RunLengthTrellis, TrellisQuantizer};
