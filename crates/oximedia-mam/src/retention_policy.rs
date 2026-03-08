#![allow(dead_code)]
//! Data retention policy engine for automated asset lifecycle management.
//!
//! Provides configurable retention rules that determine when assets should be
//! archived, moved to cold storage, or permanently deleted based on age,
//! access patterns, and metadata criteria.

use std::collections::HashMap;
use std::fmt;

/// Unique identifier for a retention policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PolicyId(u64);

impl PolicyId {
    /// Create a new policy identifier.
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Return the numeric value of the identifier.
    pub fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for PolicyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "policy-{}", self.0)
    }
}

/// The action to take when a retention rule triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionAction {
    /// Move the asset to cold / archive storage.
    Archive,
    /// Permanently delete the asset.
    Delete,
    /// Move the asset to a lower-cost storage tier.
    TierDown,
    /// Flag the asset for manual review.
    FlagForReview,
    /// Notify administrators about the asset.
    Notify,
}

impl fmt::Display for RetentionAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Archive => write!(f, "archive"),
            Self::Delete => write!(f, "delete"),
            Self::TierDown => write!(f, "tier_down"),
            Self::FlagForReview => write!(f, "flag_for_review"),
            Self::Notify => write!(f, "notify"),
        }
    }
}

/// Criteria that determine whether a retention rule matches an asset.
#[derive(Debug, Clone, PartialEq)]
pub struct RetentionCriteria {
    /// Maximum asset age in days before the rule triggers.
    pub max_age_days: Option<u64>,
    /// Minimum days since last access before the rule triggers.
    pub min_idle_days: Option<u64>,
    /// Asset must have one of these media types (e.g. `"video"`, `"audio"`).
    pub media_types: Vec<String>,
    /// Asset must have one of these tags.
    pub required_tags: Vec<String>,
    /// Asset must NOT have any of these tags.
    pub excluded_tags: Vec<String>,
    /// Minimum file size in bytes.
    pub min_size_bytes: Option<u64>,
    /// Maximum file size in bytes.
    pub max_size_bytes: Option<u64>,
}

impl Default for RetentionCriteria {
    fn default() -> Self {
        Self {
            max_age_days: None,
            min_idle_days: None,
            media_types: Vec::new(),
            required_tags: Vec::new(),
            excluded_tags: Vec::new(),
            min_size_bytes: None,
            max_size_bytes: None,
        }
    }
}

impl RetentionCriteria {
    /// Create an empty criteria set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum age in days.
    pub fn with_max_age(mut self, days: u64) -> Self {
        self.max_age_days = Some(days);
        self
    }

    /// Set minimum idle days (days since last access).
    pub fn with_min_idle(mut self, days: u64) -> Self {
        self.min_idle_days = Some(days);
        self
    }

    /// Restrict to specific media types.
    pub fn with_media_types(mut self, types: Vec<String>) -> Self {
        self.media_types = types;
        self
    }

    /// Restrict to assets with specific tags.
    pub fn with_required_tags(mut self, tags: Vec<String>) -> Self {
        self.required_tags = tags;
        self
    }

    /// Exclude assets with specific tags.
    pub fn with_excluded_tags(mut self, tags: Vec<String>) -> Self {
        self.excluded_tags = tags;
        self
    }

    /// Set minimum file size filter.
    pub fn with_min_size(mut self, bytes: u64) -> Self {
        self.min_size_bytes = Some(bytes);
        self
    }
}

/// Lightweight snapshot of an asset for retention evaluation.
#[derive(Debug, Clone)]
pub struct AssetSnapshot {
    /// Asset identifier string.
    pub id: String,
    /// Age of the asset in days.
    pub age_days: u64,
    /// Days since last access.
    pub idle_days: u64,
    /// Media type label.
    pub media_type: String,
    /// Tags associated with the asset.
    pub tags: Vec<String>,
    /// File size in bytes.
    pub size_bytes: u64,
}

