#![allow(dead_code)]
//! Broadcast compliance standard profiles for loudness normalization.
//!
//! Provides pre-configured profiles for major broadcast standards including
//! EBU R128, ATSC A/85, ARIB TR-B32, OP-59, and Free TV Australia.
//! Each profile specifies target loudness, tolerance, true-peak ceiling,
//! and loudness range constraints.

use std::collections::HashMap;
use std::fmt;

/// Region code for broadcast territory identification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BroadcastRegion {
    /// Europe (EBU member states).
    Europe,
    /// North America (US, Canada).
    NorthAmerica,
    /// Japan.
    Japan,
    /// Australia.
    Australia,
    /// South Korea.
    SouthKorea,
    /// Brazil.
    Brazil,
    /// United Kingdom.
    UnitedKingdom,
    /// Global / platform-agnostic.
    Global,
}

impl fmt::Display for BroadcastRegion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Europe => write!(f, "Europe"),
            Self::NorthAmerica => write!(f, "North America"),
            Self::Japan => write!(f, "Japan"),
            Self::Australia => write!(f, "Australia"),
            Self::SouthKorea => write!(f, "South Korea"),
            Self::Brazil => write!(f, "Brazil"),
            Self::UnitedKingdom => write!(f, "United Kingdom"),
            Self::Global => write!(f, "Global"),
        }
    }
}

/// Broadcast standard identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BroadcastStandardId {
    /// EBU R128 (European Broadcasting Union).
    EbuR128,
    /// ATSC A/85 (US broadcast).
    AtscA85,
    /// ARIB TR-B32 (Japan broadcast).
    AribTrB32,
    /// OP-59 (Australia Free TV).
    Op59,
    /// ITU-R BS.1770-4 base measurement standard.
    ItuBs1770,
    /// AGCOM 219/09 (Italy).
    AgcomItaly,
    /// CSA (Canada broadcast).
    CsaCanada,
    /// Portaria 354 (Brazil).
    PortariaBrazil,
    /// Disney+ streaming platform.
    DisneyPlus,
    /// Amazon Prime Video streaming platform.
    PrimeVideo,
    /// Apple Spatial Audio / Spatial Audio binaural headphone target.
    AppleSpatialAudio,
    /// Dolby Atmos immersive audio standard (dialog-gated).
    DolbyAtmos,
    /// TikTok short-form video platform.
    TikTok,
    /// Facebook / Instagram social video platforms.
    FacebookInstagram,
    /// Custom user-defined profile.
    Custom,
}

impl fmt::Display for BroadcastStandardId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EbuR128 => write!(f, "EBU R128"),
            Self::AtscA85 => write!(f, "ATSC A/85"),
            Self::AribTrB32 => write!(f, "ARIB TR-B32"),
            Self::Op59 => write!(f, "OP-59"),
            Self::ItuBs1770 => write!(f, "ITU-R BS.1770-4"),
            Self::AgcomItaly => write!(f, "AGCOM 219/09"),
            Self::CsaCanada => write!(f, "CSA Canada"),
            Self::PortariaBrazil => write!(f, "Portaria 354"),
            Self::DisneyPlus => write!(f, "Disney+"),
            Self::PrimeVideo => write!(f, "Prime Video"),
            Self::AppleSpatialAudio => write!(f, "Apple Spatial Audio"),
            Self::DolbyAtmos => write!(f, "Dolby Atmos"),
            Self::TikTok => write!(f, "TikTok"),
            Self::FacebookInstagram => write!(f, "Facebook/Instagram"),
            Self::Custom => write!(f, "Custom"),
        }
    }
}

/// Gating mode for loudness measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatingMode {
    /// Absolute gating only (-70 LUFS).
    AbsoluteOnly,
    /// Relative gating only (-10 LU below ungated).
    RelativeOnly,
    /// Both absolute and relative gating (EBU R128 default).
    Both,
    /// No gating (raw integration).
    None,
}

/// Loudness range constraint for a broadcast standard.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoudnessRangeConstraint {
    /// Maximum allowed LRA in LU. `None` means no limit.
    pub max_lra_lu: Option<f64>,
    /// Recommended LRA range minimum in LU.
    pub recommended_min_lu: f64,
    /// Recommended LRA range maximum in LU.
    pub recommended_max_lu: f64,
}

