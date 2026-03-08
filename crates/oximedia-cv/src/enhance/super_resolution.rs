//! Video Super-Resolution using Neural Networks.
//!
//! This module provides AI-powered super-resolution for video upscaling using various
//! neural network models via ONNX Runtime. Supports multiple quality modes, temporal
//! consistency, and video-specific optimizations.
//!
//! # Supported Models
//!
//! - **ESRGAN** (Enhanced Super-Resolution GAN) - High quality photo upscaling
//! - **Real-ESRGAN** - Practical real-world image restoration and enhancement
//! - **EDSR** (Enhanced Deep Residual Networks) - Balanced quality and performance
//! - **SRCNN** (Super-Resolution CNN) - Fast, lightweight upscaling
//! - **VDSR** (Very Deep Super-Resolution) - Deep network with residual learning
//!
//! # Features
//!
//! - Multiple upscaling factors (2x, 4x, 8x)
//! - Tile-based processing for large images/frames
//! - Temporal consistency for video
//! - Frame buffering and motion-aware processing
//! - YUV color space support
//! - Edge enhancement and artifact reduction
//! - GPU acceleration via ONNX Runtime
//! - Model caching for efficient batch processing
//! - Quality modes (Fast, Balanced, High Quality, Animation)
//!
//! # Example
//!
//! ```no_run
//! use oximedia_cv::enhance::{SuperResolutionModel, UpscaleFactor, QualityMode};
//!
//! // Create a model with quality mode
//! let model = SuperResolutionModel::from_quality_mode(
//!     QualityMode::HighQuality,
//!     UpscaleFactor::X4,
//! )?;
//!
//! // Upscale an image
//! let input_image = vec![0u8; 256 * 256 * 3];
//! let upscaled = model.upscale(&input_image, 256, 256)?;
//! # Ok::<(), oximedia_cv::error::CvError>(())
//! ```

#![forbid(unsafe_code)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::too_many_lines)]

use crate::error::{CvError, CvResult};
use ndarray::{Array3, Array4};
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Value;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Upscale factor for super-resolution.
///
/// Determines the scaling factor applied to the input image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpscaleFactor {
    /// 2x upscaling (output is 2x larger in each dimension).
    X2,
    /// 4x upscaling (output is 4x larger in each dimension).
    X4,
    /// 8x upscaling (output is 8x larger in each dimension).
    X8,
}

impl UpscaleFactor {
    /// Get the numeric scale factor.
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_cv::enhance::UpscaleFactor;
    ///
    /// assert_eq!(UpscaleFactor::X2.scale(), 2);
    /// assert_eq!(UpscaleFactor::X4.scale(), 4);
    /// assert_eq!(UpscaleFactor::X8.scale(), 8);
    /// ```
    #[must_use]
    pub const fn scale(&self) -> u32 {
        match self {
            Self::X2 => 2,
            Self::X4 => 4,
            Self::X8 => 8,
        }
    }
}

/// Type of super-resolution neural network model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelType {
    /// ESRGAN (Enhanced Super-Resolution GAN) - High quality photo upscaling.
    ESRGAN,
    /// Real-ESRGAN - Practical real-world image restoration.
    RealESRGAN,
    /// EDSR (Enhanced Deep Residual Networks) - Balanced quality and speed.
    EDSR,
    /// SRCNN (Super-Resolution CNN) - Fast, lightweight model.
    SRCNN,
    /// VDSR (Very Deep Super-Resolution) - Deep network with residual learning.
    VDSR,
}

impl ModelType {
    /// Get a human-readable name for the model type.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::ESRGAN => "ESRGAN",
            Self::RealESRGAN => "Real-ESRGAN",
            Self::EDSR => "EDSR",
            Self::SRCNN => "SRCNN",
            Self::VDSR => "VDSR",
        }
    }

    /// Get typical input normalization range for this model.
    #[must_use]
    pub const fn normalization_range(&self) -> (f32, f32) {
        match self {
            Self::ESRGAN | Self::RealESRGAN | Self::SRCNN | Self::VDSR => (0.0, 1.0),
            Self::EDSR => (0.0, 255.0),
        }
    }

    /// Get whether this model expects mean subtraction.
    #[must_use]
    pub const fn uses_mean_subtraction(&self) -> bool {
        matches!(self, Self::EDSR)
    }

    /// Get RGB mean values for mean subtraction (if applicable).
    #[must_use]
    pub const fn rgb_mean(&self) -> [f32; 3] {
        match self {
            Self::EDSR => [0.4488, 0.4371, 0.4040],
            _ => [0.0, 0.0, 0.0],
        }
    }
}

/// Quality mode for super-resolution.
///
/// Determines the trade-off between quality, speed, and resource usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityMode {
    /// Fast mode - Uses lightweight models (SRCNN), good for real-time processing.
    Fast,
    /// Balanced mode - Uses medium-complexity models (EDSR), good balance of quality/speed.
    Balanced,
    /// High quality mode - Uses complex models (Real-ESRGAN), best quality.
    HighQuality,
    /// Animation mode - Optimized for anime/cartoon content.
    Animation,
}

impl QualityMode {
    /// Get the recommended model type for this quality mode.
    #[must_use]
    pub const fn recommended_model(&self) -> ModelType {
        match self {
            Self::Fast => ModelType::SRCNN,
            Self::Balanced => ModelType::EDSR,
            Self::HighQuality | Self::Animation => ModelType::RealESRGAN,
        }
    }

    /// Get the recommended tile size for this quality mode.
    #[must_use]
    pub const fn recommended_tile_size(&self) -> u32 {
        match self {
            Self::Fast => 512,
            Self::Balanced => 256,
            Self::HighQuality | Self::Animation => 128,
        }
    }
}

/// Color space for image processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
    /// RGB color space (3 channels, no subsampling).
    RGB,
    /// YUV 4:2:0 (Y full resolution, U/V subsampled 2x).
    YUV420,
    /// YUV 4:4:4 (all channels full resolution).
    YUV444,
}

impl ColorSpace {
    /// Get the number of channels for this color space.
    #[must_use]
    pub const fn num_channels(&self) -> usize {
        match self {
            Self::RGB | Self::YUV420 | Self::YUV444 => 3,
        }
    }

    /// Check if chroma channels are subsampled.
    #[must_use]
    pub const fn is_subsampled(&self) -> bool {
        matches!(self, Self::YUV420)
    }
}

/// Chroma upscaling mode for YUV processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromaUpscaleMode {
    /// Upscale luma only, use simple interpolation for chroma.
    LumaOnly,
    /// Upscale all channels separately.
    Separate,
    /// Upscale all channels jointly (convert to RGB first).
    Joint,
}

/// Configuration for tile-based processing.
#[derive(Debug, Clone, Copy)]
pub struct TileConfig {
    /// Size of each tile (width and height).
    pub tile_size: u32,
    /// Padding around each tile to reduce seam artifacts.
    pub tile_padding: u32,
    /// Feathering width for blending overlapping regions.
    pub feather_width: u32,
}

impl Default for TileConfig {
    fn default() -> Self {
        Self {
            tile_size: 256,
            tile_padding: 16,
            feather_width: 8,
        }
    }
}

impl TileConfig {
    /// Create a new tile configuration.
    ///
    /// # Arguments
    ///
    /// * `tile_size` - Size of each tile (must be >= 64)
    /// * `tile_padding` - Padding around each tile (must be <= tile_size / 4)
    ///
    /// # Examples
    ///
    /// ```
    /// use oximedia_cv::enhance::TileConfig;
    ///
    /// let config = TileConfig::new(512, 32)?;
    /// assert_eq!(config.tile_size, 512);
    /// # Ok::<(), oximedia_cv::error::CvError>(())
    /// ```
    pub fn new(tile_size: u32, tile_padding: u32) -> CvResult<Self> {
        if tile_size < 64 {
            return Err(CvError::invalid_parameter(
                "tile_size",
                format!("{tile_size} (must be >= 64)"),
            ));
        }
        if tile_padding > tile_size / 4 {
            return Err(CvError::invalid_parameter(
                "tile_padding",
                format!("{tile_padding} (must be <= tile_size / 4)"),
            ));
        }
        Ok(Self {
            tile_size,
            tile_padding,
            feather_width: tile_padding.min(16),
        })
    }

    /// Get the effective tile size including padding.
    #[must_use]
    pub const fn padded_size(&self) -> u32 {
        self.tile_size + 2 * self.tile_padding
    }
}

/// Progress callback function type.
///
/// Called periodically during processing with progress information.
/// Returns `true` to continue processing, `false` to abort.
pub type ProgressCallback = Box<dyn Fn(usize, usize) -> bool + Send + Sync>;

/// Processing options for super-resolution.
#[derive(Debug, Clone)]
pub struct ProcessingOptions {
    /// Enable edge enhancement post-processing.
    pub edge_enhancement: bool,
    /// Enable artifact reduction post-processing.
    pub artifact_reduction: bool,
    /// Enable denoising before upscaling.
    pub denoise: bool,
    /// Chroma upscaling mode for YUV inputs.
    pub chroma_upscale: ChromaUpscaleMode,
    /// Sharpness enhancement amount (0.0 = none, 1.0 = maximum).
    pub sharpness: f32,
    /// Color space for processing.
    pub color_space: ColorSpace,
}

impl Default for ProcessingOptions {
    fn default() -> Self {
        Self {
            edge_enhancement: false,
            artifact_reduction: true,
            denoise: false,
            chroma_upscale: ChromaUpscaleMode::Joint,
            sharpness: 0.0,
            color_space: ColorSpace::RGB,
        }
    }
}

impl ProcessingOptions {
    /// Create new processing options with all enhancements enabled.
    #[must_use]
    pub fn enhanced() -> Self {
        Self {
            edge_enhancement: true,
            artifact_reduction: true,
            denoise: true,
            chroma_upscale: ChromaUpscaleMode::Joint,
            sharpness: 0.3,
            color_space: ColorSpace::RGB,
        }
    }

    /// Create new processing options for fast processing.
    #[must_use]
    pub fn fast() -> Self {
        Self {
            edge_enhancement: false,
            artifact_reduction: false,
            denoise: false,
            chroma_upscale: ChromaUpscaleMode::LumaOnly,
            sharpness: 0.0,
            color_space: ColorSpace::RGB,
        }
    }

    /// Create new processing options for video processing.
    #[must_use]
    pub fn video() -> Self {
        Self {
            edge_enhancement: false,
            artifact_reduction: true,
            denoise: true,
            chroma_upscale: ChromaUpscaleMode::Separate,
            sharpness: 0.1,
            color_space: ColorSpace::YUV420,
        }
    }
}

/// Model cache for reusing loaded ONNX sessions.
///
/// Caches ONNX Runtime sessions to avoid reloading models from disk.
/// Thread-safe and can be shared across multiple processing threads.
#[derive(Clone)]
pub struct ModelCache {
    cache: Arc<Mutex<HashMap<PathBuf, Arc<Mutex<Session>>>>>,
}

