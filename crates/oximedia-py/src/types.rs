//! Core types for Python bindings.

use oximedia_codec::{
    AudioFrame as RustAudioFrame, ChannelLayout as RustChannelLayout,
    SampleFormat as RustSampleFormat, VideoFrame as RustVideoFrame,
};
use oximedia_core::PixelFormat as RustPixelFormat;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

/// Pixel format for video frames.
#[pyclass]
#[derive(Clone, Copy)]
pub struct PixelFormat {
    inner: RustPixelFormat,
}

#[pymethods]
impl PixelFormat {
    /// YUV 4:2:0 planar format (most common).
    #[classattr]
    const YUV420P: &'static str = "yuv420p";

    /// YUV 4:2:2 planar format.
    #[classattr]
    const YUV422P: &'static str = "yuv422p";

    /// YUV 4:4:4 planar format.
    #[classattr]
    const YUV444P: &'static str = "yuv444p";

    /// Grayscale 8-bit.
    #[classattr]
    const GRAY8: &'static str = "gray8";

    /// Create a new pixel format from string.
    #[new]
    fn new(format: &str) -> PyResult<Self> {
        let inner = match format {
            "yuv420p" => RustPixelFormat::Yuv420p,
            "yuv422p" => RustPixelFormat::Yuv422p,
            "yuv444p" => RustPixelFormat::Yuv444p,
            "gray8" => RustPixelFormat::Gray8,
            _ => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Unknown pixel format: {format}"
                )))
            }
        };
        Ok(Self { inner })
    }

    /// Check if format is planar.
    fn is_planar(&self) -> bool {
        self.inner.is_planar()
    }

    /// Get number of planes.
    fn plane_count(&self) -> usize {
        self.inner.plane_count() as usize
    }

    fn __str__(&self) -> String {
        format!("{:?}", self.inner)
    }

    fn __repr__(&self) -> String {
        format!("PixelFormat('{:?}')", self.inner)
    }
}

impl PixelFormat {
    #[must_use]
    pub fn inner(&self) -> RustPixelFormat {
        self.inner
    }

    #[must_use]
    pub fn from_rust(inner: RustPixelFormat) -> Self {
        Self { inner }
    }

    /// Public Rust-facing constructor (mirrors the `#[new]` pymethods fn).
    pub fn new_rust(format: &str) -> PyResult<Self> {
        let inner = match format {
            "yuv420p" => RustPixelFormat::Yuv420p,
            "yuv422p" => RustPixelFormat::Yuv422p,
            "yuv444p" => RustPixelFormat::Yuv444p,
            "gray8" => RustPixelFormat::Gray8,
            _ => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Unknown pixel format: {format}"
                )))
            }
        };
        Ok(Self { inner })
    }
}

/// Audio sample format.
#[pyclass]
#[derive(Clone, Copy)]
pub struct SampleFormat {
    inner: RustSampleFormat,
}

#[pymethods]
impl SampleFormat {
    /// 32-bit floating point.
    #[classattr]
    const F32: &'static str = "f32";

    /// 16-bit signed integer.
    #[classattr]
    const I16: &'static str = "i16";

    /// Create a new sample format from string.
    #[new]
    fn new(format: &str) -> PyResult<Self> {
        let inner = match format {
            "f32" => RustSampleFormat::F32,
            "i16" => RustSampleFormat::I16,
            _ => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Unknown sample format: {format}"
                )))
            }
        };
        Ok(Self { inner })
    }

    /// Get sample size in bytes.
    fn sample_size(&self) -> usize {
        self.inner.sample_size()
    }

    fn __str__(&self) -> String {
        format!("{:?}", self.inner)
    }

    fn __repr__(&self) -> String {
        format!("SampleFormat('{:?}')", self.inner)
    }
}

impl SampleFormat {
    #[must_use]
    pub fn inner(&self) -> RustSampleFormat {
        self.inner
    }

    #[must_use]
    pub fn from_rust(inner: RustSampleFormat) -> Self {
        Self { inner }
    }

    /// Public Rust-facing constructor (mirrors the `#[new]` pymethods fn).
    pub fn new_rust(format: &str) -> PyResult<Self> {
        let inner = match format {
            "f32" => RustSampleFormat::F32,
            "i16" => RustSampleFormat::I16,
            _ => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Unknown sample format: {format}"
                )))
            }
        };
        Ok(Self { inner })
    }
}

/// Audio channel layout.
#[pyclass]
#[derive(Clone, Copy)]
pub struct ChannelLayout {
    inner: RustChannelLayout,
}

#[pymethods]
impl ChannelLayout {
    /// Single channel (mono).
    #[classattr]
    const MONO: &'static str = "mono";

    /// Two channels (stereo).
    #[classattr]
    const STEREO: &'static str = "stereo";

