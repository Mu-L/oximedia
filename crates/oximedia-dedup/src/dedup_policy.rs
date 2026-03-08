//! Policy types for controlling deduplication behaviour.
//!
//! Provides `DedupAction`, `DedupPolicy`, `DedupPolicyConfig`, and
//! `DedupDecision` so callers can codify rules about what to do when
//! duplicates are found.

#![allow(dead_code)]

/// Action to take when a duplicate is detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DedupAction {
    /// Delete the duplicate immediately.
    Delete,
    /// Move the duplicate to a quarantine directory.
    Quarantine,
    /// Create a symbolic link pointing to the canonical copy.
    Symlink,
    /// Keep both copies and emit a warning.
    Keep,
    /// Flag the item for manual review.
    Review,
    /// Skip (do nothing, log only).
    Skip,
}

impl DedupAction {
    /// Return `true` if this action permanently modifies or removes data.
    #[must_use]
    pub const fn is_destructive(self) -> bool {
        matches!(self, Self::Delete | Self::Quarantine)
    }

    /// Return a human-readable description of the action.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Delete => "delete duplicate",
            Self::Quarantine => "move to quarantine",
            Self::Symlink => "replace with symlink",
            Self::Keep => "keep both copies",
            Self::Review => "flag for review",
            Self::Skip => "skip / log only",
        }
    }
}

/// Configures the deduplication policy.
#[derive(Debug, Clone)]
pub struct DedupPolicyConfig {
    /// Enable strict mode: require all selected methods to agree before acting.
    pub strict_mode: bool,
    /// Minimum similarity score (0.0–1.0) required to consider items duplicates.
    pub min_similarity: f64,
    /// Action applied when an exact duplicate is found (similarity == 1.0).
    pub exact_action: DedupAction,
    /// Action applied when a near-duplicate is found.
    pub near_action: DedupAction,
    /// Whether to protect files marked as originals from deletion.
    pub protect_originals: bool,
}

impl Default for DedupPolicyConfig {
    fn default() -> Self {
        Self {
            strict_mode: false,
            min_similarity: 0.95,
            exact_action: DedupAction::Quarantine,
            near_action: DedupAction::Review,
            protect_originals: true,
        }
    }
}

impl DedupPolicyConfig {
    /// Return `true` if strict mode is enabled.
    #[must_use]
    pub const fn strict_mode(&self) -> bool {
        self.strict_mode
    }

    /// Return the minimum similarity threshold.
    #[must_use]
    pub fn min_similarity(&self) -> f64 {
        self.min_similarity
    }
}

/// The computed deduplication decision for a candidate pair.
#[derive(Debug, Clone)]
pub struct DedupDecision {
    /// Similarity score in 0.0–1.0.
    pub similarity: f64,
    /// Chosen action.
    pub action: DedupAction,
    /// Whether the decision needs human review.
    pub needs_review: bool,
    /// Optional explanation string.
    pub reason: Option<String>,
}

impl DedupDecision {
    /// Create a new `DedupDecision`.
    #[must_use]
    pub fn new(similarity: f64, action: DedupAction, reason: Option<String>) -> Self {
        let needs_review =
            matches!(action, DedupAction::Review) || (action.is_destructive() && similarity < 1.0);
        Self {
            similarity,
            action,
            needs_review,
            reason,
        }
    }

    /// Return `true` if the decision requires human review before execution.
    #[must_use]
    pub fn requires_review(&self) -> bool {
        self.needs_review
    }
}

/// Evaluates pairs of media items according to a `DedupPolicyConfig`.
#[derive(Debug, Clone)]
pub struct DedupPolicy {
    config: DedupPolicyConfig,
}

impl DedupPolicy {
    /// Create a new `DedupPolicy` from a config.
    #[must_use]
    pub fn new(config: DedupPolicyConfig) -> Self {
        Self { config }
    }