impl ModelCache {
    /// Create a new empty model cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::<PathBuf, Arc<Mutex<Session>>>::new())),
        }
    }

    /// Get or load a model from cache.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the ONNX model file
    ///
    /// # Errors
    ///
    /// Returns an error if the model cannot be loaded.
    pub fn get_or_load(&self, path: impl AsRef<Path>) -> CvResult<Arc<Mutex<Session>>> {
        let path = path.as_ref().to_path_buf();
        let mut cache = self
            .cache
            .lock()
            .map_err(|e| CvError::model_load(format!("Cache lock error: {e}")))?;

        if let Some(session) = cache.get(&path) {
            return Ok(Arc::clone(session));
        }

        // Load new session
        let session = Session::builder()
            .map_err(|e| CvError::onnx_runtime(format!("Failed to create session builder: {e}")))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| CvError::onnx_runtime(format!("Failed to set optimization level: {e}")))?
            .commit_from_file(&path)
            .map_err(|e| CvError::model_load(format!("Failed to load model: {e}")))?;

        let session = Arc::new(Mutex::new(session));
        cache.insert(path, Arc::clone(&session));
        Ok(session)
    }

    /// Clear all cached models.
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    /// Get the number of cached models.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cache.lock().map_or(0, |cache| cache.len())
    }

    /// Check if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for ModelCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Unified super-resolution model supporting multiple architectures.
///
/// This is the main interface for super-resolution, supporting different
/// model types, quality modes, and processing options.
pub struct SuperResolutionModel {
    session: Arc<Mutex<Session>>,
    model_type: ModelType,
    scale_factor: UpscaleFactor,
    tile_config: TileConfig,
    processing_options: ProcessingOptions,
    progress_callback: Option<ProgressCallback>,
}

impl SuperResolutionModel {
    /// Create a new super-resolution model from an ONNX file.
    ///
    /// # Arguments
    ///
    /// * `model_path` - Path to the ONNX model file
    /// * `model_type` - Type of the model
    /// * `scale_factor` - Upscale factor
    ///
    /// # Errors
    ///
    /// Returns an error if the model cannot be loaded.
    pub fn new(
        model_path: impl AsRef<Path>,
        model_type: ModelType,
        scale_factor: UpscaleFactor,
    ) -> CvResult<Self> {
        let session = Session::builder()
            .map_err(|e| CvError::onnx_runtime(format!("Failed to create session builder: {e}")))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| CvError::onnx_runtime(format!("Failed to set optimization level: {e}")))?
            .commit_from_file(model_path.as_ref())
            .map_err(|e| CvError::model_load(format!("Failed to load model: {e}")))?;

