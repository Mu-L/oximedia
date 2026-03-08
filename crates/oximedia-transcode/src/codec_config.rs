//! Codec-specific configuration and optimization.

use crate::{QualityMode, QualityPreset, RateControlMode, TuneMode};
use serde::{Deserialize, Serialize};

/// Codec-specific configuration.
#[derive(Debug, Clone)]
pub struct CodecConfig {
    /// Codec name.
    pub codec: String,
    /// Preset (speed/quality tradeoff).
    pub preset: QualityPreset,
    /// Tune for specific content.
    pub tune: Option<TuneMode>,
    /// Profile.
    pub profile: Option<String>,
    /// Level.
    pub level: Option<String>,
    /// Rate control mode.
    pub rate_control: RateControlMode,
    /// Additional options.
    pub options: Vec<(String, String)>,
}

impl CodecConfig {
    /// Creates a new codec configuration.
    #[must_use]
    pub fn new(codec: impl Into<String>) -> Self {
        Self {
            codec: codec.into(),
            preset: QualityPreset::Medium,
            tune: None,
            profile: None,
            level: None,
            rate_control: RateControlMode::Crf(23),
            options: Vec::new(),
        }
    }

    /// Sets the preset.
    #[must_use]
    pub fn preset(mut self, preset: QualityPreset) -> Self {
        self.preset = preset;
        self
    }

    /// Sets the tune mode.
    #[must_use]
    pub fn tune(mut self, tune: TuneMode) -> Self {
        self.tune = Some(tune);
        self
    }

    /// Sets the profile.
    #[must_use]
    pub fn profile(mut self, profile: impl Into<String>) -> Self {
        self.profile = Some(profile.into());
        self
    }

    /// Sets the level.
    #[must_use]
    pub fn level(mut self, level: impl Into<String>) -> Self {
        self.level = Some(level.into());
        self
    }

    /// Sets the rate control mode.
    #[must_use]
    pub fn rate_control(mut self, mode: RateControlMode) -> Self {
        self.rate_control = mode;
        self
    }

    /// Adds a custom option.
    #[must_use]
    pub fn option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.push((key.into(), value.into()));
        self
    }
}

/// H.264/AVC specific configuration.
#[derive(Debug, Clone)]
pub struct H264Config {
    base: CodecConfig,
}

impl H264Config {
    /// Creates a new H.264 configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            base: CodecConfig::new("h264"),
        }
    }

    /// Sets the profile (baseline, main, high, high10, high422, high444).
    #[must_use]
    pub fn profile(mut self, profile: H264Profile) -> Self {
        self.base.profile = Some(profile.as_str().to_string());
        self
    }

    /// Sets the level (e.g., "3.0", "4.0", "5.1").
    #[must_use]
    pub fn level(mut self, level: impl Into<String>) -> Self {
        self.base.level = Some(level.into());
        self
    }

    /// Enables cabac entropy coding.
    #[must_use]
    pub fn cabac(mut self, enable: bool) -> Self {
        self.base.options.push((
            "cabac".to_string(),
            if enable { "1" } else { "0" }.to_string(),
        ));
        self
    }

    /// Sets the number of reference frames.
    #[must_use]
    pub fn refs(mut self, refs: u8) -> Self {
        self.base
            .options
            .push(("refs".to_string(), refs.to_string()));
        self
    }

    /// Sets the number of B-frames.
    #[must_use]
    pub fn bframes(mut self, bframes: u8) -> Self {
        self.base
            .options
            .push(("bframes".to_string(), bframes.to_string()));
        self
    }

    /// Enables 8x8 DCT.
    #[must_use]
    pub fn dct8x8(mut self, enable: bool) -> Self {
        self.base.options.push((
            "8x8dct".to_string(),
            if enable { "1" } else { "0" }.to_string(),
        ));
        self
    }

    /// Sets the deblocking filter parameters.
    #[must_use]
    pub fn deblock(mut self, alpha: i8, beta: i8) -> Self {
        self.base
            .options
            .push(("deblock".to_string(), format!("{alpha}:{beta}")));
        self
    }

    /// Converts to base codec config.
    #[must_use]
    pub fn build(self) -> CodecConfig {
        self.base
    }
}

impl Default for H264Config {
    fn default() -> Self {
        Self::new()
    }
}

/// H.264 profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H264Profile {
    /// Baseline profile.
    Baseline,
    /// Main profile.
    Main,
    /// High profile.
    High,
    /// High 10 profile (10-bit).
    High10,
    /// High 4:2:2 profile.
    High422,
    /// High 4:4:4 profile.
    High444,
}

impl H264Profile {
    #[must_use]
    fn as_str(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Main => "main",
            Self::High => "high",
            Self::High10 => "high10",
            Self::High422 => "high422",
            Self::High444 => "high444",
        }
    }
}

/// VP9 specific configuration.
#[derive(Debug, Clone)]
pub struct Vp9Config {
    base: CodecConfig,
}

