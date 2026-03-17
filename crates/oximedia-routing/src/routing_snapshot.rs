//! Routing snapshot save/restore with atomic rollback.
//!
//! Captures the complete state of a [`CrosspointMatrix`] at a point in time
//! and allows restoring it later for instant recall or undo-style rollback.

use std::collections::HashMap;

use crate::matrix::crosspoint::CrosspointMatrix;

/// A frozen snapshot of a crosspoint matrix state.
#[derive(Debug, Clone)]
pub struct RoutingSnapshot {
    /// Snapshot identifier.
    pub id: u64,
    /// Human-readable name.
    pub name: String,
    /// Description / notes.
    pub description: String,
    /// Timestamp in microseconds (caller-supplied, not wall-clock).
    pub timestamp_us: u64,
    /// Matrix dimensions (inputs, outputs).
    pub dimensions: (usize, usize),
    /// Saved crosspoint states.
    crosspoints: HashMap<(usize, usize), f32>,
    /// Input labels at the time of capture.
    input_labels: Vec<String>,
    /// Output labels at the time of capture.
    output_labels: Vec<String>,
}

impl RoutingSnapshot {
    /// Number of active crosspoints in this snapshot.
    pub fn active_count(&self) -> usize {
        self.crosspoints.len()
    }

    /// Returns `true` if the snapshot has no active crosspoints.
    pub fn is_empty(&self) -> bool {
        self.crosspoints.is_empty()
    }

    /// Returns the saved crosspoint gain for (input, output), if connected.
    pub fn get_gain(&self, input: usize, output: usize) -> Option<f32> {
        self.crosspoints.get(&(input, output)).copied()
    }

    /// Returns all active crosspoints as (input, output, gain_db) triples.
    pub fn active_crosspoints(&self) -> Vec<(usize, usize, f32)> {
        self.crosspoints
            .iter()
            .map(|(&(i, o), &g)| (i, o, g))
            .collect()
    }

    /// Returns the input labels.
    pub fn input_labels(&self) -> &[String] {
        &self.input_labels
    }

    /// Returns the output labels.
    pub fn output_labels(&self) -> &[String] {
        &self.output_labels
    }
}

/// Difference between two snapshots.
#[derive(Debug, Clone)]
pub struct SnapshotDiff {
    /// Crosspoints that were added (not in `before` but in `after`).
    pub added: Vec<(usize, usize, f32)>,
    /// Crosspoints that were removed (in `before` but not in `after`).
    pub removed: Vec<(usize, usize, f32)>,
    /// Crosspoints whose gain changed.
    pub gain_changed: Vec<(usize, usize, f32, f32)>,
    /// Number of unchanged crosspoints.
    pub unchanged: usize,
}

impl SnapshotDiff {
    /// Returns `true` if the two snapshots are identical.
    pub fn is_identical(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.gain_changed.is_empty()
    }

    /// Total number of differences.
    pub fn diff_count(&self) -> usize {
        self.added.len() + self.removed.len() + self.gain_changed.len()
    }
}

/// Manages routing snapshots with save/restore/rollback.
#[derive(Debug)]
pub struct SnapshotManager {
    snapshots: Vec<RoutingSnapshot>,
    next_id: u64,
    /// Maximum number of snapshots to retain (0 = unlimited).
    max_snapshots: usize,
}

