// Copyright 2024 OxiMedia Project
// Licensed under the Apache License, Version 2.0

//! Rendering pipeline management (pre-render, render, post-render).

use crate::error::{Error, Result};
use crate::job::{Job, JobId};
use crate::worker::WorkerId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Pipeline stage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipelineStage {
    /// Pre-render stage (validation, setup)
    PreRender,
    /// Render stage (actual rendering)
    Render,
    /// Post-render stage (verification, assembly)
    PostRender,
}

impl std::fmt::Display for PipelineStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PreRender => write!(f, "PreRender"),
            Self::Render => write!(f, "Render"),
            Self::PostRender => write!(f, "PostRender"),
        }
    }
}

/// Pipeline task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineTask {
    /// Task ID
    pub id: String,
    /// Job ID
    pub job_id: JobId,
    /// Stage
    pub stage: PipelineStage,
    /// Status
    pub status: TaskStatus,
    /// Started at
    pub started_at: Option<DateTime<Utc>>,
    /// Completed at
    pub completed_at: Option<DateTime<Utc>>,
    /// Error message
    pub error: Option<String>,
}

/// Task status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Pending
    Pending,
    /// Running
    Running,
    /// Completed
    Completed,
    /// Failed
    Failed,
}

/// Pre-render result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreRenderResult {
    /// Assets verified
    pub assets_verified: bool,
    /// Dependencies resolved
    pub dependencies_resolved: bool,
    /// Estimated frames
    pub estimated_frames: u32,
    /// Estimated cost
    pub estimated_cost: f64,
    /// Estimated time (seconds)
    pub estimated_time: f64,
    /// Issues found
    pub issues: Vec<String>,
}

/// Render result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderResult {
    /// Frame number
    pub frame: u32,
    /// Output path
    pub output_path: PathBuf,
    /// Render time (seconds)
    pub render_time: f64,
    /// Worker ID
    pub worker_id: WorkerId,
    /// Success
    pub success: bool,
    /// Error message
    pub error: Option<String>,
}

/// Post-render result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostRenderResult {
    /// All frames verified
    pub all_frames_verified: bool,
    /// Output assembled
    pub output_assembled: bool,
    /// Final output path
    pub final_output_path: Option<PathBuf>,
    /// Quality metrics
    pub quality_metrics: HashMap<String, f64>,
}

/// Pipeline executor
pub struct Pipeline {
    tasks: HashMap<JobId, Vec<PipelineTask>>,
    pre_render_results: HashMap<JobId, PreRenderResult>,
    render_results: HashMap<JobId, Vec<RenderResult>>,
    post_render_results: HashMap<JobId, PostRenderResult>,
}