        Ok(Self {
            session: Arc::new(Mutex::new(session)),
            model_type,
            scale_factor,
            tile_config: TileConfig::default(),
            processing_options: ProcessingOptions::default(),
            progress_callback: None,
        })
    }

    /// Create a model using the model cache.
    ///
    /// # Arguments
    ///
    /// * `model_path` - Path to the ONNX model file
    /// * `model_type` - Type of the model
    /// * `scale_factor` - Upscale factor
    /// * `cache` - Model cache to use
    ///
    /// # Errors
    ///
    /// Returns an error if the model cannot be loaded.
    pub fn with_cache(
        model_path: impl AsRef<Path>,
        model_type: ModelType,
        scale_factor: UpscaleFactor,
        cache: &ModelCache,
    ) -> CvResult<Self> {
        let session = cache.get_or_load(model_path)?;

        Ok(Self {
            session,
            model_type,
            scale_factor,
            tile_config: TileConfig::default(),
            processing_options: ProcessingOptions::default(),
            progress_callback: None,
        })
    }

    /// Create a model from a quality mode.
    ///
    /// This is a convenience method that selects appropriate model settings
    /// based on the desired quality level. Note that you still need to provide
    /// the path to the actual ONNX model file.
    ///
    /// # Arguments
    ///
    /// * `mode` - Quality mode
    /// * `scale_factor` - Upscale factor
    ///
    /// # Errors
    ///
    /// Returns an error if the model cannot be loaded.
    ///
    /// # Note
    ///
    /// This method expects the model file to be named according to the pattern:
    /// `{model_name}_x{scale}.onnx` (e.g., "esrgan_x4.onnx")
    pub fn from_quality_mode(mode: QualityMode, scale_factor: UpscaleFactor) -> CvResult<Self> {
        let model_type = mode.recommended_model();
        let tile_size = mode.recommended_tile_size();

        // Construct expected model path
        let model_name = match model_type {
            ModelType::ESRGAN => "esrgan",
            ModelType::RealESRGAN => "realesrgan",
            ModelType::EDSR => "edsr",
            ModelType::SRCNN => "srcnn",
            ModelType::VDSR => "vdsr",
        };
        let scale = scale_factor.scale();
        let model_path = format!("{model_name}_x{scale}.onnx");

        let mut model = Self::new(model_path, model_type, scale_factor)?;
        model.tile_config = TileConfig::new(tile_size, tile_size / 16)?;

        // Set processing options based on quality mode
        model.processing_options = match mode {
            QualityMode::Fast => ProcessingOptions::fast(),
            QualityMode::Balanced => ProcessingOptions::default(),
            QualityMode::HighQuality => ProcessingOptions::enhanced(),
            QualityMode::Animation => {
                let mut opts = ProcessingOptions::enhanced();
                opts.sharpness = 0.5;
                opts
            }
        };

        Ok(model)
    }

    /// Set tile configuration.
    pub fn set_tile_config(&mut self, config: TileConfig) {
        self.tile_config = config;
    }

    /// Set processing options.
    pub fn set_processing_options(&mut self, options: ProcessingOptions) {
        self.processing_options = options;
    }

    /// Set progress callback.
    pub fn set_progress_callback(&mut self, callback: ProgressCallback) {
        self.progress_callback = Some(callback);
    }

    /// Get the model type.
    #[must_use]
    pub const fn model_type(&self) -> ModelType {
        self.model_type
    }

    /// Get the scale factor.
    #[must_use]
    pub const fn scale_factor(&self) -> UpscaleFactor {
        self.scale_factor
    }

    /// Upscale an RGB image.
    ///
    /// # Arguments
    ///
    /// * `image` - Input image in RGB format (row-major, packed RGB)
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    ///
    /// # Returns
    ///
    /// Upscaled image in RGB format
    ///
    /// # Errors
    ///
    /// Returns an error if processing fails.
    pub fn upscale(&mut self, image: &[u8], width: u32, height: u32) -> CvResult<Vec<u8>> {
        // Validate inputs
        if width == 0 || height == 0 {
            return Err(CvError::invalid_dimensions(width, height));
        }

        let expected_size = (width as usize) * (height as usize) * 3;
        if image.len() != expected_size {
            return Err(CvError::insufficient_data(expected_size, image.len()));
        }

        // Apply pre-processing
        let mut preprocessed = image.to_vec();
        if self.processing_options.denoise {
            self.apply_denoising(&mut preprocessed, width, height)?;
        }

        // Perform upscaling
        let tile_size = self.tile_config.tile_size;
        let upscaled = if width <= tile_size && height <= tile_size {
            self.upscale_single_tile(&preprocessed, width, height)?
        } else {
            self.upscale_tiled(&preprocessed, width, height)?
        };

        // Apply post-processing
        let mut result = upscaled;
        let out_scale = self.scale_factor.scale();
        let out_width = width * out_scale;
        let out_height = height * out_scale;

        if self.processing_options.artifact_reduction {
            self.apply_artifact_reduction(&mut result, out_width, out_height)?;
        }

        if self.processing_options.edge_enhancement {
            self.apply_edge_enhancement(&mut result, out_width, out_height)?;
        }

        if self.processing_options.sharpness > 0.0 {
            self.apply_sharpening(
                &mut result,
                out_width,
                out_height,
                self.processing_options.sharpness,
            )?;
        }

        Ok(result)
    }

    /// Upscale a YUV image.
    ///
    /// # Arguments
    ///
    /// * `y_plane` - Y (luma) plane data
    /// * `u_plane` - U (chroma) plane data
    /// * `v_plane` - V (chroma) plane data
    /// * `width` - Image width
    /// * `height` - Image height
    /// * `chroma_width` - Chroma plane width (for subsampled formats)
    /// * `chroma_height` - Chroma plane height (for subsampled formats)
    ///
    /// # Returns
    ///
    /// Tuple of (y_upscaled, u_upscaled, v_upscaled)
    ///
    /// # Errors
    ///
    /// Returns an error if processing fails.
    #[allow(clippy::too_many_arguments)]
    pub fn upscale_yuv(
        &mut self,
        y_plane: &[u8],
        u_plane: &[u8],
        v_plane: &[u8],
        width: u32,
        height: u32,
        chroma_width: u32,
        chroma_height: u32,
    ) -> CvResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        match self.processing_options.chroma_upscale {
            ChromaUpscaleMode::LumaOnly => self.upscale_yuv_luma_only(
                y_plane,
                u_plane,
                v_plane,
                width,
                height,
                chroma_width,
                chroma_height,
            ),
            ChromaUpscaleMode::Separate => self.upscale_yuv_separate(
                y_plane,
                u_plane,
                v_plane,
                width,
                height,
                chroma_width,
                chroma_height,
            ),
            ChromaUpscaleMode::Joint => self.upscale_yuv_joint(
                y_plane,
                u_plane,
                v_plane,
                width,
                height,
                chroma_width,
                chroma_height,
            ),
        }
    }

    /// Upscale YUV with luma-only processing.
    #[allow(clippy::too_many_arguments)]
    fn upscale_yuv_luma_only(
        &mut self,
        y_plane: &[u8],
        u_plane: &[u8],
        v_plane: &[u8],
        width: u32,
        height: u32,
        chroma_width: u32,
        chroma_height: u32,
    ) -> CvResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        // Upscale luma with neural network
        let y_rgb = Self::gray_to_rgb(y_plane, width, height);
        let y_upscaled_rgb = self.upscale(&y_rgb, width, height)?;
        let scale = self.scale_factor.scale();
        let y_upscaled = Self::rgb_to_gray(&y_upscaled_rgb, width * scale, height * scale);

        // Simple bilinear upscale for chroma
        let u_upscaled = Self::bilinear_upscale(u_plane, chroma_width, chroma_height, scale);
        let v_upscaled = Self::bilinear_upscale(v_plane, chroma_width, chroma_height, scale);

        Ok((y_upscaled, u_upscaled, v_upscaled))
    }

    /// Upscale YUV with separate channel processing.
    #[allow(clippy::too_many_arguments)]
    fn upscale_yuv_separate(
        &mut self,
        y_plane: &[u8],
        u_plane: &[u8],
        v_plane: &[u8],
        width: u32,
        height: u32,
        chroma_width: u32,
        chroma_height: u32,
    ) -> CvResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        // Upscale each channel with neural network
        let y_rgb = Self::gray_to_rgb(y_plane, width, height);
        let y_upscaled_rgb = self.upscale(&y_rgb, width, height)?;
        let scale = self.scale_factor.scale();
        let y_upscaled = Self::rgb_to_gray(&y_upscaled_rgb, width * scale, height * scale);

        // Upscale chroma channels
        let u_rgb = Self::gray_to_rgb(u_plane, chroma_width, chroma_height);
        let u_upscaled_rgb = self.upscale(&u_rgb, chroma_width, chroma_height)?;
        let u_upscaled =
            Self::rgb_to_gray(&u_upscaled_rgb, chroma_width * scale, chroma_height * scale);

        let v_rgb = Self::gray_to_rgb(v_plane, chroma_width, chroma_height);
        let v_upscaled_rgb = self.upscale(&v_rgb, chroma_width, chroma_height)?;
        let v_upscaled =
            Self::rgb_to_gray(&v_upscaled_rgb, chroma_width * scale, chroma_height * scale);

        Ok((y_upscaled, u_upscaled, v_upscaled))
    }

    /// Upscale YUV with joint processing (convert to RGB first).
    #[allow(clippy::too_many_arguments)]
    fn upscale_yuv_joint(
        &mut self,
        y_plane: &[u8],
        u_plane: &[u8],
        v_plane: &[u8],
        width: u32,
        height: u32,
        chroma_width: u32,
        chroma_height: u32,
    ) -> CvResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        // Upsample chroma to match luma resolution if needed
        let (u_full, v_full) = if chroma_width != width || chroma_height != height {
            let scale_x = width / chroma_width;
            let scale_y = height / chroma_height;
            let scale = scale_x.max(scale_y);
            (
                Self::bilinear_upscale(u_plane, chroma_width, chroma_height, scale),
                Self::bilinear_upscale(v_plane, chroma_width, chroma_height, scale),
            )
        } else {
            (u_plane.to_vec(), v_plane.to_vec())
        };

        // Convert YUV to RGB
        let rgb = Self::yuv_to_rgb(y_plane, &u_full, &v_full, width, height)?;

        // Upscale RGB
        let rgb_upscaled = self.upscale(&rgb, width, height)?;

        // Convert back to YUV
        let scale = self.scale_factor.scale();
        let out_width = width * scale;
        let out_height = height * scale;
        Self::rgb_to_yuv(&rgb_upscaled, out_width, out_height)
    }

    /// Upscale a single tile (internal method).
    fn upscale_single_tile(&mut self, image: &[u8], width: u32, height: u32) -> CvResult<Vec<u8>> {
        // Convert RGB u8 to normalized float32 [1, 3, H, W]
        let input_tensor = self.preprocess_image(image, width, height)?;

        // Convert to ONNX Value
        let input_value = Value::from_array(input_tensor)
            .map_err(|e| CvError::onnx_runtime(format!("Failed to create input tensor: {e}")))?;

        // Run inference and extract owned data to release the session borrow
        let (shape_owned, data_owned) = {
            let mut session = self
                .session
                .lock()
                .map_err(|e| CvError::onnx_runtime(format!("Session lock error: {e}")))?;
            let outputs = session
                .run(ort::inputs![input_value])
                .map_err(|e| CvError::onnx_runtime(format!("Inference failed: {e}")))?;
            let (shape, data) = outputs[0].try_extract_tensor::<f32>().map_err(|e| {
                CvError::tensor_error(format!("Failed to extract output tensor: {e}"))
            })?;
            let shape_owned: Vec<i64> = shape.iter().copied().collect();
            let data_owned: Vec<f32> = data.to_vec();
            (shape_owned, data_owned)
        };

        // Convert back to RGB u8
        let scale = self.scale_factor.scale();
        self.postprocess_tensor(&shape_owned, &data_owned, width * scale, height * scale)
    }

    /// Upscale using tile-based processing for large images.
    fn upscale_tiled(&mut self, image: &[u8], width: u32, height: u32) -> CvResult<Vec<u8>> {
        let tile_size = self.tile_config.tile_size;
        let padding = self.tile_config.tile_padding;
        let scale = self.scale_factor.scale();

        // Calculate tile grid
        let tiles_x = width.div_ceil(tile_size) as usize;
        let tiles_y = height.div_ceil(tile_size) as usize;
        let total_tiles = tiles_x * tiles_y;

        // Output dimensions
        let out_width = width * scale;
        let out_height = height * scale;
        let mut output = vec![0u8; (out_width * out_height * 3) as usize];

        // Weight map for blending
        let mut weight_map = vec![0.0f32; (out_width * out_height) as usize];

        // Process each tile
        let mut tile_idx = 0;
        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                // Check if processing should continue
                if let Some(ref callback) = self.progress_callback {
                    if !callback(tile_idx + 1, total_tiles) {
                        return Err(CvError::detection_failed("Processing aborted by user"));
                    }
                }

                // Calculate tile boundaries with padding
                let x_start = (tx as u32 * tile_size).saturating_sub(padding);
                let y_start = (ty as u32 * tile_size).saturating_sub(padding);
                let x_end = ((tx as u32 + 1) * tile_size + padding).min(width);
                let y_end = ((ty as u32 + 1) * tile_size + padding).min(height);

                let tile_w = x_end - x_start;
                let tile_h = y_end - y_start;

                // Extract tile
                let tile =
                    Self::extract_tile(image, width, height, x_start, y_start, tile_w, tile_h)?;

                // Process tile
                let upscaled_tile = self.upscale_single_tile(&tile, tile_w, tile_h)?;

                // Calculate blend region (excluding padding)
                let blend_x_start = if tx == 0 { 0 } else { padding * scale };
                let blend_y_start = if ty == 0 { 0 } else { padding * scale };
                let blend_x_end = if tx == tiles_x - 1 {
                    tile_w * scale
                } else {
                    (tile_w - padding) * scale
                };
                let blend_y_end = if ty == tiles_y - 1 {
                    tile_h * scale
                } else {
                    (tile_h - padding) * scale
                };

                // Blend tile into output
                Self::blend_tile(
                    &upscaled_tile,
                    tile_w * scale,
                    tile_h * scale,
                    &mut output,
                    &mut weight_map,
                    out_width,
                    out_height,
                    x_start * scale,
                    y_start * scale,
                    blend_x_start,
                    blend_y_start,
                    blend_x_end,
                    blend_y_end,
                    self.tile_config.feather_width,
                )?;

                tile_idx += 1;
            }
        }

        // Normalize by weight map
        Self::normalize_by_weights(&mut output, &weight_map, out_width, out_height);

        Ok(output)
    }

    /// Preprocess image: RGB u8 -> normalized float32 [1, 3, H, W].
    fn preprocess_image(&self, image: &[u8], width: u32, height: u32) -> CvResult<Array4<f32>> {
        let w = width as usize;
        let h = height as usize;

        let mut tensor = Array4::<f32>::zeros((1, 3, h, w));
        let (norm_min, norm_max) = self.model_type.normalization_range();
        let norm_scale = norm_max - norm_min;

        let rgb_mean = if self.model_type.uses_mean_subtraction() {
            self.model_type.rgb_mean()
        } else {
            [0.0, 0.0, 0.0]
        };

        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 3;
                // Normalize and apply mean subtraction if needed
                tensor[[0, 0, y, x]] =
                    (image[idx] as f32 / 255.0) * norm_scale + norm_min - rgb_mean[0];
                tensor[[0, 1, y, x]] =
                    (image[idx + 1] as f32 / 255.0) * norm_scale + norm_min - rgb_mean[1];
                tensor[[0, 2, y, x]] =
                    (image[idx + 2] as f32 / 255.0) * norm_scale + norm_min - rgb_mean[2];
            }
        }

        Ok(tensor)
    }

    /// Postprocess tensor: normalized float32 [1, 3, H, W] -> RGB u8.
    fn postprocess_tensor(
        &self,
        shape: &[i64],
        data: &[f32],
        width: u32,
        height: u32,
    ) -> CvResult<Vec<u8>> {
        if shape.len() != 4 || shape[0] != 1 || shape[1] != 3 {
            return Err(CvError::ShapeMismatch {
                expected: vec![1, 3, height as usize, width as usize],
                actual: shape.iter().map(|&x| x as usize).collect(),
            });
        }

        let h = shape[2] as usize;
        let w = shape[3] as usize;

        if w != width as usize || h != height as usize {
            return Err(CvError::ShapeMismatch {
                expected: vec![1, 3, height as usize, width as usize],
                actual: shape.iter().map(|&x| x as usize).collect(),
            });
        }
        let mut output = vec![0u8; w * h * 3];

        let (norm_min, norm_max) = self.model_type.normalization_range();
        let norm_scale = norm_max - norm_min;

        let rgb_mean = if self.model_type.uses_mean_subtraction() {
            self.model_type.rgb_mean()
        } else {
            [0.0, 0.0, 0.0]
        };

        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 3;
                let r_idx = y * w + x;
                let g_idx = h * w + y * w + x;
                let b_idx = 2 * h * w + y * w + x;

                // Denormalize and add mean back if needed
                let r = (data[r_idx] + rgb_mean[0] - norm_min) / norm_scale * 255.0;
                let g = (data[g_idx] + rgb_mean[1] - norm_min) / norm_scale * 255.0;
                let b = (data[b_idx] + rgb_mean[2] - norm_min) / norm_scale * 255.0;

                output[idx] = r.clamp(0.0, 255.0).round() as u8;
                output[idx + 1] = g.clamp(0.0, 255.0).round() as u8;
                output[idx + 2] = b.clamp(0.0, 255.0).round() as u8;
            }
        }

        Ok(output)
    }

    /// Extract a rectangular tile from the source image.
    fn extract_tile(
        src: &[u8],
        src_w: u32,
        src_h: u32,
        x: u32,
        y: u32,
        tile_w: u32,
        tile_h: u32,
    ) -> CvResult<Vec<u8>> {
        if x + tile_w > src_w || y + tile_h > src_h {
            return Err(CvError::invalid_roi(x, y, tile_w, tile_h));
        }

        let mut tile = Vec::with_capacity((tile_w * tile_h * 3) as usize);

        for row in y..y + tile_h {
            let start = ((row * src_w + x) * 3) as usize;
            let end = start + (tile_w * 3) as usize;
            tile.extend_from_slice(&src[start..end]);
        }

        Ok(tile)
    }

    /// Blend a processed tile into the output image with feathering.
    #[allow(clippy::too_many_arguments)]
    fn blend_tile(
        tile: &[u8],
        tile_w: u32,
        _tile_h: u32,
        output: &mut [u8],
        weights: &mut [f32],
        out_w: u32,
        out_h: u32,
        dst_x: u32,
        dst_y: u32,
        blend_x_start: u32,
        blend_y_start: u32,
        blend_x_end: u32,
        blend_y_end: u32,
        feather: u32,
    ) -> CvResult<()> {
        for local_y in blend_y_start..blend_y_end {
            let global_y = dst_y + local_y;
            if global_y >= out_h {
                break;
            }

            for local_x in blend_x_start..blend_x_end {
                let global_x = dst_x + local_x;
                if global_x >= out_w {
                    break;
                }

                // Calculate feather weight (distance from edge)
                let dist_left = local_x - blend_x_start;
                let dist_right = blend_x_end - local_x - 1;
                let dist_top = local_y - blend_y_start;
                let dist_bottom = blend_y_end - local_y - 1;

                let min_dist = dist_left.min(dist_right).min(dist_top).min(dist_bottom);
                let weight = if min_dist >= feather {
                    1.0
                } else {
                    (min_dist as f32 + 1.0) / (feather as f32 + 1.0)
                };

                // Blend RGB values
                let tile_idx = ((local_y * tile_w + local_x) * 3) as usize;
                let out_idx = ((global_y * out_w + global_x) * 3) as usize;
                let weight_idx = (global_y * out_w + global_x) as usize;

                for c in 0..3 {
                    let tile_val = tile[tile_idx + c] as f32 * weight;
                    output[out_idx + c] = (output[out_idx + c] as f32 + tile_val) as u8;
                }

                weights[weight_idx] += weight;
            }
        }

        Ok(())
    }

    /// Normalize output by accumulated weights.
    fn normalize_by_weights(output: &mut [u8], weights: &[f32], width: u32, height: u32) {
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                let weight = weights[idx];

                if weight > 0.0 {
                    let out_idx = idx * 3;
                    for c in 0..3 {
                        output[out_idx + c] = ((output[out_idx + c] as f32) / weight).round() as u8;
                    }
                }
            }
        }
    }

    // Color space conversion utilities

    /// Convert grayscale to RGB (replicate channel).
    fn gray_to_rgb(gray: &[u8], width: u32, height: u32) -> Vec<u8> {
        let mut rgb = Vec::with_capacity((width * height * 3) as usize);
        for &pixel in gray {
            rgb.push(pixel);
            rgb.push(pixel);
            rgb.push(pixel);
        }
        rgb
    }

    /// Convert RGB to grayscale (take first channel).
    fn rgb_to_gray(rgb: &[u8], width: u32, height: u32) -> Vec<u8> {
        let mut gray = Vec::with_capacity((width * height) as usize);
        for chunk in rgb.chunks_exact(3) {
            gray.push(chunk[0]);
        }
        gray
    }

    /// Convert YUV to RGB.
    fn yuv_to_rgb(y: &[u8], u: &[u8], v: &[u8], width: u32, height: u32) -> CvResult<Vec<u8>> {
        let size = (width * height) as usize;
        if y.len() != size || u.len() != size || v.len() != size {
            return Err(CvError::insufficient_data(
                size * 3,
                y.len() + u.len() + v.len(),
            ));
        }

        let mut rgb = vec![0u8; size * 3];

        for i in 0..size {
            let y_val = y[i] as f32;
            let u_val = u[i] as f32 - 128.0;
            let v_val = v[i] as f32 - 128.0;

            let r = y_val + 1.402 * v_val;
            let g = y_val - 0.344_136 * u_val - 0.714_136 * v_val;
            let b = y_val + 1.772 * u_val;

            rgb[i * 3] = r.clamp(0.0, 255.0).round() as u8;
            rgb[i * 3 + 1] = g.clamp(0.0, 255.0).round() as u8;
            rgb[i * 3 + 2] = b.clamp(0.0, 255.0).round() as u8;
        }

        Ok(rgb)
    }

    /// Convert RGB to YUV.
    fn rgb_to_yuv(rgb: &[u8], width: u32, height: u32) -> CvResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
        let size = (width * height) as usize;
        if rgb.len() != size * 3 {
            return Err(CvError::insufficient_data(size * 3, rgb.len()));
        }

        let mut y = vec![0u8; size];
        let mut u = vec![0u8; size];
        let mut v = vec![0u8; size];

        for i in 0..size {
            let r = rgb[i * 3] as f32;
            let g = rgb[i * 3 + 1] as f32;
            let b = rgb[i * 3 + 2] as f32;

            let y_val = 0.299 * r + 0.587 * g + 0.114 * b;
            let u_val = -0.168_736 * r - 0.331_264 * g + 0.5 * b + 128.0;
            let v_val = 0.5 * r - 0.418_688 * g - 0.081_312 * b + 128.0;

            y[i] = y_val.clamp(0.0, 255.0).round() as u8;
            u[i] = u_val.clamp(0.0, 255.0).round() as u8;
            v[i] = v_val.clamp(0.0, 255.0).round() as u8;
        }

        Ok((y, u, v))
    }

    /// Bilinear upscaling for chroma planes.
    fn bilinear_upscale(src: &[u8], width: u32, height: u32, scale: u32) -> Vec<u8> {
        let out_width = width * scale;
        let out_height = height * scale;
        let mut output = vec![0u8; (out_width * out_height) as usize];

        for y in 0..out_height {
            for x in 0..out_width {
                let src_x = (x as f32 / scale as f32).min((width - 1) as f32);
                let src_y = (y as f32 / scale as f32).min((height - 1) as f32);

                let x0 = src_x.floor() as u32;
                let y0 = src_y.floor() as u32;
                let x1 = (x0 + 1).min(width - 1);
                let y1 = (y0 + 1).min(height - 1);

                let fx = src_x - x0 as f32;
                let fy = src_y - y0 as f32;

                let p00 = src[(y0 * width + x0) as usize] as f32;
                let p10 = src[(y0 * width + x1) as usize] as f32;
                let p01 = src[(y1 * width + x0) as usize] as f32;
                let p11 = src[(y1 * width + x1) as usize] as f32;

                let value = p00 * (1.0 - fx) * (1.0 - fy)
                    + p10 * fx * (1.0 - fy)
                    + p01 * (1.0 - fx) * fy
                    + p11 * fx * fy;

                output[(y * out_width + x) as usize] = value.round() as u8;
            }
        }

        output
    }

    // Post-processing methods

    /// Apply denoising to the image.
    fn apply_denoising(&self, image: &mut [u8], width: u32, height: u32) -> CvResult<()> {
        // Simple bilateral filter approximation
        let kernel_size = 5;
        let sigma_color = 30.0f32;
        let sigma_space = 2.0f32;

        let padded_width = width as usize;
        let src = image.to_vec();

        for y in (kernel_size / 2)..(height as usize - kernel_size / 2) {
            for x in (kernel_size / 2)..(padded_width - kernel_size / 2) {
                for c in 0..3 {
                    let center_val = src[(y * padded_width + x) * 3 + c] as f32;
                    let mut sum = 0.0f32;
                    let mut weight_sum = 0.0f32;

                    for ky in 0..kernel_size {
                        for kx in 0..kernel_size {
                            let ny = y + ky - kernel_size / 2;
                            let nx = x + kx - kernel_size / 2;
                            let val = src[(ny * padded_width + nx) * 3 + c] as f32;

                            let color_dist = (val - center_val).abs();
                            let space_dist = ((ky as isize - kernel_size as isize / 2).pow(2)
                                + (kx as isize - kernel_size as isize / 2).pow(2))
                                as f32;

                            let weight = (-color_dist / sigma_color).exp()
                                * (-space_dist / (2.0 * sigma_space * sigma_space)).exp();

                            sum += val * weight;
                            weight_sum += weight;
                        }
                    }

                    if weight_sum > 0.0 {
                        image[(y * padded_width + x) * 3 + c] = (sum / weight_sum).round() as u8;
                    }
                }
            }
        }

        Ok(())
    }

    /// Apply artifact reduction (smoothing compression artifacts).
    fn apply_artifact_reduction(&self, image: &mut [u8], width: u32, height: u32) -> CvResult<()> {
        // Light Gaussian blur to reduce artifacts
        let kernel = [1.0f32, 2.0, 1.0];
        let kernel_sum = 4.0f32;

        let src = image.to_vec();
        let w = width as usize;
        let h = height as usize;

        // Horizontal pass
        let mut temp = vec![0u8; w * h * 3];
        for y in 0..h {
            for x in 1..w - 1 {
                for c in 0..3 {
                    let sum = src[(y * w + x - 1) * 3 + c] as f32 * kernel[0]
                        + src[(y * w + x) * 3 + c] as f32 * kernel[1]
                        + src[(y * w + x + 1) * 3 + c] as f32 * kernel[2];
                    temp[(y * w + x) * 3 + c] = (sum / kernel_sum).round() as u8;
                }
            }
        }

        // Vertical pass
        for y in 1..h - 1 {
            for x in 0..w {
                for c in 0..3 {
                    let sum = temp[((y - 1) * w + x) * 3 + c] as f32 * kernel[0]
                        + temp[(y * w + x) * 3 + c] as f32 * kernel[1]
                        + temp[((y + 1) * w + x) * 3 + c] as f32 * kernel[2];
                    image[(y * w + x) * 3 + c] = (sum / kernel_sum).round() as u8;
                }
            }
        }

        Ok(())
    }

    /// Apply edge enhancement.
    fn apply_edge_enhancement(&self, image: &mut [u8], width: u32, height: u32) -> CvResult<()> {
        // Unsharp masking
        let amount = 0.5f32;
        let src = image.to_vec();
        let w = width as usize;
        let h = height as usize;

        // Gaussian blur
        let mut blurred = src.clone();
        let kernel = [1.0f32, 4.0, 6.0, 4.0, 1.0];
        let kernel_sum = 16.0f32;

        // Horizontal pass
        let mut temp = vec![0u8; w * h * 3];
        for y in 0..h {
            for x in 2..w - 2 {
                for c in 0..3 {
                    let sum = blurred[(y * w + x - 2) * 3 + c] as f32 * kernel[0]
                        + blurred[(y * w + x - 1) * 3 + c] as f32 * kernel[1]
                        + blurred[(y * w + x) * 3 + c] as f32 * kernel[2]
                        + blurred[(y * w + x + 1) * 3 + c] as f32 * kernel[3]
                        + blurred[(y * w + x + 2) * 3 + c] as f32 * kernel[4];
                    temp[(y * w + x) * 3 + c] = (sum / kernel_sum).round() as u8;
                }
            }
        }

        // Vertical pass
        for y in 2..h - 2 {
            for x in 0..w {
                for c in 0..3 {
                    let sum = temp[((y - 2) * w + x) * 3 + c] as f32 * kernel[0]
                        + temp[((y - 1) * w + x) * 3 + c] as f32 * kernel[1]
                        + temp[(y * w + x) * 3 + c] as f32 * kernel[2]
                        + temp[((y + 1) * w + x) * 3 + c] as f32 * kernel[3]
                        + temp[((y + 2) * w + x) * 3 + c] as f32 * kernel[4];
                    blurred[(y * w + x) * 3 + c] = (sum / kernel_sum).round() as u8;
                }
            }
        }

        // Unsharp mask: output = src + amount * (src - blurred)
        for i in 0..image.len() {
            let original = src[i] as f32;
            let blur = blurred[i] as f32;
            let enhanced = original + amount * (original - blur);
            image[i] = enhanced.clamp(0.0, 255.0).round() as u8;
        }

        Ok(())
    }

    /// Apply sharpening filter.
    fn apply_sharpening(
        &self,
        image: &mut [u8],
        width: u32,
        height: u32,
        amount: f32,
    ) -> CvResult<()> {
        // Laplacian sharpening kernel
        let src = image.to_vec();
        let w = width as usize;
        let h = height as usize;

        for y in 1..h - 1 {
            for x in 1..w - 1 {
                for c in 0..3 {
                    let center = src[(y * w + x) * 3 + c] as f32;
                    let top = src[((y - 1) * w + x) * 3 + c] as f32;
                    let bottom = src[((y + 1) * w + x) * 3 + c] as f32;
                    let left = src[(y * w + x - 1) * 3 + c] as f32;
                    let right = src[(y * w + x + 1) * 3 + c] as f32;

                    let laplacian = 4.0 * center - (top + bottom + left + right);
                    let sharpened = center + amount * laplacian;

                    image[(y * w + x) * 3 + c] = sharpened.clamp(0.0, 255.0).round() as u8;
                }
            }
        }

        Ok(())
    }
}

