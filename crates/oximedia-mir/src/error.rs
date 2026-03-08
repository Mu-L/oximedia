//! Error types for MIR analysis.

use thiserror::Error;

/// MIR error type.
#[derive(Debug, Error)]
pub enum MirError {
    /// Invalid audio input.
    #[error("Invalid audio input: {0}")]
    InvalidInput(String),

    /// Insufficient data for analysis.
    #[error("Insufficient data: {0}")]
    InsufficientData(String),

    /// Analysis failed.
    #[error("Analysis failed: {0}")]
    AnalysisFailed(String),

    /// FFT error.
    #[error("FFT error: {0}")]
    FftError(String),

    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Feature extraction failed.
    #[error("Feature extraction failed: {0}")]
    FeatureExtractionFailed(String),

    /// Model error.
    #[error("Model error: {0}")]
    ModelError(String),
}

/// Result type for MIR operations.
pub type MirResult<T> = Result<T, MirError>;
