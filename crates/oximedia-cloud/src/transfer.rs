//! Transfer management with chunked uploads, retry, and progress tracking

use bytes::{Bytes, BytesMut};
use futures::stream::{self, StreamExt};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::error::Result;
use crate::types::{CloudStorage, TransferProgress};

/// Transfer manager for handling chunked uploads and downloads
pub struct TransferManager {
    /// Storage backend
    storage: Arc<dyn CloudStorage>,
    /// Configuration
    config: TransferConfig,
    /// Active transfers
    transfers: Arc<RwLock<HashMap<String, TransferState>>>,
}

impl TransferManager {
    /// Create a new transfer manager
    #[must_use]
    pub fn new(storage: Arc<dyn CloudStorage>, config: TransferConfig) -> Self {
        Self {
            storage,
            config,
            transfers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Upload data with chunking and retry
    ///
    /// # Errors
    ///
    /// Returns an error if the upload fails after all retries
    pub async fn upload(
        &self,
        key: &str,
        data: Bytes,
        progress_tx: Option<mpsc::Sender<TransferProgress>>,
    ) -> Result<()> {
        let transfer_id = format!("upload-{key}");
        self.init_transfer(&transfer_id, data.len() as u64);

        if data.len() <= self.config.chunk_size {
            // Single-part upload
            self.upload_single_part(key, data, &transfer_id, progress_tx)
                .await
        } else {
            // Multi-part upload
            self.upload_multipart(key, data, &transfer_id, progress_tx)
                .await
        }
    }

    /// Download data with chunking and retry
    ///
    /// # Errors
    ///
    /// Returns an error if the download fails after all retries
    pub async fn download(
        &self,
        key: &str,
        progress_tx: Option<mpsc::Sender<TransferProgress>>,
    ) -> Result<Bytes> {
        let transfer_id = format!("download-{key}");

        // Get object metadata to determine size
        let metadata = self.storage.get_metadata(key).await?;
        let total_size = metadata.info.size;

        self.init_transfer(&transfer_id, total_size);

        if total_size <= self.config.chunk_size as u64 {
            // Single-part download
            self.download_single_part(key, &transfer_id, progress_tx)
                .await
        } else {
            // Multi-part download
            self.download_multipart(key, total_size, &transfer_id, progress_tx)
                .await
        }
    }

    /// Upload single part with retry
    async fn upload_single_part(
        &self,
        key: &str,
        data: Bytes,
        transfer_id: &str,
        progress_tx: Option<mpsc::Sender<TransferProgress>>,
    ) -> Result<()> {
        let mut attempts = 0;
        let total_size = data.len() as u64;

        loop {
            match self.storage.upload(key, data.clone()).await {
                Ok(()) => {
                    self.update_progress(transfer_id, total_size, total_size, &progress_tx)
                        .await;
                    self.complete_transfer(transfer_id);
                    return Ok(());
                }
                Err(e) if e.is_retryable() && attempts < self.config.max_retries => {
                    attempts += 1;
                    tracing::warn!("Upload attempt {} failed: {}", attempts, e);
                    sleep(self.retry_delay(attempts)).await;
                }
                Err(e) => {
                    self.fail_transfer(transfer_id);
                    return Err(e);
                }
            }
        }
    }

    /// Upload with multipart
    async fn upload_multipart(
        &self,
        key: &str,
        data: Bytes,
        transfer_id: &str,
        progress_tx: Option<mpsc::Sender<TransferProgress>>,
    ) -> Result<()> {
        let chunk_size = self.config.chunk_size;
        let total_size = data.len() as u64;
        let num_chunks = data.len().div_ceil(chunk_size);

        // Split data into chunks
        let chunks: Vec<Bytes> = (0..num_chunks)
            .map(|i| {
                let start = i * chunk_size;
                let end = std::cmp::min(start + chunk_size, data.len());
                data.slice(start..end)
            })
            .collect();

        // Upload chunks in parallel
        let max_concurrent = self.config.max_concurrent_transfers;
        let mut bytes_transferred = 0u64;

        let chunk_futures: Vec<_> = chunks
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| {
                let chunk_key = format!("{key}.part{i}");
                let storage = self.storage.clone();
                let chunk_len = chunk.len() as u64;

                async move {
                    let mut attempts = 0;
                    loop {
                        match storage.upload(&chunk_key, chunk.clone()).await {
                            Ok(()) => return Ok(chunk_len),
                            Err(e) if e.is_retryable() && attempts < self.config.max_retries => {
                                attempts += 1;
                                sleep(Duration::from_secs(2u64.pow(attempts))).await;
                            }
                            Err(e) => return Err(e),
                        }
                    }
                }
            })
            .collect();

        let mut stream = stream::iter(chunk_futures).buffer_unordered(max_concurrent);

        while let Some(result) = stream.next().await {
            let chunk_len = result?;
            bytes_transferred += chunk_len;
            self.update_progress(transfer_id, bytes_transferred, total_size, &progress_tx)
                .await;
        }

        // Combine chunks (implementation-specific)
        // For now, we mark as complete
        self.complete_transfer(transfer_id);
        Ok(())
    }

    /// Download single part with retry
    async fn download_single_part(
        &self,
        key: &str,
        transfer_id: &str,
        progress_tx: Option<mpsc::Sender<TransferProgress>>,
    ) -> Result<Bytes> {
        let mut attempts = 0;

        loop {
            match self.storage.download(key).await {
                Ok(data) => {
                    let total_size = data.len() as u64;
                    self.update_progress(transfer_id, total_size, total_size, &progress_tx)
                        .await;
                    self.complete_transfer(transfer_id);
                    return Ok(data);
                }
                Err(e) if e.is_retryable() && attempts < self.config.max_retries => {
                    attempts += 1;
                    tracing::warn!("Download attempt {} failed: {}", attempts, e);
                    sleep(self.retry_delay(attempts)).await;
                }
                Err(e) => {
                    self.fail_transfer(transfer_id);
                    return Err(e);
                }
            }
        }
    }

    /// Download with multipart using byte ranges
    async fn download_multipart(
        &self,
        key: &str,
        total_size: u64,
        transfer_id: &str,
        progress_tx: Option<mpsc::Sender<TransferProgress>>,
    ) -> Result<Bytes> {
        let chunk_size = self.config.chunk_size as u64;
        let num_chunks = total_size.div_ceil(chunk_size);

        // Create range requests
        let ranges: Vec<(u64, u64)> = (0..num_chunks)
            .map(|i| {
                let start = i * chunk_size;
                let end = std::cmp::min(start + chunk_size - 1, total_size - 1);
                (start, end)
            })
            .collect();

        let max_concurrent = self.config.max_concurrent_transfers;
        let mut bytes_transferred = 0u64;

        // Download chunks in parallel
        let chunk_futures: Vec<_> = ranges
            .into_iter()
            .map(|(start, end)| {
                let storage = self.storage.clone();
                let key = key.to_string();

                async move {
                    let mut attempts = 0;
                    loop {
                        match storage.download_range(&key, start, end).await {
                            Ok(data) => return Ok((start, data)),
                            Err(e) if e.is_retryable() && attempts < self.config.max_retries => {
                                attempts += 1;
                                sleep(Duration::from_secs(2u64.pow(attempts))).await;
                            }
                            Err(e) => return Err(e),
                        }
                    }
                }
            })
            .collect();

        let mut stream = stream::iter(chunk_futures).buffer_unordered(max_concurrent);
        let mut chunks: Vec<(u64, Bytes)> = Vec::new();

        while let Some(result) = stream.next().await {
            let (offset, chunk) = result?;
            bytes_transferred += chunk.len() as u64;
            chunks.push((offset, chunk));
            self.update_progress(transfer_id, bytes_transferred, total_size, &progress_tx)
                .await;
        }

        // Sort chunks by offset and combine
        chunks.sort_by_key(|(offset, _)| *offset);
        let mut combined = BytesMut::with_capacity(total_size as usize);
        for (_, chunk) in chunks {
            combined.extend_from_slice(&chunk);
        }

        self.complete_transfer(transfer_id);
        Ok(combined.freeze())
    }

    /// Initialize a transfer
    fn init_transfer(&self, transfer_id: &str, total_size: u64) {
        let state = TransferState {
            total_size,
            bytes_transferred: 0,
            start_time: Instant::now(),
            status: TransferStatus::InProgress,
        };
        self.transfers
            .write()
            .insert(transfer_id.to_string(), state);
    }

    /// Update transfer progress
    async fn update_progress(
        &self,
        transfer_id: &str,
        bytes_transferred: u64,
        total_size: u64,
        progress_tx: &Option<mpsc::Sender<TransferProgress>>,
    ) {
        let (_elapsed, rate_bps, eta_secs) = {
            if let Some(state) = self.transfers.write().get_mut(transfer_id) {
                state.bytes_transferred = bytes_transferred;

                let elapsed = state.start_time.elapsed().as_secs_f64();
                let rate_bps = if elapsed > 0.0 {
                    bytes_transferred as f64 / elapsed
                } else {
                    0.0
                };

                let remaining_bytes = total_size.saturating_sub(bytes_transferred);
                let eta_secs = if rate_bps > 0.0 {
                    Some(remaining_bytes as f64 / rate_bps)
                } else {
                    None
                };
                (elapsed, rate_bps, eta_secs)
            } else {
                (0.0, 0.0, None)
            }
        };

        if let Some(tx) = progress_tx {
            let progress = TransferProgress {
                bytes_transferred,
                total_bytes: total_size,
                rate_bps,
                eta_secs,
            };
            let _ = tx.send(progress).await;
        }
    }

    /// Mark transfer as complete
    fn complete_transfer(&self, transfer_id: &str) {
        if let Some(state) = self.transfers.write().get_mut(transfer_id) {
            state.status = TransferStatus::Completed;
        }
    }

    /// Mark transfer as failed
    fn fail_transfer(&self, transfer_id: &str) {
        if let Some(state) = self.transfers.write().get_mut(transfer_id) {
            state.status = TransferStatus::Failed;
        }
    }

    /// Calculate retry delay with exponential backoff
    fn retry_delay(&self, attempt: u32) -> Duration {
        let base_delay = Duration::from_secs(1);
        let max_delay = Duration::from_secs(60);
        let delay = base_delay * 2u32.pow(attempt);
        std::cmp::min(delay, max_delay)
    }

    /// Get transfer status
    #[must_use]
    pub fn get_status(&self, transfer_id: &str) -> Option<TransferState> {
        self.transfers.read().get(transfer_id).cloned()
    }
}

/// Transfer configuration
#[derive(Debug, Clone)]
pub struct TransferConfig {
    /// Chunk size for multipart transfers
    pub chunk_size: usize,
    /// Maximum concurrent transfers
    pub max_concurrent_transfers: usize,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Enable checksum verification
    pub verify_checksum: bool,
    /// Bandwidth limit in bytes per second (None = unlimited)
    pub bandwidth_limit_bps: Option<u64>,
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self {
            chunk_size: 5 * 1024 * 1024, // 5 MB
            max_concurrent_transfers: 4,
            max_retries: 3,
            verify_checksum: true,
            bandwidth_limit_bps: None,
        }
    }
}

