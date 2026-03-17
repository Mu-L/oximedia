//! Job dependency graph for the render farm.
//!
//! Provides a directed acyclic graph (DAG) of job dependencies, topological
//! sort, cycle detection, and ready-job filtering.

/// A directed dependency graph where an edge `(A, B)` means "A depends on B".
#[allow(dead_code)]
#[derive(Debug, Default, Clone)]
pub struct DependencyGraph {
    /// Registered job IDs (vertices).
    pub nodes: Vec<u64>,
    /// Dependency edges: `(job_id, depends_on)`.
    pub edges: Vec<(u64, u64)>,
}

impl DependencyGraph {
    /// Create an empty dependency graph.
    #[allow(dead_code)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Register a job in the graph.
    /// Does nothing if the job is already present.
    #[allow(dead_code)]
    pub fn add_job(&mut self, job_id: u64) {
        if !self.nodes.contains(&job_id) {
            self.nodes.push(job_id);
        }
    }

    /// Add a dependency: `job_id` depends on `depends_on`.
    ///
    /// Both jobs must already be registered. Returns an error if either is
    /// unknown or if adding the edge would create a cycle.
    #[allow(dead_code)]
    pub fn add_dependency(&mut self, job_id: u64, depends_on: u64) -> Result<(), String> {
        if !self.nodes.contains(&job_id) {
            return Err(format!("Unknown job: {job_id}"));
        }
        if !self.nodes.contains(&depends_on) {
            return Err(format!("Unknown dependency target: {depends_on}"));
        }
        // Temporarily add edge and check for cycle
        self.edges.push((job_id, depends_on));
        if self.has_cycle() {
            self.edges.pop();
            return Err(format!(
                "Adding dependency {job_id} -> {depends_on} would create a cycle"
            ));
        }
        Ok(())
    }

    /// Return jobs that are ready to run: registered, not in `completed`,
    /// and all their dependencies are in `completed`.
    #[allow(dead_code)]
    #[must_use]
    pub fn ready_jobs(&self, completed: &[u64]) -> Vec<u64> {
        self.nodes
            .iter()
            .copied()
            .filter(|&job_id| {
                // Not already completed
                if completed.contains(&job_id) {
                    return false;
                }
                // All dependencies must be completed
                self.dependencies_of(job_id)
                    .iter()
                    .all(|dep| completed.contains(dep))
            })
            .collect()
    }

    /// Return a topological ordering of all jobs.
    /// Returns an error if the graph contains a cycle.
    #[allow(dead_code)]
    pub fn topological_sort(&self) -> Result<Vec<u64>, String> {
        if self.has_cycle() {
            return Err("Cycle detected in dependency graph".to_string());
        }

        let mut result: Vec<u64> = Vec::new();
        let mut visited: Vec<u64> = Vec::new();
        let mut temp: Vec<u64> = Vec::new();

        for &node in &self.nodes {
            if !visited.contains(&node) {
                self.dfs_visit(node, &mut visited, &mut temp, &mut result)?;
            }
        }

        Ok(result)
    }

    fn dfs_visit(
        &self,
        node: u64,
        visited: &mut Vec<u64>,
        temp: &mut Vec<u64>,
        result: &mut Vec<u64>,
    ) -> Result<(), String> {
        if temp.contains(&node) {
            return Err(format!("Cycle involving node {node}"));
        }
        if visited.contains(&node) {
            return Ok(());
        }
        temp.push(node);
        // Visit nodes that `node` depends on first
        for dep in self.dependencies_of(node) {
            self.dfs_visit(dep, visited, temp, result)?;
        }
        temp.retain(|&x| x != node);
        visited.push(node);
        result.push(node);
        Ok(())
    }

    /// Return `true` if the graph contains a directed cycle.
    #[allow(dead_code)]
    #[must_use]
    pub fn has_cycle(&self) -> bool {
        let mut visited: Vec<u64> = Vec::new();
        let mut in_stack: Vec<u64> = Vec::new();
        for &node in &self.nodes {
            if !visited.contains(&node) && self.dfs_cycle(node, &mut visited, &mut in_stack) {
                return true;
            }
        }
        false
    }

