// Copyright 2024 OxiMedia Project
// Licensed under the Apache License, Version 2.0

//! Database persistence for job queue.

use crate::job::{Job, JobStatus, Priority};
use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, OptionalExtension, Row};
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

/// Persistence errors
#[derive(Debug, Error)]
pub enum PersistenceError {
    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// Connection pool error
    #[error("Connection pool error: {0}")]
    Pool(#[from] r2d2::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Job not found
    #[error("Job not found: {0}")]
    JobNotFound(Uuid),
}

/// Result type for persistence operations
pub type Result<T> = std::result::Result<T, PersistenceError>;

/// Job persistence layer using `SQLite`
pub struct JobPersistence {
    pool: Pool<SqliteConnectionManager>,
}

impl JobPersistence {
    /// Create a new persistence layer
    ///
    /// # Errors
    ///
    /// Returns an error if database initialization fails
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let manager = SqliteConnectionManager::file(db_path);
        let pool = Pool::new(manager)?;

        let persistence = Self { pool };
        persistence.initialize_schema()?;
        Ok(persistence)
    }

    /// Create an in-memory persistence layer (for testing)
    ///
    /// # Errors
    ///
    /// Returns an error if database initialization fails
    pub fn in_memory() -> Result<Self> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::new(manager)?;

        let persistence = Self { pool };
        persistence.initialize_schema()?;
        Ok(persistence)
    }

    /// Initialize database schema
    fn initialize_schema(&self) -> Result<()> {
        let conn = self.pool.get()?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                priority INTEGER NOT NULL,
                status TEXT NOT NULL,
                payload_type TEXT NOT NULL,
                payload_data TEXT NOT NULL,
                retry_policy TEXT NOT NULL,
                resource_quota TEXT NOT NULL,
                condition_type TEXT NOT NULL,
                condition_data TEXT NOT NULL,
                dependencies TEXT NOT NULL,
                tags TEXT NOT NULL,
                created_at TEXT NOT NULL,
                scheduled_at TEXT,
                started_at TEXT,
                completed_at TEXT,
                deadline TEXT,
                attempts INTEGER NOT NULL,
                error TEXT,
                progress INTEGER NOT NULL,
                worker_id TEXT,
                next_jobs TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_jobs_priority ON jobs(priority)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_jobs_scheduled ON jobs(scheduled_at)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_jobs_deadline ON jobs(deadline)",
            [],
        )?;

        Ok(())
    }

    /// Save a job to the database
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    pub fn save_job(&self, job: &Job) -> Result<()> {
        let conn = self.pool.get()?;

        let payload_type = job.payload.job_type();
        let payload_data = serde_json::to_string(&job.payload)?;
        let retry_policy = serde_json::to_string(&job.retry_policy)?;
        let resource_quota = serde_json::to_string(&job.resource_quota)?;
        let condition_data = serde_json::to_string(&job.condition)?;
        let dependencies = serde_json::to_string(&job.dependencies)?;
        let tags = serde_json::to_string(&job.tags)?;
        let next_jobs = serde_json::to_string(&job.next_jobs)?;

        conn.execute(
            "INSERT OR REPLACE INTO jobs (
                id, name, priority, status, payload_type, payload_data,
                retry_policy, resource_quota, condition_type, condition_data,
                dependencies, tags, created_at, scheduled_at, started_at,
                completed_at, deadline, attempts, error, progress, worker_id, next_jobs
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
            )",
            params![
                job.id.to_string(),
                job.name,
                job.priority as i32,
                job.status.to_string(),
                payload_type,
                payload_data,
                retry_policy,
                resource_quota,
                "condition",
                condition_data,
                dependencies,
                tags,
                job.created_at.to_rfc3339(),
                job.scheduled_at.map(|dt| dt.to_rfc3339()),
                job.started_at.map(|dt| dt.to_rfc3339()),
                job.completed_at.map(|dt| dt.to_rfc3339()),
                job.deadline.map(|dt| dt.to_rfc3339()),
                job.attempts,
                &job.error,
                job.progress,
                &job.worker_id,
                next_jobs,
            ],
        )?;

        Ok(())
    }

    /// Get a job by ID
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails or job is not found
    pub fn get_job(&self, id: Uuid) -> Result<Job> {
        let conn = self.pool.get()?;

        let job = conn
            .query_row(
                "SELECT * FROM jobs WHERE id = ?1",
                params![id.to_string()],
                Self::row_to_job,
            )
            .optional()?
            .ok_or(PersistenceError::JobNotFound(id))?;

        Ok(job)
    }

    /// Delete a job by ID
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    pub fn delete_job(&self, id: Uuid) -> Result<()> {
        let conn = self.pool.get()?;
        conn.execute("DELETE FROM jobs WHERE id = ?1", params![id.to_string()])?;
        Ok(())
    }

    /// Get jobs by status
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    pub fn get_jobs_by_status(&self, status: JobStatus) -> Result<Vec<Job>> {
        let conn = self.pool.get()?;

        let mut stmt = conn.prepare("SELECT * FROM jobs WHERE status = ?1")?;
        let jobs = stmt
            .query_map(params![status.to_string()], Self::row_to_job)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(jobs)
    }

    /// Get pending jobs ordered by priority
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    pub fn get_pending_jobs(&self) -> Result<Vec<Job>> {
        let conn = self.pool.get()?;

        let mut stmt = conn.prepare(
            "SELECT * FROM jobs
             WHERE status = 'pending'
             ORDER BY priority DESC, created_at ASC",
        )?;

        let jobs = stmt
            .query_map([], Self::row_to_job)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(jobs)
    }

    /// Get scheduled jobs that are ready to run
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    pub fn get_scheduled_jobs_ready(&self) -> Result<Vec<Job>> {
        let conn = self.pool.get()?;
        let now = Utc::now().to_rfc3339();

        let mut stmt = conn.prepare(
            "SELECT * FROM jobs
             WHERE status = 'scheduled' AND scheduled_at <= ?1
             ORDER BY priority DESC, scheduled_at ASC",
        )?;

        let jobs = stmt
            .query_map(params![now], Self::row_to_job)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(jobs)
    }

    /// Get jobs past their deadline
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    pub fn get_jobs_past_deadline(&self) -> Result<Vec<Job>> {
        let conn = self.pool.get()?;
        let now = Utc::now().to_rfc3339();

        let mut stmt = conn.prepare(
            "SELECT * FROM jobs
             WHERE deadline IS NOT NULL AND deadline < ?1
             AND status NOT IN ('completed', 'failed', 'cancelled')",
        )?;

        let jobs = stmt
            .query_map(params![now], Self::row_to_job)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(jobs)
    }

    /// Get all jobs
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    pub fn get_all_jobs(&self) -> Result<Vec<Job>> {
        let conn = self.pool.get()?;

        let mut stmt = conn.prepare("SELECT * FROM jobs ORDER BY created_at DESC")?;
        let jobs = stmt
            .query_map([], Self::row_to_job)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(jobs)
    }

    /// Get jobs by tag
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    pub fn get_jobs_by_tag(&self, tag: &str) -> Result<Vec<Job>> {
        let conn = self.pool.get()?;

        let mut stmt = conn.prepare("SELECT * FROM jobs WHERE tags LIKE ?1")?;
        let pattern = format!("%\"{tag}\" %");
        let jobs = stmt
            .query_map(params![pattern], Self::row_to_job)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(jobs)
    }

    /// Update job status
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    pub fn update_job_status(&self, id: Uuid, status: JobStatus) -> Result<()> {
        let conn = self.pool.get()?;

        conn.execute(
            "UPDATE jobs SET status = ?1 WHERE id = ?2",
            params![status.to_string(), id.to_string()],
        )?;

        Ok(())
    }

    /// Update job progress
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    pub fn update_job_progress(&self, id: Uuid, progress: u8) -> Result<()> {
        let conn = self.pool.get()?;

        conn.execute(
            "UPDATE jobs SET progress = ?1 WHERE id = ?2",
            params![progress, id.to_string()],
        )?;

        Ok(())
    }

    /// Get job count by status
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    pub fn count_jobs_by_status(&self, status: JobStatus) -> Result<usize> {
        let conn = self.pool.get()?;

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM jobs WHERE status = ?1",
            params![status.to_string()],
            |row| row.get(0),
        )?;

        Ok(count as usize)
    }

    /// Get total job count
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    pub fn count_jobs(&self) -> Result<usize> {
        let conn = self.pool.get()?;

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM jobs", [], |row| row.get(0))?;

        Ok(count as usize)
    }

    /// Clean up old completed jobs
    ///
    /// # Errors
    ///
    /// Returns an error if database operation fails
    pub fn cleanup_old_jobs(&self, days: i64) -> Result<usize> {
        let conn = self.pool.get()?;
        let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();

        let deleted = conn.execute(
            "DELETE FROM jobs
             WHERE status IN ('completed', 'failed', 'cancelled')
             AND completed_at < ?1",
            params![cutoff],
        )?;

        Ok(deleted)
    }

    /// Convert database row to Job
    fn row_to_job(row: &Row<'_>) -> rusqlite::Result<Job> {
        let id: String = row.get(0)?;
        let payload_data: String = row.get(5)?;
        let retry_policy: String = row.get(6)?;
        let resource_quota: String = row.get(7)?;
        let condition_data: String = row.get(9)?;
        let dependencies: String = row.get(10)?;
        let tags: String = row.get(11)?;
        let created_at: String = row.get(12)?;
        let scheduled_at: Option<String> = row.get(13)?;
        let started_at: Option<String> = row.get(14)?;
        let completed_at: Option<String> = row.get(15)?;
        let deadline: Option<String> = row.get(16)?;
        let next_jobs: String = row.get(21)?;

        let priority_val: i32 = row.get(2)?;
        let priority = match priority_val {
            0 => Priority::Low,
            1 => Priority::Normal,
            2 => Priority::High,
            _ => Priority::Normal,
        };

        let status_str: String = row.get(3)?;
        let status = match status_str.as_str() {
            "running" => JobStatus::Running,
            "completed" => JobStatus::Completed,
            "failed" => JobStatus::Failed,
            "cancelled" => JobStatus::Cancelled,
            "waiting" => JobStatus::Waiting,
            "scheduled" => JobStatus::Scheduled,
            _ => JobStatus::Pending,
        };

        Ok(Job {
            id: Uuid::parse_str(&id).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
            name: row.get(1)?,
            priority,
            status,
            payload: serde_json::from_str(&payload_data).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
            retry_policy: serde_json::from_str(&retry_policy).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
            resource_quota: serde_json::from_str(&resource_quota).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
            condition: serde_json::from_str(&condition_data).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    9,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
            dependencies: serde_json::from_str(&dependencies).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    10,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
            tags: serde_json::from_str(&tags).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    11,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
            created_at: DateTime::parse_from_rfc3339(&created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        12,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
            scheduled_at: scheduled_at
                .as_ref()
                .map(|s| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)))
                .transpose()
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        13,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
            started_at: started_at
                .as_ref()
                .map(|s| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)))
                .transpose()
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        14,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
            completed_at: completed_at
                .as_ref()
                .map(|s| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)))
                .transpose()
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        15,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
            deadline: deadline
                .as_ref()
                .map(|s| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)))
                .transpose()
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        16,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
            attempts: row.get(17)?,
            error: row.get(18)?,
            progress: row.get(19)?,
            worker_id: row.get(20)?,
            next_jobs: serde_json::from_str(&next_jobs).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    21,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
        })
    }
}
