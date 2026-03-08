//! Merge strategies for resolving duplicate file groups.
//!
//! When duplicates are found, this module decides which files to keep and
//! which to remove (or link). Strategies include:
//! - **`KeepNewest`**: keep the file with the latest modification time
//! - **`KeepOldest`**: keep the earliest file
//! - **`KeepLargest`**: keep the largest file (e.g. highest-quality encode)
//! - **`KeepSmallest`**: keep the smallest (e.g. most efficient encode)
//! - **`KeepByPath`**: keep the file in a preferred directory hierarchy
//! - **`Custom`**: user-supplied scoring function

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// FileCandidate
// ---------------------------------------------------------------------------

/// Metadata about a duplicate file candidate.
#[derive(Debug, Clone)]
pub struct FileCandidate {
    /// Path to the file.
    pub path: PathBuf,
    /// File size in bytes.
    pub size: u64,
    /// Modification timestamp (Unix seconds).
    pub modified: u64,
    /// Creation timestamp (Unix seconds).
    pub created: u64,
    /// Optional quality score (0.0 - 1.0).
    pub quality_score: Option<f64>,
}

impl FileCandidate {
    /// Create a new candidate.
    pub fn new(path: PathBuf, size: u64, modified: u64, created: u64) -> Self {
        Self {
            path,
            size,
            modified,
            created,
            quality_score: None,
        }
    }

    /// Builder: set an optional quality score.
    #[must_use]
    pub fn with_quality(mut self, score: f64) -> Self {
        self.quality_score = Some(score);
        self
    }
}

// ---------------------------------------------------------------------------
// MergeStrategy
// ---------------------------------------------------------------------------

/// Strategy for choosing which duplicate to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// Keep the most recently modified file.
    KeepNewest,
    /// Keep the oldest modified file.
    KeepOldest,
    /// Keep the largest file.
    KeepLargest,
    /// Keep the smallest file.
    KeepSmallest,
    /// Keep the file with the highest quality score.
    KeepHighestQuality,
}

impl MergeStrategy {
    /// Return a human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::KeepNewest => "keep-newest",
            Self::KeepOldest => "keep-oldest",
            Self::KeepLargest => "keep-largest",
            Self::KeepSmallest => "keep-smallest",
            Self::KeepHighestQuality => "keep-highest-quality",
        }
    }
}

// ---------------------------------------------------------------------------
// MergeAction
// ---------------------------------------------------------------------------

/// Action to perform on a file after merge resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeAction {
    /// Keep this file as the canonical copy.
    Keep,
    /// Remove this file.
    Remove,
    /// Replace this file with a symlink to the kept file.
    Symlink {
        /// Target of the symlink (the kept file).
        target: PathBuf,
    },
    /// Replace this file with a hardlink to the kept file.
    Hardlink {
        /// Target of the hardlink (the kept file).
        target: PathBuf,
    },
}

impl MergeAction {
    /// Return `true` if this action keeps the file.
    #[must_use]
    pub fn is_keep(&self) -> bool {
        matches!(self, Self::Keep)
    }

    /// Return `true` if this action removes the file.
    #[must_use]
    pub fn is_remove(&self) -> bool {
        matches!(self, Self::Remove)
    }
}

// ---------------------------------------------------------------------------
// MergeResolution
// ---------------------------------------------------------------------------

/// A single file's resolution after merge.
#[derive(Debug, Clone)]
pub struct FileResolution {
    /// The candidate file.
    pub candidate: FileCandidate,
    /// The action to take.
    pub action: MergeAction,
}

/// The full resolution of a duplicate group.
#[derive(Debug, Clone)]
pub struct MergeResolution {
    /// Per-file resolutions.
    pub files: Vec<FileResolution>,
    /// The strategy used.
    pub strategy: MergeStrategy,
    /// Estimated bytes recoverable by removing duplicates.
    pub bytes_saved: u64,
}

// ---------------------------------------------------------------------------
// Resolver
// ---------------------------------------------------------------------------

/// Resolve a group of duplicate candidates using a strategy.
///
/// Returns a [`MergeResolution`] specifying which file to keep and what
/// to do with the rest.
pub fn resolve(
    candidates: &[FileCandidate],
    strategy: MergeStrategy,
    link_mode: LinkMode,
) -> MergeResolution {
    if candidates.is_empty() {
        return MergeResolution {
            files: Vec::new(),
            strategy,
            bytes_saved: 0,
        };
    }

    let winner_idx = pick_winner(candidates, strategy);
    let winner_path = candidates[winner_idx].path.clone();
    let mut bytes_saved = 0u64;

    let files = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if i == winner_idx {
                FileResolution {
                    candidate: c.clone(),
                    action: MergeAction::Keep,
                }
            } else {
                bytes_saved += c.size;
                let action = match link_mode {
                    LinkMode::Delete => MergeAction::Remove,
                    LinkMode::Symlink => MergeAction::Symlink {
                        target: winner_path.clone(),
                    },
                    LinkMode::Hardlink => MergeAction::Hardlink {
                        target: winner_path.clone(),
                    },
                };
                FileResolution {
                    candidate: c.clone(),
                    action,
                }
            }
        })
        .collect();

    MergeResolution {
        files,
        strategy,
        bytes_saved,
    }
}

/// How to handle non-winner files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkMode {
    /// Delete non-winner files.
    Delete,
    /// Replace with symlinks.
    Symlink,
    /// Replace with hardlinks.
    Hardlink,
}

