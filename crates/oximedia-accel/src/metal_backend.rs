//! Metal backend stub for macOS/iOS GPU acceleration.
//!
//! On Apple platforms, Metal is the preferred GPU API.  This module provides
//! a complete interface-level stub so that the rest of the codebase can use a
//! uniform `BackendHandle` enum regardless of whether a real Metal runtime is
//! linked.  When the `metal` Cargo feature is enabled a full Metal
//! implementation would replace these stubs.
//!
//! **Current status**: stub / interface only — all operations fall through to
//! the CPU fallback with zero-copy data passing.

#![allow(dead_code)]

use crate::cpu_fallback::CpuAccel;
use crate::error::{AccelError, AccelResult};
use crate::traits::{HardwareAccel, ScaleFilter};
use oximedia_core::PixelFormat;

/// Capability report from the Metal runtime.
#[derive(Debug, Clone)]
pub struct MetalDeviceInfo {
    /// Device name (e.g. "Apple M3 Pro").
    pub name: String,
    /// GPU family tier (1–9, approximate).
    pub gpu_family: u32,
    /// Total unified memory in bytes (Apple Silicon).
    pub unified_memory_bytes: u64,
    /// Recommended max working-set size in bytes.
    pub recommended_max_working_set_bytes: u64,
    /// Whether GPU and CPU share memory (Apple Silicon UMA).
    pub has_unified_memory: bool,
    /// Whether this is a low-power (integrated) GPU.
    pub is_low_power: bool,
    /// Whether the device is headless (no display).
    pub is_headless: bool,
}

impl MetalDeviceInfo {
    /// Create a synthetic stub info for CI / non-Metal platforms.
    #[must_use]
    pub fn stub() -> Self {
        Self {
            name: "Metal Stub (no device)".to_string(),
            gpu_family: 0,
            unified_memory_bytes: 0,
            recommended_max_working_set_bytes: 0,
            has_unified_memory: false,
            is_low_power: false,
            is_headless: true,
        }
    }

    /// Total unified memory in gigabytes.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn unified_memory_gb(&self) -> f64 {
        self.unified_memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }
}

/// State of the Metal backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetalBackendState {
    /// Metal is not available on this platform or was not compiled in.
    Unavailable,
    /// Metal is available but device enumeration failed.
    NoDevice,
    /// Metal is fully initialised and ready.
    Ready,
}

/// Metal acceleration backend.
///
/// On non-Apple platforms (or when `metal` feature is not enabled) this
/// always reports `MetalBackendState::Unavailable` and falls through to CPU.
pub struct MetalAccel {
    state: MetalBackendState,
    device_info: Option<MetalDeviceInfo>,
    cpu_fallback: CpuAccel,
}

impl MetalAccel {
    /// Attempt to initialise the Metal backend.
    ///
    /// On platforms without Metal support, returns a valid `MetalAccel` in
    /// the `Unavailable` state (never returns an `Err`).
    #[must_use]
    pub fn new() -> Self {
        // Real Metal device enumeration via the `metal` crate would go here.
        // For the stub we always report Unavailable so that CPU fallback is used.
        let state = Self::detect_state();
        let device_info = if state == MetalBackendState::Ready {
            Some(Self::enumerate_device())
        } else {
            None
        };

        Self {
            state,
            device_info,
            cpu_fallback: CpuAccel::new(),
        }
    }

    fn detect_state() -> MetalBackendState {
        // Platform gate: Metal is only available on macOS/iOS.
        #[cfg(target_os = "macos")]
        {
            // In a real implementation we would call MTLCreateSystemDefaultDevice().
            // The `metal` crate (feature-gated) would provide that binding.
            // For the stub, we return Unavailable (no crate dependency yet).
            MetalBackendState::Unavailable
        }
        #[cfg(not(target_os = "macos"))]
        {
            MetalBackendState::Unavailable
        }
    }

    fn enumerate_device() -> MetalDeviceInfo {
        // Stub: would query MTLDevice properties.
        MetalDeviceInfo::stub()
    }

    /// Returns the current backend state.
    #[must_use]
    pub fn state(&self) -> MetalBackendState {
        self.state
    }

    /// Returns device information if the backend is ready.
    #[must_use]
    pub fn device_info(&self) -> Option<&MetalDeviceInfo> {
        self.device_info.as_ref()
    }

