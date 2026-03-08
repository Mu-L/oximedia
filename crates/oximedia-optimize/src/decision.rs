//! Mode decision strategies and helpers.

/// Mode decision strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionStrategy {
    /// Use distortion only (fastest).
    DistortionOnly,
    /// Use rate-distortion optimization.
    RateDistortion,
    /// Use full trellis quantization.
    FullTrellis,
}

/// Mode decision context.
#[derive(Debug, Clone)]
pub struct DecisionContext {
    /// Current QP.
    pub qp: u8,
    /// Lambda for RD cost.
    pub lambda: f64,
    /// Available reference frames.
    pub num_references: usize,
    /// Block size.
    pub block_size: usize,
    /// Whether this is an intra block.
    pub is_intra: bool,
}

impl DecisionContext {
    /// Creates a new decision context.
    #[must_use]
    pub fn new(qp: u8, lambda: f64, block_size: usize, is_intra: bool) -> Self {
        Self {
            qp,
            lambda,
            num_references: 0,
            block_size,
            is_intra,
        }
    }
}

/// Mode decision helper.
pub struct ModeDecision {
    strategy: DecisionStrategy,
    early_termination: bool,
    max_candidates: usize,
}

impl Default for ModeDecision {
    fn default() -> Self {
        Self::new(DecisionStrategy::RateDistortion)
    }
}

impl ModeDecision {
    /// Creates a new mode decision helper.
    #[must_use]
    pub fn new(strategy: DecisionStrategy) -> Self {
        Self {
            strategy,
            early_termination: true,
            max_candidates: 8,
        }
    }

    /// Sets whether to use early termination.
    pub fn set_early_termination(&mut self, enable: bool) {
        self.early_termination = enable;
    }

    /// Sets maximum number of candidates to evaluate.
    pub fn set_max_candidates(&mut self, max: usize) {
        self.max_candidates = max;
    }

    /// Evaluates and selects the best mode.
    #[allow(dead_code)]
    pub fn select_best_mode<T>(
        &self,
        candidates: &[T],
        context: &DecisionContext,
        eval_fn: impl Fn(&T) -> (f64, f64),
    ) -> (usize, f64) {
        if candidates.is_empty() {
            return (0, f64::MAX);
        }

        let mut best_idx = 0;
        let mut best_cost = f64::MAX;

        let num_candidates = if self.early_termination {
            candidates.len().min(self.max_candidates)
        } else {
            candidates.len()
        };

        for (idx, candidate) in candidates.iter().enumerate().take(num_candidates) {
            let (distortion, rate) = eval_fn(candidate);
            let cost = self.calculate_cost(distortion, rate, context);

            if cost < best_cost {
                best_cost = cost;
                best_idx = idx;

                // Early termination if cost is very good
                if self.early_termination && cost < 10.0 {
                    break;
                }
            }
        }

        (best_idx, best_cost)
    }

    fn calculate_cost(&self, distortion: f64, rate: f64, context: &DecisionContext) -> f64 {
        match self.strategy {
            DecisionStrategy::DistortionOnly => distortion,
            DecisionStrategy::RateDistortion | DecisionStrategy::FullTrellis => {
                distortion + context.lambda * rate
            }
        }
    }

    /// Checks if a mode should be skipped based on early termination.
    #[must_use]
    pub fn should_skip_mode(&self, current_cost: f64, best_cost: f64) -> bool {
        if !self.early_termination {
            return false;
        }

        // Skip if current cost is much worse than best
        current_cost > best_cost * 1.5
    }
}

/// Split decision helper for partition decisions.
pub struct SplitDecision {
    min_size: usize,
    max_size: usize,
    complexity_threshold: f64,
}

impl Default for SplitDecision {
    fn default() -> Self {
        Self::new(4, 128, 100.0)
    }
}

impl SplitDecision {
    /// Creates a new split decision helper.
    #[must_use]
    pub const fn new(min_size: usize, max_size: usize, complexity_threshold: f64) -> Self {
        Self {
            min_size,
            max_size,
            complexity_threshold,
        }
    }

    /// Determines if a block should be split.
    #[must_use]
    pub fn should_split(&self, block_size: usize, complexity: f64, depth: usize) -> bool {
        // Don't split below minimum size
        if block_size <= self.min_size {
            return false;
        }

        // Don't split above maximum size
        if block_size > self.max_size {
            return true;
        }

        // Split based on complexity
        let adjusted_threshold = self.complexity_threshold * (1.0 + depth as f64 * 0.1);
        complexity > adjusted_threshold
    }

    /// Evaluates split decision with RD cost.
    #[allow(dead_code)]
    #[must_use]
    pub fn evaluate_split(
        &self,
        no_split_cost: f64,
        split_cost: f64,
        split_overhead: f64,
        lambda: f64,
    ) -> bool {
        let total_split_cost = split_cost + lambda * split_overhead;
        total_split_cost < no_split_cost
    }
}

