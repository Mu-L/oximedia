//! Transcoding pipeline orchestration and execution.

use crate::{
    MultiPassConfig, MultiPassEncoder, MultiPassMode, NormalizationConfig, ProgressTracker,
    QualityConfig, Result, TranscodeError, TranscodeOutput,
};
use std::path::PathBuf;

/// Pipeline stage in the transcoding workflow.
#[derive(Debug, Clone)]
pub enum PipelineStage {
    /// Input validation stage.
    Validation,
    /// Audio analysis stage (for normalization).
    AudioAnalysis,
    /// First pass encoding stage (analysis).
    FirstPass,
    /// Second pass encoding stage (final).
    SecondPass,
    /// Third pass encoding stage (optional).
    ThirdPass,
    /// Final encoding stage.
    Encode,
    /// Output verification stage.
    Verification,
}

/// Transcoding pipeline configuration.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Input file path.
    pub input: PathBuf,
    /// Output file path.
    pub output: PathBuf,
    /// Video codec name.
    pub video_codec: Option<String>,
    /// Audio codec name.
    pub audio_codec: Option<String>,
    /// Quality configuration.
    pub quality: Option<QualityConfig>,
    /// Multi-pass configuration.
    pub multipass: Option<MultiPassConfig>,
    /// Normalization configuration.
    pub normalization: Option<NormalizationConfig>,
    /// Enable progress tracking.
    pub track_progress: bool,
    /// Enable hardware acceleration.
    pub hw_accel: bool,
}

/// Transcoding pipeline orchestrator.
pub struct Pipeline {
    config: PipelineConfig,
    current_stage: PipelineStage,
    progress_tracker: Option<ProgressTracker>,
}

impl Pipeline {
    /// Creates a new pipeline with the given configuration.
    #[must_use]
    pub fn new(config: PipelineConfig) -> Self {
        Self {
            config,
            current_stage: PipelineStage::Validation,
            progress_tracker: None,
        }
    }

    /// Sets the progress tracker.
    pub fn set_progress_tracker(&mut self, tracker: ProgressTracker) {
        self.progress_tracker = Some(tracker);
    }

    /// Executes the pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error if any pipeline stage fails.
    pub async fn execute(&mut self) -> Result<TranscodeOutput> {
        // Validation stage
        self.current_stage = PipelineStage::Validation;
        self.validate()?;

        // Audio analysis (if normalization enabled)
        if self.config.normalization.is_some() {
            self.current_stage = PipelineStage::AudioAnalysis;
            self.analyze_audio().await?;
        }

        // Multi-pass encoding
        if let Some(multipass_config) = &self.config.multipass {
            let mut encoder = MultiPassEncoder::new(multipass_config.clone());

            while encoder.has_more_passes() {
                let pass = encoder.current_pass();
                self.current_stage = match pass {
                    1 => PipelineStage::FirstPass,
                    2 => PipelineStage::SecondPass,
                    _ => PipelineStage::ThirdPass,
                };

                self.execute_pass(pass, &encoder).await?;
                encoder.next_pass();
            }

            // Cleanup statistics files
            encoder.cleanup()?;
        } else {
            // Single-pass encoding
            self.current_stage = PipelineStage::Encode;
            self.execute_single_pass().await?;
        }

        // Verification
        self.current_stage = PipelineStage::Verification;
        self.verify_output().await
    }

    /// Gets the current pipeline stage.
    #[must_use]
    pub fn current_stage(&self) -> &PipelineStage {
        &self.current_stage
    }

    fn validate(&self) -> Result<()> {
        use crate::validation::{InputValidator, OutputValidator};

        // Validate input
        InputValidator::validate_path(
            self.config
                .input
                .to_str()
                .ok_or_else(|| TranscodeError::InvalidInput("Invalid input path".to_string()))?,
        )?;

        // Validate output
        OutputValidator::validate_path(
            self.config
                .output
                .to_str()
                .ok_or_else(|| TranscodeError::InvalidOutput("Invalid output path".to_string()))?,
            true,
        )?;

        Ok(())
    }

    async fn analyze_audio(&self) -> Result<()> {
        // Placeholder for audio analysis
        // In a real implementation, this would:
        // 1. Scan the audio track
        // 2. Measure loudness (LUFS)
        // 3. Calculate required gain
        Ok(())
    }

    async fn execute_pass(&self, _pass: u32, _encoder: &MultiPassEncoder) -> Result<()> {
        // Placeholder for pass execution
        // In a real implementation, this would:
        // 1. Configure encoder for the specific pass
        // 2. Process the input file
        // 3. Generate statistics or output
        Ok(())
    }

    async fn execute_single_pass(&self) -> Result<()> {
        // Placeholder for single-pass execution
        // In a real implementation, this would:
        // 1. Configure encoder
        // 2. Process the input file
        // 3. Write output
        Ok(())
    }

    async fn verify_output(&self) -> Result<TranscodeOutput> {
        // Placeholder for output verification
        // In a real implementation, this would:
        // 1. Check output file exists
        // 2. Verify it's playable
        // 3. Collect statistics

        Ok(TranscodeOutput {
            output_path: self.config.output.to_str().unwrap_or("unknown").to_string(),
            file_size: 0,
            duration: 0.0,
            video_bitrate: 0,
            audio_bitrate: 0,
            encoding_time: 0.0,
            speed_factor: 1.0,
        })
    }
}

/// Builder for transcoding pipelines.
pub struct TranscodePipeline {
    config: PipelineConfig,
}

