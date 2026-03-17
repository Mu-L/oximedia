#![allow(dead_code)]
//! Extended farm configuration and profile management.
//!
//! Provides a layered configuration model on top of
//! [`crate::CoordinatorConfig`] / [`crate::WorkerConfig`] with support for
//! named encoding profiles, resource quotas, and environment-specific
//! overrides. All structs are pure Rust with no additional dependencies
//! beyond what the crate already uses.

use std::collections::HashMap;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Encoding profile
// ---------------------------------------------------------------------------

/// A named encoding profile describing target codec parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct EncodingProfile {
    /// Human-readable profile name (e.g. "broadcast-hd").
    pub name: String,
    /// Target codec (e.g. "h264", "hevc", "av1").
    pub codec: String,
    /// Container format (e.g. "mp4", "mkv").
    pub container: String,
    /// Target bitrate in kbps (0 = CRF/CQ mode).
    pub bitrate_kbps: u32,
    /// CRF / CQ value (0 = bitrate mode).
    pub crf: u8,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Frame rate numerator.
    pub fps_num: u32,
    /// Frame rate denominator.
    pub fps_den: u32,
    /// Number of encoding passes.
    pub passes: u8,
    /// Arbitrary extra key-value parameters.
    pub extra: HashMap<String, String>,
}

impl Default for EncodingProfile {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            codec: "h264".to_string(),
            container: "mp4".to_string(),
            bitrate_kbps: 5000,
            crf: 0,
            width: 1920,
            height: 1080,
            fps_num: 30,
            fps_den: 1,
            passes: 1,
            extra: HashMap::new(),
        }
    }
}

impl EncodingProfile {
    /// Create a minimal profile with the given name and codec.
    #[must_use]
    pub fn new(name: impl Into<String>, codec: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            codec: codec.into(),
            ..Default::default()
        }
    }

    /// Compute the frame rate as a floating-point value.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn fps(&self) -> f64 {
        if self.fps_den == 0 {
            return 0.0;
        }
        f64::from(self.fps_num) / f64::from(self.fps_den)
    }

    /// Check whether this profile uses CRF / constant-quality mode.
    #[must_use]
    pub fn is_crf_mode(&self) -> bool {
        self.crf > 0 && self.bitrate_kbps == 0
    }
}

// ---------------------------------------------------------------------------
// Resource quota
// ---------------------------------------------------------------------------

/// Resource quota that limits what a single job may consume.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceQuota {
    /// Maximum CPU cores a single job may use.
    pub max_cpu_cores: u32,
    /// Maximum memory in MiB.
    pub max_memory_mib: u64,
    /// Maximum GPU devices.
    pub max_gpus: u32,
    /// Maximum wall-clock duration.
    pub max_wall_time: Duration,
    /// Maximum disk scratch space in MiB.
    pub max_scratch_mib: u64,
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self {
            max_cpu_cores: 4,
            max_memory_mib: 8192,
            max_gpus: 1,
            max_wall_time: Duration::from_secs(3600),
            max_scratch_mib: 10240,
        }
    }
}

impl ResourceQuota {
    /// Create a quota with custom CPU and memory limits.
    #[must_use]
    pub fn new(cpu: u32, mem_mib: u64) -> Self {
        Self {
            max_cpu_cores: cpu,
            max_memory_mib: mem_mib,
            ..Default::default()
        }
    }

    /// Check whether this quota allows the requested resources.
    #[must_use]
    pub fn allows(&self, cpu: u32, mem_mib: u64, gpus: u32) -> bool {
        cpu <= self.max_cpu_cores && mem_mib <= self.max_memory_mib && gpus <= self.max_gpus
    }
}

// ---------------------------------------------------------------------------
// Farm-wide configuration
// ---------------------------------------------------------------------------

/// Farm-wide configuration aggregating profiles, quotas, and policies.
#[derive(Debug, Clone)]
pub struct FarmConfig {
    /// Named encoding profiles.
    pub profiles: HashMap<String, EncodingProfile>,
    /// Named resource quotas.
    pub quotas: HashMap<String, ResourceQuota>,
    /// Default profile name to use when none is specified.
    pub default_profile: String,
    /// Default quota name.
    pub default_quota: String,
    /// Whether to allow jobs to exceed quotas in emergency mode.
    pub emergency_override: bool,
    /// Maximum number of retries for any task in the farm.
    pub global_max_retries: u32,
    /// Grace period after a worker misses a heartbeat before it is
    /// considered dead.
    pub heartbeat_grace: Duration,
}