    fn dfs_cycle(&self, node: u64, visited: &mut Vec<u64>, in_stack: &mut Vec<u64>) -> bool {
        visited.push(node);
        in_stack.push(node);

        for dep in self.dependencies_of(node) {
            if !visited.contains(&dep) {
                if self.dfs_cycle(dep, visited, in_stack) {
                    return true;
                }
            } else if in_stack.contains(&dep) {
                return true;
            }
        }

        in_stack.retain(|&x| x != node);
        false
    }

    /// Return all jobs that `job_id` directly depends on.
    #[allow(dead_code)]
    #[must_use]
    pub fn dependencies_of(&self, job_id: u64) -> Vec<u64> {
        self.edges
            .iter()
            .filter(|(j, _)| *j == job_id)
            .map(|(_, dep)| *dep)
            .collect()
    }

    /// Return all jobs that directly depend on `job_id`.
    #[allow(dead_code)]
    #[must_use]
    pub fn dependents_of(&self, job_id: u64) -> Vec<u64> {
        self.edges
            .iter()
            .filter(|(_, dep)| *dep == job_id)
            .map(|(j, _)| *j)
            .collect()
    }

    /// Return the number of registered nodes.
    #[allow(dead_code)]
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Return the number of edges.
    #[allow(dead_code)]
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

// ===========================================================================
// Job Chaining / Pipeline Support
// ===========================================================================

/// Describes how the output of one pipeline stage feeds into the next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputMapping {
    /// The output file path of the upstream job becomes the input of the downstream job.
    FilePassthrough,
    /// A specific named output artifact is forwarded.
    NamedArtifact(String),
    /// Multiple output artifacts are merged into a single input list.
    MergedArtifacts(Vec<String>),
    /// Custom key-value mapping from upstream outputs to downstream inputs.
    Custom(Vec<(String, String)>),
}

/// The current execution state of a pipeline stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageState {
    /// Waiting for upstream dependencies to complete.
    Pending,
    /// All upstream dependencies satisfied; ready to run.
    Ready,
    /// Currently executing.
    Running,
    /// Completed successfully.
    Completed,
    /// Failed with an error.
    Failed,
    /// Skipped (e.g. because an upstream stage failed and `skip_on_upstream_failure` is set).
    Skipped,
}

impl std::fmt::Display for StageState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Ready => write!(f, "Ready"),
            Self::Running => write!(f, "Running"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::Skipped => write!(f, "Skipped"),
        }
    }
}

/// A single stage within a [`JobPipeline`].
#[derive(Debug, Clone)]
pub struct PipelineStage {
    /// Unique stage identifier (scoped to the pipeline).
    pub stage_id: u64,
    /// Human-readable name.
    pub name: String,
    /// Current execution state.
    pub state: StageState,
    /// How this stage receives output from its upstream stage(s).
    pub output_mapping: OutputMapping,
    /// If `true`, this stage is skipped instead of failing the whole pipeline
    /// when an upstream stage fails.
    pub skip_on_upstream_failure: bool,
    /// Maximum number of retry attempts for this stage.
    pub max_retries: u32,
    /// Current retry count.
    pub retry_count: u32,
    /// Output artifacts produced by this stage (populated after completion).
    pub output_artifacts: Vec<String>,
    /// Error message if the stage failed.
    pub error_message: Option<String>,
}

impl PipelineStage {
    /// Create a new stage with sensible defaults.
    #[must_use]
    pub fn new(stage_id: u64, name: impl Into<String>) -> Self {
        Self {
            stage_id,
            name: name.into(),
            state: StageState::Pending,
            output_mapping: OutputMapping::FilePassthrough,
            skip_on_upstream_failure: false,
            max_retries: 0,
            retry_count: 0,
            output_artifacts: Vec::new(),
            error_message: None,
        }
    }

    /// Set the output mapping for this stage.
    #[must_use]
    pub fn with_output_mapping(mut self, mapping: OutputMapping) -> Self {
        self.output_mapping = mapping;
        self
    }

    /// Enable skip-on-upstream-failure behavior.
    #[must_use]
    pub fn with_skip_on_failure(mut self) -> Self {
        self.skip_on_upstream_failure = true;
        self
    }

