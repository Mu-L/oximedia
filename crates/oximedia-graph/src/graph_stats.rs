//! Graph statistics and complexity analysis for the filter graph pipeline.
//!
//! Provides structural metrics for a processing graph: node count, edge count,
//! average degree, maximum depth, and complexity classification.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet, VecDeque};

/// A qualitative classification of graph complexity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphComplexity {
    /// Very few nodes and edges; trivial to schedule.
    Trivial,
    /// Moderate number of nodes; standard linear pipeline.
    Simple,
    /// Many nodes with branching; requires careful scheduling.
    Moderate,
    /// Dense graph with many cross-edges; potentially expensive.
    Complex,
}

impl GraphComplexity {
    /// Returns a human-readable description of the complexity level.
    pub fn description(&self) -> &'static str {
        match self {
            GraphComplexity::Trivial => "trivial (≤2 nodes)",
            GraphComplexity::Simple => "simple (3–8 nodes, linear)",
            GraphComplexity::Moderate => "moderate (9–24 nodes, branching)",
            GraphComplexity::Complex => "complex (≥25 nodes or dense edges)",
        }
    }

    /// Returns `true` if the graph may require non-trivial scheduling.
    pub fn requires_advanced_scheduling(&self) -> bool {
        matches!(self, GraphComplexity::Moderate | GraphComplexity::Complex)
    }
}

/// Structural statistics about a directed graph.
#[derive(Debug, Clone)]
pub struct GraphStats {
    /// Number of nodes in the graph.
    node_count: usize,
    /// Number of directed edges.
    edge_count: usize,
    /// Maximum depth (longest path from any source node).
    max_depth: usize,
    /// Sum of all node in-degrees (equals `edge_count` for simple graphs).
    total_degree: usize,
}

impl GraphStats {
    /// Creates a `GraphStats` with the given raw values.
    pub fn new(node_count: usize, edge_count: usize, max_depth: usize) -> Self {
        Self {
            node_count,
            edge_count,
            max_depth,
            total_degree: edge_count,
        }
    }

    /// Returns the number of nodes.
    pub fn node_count(&self) -> usize {
        self.node_count
    }

    /// Returns the number of directed edges.
    pub fn edge_count(&self) -> usize {
        self.edge_count
    }

    /// Returns the average out-degree across all nodes.
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_degree(&self) -> f64 {
        if self.node_count == 0 {
            return 0.0;
        }
        self.edge_count as f64 / self.node_count as f64
    }

    /// Returns the maximum depth (length of the longest source-to-sink path).
    pub fn depth(&self) -> usize {
        self.max_depth
    }

    /// Classifies the graph by complexity.
    pub fn complexity(&self) -> GraphComplexity {
        if self.node_count <= 2 {
            GraphComplexity::Trivial
        } else if self.node_count <= 8 && self.avg_degree() <= 1.5 {
            GraphComplexity::Simple
        } else if self.node_count < 25 {
            GraphComplexity::Moderate
        } else {
            GraphComplexity::Complex
        }
    }

    /// Returns `true` if the graph has no edges.
    pub fn is_disconnected(&self) -> bool {
        self.edge_count == 0
    }
}

/// Analyzes a directed acyclic graph represented as an adjacency list.
///
/// Nodes are represented by `u64` IDs; edges as `(from, to)` pairs.
#[derive(Debug, Clone, Default)]
pub struct GraphAnalyzer {
    /// Adjacency list: node → list of successor nodes.
    adjacency: HashMap<u64, Vec<u64>>,
}

impl GraphAnalyzer {
    /// Creates an empty `GraphAnalyzer`.
    pub fn new() -> Self {
        Self {
            adjacency: HashMap::new(),
        }
    }

    /// Adds a node with no edges (idempotent).
    pub fn add_node(&mut self, id: u64) {
        self.adjacency.entry(id).or_default();
    }

    /// Adds a directed edge from `from` to `to`, implicitly adding both nodes.
    pub fn add_edge(&mut self, from: u64, to: u64) {
        self.adjacency.entry(from).or_default().push(to);
        self.adjacency.entry(to).or_default();
    }

    /// Computes the maximum depth via BFS from all source nodes (in-degree 0).
    fn max_depth(&self) -> usize {
        // Compute in-degrees.
        let mut in_degree: HashMap<u64, usize> = self.adjacency.keys().map(|&k| (k, 0)).collect();
        for successors in self.adjacency.values() {
            for &succ in successors {
                *in_degree.entry(succ).or_insert(0) += 1;
            }
        }

        // BFS level-by-level from all sources.
        let mut depth_map: HashMap<u64, usize> = HashMap::new();
        let mut queue: VecDeque<u64> = in_degree
            .iter()
            .filter_map(|(&n, &d)| if d == 0 { Some(n) } else { None })
            .collect();

        for &n in &queue {
            depth_map.insert(n, 0);
        }

        let mut max_d = 0;
        while let Some(node) = queue.pop_front() {
            let cur_depth = *depth_map.get(&node).unwrap_or(&0);
            if let Some(succs) = self.adjacency.get(&node) {
                for &succ in succs {
                    let new_depth = cur_depth + 1;
                    let entry = depth_map.entry(succ).or_insert(0);
                    if new_depth > *entry {
                        *entry = new_depth;
                        if new_depth > max_d {
                            max_d = new_depth;
                        }
                        queue.push_back(succ);
                    }
                }
            }
        }
        max_d
    }