impl LoudnessRangeConstraint {
    /// Create a new loudness range constraint.
    pub fn new(max_lra_lu: Option<f64>, recommended_min_lu: f64, recommended_max_lu: f64) -> Self {
        Self {
            max_lra_lu,
            recommended_min_lu,
            recommended_max_lu,
        }
    }

    /// Check if a given LRA value is within the recommended range.
    pub fn is_recommended(&self, lra_lu: f64) -> bool {
        lra_lu >= self.recommended_min_lu && lra_lu <= self.recommended_max_lu
    }

    /// Check if a given LRA value is within the maximum allowed range.
    pub fn is_compliant(&self, lra_lu: f64) -> bool {
        match self.max_lra_lu {
            Some(max) => lra_lu <= max,
            None => true,
        }
    }
}

/// Complete broadcast standard profile specification.
#[derive(Debug, Clone, PartialEq)]
pub struct BroadcastStandardProfile {
    /// Standard identifier.
    pub id: BroadcastStandardId,
    /// Human-readable name.
    pub name: String,
    /// Applicable region.
    pub region: BroadcastRegion,
    /// Target integrated loudness in LUFS.
    pub target_lufs: f64,
    /// Tolerance above target in LU.
    pub tolerance_above_lu: f64,
    /// Tolerance below target in LU.
    pub tolerance_below_lu: f64,
    /// Maximum true peak level in dBTP.
    pub max_true_peak_dbtp: f64,
    /// Gating mode for measurement.
    pub gating_mode: GatingMode,
    /// Loudness range constraint.
    pub lra_constraint: Option<LoudnessRangeConstraint>,
    /// Minimum measurement duration in seconds.
    pub min_measurement_duration_s: f64,
    /// Whether dialogue normalization metadata (dialnorm) is required.
    pub requires_dialnorm: bool,
    /// Description of the standard.
    pub description: String,
    /// Optional target dialogue level in LUFS (used by Dolby Atmos and dialog-gated standards).
    /// When `None`, no separate dialogue target is specified.
    pub dialogue_level: Option<f32>,
}

impl BroadcastStandardProfile {
    /// Create a new broadcast standard profile.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: BroadcastStandardId,
        name: &str,
        region: BroadcastRegion,
        target_lufs: f64,
        tolerance_above_lu: f64,
        tolerance_below_lu: f64,
        max_true_peak_dbtp: f64,
    ) -> Self {
        Self {
            id,
            name: name.to_string(),
            region,
            target_lufs,
            tolerance_above_lu,
            tolerance_below_lu,
            max_true_peak_dbtp,
            gating_mode: GatingMode::Both,
            lra_constraint: None,
            min_measurement_duration_s: 0.0,
            requires_dialnorm: false,
            description: String::new(),
            dialogue_level: None,
        }
    }

    /// Set the dialogue level (e.g. for Dolby Atmos dialog-gated standards).
    pub fn with_dialogue_level(mut self, level_lufs: f32) -> Self {
        self.dialogue_level = Some(level_lufs);
        self
    }

    /// Set the LRA constraint.
    pub fn with_lra_constraint(mut self, constraint: LoudnessRangeConstraint) -> Self {
        self.lra_constraint = Some(constraint);
        self
    }

    /// Set the gating mode.
    pub fn with_gating_mode(mut self, mode: GatingMode) -> Self {
        self.gating_mode = mode;
        self
    }

    /// Set whether dialnorm metadata is required.
    pub fn with_dialnorm(mut self, required: bool) -> Self {
        self.requires_dialnorm = required;
        self
    }

    /// Set the description.
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    /// Set minimum measurement duration.
    pub fn with_min_duration(mut self, seconds: f64) -> Self {
        self.min_measurement_duration_s = seconds;
        self
    }

    /// Check if a measured loudness value is compliant.
    pub fn is_loudness_compliant(&self, measured_lufs: f64) -> bool {
        let above = measured_lufs - self.target_lufs;
        above <= self.tolerance_above_lu && (-above) <= self.tolerance_below_lu
    }

    /// Check if a measured true peak is compliant.
    pub fn is_true_peak_compliant(&self, measured_dbtp: f64) -> bool {
        measured_dbtp <= self.max_true_peak_dbtp
    }

    /// Full compliance check for loudness, true peak, and LRA.
    pub fn check_compliance(
        &self,
        measured_lufs: f64,
        measured_dbtp: f64,
        measured_lra: Option<f64>,
    ) -> ComplianceResult {
        let loudness_ok = self.is_loudness_compliant(measured_lufs);
        let peak_ok = self.is_true_peak_compliant(measured_dbtp);
        let lra_ok = match (measured_lra, &self.lra_constraint) {
            (Some(lra), Some(constraint)) => constraint.is_compliant(lra),
            _ => true,
        };

        let loudness_deviation = measured_lufs - self.target_lufs;
        let peak_headroom = self.max_true_peak_dbtp - measured_dbtp;

        ComplianceResult {
            overall_pass: loudness_ok && peak_ok && lra_ok,
            loudness_pass: loudness_ok,
            peak_pass: peak_ok,
            lra_pass: lra_ok,
            loudness_deviation_lu: loudness_deviation,
            peak_headroom_db: peak_headroom,
            measured_lufs,
            measured_dbtp,
            measured_lra,
            standard_id: self.id,
        }
    }

    /// Calculate gain adjustment needed to meet target loudness.
    pub fn gain_to_target(&self, measured_lufs: f64) -> f64 {
        self.target_lufs - measured_lufs
    }
}

