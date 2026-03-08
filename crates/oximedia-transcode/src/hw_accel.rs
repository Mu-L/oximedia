//! Hardware acceleration detection and configuration.

use serde::{Deserialize, Serialize};

/// Hardware acceleration types supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HwAccelType {
    /// No hardware acceleration.
    None,
    /// NVIDIA NVENC (CUDA).
    Nvenc,
    /// Intel Quick Sync Video.
    Qsv,
    /// AMD VCE/VCN.
    Amd,
    /// Apple `VideoToolbox`.
    VideoToolbox,
    /// Vulkan acceleration.
    Vulkan,
    /// Direct3D 11.
    D3d11,
    /// VAAPI (Linux).
    Vaapi,
    /// VDPAU (Linux, legacy).
    Vdpau,
}

/// Hardware encoder information.
#[derive(Debug, Clone)]
pub struct HwEncoder {
    /// Encoder type.
    pub accel_type: HwAccelType,
    /// Codec name.
    pub codec: String,
    /// Encoder name.
    pub encoder_name: String,
    /// Whether the encoder is available.
    pub available: bool,
    /// Maximum resolution supported.
    pub max_resolution: (u32, u32),
    /// Supported features.
    pub features: Vec<HwFeature>,
}

/// Hardware encoder features.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HwFeature {
    /// Supports 10-bit encoding.
    TenBit,
    /// Supports HDR encoding.
    Hdr,
    /// Supports B-frames.
    BFrames,
    /// Supports look-ahead.
    Lookahead,
    /// Supports temporal AQ.
    TemporalAq,
    /// Supports spatial AQ.
    SpatialAq,
    /// Supports weighted prediction.
    WeightedPred,
    /// Supports custom quantization matrices.
    CustomQuant,
}

/// Hardware acceleration configuration.
#[derive(Debug, Clone)]
pub struct HwAccelConfig {
    /// Preferred acceleration type.
    pub preferred_type: HwAccelType,
    /// Fallback to software if hardware unavailable.
    pub allow_fallback: bool,
    /// Use hardware for decoding.
    pub decode: bool,
    /// Use hardware for encoding.
    pub encode: bool,
    /// Device ID to use (for multi-GPU systems).
    pub device_id: Option<u32>,
}

impl Default for HwAccelConfig {
    fn default() -> Self {
        Self {
            preferred_type: HwAccelType::None,
            allow_fallback: true,
            decode: false,
            encode: false,
            device_id: None,
        }
    }
}

impl HwAccelConfig {
    /// Creates a new hardware acceleration config.
    #[must_use]
    pub fn new(accel_type: HwAccelType) -> Self {
        Self {
            preferred_type: accel_type,
            allow_fallback: true,
            decode: true,
            encode: true,
            device_id: None,
        }
    }

    /// Sets whether to allow fallback to software.
    #[must_use]
    pub fn allow_fallback(mut self, allow: bool) -> Self {
        self.allow_fallback = allow;
        self
    }

    /// Sets whether to use hardware for decoding.
    #[must_use]
    pub fn decode(mut self, enable: bool) -> Self {
        self.decode = enable;
        self
    }

    /// Sets whether to use hardware for encoding.
    #[must_use]
    pub fn encode(mut self, enable: bool) -> Self {
        self.encode = enable;
        self
    }

    /// Sets the device ID.
    #[must_use]
    pub fn device_id(mut self, id: u32) -> Self {
        self.device_id = Some(id);
        self
    }
}