    /// Returns `true` if Metal acceleration is actually active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.state == MetalBackendState::Ready
    }

    /// Returns the backend name suitable for display.
    #[must_use]
    pub fn backend_name(&self) -> &str {
        match self.state {
            MetalBackendState::Ready => self
                .device_info
                .as_ref()
                .map(|d| d.name.as_str())
                .unwrap_or("Metal"),
            MetalBackendState::NoDevice => "Metal (no device)",
            MetalBackendState::Unavailable => "Metal (unavailable)",
        }
    }
}

impl Default for MetalAccel {
    fn default() -> Self {
        Self::new()
    }
}

impl HardwareAccel for MetalAccel {
    fn scale_image(
        &self,
        input: &[u8],
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
        format: PixelFormat,
        filter: ScaleFilter,
    ) -> AccelResult<Vec<u8>> {
        if self.state == MetalBackendState::Ready {
            // TODO(metal): dispatch to actual Metal compute shader.
            Err(AccelError::Unsupported(
                "Metal scale_image not yet implemented; falling back to CPU".to_string(),
            ))
        } else {
            self.cpu_fallback
                .scale_image(input, src_width, src_height, dst_width, dst_height, format, filter)
        }
    }

    fn convert_color(
        &self,
        input: &[u8],
        width: u32,
        height: u32,
        src_format: PixelFormat,
        dst_format: PixelFormat,
    ) -> AccelResult<Vec<u8>> {
        if self.state == MetalBackendState::Ready {
            Err(AccelError::Unsupported(
                "Metal convert_color not yet implemented; falling back to CPU".to_string(),
            ))
        } else {
            self.cpu_fallback
                .convert_color(input, width, height, src_format, dst_format)
        }
    }

    fn motion_estimation(
        &self,
        reference: &[u8],
        current: &[u8],
        width: u32,
        height: u32,
        block_size: u32,
    ) -> AccelResult<Vec<(i16, i16)>> {
        if self.state == MetalBackendState::Ready {
            Err(AccelError::Unsupported(
                "Metal motion_estimation not yet implemented; falling back to CPU".to_string(),
            ))
        } else {
            self.cpu_fallback
                .motion_estimation(reference, current, width, height, block_size)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metal_accel_new_does_not_panic() {
        let accel = MetalAccel::new();
        // On non-macOS platforms it should be Unavailable.
        #[cfg(not(target_os = "macos"))]
        assert_eq!(accel.state(), MetalBackendState::Unavailable);

        let _ = accel.backend_name();
        let _ = accel.is_active();
    }

    #[test]
    fn test_metal_accel_default() {
        let accel = MetalAccel::default();
        assert!(!accel.is_active() || accel.state() == MetalBackendState::Ready);
    }

    #[test]
    fn test_metal_device_info_stub() {
        let info = MetalDeviceInfo::stub();
        assert_eq!(info.unified_memory_bytes, 0);
        assert!((info.unified_memory_gb() - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_metal_device_info_memory_gb() {
        let info = MetalDeviceInfo {
            unified_memory_bytes: 16 * 1024 * 1024 * 1024,
            ..MetalDeviceInfo::stub()
        };
        assert!((info.unified_memory_gb() - 16.0).abs() < 0.01);
    }

    #[test]
    fn test_metal_scale_image_fallback() {
        let accel = MetalAccel::new();
        let input = vec![128u8; 4 * 4 * 3];
        let result = accel.scale_image(&input, 4, 4, 2, 2, PixelFormat::Rgb24, ScaleFilter::Nearest);
        // When unavailable, falls back to CPU which succeeds.
        assert!(result.is_ok(), "fallback should succeed: {:?}", result.err());
    }

    #[test]
    fn test_metal_convert_color_fallback() {
        let accel = MetalAccel::new();
        let input = vec![128u8; 4 * 4 * 3];
        let result =
            accel.convert_color(&input, 4, 4, PixelFormat::Rgb24, PixelFormat::Yuv420p);
        assert!(result.is_ok(), "fallback should succeed: {:?}", result.err());
    }

    #[test]
    fn test_metal_motion_estimation_fallback() {
        let accel = MetalAccel::new();
        let frame = vec![0u8; 8 * 8];
        let result = accel.motion_estimation(&frame, &frame, 8, 8, 4);
        assert!(result.is_ok());
    }

    #[test]
    fn test_metal_backend_state_unavailable_has_no_device_info() {
        let accel = MetalAccel::new();
        if accel.state() == MetalBackendState::Unavailable {
            assert!(accel.device_info().is_none());
        }
    }

    #[test]
    fn test_metal_backend_name_not_empty() {
        let accel = MetalAccel::new();
        assert!(!accel.backend_name().is_empty());
    }
}
