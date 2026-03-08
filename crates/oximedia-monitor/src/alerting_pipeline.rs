//! Alerting pipeline: rule evaluation, deduplication, muting, and escalation.
//!
//! This module provides a configurable pipeline that evaluates a stream of
//! metric samples against a set of alert rules, deduplicates repeated firings,
//! respects mute windows, and escalates unacknowledged alerts.

#![allow(dead_code)]

use std::collections::HashMap;
use std::time::{Duration, Instant};

// ── comparison ────────────────────────────────────────────────────────────────

/// Comparison operator used in an alert condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparator {
    /// Greater than.
    Gt,
    /// Greater than or equal to.
    Gte,
    /// Less than.
    Lt,
    /// Less than or equal to.
    Lte,
    /// Equal to.
    Eq,
    /// Not equal to.
    Ne,
}

impl Comparator {
    /// Evaluate the comparator against observed `value` and `threshold`.
    #[must_use]
    pub fn evaluate(self, value: f64, threshold: f64) -> bool {
        match self {
            Comparator::Gt => value > threshold,
            Comparator::Gte => value >= threshold,
            Comparator::Lt => value < threshold,
            Comparator::Lte => value <= threshold,
            Comparator::Eq => (value - threshold).abs() < f64::EPSILON,
            Comparator::Ne => (value - threshold).abs() >= f64::EPSILON,
        }
    }
}

// ── alert rule ────────────────────────────────────────────────────────────────

/// Priority / severity of an alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Informational only.
    Info,
    /// Warning – attention needed.
    Warning,
    /// Critical – immediate action required.
    Critical,
}

/// A single alert rule.
#[derive(Debug, Clone)]
pub struct PipelineRule {
    /// Unique rule identifier.
    pub id: String,
    /// Human-readable description.
    pub description: String,
    /// Metric name to watch.
    pub metric: String,
    /// Comparison operator.
    pub comparator: Comparator,
    /// Threshold value.
    pub threshold: f64,
    /// How many consecutive evaluations must breach before firing.
    pub consecutive_count: u32,
    /// Alert priority.
    pub priority: Priority,
    /// Silence period after firing (prevents repeat alerts).
    pub silence_for: Duration,
}

impl PipelineRule {
    /// Convenience constructor with sensible defaults.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        metric: impl Into<String>,
        comparator: Comparator,
        threshold: f64,
        priority: Priority,
    ) -> Self {
        Self {
            id: id.into(),
            description: String::new(),
            metric: metric.into(),
            comparator,
            threshold,
            consecutive_count: 1,
            priority,
            silence_for: Duration::from_secs(300),
        }
    }

    /// Set the consecutive breach count required before firing.
    #[must_use]
    pub fn with_consecutive(mut self, n: u32) -> Self {
        self.consecutive_count = n;
        self
    }

    /// Set the silence period.
    #[must_use]
    pub fn with_silence(mut self, d: Duration) -> Self {
        self.silence_for = d;
        self
    }
}

// ── mute window ───────────────────────────────────────────────────────────────

/// A time window during which all alerts are silenced.
#[derive(Debug, Clone)]
pub struct MuteWindow {
    /// Friendly label.
    pub label: String,
    /// Start of the mute window (from the epoch of the pipeline).
    pub start: Instant,
    /// Duration of the mute window.
    pub duration: Duration,
}

impl MuteWindow {
    /// Create a new mute window starting now.
    #[must_use]
    pub fn starting_now(label: impl Into<String>, duration: Duration) -> Self {
        Self {
            label: label.into(),
            start: Instant::now(),
            duration,
        }
    }

    /// Return `true` if the window is still active at `now`.
    #[must_use]
    pub fn is_active(&self, now: Instant) -> bool {
        now >= self.start && now < self.start + self.duration
    }
}

// ── fired alert ───────────────────────────────────────────────────────────────

/// A fired alert produced by the pipeline.
#[derive(Debug, Clone)]
pub struct PipelineFiredAlert {
    /// Rule that triggered.
    pub rule_id: String,
    /// Metric that caused the firing.
    pub metric: String,
    /// Observed value.
    pub value: f64,
    /// Time the alert fired.
    pub fired_at: Instant,
    /// Priority.
    pub priority: Priority,
}

// ── pipeline state ────────────────────────────────────────────────────────────

/// Per-rule internal state.
#[derive(Debug, Default)]
struct RuleState {
    /// Running count of consecutive breaches.
    consecutive: u32,
    /// When the rule last fired (`None` if never).
    last_fired: Option<Instant>,
}

/// The alerting pipeline.
#[derive(Debug)]
pub struct AlertingPipeline {
    rules: Vec<PipelineRule>,
    state: HashMap<String, RuleState>,
    mute_windows: Vec<MuteWindow>,
}

