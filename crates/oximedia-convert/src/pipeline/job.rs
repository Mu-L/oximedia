// Copyright 2025 OxiMedia Contributors
// Licensed under the Apache License, Version 2.0

//! Conversion job management.

use super::{AudioSettings, VideoSettings};
use crate::filters::FilterChain;
use crate::formats::ContainerFormat;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// A conversion job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionJob {
    /// Job ID
    pub id: String,
    /// Input file path
    pub input: PathBuf,
    /// Output file path
    pub output: PathBuf,
    /// Output container format
    pub container: ContainerFormat,
    /// Video conversion settings
    pub video: Option<VideoSettings>,
    /// Audio conversion settings
    pub audio: Option<AudioSettings>,
    /// Filter chain to apply
    pub filters: Option<FilterChain>,
    /// Metadata to preserve/add
    pub metadata: HashMap<String, String>,
    /// Priority level
    pub priority: JobPriority,
    /// Job status
    pub status: JobStatus,
    /// Creation time
    pub created_at: SystemTime,
    /// Start time
    pub started_at: Option<SystemTime>,
    /// Completion time
    pub completed_at: Option<SystemTime>,
    /// Error message if failed
    pub error: Option<String>,
    /// Progress percentage (0-100)
    pub progress: f64,
}

/// Job priority level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum JobPriority {
    /// Low priority
    Low = 0,
    /// Normal priority
    Normal = 1,
    /// High priority
    High = 2,
    /// Critical priority
    Critical = 3,
}

/// Job status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    /// Job is queued
    Queued,
    /// Job is currently processing
    Processing,
    /// Job completed successfully
    Completed,
    /// Job failed with error
    Failed,
    /// Job was cancelled
    Cancelled,
}

impl ConversionJob {
    /// Create a new conversion job.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        input: PathBuf,
        output: PathBuf,
        container: ContainerFormat,
        video: Option<VideoSettings>,
        audio: Option<AudioSettings>,
        filters: Option<FilterChain>,
        metadata: HashMap<String, String>,
        priority: JobPriority,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            input,
            output,
            container,
            video,
            audio,
            filters,
            metadata,
            priority,
            status: JobStatus::Queued,
            created_at: SystemTime::now(),
            started_at: None,
            completed_at: None,
            error: None,
            progress: 0.0,
        }
    }

    /// Mark job as started.
    pub fn start(&mut self) {
        self.status = JobStatus::Processing;
        self.started_at = Some(SystemTime::now());
    }

    /// Mark job as completed.
    pub fn complete(&mut self) {
        self.status = JobStatus::Completed;
        self.completed_at = Some(SystemTime::now());
        self.progress = 100.0;
    }

    /// Mark job as failed.
    pub fn fail(&mut self, error: String) {
        self.status = JobStatus::Failed;
        self.completed_at = Some(SystemTime::now());
        self.error = Some(error);
    }

    /// Update job progress.
    pub fn update_progress(&mut self, progress: f64) {
        self.progress = progress.clamp(0.0, 100.0);
    }

    /// Get elapsed time.
    #[must_use]
    pub fn elapsed_time(&self) -> Option<Duration> {
        self.started_at.and_then(|start| {
            let end = self.completed_at.unwrap_or_else(SystemTime::now);
            end.duration_since(start).ok()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversion_job_creation() {
        let job = ConversionJob::new(
            PathBuf::from("input.mp4"),
            PathBuf::from("output.webm"),
            ContainerFormat::Webm,
            None,
            None,
            None,
            HashMap::new(),
            JobPriority::Normal,
        );

        assert_eq!(job.status, JobStatus::Queued);
        assert_eq!(job.priority, JobPriority::Normal);
        assert_eq!(job.progress, 0.0);
        assert!(job.started_at.is_none());
    }

    #[test]
    fn test_job_lifecycle() {
        let mut job = ConversionJob::new(
            PathBuf::from("input.mp4"),
            PathBuf::from("output.webm"),
            ContainerFormat::Webm,
            None,
            None,
            None,
            HashMap::new(),
            JobPriority::Normal,
        );

        job.start();
        assert_eq!(job.status, JobStatus::Processing);
        assert!(job.started_at.is_some());

        job.update_progress(50.0);
        assert_eq!(job.progress, 50.0);

        job.complete();
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(job.progress, 100.0);
        assert!(job.completed_at.is_some());
    }

    #[test]
    fn test_job_failure() {
        let mut job = ConversionJob::new(
            PathBuf::from("input.mp4"),
            PathBuf::from("output.webm"),
            ContainerFormat::Webm,
            None,
            None,
            None,
            HashMap::new(),
            JobPriority::Normal,
        );

        job.start();
        job.fail("Test error".to_string());

        assert_eq!(job.status, JobStatus::Failed);
        assert_eq!(job.error, Some("Test error".to_string()));
    }

    #[test]
    fn test_job_priority_ordering() {
        assert!(JobPriority::Critical > JobPriority::High);
        assert!(JobPriority::High > JobPriority::Normal);
        assert!(JobPriority::Normal > JobPriority::Low);
    }
}