impl Default for SnapshotManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotManager {
    /// Creates a new, empty snapshot manager.
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
            next_id: 0,
            max_snapshots: 0,
        }
    }

    /// Creates a manager that retains at most `max` snapshots.
    pub fn with_max(max: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            next_id: 0,
            max_snapshots: max,
        }
    }

    /// Captures a snapshot of the given matrix.
    pub fn capture(
        &mut self,
        matrix: &CrosspointMatrix,
        name: impl Into<String>,
        description: impl Into<String>,
        timestamp_us: u64,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let mut crosspoints = HashMap::new();
        let active = matrix.get_active_crosspoints();
        for (cp_id, gain_db) in active {
            crosspoints.insert((cp_id.input, cp_id.output), gain_db);
        }

        let mut input_labels = Vec::new();
        for i in 0..matrix.input_count() {
            input_labels.push(matrix.get_input_label(i).unwrap_or("?").to_string());
        }
        let mut output_labels = Vec::new();
        for o in 0..matrix.output_count() {
            output_labels.push(matrix.get_output_label(o).unwrap_or("?").to_string());
        }

        let snapshot = RoutingSnapshot {
            id,
            name: name.into(),
            description: description.into(),
            timestamp_us,
            dimensions: (matrix.input_count(), matrix.output_count()),
            crosspoints,
            input_labels,
            output_labels,
        };

        self.snapshots.push(snapshot);

        // Evict oldest if needed
        if self.max_snapshots > 0 && self.snapshots.len() > self.max_snapshots {
            self.snapshots.remove(0);
        }

        id
    }

    /// Restores a snapshot to the given matrix.
    ///
    /// Returns `Err` if the snapshot is not found or dimensions don't match.
    pub fn restore(
        &self,
        snapshot_id: u64,
        matrix: &mut CrosspointMatrix,
    ) -> Result<(), SnapshotError> {
        let snapshot = self
            .get(snapshot_id)
            .ok_or(SnapshotError::NotFound(snapshot_id))?;

        if snapshot.dimensions != (matrix.input_count(), matrix.output_count()) {
            return Err(SnapshotError::DimensionMismatch {
                snapshot: snapshot.dimensions,
                matrix: (matrix.input_count(), matrix.output_count()),
            });
        }

        // Clear current state
        matrix.clear_all();

        // Restore all crosspoints
        for (&(input, output), &gain_db) in &snapshot.crosspoints {
            matrix.connect(input, output, Some(gain_db)).map_err(|e| {
                SnapshotError::RestoreError(format!("Failed to connect ({input},{output}): {e}"))
            })?;
        }

        // Restore labels
        for (i, label) in snapshot.input_labels.iter().enumerate() {
            let _ = matrix.set_input_label(i, label.clone());
        }
        for (o, label) in snapshot.output_labels.iter().enumerate() {
            let _ = matrix.set_output_label(o, label.clone());
        }

        Ok(())
    }

    /// Gets a snapshot by ID.
    pub fn get(&self, id: u64) -> Option<&RoutingSnapshot> {
        self.snapshots.iter().find(|s| s.id == id)
    }

    /// Returns the number of stored snapshots.
    pub fn count(&self) -> usize {
        self.snapshots.len()
    }

    /// Returns `true` if no snapshots are stored.
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// Deletes a snapshot by ID.
    pub fn delete(&mut self, id: u64) -> bool {
        if let Some(pos) = self.snapshots.iter().position(|s| s.id == id) {
            self.snapshots.remove(pos);
            true
        } else {
            false
        }
    }

    /// Returns the most recently captured snapshot.
    pub fn latest(&self) -> Option<&RoutingSnapshot> {
        self.snapshots.last()
    }

    /// Lists all snapshot IDs and names.
    pub fn list(&self) -> Vec<(u64, &str)> {
        self.snapshots
            .iter()
            .map(|s| (s.id, s.name.as_str()))
            .collect()
    }

    /// Computes a diff between two snapshots.
    pub fn diff(&self, before_id: u64, after_id: u64) -> Result<SnapshotDiff, SnapshotError> {
        let before = self
            .get(before_id)
            .ok_or(SnapshotError::NotFound(before_id))?;
        let after = self
            .get(after_id)
            .ok_or(SnapshotError::NotFound(after_id))?;

        Ok(diff_snapshots(before, after))
    }

    /// Clears all snapshots.
    pub fn clear(&mut self) {
        self.snapshots.clear();
    }
}

/// Computes the diff between two routing snapshots.
pub fn diff_snapshots(before: &RoutingSnapshot, after: &RoutingSnapshot) -> SnapshotDiff {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut gain_changed = Vec::new();
    let mut unchanged = 0usize;

    // Find removed and gain-changed
    for (&(i, o), &gain_before) in &before.crosspoints {
        match after.crosspoints.get(&(i, o)) {
            Some(&gain_after) => {
                if (gain_before - gain_after).abs() > 1e-6 {
                    gain_changed.push((i, o, gain_before, gain_after));
                } else {
                    unchanged += 1;
                }
            }
            None => {
                removed.push((i, o, gain_before));
            }
        }
    }

    // Find added
    for (&(i, o), &gain_after) in &after.crosspoints {
        if !before.crosspoints.contains_key(&(i, o)) {
            added.push((i, o, gain_after));
        }
    }

    SnapshotDiff {
        added,
        removed,
        gain_changed,
        unchanged,
    }
}

