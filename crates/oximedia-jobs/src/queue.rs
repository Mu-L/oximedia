// Copyright 2024 OxiMedia Project
// Licensed under the Apache License, Version 2.0

//! Job queue implementation with priority and dependency management.

use crate::job::{Job, JobStatus};
use crate::metrics::{MetricsCollector, QueueStats};
use crate::persistence::{JobPersistence, PersistenceError};
use crate::worker::{JobExecutor, WorkerPool};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Queue errors
#[derive(Debug, Error)]
pub enum QueueError {
    /// Persistence error
    #[error("Persistence error: {0}")]
    Persistence(#[from] PersistenceError),

    /// Job not found
    #[error("Job not found: {0}")]
    JobNotFound(Uuid),

    /// Invalid job state
    #[error("Invalid job state: {0}")]
    InvalidState(String),

    /// Dependency error
    #[error("Dependency error: {0}")]
    DependencyError(String),

    /// Queue shutdown
    #[error("Queue is shutdown")]
    Shutdown,
}

/// Result type for queue operations
pub type Result<T> = std::result::Result<T, QueueError>;

/// Job queue configuration
#[derive(Debug, Clone)]
pub struct QueueConfig {
    /// Database path
    pub db_path: String,
    /// Polling interval in seconds
    pub poll_interval_secs: u64,
    /// Max concurrent jobs
    pub max_concurrent_jobs: usize,
    /// Enable job retry
    pub enable_retry: bool,
    /// Cleanup interval in days
    pub cleanup_interval_days: i64,
    /// Enable scheduled jobs
    pub enable_scheduled_jobs: bool,
    /// Enable deadline checking
    pub enable_deadline_checking: bool,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            db_path: "jobs.db".to_string(),
            poll_interval_secs: 5,
            max_concurrent_jobs: 10,
            enable_retry: true,
            cleanup_interval_days: 30,
            enable_scheduled_jobs: true,
            enable_deadline_checking: true,
        }
    }
}

/// Job queue
pub struct JobQueue {
    /// Persistence layer
    persistence: Arc<JobPersistence>,
    /// Worker pool
    pub worker_pool: Arc<WorkerPool>,
    /// Metrics collector
    metrics: Arc<MetricsCollector>,
    /// Configuration
    config: QueueConfig,
    /// Running jobs
    running_jobs: Arc<RwLock<HashMap<Uuid, Job>>>,
    /// Completed jobs cache
    completed_jobs: Arc<RwLock<HashMap<Uuid, JobStatus>>>,
    /// Shutdown flag
    shutdown: Arc<RwLock<bool>>,
}

impl Clone for JobQueue {
    fn clone(&self) -> Self {
        Self {
            persistence: self.persistence.clone(),
            worker_pool: self.worker_pool.clone(),
            metrics: self.metrics.clone(),
            config: self.config.clone(),
            running_jobs: self.running_jobs.clone(),
            completed_jobs: self.completed_jobs.clone(),
            shutdown: self.shutdown.clone(),
        }
    }
}

impl JobQueue {
    /// Create a new job queue
    ///
    /// # Errors
    ///
    /// Returns an error if persistence initialization fails
    pub fn new(
        config: QueueConfig,
        executor: Arc<dyn JobExecutor>,
        metrics: Arc<MetricsCollector>,
        worker_config: crate::worker::WorkerConfig,
    ) -> Result<Self> {
        let persistence = Arc::new(JobPersistence::new(&config.db_path)?);
        let worker_pool = Arc::new(WorkerPool::new(executor, metrics.clone(), worker_config));

        Ok(Self {
            persistence,
            worker_pool,
            metrics,
            config,
            running_jobs: Arc::new(RwLock::new(HashMap::new())),
            completed_jobs: Arc::new(RwLock::new(HashMap::new())),
            shutdown: Arc::new(RwLock::new(false)),
        })
    }

