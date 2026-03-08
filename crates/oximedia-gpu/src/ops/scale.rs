//! Image scaling operations with various interpolation methods

use crate::{
    shader::{BindGroupLayoutBuilder, ShaderCompiler, ShaderSource},
    GpuDevice, Result,
};
use bytemuck::{Pod, Zeroable};
use once_cell::sync::OnceCell;
use wgpu::{BindGroup, BindGroupLayout, ComputePipeline};

use super::utils;

/// Scale filter type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleFilter {
    /// Nearest neighbor (fastest, lowest quality)
    Nearest,
    /// Bilinear interpolation (balanced)
    Bilinear,
    /// Bicubic interpolation (highest quality)
    Bicubic,
    /// Area averaging for downscaling
    Area,
}

impl ScaleFilter {
    fn to_filter_id(self) -> u32 {
        match self {
            Self::Nearest => 0,
            Self::Bilinear => 1,
            Self::Bicubic => 2,
            Self::Area => 3,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ScaleParams {
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
    src_stride: u32,
    dst_stride: u32,
    filter_type: u32,
    padding: u32,
}

/// Image scaling operations
pub struct ScaleOperation;

impl ScaleOperation {
    /// Scale an image
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device
    /// * `input` - Input image buffer (packed RGBA format)
    /// * `src_width` - Source image width
    /// * `src_height` - Source image height
    /// * `output` - Output image buffer (packed RGBA format)
    /// * `dst_width` - Destination image width
    /// * `dst_height` - Destination image height
    /// * `filter` - Scaling filter type
    ///
    /// # Errors
    ///
    /// Returns an error if buffer sizes are invalid or if the GPU operation fails.
    #[allow(clippy::too_many_arguments)]
    pub fn scale(
        device: &GpuDevice,
        input: &[u8],
        src_width: u32,
        src_height: u32,
        output: &mut [u8],
        dst_width: u32,
        dst_height: u32,
        filter: ScaleFilter,
    ) -> Result<()> {
        utils::validate_dimensions(src_width, src_height)?;
        utils::validate_dimensions(dst_width, dst_height)?;
        utils::validate_buffer_size(input, src_width, src_height, 4)?;
        utils::validate_buffer_size(output, dst_width, dst_height, 4)?;

        let pipeline = if filter == ScaleFilter::Area {
            Self::get_downscale_pipeline(device)?
        } else {
            Self::get_scale_pipeline(device)?
        };

        let layout = Self::get_bind_group_layout(device)?;

        Self::execute_scale(
            device, pipeline, layout, input, src_width, src_height, output, dst_width, dst_height,
            filter,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_scale(
        device: &GpuDevice,
        pipeline: &ComputePipeline,
        layout: &BindGroupLayout,
        input: &[u8],
        src_width: u32,
        src_height: u32,
        output: &mut [u8],
        dst_width: u32,
        dst_height: u32,
        filter: ScaleFilter,
    ) -> Result<()> {
        // Create buffers
        let input_buffer = utils::create_storage_buffer(device, input.len() as u64)?;
        let output_buffer = utils::create_storage_buffer(device, output.len() as u64)?;

        // Upload input data
        device.queue().write_buffer(input_buffer.buffer(), 0, input);

        // Create uniform buffer for parameters
        let params = ScaleParams {
            src_width,
            src_height,
            dst_width,
            dst_height,
            src_stride: src_width,
            dst_stride: dst_width,
            filter_type: filter.to_filter_id(),
            padding: 0,
        };
        let params_bytes = bytemuck::bytes_of(&params);
        let params_buffer = utils::create_uniform_buffer(device, params_bytes)?;

        // Create bind group
        let compiler = ShaderCompiler::new(device);
        let bind_group = compiler.create_bind_group(
            "Scale Bind Group",
            layout,
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.buffer().as_entire_binding(),
                },
            ],
        );

        // Execute compute pass
        Self::dispatch_compute(device, pipeline, &bind_group, dst_width, dst_height)?;

        // Read back results
        let readback_buffer = utils::create_readback_buffer(device, output.len() as u64)?;
        let mut encoder = device
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Scale Copy Encoder"),
            });

        output_buffer.copy_to(&mut encoder, &readback_buffer, 0, 0, output.len() as u64)?;

        device.queue().submit(Some(encoder.finish()));
        device.wait();

        let result = readback_buffer.read(device, 0, output.len() as u64)?;
        output.copy_from_slice(&result);

        Ok(())
    }

    fn dispatch_compute(
        device: &GpuDevice,
        pipeline: &ComputePipeline,
        bind_group: &BindGroup,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let mut encoder = device
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Scale Compute Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Scale Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(pipeline);
            compute_pass.set_bind_group(0, bind_group, &[]);

            let (dispatch_x, dispatch_y) = utils::calculate_dispatch_size(width, height, (16, 16));
            compute_pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
        }

        device.queue().submit(Some(encoder.finish()));
        Ok(())
    }

    fn get_bind_group_layout(device: &GpuDevice) -> Result<&'static BindGroupLayout> {
        static LAYOUT: OnceCell<BindGroupLayout> = OnceCell::new();

        Ok(LAYOUT.get_or_init(|| {
            let compiler = ShaderCompiler::new(device);
            let entries = BindGroupLayoutBuilder::new()
                .add_storage_buffer_read_only(0) // input
                .add_storage_buffer(1) // output
                .add_uniform_buffer(2) // params
                .build();

            compiler.create_bind_group_layout("Scale Bind Group Layout", &entries)
        }))
    }

    fn get_scale_pipeline(device: &GpuDevice) -> Result<&'static ComputePipeline> {
        static PIPELINE: OnceCell<ComputePipeline> = OnceCell::new();

        Ok(PIPELINE.get_or_init(|| {
            let compiler = ShaderCompiler::new(device);
            let shader = compiler
                .compile(
                    "Scale Shader",
                    ShaderSource::Embedded(crate::shader::embedded::SCALE_SHADER),
                )
                .expect("Failed to compile scale shader");

            let layout =
                Self::get_bind_group_layout(device).expect("Failed to create bind group layout");

            compiler
                .create_pipeline("Scale Pipeline", &shader, "scale_main", layout)
                .expect("Failed to create pipeline")
        }))
    }

    fn get_downscale_pipeline(device: &GpuDevice) -> Result<&'static ComputePipeline> {
        static PIPELINE: OnceCell<ComputePipeline> = OnceCell::new();

        Ok(PIPELINE.get_or_init(|| {
            let compiler = ShaderCompiler::new(device);
            let shader = compiler
                .compile(
                    "Scale Shader",
                    ShaderSource::Embedded(crate::shader::embedded::SCALE_SHADER),
                )
                .expect("Failed to compile scale shader");

            let layout =
                Self::get_bind_group_layout(device).expect("Failed to create bind group layout");

            compiler
                .create_pipeline("Downscale Pipeline", &shader, "downscale_area", layout)
                .expect("Failed to create pipeline")
        }))
    }
}
