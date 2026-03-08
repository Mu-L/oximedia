//! Cold and deep-freeze storage tier management for large media archives.
//!
//! Implements a policy-driven tiering model that classifies assets into hot,
//! warm, cold, and frozen storage tiers based on last-access time, file size,
//! and explicit policy rules.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

use std::path::PathBuf;

/// Storage tier classification.
///
/// Tiers are ordered by access speed (fastest first) and cost (cheapest last).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StorageTier {
    /// Online SSD/NVMe — instant access, highest cost.
    Hot,
    /// Nearline spinning disk — seconds to access, moderate cost.
    Warm,
    /// Tape / cloud archive — minutes to hours to access, low cost.
    Cold,
    /// Deep-freeze / glacier — hours to days to access, very low cost.
    Frozen,
}

impl std::fmt::Display for StorageTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Hot => "Hot",
            Self::Warm => "Warm",
            Self::Cold => "Cold",
            Self::Frozen => "Frozen",
        };
        write!(f, "{s}")
    }
}

impl StorageTier {
    /// Relative cost factor (Hot = 1.0 baseline).
    #[must_use]
    pub fn relative_cost(self) -> f32 {
        match self {
            Self::Hot => 1.0,
            Self::Warm => 0.4,
            Self::Cold => 0.08,
            Self::Frozen => 0.02,
        }
    }

    /// Typical maximum retrieval latency as a human-readable string.
    #[must_use]
    pub fn retrieval_latency(self) -> &'static str {
        match self {
            Self::Hot => "< 1 ms",
            Self::Warm => "< 10 s",
            Self::Cold => "< 4 h",
            Self::Frozen => "< 48 h",
        }
    }
}

// ---------------------------------------------------------------------------
// ColdStoragePolicy
// ---------------------------------------------------------------------------

/// Rules that determine when an asset should be moved to a cooler tier.
#[derive(Debug, Clone)]
pub struct ColdStoragePolicy {
    /// Move to Warm after this many days without access.
    pub warm_after_days: u32,
    /// Move to Cold after this many days without access.
    pub cold_after_days: u32,
    /// Move to Frozen after this many days without access.
    pub frozen_after_days: u32,
    /// Minimum file size in bytes to be considered for tiering.
    pub min_size_bytes: u64,
    /// If `true`, assets marked as "always-hot" are never downgraded.
    pub respect_pinned: bool,
}

impl Default for ColdStoragePolicy {
    fn default() -> Self {
        Self {
            warm_after_days: 30,
            cold_after_days: 180,
            frozen_after_days: 365,
            min_size_bytes: 1024 * 1024, // 1 MiB
            respect_pinned: true,
        }
    }
}