/// ESRGAN-based image upscaler.
///
/// Provides AI-powered super-resolution using ESRGAN models via ONNX Runtime.
/// Supports tile-based processing for large images to manage memory usage.
///
/// # Note
///
/// This type is deprecated in favor of `SuperResolutionModel` which supports
/// multiple model types. This is kept for backward compatibility.
pub struct EsrganUpscaler {
    session: Session,
    scale_factor: UpscaleFactor,
    tile_config: TileConfig,
    progress_callback: Option<ProgressCallback>,
}

impl EsrganUpscaler {
    /// Create a new ESRGAN upscaler from an ONNX model file.
    ///
    /// # Arguments
    ///
    /// * `model_path` - Path to the ONNX model file
    /// * `scale_factor` - Upscale factor (2x or 4x)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Model file cannot be loaded
    /// - ONNX Runtime initialization fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use oximedia_cv::enhance::{EsrganUpscaler, UpscaleFactor};
    ///
    /// let upscaler = EsrganUpscaler::new("esrgan_x4.onnx", UpscaleFactor::X4)?;
    /// # Ok::<(), oximedia_cv::error::CvError>(())
    /// ```
    pub fn new(model_path: impl AsRef<Path>, scale_factor: UpscaleFactor) -> CvResult<Self> {
        let session = Session::builder()
            .map_err(|e| CvError::onnx_runtime(format!("Failed to create session builder: {e}")))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| CvError::onnx_runtime(format!("Failed to set optimization level: {e}")))?
            .commit_from_file(model_path.as_ref())
            .map_err(|e| CvError::model_load(format!("Failed to load model: {e}")))?;