    /// Set the maximum retry count.
    #[must_use]
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Whether the stage can be retried.
    #[must_use]
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Whether the stage has reached a terminal state.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            StageState::Completed | StageState::Failed | StageState::Skipped
        )
    }
}

/// Error type for pipeline operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineError {
    /// A stage ID was not found in the pipeline.
    StageNotFound(u64),
    /// A duplicate stage ID was added.
    DuplicateStage(u64),
    /// Adding a dependency would create a cycle.
    CycleDetected { from: u64, to: u64 },
    /// The pipeline is in an invalid state for the requested operation.
    InvalidTransition {
        stage_id: u64,
        from: StageState,
        to: StageState,
    },
    /// An upstream stage has not completed.
    UpstreamNotReady(u64),
    /// Pipeline has already completed or failed.
    PipelineFinished,
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StageNotFound(id) => write!(f, "stage not found: {id}"),
            Self::DuplicateStage(id) => write!(f, "duplicate stage: {id}"),
            Self::CycleDetected { from, to } => {
                write!(f, "cycle detected: {from} -> {to}")
            }
            Self::InvalidTransition { stage_id, from, to } => {
                write!(f, "invalid transition for stage {stage_id}: {from} -> {to}")
            }
            Self::UpstreamNotReady(id) => write!(f, "upstream not ready for stage: {id}"),
            Self::PipelineFinished => write!(f, "pipeline has already finished"),
        }
    }
}

impl std::error::Error for PipelineError {}

/// A directed pipeline of stages with dependency tracking and output forwarding.
///
/// Stages are connected via a [`DependencyGraph`] and output artifacts flow
/// from completed upstream stages to their downstream consumers according to
/// each stage's [`OutputMapping`].
#[derive(Debug, Clone)]
pub struct JobPipeline {
    /// Human-readable pipeline name.
    pub name: String,
    /// The stages, keyed by stage ID.
    stages: std::collections::HashMap<u64, PipelineStage>,
    /// Underlying DAG tracking stage dependencies.
    graph: DependencyGraph,
}