    /// Decide whether two items with the given `similarity` should be deduped.
    ///
    /// Returns a `DedupDecision` describing what to do.
    #[must_use]
    pub fn should_dedup(&self, similarity: f64, is_original: bool) -> DedupDecision {
        // Guard: similarity below threshold → skip.
        if similarity < self.config.min_similarity {
            return DedupDecision::new(
                similarity,
                DedupAction::Skip,
                Some(format!(
                    "similarity {similarity:.3} below threshold {:.3}",
                    self.config.min_similarity
                )),
            );
        }

        // Guard: protect originals.
        if is_original && self.config.protect_originals {
            return DedupDecision::new(
                similarity,
                DedupAction::Keep,
                Some("file is marked as original".to_string()),
            );
        }

        // Exact duplicate.
        #[allow(clippy::float_cmp)]
        if similarity == 1.0 {
            let action = if self.config.strict_mode {
                self.config.exact_action
            } else {
                self.config.exact_action
            };
            return DedupDecision::new(
                similarity,
                action,
                Some("exact duplicate detected".to_string()),
            );
        }

        // Near-duplicate.
        DedupDecision::new(
            similarity,
            self.config.near_action,
            Some(format!("near-duplicate at {similarity:.3}")),
        )
    }

    /// Access the underlying config.
    #[must_use]
    pub const fn config(&self) -> &DedupPolicyConfig {
        &self.config
    }
}

impl Default for DedupPolicy {
    fn default() -> Self {
        Self::new(DedupPolicyConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_is_destructive_delete() {
        assert!(DedupAction::Delete.is_destructive());
    }

    #[test]
    fn test_action_is_destructive_quarantine() {
        assert!(DedupAction::Quarantine.is_destructive());
    }

    #[test]
    fn test_action_not_destructive_keep() {
        assert!(!DedupAction::Keep.is_destructive());
    }

    #[test]
    fn test_action_not_destructive_symlink() {
        assert!(!DedupAction::Symlink.is_destructive());
    }

    #[test]
    fn test_action_description_nonempty() {
        for action in [
            DedupAction::Delete,
            DedupAction::Quarantine,
            DedupAction::Symlink,
            DedupAction::Keep,
            DedupAction::Review,
            DedupAction::Skip,
        ] {
            assert!(!action.description().is_empty());
        }
    }

    #[test]
    fn test_policy_config_defaults() {
        let cfg = DedupPolicyConfig::default();
        assert!(!cfg.strict_mode());
        assert!((cfg.min_similarity() - 0.95).abs() < 1e-9);
        assert!(cfg.protect_originals);
    }

    #[test]
    fn test_policy_skip_below_threshold() {
        let policy = DedupPolicy::default();
        let decision = policy.should_dedup(0.50, false);
        assert_eq!(decision.action, DedupAction::Skip);
        assert!(!decision.requires_review());
    }

    #[test]
    fn test_policy_exact_duplicate() {
        let policy = DedupPolicy::default();
        let decision = policy.should_dedup(1.0, false);
        assert_eq!(decision.action, DedupAction::Quarantine);
    }

    #[test]
    fn test_policy_near_duplicate() {
        let policy = DedupPolicy::default();
        let decision = policy.should_dedup(0.97, false);
        assert_eq!(decision.action, DedupAction::Review);
    }

    #[test]
    fn test_policy_protect_original() {
        let policy = DedupPolicy::default();
        let decision = policy.should_dedup(1.0, true);
        assert_eq!(decision.action, DedupAction::Keep);
    }

    #[test]
    fn test_decision_requires_review_for_review_action() {
        let d = DedupDecision::new(0.97, DedupAction::Review, None);
        assert!(d.requires_review());
    }

    #[test]
    fn test_decision_requires_review_destructive_near_dup() {
        let d = DedupDecision::new(0.97, DedupAction::Delete, None);
        assert!(d.requires_review());
    }

    #[test]
    fn test_decision_no_review_for_exact_destructive() {
        // similarity == 1.0, destructive → NOT near-dup branch, no review flag
        let d = DedupDecision::new(1.0, DedupAction::Delete, None);
        assert!(!d.requires_review());
    }

    #[test]
    fn test_decision_skip_no_review() {
        let d = DedupDecision::new(0.5, DedupAction::Skip, None);
        assert!(!d.requires_review());
    }

    #[test]
    fn test_policy_config_strict_mode_toggle() {
        let mut cfg = DedupPolicyConfig::default();
        cfg.strict_mode = true;
        assert!(cfg.strict_mode());
    }
}
