#![allow(dead_code)]
//! I/O pipeline abstraction for chaining staged data processing operations.

/// Represents a processing stage within an I/O pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IoStage {
    /// Read raw bytes from a source.
    Read,
    /// Decompress the byte stream.
    Decompress,
    /// Validate integrity (e.g. checksum verification).
    Validate,
    /// Decrypt encrypted data.
    Decrypt,
    /// Buffer data for downstream consumers.
    Buffer,
    /// Write processed bytes to a sink.
    Write,
    /// A custom-named stage.
    Custom(String),
}

impl IoStage {
    /// Return a human-readable name for this stage.
    #[must_use]
    pub fn stage_name(&self) -> &str {
        match self {
            IoStage::Read => "read",
            IoStage::Decompress => "decompress",
            IoStage::Validate => "validate",
            IoStage::Decrypt => "decrypt",
            IoStage::Buffer => "buffer",
            IoStage::Write => "write",
            IoStage::Custom(name) => name.as_str(),
        }
    }
}

/// The result of executing an I/O pipeline.
#[derive(Debug, Clone)]
pub struct IoResult {
    /// Number of bytes processed.
    pub bytes_processed: u64,
    /// Total elapsed time in milliseconds.
    pub elapsed_ms: u64,
    /// Stages that were executed, in order.
    pub stages_executed: Vec<String>,
    /// Whether the pipeline completed without errors.
    pub success: bool,
}

impl IoResult {
    /// Calculate throughput in megabytes per second.
    ///
    /// Returns `0.0` if elapsed time is zero.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn throughput_mbps(&self) -> f64 {
        if self.elapsed_ms == 0 {
            return 0.0;
        }
        let bytes_f = self.bytes_processed as f64;
        let secs = self.elapsed_ms as f64 / 1000.0;
        (bytes_f / (1024.0 * 1024.0)) / secs
    }
}

/// A sequential pipeline of I/O stages that processes a data buffer.
#[derive(Debug, Default)]
pub struct IoPipeline {
    stages: Vec<IoStage>,
}

impl IoPipeline {
    /// Create a new, empty pipeline.
    #[must_use]
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    /// Append a stage to the end of the pipeline.
    pub fn add_stage(&mut self, stage: IoStage) -> &mut Self {
        self.stages.push(stage);
        self
    }

    /// Return the number of stages in this pipeline.
    #[must_use]
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Execute the pipeline against the provided data buffer.
    ///
    /// Each stage is applied in order, transforming `data` in place.
    /// The returned [`IoResult`] records which stages ran and aggregate stats.
    ///
    /// `elapsed_ms` is provided externally (e.g. measured by the caller) so that
    /// this pure-logic method remains testable without real I/O.
    pub fn execute(&self, data: &mut Vec<u8>, elapsed_ms: u64) -> IoResult {
        let original_len = data.len() as u64;
        let mut stages_executed = Vec::with_capacity(self.stages.len());

        for stage in &self.stages {
            // Simulate each stage with a trivial no-op transformation so that
            // the pipeline logic is exercised without real I/O.
            match stage {
                IoStage::Buffer => {
                    // Buffering: reserve extra capacity but keep content intact.
                    data.reserve(64);
                }
                IoStage::Validate
                | IoStage::Read
                | IoStage::Decompress
                | IoStage::Decrypt
                | IoStage::Write
                | IoStage::Custom(_) => {
                    // All other stages are recorded as executed but perform no transformation.
                }
            }
            stages_executed.push(stage.stage_name().to_string());
        }

        IoResult {
            bytes_processed: original_len,
            elapsed_ms,
            stages_executed,
            success: true,
        }
    }

    /// Return the list of stage names in this pipeline.
    #[must_use]
    pub fn stage_names(&self) -> Vec<&str> {
        self.stages.iter().map(IoStage::stage_name).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_name_read() {
        assert_eq!(IoStage::Read.stage_name(), "read");
    }

    #[test]
    fn test_stage_name_decompress() {
        assert_eq!(IoStage::Decompress.stage_name(), "decompress");
    }

    #[test]
    fn test_stage_name_validate() {
        assert_eq!(IoStage::Validate.stage_name(), "validate");
    }

    #[test]
    fn test_stage_name_decrypt() {
        assert_eq!(IoStage::Decrypt.stage_name(), "decrypt");
    }

    #[test]
    fn test_stage_name_buffer() {
        assert_eq!(IoStage::Buffer.stage_name(), "buffer");
    }

    #[test]
    fn test_stage_name_write() {
        assert_eq!(IoStage::Write.stage_name(), "write");
    }

    #[test]
    fn test_stage_name_custom() {
        let s = IoStage::Custom("my_stage".to_string());
        assert_eq!(s.stage_name(), "my_stage");
    }

    #[test]
    fn test_empty_pipeline() {
        let p = IoPipeline::new();
        assert_eq!(p.stage_count(), 0);
        assert!(p.stage_names().is_empty());
    }

    #[test]
    fn test_add_stages() {
        let mut p = IoPipeline::new();
        p.add_stage(IoStage::Read).add_stage(IoStage::Decompress);
        assert_eq!(p.stage_count(), 2);
        assert_eq!(p.stage_names(), vec!["read", "decompress"]);
    }

    #[test]
    fn test_execute_records_stages() {
        let mut p = IoPipeline::new();
        p.add_stage(IoStage::Read)
            .add_stage(IoStage::Validate)
            .add_stage(IoStage::Write);
        let mut data = vec![1u8, 2, 3, 4];
        let result = p.execute(&mut data, 100);
        assert!(result.success);
        assert_eq!(result.stages_executed, vec!["read", "validate", "write"]);
        assert_eq!(result.bytes_processed, 4);
        assert_eq!(result.elapsed_ms, 100);
    }

    #[test]
    fn test_throughput_mbps_zero_elapsed() {
        let r = IoResult {
            bytes_processed: 1024 * 1024,
            elapsed_ms: 0,
            stages_executed: vec![],
            success: true,
        };
        assert_eq!(r.throughput_mbps(), 0.0);
    }

    #[test]
    fn test_throughput_mbps_one_second() {
        let r = IoResult {
            bytes_processed: 1024 * 1024,
            elapsed_ms: 1000,
            stages_executed: vec![],
            success: true,
        };
        // 1 MiB in 1 second = 1 MiB/s
        let mbps = r.throughput_mbps();
        assert!((mbps - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_throughput_mbps_two_mib_half_second() {
        let r = IoResult {
            bytes_processed: 2 * 1024 * 1024,
            elapsed_ms: 500,
            stages_executed: vec![],
            success: true,
        };
        let mbps = r.throughput_mbps();
        assert!((mbps - 4.0).abs() < 1e-9);
    }

    #[test]
    fn test_execute_buffer_stage() {
        let mut p = IoPipeline::new();
        p.add_stage(IoStage::Buffer);
        let mut data = vec![0u8; 10];
        let result = p.execute(&mut data, 50);
        assert!(result.success);
        assert_eq!(result.bytes_processed, 10);
    }

    #[test]
    fn test_execute_custom_stage() {
        let mut p = IoPipeline::new();
        p.add_stage(IoStage::Custom("transcode".to_string()));
        let mut data = vec![9u8; 5];
        let result = p.execute(&mut data, 200);
        assert_eq!(result.stages_executed, vec!["transcode"]);
    }
}