impl TranscodePipeline {
    /// Creates a new pipeline builder.
    #[must_use]
    pub fn builder() -> TranscodePipelineBuilder {
        TranscodePipelineBuilder::new()
    }

    /// Sets the video codec.
    pub fn set_video_codec(&mut self, codec: &str) {
        self.config.video_codec = Some(codec.to_string());
    }

    /// Sets the audio codec.
    pub fn set_audio_codec(&mut self, codec: &str) {
        self.config.audio_codec = Some(codec.to_string());
    }

    /// Executes the pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error if the pipeline execution fails.
    pub async fn execute(&mut self) -> Result<TranscodeOutput> {
        let mut pipeline = Pipeline::new(self.config.clone());
        pipeline.execute().await
    }
}

/// Builder for creating transcoding pipelines.
pub struct TranscodePipelineBuilder {
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    video_codec: Option<String>,
    audio_codec: Option<String>,
    quality: Option<QualityConfig>,
    multipass: Option<MultiPassMode>,
    normalization: Option<NormalizationConfig>,
    track_progress: bool,
    hw_accel: bool,
}

impl TranscodePipelineBuilder {
    /// Creates a new pipeline builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            input: None,
            output: None,
            video_codec: None,
            audio_codec: None,
            quality: None,
            multipass: None,
            normalization: None,
            track_progress: false,
            hw_accel: true,
        }
    }

    /// Sets the input file.
    #[must_use]
    pub fn input(mut self, path: impl Into<PathBuf>) -> Self {
        self.input = Some(path.into());
        self
    }

    /// Sets the output file.
    #[must_use]
    pub fn output(mut self, path: impl Into<PathBuf>) -> Self {
        self.output = Some(path.into());
        self
    }

    /// Sets the video codec.
    #[must_use]
    pub fn video_codec(mut self, codec: impl Into<String>) -> Self {
        self.video_codec = Some(codec.into());
        self
    }

    /// Sets the audio codec.
    #[must_use]
    pub fn audio_codec(mut self, codec: impl Into<String>) -> Self {
        self.audio_codec = Some(codec.into());
        self
    }

    /// Sets the quality configuration.
    #[must_use]
    pub fn quality(mut self, quality: QualityConfig) -> Self {
        self.quality = Some(quality);
        self
    }

    /// Sets the multi-pass mode.
    #[must_use]
    pub fn multipass(mut self, mode: MultiPassMode) -> Self {
        self.multipass = Some(mode);
        self
    }

    /// Sets the normalization configuration.
    #[must_use]
    pub fn normalization(mut self, config: NormalizationConfig) -> Self {
        self.normalization = Some(config);
        self
    }

    /// Enables progress tracking.
    #[must_use]
    pub fn track_progress(mut self, enable: bool) -> Self {
        self.track_progress = enable;
        self
    }

    /// Enables hardware acceleration.
    #[must_use]
    pub fn hw_accel(mut self, enable: bool) -> Self {
        self.hw_accel = enable;
        self
    }

    /// Builds the transcoding pipeline.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing.
    pub fn build(self) -> Result<TranscodePipeline> {
        let input = self
            .input
            .ok_or_else(|| TranscodeError::InvalidInput("Input path not specified".to_string()))?;

        let output = self.output.ok_or_else(|| {
            TranscodeError::InvalidOutput("Output path not specified".to_string())
        })?;

        let multipass_config = self
            .multipass
            .map(|mode| MultiPassConfig::new(mode, "/tmp/transcode_stats.log"));

        Ok(TranscodePipeline {
            config: PipelineConfig {
                input,
                output,
                video_codec: self.video_codec,
                audio_codec: self.audio_codec,
                quality: self.quality,
                multipass: multipass_config,
                normalization: self.normalization,
                track_progress: self.track_progress,
                hw_accel: self.hw_accel,
            },
        })
    }
}

impl Default for TranscodePipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_builder() {
        let result = TranscodePipelineBuilder::new()
            .input("/tmp/input.mp4")
            .output("/tmp/output.mp4")
            .video_codec("vp9")
            .audio_codec("opus")
            .track_progress(true)
            .hw_accel(false)
            .build();

        assert!(result.is_ok());
        let pipeline = result.expect("should succeed in test");
        assert_eq!(pipeline.config.input, PathBuf::from("/tmp/input.mp4"));
        assert_eq!(pipeline.config.output, PathBuf::from("/tmp/output.mp4"));
        assert_eq!(pipeline.config.video_codec, Some("vp9".to_string()));
        assert_eq!(pipeline.config.audio_codec, Some("opus".to_string()));
        assert!(pipeline.config.track_progress);
        assert!(!pipeline.config.hw_accel);
    }

    #[test]
    fn test_pipeline_builder_missing_input() {
        let result = TranscodePipelineBuilder::new()
            .output("/tmp/output.mp4")
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_pipeline_builder_missing_output() {
        let result = TranscodePipelineBuilder::new()
            .input("/tmp/input.mp4")
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_pipeline_stage_flow() {
        let config = PipelineConfig {
            input: PathBuf::from("/tmp/input.mp4"),
            output: PathBuf::from("/tmp/output.mp4"),
            video_codec: None,
            audio_codec: None,
            quality: None,
            multipass: None,
            normalization: None,
            track_progress: false,
            hw_accel: true,
        };

        let pipeline = Pipeline::new(config);
        assert!(matches!(
            pipeline.current_stage(),
            PipelineStage::Validation
        ));
    }
}
