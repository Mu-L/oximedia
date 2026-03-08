//! IMF Application Profile support - SMPTE ST 2067-21 and related standards
//!
//! Application profiles constrain the IMF package to a specific set of
//! allowed essence types and parameters for a given delivery platform.

use std::collections::HashMap;

/// IMF Application Profile identifiers per SMPTE ST 2067-x
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum ApplicationProfile {
    /// App #2 - IMF Application #2 (JPEG 2000, SMPTE ST 2067-21)
    App2,
    /// App #2 Extended - JPEG 2000 extended constraints
    App2Extended,
    /// App #4 - IMF ACES Application (ACES workflow)
    App4,
    /// App #5 ACES - Academy Color Encoding System
    App5Aces,
    /// App #6 - IAB (Immersive Audio Bitstream)
    App6,
    /// App #7 - JPEG XS essence
    App7,
    /// IABMM - Immersive Audio Bitstream for Mastering and Mezzanine
    Iabmm,
}

impl ApplicationProfile {
    /// Return the URN identifier for this application profile
    #[must_use]
    pub fn urn(&self) -> &str {
        match self {
            Self::App2 => "urn:smpte:ul:060E2B34.04010105.0E090604.00000000",
            Self::App2Extended => "urn:smpte:ul:060E2B34.04010105.0E090605.00000000",
            Self::App4 => "urn:smpte:ul:060E2B34.04010105.0E090606.00000000",
            Self::App5Aces => "urn:smpte:ul:060E2B34.04010105.0E090607.00000000",
            Self::App6 => "urn:smpte:ul:060E2B34.04010105.0E090608.00000000",
            Self::App7 => "urn:smpte:ul:060E2B34.04010105.0E090609.00000000",
            Self::Iabmm => "urn:smpte:ul:060E2B34.04010105.0E09060A.00000000",
        }
    }
}

/// Constraints for IMF Application #2 per SMPTE ST 2067-21
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct App2Constraints {
    /// Maximum allowed resolution (width, height)
    pub max_resolution: (u32, u32),
    /// Maximum allowed frame rate as (numerator, denominator)
    pub max_frame_rate: (u32, u32),
    /// Maximum number of audio channels
    pub audio_channel_count: u32,
}

impl App2Constraints {
    /// ST 2067-21 App #2 default constraints
    #[must_use]
    pub fn st2067_21() -> Self {
        Self {
            max_resolution: (3840, 2160),
            max_frame_rate: (60, 1),
            audio_channel_count: 16,
        }
    }
}

/// Essence types allowed in IMF packages
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum EssenceType {
    /// MXF-wrapped JPEG 2000
    MxfJpeg2000,
    /// MXF-wrapped Apple ProRes
    MxfProRes,
    /// MXF-wrapped AVC (H.264)
    MxfAvc,
    /// MXF-wrapped IAB (Immersive Audio Bitstream)
    MxfIab,
    /// Timed Text SRT subtitle
    TtSrt,
    /// Timed Text IMSC1 (TTML-based)
    TtImsc,
}

impl EssenceType {
    /// Check if this essence type is allowed in the given application profile
    #[must_use]
    pub fn allowed_in(&self, profile: &ApplicationProfile) -> bool {
        match profile {
            ApplicationProfile::App2 | ApplicationProfile::App2Extended => {
                matches!(self, Self::MxfJpeg2000 | Self::TtSrt | Self::TtImsc)
            }
            ApplicationProfile::App4 | ApplicationProfile::App5Aces => {
                matches!(self, Self::MxfJpeg2000 | Self::TtImsc)
            }
            ApplicationProfile::App6 | ApplicationProfile::Iabmm => {
                matches!(
                    self,
                    Self::MxfJpeg2000 | Self::MxfIab | Self::TtSrt | Self::TtImsc
                )
            }
            ApplicationProfile::App7 => {
                matches!(self, Self::MxfProRes | Self::MxfAvc | Self::TtImsc)
            }
        }
    }
}

/// Validates CPL essence types against an application profile
#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct ProfileValidator;

impl ProfileValidator {
    /// Validate a list of CPL essence type strings against the given profile.
    ///
    /// Returns a list of validation error messages (empty = valid).
    #[must_use]
    pub fn validate_for_profile(
        cpl_essence_types: &[&str],
        profile: &ApplicationProfile,
    ) -> Vec<String> {
        let mut errors = Vec::new();

        for essence_type_str in cpl_essence_types {
            let essence = parse_essence_type(essence_type_str);
            match essence {
                Some(et) => {
                    if !et.allowed_in(profile) {
                        errors.push(format!(
                            "Essence type '{}' is not allowed in profile '{}'",
                            essence_type_str,
                            profile.urn()
                        ));
                    }
                }
                None => {
                    errors.push(format!("Unknown essence type: '{essence_type_str}'"));
                }
            }
        }

        errors
    }
}

/// Parse an essence type string into an `EssenceType`
fn parse_essence_type(s: &str) -> Option<EssenceType> {
    match s {
        "MxfJpeg2000" | "application/mxf+jpeg2000" => Some(EssenceType::MxfJpeg2000),
        "MxfProRes" | "application/mxf+prores" => Some(EssenceType::MxfProRes),
        "MxfAvc" | "application/mxf+avc" => Some(EssenceType::MxfAvc),
        "MxfIab" | "application/mxf+iab" => Some(EssenceType::MxfIab),
        "TtSrt" | "text/srt" => Some(EssenceType::TtSrt),
        "TtImsc" | "application/ttml+xml" => Some(EssenceType::TtImsc),
        _ => None,
    }
}

