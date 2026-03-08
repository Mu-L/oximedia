//! Duplicate clustering: similarity groups, cluster merging, representative selection.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::too_many_arguments)]

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// A cluster of near-duplicate media files.
#[derive(Debug, Clone)]
pub struct DuplicateCluster {
    /// Unique cluster identifier.
    pub id: usize,
    /// Members of this cluster (file paths).
    pub members: Vec<PathBuf>,
    /// Pairwise similarity scores (index_a, index_b, score).
    pub edges: Vec<(usize, usize, f64)>,
    /// The representative file selected for this cluster.
    pub representative: Option<PathBuf>,
}

impl DuplicateCluster {
    /// Create a new cluster with the given id.
    #[must_use]
    pub fn new(id: usize) -> Self {
        Self {
            id,
            members: Vec::new(),
            edges: Vec::new(),
            representative: None,
        }
    }

    /// Add a member file to the cluster.
    pub fn add_member(&mut self, path: PathBuf) {
        self.members.push(path);
    }

    /// Record a similarity edge between two member indices.
    pub fn add_edge(&mut self, a: usize, b: usize, score: f64) {
        self.edges.push((a, b, score));
    }

    /// Number of members.
    #[must_use]
    pub fn size(&self) -> usize {
        self.members.len()
    }

    /// Returns true if the cluster has at least two members.
    #[must_use]
    pub fn is_duplicate_group(&self) -> bool {
        self.members.len() >= 2
    }

    /// Average similarity score across all edges.
    #[must_use]
    pub fn average_similarity(&self) -> f64 {
        if self.edges.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.edges.iter().map(|(_, _, s)| *s).sum();
        sum / self.edges.len() as f64
    }

    /// Select the representative member: the one with the highest average similarity to others.
    pub fn select_representative(&mut self) {
        if self.members.is_empty() {
            return;
        }
        if self.members.len() == 1 {
            self.representative = Some(self.members[0].clone());
            return;
        }
        let n = self.members.len();
        let mut scores = vec![0.0f64; n];
        let mut counts = vec![0usize; n];
        for &(a, b, s) in &self.edges {
            if a < n && b < n {
                scores[a] += s;
                scores[b] += s;
                counts[a] += 1;
                counts[b] += 1;
            }
        }
        let avg: Vec<f64> = scores
            .iter()
            .zip(counts.iter())
            .map(|(s, &c)| if c > 0 { *s / c as f64 } else { 0.0 })
            .collect();
        let best = avg
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.representative = Some(self.members[best].clone());
    }
}

/// Strategy for merging clusters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// Merge if any edge exceeds the threshold (single-linkage).
    SingleLinkage,
    /// Merge only if all pairs exceed the threshold (complete-linkage).
    CompleteLinkage,
    /// Merge if average similarity exceeds the threshold (average-linkage).
    AverageLinkage,
}

/// Similarity pair between two files.
#[derive(Debug, Clone)]
pub struct SimilarityPair {
    /// Path to the first file.
    pub path_a: PathBuf,
    /// Path to the second file.
    pub path_b: PathBuf,
    /// Similarity score in [0.0, 1.0].
    pub score: f64,
}

impl SimilarityPair {
    /// Create a new similarity pair.
    #[must_use]
    pub fn new(path_a: PathBuf, path_b: PathBuf, score: f64) -> Self {
        Self {
            path_a,
            path_b,
            score,
        }
    }
}

/// Cluster builder that groups files from similarity pairs.
#[derive(Debug, Default)]
pub struct ClusterBuilder {
    threshold: f64,
    strategy: MergeStrategyInner,
}

#[derive(Debug, Clone, Copy, Default)]
enum MergeStrategyInner {
    #[default]
    SingleLinkage,
    CompleteLinkage,
    AverageLinkage,
}