    /// Create a new channel layout from string.
    #[new]
    fn new(layout: &str) -> PyResult<Self> {
        let inner = match layout {
            "mono" => RustChannelLayout::Mono,
            "stereo" => RustChannelLayout::Stereo,
            _ => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Unknown channel layout: {layout}"
                )))
            }
        };
        Ok(Self { inner })
    }

    /// Get number of channels.
    fn channel_count(&self) -> usize {
        self.inner.channel_count()
    }

    fn __str__(&self) -> String {
        format!("{:?}", self.inner)
    }

    fn __repr__(&self) -> String {
        format!("ChannelLayout('{:?}')", self.inner)
    }
}

impl ChannelLayout {
    #[must_use]
    pub fn inner(&self) -> RustChannelLayout {
        self.inner
    }
}

/// Rational number for frame rates and timebases.
#[pyclass]
#[derive(Clone, Copy)]
pub struct Rational {
    num: i32,
    den: i32,
}

#[pymethods]
impl Rational {
    /// Create a new rational number.
    ///
    /// # Arguments
    ///
    /// * `num` - Numerator
    /// * `den` - Denominator
    #[new]
    fn new(num: i32, den: i32) -> Self {
        Self { num, den }
    }

    /// Get numerator.
    #[getter]
    fn num(&self) -> i32 {
        self.num
    }

    /// Get denominator.
    #[getter]
    fn den(&self) -> i32 {
        self.den
    }

    /// Convert to float.
    fn to_float(&self) -> f64 {
        f64::from(self.num) / f64::from(self.den)
    }

    /// Pickle support: return constructor arguments.
    fn __getnewargs__(&self) -> (i32, i32) {
        (self.num, self.den)
    }

    fn __str__(&self) -> String {
        format!("{}/{}", self.num, self.den)
    }

    fn __repr__(&self) -> String {
        format!("Rational({}, {})", self.num, self.den)
    }
}

impl Rational {
    #[must_use]
    pub fn to_rust(&self) -> oximedia_core::Rational {
        oximedia_core::Rational::new(i64::from(self.num), i64::from(self.den))
    }

    #[must_use]
    pub fn from_rust(r: oximedia_core::Rational) -> Self {
        Self {
            num: r.num as i32,
            den: r.den as i32,
        }
    }
}

/// Video frame containing decoded pixel data.
#[pyclass]
pub struct VideoFrame {
    pub(crate) inner: RustVideoFrame,
}

#[pymethods]
impl VideoFrame {
    /// Create a new video frame.
    ///
    /// # Arguments
    ///
    /// * `width` - Frame width in pixels
    /// * `height` - Frame height in pixels
    /// * `format` - Pixel format
    #[new]
    fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        Self {
            inner: RustVideoFrame::new(format.inner(), width, height),
        }
    }

    /// Get frame width.
    #[getter]
    fn width(&self) -> u32 {
        self.inner.width
    }

    /// Get frame height.
    #[getter]
    fn height(&self) -> u32 {
        self.inner.height
    }

    /// Get pixel format.
    #[getter]
    fn format(&self) -> PixelFormat {
        PixelFormat::from_rust(self.inner.format)
    }

    /// Get presentation timestamp.
    #[getter]
    pub fn pts(&self) -> i64 {
        self.inner.timestamp.pts
    }

    /// Set presentation timestamp.
    #[setter]
    fn set_pts(&mut self, pts: i64) {
        self.inner.timestamp.pts = pts;
    }

    /// Get plane data by index.
    ///
    /// # Arguments
    ///
    /// * `index` - Plane index (0 = Y, 1 = U, 2 = V for YUV formats)
    fn plane_data<'py>(&self, py: Python<'py>, index: usize) -> PyResult<Bound<'py, PyBytes>> {
        if index >= self.inner.planes.len() {
            return Err(PyErr::new::<pyo3::exceptions::PyIndexError, _>(format!(
                "Plane index {} out of range (frame has {} planes)",
                index,
                self.inner.planes.len()
            )));
        }
        Ok(PyBytes::new(py, &self.inner.planes[index].data))
    }

    /// Get plane stride by index.
    fn plane_stride(&self, index: usize) -> PyResult<usize> {
        if index >= self.inner.planes.len() {
            return Err(PyErr::new::<pyo3::exceptions::PyIndexError, _>(format!(
                "Plane index {} out of range (frame has {} planes)",
                index,
                self.inner.planes.len()
            )));
        }
        Ok(self.inner.planes[index].stride)
    }

    /// Get number of planes.
    fn plane_count(&self) -> usize {
        self.inner.planes.len()
    }

    fn __str__(&self) -> String {
        format!(
            "VideoFrame({}x{}, {:?})",
            self.inner.width, self.inner.height, self.inner.format
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "VideoFrame(width={}, height={}, format={:?}, pts={})",
            self.inner.width, self.inner.height, self.inner.format, self.inner.timestamp.pts
        )
    }
}