/// Check whether a set of criteria matches an asset snapshot.
#[allow(clippy::cast_precision_loss)]
pub fn matches_criteria(criteria: &RetentionCriteria, asset: &AssetSnapshot) -> bool {
    if let Some(max_age) = criteria.max_age_days {
        if asset.age_days < max_age {
            return false;
        }
    }
    if let Some(min_idle) = criteria.min_idle_days {
        if asset.idle_days < min_idle {
            return false;
        }
    }
    if !criteria.media_types.is_empty() && !criteria.media_types.contains(&asset.media_type) {
        return false;
    }
    if !criteria.required_tags.is_empty()
        && !criteria
            .required_tags
            .iter()
            .any(|t| asset.tags.contains(t))
    {
        return false;
    }
    if criteria
        .excluded_tags
        .iter()
        .any(|t| asset.tags.contains(t))
    {
        return false;
    }
    if let Some(min_size) = criteria.min_size_bytes {
        if asset.size_bytes < min_size {
            return false;
        }
    }
    if let Some(max_size) = criteria.max_size_bytes {
        if asset.size_bytes > max_size {
            return false;
        }
    }
    true
}

/// A single retention rule that pairs criteria with an action.
#[derive(Debug, Clone)]
pub struct RetentionRule {
    /// Human-readable name.
    pub name: String,
    /// Evaluation priority (lower = higher priority).
    pub priority: u32,
    /// Whether the rule is currently enabled.
    pub enabled: bool,
    /// Criteria that must match.
    pub criteria: RetentionCriteria,
    /// Action to apply when matched.
    pub action: RetentionAction,
}

impl RetentionRule {
    /// Create a new retention rule.
    pub fn new(
        name: impl Into<String>,
        criteria: RetentionCriteria,
        action: RetentionAction,
    ) -> Self {
        Self {
            name: name.into(),
            priority: 100,
            enabled: true,
            criteria,
            action,
        }
    }

