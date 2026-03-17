//! Operational transformation log for media editing.
//!
//! Provides a complete OT (Operational Transformation) system: operation types,
//! a persistent log with DAG ancestry, transform/rebase primitives, and a
//! `Vec<f32>` state machine to which operations can be applied.

use crate::{CollabError, Result};
use std::collections::{HashMap, HashSet, VecDeque};

// ─────────────────────────────────────────────────────────────────────────────
// Operation types
// ─────────────────────────────────────────────────────────────────────────────

/// Elementary edit that can be applied to a `Vec<f32>` state.
#[derive(Debug, Clone, PartialEq)]
pub enum OpType {
    /// Insert `value` at `index`, shifting later elements right.
    Insert { index: usize, value: f32 },
    /// Remove the element at `index`, shifting later elements left.
    Delete { index: usize },
    /// Replace the element at `index`; records old and new values for
    /// invertibility.
    Update {
        index: usize,
        old: f32,
        new_value: f32,
    },
    /// Move the element at `from` to position `to` (index after removal).
    Move { from: usize, to: usize },
    /// A batch of operations applied as an atomic unit.
    Composite(Vec<OpType>),
}

/// A concrete, authored operation carrying identity and causal context.
#[derive(Debug, Clone, PartialEq)]
pub struct Operation {
    /// Globally unique id for this operation.
    pub id: u64,
    /// User who authored this operation.
    pub user_id: u32,
    /// Wall-clock timestamp in milliseconds.
    pub timestamp_ms: i64,
    /// Logical path of the resource being edited.
    pub path: String,
    /// The edit payload.
    pub op_type: OpType,
    /// Id of the parent operation in the causal DAG.
    pub parent_id: Option<u64>,
}

impl Operation {
    /// Construct a new operation with `parent_id = None`.
    pub fn new(
        id: u64,
        user_id: u32,
        timestamp_ms: i64,
        path: impl Into<String>,
        op_type: OpType,
    ) -> Self {
        Self {
            id,
            user_id,
            timestamp_ms,
            path: path.into(),
            op_type,
            parent_id: None,
        }
    }

