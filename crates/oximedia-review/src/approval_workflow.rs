//! Approval workflow management for review sessions.
//!
//! Provides multi-stage approval workflows, approver lists, escalation policies,
//! and deadline tracking for collaborative media review.

#![allow(dead_code)]

use std::collections::HashMap;

/// Unique identifier for an approval workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkflowId(u64);

impl WorkflowId {
    /// Create a new workflow ID.
    #[must_use]
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the inner value.
    #[must_use]
    pub fn value(self) -> u64 {
        self.0
    }
}

/// Unique identifier for an approval stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StageId(u32);

impl StageId {
    /// Create a new stage ID.
    #[must_use]
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the inner value.
    #[must_use]
    pub fn value(self) -> u32 {
        self.0
    }
}

/// Status of an approval stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageStatus {
    /// Stage has not started yet.
    Pending,
    /// Stage is currently active and awaiting decisions.
    Active,
    /// All required approvers have approved.
    Approved,
    /// At least one approver has rejected.
    Rejected,
    /// Stage was skipped due to escalation or conditions.
    Skipped,
    /// Stage deadline was exceeded without completion.
    Expired,
}

/// Decision made by an approver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// Approve the content.
    Approve,
    /// Reject the content with required changes.
    Reject,
    /// Request changes without blocking.
    RequestChanges,
    /// Abstain from decision.
    Abstain,
}

/// An approver assigned to a stage.
#[derive(Debug, Clone)]
pub struct Approver {
    /// Approver user ID.
    pub user_id: String,
    /// Display name of the approver.
    pub name: String,
    /// Whether this approver's decision is required.
    pub required: bool,
    /// Decision if made.
    pub decision: Option<ApprovalDecision>,
    /// Timestamp of decision in milliseconds since epoch.
    pub decided_at_ms: Option<u64>,
    /// Optional comment attached to the decision.
    pub comment: Option<String>,
}

impl Approver {
    /// Create a new required approver.
    #[must_use]
    pub fn required(user_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            name: name.into(),
            required: true,
            decision: None,
            decided_at_ms: None,
            comment: None,
        }
    }

    /// Create a new optional approver.
    #[must_use]
    pub fn optional(user_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            name: name.into(),
            required: false,
            decision: None,
            decided_at_ms: None,
            comment: None,
        }
    }

    /// Record a decision.
    pub fn decide(&mut self, decision: ApprovalDecision, comment: Option<String>, now_ms: u64) {
        self.decision = Some(decision);
        self.decided_at_ms = Some(now_ms);
        self.comment = comment;
    }

    /// Returns true if this approver has made a decision.
    #[must_use]
    pub fn has_decided(&self) -> bool {
        self.decision.is_some()
    }
}

/// Escalation policy for a stage.
#[derive(Debug, Clone)]
pub struct EscalationPolicy {
    /// Deadline in milliseconds from stage activation.
    pub deadline_ms: u64,
    /// User ID to escalate to when deadline is exceeded.
    pub escalate_to: String,
    /// Whether to auto-approve if deadline is exceeded without rejection.
    pub auto_approve_on_timeout: bool,
}

impl EscalationPolicy {
    /// Create a new escalation policy.
    #[must_use]
    pub fn new(
        deadline_ms: u64,
        escalate_to: impl Into<String>,
        auto_approve_on_timeout: bool,
    ) -> Self {
        Self {
            deadline_ms,
            escalate_to: escalate_to.into(),
            auto_approve_on_timeout,
        }
    }

    /// Check whether the deadline has passed.
    #[must_use]
    pub fn is_expired(&self, activated_at_ms: u64, now_ms: u64) -> bool {
        now_ms.saturating_sub(activated_at_ms) > self.deadline_ms
    }
}

/// A single stage in an approval workflow.
#[derive(Debug, Clone)]
pub struct ApprovalStage {
    /// Stage identifier.
    pub id: StageId,
    /// Human-readable name of the stage.
    pub name: String,
    /// Approvers assigned to this stage.
    pub approvers: Vec<Approver>,
    /// Current status of the stage.
    pub status: StageStatus,
    /// Timestamp when the stage became active.
    pub activated_at_ms: Option<u64>,
    /// Optional escalation policy.
    pub escalation: Option<EscalationPolicy>,
    /// Minimum number of approvals needed (0 = all required approvers).
    pub min_approvals: usize,
}