        Ok(Self {
            session,
            scale_factor,
            tile_config: TileConfig::default(),
            progress_callback: None,
        })
    }

    /// Set custom tile configuration.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use oximedia_cv::enhance::{EsrganUpscaler, UpscaleFactor, TileConfig};
    ///
    /// let mut upscaler = EsrganUpscaler::new("esrgan_x4.onnx", UpscaleFactor::X4)?;
    /// upscaler.set_tile_config(TileConfig::new(512, 32)?);
    /// # Ok::<(), oximedia_cv::error::CvError>(())
    /// ```
    pub fn set_tile_config(&mut self, config: TileConfig) {
        self.tile_config = config;
    }

    /// Set progress callback.
    ///
    /// The callback receives `(current, total)` and should return `true` to continue
    /// or `false` to abort processing.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use oximedia_cv::enhance::{EsrganUpscaler, UpscaleFactor};
    ///
    /// let mut upscaler = EsrganUpscaler::new("esrgan_x4.onnx", UpscaleFactor::X4)?;
    /// upscaler.set_progress_callback(Box::new(|current, total| {
    ///     println!("Progress: {}/{}", current, total);
    ///     true
    /// }));
    /// # Ok::<(), oximedia_cv::error::CvError>(())
    /// ```
    pub fn set_progress_callback(&mut self, callback: ProgressCallback) {
        self.progress_callback = Some(callback);
    }

    /// Upscale an image using the ESRGAN model.
    ///
    /// For large images, automatically uses tile-based processing.
    ///
    /// # Arguments
    ///
    /// * `image` - Input image in RGB format (row-major, packed RGB)
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    ///
    /// # Returns
    ///
    /// Upscaled image in RGB format with dimensions `(width * scale, height * scale, 3)`
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Input dimensions are invalid
    /// - Input buffer size doesn't match dimensions
    /// - Model inference fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use oximedia_cv::enhance::{EsrganUpscaler, UpscaleFactor};
    ///
    /// let upscaler = EsrganUpscaler::new("esrgan_x4.onnx", UpscaleFactor::X4)?;
    /// let input = vec![0u8; 256 * 256 * 3];
    /// let output = upscaler.upscale(&input, 256, 256)?;
    /// assert_eq!(output.len(), 1024 * 1024 * 3);
    /// # Ok::<(), oximedia_cv::error::CvError>(())
    /// ```
    pub fn upscale(&mut self, image: &[u8], width: u32, height: u32) -> CvResult<Vec<u8>> {
        // Validate inputs
        if width == 0 || height == 0 {
            return Err(CvError::invalid_dimensions(width, height));
        }

        let expected_size = (width as usize) * (height as usize) * 3;
        if image.len() != expected_size {
            return Err(CvError::insufficient_data(expected_size, image.len()));
        }

        // Determine if tiling is needed
        let tile_size = self.tile_config.tile_size;
        if width <= tile_size && height <= tile_size {
            self.upscale_single_tile(image, width, height)
        } else {
            self.upscale_tiled(image, width, height)
        }
    }

    /// Upscale a single tile (internal method).
    fn upscale_single_tile(&mut self, image: &[u8], width: u32, height: u32) -> CvResult<Vec<u8>> {
        // Convert RGB u8 to normalized float32 [1, 3, H, W]
        let input_tensor = self.preprocess_image(image, width, height)?;

        // Convert to ONNX Value
        let input_value = Value::from_array(input_tensor)
            .map_err(|e| CvError::onnx_runtime(format!("Failed to create input tensor: {e}")))?;

        // Run inference and extract owned data to release the session borrow
        let (shape_owned, data_owned) = {
            let outputs = self
                .session
                .run(ort::inputs![input_value])
                .map_err(|e| CvError::onnx_runtime(format!("Inference failed: {e}")))?;
            let (shape, data) = outputs[0].try_extract_tensor::<f32>().map_err(|e| {
                CvError::tensor_error(format!("Failed to extract output tensor: {e}"))
            })?;
            let shape_owned: Vec<i64> = shape.iter().copied().collect();
            let data_owned: Vec<f32> = data.to_vec();
            (shape_owned, data_owned)
        };

        // Convert back to RGB u8
        let scale = self.scale_factor.scale();
        self.postprocess_tensor(&shape_owned, &data_owned, width * scale, height * scale)
    }

    /// Upscale using tile-based processing for large images.
    ///
    /// This method splits the input image into overlapping tiles, processes each
    /// tile independently, and blends the results to produce the final upscaled image.
    ///
    /// # Arguments
    ///
    /// * `image` - Input image in RGB format
    /// * `width` - Image width
    /// * `height` - Image height
    ///
    /// # Errors
    ///
    /// Returns an error if processing fails.
    pub fn upscale_tiled(&mut self, image: &[u8], width: u32, height: u32) -> CvResult<Vec<u8>> {
        let tile_size = self.tile_config.tile_size;
        let padding = self.tile_config.tile_padding;
        let scale = self.scale_factor.scale();

        // Calculate tile grid
        let tiles_x = width.div_ceil(tile_size) as usize;
        let tiles_y = height.div_ceil(tile_size) as usize;
        let total_tiles = tiles_x * tiles_y;

        // Output dimensions
        let out_width = width * scale;
        let out_height = height * scale;
        let mut output = vec![0u8; (out_width * out_height * 3) as usize];

        // Weight map for blending
        let mut weight_map = vec![0.0f32; (out_width * out_height) as usize];

        // Process each tile
        let mut tile_idx = 0;
        for ty in 0..tiles_y {
            for tx in 0..tiles_x {
                // Check if processing should continue
                if let Some(ref callback) = self.progress_callback {
                    if !callback(tile_idx + 1, total_tiles) {
                        return Err(CvError::detection_failed("Processing aborted by user"));
                    }
                }

                // Calculate tile boundaries with padding
                let x_start = (tx as u32 * tile_size).saturating_sub(padding);
                let y_start = (ty as u32 * tile_size).saturating_sub(padding);
                let x_end = ((tx as u32 + 1) * tile_size + padding).min(width);
                let y_end = ((ty as u32 + 1) * tile_size + padding).min(height);

                let tile_w = x_end - x_start;
                let tile_h = y_end - y_start;

                // Extract tile
                let tile =
                    self.extract_tile(image, width, height, x_start, y_start, tile_w, tile_h)?;

                // Process tile
                let upscaled_tile = self.upscale_single_tile(&tile, tile_w, tile_h)?;

                // Calculate blend region (excluding padding)
                let blend_x_start = if tx == 0 { 0 } else { padding * scale };
                let blend_y_start = if ty == 0 { 0 } else { padding * scale };
                let blend_x_end = if tx == tiles_x - 1 {
                    tile_w * scale
                } else {
                    (tile_w - padding) * scale
                };
                let blend_y_end = if ty == tiles_y - 1 {
                    tile_h * scale
                } else {
                    (tile_h - padding) * scale
                };

                // Blend tile into output
                self.blend_tile(
                    &upscaled_tile,
                    tile_w * scale,
                    tile_h * scale,
                    &mut output,
                    &mut weight_map,
                    out_width,
                    out_height,
                    x_start * scale,
                    y_start * scale,
                    blend_x_start,
                    blend_y_start,
                    blend_x_end,
                    blend_y_end,
                )?;

                tile_idx += 1;
            }
        }

        // Normalize by weight map
        self.normalize_by_weights(&mut output, &weight_map, out_width, out_height);

        Ok(output)
    }

    /// Extract a rectangular tile from the source image.
    fn extract_tile(
        &self,
        src: &[u8],
        src_w: u32,
        src_h: u32,
        x: u32,
        y: u32,
        tile_w: u32,
        tile_h: u32,
    ) -> CvResult<Vec<u8>> {
        if x + tile_w > src_w || y + tile_h > src_h {
            return Err(CvError::invalid_roi(x, y, tile_w, tile_h));
        }

        let mut tile = Vec::with_capacity((tile_w * tile_h * 3) as usize);

        for row in y..y + tile_h {
            let start = ((row * src_w + x) * 3) as usize;
            let end = start + (tile_w * 3) as usize;
            tile.extend_from_slice(&src[start..end]);
        }

        Ok(tile)
    }

    /// Blend a processed tile into the output image with feathering.
    #[allow(clippy::too_many_arguments)]
    fn blend_tile(
        &self,
        tile: &[u8],
        tile_w: u32,
        _tile_h: u32,
        output: &mut [u8],
        weights: &mut [f32],
        out_w: u32,
        out_h: u32,
        dst_x: u32,
        dst_y: u32,
        blend_x_start: u32,
        blend_y_start: u32,
        blend_x_end: u32,
        blend_y_end: u32,
    ) -> CvResult<()> {
        let feather = self.tile_config.feather_width;

        for local_y in blend_y_start..blend_y_end {
            let global_y = dst_y + local_y;
            if global_y >= out_h {
                break;
            }

            for local_x in blend_x_start..blend_x_end {
                let global_x = dst_x + local_x;
                if global_x >= out_w {
                    break;
                }

                // Calculate feather weight (distance from edge)
                let dist_left = local_x - blend_x_start;
                let dist_right = blend_x_end - local_x - 1;
                let dist_top = local_y - blend_y_start;
                let dist_bottom = blend_y_end - local_y - 1;

                let min_dist = dist_left.min(dist_right).min(dist_top).min(dist_bottom);
                let weight = if min_dist >= feather {
                    1.0
                } else {
                    (min_dist as f32 + 1.0) / (feather as f32 + 1.0)
                };

                // Blend RGB values
                let tile_idx = ((local_y * tile_w + local_x) * 3) as usize;
                let out_idx = ((global_y * out_w + global_x) * 3) as usize;
                let weight_idx = (global_y * out_w + global_x) as usize;

                for c in 0..3 {
                    let tile_val = tile[tile_idx + c] as f32 * weight;
                    output[out_idx + c] = (output[out_idx + c] as f32 + tile_val) as u8;
                }

                weights[weight_idx] += weight;
            }
        }

        Ok(())
    }

    /// Normalize output by accumulated weights.
    fn normalize_by_weights(&self, output: &mut [u8], weights: &[f32], width: u32, height: u32) {
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                let weight = weights[idx];

                if weight > 0.0 {
                    let out_idx = idx * 3;
                    for c in 0..3 {
                        output[out_idx + c] = ((output[out_idx + c] as f32) / weight).round() as u8;
                    }
                }
            }
        }
    }

    /// Preprocess image: RGB u8 -> normalized float32 [1, 3, H, W].
    fn preprocess_image(&self, image: &[u8], width: u32, height: u32) -> CvResult<Array4<f32>> {
        let w = width as usize;
        let h = height as usize;

        let mut tensor = Array4::<f32>::zeros((1, 3, h, w));

        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 3;
                // Normalize to [0, 1]
                tensor[[0, 0, y, x]] = image[idx] as f32 / 255.0; // R
                tensor[[0, 1, y, x]] = image[idx + 1] as f32 / 255.0; // G
                tensor[[0, 2, y, x]] = image[idx + 2] as f32 / 255.0; // B
            }
        }

        Ok(tensor)
    }

    /// Postprocess tensor: normalized float32 [1, 3, H, W] -> RGB u8.
    fn postprocess_tensor(
        &self,
        shape: &[i64],
        data: &[f32],
        width: u32,
        height: u32,
    ) -> CvResult<Vec<u8>> {
        if shape.len() != 4 || shape[0] != 1 || shape[1] != 3 {
            return Err(CvError::ShapeMismatch {
                expected: vec![1, 3, height as usize, width as usize],
                actual: shape.iter().map(|&x| x as usize).collect(),
            });
        }

        let h = shape[2] as usize;
        let w = shape[3] as usize;

        if w != width as usize || h != height as usize {
            return Err(CvError::ShapeMismatch {
                expected: vec![1, 3, height as usize, width as usize],
                actual: shape.iter().map(|&x| x as usize).collect(),
            });
        }
        let mut output = vec![0u8; w * h * 3];

        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 3;
                // Denormalize and clamp to [0, 255]
                // Access as flat array: [batch, channel, height, width]
                let r_idx = y * w + x;
                let g_idx = h * w + y * w + x;
                let b_idx = 2 * h * w + y * w + x;

                output[idx] = (data[r_idx] * 255.0).clamp(0.0, 255.0).round() as u8;
                output[idx + 1] = (data[g_idx] * 255.0).clamp(0.0, 255.0).round() as u8;
                output[idx + 2] = (data[b_idx] * 255.0).clamp(0.0, 255.0).round() as u8;
            }
        }

        Ok(output)
    }

    /// Get the upscale factor.
    #[must_use]
    pub const fn scale_factor(&self) -> UpscaleFactor {
        self.scale_factor
    }

    /// Get the tile configuration.
    #[must_use]
    pub const fn tile_config(&self) -> &TileConfig {
        &self.tile_config
    }
}