/// Result of a compliance check against a broadcast standard.
#[derive(Debug, Clone, PartialEq)]
pub struct ComplianceResult {
    /// Whether all checks passed.
    pub overall_pass: bool,
    /// Whether loudness is within tolerance.
    pub loudness_pass: bool,
    /// Whether true peak is within limit.
    pub peak_pass: bool,
    /// Whether LRA is within limit.
    pub lra_pass: bool,
    /// Deviation from target loudness in LU.
    pub loudness_deviation_lu: f64,
    /// Headroom below true peak ceiling in dB.
    pub peak_headroom_db: f64,
    /// Measured integrated loudness.
    pub measured_lufs: f64,
    /// Measured true peak.
    pub measured_dbtp: f64,
    /// Measured LRA (if available).
    pub measured_lra: Option<f64>,
    /// Standard used for check.
    pub standard_id: BroadcastStandardId,
}

impl fmt::Display for ComplianceResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.overall_pass { "PASS" } else { "FAIL" };
        write!(
            f,
            "[{}] {} | Loudness: {:.1} LUFS (dev: {:+.1} LU) | Peak: {:.1} dBTP (headroom: {:.1} dB)",
            status, self.standard_id, self.measured_lufs, self.loudness_deviation_lu,
            self.measured_dbtp, self.peak_headroom_db
        )
    }
}

/// Registry of broadcast standard profiles.
#[derive(Debug, Clone)]
pub struct BroadcastStandardRegistry {
    /// Map of standard ID to profile.
    profiles: HashMap<BroadcastStandardId, BroadcastStandardProfile>,
}

