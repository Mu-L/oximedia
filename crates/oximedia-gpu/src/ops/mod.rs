//! GPU compute operations

pub mod colorspace;
pub mod filter;
pub mod scale;
pub mod tonemap;
pub mod transform;

pub use colorspace::{ColorSpace, ColorSpaceConversion};
pub use filter::FilterOperation;
pub use scale::{ScaleFilter, ScaleOperation};
pub use tonemap::{
    aces_tonemap, apply_gamma, apply_tonemap_frame, hable_tonemap, reinhard_tonemap,
    TonemapAlgorithm, TonemapParams,
};
pub use transform::TransformOperation;

/// Common utilities for GPU operations
pub(crate) mod utils {
    use crate::buffer::BufferType;
    use crate::{GpuBuffer, GpuDevice, GpuError, Result};

    /// Create a staging buffer and upload data
    #[allow(dead_code)]
    pub fn create_staging_buffer(device: &GpuDevice, data: &[u8]) -> Result<GpuBuffer> {
        GpuBuffer::with_data(device, data, BufferType::Staging)
    }

    /// Create a storage buffer
    pub fn create_storage_buffer(device: &GpuDevice, size: u64) -> Result<GpuBuffer> {
        GpuBuffer::new(device, size, BufferType::Storage)
    }

    /// Create a uniform buffer with data
    pub fn create_uniform_buffer(device: &GpuDevice, data: &[u8]) -> Result<GpuBuffer> {
        GpuBuffer::with_data(device, data, BufferType::Uniform)
    }

    /// Create a read-back buffer
    pub fn create_readback_buffer(device: &GpuDevice, size: u64) -> Result<GpuBuffer> {
        GpuBuffer::new(device, size, BufferType::ReadBack)
    }

    /// Calculate workgroup dispatch dimensions
    pub fn calculate_dispatch_size(
        width: u32,
        height: u32,
        workgroup_size: (u32, u32),
    ) -> (u32, u32) {
        let x = width.div_ceil(workgroup_size.0);
        let y = height.div_ceil(workgroup_size.1);
        (x, y)
    }

    /// Validate image dimensions
    pub fn validate_dimensions(width: u32, height: u32) -> Result<()> {
        if width == 0 || height == 0 {
            return Err(GpuError::InvalidDimensions { width, height });
        }
        if width > 16384 || height > 16384 {
            return Err(GpuError::InvalidDimensions { width, height });
        }
        Ok(())
    }

    /// Validate buffer size for image data
    pub fn validate_buffer_size(
        buffer: &[u8],
        width: u32,
        height: u32,
        channels: u32,
    ) -> Result<()> {
        let expected = (width * height * channels) as usize;
        let actual = buffer.len();
        if actual < expected {
            return Err(GpuError::InvalidBufferSize { expected, actual });
        }
        Ok(())
    }
}