impl ColdStoragePolicy {
    /// Validate policy constraints.
    pub fn validate(&self) -> Result<(), String> {
        if self.warm_after_days >= self.cold_after_days {
            return Err("warm_after_days must be less than cold_after_days".to_string());
        }
        if self.cold_after_days >= self.frozen_after_days {
            return Err("cold_after_days must be less than frozen_after_days".to_string());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Asset descriptor
// ---------------------------------------------------------------------------

/// Minimal descriptor for an asset under cold-storage management.
#[derive(Debug, Clone)]
pub struct AssetDescriptor {
    /// Filesystem path of the asset.
    pub path: PathBuf,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Days since last access (0 = accessed today).
    pub days_since_access: u32,
    /// Whether the asset is pinned to hot storage.
    pub pinned: bool,
    /// Current assigned tier.
    pub tier: StorageTier,
}

impl AssetDescriptor {
    /// Create a new asset descriptor defaulting to Hot tier.
    #[must_use]
    pub fn new(path: PathBuf, size_bytes: u64, days_since_access: u32) -> Self {
        Self {
            path,
            size_bytes,
            days_since_access,
            pinned: false,
            tier: StorageTier::Hot,
        }
    }
}

// ---------------------------------------------------------------------------
// ColdStorageManager
// ---------------------------------------------------------------------------

/// Manages a catalogue of assets and applies tiering policies to them.
///
/// This is a pure planning engine — it produces tier-change recommendations
/// without performing actual data movement.
pub struct ColdStorageManager {
    policy: ColdStoragePolicy,
    assets: Vec<AssetDescriptor>,
}

impl ColdStorageManager {
    /// Create a manager with the given policy.
    #[must_use]
    pub fn new(policy: ColdStoragePolicy) -> Self {
        Self {
            policy,
            assets: Vec::new(),
        }
    }

    /// Register an asset with the manager.
    pub fn register(&mut self, asset: AssetDescriptor) {
        self.assets.push(asset);
    }

    /// Compute the recommended tier for a single asset under the current policy.
    #[must_use]
    pub fn recommended_tier(&self, asset: &AssetDescriptor) -> StorageTier {
        if self.policy.respect_pinned && asset.pinned {
            return StorageTier::Hot;
        }
        if asset.size_bytes < self.policy.min_size_bytes {
            return asset.tier; // too small to bother tiering
        }
        let days = asset.days_since_access;
        if days >= self.policy.frozen_after_days {
            StorageTier::Frozen
        } else if days >= self.policy.cold_after_days {
            StorageTier::Cold
        } else if days >= self.policy.warm_after_days {
            StorageTier::Warm
        } else {
            StorageTier::Hot
        }
    }

    /// Apply the policy to all registered assets and return a list of
    /// `(path, old_tier, new_tier)` for assets that need to move.
    #[must_use]
    pub fn apply_policy(&self) -> Vec<(PathBuf, StorageTier, StorageTier)> {
        self.assets
            .iter()
            .filter_map(|a| {
                let new_tier = self.recommended_tier(a);
                if new_tier != a.tier {
                    Some((a.path.clone(), a.tier, new_tier))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Estimate monthly storage cost for all registered assets (USD, rough).
    ///
    /// Uses $0.023 / GiB-month as the Hot baseline.
    #[must_use]
    pub fn estimated_monthly_cost_usd(&self) -> f64 {
        const HOT_COST_PER_GIB: f64 = 0.023;
        self.assets
            .iter()
            .map(|a| {
                let gib = a.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                let factor = self.recommended_tier(a).relative_cost() as f64;
                gib * HOT_COST_PER_GIB * factor
            })
            .sum()
    }

    /// Return the total number of registered assets.
    #[must_use]
    pub fn asset_count(&self) -> usize {
        self.assets.len()
    }

    /// Return assets currently assigned to a given tier.
    #[must_use]
    pub fn assets_in_tier(&self, tier: StorageTier) -> Vec<&AssetDescriptor> {
        self.assets.iter().filter(|a| a.tier == tier).collect()
    }

    /// Return the policy in use.
    #[must_use]
    pub fn policy(&self) -> &ColdStoragePolicy {
        &self.policy
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_manager() -> ColdStorageManager {
        ColdStorageManager::new(ColdStoragePolicy::default())
    }

    fn asset(days: u32, mb: u64) -> AssetDescriptor {
        AssetDescriptor::new(
            PathBuf::from(format!("/archive/{days}d_{mb}mb.mxf")),
            mb * 1024 * 1024,
            days,
        )
    }

    #[test]
    fn test_tier_display() {
        assert_eq!(StorageTier::Hot.to_string(), "Hot");
        assert_eq!(StorageTier::Frozen.to_string(), "Frozen");
    }

    #[test]
    fn test_tier_ordering() {
        assert!(StorageTier::Hot < StorageTier::Warm);
        assert!(StorageTier::Warm < StorageTier::Cold);
        assert!(StorageTier::Cold < StorageTier::Frozen);
    }

    #[test]
    fn test_tier_relative_cost() {
        assert!((StorageTier::Hot.relative_cost() - 1.0).abs() < 1e-5);
        assert!(StorageTier::Frozen.relative_cost() < StorageTier::Cold.relative_cost());
    }

    #[test]
    fn test_tier_retrieval_latency_nonempty() {
        for tier in [
            StorageTier::Hot,
            StorageTier::Warm,
            StorageTier::Cold,
            StorageTier::Frozen,
        ] {
            assert!(!tier.retrieval_latency().is_empty());
        }
    }

    #[test]
    fn test_policy_default_valid() {
        assert!(ColdStoragePolicy::default().validate().is_ok());
    }

    #[test]
    fn test_policy_bad_warm_cold_order() {
        let p = ColdStoragePolicy {
            warm_after_days: 200,
            cold_after_days: 100,
            ..Default::default()
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn test_policy_bad_cold_frozen_order() {
        let p = ColdStoragePolicy {
            cold_after_days: 400,
            frozen_after_days: 300,
            ..Default::default()
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn test_recommended_tier_hot_recent() {
        let m = default_manager();
        let a = asset(5, 100); // accessed 5 days ago, 100 MiB
        assert_eq!(m.recommended_tier(&a), StorageTier::Hot);
    }

    #[test]
    fn test_recommended_tier_warm() {
        let m = default_manager();
        let a = asset(60, 100); // 60 days, well past warm threshold of 30
        assert_eq!(m.recommended_tier(&a), StorageTier::Warm);
    }

    #[test]
    fn test_recommended_tier_cold() {
        let m = default_manager();
        let a = asset(200, 100); // > 180 days
        assert_eq!(m.recommended_tier(&a), StorageTier::Cold);
    }

    #[test]
    fn test_recommended_tier_frozen() {
        let m = default_manager();
        let a = asset(400, 100); // > 365 days
        assert_eq!(m.recommended_tier(&a), StorageTier::Frozen);
    }

    #[test]
    fn test_recommended_tier_pinned_stays_hot() {
        let m = default_manager();
        let mut a = asset(500, 100);
        a.pinned = true;
        assert_eq!(m.recommended_tier(&a), StorageTier::Hot);
    }

    #[test]
    fn test_recommended_tier_small_file_unchanged() {
        let m = default_manager();
        let small = AssetDescriptor::new(PathBuf::from("/tiny.txt"), 512, 500);
        // Size < min_size_bytes → keep current tier (Hot)
        assert_eq!(m.recommended_tier(&small), StorageTier::Hot);
    }

    #[test]
    fn test_apply_policy_detects_moves() {
        let mut m = default_manager();
        let mut a = asset(400, 100);
        a.tier = StorageTier::Hot; // currently Hot but should be Frozen
        m.register(a);
        let moves = m.apply_policy();
        assert_eq!(moves.len(), 1);
        assert_eq!(moves[0].2, StorageTier::Frozen);
    }

    #[test]
    fn test_apply_policy_no_move_needed() {
        let mut m = default_manager();
        let mut a = asset(400, 100);
        a.tier = StorageTier::Frozen; // already correct
        m.register(a);
        let moves = m.apply_policy();
        assert!(moves.is_empty());
    }

    #[test]
    fn test_estimated_monthly_cost_zero_assets() {
        let m = default_manager();
        assert!((m.estimated_monthly_cost_usd() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_estimated_monthly_cost_positive() {
        let mut m = default_manager();
        m.register(asset(5, 1024)); // 1 GiB, Hot
        assert!(m.estimated_monthly_cost_usd() > 0.0);
    }

    #[test]
    fn test_asset_count() {
        let mut m = default_manager();
        assert_eq!(m.asset_count(), 0);
        m.register(asset(10, 50));
        m.register(asset(200, 100));
        assert_eq!(m.asset_count(), 2);
    }

    #[test]
    fn test_assets_in_tier() {
        let mut m = default_manager();
        let mut a = asset(10, 50);
        a.tier = StorageTier::Hot;
        let mut b = asset(200, 100);
        b.tier = StorageTier::Cold;
        m.register(a);
        m.register(b);
        assert_eq!(m.assets_in_tier(StorageTier::Hot).len(), 1);
        assert_eq!(m.assets_in_tier(StorageTier::Cold).len(), 1);
        assert!(m.assets_in_tier(StorageTier::Frozen).is_empty());
    }
}