impl BroadcastStandardRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
        }
    }

    /// Create a registry pre-loaded with all built-in standards.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Self::ebu_r128());
        registry.register(Self::atsc_a85());
        registry.register(Self::arib_tr_b32());
        registry.register(Self::op59());
        registry.register(Self::agcom_italy());
        registry.register(Self::csa_canada());
        registry.register(Self::portaria_brazil());
        registry.register(Self::disney_plus());
        registry.register(Self::prime_video());
        registry.register(Self::apple_spatial_audio());
        registry.register(Self::dolby_atmos());
        registry.register(Self::tiktok());
        registry.register(Self::facebook_instagram());
        registry
    }

    /// Register a profile.
    pub fn register(&mut self, profile: BroadcastStandardProfile) {
        self.profiles.insert(profile.id, profile);
    }

    /// Get a profile by ID.
    pub fn get(&self, id: BroadcastStandardId) -> Option<&BroadcastStandardProfile> {
        self.profiles.get(&id)
    }

    /// Get all profiles for a region.
    pub fn get_by_region(&self, region: BroadcastRegion) -> Vec<&BroadcastStandardProfile> {
        self.profiles
            .values()
            .filter(|p| p.region == region)
            .collect()
    }

    /// Get all registered profile IDs.
    pub fn list_ids(&self) -> Vec<BroadcastStandardId> {
        self.profiles.keys().copied().collect()
    }

    /// Number of registered profiles.
    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    /// EBU R128 standard profile.
    pub fn ebu_r128() -> BroadcastStandardProfile {
        BroadcastStandardProfile::new(
            BroadcastStandardId::EbuR128,
            "EBU R128",
            BroadcastRegion::Europe,
            -23.0,
            1.0,
            1.0,
            -1.0,
        )
        .with_gating_mode(GatingMode::Both)
        .with_lra_constraint(LoudnessRangeConstraint::new(Some(20.0), 5.0, 15.0))
        .with_description("European broadcast loudness normalization standard")
    }

    /// ATSC A/85 standard profile.
    pub fn atsc_a85() -> BroadcastStandardProfile {
        BroadcastStandardProfile::new(
            BroadcastStandardId::AtscA85,
            "ATSC A/85",
            BroadcastRegion::NorthAmerica,
            -24.0,
            2.0,
            2.0,
            -2.0,
        )
        .with_gating_mode(GatingMode::Both)
        .with_dialnorm(true)
        .with_description("US broadcast loudness standard with dialnorm metadata")
    }

    /// ARIB TR-B32 standard profile.
    pub fn arib_tr_b32() -> BroadcastStandardProfile {
        BroadcastStandardProfile::new(
            BroadcastStandardId::AribTrB32,
            "ARIB TR-B32",
            BroadcastRegion::Japan,
            -24.0,
            1.0,
            1.0,
            -1.0,
        )
        .with_gating_mode(GatingMode::Both)
        .with_description("Japan broadcast loudness standard")
    }

    /// OP-59 (Free TV Australia) standard profile.
    pub fn op59() -> BroadcastStandardProfile {
        BroadcastStandardProfile::new(
            BroadcastStandardId::Op59,
            "OP-59",
            BroadcastRegion::Australia,
            -24.0,
            1.0,
            1.0,
            -2.0,
        )
        .with_gating_mode(GatingMode::Both)
        .with_lra_constraint(LoudnessRangeConstraint::new(Some(20.0), 5.0, 15.0))
        .with_description("Australia Free TV loudness standard")
    }

    /// AGCOM Italy standard profile.
    pub fn agcom_italy() -> BroadcastStandardProfile {
        BroadcastStandardProfile::new(
            BroadcastStandardId::AgcomItaly,
            "AGCOM 219/09",
            BroadcastRegion::Europe,
            -24.0,
            1.0,
            1.0,
            -2.0,
        )
        .with_gating_mode(GatingMode::Both)
        .with_description("Italian broadcast loudness regulation")
    }

    /// CSA Canada standard profile.
    pub fn csa_canada() -> BroadcastStandardProfile {
        BroadcastStandardProfile::new(
            BroadcastStandardId::CsaCanada,
            "CSA Canada",
            BroadcastRegion::NorthAmerica,
            -24.0,
            2.0,
            2.0,
            -2.0,
        )
        .with_gating_mode(GatingMode::Both)
        .with_description("Canadian broadcast loudness standard")
    }

    /// Portaria 354 (Brazil) standard profile.
    pub fn portaria_brazil() -> BroadcastStandardProfile {
        BroadcastStandardProfile::new(
            BroadcastStandardId::PortariaBrazil,
            "Portaria 354",
            BroadcastRegion::Brazil,
            -23.0,
            1.0,
            1.0,
            -2.0,
        )
        .with_gating_mode(GatingMode::Both)
        .with_description("Brazilian broadcast loudness standard")
    }

    /// Disney+ streaming platform profile.
    ///
    /// Target: -24 LUFS integrated, -1.0 dBTP true peak, LRA ≤ 18 LU.
    pub fn disney_plus() -> BroadcastStandardProfile {
        BroadcastStandardProfile::new(
            BroadcastStandardId::DisneyPlus,
            "Disney+",
            BroadcastRegion::Global,
            -24.0,
            1.0,
            1.0,
            -1.0,
        )
        .with_gating_mode(GatingMode::Both)
        .with_lra_constraint(LoudnessRangeConstraint::new(Some(18.0), 5.0, 15.0))
        .with_description(
            "Disney+ streaming loudness target: -24 LUFS integrated, LRA ≤ 18 LU, -1 dBTP true peak",
        )
    }

    /// Amazon Prime Video streaming platform profile.
    ///
    /// Target: -14 LUFS integrated, -1.0 dBTP true peak, LRA ≤ 20 LU.
    pub fn prime_video() -> BroadcastStandardProfile {
        BroadcastStandardProfile::new(
            BroadcastStandardId::PrimeVideo,
            "Prime Video",
            BroadcastRegion::Global,
            -14.0,
            1.0,
            1.0,
            -1.0,
        )
        .with_gating_mode(GatingMode::Both)
        .with_lra_constraint(LoudnessRangeConstraint::new(Some(20.0), 5.0, 15.0))
        .with_description(
            "Amazon Prime Video loudness target: -14 LUFS integrated, LRA ≤ 20 LU, -1 dBTP true peak",
        )
    }

    /// Apple Spatial Audio binaural headphone target profile.
    ///
    /// Target: -16 LUFS integrated, -1.5 dBTP true peak, LRA ≤ 15 LU.
    /// Optimised for binaural rendering via Apple's headphone spatialization engine.
    pub fn apple_spatial_audio() -> BroadcastStandardProfile {
        BroadcastStandardProfile::new(
            BroadcastStandardId::AppleSpatialAudio,
            "Apple Spatial Audio",
            BroadcastRegion::Global,
            -16.0,
            1.0,
            1.0,
            -1.5,
        )
        .with_gating_mode(GatingMode::Both)
        .with_lra_constraint(LoudnessRangeConstraint::new(Some(15.0), 5.0, 12.0))
        .with_description(
            "Apple Spatial Audio binaural headphone target: -16 LUFS integrated, LRA ≤ 15 LU, -1.5 dBTP true peak",
        )
    }

    /// Dolby Atmos immersive audio standard profile.
    ///
    /// Target: -18 LUFS integrated (dialog-gated), -1.0 dBTP true peak, LRA ≤ 20 LU.
    /// The `dialogue_level` field is set to -18.0 LUFS.
    pub fn dolby_atmos() -> BroadcastStandardProfile {
        BroadcastStandardProfile::new(
            BroadcastStandardId::DolbyAtmos,
            "Dolby Atmos",
            BroadcastRegion::Global,
            -18.0,
            1.0,
            1.0,
            -1.0,
        )
        .with_gating_mode(GatingMode::Both)
        .with_lra_constraint(LoudnessRangeConstraint::new(Some(20.0), 5.0, 15.0))
        .with_dialogue_level(-18.0)
        .with_description(
            "Dolby Atmos immersive audio: -18 LUFS integrated (dialog-gated), LRA ≤ 20 LU, -1 dBTP true peak",
        )
    }

    /// TikTok short-form video platform profile.
    ///
    /// Target: -14 LUFS integrated, -1.0 dBTP true peak.
    pub fn tiktok() -> BroadcastStandardProfile {
        BroadcastStandardProfile::new(
            BroadcastStandardId::TikTok,
            "TikTok",
            BroadcastRegion::Global,
            -14.0,
            1.0,
            1.0,
            -1.0,
        )
        .with_gating_mode(GatingMode::Both)
        .with_description("TikTok loudness target: -14 LUFS integrated, -1 dBTP true peak")
    }

    /// Facebook / Instagram social video platforms profile.
    ///
    /// Target: -14 LUFS integrated, -1.0 dBTP true peak.
    pub fn facebook_instagram() -> BroadcastStandardProfile {
        BroadcastStandardProfile::new(
            BroadcastStandardId::FacebookInstagram,
            "Facebook/Instagram",
            BroadcastRegion::Global,
            -14.0,
            1.0,
            1.0,
            -1.0,
        )
        .with_gating_mode(GatingMode::Both)
        .with_description(
            "Facebook/Instagram loudness target: -14 LUFS integrated, -1 dBTP true peak",
        )
    }
}