    /// Set the priority of this rule.
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Enable or disable this rule.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

/// The result of evaluating a single asset against the policy engine.
#[derive(Debug, Clone, PartialEq)]
pub struct PolicyEvalResult {
    /// Asset identifier.
    pub asset_id: String,
    /// Name of the rule that matched (if any).
    pub matched_rule: Option<String>,
    /// Action to take.
    pub action: Option<RetentionAction>,
}

/// The retention policy engine that holds all rules and evaluates assets.
#[derive(Debug)]
pub struct RetentionPolicyEngine {
    /// All registered rules, keyed by policy ID.
    rules: HashMap<PolicyId, RetentionRule>,
    /// Auto-incrementing ID counter.
    next_id: u64,
}

impl Default for RetentionPolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RetentionPolicyEngine {
    /// Create a new empty policy engine.
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
            next_id: 1,
        }
    }

    /// Add a rule and return its assigned policy ID.
    pub fn add_rule(&mut self, rule: RetentionRule) -> PolicyId {
        let id = PolicyId::new(self.next_id);
        self.next_id += 1;
        self.rules.insert(id, rule);
        id
    }

    /// Remove a rule by its policy ID. Returns `true` if it existed.
    pub fn remove_rule(&mut self, id: PolicyId) -> bool {
        self.rules.remove(&id).is_some()
    }

    /// Return the number of rules in the engine.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Evaluate a single asset against all enabled rules.
    /// Returns the first matching rule by priority.
    pub fn evaluate(&self, asset: &AssetSnapshot) -> PolicyEvalResult {
        let mut sorted: Vec<_> = self.rules.values().filter(|r| r.enabled).collect();
        sorted.sort_by_key(|r| r.priority);

        for rule in sorted {
            if matches_criteria(&rule.criteria, asset) {
                return PolicyEvalResult {
                    asset_id: asset.id.clone(),
                    matched_rule: Some(rule.name.clone()),
                    action: Some(rule.action),
                };
            }
        }

        PolicyEvalResult {
            asset_id: asset.id.clone(),
            matched_rule: None,
            action: None,
        }
    }

    /// Evaluate a batch of assets. Returns one result per asset.
    pub fn evaluate_batch(&self, assets: &[AssetSnapshot]) -> Vec<PolicyEvalResult> {
        assets.iter().map(|a| self.evaluate(a)).collect()
    }

    /// Get a reference to a rule by ID.
    pub fn get_rule(&self, id: PolicyId) -> Option<&RetentionRule> {
        self.rules.get(&id)
    }

    /// Get a mutable reference to a rule by ID.
    pub fn get_rule_mut(&mut self, id: PolicyId) -> Option<&mut RetentionRule> {
        self.rules.get_mut(&id)
    }

    /// Return all policy IDs.
    pub fn policy_ids(&self) -> Vec<PolicyId> {
        self.rules.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_asset(id: &str, age: u64, idle: u64, media: &str, size: u64) -> AssetSnapshot {
        AssetSnapshot {
            id: id.to_string(),
            age_days: age,
            idle_days: idle,
            media_type: media.to_string(),
            tags: vec!["production".to_string()],
            size_bytes: size,
        }
    }

    #[test]
    fn test_policy_id_display() {
        let id = PolicyId::new(42);
        assert_eq!(id.to_string(), "policy-42");
        assert_eq!(id.value(), 42);
    }

    #[test]
    fn test_retention_action_display() {
        assert_eq!(RetentionAction::Archive.to_string(), "archive");
        assert_eq!(RetentionAction::Delete.to_string(), "delete");
        assert_eq!(RetentionAction::TierDown.to_string(), "tier_down");
        assert_eq!(
            RetentionAction::FlagForReview.to_string(),
            "flag_for_review"
        );
        assert_eq!(RetentionAction::Notify.to_string(), "notify");
    }

    #[test]
    fn test_criteria_default() {
        let c = RetentionCriteria::default();
        assert!(c.max_age_days.is_none());
        assert!(c.min_idle_days.is_none());
        assert!(c.media_types.is_empty());
    }

    #[test]
    fn test_criteria_builder() {
        let c = RetentionCriteria::new()
            .with_max_age(365)
            .with_min_idle(90)
            .with_media_types(vec!["video".to_string()])
            .with_min_size(1024);
        assert_eq!(c.max_age_days, Some(365));
        assert_eq!(c.min_idle_days, Some(90));
        assert_eq!(c.media_types, vec!["video"]);
        assert_eq!(c.min_size_bytes, Some(1024));
    }

    #[test]
    fn test_matches_criteria_age() {
        let c = RetentionCriteria::new().with_max_age(30);
        let young = sample_asset("a1", 10, 5, "video", 1000);
        let old = sample_asset("a2", 60, 5, "video", 1000);
        assert!(!matches_criteria(&c, &young));
        assert!(matches_criteria(&c, &old));
    }

    #[test]
    fn test_matches_criteria_idle() {
        let c = RetentionCriteria::new().with_min_idle(90);
        let active = sample_asset("a1", 100, 10, "video", 1000);
        let idle = sample_asset("a2", 100, 120, "video", 1000);
        assert!(!matches_criteria(&c, &active));
        assert!(matches_criteria(&c, &idle));
    }

    #[test]
    fn test_matches_criteria_media_type() {
        let c = RetentionCriteria::new().with_media_types(vec!["audio".to_string()]);
        let video = sample_asset("a1", 100, 100, "video", 1000);
        let audio = sample_asset("a2", 100, 100, "audio", 1000);
        assert!(!matches_criteria(&c, &video));
        assert!(matches_criteria(&c, &audio));
    }

    #[test]
    fn test_matches_criteria_excluded_tags() {
        let c = RetentionCriteria::new().with_excluded_tags(vec!["production".to_string()]);
        let asset = sample_asset("a1", 100, 100, "video", 1000);
        assert!(!matches_criteria(&c, &asset));
    }

    #[test]
    fn test_matches_criteria_size_range() {
        let mut c = RetentionCriteria::new().with_min_size(500);
        c.max_size_bytes = Some(2000);
        let small = sample_asset("a1", 100, 100, "video", 100);
        let mid = sample_asset("a2", 100, 100, "video", 1000);
        let big = sample_asset("a3", 100, 100, "video", 5000);
        assert!(!matches_criteria(&c, &small));
        assert!(matches_criteria(&c, &mid));
        assert!(!matches_criteria(&c, &big));
    }

    #[test]
    fn test_engine_add_remove_rules() {
        let mut engine = RetentionPolicyEngine::new();
        assert_eq!(engine.rule_count(), 0);
        let id = engine.add_rule(RetentionRule::new(
            "test",
            RetentionCriteria::new(),
            RetentionAction::Archive,
        ));
        assert_eq!(engine.rule_count(), 1);
        assert!(engine.remove_rule(id));
        assert_eq!(engine.rule_count(), 0);
        assert!(!engine.remove_rule(id));
    }

    #[test]
    fn test_engine_evaluate_no_match() {
        let engine = RetentionPolicyEngine::new();
        let asset = sample_asset("a1", 10, 5, "video", 1000);
        let result = engine.evaluate(&asset);
        assert!(result.matched_rule.is_none());
        assert!(result.action.is_none());
    }

    #[test]
    fn test_engine_evaluate_match() {
        let mut engine = RetentionPolicyEngine::new();
        engine.add_rule(RetentionRule::new(
            "archive_old",
            RetentionCriteria::new().with_max_age(30),
            RetentionAction::Archive,
        ));
        let asset = sample_asset("a1", 60, 5, "video", 1000);
        let result = engine.evaluate(&asset);
        assert_eq!(result.matched_rule.as_deref(), Some("archive_old"));
        assert_eq!(result.action, Some(RetentionAction::Archive));
    }

    #[test]
    fn test_engine_priority_ordering() {
        let mut engine = RetentionPolicyEngine::new();
        engine.add_rule(
            RetentionRule::new(
                "low_priority",
                RetentionCriteria::new().with_max_age(30),
                RetentionAction::Notify,
            )
            .with_priority(200),
        );
        engine.add_rule(
            RetentionRule::new(
                "high_priority",
                RetentionCriteria::new().with_max_age(30),
                RetentionAction::Delete,
            )
            .with_priority(10),
        );
        let asset = sample_asset("a1", 60, 5, "video", 1000);
        let result = engine.evaluate(&asset);
        assert_eq!(result.matched_rule.as_deref(), Some("high_priority"));
        assert_eq!(result.action, Some(RetentionAction::Delete));
    }

    #[test]
    fn test_engine_disabled_rule_skipped() {
        let mut engine = RetentionPolicyEngine::new();
        let id = engine.add_rule(RetentionRule::new(
            "disabled",
            RetentionCriteria::new().with_max_age(1),
            RetentionAction::Delete,
        ));
        engine
            .get_rule_mut(id)
            .expect("should succeed in test")
            .set_enabled(false);
        let asset = sample_asset("a1", 999, 999, "video", 1000);
        let result = engine.evaluate(&asset);
        assert!(result.action.is_none());
    }

    #[test]
    fn test_evaluate_batch() {
        let mut engine = RetentionPolicyEngine::new();
        engine.add_rule(RetentionRule::new(
            "archive",
            RetentionCriteria::new().with_max_age(30),
            RetentionAction::Archive,
        ));
        let assets = vec![
            sample_asset("a1", 10, 5, "video", 1000),
            sample_asset("a2", 60, 5, "video", 1000),
        ];
        let results = engine.evaluate_batch(&assets);
        assert_eq!(results.len(), 2);
        assert!(results[0].action.is_none());
        assert_eq!(results[1].action, Some(RetentionAction::Archive));
    }

    #[test]
    fn test_policy_ids() {
        let mut engine = RetentionPolicyEngine::new();
        let id1 = engine.add_rule(RetentionRule::new(
            "r1",
            RetentionCriteria::new(),
            RetentionAction::Archive,
        ));
        let id2 = engine.add_rule(RetentionRule::new(
            "r2",
            RetentionCriteria::new(),
            RetentionAction::Delete,
        ));
        let ids = engine.policy_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }
}