impl HwAccelType {
    /// Gets the platform name.
    #[must_use]
    pub fn platform_name(self) -> &'static str {
        match self {
            Self::None => "software",
            Self::Nvenc => "NVIDIA NVENC",
            Self::Qsv => "Intel Quick Sync",
            Self::Amd => "AMD VCE/VCN",
            Self::VideoToolbox => "Apple VideoToolbox",
            Self::Vulkan => "Vulkan",
            Self::D3d11 => "Direct3D 11",
            Self::Vaapi => "VAAPI",
            Self::Vdpau => "VDPAU",
        }
    }

    /// Checks if this acceleration type is available on the current platform.
    #[must_use]
    pub fn is_available(self) -> bool {
        // Placeholder - would perform actual detection
        match self {
            Self::None => true,
            Self::Nvenc => detect_nvenc(),
            Self::Qsv => detect_qsv(),
            Self::Amd => detect_amd(),
            Self::VideoToolbox => detect_videotoolbox(),
            Self::Vulkan => detect_vulkan(),
            Self::D3d11 => detect_d3d11(),
            Self::Vaapi => detect_vaapi(),
            Self::Vdpau => detect_vdpau(),
        }
    }

    /// Gets supported codecs for this acceleration type.
    #[must_use]
    pub fn supported_codecs(self) -> Vec<&'static str> {
        match self {
            Self::None => vec!["h264", "vp8", "vp9", "av1", "theora"],
            Self::Nvenc => vec!["h264", "h265", "av1"],
            Self::Qsv => vec!["h264", "h265", "vp9", "av1"],
            Self::Amd => vec!["h264", "h265", "av1"],
            Self::VideoToolbox => vec!["h264", "h265"],
            Self::Vulkan => vec!["h264", "h265"],
            Self::D3d11 => vec!["h264", "h265", "vp9"],
            Self::Vaapi => vec!["h264", "h265", "vp8", "vp9", "av1"],
            Self::Vdpau => vec!["h264", "h265"],
        }
    }

    /// Gets the encoder name for a given codec.
    #[must_use]
    pub fn encoder_name(self, codec: &str) -> Option<String> {
        match self {
            Self::None => Some(codec.to_string()),
            Self::Nvenc => match codec {
                "h264" => Some("h264_nvenc".to_string()),
                "h265" => Some("hevc_nvenc".to_string()),
                "av1" => Some("av1_nvenc".to_string()),
                _ => None,
            },
            Self::Qsv => match codec {
                "h264" => Some("h264_qsv".to_string()),
                "h265" => Some("hevc_qsv".to_string()),
                "vp9" => Some("vp9_qsv".to_string()),
                "av1" => Some("av1_qsv".to_string()),
                _ => None,
            },
            Self::Amd => match codec {
                "h264" => Some("h264_amf".to_string()),
                "h265" => Some("hevc_amf".to_string()),
                "av1" => Some("av1_amf".to_string()),
                _ => None,
            },
            Self::VideoToolbox => match codec {
                "h264" => Some("h264_videotoolbox".to_string()),
                "h265" => Some("hevc_videotoolbox".to_string()),
                _ => None,
            },
            Self::Vulkan => match codec {
                "h264" => Some("h264_vulkan".to_string()),
                "h265" => Some("hevc_vulkan".to_string()),
                _ => None,
            },
            Self::D3d11 => match codec {
                "h264" => Some("h264_d3d11va".to_string()),
                "h265" => Some("hevc_d3d11va".to_string()),
                "vp9" => Some("vp9_d3d11va".to_string()),
                _ => None,
            },
            Self::Vaapi => match codec {
                "h264" => Some("h264_vaapi".to_string()),
                "h265" => Some("hevc_vaapi".to_string()),
                "vp8" => Some("vp8_vaapi".to_string()),
                "vp9" => Some("vp9_vaapi".to_string()),
                "av1" => Some("av1_vaapi".to_string()),
                _ => None,
            },
            Self::Vdpau => match codec {
                "h264" => Some("h264_vdpau".to_string()),
                "h265" => Some("hevc_vdpau".to_string()),
                _ => None,
            },
        }
    }
}

/// Detects available hardware acceleration on the system.
#[must_use]
pub fn detect_available_hw_accel() -> Vec<HwAccelType> {
    let mut available = vec![HwAccelType::None];

    for accel_type in &[
        HwAccelType::Nvenc,
        HwAccelType::Qsv,
        HwAccelType::Amd,
        HwAccelType::VideoToolbox,
        HwAccelType::Vulkan,
        HwAccelType::D3d11,
        HwAccelType::Vaapi,
        HwAccelType::Vdpau,
    ] {
        if accel_type.is_available() {
            available.push(*accel_type);
        }
    }

    available
}

/// Detects the best hardware acceleration for a given codec.
#[must_use]
pub fn detect_best_hw_accel_for_codec(codec: &str) -> Option<HwAccelType> {
    detect_available_hw_accel()
        .into_iter()
        .find(|&accel_type| accel_type.supported_codecs().contains(&codec))
}

// Platform-specific detection functions
#[cfg(target_os = "linux")]
fn detect_nvenc() -> bool {
    std::path::Path::new("/dev/nvidia0").exists()
}

#[cfg(not(target_os = "linux"))]
fn detect_nvenc() -> bool {
    false // Would use platform-specific detection on Windows
}

#[cfg(target_os = "linux")]
fn detect_vaapi() -> bool {
    std::path::Path::new("/dev/dri/renderD128").exists()
}

#[cfg(not(target_os = "linux"))]
fn detect_vaapi() -> bool {
    false
}

#[cfg(target_os = "linux")]
fn detect_vdpau() -> bool {
    std::path::Path::new("/usr/lib/vdpau").exists()
}

#[cfg(not(target_os = "linux"))]
fn detect_vdpau() -> bool {
    false
}

#[cfg(target_os = "windows")]
fn detect_qsv() -> bool {
    // Would check for Intel GPU
    false
}