impl Pipeline {
    /// Create a new pipeline
    #[must_use]
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            pre_render_results: HashMap::new(),
            render_results: HashMap::new(),
            post_render_results: HashMap::new(),
        }
    }

    /// Execute pre-render stage
    pub async fn execute_pre_render(&mut self, job: &Job) -> Result<PreRenderResult> {
        let task = PipelineTask {
            id: format!("{}-prerender", job.id),
            job_id: job.id,
            stage: PipelineStage::PreRender,
            status: TaskStatus::Running,
            started_at: Some(Utc::now()),
            completed_at: None,
            error: None,
        };

        self.tasks.entry(job.id).or_default().push(task);

        // Asset verification
        let assets_verified = self.verify_assets(job).await?;

        // Dependency resolution
        let dependencies_resolved = self.resolve_dependencies(job).await?;

        // Estimate resources
        let (estimated_frames, estimated_cost, estimated_time) = self.estimate_resources(job);

        // Collect issues
        let mut issues = Vec::new();
        if !assets_verified {
            issues.push("Some assets could not be verified".to_string());
        }
        if !dependencies_resolved {
            issues.push("Some dependencies could not be resolved".to_string());
        }

        let result = PreRenderResult {
            assets_verified,
            dependencies_resolved,
            estimated_frames,
            estimated_cost,
            estimated_time,
            issues,
        };

        // Update task
        if let Some(task) = self
            .tasks
            .get_mut(&job.id)
            .and_then(|tasks| tasks.last_mut())
        {
            task.status = if result.issues.is_empty() {
                TaskStatus::Completed
            } else {
                TaskStatus::Failed
            };
            task.completed_at = Some(Utc::now());
            if !result.issues.is_empty() {
                task.error = Some(result.issues.join(", "));
            }
        }

        self.pre_render_results.insert(job.id, result.clone());

        Ok(result)
    }

    /// Record render result
    pub fn record_render_result(&mut self, job_id: JobId, result: RenderResult) {
        self.render_results.entry(job_id).or_default().push(result);
    }

    /// Execute post-render stage.
    ///
    /// Note: until real output assembly is implemented (see
    /// [`Self::assemble_output`]), this always returns `Err` after honestly
    /// recording the real frame-verification result and marking the
    /// pipeline task `Failed` with the real error — it never fabricates a
    /// completed [`PostRenderResult`].
    pub async fn execute_post_render(&mut self, job: &Job) -> Result<PostRenderResult> {
        let task = PipelineTask {
            id: format!("{}-postrender", job.id),
            job_id: job.id,
            stage: PipelineStage::PostRender,
            status: TaskStatus::Running,
            started_at: Some(Utc::now()),
            completed_at: None,
            error: None,
        };

        self.tasks.entry(job.id).or_default().push(task);

        // Verify all frames (real check against recorded render results).
        let all_frames_verified = self.verify_all_frames(job).await?;

        // Assemble output. Not implemented today (no muxing dependency in
        // this crate) — record the failure honestly on the task before
        // propagating, instead of leaving it stuck at `Running`.
        let (output_assembled, final_output_path) = match self.assemble_output(job).await {
            Ok(v) => v,
            Err(e) => {
                if let Some(task) = self
                    .tasks
                    .get_mut(&job.id)
                    .and_then(|tasks| tasks.last_mut())
                {
                    task.status = TaskStatus::Failed;
                    task.completed_at = Some(Utc::now());
                    task.error = Some(e.to_string());
                }
                return Err(e);
            }
        };

        // Calculate quality metrics
        let quality_metrics = self.calculate_quality_metrics(job).await?;

        let result = PostRenderResult {
            all_frames_verified,
            output_assembled,
            final_output_path,
            quality_metrics,
        };

        // Update task
        if let Some(task) = self
            .tasks
            .get_mut(&job.id)
            .and_then(|tasks| tasks.last_mut())
        {
            task.status = if all_frames_verified && output_assembled {
                TaskStatus::Completed
            } else {
                TaskStatus::Failed
            };
            task.completed_at = Some(Utc::now());
        }

        self.post_render_results.insert(job.id, result.clone());

        Ok(result)
    }

    /// Verify assets
    async fn verify_assets(&self, job: &Job) -> Result<bool> {
        // Check if project file exists
        if !job.submission.project_file.exists() {
            return Ok(false);
        }

        // Check all dependencies
        for dep in &job.submission.dependencies {
            if !dep.exists() {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Resolve dependencies.
    ///
    /// Real check: every declared dependency asset must actually exist on
    /// disk. A job with no declared dependencies is vacuously resolved
    /// (there is nothing to resolve). An earlier revision returned
    /// `!dependencies.is_empty()` — a fabricated signal derived from list
    /// length rather than any real resolution check (and backwards: it
    /// reported jobs with *no* dependencies as unresolved).
    // TODO(0.2.x): real dependency resolution — download missing assets,
    // verify checksums, set up plugin paths, and check license
    // availability. Today this only confirms each dependency path exists.
    async fn resolve_dependencies(&self, job: &Job) -> Result<bool> {
        for dep in &job.submission.dependencies {
            if !dep.exists() {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Estimate resources
    fn estimate_resources(&self, job: &Job) -> (u32, f64, f64) {
        // Get frame count
        let frame_count = match &job.submission.job_type {
            crate::job::JobType::ImageSequence {
                start_frame,
                end_frame,
            } => end_frame - start_frame + 1,
            crate::job::JobType::VideoRender { .. } => 100,
            _ => 1,
        };

        // Estimate cost and time
        let estimated_cost = f64::from(frame_count) * 0.01;
        let estimated_time = f64::from(frame_count) * 10.0; // 10 seconds per frame

        (frame_count, estimated_cost, estimated_time)
    }

    /// Verify all frames.
    ///
    /// Real check: every recorded [`RenderResult`] for the job must report
    /// `success`, and its `output_path` must exist on disk and be
    /// non-empty. If no render results have been recorded yet (or fewer
    /// were recorded than the pre-render stage estimated), verification
    /// honestly reports `false` rather than the previous hardcoded `true`.
    // TODO(0.2.x): also verify per-frame checksums and detect corruption
    // (e.g. via a digest recorded by the render worker), not just
    // existence/non-emptiness of the output file.
    async fn verify_all_frames(&self, job: &Job) -> Result<bool> {
        let Some(results) = self.render_results.get(&job.id) else {
            return Ok(false);
        };
        if results.is_empty() {
            return Ok(false);
        }

        if let Some(pre) = self.pre_render_results.get(&job.id) {
            if (results.len() as u32) < pre.estimated_frames {
                return Ok(false);
            }
        }

        for result in results {
            if !result.success {
                return Ok(false);
            }
            match std::fs::metadata(&result.output_path) {
                Ok(meta) if meta.len() > 0 => {}
                _ => return Ok(false),
            }
        }

        Ok(true)
    }

    /// Assemble output.
    ///
    /// Honesty note: this crate has no video muxing/encoding dependency (no
    /// `oximedia-container`/`oximedia-codec` in `Cargo.toml`), so it cannot
    /// actually combine rendered frames into a final deliverable. An
    /// earlier revision fabricated a hardcoded `/output/{job_id}.mp4` path
    /// and reported `output_assembled: true` without writing anything to
    /// disk. That is fabricated success. This now fails honestly instead.
    // TODO(0.2.x): real output assembly — combine image sequences into a
    // video (requires a muxing/encoding dependency such as
    // oximedia-container), merge render passes, and apply final
    // processing. Until then this must not report success.
    async fn assemble_output(&self, job: &Job) -> Result<(bool, Option<PathBuf>)> {
        Err(Error::Other(format!(
            "output assembly for job {} is not implemented: no muxing/encoding capability \
             is available in oximedia-renderfarm",
            job.id
        )))
    }

    /// Calculate quality metrics.
    ///
    /// Honesty note: PSNR/SSIM are full-reference metrics that require a
    /// reference frame to compare against. This render-farm pipeline has no
    /// reference (it renders from a scene/project; it does not transcode an
    /// existing reference video), so a real full-reference score cannot be
    /// computed here. An earlier revision fabricated `psnr: 42.0, ssim:
    /// 0.95` for every job regardless of content. That is fabricated
    /// success. This now fails honestly instead.
    // TODO(0.2.x): real quality metrics — either accept an explicit
    // reference asset for full-reference PSNR/SSIM/VMAF (reusing
    // oximedia-quality's public API), or compute no-reference metrics
    // (blur/noise/blockiness) directly from the rendered output frames.
    async fn calculate_quality_metrics(&self, job: &Job) -> Result<HashMap<String, f64>> {
        Err(Error::Other(format!(
            "quality metrics for job {} are not implemented: no reference frame is available \
             and no quality-assessment dependency is wired into oximedia-renderfarm",
            job.id
        )))
    }

    /// Get pre-render result
    #[must_use]
    pub fn get_pre_render_result(&self, job_id: JobId) -> Option<&PreRenderResult> {
        self.pre_render_results.get(&job_id)
    }

    /// Get render results
    #[must_use]
    pub fn get_render_results(&self, job_id: JobId) -> Vec<&RenderResult> {
        self.render_results
            .get(&job_id)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Get post-render result
    #[must_use]
    pub fn get_post_render_result(&self, job_id: JobId) -> Option<&PostRenderResult> {
        self.post_render_results.get(&job_id)
    }

    /// Get pipeline tasks for job
    #[must_use]
    pub fn get_tasks(&self, job_id: JobId) -> Vec<&PipelineTask> {
        self.tasks
            .get(&job_id)
            .map_or_else(Vec::new, |tasks| tasks.iter().collect())
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{JobSubmission, Priority};

    fn tmp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("oximedia-renderfarm-pipeline-{name}"))
    }

    #[tokio::test]
    async fn test_pipeline_creation() {
        let pipeline = Pipeline::new();
        assert_eq!(pipeline.tasks.len(), 0);
    }

    #[tokio::test]
    async fn test_pre_render_execution() -> Result<()> {
        let mut pipeline = Pipeline::new();

        let submission = JobSubmission::builder()
            .project_file(tmp_path("test.blend"))
            .frame_range(1, 10)
            .priority(Priority::Normal)
            .build()?;

        let job = Job::new(submission);

        let result = pipeline.execute_pre_render(&job).await?;
        assert!(result.estimated_frames > 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_post_render_execution_is_honest_about_unimplemented_assembly() -> Result<()> {
        // CHANGED: this test previously pinned the fabricated behavior —
        // execute_post_render() used to always succeed with a hardcoded
        // `/output/{job_id}.mp4` path and `output_assembled: true`, even
        // though nothing was ever written to disk. Output assembly is not
        // implemented (no muxing dependency in this crate), so
        // execute_post_render must now honestly report that via `Err`
        // instead of fabricating a finished output.
        let mut pipeline = Pipeline::new();

        let submission = JobSubmission::builder()
            .project_file(tmp_path("test.blend"))
            .frame_range(1, 10)
            .build()?;

        let job = Job::new(submission);

        let result = pipeline.execute_post_render(&job).await;
        assert!(
            result.is_err(),
            "post-render must not fabricate a completed assembly"
        );

        // The task must be recorded as Failed with the real error message,
        // not left dangling at `Running` forever.
        let tasks = pipeline.get_tasks(job.id);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, TaskStatus::Failed);
        assert!(tasks[0].error.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_resolve_dependencies_real_check() -> Result<()> {
        let pipeline = Pipeline::new();

        // No dependencies declared: vacuously resolved (nothing to
        // resolve). The old `!dependencies.is_empty()` fabrication would
        // have reported this case as `false`.
        let submission_empty = JobSubmission::builder()
            .project_file(tmp_path("resolve-deps-empty.blend"))
            .frame_range(1, 5)
            .build()?;
        let job_empty = Job::new(submission_empty);
        assert!(pipeline.resolve_dependencies(&job_empty).await?);

        // A declared dependency that does not exist on disk: not resolved.
        let missing_dep = tmp_path("resolve-deps-missing-dep.bin");
        let _ = std::fs::remove_file(&missing_dep);
        let submission_missing = JobSubmission::builder()
            .project_file(tmp_path("resolve-deps-missing.blend"))
            .frame_range(1, 5)
            .dependency(missing_dep)
            .build()?;
        let job_missing = Job::new(submission_missing);
        assert!(!pipeline.resolve_dependencies(&job_missing).await?);

        // A declared dependency that does exist on disk: resolved.
        let present_dep = tmp_path("resolve-deps-present-dep.bin");
        std::fs::write(&present_dep, b"asset bytes")?;
        let submission_present = JobSubmission::builder()
            .project_file(tmp_path("resolve-deps-present.blend"))
            .frame_range(1, 5)
            .dependency(present_dep.clone())
            .build()?;
        let job_present = Job::new(submission_present);
        assert!(pipeline.resolve_dependencies(&job_present).await?);

        std::fs::remove_file(&present_dep).ok();
        Ok(())
    }

    #[tokio::test]
    async fn test_verify_all_frames_real_check() -> Result<()> {
        let mut pipeline = Pipeline::new();

        let submission = JobSubmission::builder()
            .project_file(tmp_path("verify-frames.blend"))
            .frame_range(1, 2)
            .build()?;
        let job = Job::new(submission);

        // No render results recorded yet: honestly not verified (the old
        // code hardcoded `true` here regardless).
        assert!(!pipeline.verify_all_frames(&job).await?);

        // A "successful" result pointing at a file that does not actually
        // exist must still fail real verification.
        pipeline.record_render_result(
            job.id,
            RenderResult {
                frame: 1,
                output_path: tmp_path("verify-frames-missing.png"),
                render_time: 1.0,
                worker_id: WorkerId::new(),
                success: true,
                error: None,
            },
        );
        assert!(!pipeline.verify_all_frames(&job).await?);

        // A real, non-empty frame file on disk: now it verifies.
        let real_frame = tmp_path("verify-frames-real.png");
        std::fs::write(&real_frame, b"not really a png but non-empty")?;
        let mut pipeline2 = Pipeline::new();
        pipeline2.record_render_result(
            job.id,
            RenderResult {
                frame: 1,
                output_path: real_frame.clone(),
                render_time: 1.0,
                worker_id: WorkerId::new(),
                success: true,
                error: None,
            },
        );
        assert!(pipeline2.verify_all_frames(&job).await?);

        std::fs::remove_file(&real_frame).ok();
        Ok(())
    }

    #[tokio::test]
    async fn test_assemble_output_is_honest_err() -> Result<()> {
        let pipeline = Pipeline::new();
        let submission = JobSubmission::builder()
            .project_file(tmp_path("assemble.blend"))
            .frame_range(1, 5)
            .build()?;
        let job = Job::new(submission);

        let result = pipeline.assemble_output(&job).await;
        assert!(
            result.is_err(),
            "assemble_output must not fabricate a finished output path"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_calculate_quality_metrics_is_honest_err() -> Result<()> {
        let pipeline = Pipeline::new();
        let submission = JobSubmission::builder()
            .project_file(tmp_path("quality.blend"))
            .frame_range(1, 5)
            .build()?;
        let job = Job::new(submission);

        let result = pipeline.calculate_quality_metrics(&job).await;
        assert!(
            result.is_err(),
            "calculate_quality_metrics must not fabricate hardcoded psnr/ssim values"
        );

        Ok(())
    }

    #[test]
    fn test_pipeline_stage_display() {
        assert_eq!(PipelineStage::PreRender.to_string(), "PreRender");
        assert_eq!(PipelineStage::Render.to_string(), "Render");
        assert_eq!(PipelineStage::PostRender.to_string(), "PostRender");
    }

    #[tokio::test]
    async fn test_get_tasks() -> Result<()> {
        let mut pipeline = Pipeline::new();

        let submission = JobSubmission::builder()
            .project_file(tmp_path("test.blend"))
            .frame_range(1, 10)
            .build()?;

        let job = Job::new(submission);
        let job_id = job.id;

        pipeline.execute_pre_render(&job).await?;

        let tasks = pipeline.get_tasks(job_id);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].stage, PipelineStage::PreRender);

        Ok(())
    }
}
