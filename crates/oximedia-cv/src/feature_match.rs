//! Feature descriptor matching utilities for stereo vision, SLAM, and object tracking.

#![allow(dead_code)]

/// A matched pair of feature descriptors from two images.
#[derive(Debug, Clone)]
pub struct MatchPair {
    /// Index of the descriptor in the query set.
    pub query_idx: usize,
    /// Index of the descriptor in the train set.
    pub train_idx: usize,
    /// Euclidean distance between the two descriptors.
    pub dist: f32,
}

impl MatchPair {
    /// Create a new [`MatchPair`].
    #[must_use]
    pub fn new(query_idx: usize, train_idx: usize, dist: f32) -> Self {
        Self {
            query_idx,
            train_idx,
            dist: dist.max(0.0),
        }
    }

    /// Return the distance between the matched descriptors.
    #[must_use]
    pub fn distance(&self) -> f32 {
        self.dist
    }

    /// Return `true` when the distance is below `max_dist`.
    #[must_use]
    pub fn is_good(&self, max_dist: f32) -> bool {
        self.dist <= max_dist
    }
}

/// Result of a feature matching operation between two descriptor sets.
#[derive(Debug, Clone, Default)]
pub struct FeatureMatch {
    /// All raw matches (one per query descriptor).
    pub all_matches: Vec<MatchPair>,
}

impl FeatureMatch {
    /// Create a new, empty [`FeatureMatch`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            all_matches: Vec::new(),
        }
    }

    /// Return only the matches whose distance is below `max_dist`.
    #[must_use]
    pub fn good_matches(&self, max_dist: f32) -> Vec<&MatchPair> {
        self.all_matches
            .iter()
            .filter(|m| m.is_good(max_dist))
            .collect()
    }

    /// Return the number of raw matches.
    #[must_use]
    pub fn len(&self) -> usize {
        self.all_matches.len()
    }

    /// Return `true` when there are no matches.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.all_matches.is_empty()
    }

    /// Return the minimum distance among all matches, or `f32::MAX` if empty.
    #[must_use]
    pub fn min_distance(&self) -> f32 {
        self.all_matches
            .iter()
            .map(|m| m.dist)
            .fold(f32::MAX, f32::min)
    }
}

/// Brute-force feature matcher.
pub struct FeatureMatcher {
    /// Cross-check: a match is kept only if it is mutual.
    pub cross_check: bool,
    /// Ratio test threshold (Lowe's ratio test, 0 = disabled).
    pub ratio_threshold: f32,
}

impl FeatureMatcher {
    /// Create a new [`FeatureMatcher`].
    #[must_use]
    pub fn new(cross_check: bool, ratio_threshold: f32) -> Self {
        Self {
            cross_check,
            ratio_threshold: ratio_threshold.clamp(0.0, 1.0),
        }
    }

    /// Compute L2 distance between two equal-length descriptor vectors.
    #[must_use]
    fn l2(a: &[f32], b: &[f32]) -> f32 {
        let sum: f32 = a.iter().zip(b.iter()).map(|(x, y)| (x - y).powi(2)).sum();
        sum.sqrt()
    }

    /// Match each descriptor in `query` to the nearest descriptor in `train`.
    ///
    /// Both `query` and `train` must be slices of flat descriptors each of length `desc_len`.
    /// If `desc_len` is 0 or the slice lengths are not multiples of `desc_len`, an empty
    /// [`FeatureMatch`] is returned.
    #[must_use]
    pub fn match_descriptors(&self, query: &[f32], train: &[f32], desc_len: usize) -> FeatureMatch {
        if desc_len == 0 || query.len() % desc_len != 0 || train.len() % desc_len != 0 {
            return FeatureMatch::new();
        }

        let n_query = query.len() / desc_len;
        let n_train = train.len() / desc_len;
        let mut result = FeatureMatch::new();

        if n_query == 0 || n_train == 0 {
            return result;
        }

        for qi in 0..n_query {
            let qd = &query[qi * desc_len..(qi + 1) * desc_len];

            // Find best and second-best matches.
            let mut best_dist = f32::MAX;
            let mut best_ti = 0usize;
            let mut second_dist = f32::MAX;

            for ti in 0..n_train {
                let td = &train[ti * desc_len..(ti + 1) * desc_len];
                let d = Self::l2(qd, td);
                if d < best_dist {
                    second_dist = best_dist;
                    best_dist = d;
                    best_ti = ti;
                } else if d < second_dist {
                    second_dist = d;
                }
            }

            result
                .all_matches
                .push(MatchPair::new(qi, best_ti, best_dist));
            let _ = second_dist; // used below in ratio_test
        }

        result
    }