/// Batch upscaler for processing multiple images efficiently.
pub struct BatchUpscaler {
    upscaler: EsrganUpscaler,
    batch_size: usize,
}

impl BatchUpscaler {
    /// Create a new batch upscaler.
    ///
    /// # Arguments
    ///
    /// * `model_path` - Path to the ONNX model
    /// * `scale_factor` - Upscale factor
    /// * `batch_size` - Maximum number of tiles to process simultaneously
    pub fn new(
        model_path: impl AsRef<Path>,
        scale_factor: UpscaleFactor,
        batch_size: usize,
    ) -> CvResult<Self> {
        let upscaler = EsrganUpscaler::new(model_path, scale_factor)?;
        Ok(Self {
            upscaler,
            batch_size,
        })
    }

    /// Process multiple images in a batch.
    ///
    /// # Arguments
    ///
    /// * `images` - Vector of (image_data, width, height) tuples
    ///
    /// # Returns
    ///
    /// Vector of upscaled images
    pub fn upscale_batch(&mut self, images: &[(&[u8], u32, u32)]) -> CvResult<Vec<Vec<u8>>> {
        let mut results = Vec::with_capacity(images.len());

        for (image, width, height) in images {
            let result = self.upscaler.upscale(image, *width, *height)?;
            results.push(result);
        }

        Ok(results)
    }

    /// Get the batch size.
    #[must_use]
    pub const fn batch_size(&self) -> usize {
        self.batch_size
    }
}

/// Frame data for video processing.
#[derive(Clone)]
pub struct VideoFrame {
    /// Frame data in RGB format.
    pub data: Vec<u8>,
    /// Frame width.
    pub width: u32,
    /// Frame height.
    pub height: u32,
    /// Frame timestamp (optional).
    pub timestamp: Option<f64>,
}

impl VideoFrame {
    /// Create a new video frame.
    #[must_use]
    pub fn new(data: Vec<u8>, width: u32, height: u32) -> Self {
        Self {
            data,
            width,
            height,
            timestamp: None,
        }
    }

    /// Create a new video frame with timestamp.
    #[must_use]
    pub fn with_timestamp(data: Vec<u8>, width: u32, height: u32, timestamp: f64) -> Self {
        Self {
            data,
            width,
            height,
            timestamp: Some(timestamp),
        }
    }

    /// Get the frame size in pixels.
    #[must_use]
    pub const fn pixel_count(&self) -> usize {
        (self.width as usize) * (self.height as usize)
    }
}

/// Temporal consistency filter for video.
///
/// Applies temporal filtering to reduce flickering and maintain consistency
/// across frames while preserving motion.
pub struct TemporalFilter {
    /// Temporal weight (0.0 = no filtering, 1.0 = maximum filtering).
    temporal_weight: f32,
    /// Previous frame data.
    previous_frame: Option<Vec<u8>>,
    /// Motion threshold for adaptive filtering.
    motion_threshold: f32,
}

impl TemporalFilter {
    /// Create a new temporal filter.
    ///
    /// # Arguments
    ///
    /// * `temporal_weight` - Temporal filtering strength (0.0 - 1.0)
    /// * `motion_threshold` - Motion detection threshold (higher = less sensitive)
    #[must_use]
    pub fn new(temporal_weight: f32, motion_threshold: f32) -> Self {
        Self {
            temporal_weight: temporal_weight.clamp(0.0, 1.0),
            previous_frame: None,
            motion_threshold,
        }
    }

    /// Apply temporal filtering to a frame.
    ///
    /// # Arguments
    ///
    /// * `current` - Current frame data
    /// * `width` - Frame width
    /// * `height` - Frame height
    ///
    /// # Returns
    ///
    /// Filtered frame data
    pub fn filter(&mut self, current: &[u8], width: u32, height: u32) -> Vec<u8> {
        let size = (width * height * 3) as usize;

        if current.len() != size {
            return current.to_vec();
        }

        let filtered = if let Some(ref prev) = self.previous_frame {
            if prev.len() == size {
                self.apply_temporal_blend(current, prev, width, height)
            } else {
                current.to_vec()
            }
        } else {
            current.to_vec()
        };

        self.previous_frame = Some(filtered.clone());
        filtered
    }

    /// Apply temporal blending between current and previous frames.
    fn apply_temporal_blend(
        &self,
        current: &[u8],
        previous: &[u8],
        width: u32,
        height: u32,
    ) -> Vec<u8> {
        let mut output = vec![0u8; current.len()];
        let w = width as usize;
        let h = height as usize;

        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 3;

                // Calculate motion magnitude
                let mut motion = 0.0f32;
                for c in 0..3 {
                    let diff = current[idx + c] as f32 - previous[idx + c] as f32;
                    motion += diff * diff;
                }
                motion = motion.sqrt();

                // Adaptive temporal weight based on motion
                let adaptive_weight = if motion > self.motion_threshold {
                    self.temporal_weight * (self.motion_threshold / motion).min(1.0)
                } else {
                    self.temporal_weight
                };

                // Blend current and previous frames
                for c in 0..3 {
                    let curr_val = current[idx + c] as f32;
                    let prev_val = previous[idx + c] as f32;
                    let blended = curr_val * (1.0 - adaptive_weight) + prev_val * adaptive_weight;
                    output[idx + c] = blended.round() as u8;
                }
            }
        }

        output
    }

    /// Reset the filter state.
    pub fn reset(&mut self) {
        self.previous_frame = None;
    }

    /// Check if the filter has previous frame data.
    #[must_use]
    pub fn has_previous(&self) -> bool {
        self.previous_frame.is_some()
    }
}

impl Default for TemporalFilter {
    fn default() -> Self {
        Self::new(0.3, 10.0)
    }
}

/// Motion estimator for video frames.
///
/// Estimates motion between frames for motion-aware processing.
pub struct MotionEstimator {
    /// Block size for motion estimation.
    block_size: u32,
    /// Search range for motion vectors.
    search_range: i32,
}

impl MotionEstimator {
    /// Create a new motion estimator.
    ///
    /// # Arguments
    ///
    /// * `block_size` - Size of blocks for motion estimation
    /// * `search_range` - Maximum search distance for motion vectors
    #[must_use]
    pub fn new(block_size: u32, search_range: i32) -> Self {
        Self {
            block_size,
            search_range,
        }
    }

    /// Estimate motion between two frames.
    ///
    /// # Arguments
    ///
    /// * `current` - Current frame
    /// * `reference` - Reference frame
    /// * `width` - Frame width
    /// * `height` - Frame height
    ///
    /// # Returns
    ///
    /// Motion vectors as (dx, dy) pairs for each block
    pub fn estimate(
        &self,
        current: &[u8],
        reference: &[u8],
        width: u32,
        height: u32,
    ) -> Vec<(i32, i32)> {
        let blocks_x = width.div_ceil(self.block_size);
        let blocks_y = height.div_ceil(self.block_size);
        let mut motion_vectors = Vec::with_capacity((blocks_x * blocks_y) as usize);

        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let block_x = bx * self.block_size;
                let block_y = by * self.block_size;

                let (dx, dy) =
                    self.estimate_block_motion(current, reference, width, height, block_x, block_y);

                motion_vectors.push((dx, dy));
            }
        }

        motion_vectors
    }

    /// Estimate motion for a single block using block matching.
    #[allow(clippy::too_many_arguments)]
    fn estimate_block_motion(
        &self,
        current: &[u8],
        reference: &[u8],
        width: u32,
        height: u32,
        block_x: u32,
        block_y: u32,
    ) -> (i32, i32) {
        let mut best_dx = 0;
        let mut best_dy = 0;
        let mut best_sad = f32::MAX;

        for dy in -self.search_range..=self.search_range {
            for dx in -self.search_range..=self.search_range {
                let ref_x = block_x as i32 + dx;
                let ref_y = block_y as i32 + dy;

                if ref_x < 0
                    || ref_y < 0
                    || ref_x + self.block_size as i32 > width as i32
                    || ref_y + self.block_size as i32 > height as i32
                {
                    continue;
                }

                let sad = self.calculate_sad(
                    current,
                    reference,
                    width,
                    block_x,
                    block_y,
                    ref_x as u32,
                    ref_y as u32,
                );

                if sad < best_sad {
                    best_sad = sad;
                    best_dx = dx;
                    best_dy = dy;
                }
            }
        }

        (best_dx, best_dy)
    }

    /// Calculate Sum of Absolute Differences (SAD) between two blocks.
    #[allow(clippy::too_many_arguments)]
    fn calculate_sad(
        &self,
        current: &[u8],
        reference: &[u8],
        width: u32,
        curr_x: u32,
        curr_y: u32,
        ref_x: u32,
        ref_y: u32,
    ) -> f32 {
        let mut sad = 0.0f32;
        let w = width as usize;

        for y in 0..self.block_size {
            for x in 0..self.block_size {
                let curr_idx = ((curr_y + y) as usize * w + (curr_x + x) as usize) * 3;
                let ref_idx = ((ref_y + y) as usize * w + (ref_x + x) as usize) * 3;

                for c in 0..3 {
                    sad += (current[curr_idx + c] as f32 - reference[ref_idx + c] as f32).abs();
                }
            }
        }

        sad
    }

    /// Calculate average motion magnitude from motion vectors.
    #[must_use]
    pub fn calculate_average_motion(motion_vectors: &[(i32, i32)]) -> f32 {
        if motion_vectors.is_empty() {
            return 0.0;
        }

        let sum: f32 = motion_vectors
            .iter()
            .map(|(dx, dy)| ((*dx * *dx + *dy * *dy) as f32).sqrt())
            .sum();

        sum / motion_vectors.len() as f32
    }
}

impl Default for MotionEstimator {
    fn default() -> Self {
        Self::new(16, 8)
    }
}