impl Vp9Config {
    /// Creates a new VP9 configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            base: CodecConfig::new("vp9"),
        }
    }

    /// Sets the CPU used (0-8, lower = slower/better).
    #[must_use]
    pub fn cpu_used(mut self, cpu_used: u8) -> Self {
        self.base
            .options
            .push(("cpu-used".to_string(), cpu_used.to_string()));
        self
    }

    /// Sets the tile columns (for parallel encoding).
    #[must_use]
    pub fn tile_columns(mut self, columns: u8) -> Self {
        self.base
            .options
            .push(("tile-columns".to_string(), columns.to_string()));
        self
    }

    /// Sets the tile rows (for parallel encoding).
    #[must_use]
    pub fn tile_rows(mut self, rows: u8) -> Self {
        self.base
            .options
            .push(("tile-rows".to_string(), rows.to_string()));
        self
    }

    /// Sets the frame parallel encoding.
    #[must_use]
    pub fn frame_parallel(mut self, enable: bool) -> Self {
        self.base.options.push((
            "frame-parallel".to_string(),
            if enable { "1" } else { "0" }.to_string(),
        ));
        self
    }

    /// Sets the auto alt reference frames.
    #[must_use]
    pub fn auto_alt_ref(mut self, frames: u8) -> Self {
        self.base
            .options
            .push(("auto-alt-ref".to_string(), frames.to_string()));
        self
    }

    /// Sets the lag in frames.
    #[must_use]
    pub fn lag_in_frames(mut self, lag: u32) -> Self {
        self.base
            .options
            .push(("lag-in-frames".to_string(), lag.to_string()));
        self
    }

    /// Enables row-based multi-threading.
    #[must_use]
    pub fn row_mt(mut self, enable: bool) -> Self {
        self.base.options.push((
            "row-mt".to_string(),
            if enable { "1" } else { "0" }.to_string(),
        ));
        self
    }

    /// Converts to base codec config.
    #[must_use]
    pub fn build(self) -> CodecConfig {
        self.base
    }
}

impl Default for Vp9Config {
    fn default() -> Self {
        Self::new()
    }
}

/// AV1 specific configuration.
#[derive(Debug, Clone)]
pub struct Av1Config {
    base: CodecConfig,
}

impl Av1Config {
    /// Creates a new AV1 configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            base: CodecConfig::new("av1"),
        }
    }

    /// Sets the CPU used (0-8, lower = slower/better).
    #[must_use]
    pub fn cpu_used(mut self, cpu_used: u8) -> Self {
        self.base
            .options
            .push(("cpu-used".to_string(), cpu_used.to_string()));
        self
    }

    /// Sets the tile columns.
    #[must_use]
    pub fn tiles(mut self, columns: u8, rows: u8) -> Self {
        self.base
            .options
            .push(("tiles".to_string(), format!("{columns}x{rows}")));
        self
    }

    /// Enables row-based multi-threading.
    #[must_use]
    pub fn row_mt(mut self, enable: bool) -> Self {
        self.base.options.push((
            "row-mt".to_string(),
            if enable { "1" } else { "0" }.to_string(),
        ));
        self
    }

    /// Sets the usage mode (good, realtime).
    #[must_use]
    pub fn usage(mut self, usage: Av1Usage) -> Self {
        self.base
            .options
            .push(("usage".to_string(), usage.as_str().to_string()));
        self
    }

    /// Enables film grain synthesis.
    #[must_use]
    pub fn enable_film_grain(mut self, enable: bool) -> Self {
        self.base.options.push((
            "enable-film-grain".to_string(),
            if enable { "1" } else { "0" }.to_string(),
        ));
        self
    }

    /// Converts to base codec config.
    #[must_use]
    pub fn build(self) -> CodecConfig {
        self.base
    }
}

impl Default for Av1Config {
    fn default() -> Self {
        Self::new()
    }
}

/// AV1 usage modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Av1Usage {
    /// Good quality mode.
    Good,
    /// Real-time mode.
    Realtime,
}

impl Av1Usage {
    #[must_use]
    fn as_str(self) -> &'static str {
        match self {
            Self::Good => "good",
            Self::Realtime => "realtime",
        }
    }
}

/// Opus audio codec configuration.
#[derive(Debug, Clone)]
pub struct OpusConfig {
    base: CodecConfig,
}

impl OpusConfig {
    /// Creates a new Opus configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            base: CodecConfig::new("opus"),
        }
    }

    /// Sets the application type.
    #[must_use]
    pub fn application(mut self, app: OpusApplication) -> Self {
        self.base
            .options
            .push(("application".to_string(), app.as_str().to_string()));
        self
    }

    /// Sets the complexity (0-10).
    #[must_use]
    pub fn complexity(mut self, complexity: u8) -> Self {
        self.base
            .options
            .push(("complexity".to_string(), complexity.to_string()));
        self
    }

    /// Sets the frame duration in milliseconds.
    #[must_use]
    pub fn frame_duration(mut self, duration_ms: f32) -> Self {
        self.base
            .options
            .push(("frame_duration".to_string(), duration_ms.to_string()));
        self
    }

    /// Enables variable bitrate.
    #[must_use]
    pub fn vbr(mut self, enable: bool) -> Self {
        self.base.options.push((
            "vbr".to_string(),
            if enable { "on" } else { "off" }.to_string(),
        ));
        self
    }

    /// Converts to base codec config.
    #[must_use]
    pub fn build(self) -> CodecConfig {
        self.base
    }
}

