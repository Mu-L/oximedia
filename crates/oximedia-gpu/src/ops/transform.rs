//! Transform operations (DCT, FFT) for frequency domain processing

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
struct TransformParams {
    width: u32,
    height: u32,
    block_size: u32,
    transform_type: u32,
    stride: u32,
    is_inverse: u32,
    padding1: u32,
    padding2: u32,
}

/// Transform operations for frequency domain processing
pub struct TransformOperation;

impl TransformOperation {
    /// Compute 2D DCT (Discrete Cosine Transform)
    ///
    /// Computes the forward DCT on 8x8 blocks. Input dimensions must be
    /// multiples of 8.
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device
    /// * `input` - Input data (f32 values)
    /// * `output` - Output DCT coefficients
    /// * `width` - Data width (must be multiple of 8)
    /// * `height` - Data height (must be multiple of 8)
    ///
    /// # Errors
    ///
    /// Returns an error if dimensions are invalid or if the GPU operation fails.
    pub fn dct_2d(
        device: &GpuDevice,
        input: &[f32],
        output: &mut [f32],
        width: u32,
        height: u32,
    ) -> Result<()> {
        if width % 8 != 0 || height % 8 != 0 {
            return Err(GpuError::InvalidDimensions { width, height });
        }

        utils::validate_dimensions(width, height)?;

        let expected_size = (width * height) as usize;
        if input.len() < expected_size || output.len() < expected_size {
            return Err(GpuError::InvalidBufferSize {
                expected: expected_size,
                actual: input.len().min(output.len()),
            });
        }

        let pipeline = Self::get_dct_8x8_pipeline(device)?;
        let layout = Self::get_bind_group_layout(device)?;

        Self::execute_transform(
            device, pipeline, layout, input, output, width, height, 8, 0, // DCT
        )
    }

    /// Compute 2D IDCT (Inverse Discrete Cosine Transform)
    ///
    /// Computes the inverse DCT on 8x8 blocks. Input dimensions must be
    /// multiples of 8.
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device
    /// * `input` - Input DCT coefficients
    /// * `output` - Output reconstructed data
    /// * `width` - Data width (must be multiple of 8)
    /// * `height` - Data height (must be multiple of 8)
    ///
    /// # Errors
    ///
    /// Returns an error if dimensions are invalid or if the GPU operation fails.
    pub fn idct_2d(
        device: &GpuDevice,
        input: &[f32],
        output: &mut [f32],
        width: u32,
        height: u32,
    ) -> Result<()> {
        if width % 8 != 0 || height % 8 != 0 {
            return Err(GpuError::InvalidDimensions { width, height });
        }

        utils::validate_dimensions(width, height)?;

        let expected_size = (width * height) as usize;
        if input.len() < expected_size || output.len() < expected_size {
            return Err(GpuError::InvalidBufferSize {
                expected: expected_size,
                actual: input.len().min(output.len()),
            });
        }

        let pipeline = Self::get_idct_8x8_pipeline(device)?;
        let layout = Self::get_bind_group_layout(device)?;

        Self::execute_transform(
            device, pipeline, layout, input, output, width, height, 8, 1, // IDCT
        )
    }

