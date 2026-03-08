//! Color space conversion operations (RGB ↔ YUV)

use crate::{
    shader::{BindGroupLayoutBuilder, ShaderCompiler, ShaderSource},
    GpuDevice, Result,
};
use bytemuck::{Pod, Zeroable};
use once_cell::sync::OnceCell;
use wgpu::{BindGroup, BindGroupLayout, ComputePipeline};

use super::utils;

/// Color space standards
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
    /// BT.601 (SD video)
    BT601,
    /// BT.709 (HD video)
    BT709,
    /// BT.2020 (UHD video)
    BT2020,
}

impl ColorSpace {
    fn to_format_id(self) -> u32 {
        match self {
            Self::BT601 => 0,
            Self::BT709 => 1,
            Self::BT2020 => 2,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ConversionParams {
    width: u32,
    height: u32,
    stride: u32,
    format: u32,
}

/// Color space conversion operations
pub struct ColorSpaceConversion;

impl ColorSpaceConversion {
    /// Convert RGB to YUV
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device
    /// * `input` - Input RGB buffer (packed RGBA format)
    /// * `output` - Output YUV buffer (packed YUVA format)
    /// * `width` - Image width
    /// * `height` - Image height
    /// * `color_space` - Color space standard (BT.601, BT.709, BT.2020)
    ///
    /// # Errors
    ///
    /// Returns an error if buffer sizes are invalid or if the GPU operation fails.
    #[allow(clippy::too_many_arguments)]
    pub fn rgb_to_yuv(
        device: &GpuDevice,
        input: &[u8],
        output: &mut [u8],
        width: u32,
        height: u32,
        color_space: ColorSpace,
    ) -> Result<()> {
        utils::validate_dimensions(width, height)?;
        utils::validate_buffer_size(input, width, height, 4)?;
        utils::validate_buffer_size(output, width, height, 4)?;

        let pipeline = Self::get_rgb_to_yuv_pipeline(device)?;
        let layout = Self::get_bind_group_layout(device)?;

        Self::execute_conversion(
            device,
            pipeline,
            layout,
            input,
            output,
            width,
            height,
            color_space,
        )
    }

    /// Convert YUV to RGB
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device
    /// * `input` - Input YUV buffer (packed YUVA format)
    /// * `output` - Output RGB buffer (packed RGBA format)
    /// * `width` - Image width
    /// * `height` - Image height
    /// * `color_space` - Color space standard (BT.601, BT.709, BT.2020)
    ///
    /// # Errors
    ///
    /// Returns an error if buffer sizes are invalid or if the GPU operation fails.
    #[allow(clippy::too_many_arguments)]
    pub fn yuv_to_rgb(
        device: &GpuDevice,
        input: &[u8],
        output: &mut [u8],
        width: u32,
        height: u32,
        color_space: ColorSpace,
    ) -> Result<()> {
        utils::validate_dimensions(width, height)?;
        utils::validate_buffer_size(input, width, height, 4)?;
        utils::validate_buffer_size(output, width, height, 4)?;

        let pipeline = Self::get_yuv_to_rgb_pipeline(device)?;
        let layout = Self::get_bind_group_layout(device)?;

        Self::execute_conversion(
            device,
            pipeline,
            layout,
            input,
            output,
            width,
            height,
            color_space,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_conversion(
        device: &GpuDevice,
        pipeline: &ComputePipeline,
        layout: &BindGroupLayout,
        input: &[u8],
        output: &mut [u8],
        width: u32,
        height: u32,
        color_space: ColorSpace,
    ) -> Result<()> {
        // Create buffers
        let input_buffer = utils::create_storage_buffer(device, input.len() as u64)?;
        let output_buffer = utils::create_storage_buffer(device, output.len() as u64)?;

        // Upload input data
        device.queue().write_buffer(input_buffer.buffer(), 0, input);

        // Create uniform buffer for parameters
        let params = ConversionParams {
            width,
            height,
            stride: width,
            format: color_space.to_format_id(),
        };
        let params_bytes = bytemuck::bytes_of(&params);
        let params_buffer = utils::create_uniform_buffer(device, params_bytes)?;

        // Create bind group
        let compiler = ShaderCompiler::new(device);
        let bind_group = compiler.create_bind_group(
            "ColorSpace Bind Group",
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
        Self::dispatch_compute(device, pipeline, &bind_group, width, height)?;

        // Read back results
        let readback_buffer = utils::create_readback_buffer(device, output.len() as u64)?;
        let mut encoder = device
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ColorSpace Copy Encoder"),
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
                label: Some("ColorSpace Compute Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("ColorSpace Compute Pass"),
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

            compiler.create_bind_group_layout("ColorSpace Bind Group Layout", &entries)
        }))
    }

    fn get_rgb_to_yuv_pipeline(device: &GpuDevice) -> Result<&'static ComputePipeline> {
        static PIPELINE: OnceCell<ComputePipeline> = OnceCell::new();

        Ok(PIPELINE.get_or_init(|| {
            let compiler = ShaderCompiler::new(device);
            let shader = compiler
                .compile(
                    "ColorSpace Shader",
                    ShaderSource::Embedded(crate::shader::embedded::COLORSPACE_SHADER),
                )
                .expect("Failed to compile colorspace shader");

            let layout =
                Self::get_bind_group_layout(device).expect("Failed to create bind group layout");

            compiler
                .create_pipeline("RGB to YUV Pipeline", &shader, "rgb_to_yuv_main", layout)
                .expect("Failed to create pipeline")
        }))
    }

    fn get_yuv_to_rgb_pipeline(device: &GpuDevice) -> Result<&'static ComputePipeline> {
        static PIPELINE: OnceCell<ComputePipeline> = OnceCell::new();

        Ok(PIPELINE.get_or_init(|| {
            let compiler = ShaderCompiler::new(device);
            let shader = compiler
                .compile(
                    "ColorSpace Shader",
                    ShaderSource::Embedded(crate::shader::embedded::COLORSPACE_SHADER),
                )
                .expect("Failed to compile colorspace shader");

            let layout =
                Self::get_bind_group_layout(device).expect("Failed to create bind group layout");

            compiler
                .create_pipeline("YUV to RGB Pipeline", &shader, "yuv_to_rgb_main", layout)
                .expect("Failed to create pipeline")
        }))
    }
}
