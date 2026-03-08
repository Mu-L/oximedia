//! Batch file processing for loudness normalization.
//!
//! Provides utilities for processing multiple audio files with normalization.

use crate::{
    AnalysisResult, LoudnessAnalyzer, NormalizationProcessor, NormalizeError, NormalizeResult,
    ProcessorConfig, ReplayGainCalculator, ReplayGainValues,
};
use oximedia_metering::Standard;
use std::path::{Path, PathBuf};

/// Batch processing configuration.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct BatchConfig {
    /// Target loudness standard.
    pub standard: Standard,

    /// Enable true peak limiting.
    pub enable_limiter: bool,

    /// Enable dynamic range compression.
    pub enable_drc: bool,

    /// Write loudness metadata tags.
    pub write_metadata: bool,

    /// Calculate and write ReplayGain tags.
    pub write_replaygain: bool,

    /// Output format (None = same as input).
    pub output_format: Option<String>,

    /// Maximum gain adjustment in dB.
    pub max_gain_db: f64,

    /// Overwrite existing files.
    pub overwrite: bool,

    /// Process files in parallel.
    pub parallel: bool,
}

impl BatchConfig {
    /// Create a new batch configuration.
    pub fn new(standard: Standard) -> Self {
        Self {
            standard,
            enable_limiter: true,
            enable_drc: false,
            write_metadata: true,
            write_replaygain: true,
            output_format: None,
            max_gain_db: 20.0,
            overwrite: false,
            parallel: true,
        }
    }

    /// Create a minimal configuration (gain only).
    pub fn minimal(standard: Standard) -> Self {
        Self {
            standard,
            enable_limiter: false,
            enable_drc: false,
            write_metadata: false,
            write_replaygain: false,
            output_format: None,
            max_gain_db: 20.0,
            overwrite: false,
            parallel: false,
        }
    }
}

/// Batch processing result for a single file.
#[derive(Clone, Debug)]
pub struct BatchResult {
    /// Input file path.
    pub input_path: PathBuf,

    /// Output file path.
    pub output_path: PathBuf,

    /// Analysis result.
    pub analysis: AnalysisResult,

    /// Applied gain in dB.
    pub applied_gain_db: f64,

    /// ReplayGain values (if calculated).
    pub replay_gain: Option<ReplayGainValues>,

    /// Processing time in seconds.
    pub processing_time_s: f64,

    /// Success flag.
    pub success: bool,

    /// Error message (if failed).
    pub error: Option<String>,
}

impl BatchResult {
    /// Create a successful result.
    pub fn success(
        input_path: PathBuf,
        output_path: PathBuf,
        analysis: AnalysisResult,
        applied_gain_db: f64,
        processing_time_s: f64,
    ) -> Self {
        Self {
            input_path,
            output_path,
            analysis,
            applied_gain_db,
            replay_gain: None,
            processing_time_s,
            success: true,
            error: None,
        }
    }

    /// Create a failed result.
    pub fn failure(input_path: PathBuf, error: String) -> Self {
        Self {
            input_path,
            output_path: PathBuf::new(),
            analysis: create_empty_analysis(),
            applied_gain_db: 0.0,
            replay_gain: None,
            processing_time_s: 0.0,
            success: false,
            error: Some(error),
        }
    }

    /// Add ReplayGain values.
    pub fn with_replay_gain(mut self, rg: ReplayGainValues) -> Self {
        self.replay_gain = Some(rg);
        self
    }
}

/// Batch processor for normalizing multiple files.
pub struct BatchProcessor {
    config: BatchConfig,
}

impl BatchProcessor {
    /// Create a new batch processor.
    pub fn new(config: BatchConfig) -> Self {
        Self { config }
    }

    /// Process a single file.
    pub fn process_file(
        &self,
        input_path: &Path,
        output_path: &Path,
        sample_rate: f64,
        channels: usize,
    ) -> NormalizeResult<BatchResult> {
        let start_time = std::time::Instant::now();

        // Analyze the file
        let mut analyzer = LoudnessAnalyzer::new(self.config.standard, sample_rate, channels)?;

        // In a real implementation, we would:
        // 1. Open the input file
        // 2. Read audio samples
        // 3. Feed to analyzer
        // For now, this is a placeholder

        let analysis = analyzer.result();

        // Calculate gain
        let mut gain_db = analysis.recommended_gain_db;
        if gain_db.abs() > self.config.max_gain_db {
            gain_db = gain_db.signum() * self.config.max_gain_db;
        }

        // Create processor
        let processor_config = ProcessorConfig {
            sample_rate,
            channels,
            enable_limiter: self.config.enable_limiter,
            enable_drc: self.config.enable_drc,
            lookahead_ms: 5.0,
        };

        let _processor = NormalizationProcessor::new(processor_config)?;

        // In a real implementation, we would:
        // 1. Process audio samples through the processor
        // 2. Write to output file
        // 3. Write metadata if enabled

        let processing_time = start_time.elapsed().as_secs_f64();

        let mut result = BatchResult::success(
            input_path.to_path_buf(),
            output_path.to_path_buf(),
            analysis,
            gain_db,
            processing_time,
        );

        // Calculate ReplayGain if enabled
        if self.config.write_replaygain {
            let mut rg_calc = ReplayGainCalculator::new(sample_rate, channels)?;
            // In real implementation, would process samples here
            if let Ok(rg) = rg_calc.calculate() {
                result = result.with_replay_gain(rg);
            }
        }

        Ok(result)
    }