    /// Compute general 2D DCT using row-column decomposition
    ///
    /// This method works for any dimensions, not just multiples of 8.
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device
    /// * `input` - Input data (f32 values)
    /// * `output` - Output DCT coefficients
    /// * `width` - Data width
    /// * `height` - Data height
    ///
    /// # Errors
    ///
    /// Returns an error if dimensions are invalid or if the GPU operation fails.
    pub fn dct_2d_general(
        device: &GpuDevice,
        input: &[f32],
        output: &mut [f32],
        width: u32,
        height: u32,
    ) -> Result<()> {
        utils::validate_dimensions(width, height)?;

        let expected_size = (width * height) as usize;
        if input.len() < expected_size || output.len() < expected_size {
            return Err(GpuError::InvalidBufferSize {
                expected: expected_size,
                actual: input.len().min(output.len()),
            });
        }

        // Two-pass DCT: row then column
        let mut temp = vec![0.0f32; expected_size];

        // Row DCT
        let row_pipeline = Self::get_dct_row_pipeline(device)?;
        let layout = Self::get_bind_group_layout(device)?;

        Self::execute_transform(
            device,
            row_pipeline,
            layout,
            input,
            &mut temp,
            width,
            height,
            width,
            0,
        )?;

        // Column DCT
        let col_pipeline = Self::get_dct_col_pipeline(device)?;

        Self::execute_transform(
            device,
            col_pipeline,
            layout,
            &temp,
            output,
            width,
            height,
            height,
            0,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_transform(
        device: &GpuDevice,
        pipeline: &ComputePipeline,
        layout: &BindGroupLayout,
        input: &[f32],
        output: &mut [f32],
        width: u32,
        height: u32,
        block_size: u32,
        transform_type: u32,
    ) -> Result<()> {
        let input_bytes = bytemuck::cast_slice(input);
        let output_size = std::mem::size_of_val(output);

        // Create buffers
        let input_buffer = utils::create_storage_buffer(device, input_bytes.len() as u64)?;
        let output_buffer = utils::create_storage_buffer(device, output_size as u64)?;

        // Upload input data
        device
            .queue()
            .write_buffer(input_buffer.buffer(), 0, input_bytes);

        // Create uniform buffer for parameters
        let params = TransformParams {
            width,
            height,
            block_size,
            transform_type,
            stride: width,
            is_inverse: 0,
            padding1: 0,
            padding2: 0,
        };
        let params_bytes = bytemuck::bytes_of(&params);
        let params_buffer = utils::create_uniform_buffer(device, params_bytes)?;

        // Create bind group
        let compiler = ShaderCompiler::new(device);
        let bind_group = compiler.create_bind_group(
            "Transform Bind Group",
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
        Self::dispatch_compute(device, pipeline, &bind_group, width, height, block_size)?;

        // Read back results
        let readback_buffer = utils::create_readback_buffer(device, output_size as u64)?;
        let mut encoder = device
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Transform Copy Encoder"),
            });

        output_buffer.copy_to(&mut encoder, &readback_buffer, 0, 0, output_size as u64)?;

        device.queue().submit(Some(encoder.finish()));
        device.wait();

        let result = readback_buffer.read(device, 0, output_size as u64)?;
        let result_f32: &[f32] = bytemuck::cast_slice(&result);
        output.copy_from_slice(result_f32);

        Ok(())
    }

    fn dispatch_compute(
        device: &GpuDevice,
        pipeline: &ComputePipeline,
        bind_group: &BindGroup,
        width: u32,
        height: u32,
        block_size: u32,
    ) -> Result<()> {
        let mut encoder = device
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Transform Compute Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Transform Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(pipeline);
            compute_pass.set_bind_group(0, bind_group, &[]);

            if block_size == 8 {
                // For 8x8 DCT, dispatch one workgroup per block
                let dispatch_x = width / 8;
                let dispatch_y = height / 8;
                compute_pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
            } else {
                // For row/column transforms
                let total_elements = width * height;
                let dispatch = total_elements.div_ceil(256);
                compute_pass.dispatch_workgroups(dispatch, 1, 1);
            }
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

            compiler.create_bind_group_layout("Transform Bind Group Layout", &entries)
        }))
    }

    fn get_dct_8x8_pipeline(device: &GpuDevice) -> Result<&'static ComputePipeline> {
        static PIPELINE: OnceCell<ComputePipeline> = OnceCell::new();

        Ok(PIPELINE.get_or_init(|| {
            let compiler = ShaderCompiler::new(device);
            let shader = compiler
                .compile(
                    "Transform Shader",
                    ShaderSource::Embedded(crate::shader::embedded::TRANSFORM_SHADER),
                )
                .expect("Failed to compile transform shader");

            let layout =
                Self::get_bind_group_layout(device).expect("Failed to create bind group layout");

            compiler
                .create_pipeline("DCT 8x8 Pipeline", &shader, "dct_8x8", layout)
                .expect("Failed to create pipeline")
        }))
    }

    fn get_idct_8x8_pipeline(device: &GpuDevice) -> Result<&'static ComputePipeline> {
        static PIPELINE: OnceCell<ComputePipeline> = OnceCell::new();

        Ok(PIPELINE.get_or_init(|| {
            let compiler = ShaderCompiler::new(device);
            let shader = compiler
                .compile(
                    "Transform Shader",
                    ShaderSource::Embedded(crate::shader::embedded::TRANSFORM_SHADER),
                )
                .expect("Failed to compile transform shader");

            let layout =
                Self::get_bind_group_layout(device).expect("Failed to create bind group layout");

            compiler
                .create_pipeline("IDCT 8x8 Pipeline", &shader, "idct_8x8", layout)
                .expect("Failed to create pipeline")
        }))
    }

    fn get_dct_row_pipeline(device: &GpuDevice) -> Result<&'static ComputePipeline> {
        static PIPELINE: OnceCell<ComputePipeline> = OnceCell::new();

        Ok(PIPELINE.get_or_init(|| {
            let compiler = ShaderCompiler::new(device);
            let shader = compiler
                .compile(
                    "Transform Shader",
                    ShaderSource::Embedded(crate::shader::embedded::TRANSFORM_SHADER),
                )
                .expect("Failed to compile transform shader");

            let layout =
                Self::get_bind_group_layout(device).expect("Failed to create bind group layout");

            compiler
                .create_pipeline("DCT Row Pipeline", &shader, "dct_row", layout)
                .expect("Failed to create pipeline")
        }))
    }

    fn get_dct_col_pipeline(device: &GpuDevice) -> Result<&'static ComputePipeline> {
        static PIPELINE: OnceCell<ComputePipeline> = OnceCell::new();

        Ok(PIPELINE.get_or_init(|| {
            let compiler = ShaderCompiler::new(device);
            let shader = compiler
                .compile(
                    "Transform Shader",
                    ShaderSource::Embedded(crate::shader::embedded::TRANSFORM_SHADER),
                )
                .expect("Failed to compile transform shader");

            let layout =
                Self::get_bind_group_layout(device).expect("Failed to create bind group layout");

            compiler
                .create_pipeline("DCT Column Pipeline", &shader, "dct_col", layout)
                .expect("Failed to create pipeline")
        }))
    }
}