impl Default for FarmConfig {
    fn default() -> Self {
        let mut profiles = HashMap::new();
        profiles.insert("default".to_string(), EncodingProfile::default());
        let mut quotas = HashMap::new();
        quotas.insert("default".to_string(), ResourceQuota::default());
        Self {
            profiles,
            quotas,
            default_profile: "default".to_string(),
            default_quota: "default".to_string(),
            emergency_override: false,
            global_max_retries: 3,
            heartbeat_grace: Duration::from_secs(90),
        }
    }
}

impl FarmConfig {
    /// Create a new configuration with sensible defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or replace an encoding profile.
    pub fn add_profile(&mut self, profile: EncodingProfile) {
        self.profiles.insert(profile.name.clone(), profile);
    }

    /// Add or replace a resource quota.
    pub fn add_quota(&mut self, name: impl Into<String>, quota: ResourceQuota) {
        self.quotas.insert(name.into(), quota);
    }

    /// Look up an encoding profile by name.
    #[must_use]
    pub fn get_profile(&self, name: &str) -> Option<&EncodingProfile> {
        self.profiles.get(name)
    }

    /// Look up a resource quota by name.
    #[must_use]
    pub fn get_quota(&self, name: &str) -> Option<&ResourceQuota> {
        self.quotas.get(name)
    }

    /// Return the default encoding profile.
    #[must_use]
    pub fn default_profile(&self) -> Option<&EncodingProfile> {
        self.profiles.get(&self.default_profile)
    }

    /// Return the default resource quota.
    #[must_use]
    pub fn default_quota(&self) -> Option<&ResourceQuota> {
        self.quotas.get(&self.default_quota)
    }

    /// Total number of registered profiles.
    #[must_use]
    pub fn profile_count(&self) -> usize {
        self.profiles.len()
    }

    /// Total number of registered quotas.
    #[must_use]
    pub fn quota_count(&self) -> usize {
        self.quotas.len()
    }
}

// ---------------------------------------------------------------------------
// TOML / config-file loading
// ---------------------------------------------------------------------------

/// Intermediate serde types used exclusively for TOML deserialization.
mod toml_schema {
    use serde::Deserialize;
    use std::collections::HashMap;

    #[derive(Debug, Deserialize, Default)]
    pub struct TomlRoot {
        #[serde(default)]
        pub farm: TomlFarm,
        #[serde(default)]
        pub profiles: Vec<TomlProfile>,
        #[serde(default)]
        pub quotas: Vec<TomlQuota>,
    }

    #[derive(Debug, Deserialize, Default)]
    pub struct TomlFarm {
        pub default_profile: Option<String>,
        pub default_quota: Option<String>,
        #[serde(default)]
        pub emergency_override: bool,
        pub global_max_retries: Option<u32>,
        pub heartbeat_grace_secs: Option<u64>,
    }

    #[derive(Debug, Deserialize, Default)]
    pub struct TomlProfile {
        pub name: String,
        #[serde(default = "default_h264")]
        pub codec: String,
        #[serde(default = "default_mp4")]
        pub container: String,
        #[serde(default)]
        pub bitrate_kbps: u32,
        #[serde(default)]
        pub crf: u8,
        #[serde(default = "default_1920")]
        pub width: u32,
        #[serde(default = "default_1080")]
        pub height: u32,
        #[serde(default = "default_30")]
        pub fps_num: u32,
        #[serde(default = "default_1")]
        pub fps_den: u32,
        #[serde(default = "default_1u8")]
        pub passes: u8,
        #[serde(default)]
        pub extra: HashMap<String, String>,
    }

    #[derive(Debug, Deserialize, Default)]
    pub struct TomlQuota {
        pub name: String,
        #[serde(default = "default_4")]
        pub max_cpu_cores: u32,
        #[serde(default = "default_8192")]
        pub max_memory_mib: u64,
        #[serde(default = "default_1")]
        pub max_gpus: u32,
        #[serde(default = "default_3600")]
        pub max_wall_time_secs: u64,
        #[serde(default = "default_10240")]
        pub max_scratch_mib: u64,
    }

    fn default_h264() -> String {
        "h264".to_string()
    }
    fn default_mp4() -> String {
        "mp4".to_string()
    }
    fn default_1920() -> u32 {
        1920
    }
    fn default_1080() -> u32 {
        1080
    }
    fn default_30() -> u32 {
        30
    }
    fn default_1() -> u32 {
        1
    }
    fn default_1u8() -> u8 {
        1
    }
    fn default_4() -> u32 {
        4
    }
    fn default_8192() -> u64 {
        8192
    }
    fn default_3600() -> u64 {
        3600
    }
    fn default_10240() -> u64 {
        10240
    }
}