#[cfg(not(target_os = "windows"))]
fn detect_qsv() -> bool {
    false
}

#[cfg(target_os = "windows")]
fn detect_amd() -> bool {
    // Would check for AMD GPU
    false
}

#[cfg(not(target_os = "windows"))]
fn detect_amd() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn detect_videotoolbox() -> bool {
    true // Available on all modern Macs
}

#[cfg(not(target_os = "macos"))]
fn detect_videotoolbox() -> bool {
    false
}

#[cfg(target_os = "windows")]
fn detect_d3d11() -> bool {
    true // Available on Windows 7+
}

#[cfg(not(target_os = "windows"))]
fn detect_d3d11() -> bool {
    false
}

fn detect_vulkan() -> bool {
    // Would check for Vulkan support
    false
}

impl HwEncoder {
    /// Creates a new hardware encoder info.
    #[must_use]
    pub fn new(
        accel_type: HwAccelType,
        codec: impl Into<String>,
        encoder_name: impl Into<String>,
    ) -> Self {
        Self {
            accel_type,
            codec: codec.into(),
            encoder_name: encoder_name.into(),
            available: false,
            max_resolution: (7680, 4320), // 8K
            features: Vec::new(),
        }
    }

    /// Sets whether the encoder is available.
    #[must_use]
    pub fn available(mut self, available: bool) -> Self {
        self.available = available;
        self
    }

    /// Sets the maximum resolution.
    #[must_use]
    pub fn max_resolution(mut self, width: u32, height: u32) -> Self {
        self.max_resolution = (width, height);
        self
    }

    /// Adds a supported feature.
    #[must_use]
    pub fn with_feature(mut self, feature: HwFeature) -> Self {
        self.features.push(feature);
        self
    }

    /// Checks if a feature is supported.
    #[must_use]
    pub fn supports_feature(&self, feature: HwFeature) -> bool {
        self.features.contains(&feature)
    }
}

/// Gets information about all available hardware encoders.
#[must_use]
pub fn get_available_encoders() -> Vec<HwEncoder> {
    let mut encoders = Vec::new();

    for accel_type in detect_available_hw_accel() {
        for codec in accel_type.supported_codecs() {
            if let Some(encoder_name) = accel_type.encoder_name(codec) {
                let encoder = HwEncoder::new(accel_type, codec, encoder_name).available(true);
                encoders.push(encoder);
            }
        }
    }

    encoders
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hw_accel_type_platform_name() {
        assert_eq!(HwAccelType::Nvenc.platform_name(), "NVIDIA NVENC");
        assert_eq!(HwAccelType::Qsv.platform_name(), "Intel Quick Sync");
        assert_eq!(HwAccelType::Vaapi.platform_name(), "VAAPI");
    }

    #[test]
    fn test_hw_accel_supported_codecs() {
        let codecs = HwAccelType::Nvenc.supported_codecs();
        assert!(codecs.contains(&"h264"));
        assert!(codecs.contains(&"h265"));
    }

    #[test]
    fn test_hw_accel_encoder_name() {
        assert_eq!(
            HwAccelType::Nvenc.encoder_name("h264"),
            Some("h264_nvenc".to_string())
        );
        assert_eq!(
            HwAccelType::Qsv.encoder_name("h265"),
            Some("hevc_qsv".to_string())
        );
    }

    #[test]
    fn test_hw_accel_config() {
        let config = HwAccelConfig::new(HwAccelType::Nvenc)
            .allow_fallback(false)
            .decode(true)
            .encode(true)
            .device_id(0);

        assert_eq!(config.preferred_type, HwAccelType::Nvenc);
        assert!(!config.allow_fallback);
        assert!(config.decode);
        assert!(config.encode);
        assert_eq!(config.device_id, Some(0));
    }

    #[test]
    fn test_detect_available_hw_accel() {
        let available = detect_available_hw_accel();
        assert!(available.contains(&HwAccelType::None)); // Always available
    }

    #[test]
    fn test_hw_encoder_creation() {
        let encoder = HwEncoder::new(HwAccelType::Nvenc, "h264", "h264_nvenc")
            .available(true)
            .max_resolution(3840, 2160)
            .with_feature(HwFeature::TenBit)
            .with_feature(HwFeature::Lookahead);

        assert_eq!(encoder.accel_type, HwAccelType::Nvenc);
        assert_eq!(encoder.codec, "h264");
        assert!(encoder.available);
        assert_eq!(encoder.max_resolution, (3840, 2160));
        assert!(encoder.supports_feature(HwFeature::TenBit));
        assert!(encoder.supports_feature(HwFeature::Lookahead));
        assert!(!encoder.supports_feature(HwFeature::Hdr));
    }
}
