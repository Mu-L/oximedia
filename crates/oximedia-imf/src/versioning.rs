//! IMF Package versioning - tracking composition versions and supplements
//!
//! Supports the versioning concepts from SMPTE ST 2067-2 for managing
//! Original, Extension, Supplement, and Substitution package types.

/// The kind of a package version
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum VersionKind {
    /// Original package - no base version
    Original,
    /// Extension of an existing package
    Extension,
    /// Supplement to an existing package (adds tracks)
    Supplement,
    /// Substitution for an existing package (replaces content)
    Substitution,
}

impl VersionKind {
    /// Returns `true` if this version kind requires a base version ID
    #[must_use]
    pub fn requires_base(&self) -> bool {
        matches!(
            self,
            Self::Extension | Self::Supplement | Self::Substitution
        )
    }
}

/// A single version of an IMF package
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PackageVersion {
    /// Unique identifier for this version
    pub id: String,
    /// ID of the base version this derives from (if any)
    pub base_version_id: Option<String>,
    /// The kind of this version
    pub kind: VersionKind,
    /// Human-readable description of this version
    pub description: String,
}

impl PackageVersion {
    /// Create a new `PackageVersion`
    #[must_use]
    pub fn new(
        id: String,
        base_version_id: Option<String>,
        kind: VersionKind,
        description: String,
    ) -> Self {
        Self {
            id,
            base_version_id,
            kind,
            description,
        }
    }
}

/// A chain of package versions
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct VersionChain {
    /// All versions in this chain, ordered by insertion
    pub versions: Vec<PackageVersion>,
}

impl VersionChain {
    /// Create an empty `VersionChain`
    #[must_use]
    pub fn new() -> Self {
        Self {
            versions: Vec::new(),
        }
    }

    /// Add a version to the chain
    pub fn add_version(&mut self, v: PackageVersion) {
        self.versions.push(v);
    }

    /// Return the latest (most recently added) version
    #[must_use]
    pub fn latest(&self) -> Option<&PackageVersion> {
        self.versions.last()
    }

    /// Return all versions in the chain in insertion order
    #[must_use]
    pub fn full_chain(&self) -> Vec<&PackageVersion> {
        self.versions.iter().collect()
    }
}

/// A resource reference within a composition segment
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Resource {
    /// Unique identifier for this resource reference
    pub id: String,
    /// ID of the asset (track file) this resource references
    pub asset_id: String,
    /// Edit rate as (numerator, denominator)
    pub edit_rate: (u32, u32),
    /// Frame offset into the asset where playback begins
    pub entry_point: u64,
    /// Number of frames from the asset to play
    pub source_duration: u64,
    /// Number of times to repeat this resource
    pub repeat_count: u32,
}

impl Resource {
    /// Create a new `Resource`
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        asset_id: String,
        edit_rate: (u32, u32),
        entry_point: u64,
        source_duration: u64,
        repeat_count: u32,
    ) -> Self {
        Self {
            id,
            asset_id,
            edit_rate,
            entry_point,
            source_duration,
            repeat_count,
        }
    }

    /// The effective duration is `source_duration * repeat_count`
    #[must_use]
    pub fn effective_duration(&self) -> u64 {
        self.source_duration * u64::from(self.repeat_count)
    }
}

/// A composition segment containing ordered resource references for a virtual track
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CompositionSegment {
    /// Virtual track ID this segment belongs to
    pub virtual_track_id: String,
    /// Ordered list of resource references
    pub resource_list: Vec<Resource>,
}

impl CompositionSegment {
    /// Create a new `CompositionSegment`
    #[must_use]
    pub fn new(virtual_track_id: String) -> Self {
        Self {
            virtual_track_id,
            resource_list: Vec::new(),
        }
    }

    /// Add a resource to this segment
    pub fn add_resource(&mut self, resource: Resource) {
        self.resource_list.push(resource);
    }
}

/// Validates track file resources in a composition
#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct TrackFileValidator;