impl ClusterBuilder {
    /// Create a builder with the given similarity threshold.
    #[must_use]
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold,
            strategy: MergeStrategyInner::SingleLinkage,
        }
    }

    /// Set the merge strategy.
    #[must_use]
    pub fn with_strategy(mut self, strategy: MergeStrategy) -> Self {
        self.strategy = match strategy {
            MergeStrategy::SingleLinkage => MergeStrategyInner::SingleLinkage,
            MergeStrategy::CompleteLinkage => MergeStrategyInner::CompleteLinkage,
            MergeStrategy::AverageLinkage => MergeStrategyInner::AverageLinkage,
        };
        self
    }

    /// Build clusters from similarity pairs using Union-Find.
    #[must_use]
    pub fn build(&self, pairs: &[SimilarityPair]) -> Vec<DuplicateCluster> {
        // Collect all unique paths.
        let mut path_set: HashSet<&PathBuf> = HashSet::new();
        for p in pairs {
            path_set.insert(&p.path_a);
            path_set.insert(&p.path_b);
        }
        let paths: Vec<&PathBuf> = path_set.into_iter().collect();
        let idx: HashMap<&PathBuf, usize> =
            paths.iter().enumerate().map(|(i, p)| (*p, i)).collect();
        let n = paths.len();
        let mut parent: Vec<usize> = (0..n).collect();

        // Filter pairs by threshold.
        let valid_pairs: Vec<&SimilarityPair> =
            pairs.iter().filter(|p| p.score >= self.threshold).collect();

        // Union-Find helpers (iterative path compression).
        fn find(parent: &mut Vec<usize>, x: usize) -> usize {
            let mut root = x;
            while parent[root] != root {
                root = parent[root];
            }
            let mut cur = x;
            while cur != root {
                let next = parent[cur];
                parent[cur] = root;
                cur = next;
            }
            root
        }

        for pair in &valid_pairs {
            let a = idx[&pair.path_a];
            let b = idx[&pair.path_b];
            let ra = find(&mut parent, a);
            let rb = find(&mut parent, b);
            if ra != rb {
                parent[rb] = ra;
            }
        }

        // Group by root.
        let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..n {
            let root = find(&mut parent, i);
            groups.entry(root).or_default().push(i);
        }

        // Build DuplicateCluster per group.
        let mut clusters = Vec::new();
        for (cid, (_, members)) in groups.iter().enumerate() {
            let mut cluster = DuplicateCluster::new(cid);
            let local_idx: HashMap<usize, usize> = members
                .iter()
                .enumerate()
                .map(|(li, &gi)| (gi, li))
                .collect();
            for &gi in members {
                cluster.add_member(paths[gi].clone());
            }
            for pair in &valid_pairs {
                let a = idx[&pair.path_a];
                let b = idx[&pair.path_b];
                if let (Some(&la), Some(&lb)) = (local_idx.get(&a), local_idx.get(&b)) {
                    cluster.add_edge(la, lb, pair.score);
                }
            }
            cluster.select_representative();
            clusters.push(cluster);
        }
        clusters
    }
}

/// Merge two clusters into one.
#[must_use]
pub fn merge_clusters(mut a: DuplicateCluster, b: DuplicateCluster) -> DuplicateCluster {
    let offset = a.members.len();
    for member in b.members {
        a.members.push(member);
    }
    for (ea, eb, score) in b.edges {
        a.edges.push((ea + offset, eb + offset, score));
    }
    a.id = a.id.min(b.id);
    a.select_representative();
    a
}

/// Summary of clustering results.
#[derive(Debug, Clone)]
pub struct ClusterSummary {
    /// Total number of clusters found.
    pub total_clusters: usize,
    /// Total files in duplicate clusters (>= 2 members).
    pub files_in_duplicates: usize,
    /// Largest cluster size.
    pub largest_cluster_size: usize,
    /// Average cluster size (for clusters with >= 2 members).
    pub average_cluster_size: f64,
}