    /// Create a new job queue with in-memory persistence (for testing)
    ///
    /// # Errors
    ///
    /// Returns an error if persistence initialization fails
    pub fn in_memory(
        executor: Arc<dyn JobExecutor>,
        metrics: Arc<MetricsCollector>,
        worker_config: crate::worker::WorkerConfig,
    ) -> Result<Self> {
        let persistence = Arc::new(JobPersistence::in_memory()?);
        let worker_pool = Arc::new(WorkerPool::new(executor, metrics.clone(), worker_config));

        Ok(Self {
            persistence,
            worker_pool,
            metrics,
            config: QueueConfig::default(),
            running_jobs: Arc::new(RwLock::new(HashMap::new())),
            completed_jobs: Arc::new(RwLock::new(HashMap::new())),
            shutdown: Arc::new(RwLock::new(false)),
        })
    }

    /// Start the queue
    pub async fn start(&self) {
        info!("Starting job queue");
        self.worker_pool.start().await;
        self.load_existing_jobs().await;
        self.start_queue_processor();
        self.start_scheduled_job_processor();
        self.start_deadline_checker();
        self.start_stats_updater();
        self.start_cleanup_task();
    }

    /// Load existing jobs from persistence
    async fn load_existing_jobs(&self) {
        match self.persistence.get_all_jobs() {
            Ok(jobs) => {
                info!("Loaded {} existing jobs", jobs.len());
                let mut completed = self.completed_jobs.write().await;
                for job in jobs {
                    if matches!(
                        job.status,
                        JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
                    ) {
                        completed.insert(job.id, job.status);
                    }
                }
            }
            Err(e) => {
                error!("Failed to load existing jobs: {}", e);
            }
        }
    }

    /// Submit a job to the queue
    ///
    /// # Errors
    ///
    /// Returns an error if job cannot be persisted
    pub async fn submit(&self, job: Job) -> Result<Uuid> {
        if *self.shutdown.read().await {
            return Err(QueueError::Shutdown);
        }

        let job_id = job.id;
        info!("Submitting job {} ({})", job.name, job_id);

        self.persistence.save_job(&job)?;

        Ok(job_id)
    }

    /// Cancel a job
    ///
    /// # Errors
    ///
    /// Returns an error if job cannot be found or cancelled
    pub async fn cancel(&self, job_id: Uuid) -> Result<()> {
        info!("Cancelling job {}", job_id);

        let mut job = self.persistence.get_job(job_id)?;

        match job.status {
            JobStatus::Pending | JobStatus::Scheduled | JobStatus::Waiting => {
                job.mark_cancelled();
                self.persistence.save_job(&job)?;
                Ok(())
            }
            JobStatus::Running => {
                self.worker_pool.cancel_job(job_id).await;
                job.mark_cancelled();
                self.persistence.save_job(&job)?;
                Ok(())
            }
            _ => Err(QueueError::InvalidState(format!(
                "Cannot cancel job in state: {}",
                job.status
            ))),
        }
    }

    /// Get job by ID
    ///
    /// # Errors
    ///
    /// Returns an error if job cannot be found
    #[allow(clippy::unused_async)]
    pub async fn get_job(&self, job_id: Uuid) -> Result<Job> {
        self.persistence.get_job(job_id).map_err(Into::into)
    }

    /// Get jobs by status
    ///
    /// # Errors
    ///
    /// Returns an error if persistence operation fails
    #[allow(clippy::unused_async)]
    pub async fn get_jobs_by_status(&self, status: JobStatus) -> Result<Vec<Job>> {
        self.persistence
            .get_jobs_by_status(status)
            .map_err(Into::into)
    }

    /// Get all jobs
    ///
    /// # Errors
    ///
    /// Returns an error if persistence operation fails
    #[allow(clippy::unused_async)]
    pub async fn get_all_jobs(&self) -> Result<Vec<Job>> {
        self.persistence.get_all_jobs().map_err(Into::into)
    }

    /// Update job progress
    ///
    /// # Errors
    ///
    /// Returns an error if persistence operation fails
    #[allow(clippy::unused_async)]
    pub async fn update_progress(&self, job_id: Uuid, progress: u8) -> Result<()> {
        self.persistence
            .update_job_progress(job_id, progress)
            .map_err(Into::into)
    }

    /// Shutdown the queue
    pub async fn shutdown(&self) {
        info!("Shutting down job queue");
        *self.shutdown.write().await = true;
        self.worker_pool.shutdown().await;
        sleep(Duration::from_secs(2)).await;
        info!("Job queue shutdown complete");
    }