impl ApprovalStage {
    /// Create a new approval stage.
    #[must_use]
    pub fn new(id: StageId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            approvers: Vec::new(),
            status: StageStatus::Pending,
            activated_at_ms: None,
            escalation: None,
            min_approvals: 0,
        }
    }

    /// Add an approver to this stage.
    pub fn add_approver(&mut self, approver: Approver) {
        self.approvers.push(approver);
    }

    /// Set an escalation policy.
    pub fn set_escalation(&mut self, policy: EscalationPolicy) {
        self.escalation = Some(policy);
    }

    /// Activate this stage.
    pub fn activate(&mut self, now_ms: u64) {
        self.status = StageStatus::Active;
        self.activated_at_ms = Some(now_ms);
    }

    /// Count approvals from required approvers.
    #[must_use]
    pub fn required_approval_count(&self) -> usize {
        self.approvers
            .iter()
            .filter(|a| a.required && a.decision == Some(ApprovalDecision::Approve))
            .count()
    }

    /// Count total required approvers.
    #[must_use]
    pub fn total_required(&self) -> usize {
        self.approvers.iter().filter(|a| a.required).count()
    }

    /// Check whether any required approver has rejected.
    #[must_use]
    pub fn has_rejection(&self) -> bool {
        self.approvers
            .iter()
            .any(|a| a.required && a.decision == Some(ApprovalDecision::Reject))
    }

    /// Evaluate and update stage status based on current decisions.
    pub fn evaluate(&mut self, now_ms: u64) {
        if self.status != StageStatus::Active {
            return;
        }

        if self.has_rejection() {
            self.status = StageStatus::Rejected;
            return;
        }

        let needed = if self.min_approvals == 0 {
            self.total_required()
        } else {
            self.min_approvals
        };

        if self.required_approval_count() >= needed {
            self.status = StageStatus::Approved;
            return;
        }

        // Check escalation deadline
        if let (Some(policy), Some(activated)) = (&self.escalation, self.activated_at_ms) {
            if policy.is_expired(activated, now_ms) {
                if policy.auto_approve_on_timeout {
                    self.status = StageStatus::Approved;
                } else {
                    self.status = StageStatus::Expired;
                }
            }
        }
    }
}

/// An approval workflow containing multiple sequential stages.
#[derive(Debug)]
pub struct ApprovalWorkflow {
    /// Workflow identifier.
    pub id: WorkflowId,
    /// Workflow name.
    pub name: String,
    /// Ordered list of stages.
    pub stages: Vec<ApprovalStage>,
    /// Index of the currently active stage.
    pub current_stage: usize,
    /// Metadata attached to this workflow.
    pub metadata: HashMap<String, String>,
}