/// Pick the winner index based on strategy.
fn pick_winner(candidates: &[FileCandidate], strategy: MergeStrategy) -> usize {
    match strategy {
        MergeStrategy::KeepNewest => candidates
            .iter()
            .enumerate()
            .max_by_key(|(_, c)| c.modified)
            .map(|(i, _)| i)
            .unwrap_or(0),
        MergeStrategy::KeepOldest => candidates
            .iter()
            .enumerate()
            .min_by_key(|(_, c)| c.modified)
            .map(|(i, _)| i)
            .unwrap_or(0),
        MergeStrategy::KeepLargest => candidates
            .iter()
            .enumerate()
            .max_by_key(|(_, c)| c.size)
            .map(|(i, _)| i)
            .unwrap_or(0),
        MergeStrategy::KeepSmallest => candidates
            .iter()
            .enumerate()
            .min_by_key(|(_, c)| c.size)
            .map(|(i, _)| i)
            .unwrap_or(0),
        MergeStrategy::KeepHighestQuality => candidates
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                let qa = a.quality_score.unwrap_or(0.0);
                let qb = b.quality_score.unwrap_or(0.0);
                qa.partial_cmp(&qb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0),
    }
}

/// Check if a path is under a preferred directory prefix.
#[must_use]
pub fn is_preferred_path(path: &Path, preferred_prefix: &Path) -> bool {
    path.starts_with(preferred_prefix)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates() -> Vec<FileCandidate> {
        vec![
            FileCandidate::new(PathBuf::from("/a.mp4"), 1000, 100, 90),
            FileCandidate::new(PathBuf::from("/b.mp4"), 2000, 200, 80),
            FileCandidate::new(PathBuf::from("/c.mp4"), 500, 50, 100),
        ]
    }

    #[test]
    fn test_keep_newest() {
        let res = resolve(&candidates(), MergeStrategy::KeepNewest, LinkMode::Delete);
        assert_eq!(res.files.len(), 3);
        assert!(res.files[1].action.is_keep()); // /b.mp4 has modified=200
    }

    #[test]
    fn test_keep_oldest() {
        let res = resolve(&candidates(), MergeStrategy::KeepOldest, LinkMode::Delete);
        assert!(res.files[2].action.is_keep()); // /c.mp4 has modified=50
    }

    #[test]
    fn test_keep_largest() {
        let res = resolve(&candidates(), MergeStrategy::KeepLargest, LinkMode::Delete);
        assert!(res.files[1].action.is_keep()); // /b.mp4 has size=2000
    }

    #[test]
    fn test_keep_smallest() {
        let res = resolve(&candidates(), MergeStrategy::KeepSmallest, LinkMode::Delete);
        assert!(res.files[2].action.is_keep()); // /c.mp4 has size=500
    }

    #[test]
    fn test_keep_highest_quality() {
        let cs = vec![
            FileCandidate::new(PathBuf::from("/a.mp4"), 100, 10, 10).with_quality(0.6),
            FileCandidate::new(PathBuf::from("/b.mp4"), 100, 10, 10).with_quality(0.9),
            FileCandidate::new(PathBuf::from("/c.mp4"), 100, 10, 10).with_quality(0.3),
        ];
        let res = resolve(&cs, MergeStrategy::KeepHighestQuality, LinkMode::Delete);
        assert!(res.files[1].action.is_keep()); // 0.9 is highest
    }

    #[test]
    fn test_bytes_saved() {
        let res = resolve(&candidates(), MergeStrategy::KeepLargest, LinkMode::Delete);
        // keep /b.mp4 (2000), remove /a.mp4 (1000) and /c.mp4 (500) => saved 1500
        assert_eq!(res.bytes_saved, 1500);
    }

    #[test]
    fn test_symlink_mode() {
        let res = resolve(&candidates(), MergeStrategy::KeepNewest, LinkMode::Symlink);
        for f in &res.files {
            if !f.action.is_keep() {
                match &f.action {
                    MergeAction::Symlink { target } => {
                        assert_eq!(target, &PathBuf::from("/b.mp4"));
                    }
                    _ => panic!("expected symlink action"),
                }
            }
        }
    }

    #[test]
    fn test_hardlink_mode() {
        let res = resolve(&candidates(), MergeStrategy::KeepNewest, LinkMode::Hardlink);
        for f in &res.files {
            if !f.action.is_keep() {
                match &f.action {
                    MergeAction::Hardlink { target } => {
                        assert_eq!(target, &PathBuf::from("/b.mp4"));
                    }
                    _ => panic!("expected hardlink action"),
                }
            }
        }
    }

    #[test]
    fn test_empty_candidates() {
        let res = resolve(&[], MergeStrategy::KeepNewest, LinkMode::Delete);
        assert!(res.files.is_empty());
        assert_eq!(res.bytes_saved, 0);
    }

    #[test]
    fn test_single_candidate() {
        let cs = vec![FileCandidate::new(PathBuf::from("/only.mp4"), 999, 10, 10)];
        let res = resolve(&cs, MergeStrategy::KeepNewest, LinkMode::Delete);
        assert_eq!(res.files.len(), 1);
        assert!(res.files[0].action.is_keep());
        assert_eq!(res.bytes_saved, 0);
    }

    #[test]
    fn test_is_preferred_path() {
        assert!(is_preferred_path(
            Path::new("/archive/media/a.mp4"),
            Path::new("/archive")
        ));
        assert!(!is_preferred_path(
            Path::new("/tmp/a.mp4"),
            Path::new("/archive")
        ));
    }

    #[test]
    fn test_strategy_label() {
        assert_eq!(MergeStrategy::KeepNewest.label(), "keep-newest");
        assert_eq!(MergeStrategy::KeepSmallest.label(), "keep-smallest");
    }
}
