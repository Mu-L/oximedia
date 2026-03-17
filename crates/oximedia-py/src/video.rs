//! Video codec bindings.

use crate::error::PyOxiResult;
use crate::types::{EncoderConfig, VideoFrame};
use oximedia_codec::{Av1Decoder as RustAv1Decoder, Av1Encoder as RustAv1Encoder};
use oximedia_codec::{VideoDecoder as RustVideoDecoder, VideoEncoder as RustVideoEncoder};
use oximedia_codec::{Vp8Decoder as RustVp8Decoder, Vp9Decoder as RustVp9Decoder};
use pyo3::prelude::*;

/// AV1 video decoder.
///
/// Decodes AV1 compressed video packets to raw video frames.
///
/// # Example
///
/// ```python
/// decoder = Av1Decoder()
/// decoder.send_packet(packet_data, pts=0)
/// frame = decoder.receive_frame()
/// if frame:
///     print(f"Decoded frame: {frame.width}x{frame.height}")
/// ```
#[pyclass]
pub struct Av1Decoder {
    inner: RustAv1Decoder,
}

#[pymethods]
impl Av1Decoder {
    /// Create a new AV1 decoder.
    #[new]
    fn new() -> PyOxiResult<Self> {
        let config = oximedia_codec::DecoderConfig::default();
        let inner = RustAv1Decoder::new(config).map_err(crate::error::from_codec_error)?;
        Ok(Self { inner })
    }

    /// Send a compressed packet to the decoder.
    ///
    /// # Arguments
    ///
    /// * `data` - Compressed packet data
    /// * `pts` - Presentation timestamp
    fn send_packet(&mut self, data: &[u8], pts: i64) -> PyOxiResult<()> {
        self.inner
            .send_packet(data, pts)
            .map_err(crate::error::from_codec_error)
    }

    /// Receive a decoded frame.
    ///
    /// Returns `None` if more data is needed.
    fn receive_frame(&mut self) -> PyOxiResult<Option<VideoFrame>> {
        self.inner
            .receive_frame()
            .map(|opt| opt.map(VideoFrame::from_rust))
            .map_err(crate::error::from_codec_error)
    }

    /// Flush the decoder.
    ///
    /// Call after all packets have been sent to retrieve remaining frames.
    fn flush(&mut self) -> PyOxiResult<()> {
        self.inner.flush().map_err(crate::error::from_codec_error)
    }

    /// Reset the decoder state.
    fn reset(&mut self) {
        self.inner.reset();
    }

    /// Get output dimensions (width, height) if known.
    fn dimensions(&self) -> Option<(u32, u32)> {
        self.inner.dimensions()
    }

    /// Context manager __enter__: return self.
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Context manager __exit__: flush and reset the decoder.
    #[pyo3(signature = (_exc_type, _exc_val, _exc_tb))]
    fn __exit__(
        &mut self,
        _exc_type: Option<Py<PyAny>>,
        _exc_val: Option<Py<PyAny>>,
        _exc_tb: Option<Py<PyAny>>,
    ) -> PyResult<bool> {
        let _ = self.flush();
        self.reset();
        Ok(false)
    }

    fn __str__(&self) -> String {
        "Av1Decoder".to_string()
    }

    fn __repr__(&self) -> String {
        format!("Av1Decoder(dimensions={:?})", self.inner.dimensions())
    }
}

/// AV1 video encoder.
///
/// Encodes raw video frames to AV1 compressed packets.
///
/// # Example
///
/// ```python
/// config = EncoderConfig(width=1920, height=1080, framerate=(30, 1), crf=28.0)
/// encoder = Av1Encoder(config)
/// encoder.send_frame(frame)
/// packet = encoder.receive_packet()
/// if packet:
///     print(f"Encoded packet: {len(packet['data'])} bytes, keyframe={packet['keyframe']}")
/// ```
#[pyclass]
pub struct Av1Encoder {
    inner: RustAv1Encoder,
}