impl Default for OpusConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Opus application types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpusApplication {
    /// Voice over IP.
    Voip,
    /// Audio streaming.
    Audio,
    /// Low delay.
    LowDelay,
}

impl OpusApplication {
    #[must_use]
    fn as_str(self) -> &'static str {
        match self {
            Self::Voip => "voip",
            Self::Audio => "audio",
            Self::LowDelay => "lowdelay",
        }
    }
}

/// Creates codec configuration from quality mode.
#[must_use]
pub fn codec_config_from_quality(codec: &str, quality: QualityMode) -> CodecConfig {
    let preset = quality.to_preset();
    let crf = quality.to_crf();

    match codec {
        "h264" => H264Config::new()
            .profile(H264Profile::High)
            .refs(3)
            .bframes(3)
            .build()
            .preset(preset)
            .rate_control(RateControlMode::Crf(crf)),
        "vp9" => Vp9Config::new()
            .cpu_used(preset.cpu_used())
            .row_mt(true)
            .build()
            .preset(preset)
            .rate_control(RateControlMode::Crf(crf)),
        "av1" => Av1Config::new()
            .cpu_used(preset.cpu_used())
            .row_mt(true)
            .usage(Av1Usage::Good)
            .build()
            .preset(preset)
            .rate_control(RateControlMode::Crf(crf)),
        "opus" => OpusConfig::new()
            .application(OpusApplication::Audio)
            .complexity(10)
            .vbr(true)
            .build()
            .preset(preset),
        _ => CodecConfig::new(codec)
            .preset(preset)
            .rate_control(RateControlMode::Crf(crf)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codec_config_new() {
        let config = CodecConfig::new("h264");
        assert_eq!(config.codec, "h264");
        assert_eq!(config.preset, QualityPreset::Medium);
    }

    #[test]
    fn test_h264_config() {
        let config = H264Config::new()
            .profile(H264Profile::High)
            .level("4.0")
            .refs(3)
            .bframes(3)
            .cabac(true)
            .dct8x8(true)
            .build();

        assert_eq!(config.codec, "h264");
        assert_eq!(config.profile, Some("high".to_string()));
        assert_eq!(config.level, Some("4.0".to_string()));
        assert!(config.options.len() > 0);
    }

    #[test]
    fn test_vp9_config() {
        let config = Vp9Config::new()
            .cpu_used(4)
            .tile_columns(2)
            .tile_rows(1)
            .row_mt(true)
            .build();

        assert_eq!(config.codec, "vp9");
        assert!(config
            .options
            .iter()
            .any(|(k, v)| k == "cpu-used" && v == "4"));
        assert!(config
            .options
            .iter()
            .any(|(k, v)| k == "row-mt" && v == "1"));
    }

    #[test]
    fn test_av1_config() {
        let config = Av1Config::new()
            .cpu_used(6)
            .tiles(4, 2)
            .usage(Av1Usage::Good)
            .row_mt(true)
            .build();

        assert_eq!(config.codec, "av1");
        assert!(config
            .options
            .iter()
            .any(|(k, v)| k == "cpu-used" && v == "6"));
        assert!(config
            .options
            .iter()
            .any(|(k, v)| k == "tiles" && v == "4x2"));
    }

    #[test]
    fn test_opus_config() {
        let config = OpusConfig::new()
            .application(OpusApplication::Audio)
            .complexity(10)
            .vbr(true)
            .build();

        assert_eq!(config.codec, "opus");
        assert!(config
            .options
            .iter()
            .any(|(k, v)| k == "application" && v == "audio"));
        assert!(config
            .options
            .iter()
            .any(|(k, v)| k == "complexity" && v == "10"));
    }

    #[test]
    fn test_codec_config_from_quality() {
        let config = codec_config_from_quality("h264", QualityMode::High);
        assert_eq!(config.codec, "h264");
        assert_eq!(config.preset, QualityPreset::Slow);
        assert_eq!(config.rate_control, RateControlMode::Crf(20));
    }

    #[test]
    fn test_h264_profiles() {
        assert_eq!(H264Profile::Baseline.as_str(), "baseline");
        assert_eq!(H264Profile::Main.as_str(), "main");
        assert_eq!(H264Profile::High.as_str(), "high");
    }

    #[test]
    fn test_av1_usage() {
        assert_eq!(Av1Usage::Good.as_str(), "good");
        assert_eq!(Av1Usage::Realtime.as_str(), "realtime");
    }

    #[test]
    fn test_opus_application() {
        assert_eq!(OpusApplication::Voip.as_str(), "voip");
        assert_eq!(OpusApplication::Audio.as_str(), "audio");
        assert_eq!(OpusApplication::LowDelay.as_str(), "lowdelay");
    }
}
