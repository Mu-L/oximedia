//! Wave 15 integration tests for oximedia-pipeline.
//!
//! Covers:
//!  - `test_pipeline_validation_disconnected_nodes` — validator flags a
//!    node that is not reachable from any source.
//!  - `test_pipeline_topo_sort_random_graph` — seeded xorshift-generated
//!    DAGs verified against the topological-order invariant.

use std::collections::HashMap;

use oximedia_pipeline::graph::{Edge, PipelineGraph};
use oximedia_pipeline::node::{
    FilterConfig, FrameFormat, NodeId, NodeSpec, SinkConfig, SourceConfig, StreamSpec,
};
use oximedia_pipeline::validation::{PipelineValidator, ValidationError};

// ── helpers ───────────────────────────────────────────────────────────────────

fn vs() -> StreamSpec {
    StreamSpec::video(FrameFormat::Yuv420p, 1920, 1080, 25)
}

fn filter_node(name: &str) -> NodeSpec {
    NodeSpec::filter(name, FilterConfig::Hflip, vs(), vs())
}

fn source_node(name: &str) -> NodeSpec {
    NodeSpec::source(name, SourceConfig::File("x.mp4".into()), vs())
}

fn sink_node(name: &str) -> NodeSpec {
    NodeSpec::sink(name, SinkConfig::Null, vs())
}

fn add_edge(g: &mut PipelineGraph, from: NodeId, to: NodeId) {
    g.edges.push(Edge {
        from_node: from,
        from_pad: "default".into(),
        to_node: to,
        to_pad: "default".into(),
    });
}

/// Assert that `order` is a valid topological order for `graph`:
/// every edge `(u, v)` must have `u` appearing before `v`.
fn assert_topo_valid(graph: &PipelineGraph, order: &[NodeId]) {
    let pos: HashMap<NodeId, usize> = order.iter().enumerate().map(|(i, &id)| (id, i)).collect();
    assert_eq!(
        order.len(),
        graph.nodes.len(),
        "topo order must contain all nodes"
    );
    for edge in &graph.edges {
        let fp = pos.get(&edge.from_node).copied().unwrap_or(usize::MAX);
        let tp = pos.get(&edge.to_node).copied().unwrap_or(usize::MAX);
        assert!(
            fp < tp,
            "topo invariant broken: edge from pos {fp} to pos {tp} (from={:?}, to={:?})",
            edge.from_node,
            edge.to_node
        );
    }
}

// ── Tiny seeded PRNG (32-bit xorshift) ───────────────────────────────────────

struct Xorshift32 {
    state: u32,
}

impl Xorshift32 {
    fn new(seed: u32) -> Self {
        // Seed must be non-zero; use 1 as fallback.
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    fn next(&mut self) -> u32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        self.state
    }

    /// Return a value in `0..n` (exclusive). Panics if `n == 0`.
    fn next_usize(&mut self, n: usize) -> usize {
        (self.next() as usize) % n
    }
}

/// Build a random DAG with `node_count` source-filter nodes using `rng`.
///
/// Strategy: assign each node a "layer" in `0..layer_count`, then for each
/// node in layer `k` draw `edge_density` random targets in layer `k+1..`.
/// This guarantees the graph is acyclic by construction.
fn build_random_dag(node_count: usize, rng: &mut Xorshift32) -> PipelineGraph {
    assert!(node_count >= 2);
    let mut g = PipelineGraph::new();

    // All nodes are filters/sources/sinks; we use a source for node 0,
    // sink for node N-1, and filters for the rest.
    let ids: Vec<NodeId> = (0..node_count)
        .map(|i| {
            let spec = if i == 0 {
                source_node(&format!("n{i}"))
            } else if i == node_count - 1 {
                sink_node(&format!("n{i}"))
            } else {
                filter_node(&format!("n{i}"))
            };
            g.add_node(spec)
        })
        .collect();

    // Add edges only from lower-indexed to higher-indexed nodes so the graph
    // is always a DAG.  Draw ~1-2 edges per intermediate node.
    for from_idx in 0..node_count - 1 {
        // Number of forward edges from this node (1 or 2).
        let n_edges = 1 + rng.next_usize(2);
        for _ in 0..n_edges {
            // Target must have a strictly higher index.
            let range = node_count - from_idx - 1;
            if range == 0 {
                break;
            }
            let to_idx = from_idx + 1 + rng.next_usize(range);
            add_edge(&mut g, ids[from_idx], ids[to_idx]);
        }
    }

    g
}

// ── Test 3: validator flags disconnected (unreachable) node ──────────────────

#[test]
fn test_pipeline_validation_disconnected_nodes() {
    let mut g = PipelineGraph::new();

    // Build a valid source→sink chain.
    let src = g.add_node(source_node("src"));
    let sk = g.add_node(sink_node("sink"));
    add_edge(&mut g, src, sk);

    // Add a filter that is completely disconnected (no input wired, no output
    // wired — it just floats in the graph without being connected to the
    // reachable component).
    g.add_node(filter_node("orphan_filter"));

    let report = PipelineValidator::new().validate(&g);

    // The validator must report at least one error.
    assert!(
        !report.is_valid,
        "graph with disconnected node should be invalid"
    );

    // There must be a DisconnectedNode error naming "orphan_filter".
    let has_disconnected = report.errors.iter().any(|e| match e {
        ValidationError::DisconnectedNode { node_name } => node_name == "orphan_filter",
        _ => false,
    });
    assert!(
        has_disconnected,
        "expected DisconnectedNode error for 'orphan_filter'; got errors: {:?}",
        report.errors
    );
}

// ── Test 4: topological sort is valid on 5 random DAGs of 8–12 nodes ─────────

#[test]
fn test_pipeline_topo_sort_random_graph() {
    // Five deterministic seeds producing different graph shapes.
    let seeds: [(u32, usize); 5] = [
        (0xDEAD_BEEF, 8),
        (0xCAFE_BABE, 9),
        (0x1234_5678, 10),
        (0xABCD_EF01, 11),
        (0xF00D_FACE, 12),
    ];

    for (seed, node_count) in seeds {
        let mut rng = Xorshift32::new(seed);
        let graph = build_random_dag(node_count, &mut rng);

        // The graph must be acyclic by construction, so topo sort must succeed.
        let order = graph
            .topological_sort()
            .unwrap_or_else(|e| panic!("topo sort failed for seed={seed:#010x}: {e}"));

        // (a) All node IDs appear in the order.
        assert_eq!(
            order.len(),
            graph.nodes.len(),
            "seed={seed:#010x}: topo order length {} != node count {}",
            order.len(),
            graph.nodes.len()
        );

        // (b) Every edge points forward in the sort order.
        assert_topo_valid(&graph, &order);
    }
}