/// Errors from snapshot operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SnapshotError {
    /// Snapshot not found.
    #[error("Snapshot not found: {0}")]
    NotFound(u64),
    /// Dimension mismatch between snapshot and target matrix.
    #[error("Dimension mismatch: snapshot {snapshot:?} vs matrix {matrix:?}")]
    DimensionMismatch {
        snapshot: (usize, usize),
        matrix: (usize, usize),
    },
    /// Error during restore.
    #[error("Restore error: {0}")]
    RestoreError(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_matrix() -> CrosspointMatrix {
        let mut m = CrosspointMatrix::new(4, 4);
        m.connect(0, 0, Some(-6.0)).expect("valid");
        m.connect(1, 1, Some(0.0)).expect("valid");
        m.connect(2, 3, Some(-12.0)).expect("valid");
        m
    }

    #[test]
    fn test_capture_and_count() {
        let mut mgr = SnapshotManager::new();
        let m = make_matrix();
        let id = mgr.capture(&m, "snap1", "test", 1000);
        assert_eq!(id, 0);
        assert_eq!(mgr.count(), 1);
    }

    #[test]
    fn test_restore_roundtrip() {
        let mut mgr = SnapshotManager::new();
        let original = make_matrix();
        let id = mgr.capture(&original, "before", "", 0);

        let mut target = CrosspointMatrix::new(4, 4);
        assert!(!target.is_connected(0, 0));

        mgr.restore(id, &mut target).expect("restore ok");
        assert!(target.is_connected(0, 0));
        assert!(target.is_connected(1, 1));
        assert!(target.is_connected(2, 3));
    }

    #[test]
    fn test_restore_not_found() {
        let mgr = SnapshotManager::new();
        let mut m = CrosspointMatrix::new(4, 4);
        let result = mgr.restore(99, &mut m);
        assert!(result.is_err());
    }

    #[test]
    fn test_restore_dimension_mismatch() {
        let mut mgr = SnapshotManager::new();
        let m4 = make_matrix();
        let id = mgr.capture(&m4, "4x4", "", 0);

        let mut m8 = CrosspointMatrix::new(8, 8);
        let result = mgr.restore(id, &mut m8);
        assert!(matches!(
            result,
            Err(SnapshotError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn test_snapshot_active_count() {
        let mut mgr = SnapshotManager::new();
        let m = make_matrix();
        mgr.capture(&m, "s", "", 0);
        let snap = mgr.get(0).expect("exists");
        assert_eq!(snap.active_count(), 3);
        assert!(!snap.is_empty());
    }

    #[test]
    fn test_snapshot_get_gain() {
        let mut mgr = SnapshotManager::new();
        let m = make_matrix();
        mgr.capture(&m, "s", "", 0);
        let snap = mgr.get(0).expect("exists");
        assert!((snap.get_gain(0, 0).expect("exists") - (-6.0)).abs() < 1e-6);
        assert!(snap.get_gain(3, 3).is_none());
    }

    #[test]
    fn test_delete_snapshot() {
        let mut mgr = SnapshotManager::new();
        let m = make_matrix();
        let id = mgr.capture(&m, "s", "", 0);
        assert!(mgr.delete(id));
        assert_eq!(mgr.count(), 0);
        assert!(!mgr.delete(id)); // already deleted
    }

    #[test]
    fn test_latest() {
        let mut mgr = SnapshotManager::new();
        let m = make_matrix();
        mgr.capture(&m, "first", "", 0);
        mgr.capture(&m, "second", "", 100);
        assert_eq!(mgr.latest().expect("exists").name, "second");
    }

    #[test]
    fn test_list() {
        let mut mgr = SnapshotManager::new();
        let m = make_matrix();
        mgr.capture(&m, "alpha", "", 0);
        mgr.capture(&m, "beta", "", 100);
        let list = mgr.list();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_max_snapshots_eviction() {
        let mut mgr = SnapshotManager::with_max(2);
        let m = make_matrix();
        mgr.capture(&m, "a", "", 0);
        mgr.capture(&m, "b", "", 100);
        mgr.capture(&m, "c", "", 200);
        assert_eq!(mgr.count(), 2);
        // First snapshot should have been evicted
        assert!(mgr.get(0).is_none());
        assert!(mgr.get(1).is_some());
    }

    #[test]
    fn test_diff_identical() {
        let mut mgr = SnapshotManager::new();
        let m = make_matrix();
        let id1 = mgr.capture(&m, "a", "", 0);
        let id2 = mgr.capture(&m, "b", "", 100);
        let diff = mgr.diff(id1, id2).expect("valid");
        assert!(diff.is_identical());
        assert_eq!(diff.diff_count(), 0);
        assert_eq!(diff.unchanged, 3);
    }

    #[test]
    fn test_diff_added_removed() {
        let mut mgr = SnapshotManager::new();
        let mut m1 = CrosspointMatrix::new(4, 4);
        m1.connect(0, 0, Some(0.0)).expect("valid");
        let id1 = mgr.capture(&m1, "before", "", 0);

        let mut m2 = CrosspointMatrix::new(4, 4);
        m2.connect(1, 1, Some(-3.0)).expect("valid");
        let id2 = mgr.capture(&m2, "after", "", 100);

        let diff = mgr.diff(id1, id2).expect("valid");
        assert_eq!(diff.removed.len(), 1); // (0,0) removed
        assert_eq!(diff.added.len(), 1); // (1,1) added
        assert_eq!(diff.unchanged, 0);
    }

    #[test]
    fn test_diff_gain_changed() {
        let mut mgr = SnapshotManager::new();
        let mut m1 = CrosspointMatrix::new(4, 4);
        m1.connect(0, 0, Some(0.0)).expect("valid");
        let id1 = mgr.capture(&m1, "before", "", 0);

        let mut m2 = CrosspointMatrix::new(4, 4);
        m2.connect(0, 0, Some(-6.0)).expect("valid");
        let id2 = mgr.capture(&m2, "after", "", 100);

        let diff = mgr.diff(id1, id2).expect("valid");
        assert_eq!(diff.gain_changed.len(), 1);
        let (i, o, before, after) = diff.gain_changed[0];
        assert_eq!((i, o), (0, 0));
        assert!((before - 0.0).abs() < 1e-6);
        assert!((after - (-6.0)).abs() < 1e-6);
    }

    #[test]
    fn test_diff_not_found() {
        let mgr = SnapshotManager::new();
        assert!(mgr.diff(0, 1).is_err());
    }

    #[test]
    fn test_clear() {
        let mut mgr = SnapshotManager::new();
        let m = make_matrix();
        mgr.capture(&m, "a", "", 0);
        mgr.clear();
        assert!(mgr.is_empty());
    }

    #[test]
    fn test_empty_snapshot() {
        let mut mgr = SnapshotManager::new();
        let m = CrosspointMatrix::new(4, 4);
        mgr.capture(&m, "empty", "", 0);
        let snap = mgr.get(0).expect("exists");
        assert!(snap.is_empty());
        assert_eq!(snap.active_count(), 0);
    }

    #[test]
    fn test_snapshot_labels_preserved() {
        let mut mgr = SnapshotManager::new();
        let mut m = CrosspointMatrix::new(2, 2);
        m.set_input_label(0, "Mic 1".to_string()).expect("ok");
        m.set_output_label(1, "Mon R".to_string()).expect("ok");
        mgr.capture(&m, "labeled", "", 0);
        let snap = mgr.get(0).expect("exists");
        assert_eq!(snap.input_labels()[0], "Mic 1");
        assert_eq!(snap.output_labels()[1], "Mon R");
    }

    #[test]
    fn test_restore_preserves_labels() {
        let mut mgr = SnapshotManager::new();
        let mut m = CrosspointMatrix::new(2, 2);
        m.set_input_label(0, "Source A".to_string()).expect("ok");
        m.connect(0, 0, Some(0.0)).expect("ok");
        let id = mgr.capture(&m, "snap", "", 0);

        let mut m2 = CrosspointMatrix::new(2, 2);
        mgr.restore(id, &mut m2).expect("ok");
        assert_eq!(m2.get_input_label(0), Some("Source A"));
    }

    #[test]
    fn test_active_crosspoints_list() {
        let mut mgr = SnapshotManager::new();
        let m = make_matrix();
        mgr.capture(&m, "s", "", 0);
        let snap = mgr.get(0).expect("exists");
        let active = snap.active_crosspoints();
        assert_eq!(active.len(), 3);
    }
}
