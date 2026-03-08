//! Convolution filter operations (blur, sharpen, edge detection)

use crate::{
    shader::{BindGroupLayoutBuilder, ShaderCompiler, ShaderSource},
    GpuDevice, GpuError, Result,
};
use bytemuck::{Pod, Zeroable};
use once_cell::sync::OnceCell;
use wgpu::{BindGroup, BindGroupLayout, ComputePipeline};

use super::utils;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct FilterParams {
    width: u32,
    height: u32,
    stride: u32,
    kernel_size: u32,
    normalize: u32,
    filter_type: u32,
    padding: u32,
    sigma: f32,
}

/// Convolution filter operations
pub struct FilterOperation;

impl FilterOperation {
    /// Apply Gaussian blur
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device
    /// * `input` - Input image buffer (packed RGBA format)
    /// * `output` - Output image buffer (packed RGBA format)
    /// * `width` - Image width
    /// * `height` - Image height
    /// * `sigma` - Blur radius (standard deviation)
    ///
    /// # Errors
    ///
    /// Returns an error if buffer sizes are invalid or if the GPU operation fails.
    #[allow(clippy::too_many_arguments)]
    pub fn gaussian_blur(
        device: &GpuDevice,
        input: &[u8],
        output: &mut [u8],
        width: u32,
        height: u32,
        sigma: f32,
    ) -> Result<()> {
        utils::validate_dimensions(width, height)?;
        utils::validate_buffer_size(input, width, height, 4)?;
        utils::validate_buffer_size(output, width, height, 4)?;

        let kernel_size = Self::calculate_kernel_size(sigma);
        let pipeline = Self::get_gaussian_pipeline(device)?;
        let layout = Self::get_bind_group_layout(device)?;

        Self::execute_filter(
            device,
            pipeline,
            layout,
            input,
            output,
            width,
            height,
            kernel_size,
            1, // Gaussian filter type
            sigma,
        )
    }

    /// Apply sharpening filter (unsharp mask)
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device
    /// * `input` - Input image buffer (packed RGBA format)
    /// * `output` - Output image buffer (packed RGBA format)
    /// * `width` - Image width
    /// * `height` - Image height
    /// * `amount` - Sharpening strength
    ///
    /// # Errors
    ///
    /// Returns an error if buffer sizes are invalid or if the GPU operation fails.
    #[allow(clippy::too_many_arguments)]
    pub fn sharpen(
        device: &GpuDevice,
        input: &[u8],
        output: &mut [u8],
        width: u32,
        height: u32,
        amount: f32,
    ) -> Result<()> {
        utils::validate_dimensions(width, height)?;
        utils::validate_buffer_size(input, width, height, 4)?;
        utils::validate_buffer_size(output, width, height, 4)?;

        let pipeline = Self::get_sharpen_pipeline(device)?;
        let layout = Self::get_bind_group_layout(device)?;

        Self::execute_filter(
            device, pipeline, layout, input, output, width, height,
            5, // Kernel size for sharpening
            2, // Sharpen filter type
            amount,
        )
    }

    /// Detect edges using Sobel operator
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device
    /// * `input` - Input image buffer (packed RGBA format)
    /// * `output` - Output image buffer (packed RGBA format)
    /// * `width` - Image width
    /// * `height` - Image height
    ///
    /// # Errors
    ///
    /// Returns an error if buffer sizes are invalid or if the GPU operation fails.
    pub fn edge_detect(
        device: &GpuDevice,
        input: &[u8],
        output: &mut [u8],
        width: u32,
        height: u32,
    ) -> Result<()> {
        utils::validate_dimensions(width, height)?;
        utils::validate_buffer_size(input, width, height, 4)?;
        utils::validate_buffer_size(output, width, height, 4)?;

        let pipeline = Self::get_edge_detect_pipeline(device)?;
        let layout = Self::get_bind_group_layout(device)?;

        Self::execute_filter(
            device, pipeline, layout, input, output, width, height, 3, // 3x3 Sobel kernel
            3, // Edge detect filter type
            0.0,
        )
    }