    /// Start queue processor
    fn start_queue_processor(&self) {
        let persistence = self.persistence.clone();
        let worker_pool = self.worker_pool.clone();
        let running_jobs = self.running_jobs.clone();
        let completed_jobs = self.completed_jobs.clone();
        let shutdown = self.shutdown.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(Duration::from_secs(config.poll_interval_secs));

            loop {
                interval.tick().await;

                if *shutdown.read().await {
                    break;
                }

                Self::process_pending_jobs(
                    &persistence,
                    &worker_pool,
                    &running_jobs,
                    &completed_jobs,
                    config.max_concurrent_jobs,
                    config.enable_retry,
                )
                .await;
            }
        });
    }

    /// Process pending jobs
    async fn process_pending_jobs(
        persistence: &JobPersistence,
        worker_pool: &WorkerPool,
        running_jobs: &RwLock<HashMap<Uuid, Job>>,
        completed_jobs: &RwLock<HashMap<Uuid, JobStatus>>,
        max_concurrent: usize,
        enable_retry: bool,
    ) {
        let running_count = running_jobs.read().await.len();
        if running_count >= max_concurrent {
            debug!("Max concurrent jobs reached: {}", running_count);
            return;
        }

        let available_slots = max_concurrent - running_count;

        let pending_jobs = match persistence.get_pending_jobs() {
            Ok(jobs) => jobs,
            Err(e) => {
                error!("Failed to get pending jobs: {}", e);
                return;
            }
        };

        let completed = completed_jobs.read().await;

        let mut ready_jobs: Vec<Job> = pending_jobs
            .into_iter()
            .filter(|job| job.is_ready(&completed))
            .take(available_slots)
            .collect();

        drop(completed);

        if enable_retry {
            let failed_jobs = match persistence.get_jobs_by_status(JobStatus::Failed) {
                Ok(jobs) => jobs,
                Err(e) => {
                    error!("Failed to get failed jobs: {}", e);
                    vec![]
                }
            };

            for mut job in failed_jobs {
                if job.should_retry() {
                    if let Some(retry_time) = job.next_retry_time() {
                        if Utc::now() >= retry_time {
                            info!("Retrying job {} (attempt {})", job.id, job.attempts + 1);
                            job.reset_for_retry();
                            if let Err(e) = persistence.save_job(&job) {
                                error!("Failed to save retry job: {}", e);
                            } else {
                                ready_jobs.push(job);
                            }
                        }
                    }
                }
            }
        }

        for job in ready_jobs {
            let job_id = job.id;
            running_jobs.write().await.insert(job_id, job.clone());

            if let Err(e) = worker_pool.submit(job).await {
                error!("Failed to submit job {}: {}", job_id, e);
                running_jobs.write().await.remove(&job_id);
            }
        }

        Self::check_completed_jobs(persistence, running_jobs, completed_jobs).await;
    }

    /// Check for completed jobs
    async fn check_completed_jobs(
        persistence: &JobPersistence,
        running_jobs: &RwLock<HashMap<Uuid, Job>>,
        completed_jobs: &RwLock<HashMap<Uuid, JobStatus>>,
    ) {
        let running = running_jobs.read().await;
        let job_ids: Vec<Uuid> = running.keys().copied().collect();
        drop(running);

        for job_id in job_ids {
            if let Ok(job) = persistence.get_job(job_id) {
                if matches!(
                    job.status,
                    JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
                ) {
                    running_jobs.write().await.remove(&job_id);
                    completed_jobs.write().await.insert(job_id, job.status);

                    if !job.next_jobs.is_empty() {
                        Self::trigger_next_jobs(persistence, &job).await;
                    }
                }
            }
        }
    }

    /// Trigger next jobs in pipeline
    #[allow(clippy::unused_async)]
    async fn trigger_next_jobs(persistence: &JobPersistence, completed_job: &Job) {
        for next_id in &completed_job.next_jobs {
            if let Ok(mut next_job) = persistence.get_job(*next_id) {
                if next_job.status == JobStatus::Waiting {
                    next_job.status = JobStatus::Pending;
                    if let Err(e) = persistence.save_job(&next_job) {
                        error!("Failed to trigger next job {}: {}", next_id, e);
                    } else {
                        info!("Triggered next job {} after {}", next_id, completed_job.id);
                    }
                }
            }
        }
    }

    /// Start scheduled job processor
    fn start_scheduled_job_processor(&self) {
        if !self.config.enable_scheduled_jobs {
            return;
        }

        let persistence = self.persistence.clone();
        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                interval.tick().await;

                if *shutdown.read().await {
                    break;
                }

                match persistence.get_scheduled_jobs_ready() {
                    Ok(jobs) => {
                        for mut job in jobs {
                            job.status = JobStatus::Pending;
                            if let Err(e) = persistence.save_job(&job) {
                                error!("Failed to activate scheduled job {}: {}", job.id, e);
                            } else {
                                info!("Activated scheduled job {}", job.id);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to get scheduled jobs: {}", e);
                    }
                }
            }
        });
    }

    /// Start deadline checker
    fn start_deadline_checker(&self) {
        if !self.config.enable_deadline_checking {
            return;
        }

        let persistence = self.persistence.clone();
        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300));

            loop {
                interval.tick().await;

                if *shutdown.read().await {
                    break;
                }

                match persistence.get_jobs_past_deadline() {
                    Ok(jobs) => {
                        for mut job in jobs {
                            warn!("Job {} past deadline", job.id);
                            job.mark_failed("Deadline exceeded".to_string());
                            if let Err(e) = persistence.save_job(&job) {
                                error!("Failed to fail job {}: {}", job.id, e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to check deadlines: {}", e);
                    }
                }
            }
        });
    }

    /// Start stats updater
    fn start_stats_updater(&self) {
        let persistence = self.persistence.clone();
        let metrics = self.metrics.clone();
        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                if *shutdown.read().await {
                    break;
                }

                let mut stats = QueueStats {
                    total_jobs: persistence.count_jobs().unwrap_or(0),
                    pending_jobs: persistence
                        .count_jobs_by_status(JobStatus::Pending)
                        .unwrap_or(0),
                    running_jobs: persistence
                        .count_jobs_by_status(JobStatus::Running)
                        .unwrap_or(0),
                    completed_jobs: persistence
                        .count_jobs_by_status(JobStatus::Completed)
                        .unwrap_or(0),
                    failed_jobs: persistence
                        .count_jobs_by_status(JobStatus::Failed)
                        .unwrap_or(0),
                    cancelled_jobs: persistence
                        .count_jobs_by_status(JobStatus::Cancelled)
                        .unwrap_or(0),
                    scheduled_jobs: persistence
                        .count_jobs_by_status(JobStatus::Scheduled)
                        .unwrap_or(0),
                    ..Default::default()
                };

                stats.calculate_success_rate();

                let job_metrics = metrics.get_job_metrics().await;
                if !job_metrics.is_empty() {
                    let total_duration: f64 =
                        job_metrics.iter().filter_map(|m| m.duration_secs).sum();
                    #[allow(clippy::cast_precision_loss)]
                    {
                        stats.avg_processing_time_secs = total_duration / job_metrics.len() as f64;
                    }
                }

                metrics.update_queue_stats(stats).await;
            }
        });
    }

    /// Start cleanup task
    fn start_cleanup_task(&self) {
        let persistence = self.persistence.clone();
        let metrics = self.metrics.clone();
        let shutdown = self.shutdown.clone();
        let cleanup_days = self.config.cleanup_interval_days;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(86400));

            loop {
                interval.tick().await;

                if *shutdown.read().await {
                    break;
                }

                match persistence.cleanup_old_jobs(cleanup_days) {
                    Ok(deleted) => {
                        if deleted > 0 {
                            info!("Cleaned up {} old jobs", deleted);
                        }
                    }
                    Err(e) => {
                        error!("Failed to cleanup old jobs: {}", e);
                    }
                }

                metrics.cleanup_old_metrics(cleanup_days).await;
            }
        });
    }

    /// Get queue statistics
    pub async fn get_stats(&self) -> QueueStats {
        self.metrics.get_queue_stats().await
    }
}