impl JobPipeline {
    /// Create a new empty pipeline.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            stages: std::collections::HashMap::new(),
            graph: DependencyGraph::new(),
        }
    }

    /// Add a stage to the pipeline.
    ///
    /// # Errors
    ///
    /// Returns `PipelineError::DuplicateStage` if the stage ID already exists.
    pub fn add_stage(&mut self, stage: PipelineStage) -> std::result::Result<(), PipelineError> {
        if self.stages.contains_key(&stage.stage_id) {
            return Err(PipelineError::DuplicateStage(stage.stage_id));
        }
        self.graph.add_job(stage.stage_id);
        self.stages.insert(stage.stage_id, stage);
        Ok(())
    }

    /// Declare that `stage_id` depends on `upstream_id` (upstream must complete first).
    ///
    /// # Errors
    ///
    /// Returns `PipelineError::StageNotFound` if either stage is unknown, or
    /// `PipelineError::CycleDetected` if the dependency would create a cycle.
    pub fn add_dependency(
        &mut self,
        stage_id: u64,
        upstream_id: u64,
    ) -> std::result::Result<(), PipelineError> {
        if !self.stages.contains_key(&stage_id) {
            return Err(PipelineError::StageNotFound(stage_id));
        }
        if !self.stages.contains_key(&upstream_id) {
            return Err(PipelineError::StageNotFound(upstream_id));
        }
        self.graph
            .add_dependency(stage_id, upstream_id)
            .map_err(|_| PipelineError::CycleDetected {
                from: stage_id,
                to: upstream_id,
            })
    }

    /// Chain two stages: `downstream` depends on `upstream`, with file passthrough.
    ///
    /// This is a convenience method combining `add_dependency`.
    ///
    /// # Errors
    ///
    /// Returns errors from `add_dependency`.
    pub fn chain(
        &mut self,
        upstream_id: u64,
        downstream_id: u64,
    ) -> std::result::Result<(), PipelineError> {
        self.add_dependency(downstream_id, upstream_id)
    }

    /// Return all stages that are currently ready to execute (all upstream
    /// dependencies completed).
    #[must_use]
    pub fn ready_stages(&self) -> Vec<u64> {
        let completed: Vec<u64> = self
            .stages
            .iter()
            .filter(|(_, s)| matches!(s.state, StageState::Completed | StageState::Skipped))
            .map(|(&id, _)| id)
            .collect();
        self.graph
            .ready_jobs(&completed)
            .into_iter()
            .filter(|id| {
                self.stages
                    .get(id)
                    .map_or(false, |s| s.state == StageState::Pending)
            })
            .collect()
    }

    /// Transition a stage to `Running`.
    ///
    /// # Errors
    ///
    /// Returns `PipelineError::StageNotFound` or `PipelineError::InvalidTransition`.
    pub fn start_stage(&mut self, stage_id: u64) -> std::result::Result<(), PipelineError> {
        let stage = self
            .stages
            .get_mut(&stage_id)
            .ok_or(PipelineError::StageNotFound(stage_id))?;
        if stage.state != StageState::Pending {
            return Err(PipelineError::InvalidTransition {
                stage_id,
                from: stage.state,
                to: StageState::Running,
            });
        }
        stage.state = StageState::Running;
        Ok(())
    }

    /// Mark a stage as completed with the given output artifacts.
    ///
    /// # Errors
    ///
    /// Returns `PipelineError::StageNotFound` or `PipelineError::InvalidTransition`.
    pub fn complete_stage(
        &mut self,
        stage_id: u64,
        artifacts: Vec<String>,
    ) -> std::result::Result<(), PipelineError> {
        let stage = self
            .stages
            .get_mut(&stage_id)
            .ok_or(PipelineError::StageNotFound(stage_id))?;
        if stage.state != StageState::Running {
            return Err(PipelineError::InvalidTransition {
                stage_id,
                from: stage.state,
                to: StageState::Completed,
            });
        }
        stage.output_artifacts = artifacts;
        stage.state = StageState::Completed;
        Ok(())
    }

    /// Mark a stage as failed. Downstream stages with `skip_on_upstream_failure`
    /// will be transitioned to `Skipped`.
    ///
    /// # Errors
    ///
    /// Returns `PipelineError::StageNotFound` or `PipelineError::InvalidTransition`.
    pub fn fail_stage(
        &mut self,
        stage_id: u64,
        error: impl Into<String>,
    ) -> std::result::Result<(), PipelineError> {
        let stage = self
            .stages
            .get_mut(&stage_id)
            .ok_or(PipelineError::StageNotFound(stage_id))?;
        if stage.state != StageState::Running {
            return Err(PipelineError::InvalidTransition {
                stage_id,
                from: stage.state,
                to: StageState::Failed,
            });
        }
        stage.error_message = Some(error.into());
        stage.state = StageState::Failed;

        // Cascade: skip downstream stages that have skip_on_upstream_failure set
        let dependents = self.graph.dependents_of(stage_id);
        for dep_id in dependents {
            if let Some(dep_stage) = self.stages.get_mut(&dep_id) {
                if dep_stage.skip_on_upstream_failure && dep_stage.state == StageState::Pending {
                    dep_stage.state = StageState::Skipped;
                }
            }
        }

        Ok(())
    }

    /// Collect the output artifacts from upstream stages that feed into `stage_id`.
    #[must_use]
    pub fn upstream_artifacts(&self, stage_id: u64) -> Vec<String> {
        let upstream_ids = self.graph.dependencies_of(stage_id);
        let mut artifacts = Vec::new();
        for uid in upstream_ids {
            if let Some(stage) = self.stages.get(&uid) {
                artifacts.extend(stage.output_artifacts.iter().cloned());
            }
        }
        artifacts
    }

    /// Get a reference to a stage by ID.
    #[must_use]
    pub fn get_stage(&self, stage_id: u64) -> Option<&PipelineStage> {
        self.stages.get(&stage_id)
    }

    /// Get the total number of stages.
    #[must_use]
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Whether all stages have reached a terminal state.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.stages.values().all(|s| s.is_terminal())
    }

    /// Whether all stages completed successfully (none failed, none pending).
    #[must_use]
    pub fn is_successful(&self) -> bool {
        self.stages
            .values()
            .all(|s| matches!(s.state, StageState::Completed | StageState::Skipped))
    }

    /// Whether any stage has failed.
    #[must_use]
    pub fn has_failures(&self) -> bool {
        self.stages.values().any(|s| s.state == StageState::Failed)
    }

    /// Return a topological ordering of stage IDs.
    ///
    /// # Errors
    ///
    /// Returns an error string if the graph has a cycle (should not happen if
    /// `add_dependency` is the only way to add edges).
    pub fn execution_order(&self) -> std::result::Result<Vec<u64>, String> {
        self.graph.topological_sort()
    }

    /// Summary of stage states: (pending, running, completed, failed, skipped).
    #[must_use]
    pub fn state_summary(&self) -> (usize, usize, usize, usize, usize) {
        let mut pending = 0;
        let mut running = 0;
        let mut completed = 0;
        let mut failed = 0;
        let mut skipped = 0;
        for s in self.stages.values() {
            match s.state {
                StageState::Pending | StageState::Ready => pending += 1,
                StageState::Running => running += 1,
                StageState::Completed => completed += 1,
                StageState::Failed => failed += 1,
                StageState::Skipped => skipped += 1,
            }
        }
        (pending, running, completed, failed, skipped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a simple linear chain: 1 -> 2 -> 3 (3 must run first).
    fn linear_graph() -> DependencyGraph {
        let mut g = DependencyGraph::new();
        g.add_job(1);
        g.add_job(2);
        g.add_job(3);
        g.add_dependency(1, 2).expect("failed to add dependency"); // 1 depends on 2
        g.add_dependency(2, 3).expect("failed to add dependency"); // 2 depends on 3
        g
    }

    #[test]
    fn test_add_job() {
        let mut g = DependencyGraph::new();
        g.add_job(10);
        g.add_job(20);
        assert_eq!(g.nodes.len(), 2);
        // Adding same ID again is idempotent
        g.add_job(10);
        assert_eq!(g.nodes.len(), 2);
    }

    #[test]
    fn test_add_dependency_unknown_job() {
        let mut g = DependencyGraph::new();
        g.add_job(1);
        let err = g.add_dependency(1, 99);
        assert!(err.is_err());
    }

    #[test]
    fn test_dependencies_of() {
        let g = linear_graph();
        let deps = g.dependencies_of(1);
        assert_eq!(deps, vec![2]);
        let deps3 = g.dependencies_of(3);
        assert!(deps3.is_empty());
    }

    #[test]
    fn test_dependents_of() {
        let g = linear_graph();
        let dependents = g.dependents_of(2);
        assert_eq!(dependents, vec![1]);
    }

    #[test]
    fn test_no_cycle() {
        let g = linear_graph();
        assert!(!g.has_cycle());
    }

    #[test]
    fn test_cycle_detection() {
        let mut g = DependencyGraph::new();
        g.add_job(1);
        g.add_job(2);
        g.add_dependency(1, 2).expect("failed to add dependency");
        // Attempt to create a cycle: 2 depends on 1
        let result = g.add_dependency(2, 1);
        assert!(result.is_err());
        // The graph must not have a cycle (the edge was rolled back)
        assert!(!g.has_cycle());
    }

    #[test]
    fn test_self_loop_detection() {
        let mut g = DependencyGraph::new();
        g.add_job(1);
        let result = g.add_dependency(1, 1);
        assert!(result.is_err());
        assert!(!g.has_cycle());
    }

    #[test]
    fn test_topological_sort_linear() {
        let g = linear_graph();
        let order = g
            .topological_sort()
            .expect("topological_sort should succeed");
        // 3 must come before 2, and 2 before 1
        let pos: std::collections::HashMap<u64, usize> =
            order.iter().enumerate().map(|(i, &j)| (j, i)).collect();
        assert!(pos[&3] < pos[&2]);
        assert!(pos[&2] < pos[&1]);
    }

    #[test]
    fn test_topological_sort_empty() {
        let g = DependencyGraph::new();
        let order = g
            .topological_sort()
            .expect("topological_sort should succeed");
        assert!(order.is_empty());
    }

    #[test]
    fn test_ready_jobs_none_completed() {
        let g = linear_graph();
        // Only job 3 (no deps) should be ready
        let ready = g.ready_jobs(&[]);
        assert_eq!(ready, vec![3]);
    }

    #[test]
    fn test_ready_jobs_after_completion() {
        let g = linear_graph();
        // After 3 completes, job 2 becomes ready
        let ready = g.ready_jobs(&[3]);
        assert!(ready.contains(&2));
        assert!(!ready.contains(&3));
    }

    #[test]
    fn test_ready_jobs_all_completed() {
        let g = linear_graph();
        let ready = g.ready_jobs(&[1, 2, 3]);
        assert!(ready.is_empty());
    }

    #[test]
    fn test_diamond_dependency() {
        // Jobs: 1 depends on 2 and 3; both 2 and 3 depend on 4
        let mut g = DependencyGraph::new();
        for id in [1, 2, 3, 4] {
            g.add_job(id);
        }
        g.add_dependency(1, 2).expect("failed to add dependency");
        g.add_dependency(1, 3).expect("failed to add dependency");
        g.add_dependency(2, 4).expect("failed to add dependency");
        g.add_dependency(3, 4).expect("failed to add dependency");
        assert!(!g.has_cycle());
        let order = g
            .topological_sort()
            .expect("topological_sort should succeed");
        let pos: std::collections::HashMap<u64, usize> =
            order.iter().enumerate().map(|(i, &j)| (j, i)).collect();
        assert!(pos[&4] < pos[&2]);
        assert!(pos[&4] < pos[&3]);
        assert!(pos[&2] < pos[&1]);
        assert!(pos[&3] < pos[&1]);
    }

    // ========================================================================
    // Pipeline tests
    // ========================================================================

    #[test]
    fn test_pipeline_create_and_add_stages() {
        let mut pipeline = JobPipeline::new("transcode-pipeline");
        pipeline
            .add_stage(PipelineStage::new(1, "decode"))
            .expect("add_stage should succeed");
        pipeline
            .add_stage(PipelineStage::new(2, "filter"))
            .expect("add_stage should succeed");
        pipeline
            .add_stage(PipelineStage::new(3, "encode"))
            .expect("add_stage should succeed");
        assert_eq!(pipeline.stage_count(), 3);
    }

    #[test]
    fn test_pipeline_duplicate_stage() {
        let mut pipeline = JobPipeline::new("p");
        pipeline
            .add_stage(PipelineStage::new(1, "a"))
            .expect("add_stage should succeed");
        let err = pipeline.add_stage(PipelineStage::new(1, "b"));
        assert!(err.is_err());
        assert_eq!(err.unwrap_err(), PipelineError::DuplicateStage(1));
    }

    #[test]
    fn test_pipeline_chain_and_ready_stages() {
        let mut pipeline = JobPipeline::new("p");
        pipeline
            .add_stage(PipelineStage::new(1, "decode"))
            .expect("add_stage should succeed");
        pipeline
            .add_stage(PipelineStage::new(2, "encode"))
            .expect("add_stage should succeed");
        pipeline.chain(1, 2).expect("chain should succeed");

        // Only stage 1 (no deps) should be ready
        let ready = pipeline.ready_stages();
        assert_eq!(ready, vec![1]);
    }

    #[test]
    fn test_pipeline_stage_lifecycle() {
        let mut pipeline = JobPipeline::new("p");
        pipeline
            .add_stage(PipelineStage::new(1, "decode"))
            .expect("add_stage should succeed");
        pipeline
            .add_stage(PipelineStage::new(2, "encode"))
            .expect("add_stage should succeed");
        pipeline.chain(1, 2).expect("chain should succeed");

        // Start and complete stage 1
        pipeline.start_stage(1).expect("start_stage should succeed");
        assert_eq!(
            pipeline.get_stage(1).expect("stage should exist").state,
            StageState::Running
        );
        pipeline
            .complete_stage(1, vec!["/tmp/decoded.raw".to_string()])
            .expect("complete_stage should succeed");
        assert_eq!(
            pipeline.get_stage(1).expect("stage should exist").state,
            StageState::Completed
        );

        // Now stage 2 should be ready
        let ready = pipeline.ready_stages();
        assert_eq!(ready, vec![2]);

        // Upstream artifacts should include stage 1's output
        let artifacts = pipeline.upstream_artifacts(2);
        assert_eq!(artifacts, vec!["/tmp/decoded.raw".to_string()]);

        // Complete stage 2
        pipeline.start_stage(2).expect("start_stage should succeed");
        pipeline
            .complete_stage(2, vec!["/tmp/output.mp4".to_string()])
            .expect("complete_stage should succeed");
        assert!(pipeline.is_finished());
        assert!(pipeline.is_successful());
    }

    #[test]
    fn test_pipeline_fail_stage_cascades_skip() {
        let mut pipeline = JobPipeline::new("p");
        pipeline
            .add_stage(PipelineStage::new(1, "decode"))
            .expect("add_stage should succeed");
        pipeline
            .add_stage(PipelineStage::new(2, "encode").with_skip_on_failure())
            .expect("add_stage should succeed");
        pipeline.chain(1, 2).expect("chain should succeed");

        pipeline.start_stage(1).expect("start_stage should succeed");
        pipeline
            .fail_stage(1, "decoder crash")
            .expect("fail_stage should succeed");

        assert_eq!(
            pipeline.get_stage(1).expect("stage should exist").state,
            StageState::Failed
        );
        // Stage 2 should be skipped
        assert_eq!(
            pipeline.get_stage(2).expect("stage should exist").state,
            StageState::Skipped
        );
        assert!(pipeline.is_finished());
        assert!(pipeline.has_failures());
        assert!(!pipeline.is_successful());
    }

    #[test]
    fn test_pipeline_invalid_transition() {
        let mut pipeline = JobPipeline::new("p");
        pipeline
            .add_stage(PipelineStage::new(1, "s1"))
            .expect("add_stage should succeed");
        // Cannot complete a pending stage
        let err = pipeline.complete_stage(1, vec![]);
        assert!(err.is_err());
    }

    #[test]
    fn test_pipeline_execution_order() {
        let mut pipeline = JobPipeline::new("p");
        pipeline
            .add_stage(PipelineStage::new(1, "decode"))
            .expect("add_stage should succeed");
        pipeline
            .add_stage(PipelineStage::new(2, "filter"))
            .expect("add_stage should succeed");
        pipeline
            .add_stage(PipelineStage::new(3, "encode"))
            .expect("add_stage should succeed");
        pipeline.chain(1, 2).expect("chain should succeed");
        pipeline.chain(2, 3).expect("chain should succeed");

        let order = pipeline.execution_order().expect("should produce order");
        let pos: std::collections::HashMap<u64, usize> =
            order.iter().enumerate().map(|(i, &j)| (j, i)).collect();
        assert!(pos[&1] < pos[&2]);
        assert!(pos[&2] < pos[&3]);
    }

    #[test]
    fn test_pipeline_state_summary() {
        let mut pipeline = JobPipeline::new("p");
        pipeline
            .add_stage(PipelineStage::new(1, "a"))
            .expect("add_stage should succeed");
        pipeline
            .add_stage(PipelineStage::new(2, "b"))
            .expect("add_stage should succeed");
        pipeline
            .add_stage(PipelineStage::new(3, "c"))
            .expect("add_stage should succeed");
        pipeline.chain(1, 2).expect("chain should succeed");
        pipeline.chain(2, 3).expect("chain should succeed");

        pipeline.start_stage(1).expect("start_stage should succeed");
        pipeline
            .complete_stage(1, vec![])
            .expect("complete_stage should succeed");
        pipeline.start_stage(2).expect("start_stage should succeed");

        let (pending, running, completed, failed, skipped) = pipeline.state_summary();
        assert_eq!(pending, 1);
        assert_eq!(running, 1);
        assert_eq!(completed, 1);
        assert_eq!(failed, 0);
        assert_eq!(skipped, 0);
    }

    #[test]
    fn test_pipeline_output_mapping_named_artifact() {
        let mut pipeline = JobPipeline::new("p");
        pipeline
            .add_stage(
                PipelineStage::new(1, "analyze")
                    .with_output_mapping(OutputMapping::NamedArtifact("report.json".to_string())),
            )
            .expect("add_stage should succeed");
        let stage = pipeline.get_stage(1).expect("stage should exist");
        assert_eq!(
            stage.output_mapping,
            OutputMapping::NamedArtifact("report.json".to_string())
        );
    }

    #[test]
    fn test_pipeline_retry_support() {
        let stage = PipelineStage::new(1, "encode").with_max_retries(3);
        assert!(stage.can_retry());
        assert_eq!(stage.max_retries, 3);
        assert_eq!(stage.retry_count, 0);
    }

    #[test]
    fn test_pipeline_cycle_detection() {
        let mut pipeline = JobPipeline::new("p");
        pipeline
            .add_stage(PipelineStage::new(1, "a"))
            .expect("add_stage should succeed");
        pipeline
            .add_stage(PipelineStage::new(2, "b"))
            .expect("add_stage should succeed");
        pipeline.chain(1, 2).expect("chain should succeed");
        let err = pipeline.chain(2, 1);
        assert!(err.is_err());
        assert_eq!(
            err.unwrap_err(),
            PipelineError::CycleDetected { from: 1, to: 2 }
        );
    }

    #[test]
    fn test_pipeline_diamond_topology() {
        // decode -> [filter_a, filter_b] -> mux
        let mut pipeline = JobPipeline::new("diamond");
        pipeline
            .add_stage(PipelineStage::new(1, "decode"))
            .expect("add_stage should succeed");
        pipeline
            .add_stage(PipelineStage::new(2, "filter_a"))
            .expect("add_stage should succeed");
        pipeline
            .add_stage(PipelineStage::new(3, "filter_b"))
            .expect("add_stage should succeed");
        pipeline
            .add_stage(PipelineStage::new(4, "mux").with_output_mapping(
                OutputMapping::MergedArtifacts(vec![
                    "video.raw".to_string(),
                    "audio.raw".to_string(),
                ]),
            ))
            .expect("add_stage should succeed");
        pipeline.chain(1, 2).expect("chain should succeed");
        pipeline.chain(1, 3).expect("chain should succeed");
        pipeline.chain(2, 4).expect("chain should succeed");
        pipeline.chain(3, 4).expect("chain should succeed");

        // Only decode should be ready initially
        let ready = pipeline.ready_stages();
        assert_eq!(ready, vec![1]);

        // Complete decode
        pipeline.start_stage(1).expect("start_stage should succeed");
        pipeline
            .complete_stage(1, vec!["raw_input".to_string()])
            .expect("complete_stage should succeed");

        // Now filter_a and filter_b should be ready
        let mut ready = pipeline.ready_stages();
        ready.sort();
        assert_eq!(ready, vec![2, 3]);

        // Complete both filters
        pipeline.start_stage(2).expect("start_stage should succeed");
        pipeline
            .complete_stage(2, vec!["video.raw".to_string()])
            .expect("complete_stage should succeed");
        pipeline.start_stage(3).expect("start_stage should succeed");
        pipeline
            .complete_stage(3, vec!["audio.raw".to_string()])
            .expect("complete_stage should succeed");

        // Now mux should be ready
        let ready = pipeline.ready_stages();
        assert_eq!(ready, vec![4]);

        // Upstream artifacts for mux should include both filter outputs
        let mut artifacts = pipeline.upstream_artifacts(4);
        artifacts.sort();
        assert_eq!(
            artifacts,
            vec!["audio.raw".to_string(), "video.raw".to_string()]
        );
    }

    #[test]
    fn test_stage_state_display() {
        assert_eq!(StageState::Pending.to_string(), "Pending");
        assert_eq!(StageState::Running.to_string(), "Running");
        assert_eq!(StageState::Completed.to_string(), "Completed");
        assert_eq!(StageState::Failed.to_string(), "Failed");
        assert_eq!(StageState::Skipped.to_string(), "Skipped");
    }

    #[test]
    fn test_pipeline_error_display() {
        let err = PipelineError::StageNotFound(42);
        assert_eq!(err.to_string(), "stage not found: 42");
        let err = PipelineError::CycleDetected { from: 1, to: 2 };
        assert_eq!(err.to_string(), "cycle detected: 1 -> 2");
    }
}