/// Metadata describing an IMF application profile instance
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ApplicationMetadata {
    /// The application profile
    pub profile: ApplicationProfile,
    /// Version string (e.g. "1.0")
    pub version: String,
    /// Arbitrary key-value constraint metadata
    pub constraints: HashMap<String, String>,
}

impl ApplicationMetadata {
    /// Create new application metadata for the given profile and version
    #[must_use]
    pub fn new(profile: ApplicationProfile, version: String) -> Self {
        Self {
            profile,
            version,
            constraints: HashMap::new(),
        }
    }

    /// Add a constraint key-value pair
    pub fn add_constraint(&mut self, key: String, value: String) {
        self.constraints.insert(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_application_profile_urn() {
        let profile = ApplicationProfile::App2;
        assert!(profile.urn().starts_with("urn:smpte:ul:"));
    }

    #[test]
    fn test_all_profiles_have_distinct_urns() {
        let profiles = [
            ApplicationProfile::App2,
            ApplicationProfile::App2Extended,
            ApplicationProfile::App4,
            ApplicationProfile::App5Aces,
            ApplicationProfile::App6,
            ApplicationProfile::App7,
            ApplicationProfile::Iabmm,
        ];
        let urns: Vec<&str> = profiles.iter().map(ApplicationProfile::urn).collect();
        let unique: std::collections::HashSet<&&str> = urns.iter().collect();
        assert_eq!(unique.len(), urns.len(), "All URNs must be unique");
    }

    #[test]
    fn test_app2_constraints() {
        let c = App2Constraints::st2067_21();
        assert_eq!(c.max_resolution, (3840, 2160));
        assert_eq!(c.max_frame_rate, (60, 1));
        assert_eq!(c.audio_channel_count, 16);
    }

    #[test]
    fn test_essence_allowed_in_app2() {
        let profile = ApplicationProfile::App2;
        assert!(EssenceType::MxfJpeg2000.allowed_in(&profile));
        assert!(EssenceType::TtSrt.allowed_in(&profile));
        assert!(EssenceType::TtImsc.allowed_in(&profile));
        assert!(!EssenceType::MxfProRes.allowed_in(&profile));
        assert!(!EssenceType::MxfAvc.allowed_in(&profile));
        assert!(!EssenceType::MxfIab.allowed_in(&profile));
    }

    #[test]
    fn test_essence_allowed_in_app7() {
        let profile = ApplicationProfile::App7;
        assert!(!EssenceType::MxfJpeg2000.allowed_in(&profile));
        assert!(EssenceType::MxfProRes.allowed_in(&profile));
        assert!(EssenceType::MxfAvc.allowed_in(&profile));
        assert!(EssenceType::TtImsc.allowed_in(&profile));
        assert!(!EssenceType::TtSrt.allowed_in(&profile));
    }

    #[test]
    fn test_essence_allowed_in_iabmm() {
        let profile = ApplicationProfile::Iabmm;
        assert!(EssenceType::MxfIab.allowed_in(&profile));
        assert!(EssenceType::MxfJpeg2000.allowed_in(&profile));
    }

    #[test]
    fn test_profile_validator_valid() {
        let types = ["MxfJpeg2000", "TtImsc"];
        let errors = ProfileValidator::validate_for_profile(&types, &ApplicationProfile::App2);
        assert!(errors.is_empty(), "Expected no errors, got: {errors:?}");
    }

    #[test]
    fn test_profile_validator_invalid_essence() {
        let types = ["MxfProRes"];
        let errors = ProfileValidator::validate_for_profile(&types, &ApplicationProfile::App2);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("not allowed"));
    }

    #[test]
    fn test_profile_validator_unknown_essence() {
        let types = ["SomeWeirdFormat"];
        let errors = ProfileValidator::validate_for_profile(&types, &ApplicationProfile::App2);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Unknown"));
    }

    #[test]
    fn test_application_metadata_creation() {
        let mut meta = ApplicationMetadata::new(ApplicationProfile::App2, "1.0".to_string());
        meta.add_constraint("max_bits".to_string(), "12".to_string());
        assert_eq!(meta.version, "1.0");
        assert_eq!(
            meta.constraints.get("max_bits").map(String::as_str),
            Some("12")
        );
    }

    #[test]
    fn test_application_metadata_constraints_map() {
        let mut meta = ApplicationMetadata::new(ApplicationProfile::App4, "2.0".to_string());
        meta.add_constraint("color_space".to_string(), "ACES".to_string());
        meta.add_constraint("bit_depth".to_string(), "16".to_string());
        assert_eq!(meta.constraints.len(), 2);
    }

    #[test]
    fn test_parse_essence_type_round_trip() {
        assert_eq!(
            parse_essence_type("MxfJpeg2000"),
            Some(EssenceType::MxfJpeg2000)
        );
        assert_eq!(
            parse_essence_type("application/mxf+jpeg2000"),
            Some(EssenceType::MxfJpeg2000)
        );
        assert_eq!(parse_essence_type("unknown_type"), None);
    }
}
