//! Genlock synchronization
//!
//! Provides frame-accurate synchronization using genlock signals
//! for multi-camera and LED wall sync.

use super::{SyncStatus, SyncTimestamp};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Genlock configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenlockConfig {
    /// Target frame rate
    pub frame_rate: f64,
    /// Sync tolerance in microseconds
    pub tolerance_us: u64,
    /// Enable auto-recovery
    pub auto_recovery: bool,
}

impl Default for GenlockConfig {
    fn default() -> Self {
        Self {
            frame_rate: 60.0,
            tolerance_us: 100,
            auto_recovery: true,
        }
    }
}

/// Genlock synchronization
pub struct GenlockSync {
    config: GenlockConfig,
    status: SyncStatus,
    reference_time: Option<Instant>,
    frame_count: u64,
    last_sync: Option<SyncTimestamp>,
}

impl GenlockSync {
    /// Create new genlock sync
    pub fn new(config: GenlockConfig) -> Result<Self> {
        Ok(Self {
            config,
            status: SyncStatus::Unlocked,
            reference_time: None,
            frame_count: 0,
            last_sync: None,
        })
    }

    /// Wait for next frame sync
    pub fn wait_for_frame(&mut self) -> Result<SyncTimestamp> {
        let now = Instant::now();

        // Initialize reference time on first call
        if self.reference_time.is_none() {
            self.reference_time = Some(now);
            self.status = SyncStatus::Locking;
        }

        let reference = self
            .reference_time
            .expect("invariant: reference_time set just above in is_none() branch");
        let frame_duration = Duration::from_secs_f64(1.0 / self.config.frame_rate);

        // Calculate target time for this frame
        let target_time = reference + frame_duration * self.frame_count as u32;

        // Wait if we're early
        if now < target_time {
            let wait_time = target_time.duration_since(now);
            std::thread::sleep(wait_time);
        }

        // Check sync status
        let actual_time = Instant::now();
        let offset = if actual_time >= target_time {
            actual_time.duration_since(target_time)
        } else {
            Duration::ZERO
        };

        if offset.as_micros() as u64 > self.config.tolerance_us {
            self.status = SyncStatus::Locking;
            if self.config.auto_recovery {
                // Reset reference time to recover
                self.reference_time = Some(actual_time);
                self.frame_count = 0;
            }
        } else {
            self.status = SyncStatus::Locked;
        }

        let timestamp = SyncTimestamp::new(
            actual_time.duration_since(reference).as_nanos() as u64,
            self.frame_count,
        );

        self.last_sync = Some(timestamp);
        self.frame_count += 1;

        Ok(timestamp)
    }

    /// Get sync status
    #[must_use]
    pub fn status(&self) -> SyncStatus {
        self.status
    }

    /// Get current frame count
    #[must_use]
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Reset synchronization
    pub fn reset(&mut self) {
        self.reference_time = None;
        self.frame_count = 0;
        self.status = SyncStatus::Unlocked;
        self.last_sync = None;
    }

    /// Get configuration
    #[must_use]
    pub fn config(&self) -> &GenlockConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genlock_creation() {
        let config = GenlockConfig::default();
        let genlock = GenlockSync::new(config);
        assert!(genlock.is_ok());
    }

    #[test]
    fn test_genlock_status() {
        let config = GenlockConfig::default();
        let genlock = GenlockSync::new(config).expect("should succeed in test");
        assert_eq!(genlock.status(), SyncStatus::Unlocked);
    }

    #[test]
    fn test_genlock_reset() {
        let config = GenlockConfig::default();
        let mut genlock = GenlockSync::new(config).expect("should succeed in test");
        genlock.reset();
        assert_eq!(genlock.frame_count(), 0);
        assert_eq!(genlock.status(), SyncStatus::Unlocked);
    }
}