/// Load a [`FarmConfig`] from a TOML string.
///
/// The expected schema is:
///
/// ```toml
/// [farm]
/// default_profile = "hd-h264"
/// default_quota = "standard"
/// emergency_override = false
/// global_max_retries = 3
/// heartbeat_grace_secs = 90
///
/// [[profiles]]
/// name = "hd-h264"
/// codec = "h264"
/// container = "mp4"
/// bitrate_kbps = 8000
/// crf = 0
/// width = 1920
/// height = 1080
/// fps_num = 30
/// fps_den = 1
/// passes = 1
///
/// [[quotas]]
/// name = "standard"
/// max_cpu_cores = 8
/// max_memory_mib = 16384
/// max_gpus = 1
/// max_wall_time_secs = 3600
/// max_scratch_mib = 20480
/// ```
///
/// All fields in `[[profiles]]` and `[[quotas]]` except `name` are optional
/// and fall back to the same defaults as [`EncodingProfile::default`] and
/// [`ResourceQuota::default`].
///
/// # Errors
///
/// Returns [`crate::FarmError::InvalidConfig`] when the TOML cannot be parsed
/// or contains structurally invalid data.
pub fn load_farm_config_from_toml(toml_str: &str) -> crate::Result<FarmConfig> {
    use std::time::Duration;
    use toml_schema::TomlRoot;

    let root: TomlRoot = toml::from_str(toml_str)
        .map_err(|e| crate::FarmError::InvalidConfig(format!("TOML parse error: {e}")))?;

    let mut cfg = FarmConfig::new();

    // ── [farm] section ────────────────────────────────────────────────────────
    let farm = &root.farm;
    if let Some(ref dp) = farm.default_profile {
        cfg.default_profile = dp.clone();
    }
    if let Some(ref dq) = farm.default_quota {
        cfg.default_quota = dq.clone();
    }
    cfg.emergency_override = farm.emergency_override;
    if let Some(retries) = farm.global_max_retries {
        cfg.global_max_retries = retries;
    }
    if let Some(grace) = farm.heartbeat_grace_secs {
        cfg.heartbeat_grace = Duration::from_secs(grace);
    }

    // ── [[profiles]] section ──────────────────────────────────────────────────
    for tp in &root.profiles {
        if tp.name.is_empty() {
            return Err(crate::FarmError::InvalidConfig(
                "profile entry missing required 'name' field".to_string(),
            ));
        }
        let profile = EncodingProfile {
            name: tp.name.clone(),
            codec: tp.codec.clone(),
            container: tp.container.clone(),
            bitrate_kbps: tp.bitrate_kbps,
            crf: tp.crf,
            width: tp.width,
            height: tp.height,
            fps_num: tp.fps_num,
            fps_den: tp.fps_den,
            passes: tp.passes,
            extra: tp.extra.clone(),
        };
        cfg.add_profile(profile);
    }

    // ── [[quotas]] section ────────────────────────────────────────────────────
    for tq in &root.quotas {
        if tq.name.is_empty() {
            return Err(crate::FarmError::InvalidConfig(
                "quota entry missing required 'name' field".to_string(),
            ));
        }
        let quota = ResourceQuota {
            max_cpu_cores: tq.max_cpu_cores,
            max_memory_mib: tq.max_memory_mib,
            max_gpus: tq.max_gpus,
            max_wall_time: Duration::from_secs(tq.max_wall_time_secs),
            max_scratch_mib: tq.max_scratch_mib,
        };
        cfg.add_quota(tq.name.clone(), quota);
    }

    Ok(cfg)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoding_profile_default() {
        let p = EncodingProfile::default();
        assert_eq!(p.codec, "h264");
        assert_eq!(p.width, 1920);
        assert_eq!(p.height, 1080);
    }

    #[test]
    fn test_encoding_profile_fps() {
        let p = EncodingProfile {
            fps_num: 24000,
            fps_den: 1001,
            ..Default::default()
        };
        let fps = p.fps();
        assert!((fps - 23.976).abs() < 0.01);
    }

    #[test]
    fn test_encoding_profile_fps_zero_den() {
        let p = EncodingProfile {
            fps_den: 0,
            ..Default::default()
        };
        assert!((p.fps() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_is_crf_mode() {
        let mut p = EncodingProfile::default();
        p.crf = 23;
        p.bitrate_kbps = 0;
        assert!(p.is_crf_mode());
    }

    #[test]
    fn test_is_not_crf_mode() {
        let p = EncodingProfile::default(); // bitrate > 0, crf = 0
        assert!(!p.is_crf_mode());
    }

    #[test]
    fn test_resource_quota_allows() {
        let q = ResourceQuota::new(8, 16384);
        assert!(q.allows(4, 8192, 1));
        assert!(!q.allows(16, 8192, 1));
        assert!(!q.allows(4, 32768, 1));
    }

    #[test]
    fn test_farm_config_default() {
        let cfg = FarmConfig::new();
        assert_eq!(cfg.profile_count(), 1);
        assert_eq!(cfg.quota_count(), 1);
        assert!(cfg.default_profile().is_some());
        assert!(cfg.default_quota().is_some());
    }

    #[test]
    fn test_add_profile() {
        let mut cfg = FarmConfig::new();
        cfg.add_profile(EncodingProfile::new("4k-hevc", "hevc"));
        assert_eq!(cfg.profile_count(), 2);
        let p = cfg
            .get_profile("4k-hevc")
            .expect("get_profile should succeed");
        assert_eq!(p.codec, "hevc");
    }

    #[test]
    fn test_add_quota() {
        let mut cfg = FarmConfig::new();
        cfg.add_quota("heavy", ResourceQuota::new(32, 65536));
        assert_eq!(cfg.quota_count(), 2);
        let q = cfg.get_quota("heavy").expect("get_quota should succeed");
        assert_eq!(q.max_cpu_cores, 32);
    }

    #[test]
    fn test_missing_profile() {
        let cfg = FarmConfig::new();
        assert!(cfg.get_profile("nonexistent").is_none());
    }

    #[test]
    fn test_missing_quota() {
        let cfg = FarmConfig::new();
        assert!(cfg.get_quota("nonexistent").is_none());
    }

    #[test]
    fn test_encoding_profile_new() {
        let p = EncodingProfile::new("web", "vp9");
        assert_eq!(p.name, "web");
        assert_eq!(p.codec, "vp9");
        assert_eq!(p.container, "mp4"); // default
    }

    // ── TOML loading ──────────────────────────────────────────────────────────

    #[test]
    fn test_load_farm_config_from_toml_basic() {
        let toml = r#"
[farm]
default_profile = "hd"
default_quota = "std"
emergency_override = true
global_max_retries = 5
heartbeat_grace_secs = 120
"#;
        let cfg = super::load_farm_config_from_toml(toml).expect("should parse");
        assert_eq!(cfg.default_profile, "hd");
        assert_eq!(cfg.default_quota, "std");
        assert!(cfg.emergency_override);
        assert_eq!(cfg.global_max_retries, 5);
        assert_eq!(cfg.heartbeat_grace, std::time::Duration::from_secs(120));
    }

    #[test]
    fn test_load_farm_config_from_toml_with_profiles_and_quotas() {
        let toml = r#"
[farm]
default_profile = "broadcast"
default_quota = "heavy"

[[profiles]]
name = "broadcast"
codec = "hevc"
container = "ts"
bitrate_kbps = 15000
width = 1920
height = 1080
fps_num = 50
fps_den = 1
passes = 2

[[quotas]]
name = "heavy"
max_cpu_cores = 16
max_memory_mib = 32768
max_gpus = 2
max_wall_time_secs = 7200
max_scratch_mib = 51200
"#;
        let cfg = super::load_farm_config_from_toml(toml).expect("should parse");
        // Profiles: the built-in "default" + "broadcast"
        assert_eq!(cfg.profile_count(), 2);
        let p = cfg.get_profile("broadcast").expect("profile should exist");
        assert_eq!(p.codec, "hevc");
        assert_eq!(p.bitrate_kbps, 15000);
        assert_eq!(p.fps_num, 50);
        assert_eq!(p.passes, 2);

        // Quotas: built-in "default" + "heavy"
        assert_eq!(cfg.quota_count(), 2);
        let q = cfg.get_quota("heavy").expect("quota should exist");
        assert_eq!(q.max_cpu_cores, 16);
        assert_eq!(q.max_memory_mib, 32768);
        assert_eq!(q.max_gpus, 2);
    }

    #[test]
    fn test_load_farm_config_from_toml_invalid_returns_error() {
        let bad_toml = "this is [not valid toml!!";
        let result = super::load_farm_config_from_toml(bad_toml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("TOML parse error")
                || err.to_string().contains("Invalid configuration")
        );
    }

    #[test]
    fn test_load_farm_config_from_toml_empty_input_uses_defaults() {
        let cfg = super::load_farm_config_from_toml("").expect("empty TOML should parse");
        // Should have the default profile and quota already present from FarmConfig::new().
        assert_eq!(cfg.profile_count(), 1);
        assert_eq!(cfg.quota_count(), 1);
        assert_eq!(cfg.default_profile, "default");
        assert_eq!(cfg.default_quota, "default");
    }
}