impl VideoFrame {
    #[must_use]
    pub fn from_rust(inner: RustVideoFrame) -> Self {
        Self { inner }
    }

    #[must_use]
    pub fn inner(&self) -> &RustVideoFrame {
        &self.inner
    }

    /// Public Rust-facing constructor (mirrors the `#[new]` pymethods fn).
    #[must_use]
    pub fn new_rust(width: u32, height: u32, format: PixelFormat) -> Self {
        Self {
            inner: RustVideoFrame::new(format.inner(), width, height),
        }
    }

    /// Public Rust-facing width accessor.
    #[must_use]
    pub fn width_rust(&self) -> u32 {
        self.inner.width
    }

    /// Public Rust-facing height accessor.
    #[must_use]
    pub fn height_rust(&self) -> u32 {
        self.inner.height
    }

    /// Public Rust-facing PTS setter.
    pub fn set_pts_rust(&mut self, pts: i64) {
        self.inner.timestamp.pts = pts;
    }
}

/// Audio frame containing decoded PCM samples.
#[pyclass]
pub struct AudioFrame {
    inner: RustAudioFrame,
}

#[pymethods]
impl AudioFrame {
    /// Create a new audio frame.
    ///
    /// # Arguments
    ///
    /// * `samples` - Raw sample data (interleaved if multi-channel)
    /// * `sample_count` - Number of samples per channel
    /// * `sample_rate` - Sample rate in Hz
    /// * `channels` - Number of channels
    /// * `format` - Sample format
    #[new]
    #[pyo3(signature = (samples, sample_count, sample_rate, channels, format))]
    fn new(
        samples: Vec<u8>,
        sample_count: usize,
        sample_rate: u32,
        channels: usize,
        format: SampleFormat,
    ) -> Self {
        Self {
            inner: RustAudioFrame::new(
                samples,
                sample_count,
                sample_rate,
                channels,
                format.inner(),
            ),
        }
    }

    /// Get sample data as bytes.
    fn samples<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.samples)
    }

    /// Get number of samples per channel.
    #[getter]
    pub fn sample_count(&self) -> usize {
        self.inner.sample_count
    }

    /// Get sample rate in Hz.
    #[getter]
    pub fn sample_rate(&self) -> u32 {
        self.inner.sample_rate
    }

    /// Get number of channels.
    #[getter]
    pub fn channels(&self) -> usize {
        self.inner.channels
    }

    /// Get sample format.
    #[getter]
    fn format(&self) -> SampleFormat {
        SampleFormat::from_rust(self.inner.format)
    }

    /// Get presentation timestamp.
    #[getter]
    fn pts(&self) -> Option<i64> {
        self.inner.pts
    }

    /// Get duration in seconds.
    fn duration_seconds(&self) -> f64 {
        self.inner.duration_seconds()
    }

    /// Convert samples to f32 array.
    pub fn to_f32(&self) -> PyResult<Vec<f32>> {
        self.inner.to_f32().map_err(crate::error::from_codec_error)
    }

    /// Convert samples to i16 array.
    fn to_i16(&self) -> PyResult<Vec<i16>> {
        self.inner.to_i16().map_err(crate::error::from_codec_error)
    }

    fn __str__(&self) -> String {
        format!(
            "AudioFrame({} samples, {}Hz, {} channels, {:?})",
            self.inner.sample_count, self.inner.sample_rate, self.inner.channels, self.inner.format
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "AudioFrame(sample_count={}, sample_rate={}, channels={}, format={:?})",
            self.inner.sample_count, self.inner.sample_rate, self.inner.channels, self.inner.format
        )
    }
}

impl AudioFrame {
    #[must_use]
    pub fn from_rust(inner: RustAudioFrame) -> Self {
        Self { inner }
    }

    #[must_use]
    pub fn inner(&self) -> &RustAudioFrame {
        &self.inner
    }
}

/// Encoder preset (speed vs quality tradeoff).
#[pyclass]
#[derive(Clone, Copy)]
pub struct EncoderPreset {
    inner: oximedia_codec::EncoderPreset,
}

#[pymethods]
impl EncoderPreset {
    /// Fastest encoding, lowest quality.
    #[classattr]
    const ULTRAFAST: &'static str = "ultrafast";

    /// Fast encoding.
    #[classattr]
    const FAST: &'static str = "fast";

    /// Balanced speed and quality.
    #[classattr]
    const MEDIUM: &'static str = "medium";

    /// Slower, better quality.
    #[classattr]
    const SLOW: &'static str = "slow";

    /// Very slow, high quality.
    #[classattr]
    const VERYSLOW: &'static str = "veryslow";

