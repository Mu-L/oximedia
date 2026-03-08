#![allow(dead_code)]

//! Route optimization for media signal paths.
//!
//! Evaluates candidate routes and selects the best one based on
//! configurable optimization goals such as minimal latency,
//! maximum reliability, or lowest cost.

/// Optimization goal for route selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationGoal {
    /// Minimize end-to-end latency.
    MinLatency,
    /// Maximize path reliability (fewest hops / highest uptime).
    MaxReliability,
    /// Minimize monetary cost.
    MinCost,
    /// Balance latency and reliability equally.
    Balanced,
}

/// A candidate route to evaluate.
#[derive(Debug, Clone)]
pub struct CandidateRoute {
    /// Route identifier.
    pub id: String,
    /// Estimated latency in microseconds.
    pub latency_us: f64,
    /// Reliability score 0.0..=1.0.
    pub reliability: f64,
    /// Cost per hour of use.
    pub cost_per_hour: f64,
    /// Number of hops.
    pub hop_count: u32,
}

impl CandidateRoute {
    /// Create a new candidate route.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            latency_us: 0.0,
            reliability: 1.0,
            cost_per_hour: 0.0,
            hop_count: 1,
        }
    }

    /// Set latency.
    pub fn with_latency_us(mut self, us: f64) -> Self {
        self.latency_us = us;
        self
    }

    /// Set reliability.
    pub fn with_reliability(mut self, r: f64) -> Self {
        self.reliability = r.clamp(0.0, 1.0);
        self
    }

    /// Set cost.
    pub fn with_cost(mut self, c: f64) -> Self {
        self.cost_per_hour = c;
        self
    }

    /// Set hop count.
    pub fn with_hops(mut self, h: u32) -> Self {
        self.hop_count = h;
        self
    }
}

/// Result of an optimization run.
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    /// The chosen route id.
    pub chosen_id: String,
    /// Composite score (higher is better).
    pub score: f64,
    /// All scored candidates sorted best-first.
    pub ranked: Vec<(String, f64)>,
}

/// Route optimizer engine.
#[derive(Debug, Clone)]
pub struct RouteOptimizer {
    goal: OptimizationGoal,
    /// Weight for latency component in balanced mode (0..=1).
    latency_weight: f64,
    /// Weight for reliability component in balanced mode (0..=1).
    reliability_weight: f64,
    /// Weight for cost component in balanced mode (0..=1).
    cost_weight: f64,
    /// Maximum acceptable latency (0 = no limit).
    max_latency_us: f64,
    /// Minimum acceptable reliability (0 = no limit).
    min_reliability: f64,
}

impl RouteOptimizer {
    /// Create a new optimizer with the given goal.
    pub fn new(goal: OptimizationGoal) -> Self {
        Self {
            goal,
            latency_weight: 0.4,
            reliability_weight: 0.4,
            cost_weight: 0.2,
            max_latency_us: 0.0,
            min_reliability: 0.0,
        }
    }

    /// Set balanced-mode weights. They need not sum to 1; they are normalized.
    #[allow(clippy::cast_precision_loss)]
    pub fn with_weights(mut self, latency: f64, reliability: f64, cost: f64) -> Self {
        let total = latency + reliability + cost;
        if total > 0.0 {
            self.latency_weight = latency / total;
            self.reliability_weight = reliability / total;
            self.cost_weight = cost / total;
        }
        self
    }

    /// Set a hard latency constraint.
    pub fn with_max_latency(mut self, us: f64) -> Self {
        self.max_latency_us = us;
        self
    }

    /// Set a hard reliability constraint.
    pub fn with_min_reliability(mut self, r: f64) -> Self {
        self.min_reliability = r;
        self
    }

    /// Score a single candidate. Higher is better.
    #[allow(clippy::cast_precision_loss)]
    fn score(&self, route: &CandidateRoute) -> f64 {
        match self.goal {
            OptimizationGoal::MinLatency => {
                if route.latency_us <= 0.0 {
                    return f64::MAX;
                }
                1_000_000.0 / route.latency_us
            }
            OptimizationGoal::MaxReliability => route.reliability * 1000.0,
            OptimizationGoal::MinCost => {
                if route.cost_per_hour <= 0.0 {
                    return f64::MAX;
                }
                1000.0 / route.cost_per_hour
            }
            OptimizationGoal::Balanced => {
                let lat_score = if route.latency_us > 0.0 {
                    1_000_000.0 / route.latency_us
                } else {
                    1_000_000.0
                };
                let rel_score = route.reliability * 1000.0;
                let cost_score = if route.cost_per_hour > 0.0 {
                    1000.0 / route.cost_per_hour
                } else {
                    1000.0
                };
                self.latency_weight * lat_score
                    + self.reliability_weight * rel_score
                    + self.cost_weight * cost_score
            }
        }
    }