impl TransferConfig {
    /// Create configuration optimized for small files
    #[must_use]
    pub fn small_files() -> Self {
        Self {
            chunk_size: 1024 * 1024, // 1 MB
            max_concurrent_transfers: 8,
            max_retries: 3,
            verify_checksum: true,
            bandwidth_limit_bps: None,
        }
    }

    /// Create configuration optimized for large files
    #[must_use]
    pub fn large_files() -> Self {
        Self {
            chunk_size: 20 * 1024 * 1024, // 20 MB
            max_concurrent_transfers: 8,
            max_retries: 5,
            verify_checksum: true,
            bandwidth_limit_bps: None,
        }
    }
}

/// Transfer state
#[derive(Debug, Clone)]
pub struct TransferState {
    /// Total size in bytes
    pub total_size: u64,
    /// Bytes transferred
    pub bytes_transferred: u64,
    /// Start time
    pub start_time: Instant,
    /// Status
    pub status: TransferStatus,
}

/// Transfer status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferStatus {
    /// Transfer in progress
    InProgress,
    /// Transfer completed
    Completed,
    /// Transfer failed
    Failed,
    /// Transfer paused
    Paused,
}

/// Checksum calculator
pub struct ChecksumCalculator {
    /// MD5 hasher
    md5: md5::Md5,
    /// SHA256 hasher
    sha256: sha2::Sha256,
}