impl ApprovalWorkflow {
    /// Create a new approval workflow.
    #[must_use]
    pub fn new(id: WorkflowId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            stages: Vec::new(),
            current_stage: 0,
            metadata: HashMap::new(),
        }
    }

    /// Add a stage to the workflow.
    pub fn add_stage(&mut self, stage: ApprovalStage) {
        self.stages.push(stage);
    }

    /// Start the workflow by activating the first stage.
    pub fn start(&mut self, now_ms: u64) -> bool {
        if self.stages.is_empty() {
            return false;
        }
        self.current_stage = 0;
        self.stages[0].activate(now_ms);
        true
    }

    /// Advance to the next stage if the current one is approved.
    pub fn advance(&mut self, now_ms: u64) -> bool {
        let next = self.current_stage + 1;
        if next >= self.stages.len() {
            return false;
        }
        self.current_stage = next;
        self.stages[next].activate(now_ms);
        true
    }

    /// Get the current active stage.
    #[must_use]
    pub fn current(&self) -> Option<&ApprovalStage> {
        self.stages.get(self.current_stage)
    }

    /// Get the current active stage mutably.
    #[must_use]
    pub fn current_mut(&mut self) -> Option<&mut ApprovalStage> {
        self.stages.get_mut(self.current_stage)
    }

    /// Returns true if all stages have been approved.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.stages
            .iter()
            .all(|s| matches!(s.status, StageStatus::Approved | StageStatus::Skipped))
    }

    /// Returns true if any stage has been rejected.
    #[must_use]
    pub fn is_rejected(&self) -> bool {
        self.stages
            .iter()
            .any(|s| s.status == StageStatus::Rejected)
    }

    /// Count completed stages.
    #[must_use]
    pub fn completed_stage_count(&self) -> usize {
        self.stages
            .iter()
            .filter(|s| {
                matches!(
                    s.status,
                    StageStatus::Approved | StageStatus::Skipped | StageStatus::Rejected
                )
            })
            .count()
    }

    /// Insert metadata.
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_stage(id: u32, name: &str) -> ApprovalStage {
        ApprovalStage::new(StageId::new(id), name)
    }

    fn make_workflow() -> ApprovalWorkflow {
        ApprovalWorkflow::new(WorkflowId::new(1), "Test Workflow")
    }

    #[test]
    fn test_workflow_id_value() {
        let id = WorkflowId::new(42);
        assert_eq!(id.value(), 42);
    }

    #[test]
    fn test_stage_id_value() {
        let id = StageId::new(7);
        assert_eq!(id.value(), 7);
    }

    #[test]
    fn test_approver_required_creation() {
        let a = Approver::required("u1", "Alice");
        assert!(a.required);
        assert!(!a.has_decided());
    }

    #[test]
    fn test_approver_optional_creation() {
        let a = Approver::optional("u2", "Bob");
        assert!(!a.required);
    }

    #[test]
    fn test_approver_decide() {
        let mut a = Approver::required("u1", "Alice");
        a.decide(ApprovalDecision::Approve, Some("LGTM".into()), 1000);
        assert!(a.has_decided());
        assert_eq!(a.decision, Some(ApprovalDecision::Approve));
        assert_eq!(a.decided_at_ms, Some(1000));
    }

    #[test]
    fn test_escalation_policy_expired() {
        let policy = EscalationPolicy::new(5000, "manager", false);
        assert!(!policy.is_expired(0, 4000));
        assert!(policy.is_expired(0, 6000));
    }

    #[test]
    fn test_stage_activate() {
        let mut stage = make_stage(1, "Review");
        stage.activate(1000);
        assert_eq!(stage.status, StageStatus::Active);
        assert_eq!(stage.activated_at_ms, Some(1000));
    }

    #[test]
    fn test_stage_evaluate_approved() {
        let mut stage = make_stage(1, "Review");
        let mut a = Approver::required("u1", "Alice");
        a.decide(ApprovalDecision::Approve, None, 1000);
        stage.add_approver(a);
        stage.activate(0);
        stage.evaluate(1000);
        assert_eq!(stage.status, StageStatus::Approved);
    }

    #[test]
    fn test_stage_evaluate_rejected() {
        let mut stage = make_stage(1, "Review");
        let mut a = Approver::required("u1", "Alice");
        a.decide(ApprovalDecision::Reject, None, 1000);
        stage.add_approver(a);
        stage.activate(0);
        stage.evaluate(1000);
        assert_eq!(stage.status, StageStatus::Rejected);
    }

    #[test]
    fn test_stage_evaluate_expired() {
        let mut stage = make_stage(1, "Review");
        stage.add_approver(Approver::required("u1", "Alice"));
        stage.set_escalation(EscalationPolicy::new(5000, "manager", false));
        stage.activate(0);
        stage.evaluate(6000);
        assert_eq!(stage.status, StageStatus::Expired);
    }

    #[test]
    fn test_stage_evaluate_auto_approve_on_timeout() {
        let mut stage = make_stage(1, "Review");
        stage.add_approver(Approver::required("u1", "Alice"));
        stage.set_escalation(EscalationPolicy::new(5000, "manager", true));
        stage.activate(0);
        stage.evaluate(6000);
        assert_eq!(stage.status, StageStatus::Approved);
    }

    #[test]
    fn test_workflow_start() {
        let mut wf = make_workflow();
        wf.add_stage(make_stage(1, "Stage 1"));
        wf.add_stage(make_stage(2, "Stage 2"));
        let started = wf.start(1000);
        assert!(started);
        assert_eq!(
            wf.current().expect("should succeed in test").status,
            StageStatus::Active
        );
    }

    #[test]
    fn test_workflow_advance() {
        let mut wf = make_workflow();
        wf.add_stage(make_stage(1, "Stage 1"));
        wf.add_stage(make_stage(2, "Stage 2"));
        wf.start(0);
        let advanced = wf.advance(1000);
        assert!(advanced);
        assert_eq!(wf.current_stage, 1);
        assert_eq!(
            wf.current().expect("should succeed in test").status,
            StageStatus::Active
        );
    }

    #[test]
    fn test_workflow_is_complete_all_approved() {
        let mut wf = make_workflow();
        let mut s1 = make_stage(1, "Stage 1");
        s1.status = StageStatus::Approved;
        let mut s2 = make_stage(2, "Stage 2");
        s2.status = StageStatus::Approved;
        wf.add_stage(s1);
        wf.add_stage(s2);
        assert!(wf.is_complete());
    }

    #[test]
    fn test_workflow_is_rejected() {
        let mut wf = make_workflow();
        let mut s1 = make_stage(1, "Stage 1");
        s1.status = StageStatus::Rejected;
        wf.add_stage(s1);
        assert!(wf.is_rejected());
    }

    #[test]
    fn test_workflow_completed_stage_count() {
        let mut wf = make_workflow();
        let mut s1 = make_stage(1, "Stage 1");
        s1.status = StageStatus::Approved;
        let s2 = make_stage(2, "Stage 2");
        wf.add_stage(s1);
        wf.add_stage(s2);
        assert_eq!(wf.completed_stage_count(), 1);
    }

    #[test]
    fn test_workflow_metadata() {
        let mut wf = make_workflow();
        wf.set_metadata("project", "acme-ad");
        assert_eq!(
            wf.metadata.get("project").map(String::as_str),
            Some("acme-ad")
        );
    }

    #[test]
    fn test_workflow_start_empty_returns_false() {
        let mut wf = make_workflow();
        assert!(!wf.start(0));
    }
}