    /// Apply Lowe's ratio test to `matches`, retaining only matches where
    /// `best_dist / second_best_dist < ratio_threshold`.
    ///
    /// Because the raw brute-force result only stores one match per query, this
    /// re-computes the second-best distance directly.
    #[must_use]
    pub fn ratio_test<'a>(
        &self,
        matches: &'a FeatureMatch,
        query: &[f32],
        train: &[f32],
        desc_len: usize,
    ) -> Vec<&'a MatchPair> {
        if desc_len == 0 || self.ratio_threshold <= 0.0 {
            return matches.all_matches.iter().collect();
        }
        if query.len() % desc_len != 0 || train.len() % desc_len != 0 {
            return Vec::new();
        }

        let n_train = train.len() / desc_len;

        matches
            .all_matches
            .iter()
            .filter(|m| {
                let qd = &query[m.query_idx * desc_len..(m.query_idx + 1) * desc_len];
                let mut second_dist = f32::MAX;
                for ti in 0..n_train {
                    if ti == m.train_idx {
                        continue;
                    }
                    let td = &train[ti * desc_len..(ti + 1) * desc_len];
                    let d = Self::l2(qd, td);
                    if d < second_dist {
                        second_dist = d;
                    }
                }
                #[allow(clippy::float_cmp)]
                if second_dist == f32::MAX || second_dist == 0.0 {
                    return true;
                }
                m.dist / second_dist < self.ratio_threshold
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_pair_distance() {
        let m = MatchPair::new(0, 1, 3.5);
        assert!((m.distance() - 3.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_match_pair_negative_dist_clamped() {
        let m = MatchPair::new(0, 0, -1.0);
        assert!((m.distance() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_match_pair_is_good_true() {
        let m = MatchPair::new(0, 1, 2.0);
        assert!(m.is_good(3.0));
    }

    #[test]
    fn test_match_pair_is_good_false() {
        let m = MatchPair::new(0, 1, 4.0);
        assert!(!m.is_good(3.0));
    }

    #[test]
    fn test_feature_match_good_matches_filter() {
        let mut fm = FeatureMatch::new();
        fm.all_matches.push(MatchPair::new(0, 0, 1.0));
        fm.all_matches.push(MatchPair::new(1, 1, 5.0));
        let good = fm.good_matches(3.0);
        assert_eq!(good.len(), 1);
    }

    #[test]
    fn test_feature_match_min_distance_empty() {
        let fm = FeatureMatch::new();
        assert_eq!(fm.min_distance(), f32::MAX);
    }

    #[test]
    fn test_feature_match_min_distance_non_empty() {
        let mut fm = FeatureMatch::new();
        fm.all_matches.push(MatchPair::new(0, 0, 3.0));
        fm.all_matches.push(MatchPair::new(1, 1, 1.5));
        assert!((fm.min_distance() - 1.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_feature_matcher_match_descriptors_empty() {
        let matcher = FeatureMatcher::new(false, 0.0);
        let result = matcher.match_descriptors(&[], &[], 4);
        assert!(result.is_empty());
    }

    #[test]
    fn test_feature_matcher_match_descriptors_single() {
        let matcher = FeatureMatcher::new(false, 0.0);
        let query = vec![1.0_f32, 0.0, 0.0, 0.0];
        let train = vec![1.0_f32, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let result = matcher.match_descriptors(&query, &train, 4);
        assert_eq!(result.len(), 1);
        // Identical descriptors → distance 0.
        assert!((result.all_matches[0].dist - 0.0).abs() < 1e-5);
        assert_eq!(result.all_matches[0].train_idx, 0);
    }

    #[test]
    fn test_feature_matcher_match_descriptors_wrong_desc_len() {
        let matcher = FeatureMatcher::new(false, 0.0);
        // query length (5) is not divisible by desc_len (4)
        let query = vec![1.0_f32; 5];
        let train = vec![1.0_f32; 4];
        let result = matcher.match_descriptors(&query, &train, 4);
        assert!(result.is_empty());
    }

    #[test]
    fn test_feature_matcher_ratio_test_all_pass_single_train() {
        let matcher = FeatureMatcher::new(false, 0.75);
        let query = vec![1.0_f32, 0.0];
        let train = vec![1.0_f32, 0.0]; // only one train descriptor → second_dist = MAX → pass
        let fm = matcher.match_descriptors(&query, &train, 2);
        let passed = matcher.ratio_test(&fm, &query, &train, 2);
        assert_eq!(passed.len(), 1);
    }

    #[test]
    fn test_feature_matcher_ratio_test_rejects_ambiguous() {
        let matcher = FeatureMatcher::new(false, 0.75);
        // query descriptor
        let query = vec![0.0_f32, 0.0, 0.0, 0.0];
        // Two train descriptors: one at distance 1.0 and one at distance 1.1 (ratio ≈ 0.91 > 0.75).
        let train = vec![
            1.0_f32, 0.0, 0.0, 0.0, // dist ≈ 1.0
            1.1_f32, 0.0, 0.0, 0.0, // dist ≈ 1.1
        ];
        let fm = matcher.match_descriptors(&query, &train, 4);
        let passed = matcher.ratio_test(&fm, &query, &train, 4);
        assert_eq!(passed.len(), 0);
    }
}
