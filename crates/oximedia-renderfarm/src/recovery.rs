// Copyright 2024 OxiMedia Project
// Licensed under the Apache License, Version 2.0

//! Recovery and checkpointing for long-running renders.

use crate::error::{Error, Result};
use crate::job::JobId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Checkpoint data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Job ID
    pub job_id: JobId,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Completed frames
    pub completed_frames: Vec<u32>,
    /// In-progress frames
    pub in_progress_frames: Vec<u32>,
    /// Failed frames
    pub failed_frames: Vec<u32>,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Recovery manager
pub struct RecoveryManager {
    checkpoints: HashMap<JobId, Vec<Checkpoint>>,
    #[allow(dead_code)]
    checkpoint_interval: u64,
}

impl RecoveryManager {
    /// Create a new recovery manager
    #[must_use]
    pub fn new(checkpoint_interval: u64) -> Self {
        Self {
            checkpoints: HashMap::new(),
            checkpoint_interval,
        }
    }

    /// Create checkpoint
    pub fn create_checkpoint(&mut self, checkpoint: Checkpoint) {
        self.checkpoints
            .entry(checkpoint.job_id)
            .or_default()
            .push(checkpoint);
    }

    /// Get latest checkpoint
    #[must_use]
    pub fn get_latest_checkpoint(&self, job_id: JobId) -> Option<&Checkpoint> {
        self.checkpoints.get(&job_id)?.last()
    }

    /// Recover from checkpoint
    pub fn recover(&self, job_id: JobId) -> Result<Checkpoint> {
        self.get_latest_checkpoint(job_id)
            .cloned()
            .ok_or_else(|| Error::Checkpoint(format!("No checkpoint found for job {job_id}")))
    }

    /// List checkpoints for job
    #[must_use]
    pub fn list_checkpoints(&self, job_id: JobId) -> Vec<&Checkpoint> {
        self.checkpoints
            .get(&job_id)
            .map_or_else(Vec::new, |cps| cps.iter().collect())
    }
}

impl Default for RecoveryManager {
    fn default() -> Self {
        Self::new(300) // 5 minutes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_manager_creation() {
        let manager = RecoveryManager::new(300);
        assert_eq!(manager.checkpoint_interval, 300);
    }

    #[test]
    fn test_create_checkpoint() {
        let mut manager = RecoveryManager::new(300);
        let job_id = JobId::new();

        let checkpoint = Checkpoint {
            job_id,
            timestamp: Utc::now(),
            completed_frames: vec![1, 2, 3],
            in_progress_frames: vec![4],
            failed_frames: vec![],
            metadata: HashMap::new(),
        };

        manager.create_checkpoint(checkpoint);
        assert!(manager.get_latest_checkpoint(job_id).is_some());
    }

    #[test]
    fn test_recover_from_checkpoint() -> Result<()> {
        let mut manager = RecoveryManager::new(300);
        let job_id = JobId::new();

        let checkpoint = Checkpoint {
            job_id,
            timestamp: Utc::now(),
            completed_frames: vec![1, 2, 3],
            in_progress_frames: vec![4],
            failed_frames: vec![],
            metadata: HashMap::new(),
        };

        manager.create_checkpoint(checkpoint);

        let recovered = manager.recover(job_id)?;
        assert_eq!(recovered.completed_frames.len(), 3);

        Ok(())
    }
}