    /// Construct a child operation with an explicit parent.
    pub fn with_parent(
        id: u64,
        user_id: u32,
        timestamp_ms: i64,
        path: impl Into<String>,
        op_type: OpType,
        parent_id: u64,
    ) -> Self {
        Self {
            id,
            user_id,
            timestamp_ms,
            path: path.into(),
            op_type,
            parent_id: Some(parent_id),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// OperationLog
// ─────────────────────────────────────────────────────────────────────────────

/// A sequential log of operations with a pointer to the current head.
#[derive(Debug, Default)]
pub struct OperationLog {
    /// All recorded operations in submission order.
    pub entries: Vec<Operation>,
    /// The id of the last committed operation (0 means empty).
    pub head_id: u64,
}

impl OperationLog {
    /// Create an empty log.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            head_id: 0,
        }
    }

    /// Append an operation to the log, updating `head_id`.
    pub fn push(&mut self, op: Operation) {
        self.head_id = op.id;
        self.entries.push(op);
    }

    /// Look up an operation by id.
    pub fn get(&self, id: u64) -> Option<&Operation> {
        self.entries.iter().find(|o| o.id == id)
    }

    /// Number of operations recorded.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when the log contains no operations.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Operational transformation
// ─────────────────────────────────────────────────────────────────────────────

/// Transform two concurrently submitted operations so that each can be applied
/// after the other while preserving intent.
///
/// Returns `(op1_transformed, op2_transformed)` where:
/// - `op1_transformed` is `op1` adjusted to be applied after `op2`.
/// - `op2_transformed` is `op2` adjusted to be applied after `op1`.
///
/// Only `Insert` and `Delete` index adjustments are currently handled; all
/// other combination preserve their indices unchanged.
pub fn transform(op1: &Operation, op2: &Operation) -> (Operation, Operation) {
    let new_op1_type = transform_type(&op1.op_type, &op2.op_type, true);
    let new_op2_type = transform_type(&op2.op_type, &op1.op_type, false);

    let mut t1 = op1.clone();
    let mut t2 = op2.clone();
    t1.op_type = new_op1_type;
    t2.op_type = new_op2_type;
    (t1, t2)
}

/// Core index-adjustment logic for a pair of `OpType`s.
///
/// `is_first` indicates whether `a` had priority (was "submitted first" in FIFO
/// order).  When both Insert at the same index, the first keeps its index and
/// the second gets index + 1.
fn transform_type(a: &OpType, b: &OpType, is_first: bool) -> OpType {
    match (a, b) {
        // ── Insert vs Insert ─────────────────────────────────────────────────
        (
            OpType::Insert {
                index: ia,
                value: va,
            },
            OpType::Insert { index: ib, .. },
        ) => {
            // FIFO tiebreak: first keeps its index; second shifts right.
            let new_index = if ia < ib || (ia == ib && is_first) {
                *ia
            } else {
                ia + 1
            };
            OpType::Insert {
                index: new_index,
                value: *va,
            }
        }

        // ── Insert vs Delete ─────────────────────────────────────────────────
        // `a` is Insert, `b` is Delete.  If `b` deleted an element before the
        // insertion site the insertion index shifts left.
        (
            OpType::Insert {
                index: ia,
                value: va,
            },
            OpType::Delete { index: ib },
        ) => {
            let new_index = if ib < ia { ia - 1 } else { *ia };
            OpType::Insert {
                index: new_index,
                value: *va,
            }
        }

        // ── Delete vs Insert ─────────────────────────────────────────────────
        // `a` is Delete, `b` is Insert.  If `b` inserts before (or at) the
        // deletion site, the deletion index shifts right.
        (OpType::Delete { index: ia }, OpType::Insert { index: ib, .. }) => {
            let new_index = if ib <= ia { ia + 1 } else { *ia };
            OpType::Delete { index: new_index }
        }

        // ── Delete vs Delete ─────────────────────────────────────────────────
        (OpType::Delete { index: ia }, OpType::Delete { index: ib }) => {
            if ia == ib {
                // Both deleted the same element.  The second becomes a no-op
                // modelled as a Delete at a sentinel index that will never be
                // reached (usize::MAX) — callers must handle this gracefully.
                if is_first {
                    OpType::Delete { index: *ia }
                } else {
                    OpType::Delete { index: usize::MAX }
                }
            } else if ib < ia {
                // `b` removed an element before `a`'s target: shift left.
                OpType::Delete { index: ia - 1 }
            } else {
                OpType::Delete { index: *ia }
            }
        }

        // All other combinations (Update, Move, Composite) are returned
        // unchanged because their transformation semantics are context-specific.
        _ => a.clone(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Apply
// ─────────────────────────────────────────────────────────────────────────────

/// Apply a single `Operation` to a mutable `Vec<f32>` state.
pub fn apply(state: &mut Vec<f32>, op: &Operation) -> Result<()> {
    apply_type(state, &op.op_type)
}

fn apply_type(state: &mut Vec<f32>, op_type: &OpType) -> Result<()> {
    match op_type {
        OpType::Insert { index, value } => {
            if *index > state.len() {
                return Err(CollabError::InvalidOperation(format!(
                    "Insert index {} out of bounds (len {})",
                    index,
                    state.len()
                )));
            }
            state.insert(*index, *value);
            Ok(())
        }

        OpType::Delete { index } => {
            // Sentinel no-op from transform.
            if *index == usize::MAX {
                return Ok(());
            }
            if *index >= state.len() {
                return Err(CollabError::InvalidOperation(format!(
                    "Delete index {} out of bounds (len {})",
                    index,
                    state.len()
                )));
            }
            state.remove(*index);
            Ok(())
        }

        OpType::Update {
            index, new_value, ..
        } => {
            let len = state.len();
            let elem = state.get_mut(*index).ok_or_else(|| {
                CollabError::InvalidOperation(format!(
                    "Update index {} out of bounds (len {})",
                    index, len
                ))
            })?;
            *elem = *new_value;
            Ok(())
        }

        OpType::Move { from, to } => {
            if *from >= state.len() {
                return Err(CollabError::InvalidOperation(format!(
                    "Move from {} out of bounds (len {})",
                    from,
                    state.len()
                )));
            }
            let val = state.remove(*from);
            let insert_at = (*to).min(state.len());
            state.insert(insert_at, val);
            Ok(())
        }

        OpType::Composite(sub_ops) => {
            for sub in sub_ops {
                apply_type(state, sub)?;
            }
            Ok(())
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Rebase
// ─────────────────────────────────────────────────────────────────────────────

/// Rebase a sequence of operations `ops` over a different history `base`.
///
/// Each op in `ops` is transformed against every op in `base` sequentially,
/// similar to `git rebase`.  The result is a new sequence of operations that
/// applies cleanly on top of `base`.
pub fn rebase(ops: &[Operation], base: &[Operation]) -> Vec<Operation> {
    let mut rebased: Vec<Operation> = ops.to_vec();

    for base_op in base {
        rebased = rebased
            .into_iter()
            .map(|op| {
                // Base op was committed first (it has priority).  We call
                // transform(base_op, op) where base_op is "first" and op is
                // "second"; the second result is op adjusted to come after
                // base_op.
                let (_, transformed) = transform(base_op, &op);
                transformed
            })
            .collect();
    }

    rebased
}

// ─────────────────────────────────────────────────────────────────────────────
// OpDag
// ─────────────────────────────────────────────────────────────────────────────

/// A directed acyclic graph of operations linked by parent→child edges.
///
/// `edges` maps each parent op id to the set of its direct children.
#[derive(Debug, Default)]
pub struct OpDag {
    /// Parent id → list of child ids.
    pub edges: HashMap<u64, Vec<u64>>,
    /// Set of all node ids (roots included).
    nodes: HashSet<u64>,
}

impl OpDag {
    /// Create an empty DAG.
    pub fn new() -> Self {
        Self {
            edges: HashMap::new(),
            nodes: HashSet::new(),
        }
    }

    /// Insert an operation into the DAG, wiring its `parent_id` edge.
    pub fn insert(&mut self, op: &Operation) {
        self.nodes.insert(op.id);
        if let Some(parent) = op.parent_id {
            self.nodes.insert(parent);
            self.edges.entry(parent).or_default().push(op.id);
        }
    }

    /// Build a DAG from an `OperationLog`.
    pub fn from_log(log: &OperationLog) -> Self {
        let mut dag = Self::new();
        for op in &log.entries {
            dag.insert(op);
        }
        dag
    }

    /// Topological ordering of all nodes in the DAG (Kahn's algorithm).
    ///
    /// Nodes with no incoming edges come first.  The returned order is
    /// deterministic for a given graph (ties broken by ascending id).
    pub fn topological_order(&self) -> Vec<u64> {
        // Build in-degree map.
        let mut in_degree: HashMap<u64, usize> = self.nodes.iter().map(|&n| (n, 0)).collect();
        for children in self.edges.values() {
            for &child in children {
                *in_degree.entry(child).or_insert(0) += 1;
            }
        }

        // Queue of nodes with no remaining dependencies (min-heap via BTreeSet).
        let mut ready: std::collections::BTreeSet<u64> = in_degree
            .iter()
            .filter_map(|(&n, &deg)| if deg == 0 { Some(n) } else { None })
            .collect();

        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(&n) = ready.iter().next() {
            ready.remove(&n);
            order.push(n);
            if let Some(children) = self.edges.get(&n) {
                let mut sorted_children = children.clone();
                sorted_children.sort_unstable();
                for child in sorted_children {
                    let deg = in_degree.entry(child).or_insert(0);
                    if *deg > 0 {
                        *deg -= 1;
                    }
                    if *deg == 0 {
                        ready.insert(child);
                    }
                }
            }
        }

        order
    }

    /// Return the causal ancestors of `op_id` in reverse-topological order
    /// (most recent ancestor first).
    ///
    /// Uses BFS walking parent links via a reverse-edge index.
    pub fn causal_order(&self, op_id: u64) -> Vec<u64> {
        // Build reverse (child→parent) map from the edge set.
        let mut parent_of: HashMap<u64, Vec<u64>> = HashMap::new();
        for (&parent, children) in &self.edges {
            for &child in children {
                parent_of.entry(child).or_default().push(parent);
            }
        }

        let mut visited: HashSet<u64> = HashSet::new();
        let mut queue: VecDeque<u64> = VecDeque::new();
        let mut order: Vec<u64> = Vec::new();

        queue.push_back(op_id);
        visited.insert(op_id);

        while let Some(current) = queue.pop_front() {
            order.push(current);
            if let Some(parents) = parent_of.get(&current) {
                let mut sorted = parents.clone();
                sorted.sort_unstable_by(|a, b| b.cmp(a)); // descending id first
                for p in sorted {
                    if visited.insert(p) {
                        queue.push_back(p);
                    }
                }
            }
        }

        order
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn ins(id: u64, index: usize, value: f32) -> Operation {
        Operation::new(id, 1, 0, "test", OpType::Insert { index, value })
    }

    fn del(id: u64, index: usize) -> Operation {
        Operation::new(id, 1, 0, "test", OpType::Delete { index })
    }

    fn upd(id: u64, index: usize, old: f32, new_value: f32) -> Operation {
        Operation::new(
            id,
            1,
            0,
            "test",
            OpType::Update {
                index,
                old,
                new_value,
            },
        )
    }

    // ── transform: Insert vs Insert ──────────────────────────────────────────

    #[test]
    fn test_transform_insert_insert_same_index_first() {
        let op1 = ins(1, 3, 1.0);
        let op2 = ins(2, 3, 2.0);
        let (t1, t2) = transform(&op1, &op2);
        // op1 was first: keeps index 3; op2 shifts to 4
        assert_eq!(
            t1.op_type,
            OpType::Insert {
                index: 3,
                value: 1.0
            }
        );
        assert_eq!(
            t2.op_type,
            OpType::Insert {
                index: 4,
                value: 2.0
            }
        );
    }

    #[test]
    fn test_transform_insert_insert_different_indices() {
        // op1 inserts at index 2 (first); op2 inserts at index 5 (second).
        // After op1 inserts at 2, everything at index ≥ 2 shifts right by 1.
        // So op2 (which targets index 5 ≥ 2) must shift to index 6 when
        // applied after op1.  op1 is unaffected (op2 is after op1's site).
        let op1 = ins(1, 2, 1.0);
        let op2 = ins(2, 5, 2.0);
        let (t1, t2) = transform(&op1, &op2);
        assert_eq!(
            t1.op_type,
            OpType::Insert {
                index: 2,
                value: 1.0
            }
        );
        assert_eq!(
            t2.op_type,
            OpType::Insert {
                index: 6,
                value: 2.0
            }
        );
    }

    #[test]
    fn test_transform_insert_before_delete() {
        // op1 = Insert at 2; op2 = Delete at 5
        // After op2's delete (index 5), op1's insert index (2) is unaffected.
        let op1 = ins(1, 2, 9.9);
        let op2 = del(2, 5);
        let (t1, _t2) = transform(&op1, &op2);
        assert_eq!(
            t1.op_type,
            OpType::Insert {
                index: 2,
                value: 9.9
            }
        );
    }

    #[test]
    fn test_transform_insert_after_delete() {
        // op1 = Insert at 5; op2 = Delete at 2
        // op2 deletes element 2 before op1's insert site: shift left.
        let op1 = ins(1, 5, 9.9);
        let op2 = del(2, 2);
        let (t1, _t2) = transform(&op1, &op2);
        assert_eq!(
            t1.op_type,
            OpType::Insert {
                index: 4,
                value: 9.9
            }
        );
    }

    // ── transform: Delete vs Insert ──────────────────────────────────────────

    #[test]
    fn test_transform_delete_vs_insert_before() {
        // op1 = Delete at 5; op2 = Insert at 3 (before delete site → shift right)
        let op1 = del(1, 5);
        let op2 = ins(2, 3, 1.0);
        let (t1, _t2) = transform(&op1, &op2);
        assert_eq!(t1.op_type, OpType::Delete { index: 6 });
    }

    #[test]
    fn test_transform_delete_vs_insert_after() {
        // op1 = Delete at 2; op2 = Insert at 7 (after delete site → no shift)
        let op1 = del(1, 2);
        let op2 = ins(2, 7, 1.0);
        let (t1, _t2) = transform(&op1, &op2);
        assert_eq!(t1.op_type, OpType::Delete { index: 2 });
    }

    // ── transform: Delete vs Delete ──────────────────────────────────────────

    #[test]
    fn test_transform_delete_delete_same_index() {
        let op1 = del(1, 4);
        let op2 = del(2, 4);
        let (t1, t2) = transform(&op1, &op2);
        // First keeps its index; second becomes no-op sentinel.
        assert_eq!(t1.op_type, OpType::Delete { index: 4 });
        assert_eq!(t2.op_type, OpType::Delete { index: usize::MAX });
    }

    #[test]
    fn test_transform_delete_delete_before() {
        // op1 = Delete at 6; op2 = Delete at 2
        // op2 removes before op1: op1 shifts left.
        let op1 = del(1, 6);
        let op2 = del(2, 2);
        let (t1, _t2) = transform(&op1, &op2);
        assert_eq!(t1.op_type, OpType::Delete { index: 5 });
    }

    // ── apply ────────────────────────────────────────────────────────────────

    #[test]
    fn test_apply_insert() {
        let mut state = vec![1.0, 2.0, 3.0];
        let op = ins(1, 1, 9.9);
        apply(&mut state, &op).expect("apply should succeed");
        assert_eq!(state, vec![1.0, 9.9, 2.0, 3.0]);
    }

    #[test]
    fn test_apply_delete() {
        let mut state = vec![1.0, 2.0, 3.0];
        let op = del(1, 1);
        apply(&mut state, &op).expect("apply should succeed");
        assert_eq!(state, vec![1.0, 3.0]);
    }

    #[test]
    fn test_apply_update() {
        let mut state = vec![1.0, 2.0, 3.0];
        let op = upd(1, 1, 2.0, 99.0);
        apply(&mut state, &op).expect("apply should succeed");
        assert_eq!(state, vec![1.0, 99.0, 3.0]);
    }

    #[test]
    fn test_apply_move() {
        let mut state = vec![10.0, 20.0, 30.0, 40.0];
        let op = Operation::new(1, 1, 0, "t", OpType::Move { from: 0, to: 2 });
        apply(&mut state, &op).expect("apply should succeed");
        // Remove index 0 (10.0), insert at index 2 in the shortened list.
        assert_eq!(state, vec![20.0, 30.0, 10.0, 40.0]);
    }

    #[test]
    fn test_apply_composite() {
        let mut state = vec![1.0, 2.0, 3.0];
        let op = Operation::new(
            1,
            1,
            0,
            "t",
            OpType::Composite(vec![
                OpType::Insert {
                    index: 0,
                    value: 0.0,
                },
                OpType::Delete { index: 2 }, // originally index 1, now 2 after insert
            ]),
        );
        apply(&mut state, &op).expect("apply should succeed");
        assert_eq!(state, vec![0.0, 1.0, 3.0]);
    }

    #[test]
    fn test_apply_insert_out_of_bounds() {
        let mut state = vec![1.0];
        let op = ins(1, 99, 0.0);
        assert!(apply(&mut state, &op).is_err());
    }

    #[test]
    fn test_apply_delete_out_of_bounds() {
        let mut state = vec![1.0];
        let op = del(1, 5);
        assert!(apply(&mut state, &op).is_err());
    }

    #[test]
    fn test_apply_delete_sentinel_noop() {
        let mut state = vec![1.0, 2.0];
        let op = Operation::new(1, 1, 0, "t", OpType::Delete { index: usize::MAX });
        apply(&mut state, &op).expect("sentinel noop should not fail");
        assert_eq!(state, vec![1.0, 2.0]);
    }

    // ── OperationLog ─────────────────────────────────────────────────────────

    #[test]
    fn test_log_push_and_head() {
        let mut log = OperationLog::new();
        assert!(log.is_empty());
        log.push(ins(1, 0, 1.0));
        log.push(ins(2, 1, 2.0));
        assert_eq!(log.head_id, 2);
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn test_log_get() {
        let mut log = OperationLog::new();
        log.push(del(42, 3));
        let found = log.get(42).expect("should find op 42");
        assert_eq!(found.id, 42);
    }

    // ── rebase ───────────────────────────────────────────────────────────────

    #[test]
    fn test_rebase_empty() {
        let ops = vec![ins(1, 0, 1.0)];
        let result = rebase(&ops, &[]);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_rebase_shifts_index() {
        // ops = Insert at 0; base = Insert at 0 (submitted first)
        // After rebasing, our Insert should shift right.
        let ops = vec![ins(2, 0, 2.0)];
        let base = vec![ins(1, 0, 1.0)];
        let rebased = rebase(&ops, &base);
        // Our insert was "second" (is_first=true for base op), so ours shifts to 1.
        match &rebased[0].op_type {
            OpType::Insert { index, .. } => assert_eq!(*index, 1),
            other => panic!("unexpected op type: {:?}", other),
        }
    }

    // ── OpDag ────────────────────────────────────────────────────────────────

    #[test]
    fn test_dag_topological_order_linear() {
        let mut log = OperationLog::new();
        log.push(ins(1, 0, 1.0));
        log.push(Operation::with_parent(
            2,
            1,
            0,
            "t",
            OpType::Delete { index: 0 },
            1,
        ));
        log.push(Operation::with_parent(
            3,
            1,
            0,
            "t",
            OpType::Delete { index: 0 },
            2,
        ));

        let dag = OpDag::from_log(&log);
        let order = dag.topological_order();
        assert_eq!(order, vec![1, 2, 3]);
    }

    #[test]
    fn test_dag_topological_order_branching() {
        // 1 → 2 and 1 → 3 (two children of 1)
        let mut log = OperationLog::new();
        log.push(ins(1, 0, 1.0));
        log.push(Operation::with_parent(
            2,
            1,
            0,
            "t",
            OpType::Delete { index: 0 },
            1,
        ));
        log.push(Operation::with_parent(
            3,
            2,
            0,
            "t",
            OpType::Delete { index: 0 },
            1,
        ));

        let dag = OpDag::from_log(&log);
        let order = dag.topological_order();
        // 1 must come before 2 and 3.
        let pos: HashMap<u64, usize> = order.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&1] < pos[&2]);
        assert!(pos[&1] < pos[&3]);
    }

    #[test]
    fn test_dag_causal_order() {
        let mut log = OperationLog::new();
        log.push(ins(1, 0, 1.0));
        log.push(Operation::with_parent(
            2,
            1,
            0,
            "t",
            OpType::Delete { index: 0 },
            1,
        ));
        log.push(Operation::with_parent(
            3,
            1,
            0,
            "t",
            OpType::Delete { index: 0 },
            2,
        ));

        let dag = OpDag::from_log(&log);
        let ancestors = dag.causal_order(3);
        // Must include 3 and all ancestors: 2, 1.
        assert!(ancestors.contains(&3));
        assert!(ancestors.contains(&2));
        assert!(ancestors.contains(&1));
    }

    #[test]
    fn test_dag_causal_order_root() {
        let mut log = OperationLog::new();
        log.push(ins(1, 0, 1.0));
        let dag = OpDag::from_log(&log);
        let ancestors = dag.causal_order(1);
        assert_eq!(ancestors, vec![1]);
    }
}