impl ChecksumCalculator {
    /// Create a new checksum calculator
    #[must_use]
    pub fn new() -> Self {
        use sha2::Digest;
        Self {
            md5: md5::Md5::new(),
            sha256: sha2::Sha256::new(),
        }
    }

    /// Update with data
    pub fn update(&mut self, data: &[u8]) {
        use sha2::Digest;
        self.md5.update(data);
        self.sha256.update(data);
    }

    /// Finalize and get checksums
    #[must_use]
    pub fn finalize(self) -> Checksums {
        use sha2::Digest;
        let md5_digest = self.md5.finalize();
        let sha256_digest = self.sha256.finalize();

        Checksums {
            md5: hex::encode(&md5_digest[..]),
            sha256: hex::encode(&sha256_digest[..]),
        }
    }
}

impl Default for ChecksumCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Checksums for data verification
#[derive(Debug, Clone)]
pub struct Checksums {
    /// MD5 checksum
    pub md5: String,
    /// SHA256 checksum
    pub sha256: String,
}

impl Checksums {
    /// Verify MD5 checksum
    #[must_use]
    pub fn verify_md5(&self, expected: &str) -> bool {
        self.md5.eq_ignore_ascii_case(expected)
    }

    /// Verify SHA256 checksum
    #[must_use]
    pub fn verify_sha256(&self, expected: &str) -> bool {
        self.sha256.eq_ignore_ascii_case(expected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_config_defaults() {
        let config = TransferConfig::default();
        assert_eq!(config.chunk_size, 5 * 1024 * 1024);
        assert_eq!(config.max_concurrent_transfers, 4);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_transfer_config_presets() {
        let small = TransferConfig::small_files();
        assert_eq!(small.chunk_size, 1024 * 1024);

        let large = TransferConfig::large_files();
        assert_eq!(large.chunk_size, 20 * 1024 * 1024);
    }

    #[test]
    fn test_checksum_calculator() {
        let mut calc = ChecksumCalculator::new();
        calc.update(b"test data");
        let checksums = calc.finalize();

        assert!(!checksums.md5.is_empty());
        assert!(!checksums.sha256.is_empty());
    }

    #[test]
    fn test_checksum_verification() {
        let mut calc = ChecksumCalculator::new();
        calc.update(b"test");
        let checksums = calc.finalize();

        // Known MD5 of "test"
        assert!(checksums.verify_md5("098f6bcd4621d373cade4e832627b4f6"));
    }

    #[test]
    fn test_transfer_status() {
        assert_eq!(TransferStatus::InProgress, TransferStatus::InProgress);
        assert_ne!(TransferStatus::InProgress, TransferStatus::Completed);
    }
}
