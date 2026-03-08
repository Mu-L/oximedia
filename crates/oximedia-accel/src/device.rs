//! GPU device enumeration and selection.

use crate::error::{AccelError, AccelResult};
use std::sync::Arc;
use vulkano::device::{
    Device, DeviceCreateInfo, DeviceExtensions, Queue, QueueCreateInfo, QueueFlags,
};
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::{Version, VulkanLibrary};

/// Device selection strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DevicePreference {
    /// Prefer discrete GPUs (dedicated graphics cards).
    #[default]
    Discrete,
    /// Prefer integrated GPUs.
    Integrated,
    /// Select the first available device.
    Any,
    /// Select device with most memory.
    MostMemory,
    /// Select device with most compute units.
    MostCompute,
}

/// Device selector configuration.
#[derive(Debug, Clone)]
pub struct DeviceSelector {
    /// Device preference strategy.
    pub preference: DevicePreference,
    /// Minimum Vulkan API version required.
    pub min_api_version: Version,
    /// Required device extensions.
    pub required_extensions: DeviceExtensions,
}

impl Default for DeviceSelector {
    fn default() -> Self {
        Self {
            preference: DevicePreference::default(),
            min_api_version: Version::V1_0,
            required_extensions: DeviceExtensions::empty(),
        }
    }
}

impl DeviceSelector {
    /// Creates a new device selector with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the device preference.
    #[must_use]
    pub fn with_preference(mut self, preference: DevicePreference) -> Self {
        self.preference = preference;
        self
    }

    /// Sets the minimum API version.
    #[must_use]
    pub fn with_min_api_version(mut self, version: Version) -> Self {
        self.min_api_version = version;
        self
    }

    /// Selects a Vulkan device based on the configured preferences.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Vulkan library cannot be loaded
    /// - No suitable instance can be created
    /// - No suitable device is found
    /// - Device creation fails
    pub fn select(&self) -> AccelResult<VulkanDevice> {
        let library = VulkanLibrary::new()
            .map_err(|e| AccelError::VulkanInit(format!("Failed to load Vulkan library: {e:?}")))?;

        let instance = Instance::new(
            library,
            InstanceCreateInfo {
                application_name: Some("OxiMedia".to_string()),
                application_version: Version::V1_0,
                ..Default::default()
            },
        )
        .map_err(|e| AccelError::VulkanInit(format!("Failed to create instance: {e:?}")))?;

        let physical_device = instance
            .enumerate_physical_devices()
            .map_err(|e| {
                AccelError::DeviceSelection(format!("Failed to enumerate devices: {e:?}"))
            })?
            .filter(|dev| {
                dev.api_version() >= self.min_api_version
                    && dev
                        .supported_extensions()
                        .contains(&self.required_extensions)
            })
            .max_by_key(|dev| self.score_device(dev))
            .ok_or(AccelError::NoDevice)?;

        tracing::info!(
            "Selected GPU: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type
        );

        let queue_family_index = physical_device
            .queue_family_properties()
            .iter()
            .position(|q| q.queue_flags.contains(QueueFlags::COMPUTE))
            .ok_or_else(|| {
                AccelError::DeviceSelection("No compute queue family found".to_string())
            })?;

        let (device, mut queues) = Device::new(
            physical_device.clone(),
            DeviceCreateInfo {
                enabled_extensions: self.required_extensions,
                queue_create_infos: vec![QueueCreateInfo {
                    #[allow(clippy::cast_possible_truncation)]
                    queue_family_index: queue_family_index as u32,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .map_err(|e| AccelError::DeviceSelection(format!("Failed to create device: {e:?}")))?;

        let queue = queues.next().ok_or_else(|| {
            AccelError::DeviceSelection("Failed to get compute queue".to_string())
        })?;

        let allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));

        Ok(VulkanDevice {
            physical_device,
            device,
            queue,
            allocator,
        })
    }

    fn score_device(&self, device: &vulkano::device::physical::PhysicalDevice) -> u32 {
        let props = device.properties();
        let device_type = props.device_type;

        let mut score = match self.preference {
            DevicePreference::Discrete => {
                if device_type == vulkano::device::physical::PhysicalDeviceType::DiscreteGpu {
                    1000
                } else {
                    0
                }
            }
            DevicePreference::Integrated => {
                if device_type == vulkano::device::physical::PhysicalDeviceType::IntegratedGpu {
                    1000
                } else {
                    0
                }
            }
            DevicePreference::Any => 100,
            DevicePreference::MostMemory => {
                // Score based on available memory (in MB)
                let memory = device
                    .memory_properties()
                    .memory_heaps
                    .iter()
                    .map(|heap| heap.size / (1024 * 1024))
                    .sum::<u64>();
                #[allow(clippy::cast_possible_truncation)]
                {
                    memory.min(u64::from(u32::MAX)) as u32
                }
            }
            DevicePreference::MostCompute => {
                // Score based on compute capability approximation
                props.max_compute_work_group_count[0]
            }
        };

        // Add bonus for discrete GPUs regardless of preference
        if device_type == vulkano::device::physical::PhysicalDeviceType::DiscreteGpu {
            score += 100;
        }

        score
    }
}

/// Represents a selected Vulkan device with its associated resources.
pub struct VulkanDevice {
    /// The physical device.
    pub physical_device: Arc<vulkano::device::physical::PhysicalDevice>,
    /// The logical device.
    pub device: Arc<Device>,
    /// The compute queue.
    pub queue: Arc<Queue>,
    /// Memory allocator.
    pub allocator: Arc<StandardMemoryAllocator>,
}

impl VulkanDevice {
    /// Returns the device name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.physical_device.properties().device_name
    }

    /// Returns the device type.
    #[must_use]
    pub fn device_type(&self) -> vulkano::device::physical::PhysicalDeviceType {
        self.physical_device.properties().device_type
    }

    /// Returns the total device memory in bytes.
    #[must_use]
    pub fn total_memory(&self) -> u64 {
        self.physical_device
            .memory_properties()
            .memory_heaps
            .iter()
            .map(|heap| heap.size)
            .sum()
    }
}