impl Default for BroadcastStandardRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ebu_r128_profile() {
        let profile = BroadcastStandardRegistry::ebu_r128();
        assert_eq!(profile.id, BroadcastStandardId::EbuR128);
        assert!((profile.target_lufs - (-23.0)).abs() < f64::EPSILON);
        assert!((profile.max_true_peak_dbtp - (-1.0)).abs() < f64::EPSILON);
        assert_eq!(profile.region, BroadcastRegion::Europe);
    }

    #[test]
    fn test_atsc_a85_profile() {
        let profile = BroadcastStandardRegistry::atsc_a85();
        assert_eq!(profile.id, BroadcastStandardId::AtscA85);
        assert!((profile.target_lufs - (-24.0)).abs() < f64::EPSILON);
        assert!(profile.requires_dialnorm);
    }

    #[test]
    fn test_loudness_compliance_pass() {
        let profile = BroadcastStandardRegistry::ebu_r128();
        assert!(profile.is_loudness_compliant(-23.0));
        assert!(profile.is_loudness_compliant(-22.5));
        assert!(profile.is_loudness_compliant(-23.8));
    }

    #[test]
    fn test_loudness_compliance_fail() {
        let profile = BroadcastStandardRegistry::ebu_r128();
        assert!(!profile.is_loudness_compliant(-20.0));
        assert!(!profile.is_loudness_compliant(-25.5));
    }

    #[test]
    fn test_true_peak_compliance() {
        let profile = BroadcastStandardRegistry::ebu_r128();
        assert!(profile.is_true_peak_compliant(-3.0));
        assert!(profile.is_true_peak_compliant(-1.0));
        assert!(!profile.is_true_peak_compliant(0.0));
    }

    #[test]
    fn test_full_compliance_check_pass() {
        let profile = BroadcastStandardRegistry::ebu_r128();
        let result = profile.check_compliance(-23.0, -2.0, Some(10.0));
        assert!(result.overall_pass);
        assert!(result.loudness_pass);
        assert!(result.peak_pass);
        assert!(result.lra_pass);
    }

    #[test]
    fn test_full_compliance_check_fail_loudness() {
        let profile = BroadcastStandardRegistry::ebu_r128();
        let result = profile.check_compliance(-19.0, -2.0, Some(10.0));
        assert!(!result.overall_pass);
        assert!(!result.loudness_pass);
        assert!(result.peak_pass);
    }

    #[test]
    fn test_full_compliance_check_fail_peak() {
        let profile = BroadcastStandardRegistry::ebu_r128();
        let result = profile.check_compliance(-23.0, 0.5, Some(10.0));
        assert!(!result.overall_pass);
        assert!(result.loudness_pass);
        assert!(!result.peak_pass);
    }

    #[test]
    fn test_full_compliance_check_fail_lra() {
        let profile = BroadcastStandardRegistry::ebu_r128();
        let result = profile.check_compliance(-23.0, -2.0, Some(25.0));
        assert!(!result.overall_pass);
        assert!(result.loudness_pass);
        assert!(result.peak_pass);
        assert!(!result.lra_pass);
    }

    #[test]
    fn test_gain_to_target() {
        let profile = BroadcastStandardRegistry::ebu_r128();
        let gain = profile.gain_to_target(-28.0);
        assert!((gain - 5.0).abs() < f64::EPSILON);
        let gain2 = profile.gain_to_target(-20.0);
        assert!((gain2 - (-3.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_lra_constraint() {
        let constraint = LoudnessRangeConstraint::new(Some(20.0), 5.0, 15.0);
        assert!(constraint.is_compliant(15.0));
        assert!(constraint.is_compliant(20.0));
        assert!(!constraint.is_compliant(25.0));
        assert!(constraint.is_recommended(10.0));
        assert!(!constraint.is_recommended(3.0));
        assert!(!constraint.is_recommended(18.0));
    }

    #[test]
    fn test_registry_with_defaults() {
        let registry = BroadcastStandardRegistry::with_defaults();
        // 7 broadcast + 6 streaming platform = 13 standards
        assert_eq!(registry.len(), 13);
        assert!(!registry.is_empty());
        assert!(registry.get(BroadcastStandardId::EbuR128).is_some());
        assert!(registry.get(BroadcastStandardId::AtscA85).is_some());
        assert!(registry.get(BroadcastStandardId::AribTrB32).is_some());
        assert!(registry.get(BroadcastStandardId::DisneyPlus).is_some());
        assert!(registry.get(BroadcastStandardId::PrimeVideo).is_some());
        assert!(registry
            .get(BroadcastStandardId::AppleSpatialAudio)
            .is_some());
        assert!(registry.get(BroadcastStandardId::DolbyAtmos).is_some());
        assert!(registry.get(BroadcastStandardId::TikTok).is_some());
        assert!(registry
            .get(BroadcastStandardId::FacebookInstagram)
            .is_some());
    }

    #[test]
    fn test_registry_by_region() {
        let registry = BroadcastStandardRegistry::with_defaults();
        let europe = registry.get_by_region(BroadcastRegion::Europe);
        assert!(europe.len() >= 2);
        let na = registry.get_by_region(BroadcastRegion::NorthAmerica);
        assert!(na.len() >= 2);
    }

    #[test]
    fn test_compliance_result_display() {
        let profile = BroadcastStandardRegistry::ebu_r128();
        let result = profile.check_compliance(-23.0, -2.0, Some(10.0));
        let display = format!("{result}");
        assert!(display.contains("PASS"));
        assert!(display.contains("EBU R128"));
    }

    #[test]
    fn test_broadcast_region_display() {
        assert_eq!(format!("{}", BroadcastRegion::Europe), "Europe");
        assert_eq!(format!("{}", BroadcastRegion::Japan), "Japan");
        assert_eq!(
            format!("{}", BroadcastRegion::NorthAmerica),
            "North America"
        );
    }

    #[test]
    fn test_gating_mode_variants() {
        assert_ne!(GatingMode::AbsoluteOnly, GatingMode::RelativeOnly);
        assert_ne!(GatingMode::Both, GatingMode::None);
        assert_eq!(GatingMode::Both, GatingMode::Both);
    }

    #[test]
    fn test_custom_profile() {
        let profile = BroadcastStandardProfile::new(
            BroadcastStandardId::Custom,
            "My Custom",
            BroadcastRegion::Global,
            -16.0,
            2.0,
            2.0,
            -1.0,
        )
        .with_min_duration(10.0);
        assert_eq!(profile.id, BroadcastStandardId::Custom);
        assert!((profile.min_measurement_duration_s - 10.0).abs() < f64::EPSILON);
        assert!(profile.is_loudness_compliant(-15.0));
        assert!(!profile.is_loudness_compliant(-12.0));
    }

    #[test]
    fn test_disney_plus_profile() {
        let profile = BroadcastStandardRegistry::disney_plus();
        assert_eq!(profile.id, BroadcastStandardId::DisneyPlus);
        assert!((profile.target_lufs - (-24.0)).abs() < f64::EPSILON);
        assert!((profile.max_true_peak_dbtp - (-1.0)).abs() < f64::EPSILON);
        assert_eq!(profile.region, BroadcastRegion::Global);
        let lra = profile
            .lra_constraint
            .expect("Disney+ must have LRA constraint");
        assert_eq!(lra.max_lra_lu, Some(18.0));
        assert!(!lra.is_compliant(20.0));
    }

    #[test]
    fn test_prime_video_profile() {
        let profile = BroadcastStandardRegistry::prime_video();
        assert_eq!(profile.id, BroadcastStandardId::PrimeVideo);
        assert!((profile.target_lufs - (-14.0)).abs() < f64::EPSILON);
        assert!((profile.max_true_peak_dbtp - (-1.0)).abs() < f64::EPSILON);
        let lra = profile
            .lra_constraint
            .expect("Prime Video must have LRA constraint");
        assert_eq!(lra.max_lra_lu, Some(20.0));
    }

    #[test]
    fn test_apple_spatial_audio_profile() {
        let profile = BroadcastStandardRegistry::apple_spatial_audio();
        assert_eq!(profile.id, BroadcastStandardId::AppleSpatialAudio);
        assert!((profile.target_lufs - (-16.0)).abs() < f64::EPSILON);
        assert!((profile.max_true_peak_dbtp - (-1.5)).abs() < f64::EPSILON);
        let lra = profile
            .lra_constraint
            .expect("Apple Spatial Audio must have LRA constraint");
        assert_eq!(lra.max_lra_lu, Some(15.0));
        // LRA of 16 exceeds the 15 LU ceiling
        assert!(!lra.is_compliant(16.0));
    }

    #[test]
    fn test_dolby_atmos_profile() {
        let profile = BroadcastStandardRegistry::dolby_atmos();
        assert_eq!(profile.id, BroadcastStandardId::DolbyAtmos);
        assert!((profile.target_lufs - (-18.0)).abs() < f64::EPSILON);
        assert!((profile.max_true_peak_dbtp - (-1.0)).abs() < f64::EPSILON);
        // Dialogue level must be set to -18.0 for Dolby Atmos
        let dialogue = profile
            .dialogue_level
            .expect("Dolby Atmos must have dialogue_level");
        assert!((f64::from(dialogue) - (-18.0)).abs() < f64::EPSILON);
        let lra = profile
            .lra_constraint
            .expect("Dolby Atmos must have LRA constraint");
        assert_eq!(lra.max_lra_lu, Some(20.0));
    }

    #[test]
    fn test_tiktok_profile() {
        let profile = BroadcastStandardRegistry::tiktok();
        assert_eq!(profile.id, BroadcastStandardId::TikTok);
        assert!((profile.target_lufs - (-14.0)).abs() < f64::EPSILON);
        assert!((profile.max_true_peak_dbtp - (-1.0)).abs() < f64::EPSILON);
        // TikTok has no explicit LRA ceiling in the spec
        assert!(profile.dialogue_level.is_none());
    }

    #[test]
    fn test_facebook_instagram_profile() {
        let profile = BroadcastStandardRegistry::facebook_instagram();
        assert_eq!(profile.id, BroadcastStandardId::FacebookInstagram);
        assert!((profile.target_lufs - (-14.0)).abs() < f64::EPSILON);
        assert!((profile.max_true_peak_dbtp - (-1.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_streaming_platform_ids_display() {
        assert_eq!(format!("{}", BroadcastStandardId::DisneyPlus), "Disney+");
        assert_eq!(
            format!("{}", BroadcastStandardId::PrimeVideo),
            "Prime Video"
        );
        assert_eq!(
            format!("{}", BroadcastStandardId::AppleSpatialAudio),
            "Apple Spatial Audio"
        );
        assert_eq!(
            format!("{}", BroadcastStandardId::DolbyAtmos),
            "Dolby Atmos"
        );
        assert_eq!(format!("{}", BroadcastStandardId::TikTok), "TikTok");
        assert_eq!(
            format!("{}", BroadcastStandardId::FacebookInstagram),
            "Facebook/Instagram"
        );
    }

    #[test]
    fn test_dialogue_level_field() {
        // Non-Atmos profiles should have no dialogue_level
        let ebu = BroadcastStandardRegistry::ebu_r128();
        assert!(ebu.dialogue_level.is_none());
        // Dolby Atmos must carry its dialogue level
        let atmos = BroadcastStandardRegistry::dolby_atmos();
        assert!(atmos.dialogue_level.is_some());
        // Custom profile set via builder
        let custom = BroadcastStandardProfile::new(
            BroadcastStandardId::Custom,
            "Test",
            BroadcastRegion::Global,
            -18.0,
            1.0,
            1.0,
            -1.0,
        )
        .with_dialogue_level(-18.0);
        let dl = custom.dialogue_level.expect("must have dialogue level");
        assert!((f64::from(dl) - (-18.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_global_region_streaming_standards() {
        let registry = BroadcastStandardRegistry::with_defaults();
        let global = registry.get_by_region(BroadcastRegion::Global);
        // All 6 new streaming platform profiles belong to Global region
        assert!(global.len() >= 6);
        let global_ids: Vec<BroadcastStandardId> = global.iter().map(|p| p.id).collect();
        assert!(global_ids.contains(&BroadcastStandardId::DisneyPlus));
        assert!(global_ids.contains(&BroadcastStandardId::PrimeVideo));
        assert!(global_ids.contains(&BroadcastStandardId::AppleSpatialAudio));
        assert!(global_ids.contains(&BroadcastStandardId::DolbyAtmos));
        assert!(global_ids.contains(&BroadcastStandardId::TikTok));
        assert!(global_ids.contains(&BroadcastStandardId::FacebookInstagram));
    }
}