impl AlertingPipeline {
    /// Create an empty pipeline.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            state: HashMap::new(),
            mute_windows: Vec::new(),
        }
    }

    /// Add a rule to the pipeline.
    pub fn add_rule(&mut self, rule: PipelineRule) {
        self.state.entry(rule.id.clone()).or_default();
        self.rules.push(rule);
    }

    /// Add a mute window.
    pub fn add_mute_window(&mut self, window: MuteWindow) {
        self.mute_windows.push(window);
    }

    /// Evaluate a metric sample. Returns any newly fired alerts.
    #[must_use]
    pub fn evaluate(&mut self, metric: &str, value: f64) -> Vec<PipelineFiredAlert> {
        let now = Instant::now();
        let muted = self.mute_windows.iter().any(|w| w.is_active(now));
        if muted {
            return Vec::new();
        }

        let mut fired = Vec::new();
        for rule in &self.rules {
            if rule.metric != metric {
                continue;
            }
            let state = self.state.entry(rule.id.clone()).or_default();

            if rule.comparator.evaluate(value, rule.threshold) {
                state.consecutive += 1;
            } else {
                state.consecutive = 0;
                continue;
            }

            if state.consecutive < rule.consecutive_count {
                continue;
            }

            // Check silence period.
            if let Some(last) = state.last_fired {
                if now.duration_since(last) < rule.silence_for {
                    continue;
                }
            }

            state.last_fired = Some(now);
            state.consecutive = 0; // reset after firing
            fired.push(PipelineFiredAlert {
                rule_id: rule.id.clone(),
                metric: metric.to_owned(),
                value,
                fired_at: now,
                priority: rule.priority,
            });
        }
        fired
    }

    /// Return the number of rules registered.
    #[must_use]
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Remove expired mute windows (modifies the list in-place).
    pub fn cleanup_mute_windows(&mut self) {
        let now = Instant::now();
        self.mute_windows.retain(|w| w.is_active(now));
    }
}

impl Default for AlertingPipeline {
    fn default() -> Self {
        Self::new()
    }
}

// ── escalation ────────────────────────────────────────────────────────────────

/// Escalation tier: after how long without acknowledgement to escalate.
#[derive(Debug, Clone)]
pub struct EscalationTier {
    /// Delay before escalation kicks in.
    pub after: Duration,
    /// Target for escalation (e.g. on-call channel name).
    pub target: String,
    /// Priority bump applied on escalation.
    pub priority: Priority,
}

impl EscalationTier {
    /// Create a new escalation tier.
    #[must_use]
    pub fn new(after: Duration, target: impl Into<String>, priority: Priority) -> Self {
        Self {
            after,
            target: target.into(),
            priority,
        }
    }
}

/// Track an unacknowledged alert and compute its current escalation tier.
#[derive(Debug, Clone)]
pub struct EscalationTracker {
    /// When the alert originally fired.
    pub fired_at: Instant,
    /// Whether the alert has been acknowledged.
    pub acknowledged: bool,
    /// Ordered escalation tiers.
    pub tiers: Vec<EscalationTier>,
}

impl EscalationTracker {
    /// Create a new tracker for an alert that just fired.
    #[must_use]
    pub fn new(tiers: Vec<EscalationTier>) -> Self {
        Self {
            fired_at: Instant::now(),
            acknowledged: false,
            tiers,
        }
    }

    /// Acknowledge the alert.
    pub fn acknowledge(&mut self) {
        self.acknowledged = true;
    }