/// Reference selection helper.
pub struct ReferenceDecision {
    max_references: usize,
    enable_multi_ref: bool,
}

impl Default for ReferenceDecision {
    fn default() -> Self {
        Self::new(3)
    }
}

impl ReferenceDecision {
    /// Creates a new reference decision helper.
    #[must_use]
    pub fn new(max_references: usize) -> Self {
        Self {
            max_references,
            enable_multi_ref: true,
        }
    }

    /// Selects which references to search.
    #[must_use]
    pub fn select_references(&self, available_refs: usize, complexity: f64) -> Vec<usize> {
        if !self.enable_multi_ref || available_refs == 0 {
            return vec![0];
        }

        let num_refs = if complexity > 200.0 {
            // High complexity: search more references
            self.max_references.min(available_refs)
        } else if complexity > 100.0 {
            // Medium complexity: search some references
            (self.max_references / 2).max(1).min(available_refs)
        } else {
            // Low complexity: search only primary reference
            1
        };

        (0..num_refs).collect()
    }

    /// Determines if bidirectional prediction should be tried.
    #[must_use]
    pub fn should_try_bipred(&self, forward_cost: f64, backward_cost: f64) -> bool {
        // Try bipred if both forward and backward are reasonably good
        let avg_cost = (forward_cost + backward_cost) / 2.0;
        forward_cost < 1000.0 && backward_cost < 1000.0 && avg_cost < 500.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_context_creation() {
        let ctx = DecisionContext::new(26, 1.0, 16, true);
        assert_eq!(ctx.qp, 26);
        assert_eq!(ctx.lambda, 1.0);
        assert_eq!(ctx.block_size, 16);
        assert!(ctx.is_intra);
    }

    #[test]
    fn test_mode_decision_creation() {
        let decision = ModeDecision::default();
        assert_eq!(decision.strategy, DecisionStrategy::RateDistortion);
        assert!(decision.early_termination);
    }

    #[test]
    fn test_distortion_only_strategy() {
        let decision = ModeDecision::new(DecisionStrategy::DistortionOnly);
        let context = DecisionContext::new(26, 1.0, 16, true);
        let cost = decision.calculate_cost(100.0, 50.0, &context);
        assert_eq!(cost, 100.0); // Only distortion
    }

    #[test]
    fn test_rd_strategy() {
        let decision = ModeDecision::new(DecisionStrategy::RateDistortion);
        let context = DecisionContext::new(26, 2.0, 16, true);
        let cost = decision.calculate_cost(100.0, 50.0, &context);
        assert_eq!(cost, 200.0); // 100 + 2*50
    }

    #[test]
    fn test_should_skip_mode() {
        let decision = ModeDecision::default();
        assert!(!decision.should_skip_mode(100.0, 100.0));
        assert!(decision.should_skip_mode(200.0, 100.0));
    }

    #[test]
    fn test_split_decision_min_size() {
        let decision = SplitDecision::default();
        assert!(!decision.should_split(4, 1000.0, 0)); // At min size
        assert!(decision.should_split(8, 1000.0, 0)); // Above min, high complexity
    }

    #[test]
    fn test_split_decision_complexity() {
        let decision = SplitDecision::default();
        assert!(!decision.should_split(16, 50.0, 0)); // Low complexity
        assert!(decision.should_split(16, 200.0, 0)); // High complexity
    }

    #[test]
    fn test_reference_decision_selection() {
        let decision = ReferenceDecision::default();

        // Low complexity: only 1 reference
        let refs = decision.select_references(5, 50.0);
        assert_eq!(refs.len(), 1);

        // High complexity: multiple references
        let refs = decision.select_references(5, 300.0);
        assert!(refs.len() > 1);
        assert!(refs.len() <= 3); // Max references
    }

    #[test]
    fn test_should_try_bipred() {
        let decision = ReferenceDecision::default();

        // Good costs: try bipred
        assert!(decision.should_try_bipred(400.0, 400.0));

        // Bad costs: don't try bipred
        assert!(!decision.should_try_bipred(2000.0, 2000.0));
    }

    #[test]
    fn test_select_best_mode() {
        let decision = ModeDecision::default();
        let context = DecisionContext::new(26, 1.0, 16, true);

        let candidates = vec![0, 1, 2, 3];
        let (best_idx, cost) = decision.select_best_mode(&candidates, &context, |&c| {
            // Mode 2 has best cost
            match c {
                0 => (200.0, 50.0),
                1 => (150.0, 40.0),
                2 => (100.0, 30.0), // Best
                _ => (180.0, 45.0),
            }
        });

        assert_eq!(best_idx, 2);
        assert!(cost < 150.0);
    }
}