#[pymethods]
impl Av1Encoder {
    /// Create a new AV1 encoder.
    ///
    /// # Arguments
    ///
    /// * `config` - Encoder configuration
    #[new]
    fn new(config: EncoderConfig) -> PyOxiResult<Self> {
        let inner =
            RustAv1Encoder::new(config.inner().clone()).map_err(crate::error::from_codec_error)?;
        Ok(Self { inner })
    }

    /// Send a raw frame to the encoder.
    ///
    /// # Arguments
    ///
    /// * `frame` - Video frame to encode
    fn send_frame(&mut self, frame: &VideoFrame) -> PyOxiResult<()> {
        self.inner
            .send_frame(frame.inner())
            .map_err(crate::error::from_codec_error)
    }

    /// Receive an encoded packet.
    ///
    /// Returns `None` if more frames are needed.
    ///
    /// Returns a dictionary with keys:
    /// - `data`: bytes - Compressed packet data
    /// - `pts`: int - Presentation timestamp
    /// - `dts`: int - Decode timestamp
    /// - `keyframe`: bool - Is this a keyframe
    /// - `duration`: Optional[int] - Duration in timebase units
    fn receive_packet(&mut self, py: Python<'_>) -> PyOxiResult<Option<Py<PyAny>>> {
        let packet = self
            .inner
            .receive_packet()
            .map_err(crate::error::from_codec_error)?;

        match packet {
            Some(pkt) => {
                let dict = pyo3::types::PyDict::new(py);
                dict.set_item("data", pyo3::types::PyBytes::new(py, &pkt.data))?;
                dict.set_item("pts", pkt.pts)?;
                dict.set_item("dts", pkt.dts)?;
                dict.set_item("keyframe", pkt.keyframe)?;
                dict.set_item("duration", pkt.duration)?;
                Ok(Some(dict.into()))
            }
            None => Ok(None),
        }
    }

    /// Flush the encoder.
    ///
    /// Call after all frames have been sent to retrieve remaining packets.
    fn flush(&mut self) -> PyOxiResult<()> {
        self.inner.flush().map_err(crate::error::from_codec_error)
    }

    /// Context manager __enter__: return self.
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Context manager __exit__: flush the encoder.
    #[pyo3(signature = (_exc_type, _exc_val, _exc_tb))]
    fn __exit__(
        &mut self,
        _exc_type: Option<Py<PyAny>>,
        _exc_val: Option<Py<PyAny>>,
        _exc_tb: Option<Py<PyAny>>,
    ) -> PyResult<bool> {
        let _ = self.flush();
        Ok(false)
    }

    fn __str__(&self) -> String {
        "Av1Encoder".to_string()
    }

    fn __repr__(&self) -> String {
        format!("Av1Encoder(config={:?})", self.inner.config())
    }
}

/// VP9 video decoder.
///
/// Decodes VP9 compressed video packets to raw video frames.
///
/// # Example
///
/// ```python
/// decoder = Vp9Decoder()
/// decoder.send_packet(packet_data, pts=0)
/// frame = decoder.receive_frame()
/// if frame:
///     print(f"Decoded frame: {frame.width}x{frame.height}")
/// ```
#[pyclass]
pub struct Vp9Decoder {
    inner: RustVp9Decoder,
}

#[pymethods]
impl Vp9Decoder {
    /// Create a new VP9 decoder.
    #[new]
    fn new() -> PyOxiResult<Self> {
        let config = oximedia_codec::DecoderConfig::default();
        let inner = RustVp9Decoder::new(config).map_err(crate::error::from_codec_error)?;
        Ok(Self { inner })
    }

    /// Send a compressed packet to the decoder.
    ///
    /// # Arguments
    ///
    /// * `data` - Compressed packet data
    /// * `pts` - Presentation timestamp
    fn send_packet(&mut self, data: &[u8], pts: i64) -> PyOxiResult<()> {
        self.inner
            .send_packet(data, pts)
            .map_err(crate::error::from_codec_error)
    }

    /// Receive a decoded frame.
    ///
    /// Returns `None` if more data is needed.
    fn receive_frame(&mut self) -> PyOxiResult<Option<VideoFrame>> {
        self.inner
            .receive_frame()
            .map(|opt| opt.map(VideoFrame::from_rust))
            .map_err(crate::error::from_codec_error)
    }