    /// Apply custom convolution kernel
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device
    /// * `input` - Input image buffer (packed RGBA format)
    /// * `output` - Output image buffer (packed RGBA format)
    /// * `width` - Image width
    /// * `height` - Image height
    /// * `kernel` - Convolution kernel (must be square and odd-sized)
    /// * `normalize` - Whether to normalize the kernel
    ///
    /// # Errors
    ///
    /// Returns an error if buffer sizes are invalid or if the GPU operation fails.
    #[allow(clippy::too_many_arguments)]
    pub fn convolve(
        device: &GpuDevice,
        input: &[u8],
        output: &mut [u8],
        width: u32,
        height: u32,
        kernel: &[f32],
        normalize: bool,
    ) -> Result<()> {
        utils::validate_dimensions(width, height)?;
        utils::validate_buffer_size(input, width, height, 4)?;
        utils::validate_buffer_size(output, width, height, 4)?;

        let kernel_size = (kernel.len() as f32).sqrt() as u32;
        if kernel_size * kernel_size != kernel.len() as u32 {
            return Err(GpuError::Internal("Kernel must be square".to_string()));
        }
        if kernel_size % 2 == 0 {
            return Err(GpuError::Internal("Kernel size must be odd".to_string()));
        }

        let pipeline = Self::get_convolve_pipeline(device)?;
        let layout = Self::get_bind_group_layout_with_kernel(device)?;

        Self::execute_convolve(
            device,
            pipeline,
            layout,
            input,
            output,
            width,
            height,
            kernel,
            kernel_size,
            normalize,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_filter(
        device: &GpuDevice,
        pipeline: &ComputePipeline,
        layout: &BindGroupLayout,
        input: &[u8],
        output: &mut [u8],
        width: u32,
        height: u32,
        kernel_size: u32,
        filter_type: u32,
        sigma: f32,
    ) -> Result<()> {
        // Create buffers
        let input_buffer = utils::create_storage_buffer(device, input.len() as u64)?;
        let output_buffer = utils::create_storage_buffer(device, output.len() as u64)?;

        // Upload input data
        device.queue().write_buffer(input_buffer.buffer(), 0, input);

        // Create uniform buffer for parameters
        let params = FilterParams {
            width,
            height,
            stride: width,
            kernel_size,
            normalize: 1,
            filter_type,
            padding: 0,
            sigma,
        };
        let params_bytes = bytemuck::bytes_of(&params);
        let params_buffer = utils::create_uniform_buffer(device, params_bytes)?;

        // Create bind group
        let compiler = ShaderCompiler::new(device);
        let bind_group = compiler.create_bind_group(
            "Filter Bind Group",
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
                label: Some("Filter Copy Encoder"),
            });

        output_buffer.copy_to(&mut encoder, &readback_buffer, 0, 0, output.len() as u64)?;

        device.queue().submit(Some(encoder.finish()));
        device.wait();

        let result = readback_buffer.read(device, 0, output.len() as u64)?;
        output.copy_from_slice(&result);

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_convolve(
        device: &GpuDevice,
        pipeline: &ComputePipeline,
        layout: &BindGroupLayout,
        input: &[u8],
        output: &mut [u8],
        width: u32,
        height: u32,
        kernel: &[f32],
        kernel_size: u32,
        normalize: bool,
    ) -> Result<()> {
        // Create buffers
        let input_buffer = utils::create_storage_buffer(device, input.len() as u64)?;
        let output_buffer = utils::create_storage_buffer(device, output.len() as u64)?;

        // Upload input data
        device.queue().write_buffer(input_buffer.buffer(), 0, input);

        // Create kernel buffer
        let kernel_bytes = bytemuck::cast_slice(kernel);
        let kernel_buffer = utils::create_storage_buffer(device, kernel_bytes.len() as u64)?;
        device
            .queue()
            .write_buffer(kernel_buffer.buffer(), 0, kernel_bytes);

        // Create uniform buffer for parameters
        let params = FilterParams {
            width,
            height,
            stride: width,
            kernel_size,
            normalize: u32::from(normalize),
            filter_type: 0, // Custom kernel
            padding: 0,
            sigma: 0.0,
        };
        let params_bytes = bytemuck::bytes_of(&params);
        let params_buffer = utils::create_uniform_buffer(device, params_bytes)?;

        // Create bind group
        let compiler = ShaderCompiler::new(device);
        let bind_group = compiler.create_bind_group(
            "Filter Bind Group",
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
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: kernel_buffer.buffer().as_entire_binding(),
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
                label: Some("Filter Copy Encoder"),
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
                label: Some("Filter Compute Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Filter Compute Pass"),
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

    fn calculate_kernel_size(sigma: f32) -> u32 {
        // Use 3-sigma rule: kernel size = 2 * ceil(3 * sigma) + 1
        let radius = (3.0 * sigma).ceil() as u32;
        2 * radius + 1
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

            compiler.create_bind_group_layout("Filter Bind Group Layout", &entries)
        }))
    }

    fn get_bind_group_layout_with_kernel(device: &GpuDevice) -> Result<&'static BindGroupLayout> {
        static LAYOUT: OnceCell<BindGroupLayout> = OnceCell::new();

        Ok(LAYOUT.get_or_init(|| {
            let compiler = ShaderCompiler::new(device);
            let entries = BindGroupLayoutBuilder::new()
                .add_storage_buffer_read_only(0) // input
                .add_storage_buffer(1) // output
                .add_uniform_buffer(2) // params
                .add_storage_buffer_read_only(3) // kernel
                .build();

            compiler.create_bind_group_layout("Filter Bind Group Layout (with kernel)", &entries)
        }))
    }