impl TrackFileValidator {
    /// Verify that all resources have non-empty asset references.
    ///
    /// Returns a list of error messages (empty = valid).
    #[must_use]
    pub fn verify_resources(resources: &[Resource]) -> Vec<String> {
        let mut errors = Vec::new();

        for resource in resources {
            if resource.asset_id.is_empty() {
                errors.push(format!("Resource '{}' has an empty asset_id", resource.id));
            }
            if resource.id.is_empty() {
                errors.push("Found a resource with an empty id".to_string());
            }
            if resource.source_duration == 0 {
                errors.push(format!(
                    "Resource '{}' has source_duration of 0",
                    resource.id
                ));
            }
            if resource.repeat_count == 0 {
                errors.push(format!("Resource '{}' has repeat_count of 0", resource.id));
            }
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_kind_requires_base() {
        assert!(!VersionKind::Original.requires_base());
        assert!(VersionKind::Extension.requires_base());
        assert!(VersionKind::Supplement.requires_base());
        assert!(VersionKind::Substitution.requires_base());
    }

    #[test]
    fn test_package_version_creation() {
        let v = PackageVersion::new(
            "v1".to_string(),
            None,
            VersionKind::Original,
            "Initial release".to_string(),
        );
        assert_eq!(v.id, "v1");
        assert!(v.base_version_id.is_none());
        assert_eq!(v.kind, VersionKind::Original);
    }

    #[test]
    fn test_version_chain_empty() {
        let chain = VersionChain::new();
        assert!(chain.latest().is_none());
        assert!(chain.full_chain().is_empty());
    }

    #[test]
    fn test_version_chain_add_and_latest() {
        let mut chain = VersionChain::new();
        chain.add_version(PackageVersion::new(
            "v1".to_string(),
            None,
            VersionKind::Original,
            "Original".to_string(),
        ));
        chain.add_version(PackageVersion::new(
            "v2".to_string(),
            Some("v1".to_string()),
            VersionKind::Extension,
            "Extended version".to_string(),
        ));

        assert_eq!(chain.latest().map(|v| v.id.as_str()), Some("v2"));
    }

    #[test]
    fn test_version_chain_full_chain() {
        let mut chain = VersionChain::new();
        for i in 0..3u32 {
            chain.add_version(PackageVersion::new(
                format!("v{i}"),
                if i == 0 {
                    None
                } else {
                    Some(format!("v{}", i - 1))
                },
                if i == 0 {
                    VersionKind::Original
                } else {
                    VersionKind::Extension
                },
                format!("Version {i}"),
            ));
        }
        assert_eq!(chain.full_chain().len(), 3);
    }

    #[test]
    fn test_resource_effective_duration() {
        let r = Resource::new("r1".to_string(), "asset1".to_string(), (24, 1), 0, 100, 3);
        assert_eq!(r.effective_duration(), 300);
    }

    #[test]
    fn test_resource_effective_duration_no_repeat() {
        let r = Resource::new("r1".to_string(), "asset1".to_string(), (24, 1), 10, 50, 1);
        assert_eq!(r.effective_duration(), 50);
    }

    #[test]
    fn test_composition_segment_add_resource() {
        let mut seg = CompositionSegment::new("track-1".to_string());
        seg.add_resource(Resource::new(
            "r1".to_string(),
            "asset1".to_string(),
            (24, 1),
            0,
            100,
            1,
        ));
        assert_eq!(seg.resource_list.len(), 1);
    }

    #[test]
    fn test_track_file_validator_valid() {
        let resources = vec![Resource::new(
            "r1".to_string(),
            "asset1".to_string(),
            (24, 1),
            0,
            100,
            1,
        )];
        let errors = TrackFileValidator::verify_resources(&resources);
        assert!(errors.is_empty(), "Expected no errors, got: {errors:?}");
    }

    #[test]
    fn test_track_file_validator_empty_asset_id() {
        let resources = vec![Resource::new(
            "r1".to_string(),
            String::new(), // empty asset_id
            (24, 1),
            0,
            100,
            1,
        )];
        let errors = TrackFileValidator::verify_resources(&resources);
        assert!(!errors.is_empty());
        assert!(errors[0].contains("empty asset_id"));
    }

    #[test]
    fn test_track_file_validator_zero_duration() {
        let resources = vec![Resource::new(
            "r1".to_string(),
            "asset1".to_string(),
            (24, 1),
            0,
            0, // zero duration
            1,
        )];
        let errors = TrackFileValidator::verify_resources(&resources);
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_track_file_validator_zero_repeat_count() {
        let resources = vec![Resource::new(
            "r1".to_string(),
            "asset1".to_string(),
            (24, 1),
            0,
            100,
            0, // zero repeat_count
        )];
        let errors = TrackFileValidator::verify_resources(&resources);
        assert!(!errors.is_empty());
    }
}