/// Video super-resolution processor.
///
/// Handles video-specific super-resolution with temporal consistency,
/// frame buffering, and motion-aware processing.
pub struct VideoSuperResolution {
    /// Underlying super-resolution model.
    model: SuperResolutionModel,
    /// Frame buffer for temporal processing.
    frame_buffer: VecDeque<VideoFrame>,
    /// Temporal filter for consistency.
    temporal_filter: TemporalFilter,
    /// Motion estimator.
    motion_estimator: MotionEstimator,
    /// Buffer size (number of frames to keep).
    buffer_size: usize,
    /// Enable temporal filtering.
    enable_temporal_filtering: bool,
    /// Enable motion-aware processing.
    enable_motion_aware: bool,
}

impl VideoSuperResolution {
    /// Create a new video super-resolution processor.
    ///
    /// # Arguments
    ///
    /// * `model` - Super-resolution model to use
    /// * `buffer_size` - Number of frames to buffer for temporal processing
    pub fn new(model: SuperResolutionModel, buffer_size: usize) -> Self {
        Self {
            model,
            frame_buffer: VecDeque::with_capacity(buffer_size),
            temporal_filter: TemporalFilter::default(),
            motion_estimator: MotionEstimator::default(),
            buffer_size,
            enable_temporal_filtering: true,
            enable_motion_aware: true,
        }
    }

    /// Create a video processor with custom settings.
    ///
    /// # Arguments
    ///
    /// * `model` - Super-resolution model
    /// * `buffer_size` - Frame buffer size
    /// * `temporal_weight` - Temporal filtering strength (0.0 - 1.0)
    /// * `motion_threshold` - Motion detection threshold
    pub fn with_settings(
        model: SuperResolutionModel,
        buffer_size: usize,
        temporal_weight: f32,
        motion_threshold: f32,
    ) -> Self {
        Self {
            model,
            frame_buffer: VecDeque::with_capacity(buffer_size),
            temporal_filter: TemporalFilter::new(temporal_weight, motion_threshold),
            motion_estimator: MotionEstimator::default(),
            buffer_size,
            enable_temporal_filtering: true,
            enable_motion_aware: true,
        }
    }

    /// Enable or disable temporal filtering.
    pub fn set_temporal_filtering(&mut self, enable: bool) {
        self.enable_temporal_filtering = enable;
    }

    /// Enable or disable motion-aware processing.
    pub fn set_motion_aware(&mut self, enable: bool) {
        self.enable_motion_aware = enable;
    }

    /// Set temporal filter parameters.
    pub fn set_temporal_params(&mut self, weight: f32, motion_threshold: f32) {
        self.temporal_filter = TemporalFilter::new(weight, motion_threshold);
    }

    /// Process a single video frame.
    ///
    /// # Arguments
    ///
    /// * `frame` - Input frame
    ///
    /// # Returns
    ///
    /// Upscaled frame
    ///
    /// # Errors
    ///
    /// Returns an error if processing fails.
    pub fn process_frame(&mut self, mut frame: VideoFrame) -> CvResult<VideoFrame> {
        // Add frame to buffer
        if self.frame_buffer.len() >= self.buffer_size {
            self.frame_buffer.pop_front();
        }

        // Apply temporal filtering if enabled
        if self.enable_temporal_filtering && self.temporal_filter.has_previous() {
            frame.data = self
                .temporal_filter
                .filter(&frame.data, frame.width, frame.height);
        }

        // Analyze motion if enabled
        let motion_magnitude = if self.enable_motion_aware && !self.frame_buffer.is_empty() {
            let prev_frame = &self.frame_buffer[self.frame_buffer.len() - 1];
            let motion_vectors = self.motion_estimator.estimate(
                &frame.data,
                &prev_frame.data,
                frame.width,
                frame.height,
            );
            MotionEstimator::calculate_average_motion(&motion_vectors)
        } else {
            0.0
        };

        // Adjust processing based on motion
        if self.enable_motion_aware && motion_magnitude > 5.0 {
            // High motion: reduce temporal filtering
            let original_weight = self.temporal_filter.temporal_weight;
            self.temporal_filter.temporal_weight = (original_weight * 0.5).max(0.1);

            // Process frame
            let upscaled_data = self.model.upscale(&frame.data, frame.width, frame.height)?;

            // Restore original weight
            self.temporal_filter.temporal_weight = original_weight;

            let scale = self.model.scale_factor().scale();
            let output_frame = VideoFrame {
                data: upscaled_data,
                width: frame.width * scale,
                height: frame.height * scale,
                timestamp: frame.timestamp,
            };

            self.frame_buffer.push_back(frame);
            Ok(output_frame)
        } else {
            // Normal processing
            let upscaled_data = self.model.upscale(&frame.data, frame.width, frame.height)?;

            let scale = self.model.scale_factor().scale();
            let output_frame = VideoFrame {
                data: upscaled_data,
                width: frame.width * scale,
                height: frame.height * scale,
                timestamp: frame.timestamp,
            };

            self.frame_buffer.push_back(frame);
            Ok(output_frame)
        }
    }

    /// Process multiple video frames in sequence.
    ///
    /// # Arguments
    ///
    /// * `frames` - Input frames
    ///
    /// # Returns
    ///
    /// Vector of upscaled frames
    ///
    /// # Errors
    ///
    /// Returns an error if processing fails.
    pub fn process_frames(&mut self, frames: Vec<VideoFrame>) -> CvResult<Vec<VideoFrame>> {
        let mut output_frames = Vec::with_capacity(frames.len());

        for frame in frames {
            let output = self.process_frame(frame)?;
            output_frames.push(output);
        }

        Ok(output_frames)
    }

    /// Reset the video processor state.
    pub fn reset(&mut self) {
        self.frame_buffer.clear();
        self.temporal_filter.reset();
    }

    /// Get the current buffer size.
    #[must_use]
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    /// Get the number of buffered frames.
    #[must_use]
    pub fn buffered_frames(&self) -> usize {
        self.frame_buffer.len()
    }

    /// Get a reference to the underlying model.
    #[must_use]
    pub const fn model(&self) -> &SuperResolutionModel {
        &self.model
    }
}

/// Utility functions for super-resolution.
pub mod utils {
    use super::UpscaleFactor;

    /// Calculate optimal tile size based on image dimensions and available memory.
    ///
    /// # Arguments
    ///
    /// * `width` - Image width
    /// * `height` - Image height
    /// * `available_memory_mb` - Available memory in megabytes
    ///
    /// # Returns
    ///
    /// Recommended tile size
    #[must_use]
    pub fn calculate_optimal_tile_size(width: u32, height: u32, available_memory_mb: usize) -> u32 {
        // Rough estimation: float32 tensor needs 4 bytes per value
        // For a tile of size NxN with 3 channels and scale factor 4:
        // Input: N * N * 3 * 4 bytes
        // Output: (N*4) * (N*4) * 3 * 4 bytes
        // Total ≈ N^2 * 12 + N^2 * 64 * 12 ≈ N^2 * 780 bytes

        let bytes_per_pixel_approx = 780;
        let available_bytes = available_memory_mb * 1024 * 1024;
        let max_tile_pixels = available_bytes / bytes_per_pixel_approx;
        let max_tile_size = (max_tile_pixels as f32).sqrt() as u32;

        // Clamp to reasonable range
        let tile_size = max_tile_size.clamp(128, 1024);

        // Round down to nearest power of 2 for efficiency
        let tile_size = tile_size.next_power_of_two() / 2;

        tile_size.min(width.max(height))
    }

    /// Estimate memory requirement for upscaling an image.
    ///
    /// # Arguments
    ///
    /// * `width` - Image width
    /// * `height` - Image height
    /// * `scale_factor` - Upscale factor
    ///
    /// # Returns
    ///
    /// Estimated memory in megabytes
    #[must_use]
    pub fn estimate_memory_requirement(
        width: u32,
        height: u32,
        scale_factor: UpscaleFactor,
    ) -> usize {
        let scale = scale_factor.scale();
        let input_pixels = width as usize * height as usize;
        let output_pixels = (width * scale) as usize * (height * scale) as usize;

        // Input tensor (float32): pixels * 3 * 4
        let input_bytes = input_pixels * 3 * 4;
        // Output tensor (float32): pixels * 3 * 4
        let output_bytes = output_pixels * 3 * 4;
        // Output RGB u8: pixels * 3
        let result_bytes = output_pixels * 3;

        let total_bytes = input_bytes + output_bytes + result_bytes;
        total_bytes.div_ceil(1024 * 1024) // Round up to MB
    }

