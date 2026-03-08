//! Dolby Vision stream validation.
//!
//! Provides validation utilities for Dolby Vision stream parameters,
//! ensuring compliance with Dolby specification requirements.

#![allow(dead_code)]

/// Validation errors that can occur during Dolby Vision stream validation.
#[derive(Debug, Clone, PartialEq)]
pub enum DvValidationError {
    /// Profile number is not valid.
    InvalidProfile(u8),
    /// Level number is not valid for the given profile.
    InvalidLevel(u8),
    /// RPU metadata is missing from the stream.
    MissingRpu,
    /// Maximum Content Light Level value is out of specification.
    InvalidMaxCll(u32),
    /// Color volume metadata block is absent.
    MissingColorVolume,
    /// A metadata field value is outside the permitted range.
    MetadataOutOfRange {
        /// Name of the field that is out of range.
        field: String,
        /// Actual value of that field.
        value: f64,
    },
}

impl std::fmt::Display for DvValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidProfile(p) => write!(f, "Invalid Dolby Vision profile: {p}"),
            Self::InvalidLevel(l) => write!(f, "Invalid Dolby Vision level: {l}"),
            Self::MissingRpu => write!(f, "RPU metadata is missing"),
            Self::InvalidMaxCll(cll) => write!(f, "MaxCLL value out of range: {cll}"),
            Self::MissingColorVolume => write!(f, "Color volume metadata is missing"),
            Self::MetadataOutOfRange { field, value } => {
                write!(f, "Metadata field '{field}' out of range: {value}")
            }
        }
    }
}

impl std::error::Error for DvValidationError {}

/// Parameters describing a Dolby Vision stream for validation.
#[derive(Debug, Clone)]
pub struct DvStreamParams {
    /// Dolby Vision profile number (4, 5, 7, 8, 9).
    pub profile: u8,
    /// Dolby Vision level number within the profile.
    pub level: u8,
    /// Maximum Content Light Level in cd/m².
    pub max_cll: u32,
    /// Maximum Frame-Average Light Level in cd/m².
    pub max_fall: u32,
    /// Minimum mastering display luminance in nits.
    pub min_luminance_nits: f64,
    /// Maximum mastering display luminance in nits.
    pub max_luminance_nits: f64,
}

impl DvStreamParams {
    /// Create stream parameters for Profile 4 (SDR/HDR hybrid).
    #[must_use]
    pub fn profile4() -> Self {
        Self {
            profile: 4,
            level: 9,
            max_cll: 1000,
            max_fall: 400,
            min_luminance_nits: 0.005,
            max_luminance_nits: 1000.0,
        }
    }

    /// Create stream parameters for Profile 5 (single-track, IPT-PQ).
    #[must_use]
    pub fn profile5() -> Self {
        Self {
            profile: 5,
            level: 9,
            max_cll: 4000,
            max_fall: 1000,
            min_luminance_nits: 0.005,
            max_luminance_nits: 4000.0,
        }
    }

    /// Create stream parameters for Profile 8.1 (BL-only, low-latency).
    #[must_use]
    pub fn profile8_1() -> Self {
        Self {
            profile: 8,
            level: 1,
            max_cll: 1000,
            max_fall: 400,
            min_luminance_nits: 0.005,
            max_luminance_nits: 1000.0,
        }
    }
}

/// Validate a Dolby Vision stream and return a list of validation errors.
///
/// Returns an empty vector if the stream is fully valid.
#[must_use]
pub fn validate_dv_stream(params: &DvStreamParams) -> Vec<DvValidationError> {
    let mut errors = Vec::new();

    // Validate profile
    if !valid_profiles().contains(&params.profile) {
        errors.push(DvValidationError::InvalidProfile(params.profile));
        // Cannot validate level if profile is unknown
        return errors;
    }

    // Validate level for profile
    let valid_levels = valid_levels_for_profile(params.profile);
    if !valid_levels.contains(&params.level) {
        errors.push(DvValidationError::InvalidLevel(params.level));
    }

    // Validate MaxCLL
    if !cll_is_reasonable(params.max_cll) {
        errors.push(DvValidationError::InvalidMaxCll(params.max_cll));
    }

    // Validate luminance range
    if params.min_luminance_nits < 0.0 || params.min_luminance_nits > 10.0 {
        errors.push(DvValidationError::MetadataOutOfRange {
            field: "min_luminance_nits".to_string(),
            value: params.min_luminance_nits,
        });
    }

    if params.max_luminance_nits < 1.0 || params.max_luminance_nits > 10_000.0 {
        errors.push(DvValidationError::MetadataOutOfRange {
            field: "max_luminance_nits".to_string(),
            value: params.max_luminance_nits,
        });
    }

    if params.min_luminance_nits >= params.max_luminance_nits {
        errors.push(DvValidationError::MetadataOutOfRange {
            field: "luminance_range".to_string(),
            value: params.max_luminance_nits - params.min_luminance_nits,
        });
    }

    // Validate MaxFALL <= MaxCLL
    if params.max_fall > params.max_cll {
        errors.push(DvValidationError::MetadataOutOfRange {
            field: "max_fall".to_string(),
            value: f64::from(params.max_fall),
        });
    }

    errors
}