    fn get_gaussian_pipeline(device: &GpuDevice) -> Result<&'static ComputePipeline> {
        static PIPELINE: OnceCell<ComputePipeline> = OnceCell::new();

        Ok(PIPELINE.get_or_init(|| {
            let compiler = ShaderCompiler::new(device);
            let shader = compiler
                .compile(
                    "Filter Shader",
                    ShaderSource::Embedded(crate::shader::embedded::FILTER_SHADER),
                )
                .expect("Failed to compile filter shader");

            let layout =
                Self::get_bind_group_layout(device).expect("Failed to create bind group layout");

            compiler
                .create_pipeline("Gaussian Blur Pipeline", &shader, "convolve_main", layout)
                .expect("Failed to create pipeline")
        }))
    }

    fn get_sharpen_pipeline(device: &GpuDevice) -> Result<&'static ComputePipeline> {
        static PIPELINE: OnceCell<ComputePipeline> = OnceCell::new();

        Ok(PIPELINE.get_or_init(|| {
            let compiler = ShaderCompiler::new(device);
            let shader = compiler
                .compile(
                    "Filter Shader",
                    ShaderSource::Embedded(crate::shader::embedded::FILTER_SHADER),
                )
                .expect("Failed to compile filter shader");

            let layout =
                Self::get_bind_group_layout(device).expect("Failed to create bind group layout");

            compiler
                .create_pipeline("Sharpen Pipeline", &shader, "unsharp_mask", layout)
                .expect("Failed to create pipeline")
        }))
    }

    fn get_edge_detect_pipeline(device: &GpuDevice) -> Result<&'static ComputePipeline> {
        static PIPELINE: OnceCell<ComputePipeline> = OnceCell::new();

        Ok(PIPELINE.get_or_init(|| {
            let compiler = ShaderCompiler::new(device);
            let shader = compiler
                .compile(
                    "Filter Shader",
                    ShaderSource::Embedded(crate::shader::embedded::FILTER_SHADER),
                )
                .expect("Failed to compile filter shader");

            let layout =
                Self::get_bind_group_layout(device).expect("Failed to create bind group layout");

            compiler
                .create_pipeline("Edge Detect Pipeline", &shader, "edge_detect", layout)
                .expect("Failed to create pipeline")
        }))
    }

    fn get_convolve_pipeline(device: &GpuDevice) -> Result<&'static ComputePipeline> {
        static PIPELINE: OnceCell<ComputePipeline> = OnceCell::new();

        Ok(PIPELINE.get_or_init(|| {
            let compiler = ShaderCompiler::new(device);
            let shader = compiler
                .compile(
                    "Filter Shader",
                    ShaderSource::Embedded(crate::shader::embedded::FILTER_SHADER),
                )
                .expect("Failed to compile filter shader");

            let layout = Self::get_bind_group_layout_with_kernel(device)
                .expect("Failed to create bind group layout");

            compiler
                .create_pipeline("Convolve Pipeline", &shader, "convolve_main", layout)
                .expect("Failed to create pipeline")
        }))
    }
}