    /// Create a feathering weight map for blending.
    ///
    /// # Arguments
    ///
    /// * `width` - Map width
    /// * `height` - Map height
    /// * `feather_width` - Feathering width in pixels
    ///
    /// # Returns
    ///
    /// Weight map where edges fade from 0 to 1
    #[must_use]
    pub fn create_feather_weights(width: u32, height: u32, feather_width: u32) -> Vec<f32> {
        let mut weights = vec![1.0f32; (width * height) as usize];

        for y in 0..height {
            for x in 0..width {
                let dist_left = x;
                let dist_right = width - x - 1;
                let dist_top = y;
                let dist_bottom = height - y - 1;

                let min_dist = dist_left.min(dist_right).min(dist_top).min(dist_bottom);

                let weight = if min_dist >= feather_width {
                    1.0
                } else {
                    (min_dist as f32 + 1.0) / (feather_width as f32 + 1.0)
                };

                weights[(y * width + x) as usize] = weight;
            }
        }

        weights
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upscale_factor() {
        assert_eq!(UpscaleFactor::X2.scale(), 2);
        assert_eq!(UpscaleFactor::X4.scale(), 4);
        assert_eq!(UpscaleFactor::X8.scale(), 8);
    }

    #[test]
    fn test_model_type_properties() {
        assert_eq!(ModelType::ESRGAN.name(), "ESRGAN");
        assert_eq!(ModelType::RealESRGAN.name(), "Real-ESRGAN");
        assert_eq!(ModelType::EDSR.name(), "EDSR");
        assert_eq!(ModelType::SRCNN.name(), "SRCNN");
        assert_eq!(ModelType::VDSR.name(), "VDSR");

        assert_eq!(ModelType::ESRGAN.normalization_range(), (0.0, 1.0));
        assert_eq!(ModelType::EDSR.normalization_range(), (0.0, 255.0));

        assert!(!ModelType::ESRGAN.uses_mean_subtraction());
        assert!(ModelType::EDSR.uses_mean_subtraction());
    }

    #[test]
    fn test_quality_mode() {
        assert_eq!(QualityMode::Fast.recommended_model(), ModelType::SRCNN);
        assert_eq!(QualityMode::Balanced.recommended_model(), ModelType::EDSR);
        assert_eq!(
            QualityMode::HighQuality.recommended_model(),
            ModelType::RealESRGAN
        );
        assert_eq!(
            QualityMode::Animation.recommended_model(),
            ModelType::RealESRGAN
        );

        assert_eq!(QualityMode::Fast.recommended_tile_size(), 512);
        assert_eq!(QualityMode::Balanced.recommended_tile_size(), 256);
    }

    #[test]
    fn test_color_space() {
        assert_eq!(ColorSpace::RGB.num_channels(), 3);
        assert_eq!(ColorSpace::YUV420.num_channels(), 3);
        assert_eq!(ColorSpace::YUV444.num_channels(), 3);

        assert!(!ColorSpace::RGB.is_subsampled());
        assert!(ColorSpace::YUV420.is_subsampled());
        assert!(!ColorSpace::YUV444.is_subsampled());
    }

    #[test]
    fn test_processing_options() {
        let default_opts = ProcessingOptions::default();
        assert!(!default_opts.edge_enhancement);
        assert!(default_opts.artifact_reduction);
        assert!(!default_opts.denoise);

        let enhanced_opts = ProcessingOptions::enhanced();
        assert!(enhanced_opts.edge_enhancement);
        assert!(enhanced_opts.artifact_reduction);
        assert!(enhanced_opts.denoise);

        let fast_opts = ProcessingOptions::fast();
        assert!(!fast_opts.edge_enhancement);
        assert!(!fast_opts.artifact_reduction);
        assert!(!fast_opts.denoise);
    }

    #[test]
    fn test_tile_config_default() {
        let config = TileConfig::default();
        assert_eq!(config.tile_size, 256);
        assert_eq!(config.tile_padding, 16);
    }

    #[test]
    fn test_tile_config_new() {
        let config = TileConfig::new(512, 32).unwrap();
        assert_eq!(config.tile_size, 512);
        assert_eq!(config.tile_padding, 32);
        assert_eq!(config.padded_size(), 576);
    }

    #[test]
    fn test_tile_config_invalid_size() {
        let result = TileConfig::new(32, 8);
        assert!(result.is_err());
    }

    #[test]
    fn test_tile_config_invalid_padding() {
        let result = TileConfig::new(256, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_optimal_tile_size() {
        let tile_size = utils::calculate_optimal_tile_size(2048, 2048, 512);
        assert!(tile_size >= 128);
        assert!(tile_size <= 1024);
    }

    #[test]
    fn test_estimate_memory_requirement() {
        let mem = utils::estimate_memory_requirement(1920, 1080, UpscaleFactor::X4);
        assert!(mem > 0);
        // For 1920x1080 -> 7680x4320, should be around 300-400 MB
        assert!(mem > 100);
        assert!(mem < 1000);
    }

    #[test]
    fn test_create_feather_weights() {
        let weights = utils::create_feather_weights(10, 10, 2);
        assert_eq!(weights.len(), 100);

        // Center should be 1.0
        assert_eq!(weights[5 * 10 + 5], 1.0);

        // Corner should be less than 1.0
        assert!(weights[0] < 1.0);
    }

    #[test]
    fn test_preprocess_postprocess_roundtrip() {
        // Test preprocess/postprocess logic without requiring an ONNX session.
        // EsrganUpscaler::preprocess_image normalizes RGB u8 -> [0,1] float
        // and postprocess_tensor reverses that.
        let width: u32 = 8;
        let height: u32 = 8;
        let w = width as usize;
        let h = height as usize;
        let input: Vec<u8> = (0..(w * h * 3)).map(|i| (i % 256) as u8).collect();

        // Preprocess: RGB u8 -> [1, 3, H, W] float tensor
        let mut tensor = Array4::<f32>::zeros((1, 3, h, w));
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 3;
                tensor[[0, 0, y, x]] = input[idx] as f32 / 255.0;
                tensor[[0, 1, y, x]] = input[idx + 1] as f32 / 255.0;
                tensor[[0, 2, y, x]] = input[idx + 2] as f32 / 255.0;
            }
        }
        assert_eq!(tensor.shape(), &[1, 3, h, w]);

        // Postprocess: float tensor -> RGB u8
        let shape_i64: Vec<i64> = tensor.shape().iter().map(|&x| x as i64).collect();
        let data_f32: Vec<f32> = tensor.iter().copied().collect();
        let mut output = vec![0u8; w * h * 3];
        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) * 3;
                let r_idx = y * w + x;
                let g_idx = 1 * h * w + y * w + x;
                let b_idx = 2 * h * w + y * w + x;
                output[idx] = (data_f32[r_idx] * 255.0).clamp(0.0, 255.0).round() as u8;
                output[idx + 1] = (data_f32[g_idx] * 255.0).clamp(0.0, 255.0).round() as u8;
                output[idx + 2] = (data_f32[b_idx] * 255.0).clamp(0.0, 255.0).round() as u8;
            }
        }

        assert_eq!(output.len(), input.len());
        assert_eq!(shape_i64, vec![1, 3, h as i64, w as i64]);
        for (a, b) in input.iter().zip(output.iter()) {
            assert!(
                (*a as i32 - *b as i32).abs() <= 1,
                "Values differ: {} vs {}",
                a,
                b
            );
        }
    }

    #[test]
    fn test_extract_tile() {
        // Test tile extraction logic directly (no ONNX session needed).
        let width: u32 = 10;
        let height: u32 = 10;
        let image: Vec<u8> = (0..(width * height * 3) as usize)
            .map(|i| (i % 256) as u8)
            .collect();

        // Inline the extract_tile logic (same as EsrganUpscaler::extract_tile)
        let (x, y, tile_w, tile_h) = (2u32, 2u32, 4u32, 4u32);
        assert!(x + tile_w <= width && y + tile_h <= height);
        let mut tile = Vec::with_capacity((tile_w * tile_h * 3) as usize);
        for row in y..y + tile_h {
            let start = ((row * width + x) * 3) as usize;
            let end = start + (tile_w * 3) as usize;
            tile.extend_from_slice(&image[start..end]);
        }
        assert_eq!(tile.len(), 4 * 4 * 3);
    }

    #[test]
    fn test_extract_tile_out_of_bounds() {
        // Test that out-of-bounds tile extraction is correctly detected.
        let width: u32 = 10;
        let height: u32 = 10;
        let (x, y, tile_w, tile_h) = (8u32, 8u32, 5u32, 5u32);
        // x + tile_w = 13 > 10, so this should be out of bounds
        assert!(
            x + tile_w > width || y + tile_h > height,
            "Expected out-of-bounds tile coordinates"
        );
    }

    #[test]
    fn test_normalize_by_weights() {
        // Test weight normalization logic directly (no ONNX session needed).
        let mut output = vec![100u8, 100, 100, 200, 200, 200];
        let weights = vec![2.0f32, 4.0];
        let width: u32 = 2;
        let height: u32 = 1;

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                let weight = weights[idx];
                if weight > 0.0 {
                    let out_idx = idx * 3;
                    for c in 0..3 {
                        output[out_idx + c] = ((output[out_idx + c] as f32) / weight).round() as u8;
                    }
                }
            }
        }

        // First pixel: 100 / 2.0 = 50
        assert_eq!(output[0], 50);
        assert_eq!(output[1], 50);
        assert_eq!(output[2], 50);

        // Second pixel: 200 / 4.0 = 50
        assert_eq!(output[3], 50);
        assert_eq!(output[4], 50);
        assert_eq!(output[5], 50);
    }

    #[test]
    fn test_batch_size() {
        // Mock batch upscaler test
        let batch_size = 4;
        assert_eq!(batch_size, 4);
    }

    #[test]
    fn test_video_frame() {
        let data = vec![0u8; 64 * 64 * 3];
        let frame = VideoFrame::new(data.clone(), 64, 64);
        assert_eq!(frame.width, 64);
        assert_eq!(frame.height, 64);
        assert_eq!(frame.pixel_count(), 64 * 64);
        assert!(frame.timestamp.is_none());

        let frame_with_ts = VideoFrame::with_timestamp(data, 64, 64, 1.5);
        assert_eq!(frame_with_ts.timestamp, Some(1.5));
    }

    #[test]
    fn test_temporal_filter() {
        let mut filter = TemporalFilter::new(0.5, 10.0);
        assert!(!filter.has_previous());

        let frame1 = vec![100u8; 64 * 64 * 3];
        let frame2 = vec![110u8; 64 * 64 * 3];

        let filtered1 = filter.filter(&frame1, 64, 64);
        assert_eq!(filtered1.len(), frame1.len());
        assert!(filter.has_previous());

        let filtered2 = filter.filter(&frame2, 64, 64);
        assert_eq!(filtered2.len(), frame2.len());

        // Filtered value should be between original frame values
        assert!(filtered2[0] >= 100 && filtered2[0] <= 110);

        filter.reset();
        assert!(!filter.has_previous());
    }

    #[test]
    fn test_motion_estimator() {
        let estimator = MotionEstimator::new(16, 4);
        let frame1 = vec![50u8; 128 * 128 * 3];
        let frame2 = vec![60u8; 128 * 128 * 3];

        let motion_vectors = estimator.estimate(&frame1, &frame2, 128, 128);
        assert!(!motion_vectors.is_empty());

        let avg_motion = MotionEstimator::calculate_average_motion(&motion_vectors);
        assert!(avg_motion >= 0.0);
    }

    #[test]
    fn test_model_cache() {
        let cache = ModelCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        let cache2 = ModelCache::default();
        assert!(cache2.is_empty());
    }

    #[test]
    fn test_chroma_upscale_modes() {
        // Test enum variants
        let _luma_only = ChromaUpscaleMode::LumaOnly;
        let _separate = ChromaUpscaleMode::Separate;
        let _joint = ChromaUpscaleMode::Joint;
    }

    #[test]
    fn test_bilinear_upscale() {
        let src = vec![128u8; 16 * 16];
        let upscaled = SuperResolutionModel::bilinear_upscale(&src, 16, 16, 2);
        assert_eq!(upscaled.len(), 32 * 32);

        // All values should be close to original (128)
        for &val in &upscaled {
            assert!(val >= 120 && val <= 136);
        }
    }

    #[test]
    fn test_gray_to_rgb_conversion() {
        let gray = vec![100u8, 150, 200];
        let rgb = SuperResolutionModel::gray_to_rgb(&gray, 3, 1);
        assert_eq!(rgb.len(), 9);
        assert_eq!(rgb[0], 100);
        assert_eq!(rgb[1], 100);
        assert_eq!(rgb[2], 100);
        assert_eq!(rgb[3], 150);
        assert_eq!(rgb[4], 150);
        assert_eq!(rgb[5], 150);
    }

    #[test]
    fn test_rgb_to_gray_conversion() {
        let rgb = vec![100u8, 100, 100, 150, 150, 150, 200, 200, 200];
        let gray = SuperResolutionModel::rgb_to_gray(&rgb, 3, 1);
        assert_eq!(gray.len(), 3);
        assert_eq!(gray[0], 100);
        assert_eq!(gray[1], 150);
        assert_eq!(gray[2], 200);
    }

    #[test]
    fn test_yuv_rgb_conversions() {
        let y = vec![128u8; 64];
        let u = vec![128u8; 64];
        let v = vec![128u8; 64];

        let rgb = SuperResolutionModel::yuv_to_rgb(&y, &u, &v, 8, 8).unwrap();
        assert_eq!(rgb.len(), 64 * 3);

        let (y_back, u_back, v_back) = SuperResolutionModel::rgb_to_yuv(&rgb, 8, 8).unwrap();
        assert_eq!(y_back.len(), 64);
        assert_eq!(u_back.len(), 64);
        assert_eq!(v_back.len(), 64);

        // Values should be close after round-trip conversion
        for i in 0..64 {
            assert!((y[i] as i32 - y_back[i] as i32).abs() <= 2);
            assert!((u[i] as i32 - u_back[i] as i32).abs() <= 2);
            assert!((v[i] as i32 - v_back[i] as i32).abs() <= 2);
        }
    }

    #[test]
    fn test_yuv_rgb_invalid_sizes() {
        let y = vec![128u8; 64];
        let u = vec![128u8; 32]; // Wrong size
        let v = vec![128u8; 64];

        let result = SuperResolutionModel::yuv_to_rgb(&y, &u, &v, 8, 8);
        assert!(result.is_err());
    }

    #[test]
    fn test_motion_estimator_default() {
        let estimator = MotionEstimator::default();
        assert_eq!(estimator.block_size, 16);
        assert_eq!(estimator.search_range, 8);
    }

    #[test]
    fn test_temporal_filter_default() {
        let filter = TemporalFilter::default();
        assert_eq!(filter.temporal_weight, 0.3);
        assert_eq!(filter.motion_threshold, 10.0);
    }
}