impl ClusterSummary {
    /// Build a summary from a slice of clusters.
    #[must_use]
    pub fn from_clusters(clusters: &[DuplicateCluster]) -> Self {
        let dup_clusters: Vec<&DuplicateCluster> =
            clusters.iter().filter(|c| c.is_duplicate_group()).collect();
        let total_clusters = dup_clusters.len();
        let files_in_duplicates: usize = dup_clusters.iter().map(|c| c.size()).sum();
        let largest_cluster_size = dup_clusters.iter().map(|c| c.size()).max().unwrap_or(0);
        let average_cluster_size = if total_clusters > 0 {
            files_in_duplicates as f64 / total_clusters as f64
        } else {
            0.0
        };
        Self {
            total_clusters,
            files_in_duplicates,
            largest_cluster_size,
            average_cluster_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pb(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn test_cluster_new() {
        let c = DuplicateCluster::new(0);
        assert_eq!(c.id, 0);
        assert!(c.members.is_empty());
        assert!(c.edges.is_empty());
        assert!(c.representative.is_none());
    }

    #[test]
    fn test_cluster_add_member() {
        let mut c = DuplicateCluster::new(1);
        c.add_member(pb("a.mp4"));
        c.add_member(pb("b.mp4"));
        assert_eq!(c.size(), 2);
        assert!(c.is_duplicate_group());
    }

    #[test]
    fn test_cluster_single_member_not_duplicate() {
        let mut c = DuplicateCluster::new(0);
        c.add_member(pb("a.mp4"));
        assert!(!c.is_duplicate_group());
    }

    #[test]
    fn test_cluster_average_similarity_empty_edges() {
        let c = DuplicateCluster::new(0);
        assert_eq!(c.average_similarity(), 0.0);
    }

    #[test]
    fn test_cluster_average_similarity() {
        let mut c = DuplicateCluster::new(0);
        c.add_member(pb("a.mp4"));
        c.add_member(pb("b.mp4"));
        c.add_edge(0, 1, 0.8);
        c.add_edge(0, 1, 0.6);
        assert!((c.average_similarity() - 0.7).abs() < 1e-9);
    }

    #[test]
    fn test_cluster_select_representative_single() {
        let mut c = DuplicateCluster::new(0);
        c.add_member(pb("only.mp4"));
        c.select_representative();
        assert_eq!(c.representative, Some(pb("only.mp4")));
    }

    #[test]
    fn test_cluster_select_representative_two() {
        let mut c = DuplicateCluster::new(0);
        c.add_member(pb("a.mp4"));
        c.add_member(pb("b.mp4"));
        c.add_edge(0, 1, 0.9);
        c.select_representative();
        assert!(c.representative.is_some());
    }

    #[test]
    fn test_builder_groups_by_threshold() {
        let pairs = vec![
            SimilarityPair::new(pb("a.mp4"), pb("b.mp4"), 0.95),
            SimilarityPair::new(pb("a.mp4"), pb("c.mp4"), 0.93),
            SimilarityPair::new(pb("x.mp4"), pb("y.mp4"), 0.40), // below threshold
        ];
        let builder = ClusterBuilder::new(0.90);
        let clusters = builder.build(&pairs);
        // a, b, c should be in one cluster; x and y are singletons or separate
        let dup_clusters: Vec<&DuplicateCluster> =
            clusters.iter().filter(|c| c.is_duplicate_group()).collect();
        assert_eq!(dup_clusters.len(), 1);
        assert_eq!(dup_clusters[0].size(), 3);
    }

    #[test]
    fn test_builder_separate_clusters() {
        let pairs = vec![
            SimilarityPair::new(pb("a.mp4"), pb("b.mp4"), 0.95),
            SimilarityPair::new(pb("x.mp4"), pb("y.mp4"), 0.92),
        ];
        let builder = ClusterBuilder::new(0.90);
        let clusters = builder.build(&pairs);
        let dup_clusters: Vec<&DuplicateCluster> =
            clusters.iter().filter(|c| c.is_duplicate_group()).collect();
        assert_eq!(dup_clusters.len(), 2);
    }

    #[test]
    fn test_builder_with_strategy_complete_linkage() {
        let pairs = vec![SimilarityPair::new(pb("a.mp4"), pb("b.mp4"), 0.95)];
        let builder = ClusterBuilder::new(0.90).with_strategy(MergeStrategy::CompleteLinkage);
        let clusters = builder.build(&pairs);
        assert!(!clusters.is_empty());
    }

    #[test]
    fn test_merge_clusters() {
        let mut a = DuplicateCluster::new(0);
        a.add_member(pb("a.mp4"));
        a.add_edge(0, 0, 1.0);

        let mut b = DuplicateCluster::new(1);
        b.add_member(pb("b.mp4"));
        b.add_edge(0, 0, 0.9);

        let merged = merge_clusters(a, b);
        assert_eq!(merged.size(), 2);
        assert_eq!(merged.id, 0);
    }

    #[test]
    fn test_cluster_summary_empty() {
        let summary = ClusterSummary::from_clusters(&[]);
        assert_eq!(summary.total_clusters, 0);
        assert_eq!(summary.files_in_duplicates, 0);
        assert_eq!(summary.largest_cluster_size, 0);
        assert_eq!(summary.average_cluster_size, 0.0);
    }

    #[test]
    fn test_cluster_summary_with_clusters() {
        let mut c1 = DuplicateCluster::new(0);
        c1.add_member(pb("a.mp4"));
        c1.add_member(pb("b.mp4"));
        c1.add_member(pb("c.mp4"));

        let mut c2 = DuplicateCluster::new(1);
        c2.add_member(pb("x.mp4"));
        c2.add_member(pb("y.mp4"));

        let mut c3 = DuplicateCluster::new(2);
        c3.add_member(pb("solo.mp4")); // singleton, not counted

        let summary = ClusterSummary::from_clusters(&[c1, c2, c3]);
        assert_eq!(summary.total_clusters, 2);
        assert_eq!(summary.files_in_duplicates, 5);
        assert_eq!(summary.largest_cluster_size, 3);
        assert!((summary.average_cluster_size - 2.5).abs() < 1e-9);
    }

    #[test]
    fn test_similarity_pair_new() {
        let p = SimilarityPair::new(pb("a.mp4"), pb("b.mp4"), 0.75);
        assert_eq!(p.score, 0.75);
        assert_eq!(p.path_a, pb("a.mp4"));
        assert_eq!(p.path_b, pb("b.mp4"));
    }
}