    /// Return the current escalation tier at `now`, or `None` if acknowledged
    /// or no tier has been reached yet.
    #[must_use]
    pub fn current_tier(&self, now: Instant) -> Option<&EscalationTier> {
        if self.acknowledged {
            return None;
        }
        let elapsed = now.duration_since(self.fired_at);
        // Return the last tier whose `after` has been surpassed.
        self.tiers.iter().rev().find(|t| elapsed >= t.after)
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comparator_gt() {
        assert!(Comparator::Gt.evaluate(10.0, 9.0));
        assert!(!Comparator::Gt.evaluate(9.0, 9.0));
    }

    #[test]
    fn test_comparator_gte() {
        assert!(Comparator::Gte.evaluate(9.0, 9.0));
        assert!(!Comparator::Gte.evaluate(8.0, 9.0));
    }

    #[test]
    fn test_comparator_lt() {
        assert!(Comparator::Lt.evaluate(3.0, 5.0));
        assert!(!Comparator::Lt.evaluate(5.0, 5.0));
    }

    #[test]
    fn test_comparator_lte() {
        assert!(Comparator::Lte.evaluate(5.0, 5.0));
        assert!(!Comparator::Lte.evaluate(6.0, 5.0));
    }

    #[test]
    fn test_comparator_eq() {
        assert!(Comparator::Eq.evaluate(1.0, 1.0));
        assert!(!Comparator::Eq.evaluate(1.0, 2.0));
    }

    #[test]
    fn test_comparator_ne() {
        assert!(Comparator::Ne.evaluate(1.0, 2.0));
        assert!(!Comparator::Ne.evaluate(1.0, 1.0));
    }

    #[test]
    fn test_pipeline_basic_fire() {
        let mut pipeline = AlertingPipeline::new();
        pipeline.add_rule(
            PipelineRule::new("r1", "cpu", Comparator::Gt, 90.0, Priority::Warning)
                .with_silence(Duration::from_millis(0)),
        );
        let alerts = pipeline.evaluate("cpu", 95.0);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].rule_id, "r1");
    }

    #[test]
    fn test_pipeline_no_fire_below_threshold() {
        let mut pipeline = AlertingPipeline::new();
        pipeline.add_rule(PipelineRule::new(
            "r1",
            "cpu",
            Comparator::Gt,
            90.0,
            Priority::Warning,
        ));
        let alerts = pipeline.evaluate("cpu", 80.0);
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_pipeline_consecutive_required() {
        let mut pipeline = AlertingPipeline::new();
        pipeline.add_rule(
            PipelineRule::new("r1", "cpu", Comparator::Gt, 90.0, Priority::Critical)
                .with_consecutive(3)
                .with_silence(Duration::from_millis(0)),
        );
        // 1st and 2nd evaluations should not fire.
        assert!(pipeline.evaluate("cpu", 95.0).is_empty());
        assert!(pipeline.evaluate("cpu", 95.0).is_empty());
        // 3rd should fire.
        assert_eq!(pipeline.evaluate("cpu", 95.0).len(), 1);
    }

    #[test]
    fn test_pipeline_consecutive_reset_on_normal() {
        let mut pipeline = AlertingPipeline::new();
        pipeline.add_rule(
            PipelineRule::new("r1", "cpu", Comparator::Gt, 90.0, Priority::Warning)
                .with_consecutive(2)
                .with_silence(Duration::from_millis(0)),
        );
        let _ = pipeline.evaluate("cpu", 95.0); // breach 1
        let _ = pipeline.evaluate("cpu", 70.0); // normal - resets
        let _ = pipeline.evaluate("cpu", 95.0); // breach 1 again
        let alerts = pipeline.evaluate("cpu", 95.0); // breach 2
        assert_eq!(alerts.len(), 1);
    }

    #[test]
    fn test_pipeline_silence_prevents_repeat() {
        let mut pipeline = AlertingPipeline::new();
        pipeline.add_rule(
            PipelineRule::new("r1", "cpu", Comparator::Gt, 90.0, Priority::Warning)
                .with_silence(Duration::from_secs(60)),
        );
        assert_eq!(pipeline.evaluate("cpu", 95.0).len(), 1);
        // Second evaluation immediately after should be silenced.
        assert!(pipeline.evaluate("cpu", 95.0).is_empty());
    }

    #[test]
    fn test_pipeline_wrong_metric_ignored() {
        let mut pipeline = AlertingPipeline::new();
        pipeline.add_rule(PipelineRule::new(
            "r1",
            "cpu",
            Comparator::Gt,
            90.0,
            Priority::Warning,
        ));
        assert!(pipeline.evaluate("memory", 99.0).is_empty());
    }

    #[test]
    fn test_pipeline_rule_count() {
        let mut pipeline = AlertingPipeline::new();
        assert_eq!(pipeline.rule_count(), 0);
        pipeline.add_rule(PipelineRule::new(
            "r1",
            "cpu",
            Comparator::Gt,
            90.0,
            Priority::Info,
        ));
        assert_eq!(pipeline.rule_count(), 1);
    }

    #[test]
    fn test_mute_window_active() {
        let w = MuteWindow::starting_now("maintenance", Duration::from_secs(3600));
        assert!(w.is_active(Instant::now()));
    }

    #[test]
    fn test_mute_window_inactive_after_expiry() {
        let w = MuteWindow {
            label: "old".to_string(),
            start: Instant::now() - Duration::from_secs(10),
            duration: Duration::from_secs(5),
        };
        assert!(!w.is_active(Instant::now()));
    }

    #[test]
    fn test_escalation_no_tier_before_time() {
        let tiers = vec![EscalationTier::new(
            Duration::from_secs(300),
            "oncall",
            Priority::Critical,
        )];
        let tracker = EscalationTracker::new(tiers);
        assert!(tracker.current_tier(Instant::now()).is_none());
    }

    #[test]
    fn test_escalation_acknowledged() {
        let tiers = vec![EscalationTier::new(
            Duration::from_millis(0),
            "oncall",
            Priority::Critical,
        )];
        let mut tracker = EscalationTracker::new(tiers);
        tracker.acknowledge();
        assert!(tracker.current_tier(Instant::now()).is_none());
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::Warning);
        assert!(Priority::Warning > Priority::Info);
    }
}