    /// Flush the decoder.
    ///
    /// Call after all packets have been sent to retrieve remaining frames.
    fn flush(&mut self) -> PyOxiResult<()> {
        self.inner.flush().map_err(crate::error::from_codec_error)
    }

    /// Reset the decoder state.
    fn reset(&mut self) {
        self.inner.reset();
    }

    /// Get output dimensions (width, height) if known.
    fn dimensions(&self) -> Option<(u32, u32)> {
        self.inner.dimensions()
    }

    /// Context manager __enter__: return self.
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Context manager __exit__: flush and reset.
    #[pyo3(signature = (_exc_type, _exc_val, _exc_tb))]
    fn __exit__(
        &mut self,
        _exc_type: Option<Py<PyAny>>,
        _exc_val: Option<Py<PyAny>>,
        _exc_tb: Option<Py<PyAny>>,
    ) -> PyResult<bool> {
        let _ = self.flush();
        self.reset();
        Ok(false)
    }

    fn __str__(&self) -> String {
        "Vp9Decoder".to_string()
    }

    fn __repr__(&self) -> String {
        format!("Vp9Decoder(dimensions={:?})", self.inner.dimensions())
    }
}

/// VP8 video decoder.
///
/// Decodes VP8 compressed video packets to raw video frames.
///
/// # Example
///
/// ```python
/// decoder = Vp8Decoder()
/// decoder.send_packet(packet_data, pts=0)
/// frame = decoder.receive_frame()
/// if frame:
///     print(f"Decoded frame: {frame.width}x{frame.height}")
/// ```
#[pyclass]
pub struct Vp8Decoder {
    inner: RustVp8Decoder,
}

#[pymethods]
impl Vp8Decoder {
    /// Create a new VP8 decoder.
    #[new]
    fn new() -> PyOxiResult<Self> {
        let config = oximedia_codec::DecoderConfig::default();
        let inner = RustVp8Decoder::new(config).map_err(crate::error::from_codec_error)?;
        Ok(Self { inner })
    }

    /// Send a compressed packet to the decoder.
    ///
    /// # Arguments
    ///
    /// * `data` - Compressed packet data
    /// * `pts` - Presentation timestamp
    fn send_packet(&mut self, data: &[u8], pts: i64) -> PyOxiResult<()> {
        self.inner
            .send_packet(data, pts)
            .map_err(crate::error::from_codec_error)
    }

    /// Receive a decoded frame.
    ///
    /// Returns `None` if more data is needed.
    fn receive_frame(&mut self) -> PyOxiResult<Option<VideoFrame>> {
        self.inner
            .receive_frame()
            .map(|opt| opt.map(VideoFrame::from_rust))
            .map_err(crate::error::from_codec_error)
    }

    /// Flush the decoder.
    ///
    /// Call after all packets have been sent to retrieve remaining frames.
    fn flush(&mut self) -> PyOxiResult<()> {
        self.inner.flush().map_err(crate::error::from_codec_error)
    }

    /// Reset the decoder state.
    fn reset(&mut self) {
        self.inner.reset();
    }

    /// Get output dimensions (width, height) if known.
    fn dimensions(&self) -> Option<(u32, u32)> {
        self.inner.dimensions()
    }

    /// Context manager __enter__: return self.
    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    /// Context manager __exit__: flush and reset.
    #[pyo3(signature = (_exc_type, _exc_val, _exc_tb))]
    fn __exit__(
        &mut self,
        _exc_type: Option<Py<PyAny>>,
        _exc_val: Option<Py<PyAny>>,
        _exc_tb: Option<Py<PyAny>>,
    ) -> PyResult<bool> {
        let _ = self.flush();
        self.reset();
        Ok(false)
    }

    fn __str__(&self) -> String {
        "Vp8Decoder".to_string()
    }

    fn __repr__(&self) -> String {
        format!("Vp8Decoder(dimensions={:?})", self.inner.dimensions())
    }
}