    /// Create a new encoder preset from string.
    #[new]
    fn new(preset: &str) -> PyResult<Self> {
        let inner = match preset {
            "ultrafast" => oximedia_codec::EncoderPreset::Ultrafast,
            "superfast" => oximedia_codec::EncoderPreset::Superfast,
            "veryfast" => oximedia_codec::EncoderPreset::Veryfast,
            "faster" => oximedia_codec::EncoderPreset::Faster,
            "fast" => oximedia_codec::EncoderPreset::Fast,
            "medium" => oximedia_codec::EncoderPreset::Medium,
            "slow" => oximedia_codec::EncoderPreset::Slow,
            "slower" => oximedia_codec::EncoderPreset::Slower,
            "veryslow" => oximedia_codec::EncoderPreset::Veryslow,
            "placebo" => oximedia_codec::EncoderPreset::Placebo,
            _ => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Unknown encoder preset: {preset}"
                )))
            }
        };
        Ok(Self { inner })
    }

    fn __str__(&self) -> String {
        format!("{:?}", self.inner)
    }

    fn __repr__(&self) -> String {
        format!("EncoderPreset('{:?}')", self.inner)
    }
}

impl EncoderPreset {
    #[must_use]
    pub fn inner(&self) -> oximedia_codec::EncoderPreset {
        self.inner
    }
}

/// Encoder configuration.
#[pyclass]
#[derive(Clone)]
pub struct EncoderConfig {
    pub(crate) inner: oximedia_codec::EncoderConfig,
}

#[pymethods]
impl EncoderConfig {
    /// Create a new encoder configuration.
    ///
    /// # Arguments
    ///
    /// * `width` - Frame width in pixels
    /// * `height` - Frame height in pixels
    /// * `framerate` - Frame rate as (numerator, denominator) tuple
    /// * `crf` - Constant rate factor (quality, lower = better, typically 18-28)
    /// * `preset` - Encoder preset (default: "medium")
    /// * `keyint` - Keyframe interval in frames (default: 250)
    #[new]
    #[pyo3(signature = (width, height, framerate=(30, 1), crf=28.0, preset=None, keyint=250))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        width: u32,
        height: u32,
        framerate: (i32, i32),
        crf: f32,
        preset: Option<EncoderPreset>,
        keyint: u32,
    ) -> Self {
        let mut inner = oximedia_codec::EncoderConfig::default();
        inner.width = width;
        inner.height = height;
        inner.framerate =
            oximedia_core::Rational::new(i64::from(framerate.0), i64::from(framerate.1));
        inner.bitrate = oximedia_codec::BitrateMode::Crf(crf);
        inner.preset = preset.map_or(oximedia_codec::EncoderPreset::Medium, |p| p.inner());
        inner.keyint = keyint;
        Self { inner }
    }

    /// Get frame width.
    #[getter]
    fn width(&self) -> u32 {
        self.inner.width
    }

    /// Get frame height.
    #[getter]
    fn height(&self) -> u32 {
        self.inner.height
    }

    /// Get frame rate as (numerator, denominator).
    #[getter]
    fn framerate(&self) -> (i32, i32) {
        (
            self.inner.framerate.num as i32,
            self.inner.framerate.den as i32,
        )
    }

    /// Get keyframe interval.
    #[getter]
    fn keyint(&self) -> u32 {
        self.inner.keyint
    }

    /// Pickle support: reduce to constructor args.
    fn __getstate__(&self) -> (u32, u32, (i32, i32), u32) {
        (
            self.inner.width,
            self.inner.height,
            (
                self.inner.framerate.num as i32,
                self.inner.framerate.den as i32,
            ),
            self.inner.keyint,
        )
    }

    /// Pickle support: restore from constructor args.
    fn __setstate__(&mut self, state: (u32, u32, (i32, i32), u32)) -> PyResult<()> {
        let (width, height, (fps_num, fps_den), keyint) = state;
        self.inner.width = width;
        self.inner.height = height;
        self.inner.framerate = oximedia_core::Rational::new(i64::from(fps_num), i64::from(fps_den));
        self.inner.keyint = keyint;
        Ok(())
    }

    fn __str__(&self) -> String {
        format!(
            "EncoderConfig({}x{}, {}fps)",
            self.inner.width,
            self.inner.height,
            self.inner.framerate.num / self.inner.framerate.den
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "EncoderConfig(width={}, height={}, framerate=({}, {}), keyint={})",
            self.inner.width,
            self.inner.height,
            self.inner.framerate.num,
            self.inner.framerate.den,
            self.inner.keyint
        )
    }
}

impl EncoderConfig {
    #[must_use]
    pub fn inner(&self) -> &oximedia_codec::EncoderConfig {
        &self.inner
    }
}