/// Return the list of valid Dolby Vision profile numbers.
#[must_use]
pub fn valid_profiles() -> Vec<u8> {
    vec![4, 5, 7, 8, 9]
}

/// Return valid level numbers for a given Dolby Vision profile.
///
/// Returns an empty vector for unknown profiles.
#[must_use]
pub fn valid_levels_for_profile(profile: u8) -> Vec<u8> {
    match profile {
        4 => vec![9],
        5 => vec![9],
        7 => vec![6, 10, 14],
        8 => vec![1, 2, 3, 4, 5, 6],
        9 => vec![9],
        _ => vec![],
    }
}

/// Check whether a MaxCLL value is within the reasonable range (0–10000 cd/m²).
#[must_use]
pub fn cll_is_reasonable(max_cll: u32) -> bool {
    max_cll <= 10_000
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_profiles_contains_expected() {
        let profiles = valid_profiles();
        for p in [4u8, 5, 7, 8, 9] {
            assert!(profiles.contains(&p), "Profile {p} should be valid");
        }
    }

    #[test]
    fn test_valid_profiles_does_not_contain_invalid() {
        let profiles = valid_profiles();
        assert!(!profiles.contains(&0));
        assert!(!profiles.contains(&6));
        assert!(!profiles.contains(&99));
    }

    #[test]
    fn test_valid_levels_for_profile_5() {
        let levels = valid_levels_for_profile(5);
        assert!(levels.contains(&9));
        assert_eq!(levels.len(), 1);
    }

    #[test]
    fn test_valid_levels_for_profile_8() {
        let levels = valid_levels_for_profile(8);
        assert!(levels.contains(&1));
        assert!(levels.contains(&6));
        assert!(!levels.contains(&9));
    }

    #[test]
    fn test_valid_levels_for_unknown_profile() {
        let levels = valid_levels_for_profile(99);
        assert!(levels.is_empty());
    }

    #[test]
    fn test_cll_is_reasonable_zero() {
        assert!(cll_is_reasonable(0));
    }

    #[test]
    fn test_cll_is_reasonable_max() {
        assert!(cll_is_reasonable(10_000));
    }

    #[test]
    fn test_cll_is_reasonable_over_max() {
        assert!(!cll_is_reasonable(10_001));
        assert!(!cll_is_reasonable(u32::MAX));
    }

    #[test]
    fn test_validate_dv_stream_profile5_valid() {
        let params = DvStreamParams::profile5();
        let errors = validate_dv_stream(&params);
        assert!(
            errors.is_empty(),
            "Profile 5 defaults should be valid, got: {errors:?}"
        );
    }

    #[test]
    fn test_validate_dv_stream_invalid_profile() {
        let mut params = DvStreamParams::profile5();
        params.profile = 6;
        let errors = validate_dv_stream(&params);
        assert!(errors
            .iter()
            .any(|e| matches!(e, DvValidationError::InvalidProfile(6))));
    }

    #[test]
    fn test_validate_dv_stream_invalid_level() {
        let mut params = DvStreamParams::profile5();
        params.level = 99;
        let errors = validate_dv_stream(&params);
        assert!(errors
            .iter()
            .any(|e| matches!(e, DvValidationError::InvalidLevel(99))));
    }

    #[test]
    fn test_validate_dv_stream_invalid_cll() {
        let mut params = DvStreamParams::profile5();
        params.max_cll = 99_999;
        let errors = validate_dv_stream(&params);
        assert!(errors
            .iter()
            .any(|e| matches!(e, DvValidationError::InvalidMaxCll(99_999))));
    }

    #[test]
    fn test_validate_dv_stream_fall_exceeds_cll() {
        let mut params = DvStreamParams::profile5();
        params.max_cll = 1000;
        params.max_fall = 2000;
        let errors = validate_dv_stream(&params);
        assert!(
            !errors.is_empty(),
            "MaxFALL > MaxCLL should produce an error"
        );
    }

    #[test]
    fn test_validate_dv_stream_inverted_luminance() {
        let mut params = DvStreamParams::profile5();
        params.min_luminance_nits = 5000.0;
        params.max_luminance_nits = 100.0;
        let errors = validate_dv_stream(&params);
        // Both out-of-range AND inverted
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_dv_validation_error_display() {
        let e = DvValidationError::InvalidProfile(3);
        let msg = e.to_string();
        assert!(msg.contains("3"));

        let e2 = DvValidationError::MetadataOutOfRange {
            field: "foo".to_string(),
            value: 42.0,
        };
        assert!(e2.to_string().contains("foo"));
    }

    #[test]
    fn test_profile4_params_valid() {
        let params = DvStreamParams::profile4();
        let errors = validate_dv_stream(&params);
        assert!(
            errors.is_empty(),
            "Profile 4 defaults should be valid, got: {errors:?}"
        );
    }

    #[test]
    fn test_profile8_1_params_valid() {
        let params = DvStreamParams::profile8_1();
        let errors = validate_dv_stream(&params);
        assert!(
            errors.is_empty(),
            "Profile 8.1 defaults should be valid, got: {errors:?}"
        );
    }
}
