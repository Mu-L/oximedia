//! GPU device management and enumeration

use crate::{GpuError, Result};
use std::sync::Arc;
use wgpu::{
    Adapter, Device, DeviceDescriptor, Features, Instance, Limits, PowerPreference, Queue,
    RequestAdapterOptions,
};

/// Information about a GPU device
#[derive(Debug, Clone)]
pub struct GpuDeviceInfo {
    /// Device name
    pub name: String,
    /// Vendor ID
    pub vendor: u32,
    /// Device ID
    pub device: u32,
    /// Device type (discrete, integrated, virtual, cpu, unknown)
    pub device_type: String,
    /// Backend being used (Vulkan, Metal, DX12, etc.)
    pub backend: String,
}

/// GPU device wrapper
///
/// This structure manages the WGPU device and queue, providing a safe
/// interface for GPU operations.
pub struct GpuDevice {
    device: Arc<Device>,
    queue: Arc<Queue>,
    info: GpuDeviceInfo,
    #[allow(dead_code)]
    adapter: Adapter,
}

impl GpuDevice {
    /// Create a new GPU device
    ///
    /// # Arguments
    ///
    /// * `device_index` - Optional device index for multi-GPU selection
    ///
    /// # Errors
    ///
    /// Returns an error if no suitable adapter is found or device request fails.
    pub fn new(device_index: Option<usize>) -> Result<Self> {
        let instance = Self::create_instance();
        let adapter = pollster::block_on(Self::select_adapter(&instance, device_index))?;

        let info = Self::adapter_info(&adapter);

        let (device, queue) = pollster::block_on(Self::request_device(&adapter))?;

        Ok(Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
            info,
            adapter,
        })
    }

    /// List all available GPU devices
    pub fn list_devices() -> Result<Vec<GpuDeviceInfo>> {
        let instance = Self::create_instance();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let adapters = instance.enumerate_adapters(wgpu::Backends::all());
            Ok(adapters.iter().map(Self::adapter_info).collect())
        }

        #[cfg(target_arch = "wasm32")]
        {
            // On wasm, enumerate_adapters is not available; request a single adapter instead
            let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            }));
            match adapter {
                Some(a) => Ok(vec![Self::adapter_info(&a)]),
                None => Ok(Vec::new()),
            }
        }
    }

    /// Get device information
    #[must_use]
    pub fn info(&self) -> &GpuDeviceInfo {
        &self.info
    }

    /// Get the WGPU device
    #[must_use]
    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    /// Get the WGPU queue
    #[must_use]
    pub fn queue(&self) -> &Arc<Queue> {
        &self.queue
    }

    /// Wait for all GPU operations to complete
    pub fn wait(&self) {
        self.device.poll(wgpu::Maintain::Wait);
    }

    fn create_instance() -> Instance {
        Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        })
    }

    async fn select_adapter(instance: &Instance, device_index: Option<usize>) -> Result<Adapter> {
        if let Some(index) = device_index {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let adapters = instance.enumerate_adapters(wgpu::Backends::all());
                return adapters.into_iter().nth(index).ok_or(GpuError::NoAdapter);
            }

            #[cfg(target_arch = "wasm32")]
            {
                // On wasm, enumerate_adapters is not available; only index 0 is supported
                if index != 0 {
                    return Err(GpuError::NoAdapter);
                }
                return instance
                    .request_adapter(&RequestAdapterOptions {
                        power_preference: PowerPreference::HighPerformance,
                        compatible_surface: None,
                        force_fallback_adapter: false,
                    })
                    .await
                    .ok_or(GpuError::NoAdapter);
            }
        } else {
            // Select high-performance adapter by default
            instance
                .request_adapter(&RequestAdapterOptions {
                    power_preference: PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await
                .ok_or(GpuError::NoAdapter)
        }
    }

    async fn request_device(adapter: &Adapter) -> Result<(Device, Queue)> {
        adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("OxiMedia GPU Device"),
                    required_features: Features::empty(),
                    required_limits: Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuError::DeviceRequest(e.to_string()))
    }

    fn adapter_info(adapter: &Adapter) -> GpuDeviceInfo {
        let info = adapter.get_info();

        let device_type = match info.device_type {
            wgpu::DeviceType::DiscreteGpu => "discrete",
            wgpu::DeviceType::IntegratedGpu => "integrated",
            wgpu::DeviceType::VirtualGpu => "virtual",
            wgpu::DeviceType::Cpu => "cpu",
            wgpu::DeviceType::Other => "unknown",
        };

        let backend = match info.backend {
            wgpu::Backend::Vulkan => "Vulkan",
            wgpu::Backend::Metal => "Metal",
            wgpu::Backend::Dx12 => "DirectX 12",
            wgpu::Backend::Gl => "OpenGL",
            wgpu::Backend::BrowserWebGpu => "WebGPU",
            _ => "Unknown",
        };

        GpuDeviceInfo {
            name: info.name,
            vendor: info.vendor,
            device: info.device,
            device_type: device_type.to_string(),
            backend: backend.to_string(),
        }
    }
}

impl std::fmt::Debug for GpuDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuDevice")
            .field("info", &self.info)
            .finish()
    }
}