    /// Process all files in a directory.
    pub fn process_directory(
        &self,
        input_dir: &Path,
        output_dir: &Path,
    ) -> NormalizeResult<Vec<BatchResult>> {
        if !input_dir.is_dir() {
            return Err(NormalizeError::InvalidConfig(
                "Input path is not a directory".to_string(),
            ));
        }

        // Create output directory if it doesn't exist
        if !output_dir.exists() {
            std::fs::create_dir_all(output_dir)?;
        }

        let results = Vec::new();

        // In a real implementation, we would:
        // 1. Scan directory for audio files
        // 2. Process each file (potentially in parallel if config.parallel is true)
        // 3. Collect results

        // Placeholder for directory processing
        // This would iterate over files and call process_file for each

        Ok(results)
    }

    /// Process a list of files.
    pub fn process_files(
        &self,
        files: &[(PathBuf, PathBuf)],
        sample_rate: f64,
        channels: usize,
    ) -> NormalizeResult<Vec<BatchResult>> {
        let mut results = Vec::new();

        for (input_path, output_path) in files {
            // Check if output exists and overwrite is disabled
            if output_path.exists() && !self.config.overwrite {
                results.push(BatchResult::failure(
                    input_path.clone(),
                    "Output file exists and overwrite is disabled".to_string(),
                ));
                continue;
            }

            match self.process_file(input_path, output_path, sample_rate, channels) {
                Ok(result) => results.push(result),
                Err(e) => results.push(BatchResult::failure(input_path.clone(), e.to_string())),
            }
        }

        Ok(results)
    }

    /// Get the batch configuration.
    pub fn config(&self) -> &BatchConfig {
        &self.config
    }

    /// Generate a summary report for batch results.
    pub fn generate_report(results: &[BatchResult]) -> BatchReport {
        let total = results.len();
        let successful = results.iter().filter(|r| r.success).count();
        let failed = total - successful;

        let avg_gain = if successful > 0 {
            results
                .iter()
                .filter(|r| r.success)
                .map(|r| r.applied_gain_db)
                .sum::<f64>()
                / successful as f64
        } else {
            0.0
        };

        let total_time = results.iter().map(|r| r.processing_time_s).sum::<f64>();

        BatchReport {
            total_files: total,
            successful_files: successful,
            failed_files: failed,
            average_gain_db: avg_gain,
            total_processing_time_s: total_time,
        }
    }
}

/// Batch processing summary report.
#[derive(Clone, Debug)]
pub struct BatchReport {
    /// Total number of files processed.
    pub total_files: usize,

    /// Number of successfully processed files.
    pub successful_files: usize,

    /// Number of failed files.
    pub failed_files: usize,

    /// Average gain applied in dB.
    pub average_gain_db: f64,

    /// Total processing time in seconds.
    pub total_processing_time_s: f64,
}

impl BatchReport {
    /// Format as human-readable string.
    pub fn format(&self) -> String {
        format!(
            "Batch Processing Report\n\
             =======================\n\
             Total files: {}\n\
             Successful: {}\n\
             Failed: {}\n\
             Average gain: {:+.2} dB\n\
             Total time: {:.2}s",
            self.total_files,
            self.successful_files,
            self.failed_files,
            self.average_gain_db,
            self.total_processing_time_s
        )
    }
}

/// Create an empty analysis result for error cases.
fn create_empty_analysis() -> AnalysisResult {
    use oximedia_metering::{ComplianceResult, LoudnessMetrics};

    AnalysisResult {
        integrated_lufs: f64::NEG_INFINITY,
        loudness_range: 0.0,
        true_peak_dbtp: f64::NEG_INFINITY,
        target_lufs: -23.0,
        max_peak_dbtp: -1.0,
        recommended_gain_db: 0.0,
        safe_gain_db: 0.0,
        is_compliant: false,
        compliance: ComplianceResult {
            standard: Standard::EbuR128,
            loudness_compliant: false,
            peak_compliant: false,
            lra_acceptable: false,
            integrated_lufs: f64::NEG_INFINITY,
            true_peak_dbtp: f64::NEG_INFINITY,
            loudness_range: 0.0,
            target_lufs: -23.0,
            max_peak_dbtp: -1.0,
            deviation_lu: 0.0,
        },
        metrics: LoudnessMetrics::default(),
        standard: Standard::EbuR128,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_config_creation() {
        let config = BatchConfig::new(Standard::EbuR128);
        assert!(config.enable_limiter);
        assert!(config.write_metadata);

        let minimal = BatchConfig::minimal(Standard::Spotify);
        assert!(!minimal.enable_limiter);
        assert!(!minimal.write_metadata);
    }

    #[test]
    fn test_batch_processor_creation() {
        let config = BatchConfig::new(Standard::EbuR128);
        let processor = BatchProcessor::new(config);
        assert!(processor.config().enable_limiter);
    }

    #[test]
    fn test_batch_result() {
        let result = BatchResult::failure(PathBuf::from("test.wav"), "Test error".to_string());
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_batch_report_format() {
        let report = BatchReport {
            total_files: 10,
            successful_files: 8,
            failed_files: 2,
            average_gain_db: -3.5,
            total_processing_time_s: 125.5,
        };

        let formatted = report.format();
        assert!(formatted.contains("Total files: 10"));
        assert!(formatted.contains("Successful: 8"));
    }
}