    /// Filter candidates that violate hard constraints.
    fn filter<'a>(&self, routes: &'a [CandidateRoute]) -> Vec<&'a CandidateRoute> {
        routes
            .iter()
            .filter(|r| {
                if self.max_latency_us > 0.0 && r.latency_us > self.max_latency_us {
                    return false;
                }
                if self.min_reliability > 0.0 && r.reliability < self.min_reliability {
                    return false;
                }
                true
            })
            .collect()
    }

    /// Optimize: pick the best route from candidates.
    /// Returns `None` if no candidates survive filtering.
    pub fn optimize(&self, candidates: &[CandidateRoute]) -> Option<OptimizationResult> {
        let filtered = self.filter(candidates);
        if filtered.is_empty() {
            return None;
        }

        let mut scored: Vec<(String, f64)> = filtered
            .iter()
            .map(|r| (r.id.clone(), self.score(r)))
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let best = scored[0].clone();
        Some(OptimizationResult {
            chosen_id: best.0,
            score: best.1,
            ranked: scored,
        })
    }

    /// Current optimization goal.
    pub fn goal(&self) -> OptimizationGoal {
        self.goal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn routes() -> Vec<CandidateRoute> {
        vec![
            CandidateRoute::new("fast")
                .with_latency_us(100.0)
                .with_reliability(0.95)
                .with_cost(10.0)
                .with_hops(2),
            CandidateRoute::new("reliable")
                .with_latency_us(500.0)
                .with_reliability(0.999)
                .with_cost(20.0)
                .with_hops(3),
            CandidateRoute::new("cheap")
                .with_latency_us(1000.0)
                .with_reliability(0.90)
                .with_cost(2.0)
                .with_hops(5),
        ]
    }

    #[test]
    fn test_min_latency_picks_fastest() {
        let opt = RouteOptimizer::new(OptimizationGoal::MinLatency);
        let result = opt.optimize(&routes()).expect("should succeed in test");
        assert_eq!(result.chosen_id, "fast");
    }

    #[test]
    fn test_max_reliability_picks_most_reliable() {
        let opt = RouteOptimizer::new(OptimizationGoal::MaxReliability);
        let result = opt.optimize(&routes()).expect("should succeed in test");
        assert_eq!(result.chosen_id, "reliable");
    }

    #[test]
    fn test_min_cost_picks_cheapest() {
        let opt = RouteOptimizer::new(OptimizationGoal::MinCost);
        let result = opt.optimize(&routes()).expect("should succeed in test");
        assert_eq!(result.chosen_id, "cheap");
    }

    #[test]
    fn test_balanced_returns_result() {
        let opt = RouteOptimizer::new(OptimizationGoal::Balanced);
        let result = opt.optimize(&routes());
        assert!(result.is_some());
    }

    #[test]
    fn test_empty_candidates() {
        let opt = RouteOptimizer::new(OptimizationGoal::MinLatency);
        assert!(opt.optimize(&[]).is_none());
    }

    #[test]
    fn test_latency_constraint_filters() {
        let opt = RouteOptimizer::new(OptimizationGoal::MinCost).with_max_latency(200.0);
        let result = opt.optimize(&routes()).expect("should succeed in test");
        // Only "fast" has latency <= 200
        assert_eq!(result.chosen_id, "fast");
    }

    #[test]
    fn test_reliability_constraint_filters() {
        let opt = RouteOptimizer::new(OptimizationGoal::MinCost).with_min_reliability(0.99);
        let result = opt.optimize(&routes()).expect("should succeed in test");
        // Only "reliable" has reliability >= 0.99
        assert_eq!(result.chosen_id, "reliable");
    }

    #[test]
    fn test_all_filtered_returns_none() {
        let opt = RouteOptimizer::new(OptimizationGoal::MinLatency).with_max_latency(1.0); // nothing is under 1us
        assert!(opt.optimize(&routes()).is_none());
    }

    #[test]
    fn test_ranked_ordering() {
        let opt = RouteOptimizer::new(OptimizationGoal::MinLatency);
        let result = opt.optimize(&routes()).expect("should succeed in test");
        // Scores should be descending
        for w in result.ranked.windows(2) {
            assert!(w[0].1 >= w[1].1);
        }
    }

    #[test]
    fn test_custom_weights() {
        let opt = RouteOptimizer::new(OptimizationGoal::Balanced).with_weights(0.0, 0.0, 1.0); // only cost matters
        let result = opt.optimize(&routes()).expect("should succeed in test");
        assert_eq!(result.chosen_id, "cheap");
    }

    #[test]
    fn test_goal_accessor() {
        let opt = RouteOptimizer::new(OptimizationGoal::MaxReliability);
        assert_eq!(opt.goal(), OptimizationGoal::MaxReliability);
    }

    #[test]
    fn test_candidate_builder() {
        let r = CandidateRoute::new("test")
            .with_latency_us(42.0)
            .with_reliability(0.99)
            .with_cost(5.0)
            .with_hops(3);
        assert_eq!(r.id, "test");
        assert!((r.latency_us - 42.0).abs() < f64::EPSILON);
        assert_eq!(r.hop_count, 3);
    }

    #[test]
    fn test_reliability_clamped() {
        let r = CandidateRoute::new("over").with_reliability(1.5);
        assert!((r.reliability - 1.0).abs() < f64::EPSILON);
        let r2 = CandidateRoute::new("under").with_reliability(-0.5);
        assert!(r2.reliability.abs() < f64::EPSILON);
    }

    #[test]
    fn test_single_candidate_always_chosen() {
        let opt = RouteOptimizer::new(OptimizationGoal::Balanced);
        let cands = vec![CandidateRoute::new("only").with_latency_us(500.0)];
        let result = opt.optimize(&cands).expect("should succeed in test");
        assert_eq!(result.chosen_id, "only");
    }

    #[test]
    fn test_zero_latency_route_scores_high() {
        let opt = RouteOptimizer::new(OptimizationGoal::MinLatency);
        let cands = vec![
            CandidateRoute::new("zero_lat").with_latency_us(0.0),
            CandidateRoute::new("some_lat").with_latency_us(100.0),
        ];
        let result = opt.optimize(&cands).expect("should succeed in test");
        assert_eq!(result.chosen_id, "zero_lat");
    }

    #[test]
    fn test_optimization_result_score_positive() {
        let opt = RouteOptimizer::new(OptimizationGoal::MaxReliability);
        let result = opt.optimize(&routes()).expect("should succeed in test");
        assert!(result.score > 0.0);
    }
}