    /// Returns the set of all unique node IDs.
    pub fn nodes(&self) -> HashSet<u64> {
        let mut nodes: HashSet<u64> = self.adjacency.keys().copied().collect();
        for succs in self.adjacency.values() {
            for &s in succs {
                nodes.insert(s);
            }
        }
        nodes
    }

    /// Returns the total number of directed edges.
    pub fn edge_count(&self) -> usize {
        self.adjacency.values().map(|v| v.len()).sum()
    }

    /// Computes and returns a `GraphStats` snapshot.
    pub fn analyze(&self) -> GraphStats {
        let node_count = self.nodes().len();
        let edge_count = self.edge_count();
        let max_depth = self.max_depth();
        GraphStats::new(node_count, edge_count, max_depth)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_description_trivial() {
        assert!(GraphComplexity::Trivial.description().contains("trivial"));
    }

    #[test]
    fn test_complexity_requires_advanced_trivial() {
        assert!(!GraphComplexity::Trivial.requires_advanced_scheduling());
    }

    #[test]
    fn test_complexity_requires_advanced_complex() {
        assert!(GraphComplexity::Complex.requires_advanced_scheduling());
    }

    #[test]
    fn test_graph_stats_avg_degree_zero_nodes() {
        let s = GraphStats::new(0, 0, 0);
        assert_eq!(s.avg_degree(), 0.0);
    }

    #[test]
    fn test_graph_stats_avg_degree() {
        let s = GraphStats::new(4, 4, 3);
        assert!((s.avg_degree() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_graph_stats_complexity_trivial() {
        let s = GraphStats::new(2, 1, 1);
        assert_eq!(s.complexity(), GraphComplexity::Trivial);
    }

    #[test]
    fn test_graph_stats_complexity_simple() {
        let s = GraphStats::new(5, 4, 4);
        assert_eq!(s.complexity(), GraphComplexity::Simple);
    }

    #[test]
    fn test_graph_stats_complexity_moderate() {
        let s = GraphStats::new(10, 12, 5);
        assert_eq!(s.complexity(), GraphComplexity::Moderate);
    }

    #[test]
    fn test_graph_stats_complexity_complex() {
        let s = GraphStats::new(30, 60, 10);
        assert_eq!(s.complexity(), GraphComplexity::Complex);
    }

    #[test]
    fn test_graph_stats_is_disconnected() {
        let s = GraphStats::new(3, 0, 0);
        assert!(s.is_disconnected());
    }

    #[test]
    fn test_analyzer_empty_graph() {
        let a = GraphAnalyzer::new();
        let stats = a.analyze();
        assert_eq!(stats.node_count(), 0);
        assert_eq!(stats.edge_count(), 0);
        assert_eq!(stats.depth(), 0);
    }

    #[test]
    fn test_analyzer_linear_chain() {
        // 0 -> 1 -> 2 -> 3 (depth 3)
        let mut a = GraphAnalyzer::new();
        a.add_edge(0, 1);
        a.add_edge(1, 2);
        a.add_edge(2, 3);
        let stats = a.analyze();
        assert_eq!(stats.node_count(), 4);
        assert_eq!(stats.edge_count(), 3);
        assert_eq!(stats.depth(), 3);
    }

    #[test]
    fn test_analyzer_branching_graph() {
        // 0 -> 1 -> 3
        // 0 -> 2 -> 3
        let mut a = GraphAnalyzer::new();
        a.add_edge(0, 1);
        a.add_edge(0, 2);
        a.add_edge(1, 3);
        a.add_edge(2, 3);
        let stats = a.analyze();
        assert_eq!(stats.node_count(), 4);
        assert_eq!(stats.edge_count(), 4);
        assert_eq!(stats.depth(), 2);
    }

    #[test]
    fn test_analyzer_isolated_nodes() {
        let mut a = GraphAnalyzer::new();
        a.add_node(0);
        a.add_node(1);
        let stats = a.analyze();
        assert_eq!(stats.node_count(), 2);
        assert_eq!(stats.edge_count(), 0);
        assert_eq!(stats.depth(), 0);
    }

    #[test]
    fn test_complexity_moderate_requires_advanced() {
        assert!(GraphComplexity::Moderate.requires_advanced_scheduling());
    }
}
