#![allow(dead_code)]
//! Directed acyclic graph (DAG) for render job dependencies.
//!
//! Models complex dependency chains between render jobs — e.g. compositing
//! jobs that depend on completed layer renders, or final output that depends
//! on multiple upstream tasks.

use std::collections::{HashMap, HashSet, VecDeque};

/// Unique identifier for a task within the dependency graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId(String);

impl TaskId {
    /// Creates a new task identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Execution status of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskStatus {
    /// Not yet runnable (dependencies incomplete).
    Pending,
    /// All dependencies satisfied — ready to execute.
    Ready,
    /// Currently executing.
    Running,
    /// Completed successfully.
    Completed,
    /// Failed.
    Failed,
}

/// A node in the dependency graph.
#[derive(Debug, Clone)]
struct TaskNode {
    /// Human-readable label.
    label: String,
    /// Current status.
    status: TaskStatus,
    /// Set of task IDs that this task depends on.
    dependencies: HashSet<TaskId>,
    /// Set of task IDs that depend on this task.
    dependents: HashSet<TaskId>,
}

/// A directed acyclic graph of render job tasks.
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// All task nodes keyed by their ID.
    nodes: HashMap<TaskId, TaskNode>,
}

impl DependencyGraph {
    /// Creates a new empty dependency graph.
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    /// Adds a task to the graph with no dependencies.
    ///
    /// Returns `false` if the task ID already exists.
    pub fn add_task(&mut self, id: TaskId, label: impl Into<String>) -> bool {
        if self.nodes.contains_key(&id) {
            return false;
        }
        self.nodes.insert(
            id,
            TaskNode {
                label: label.into(),
                status: TaskStatus::Pending,
                dependencies: HashSet::new(),
                dependents: HashSet::new(),
            },
        );
        true
    }

    /// Adds a dependency edge: `task` depends on `depends_on`.
    ///
    /// Returns `false` if either task does not exist or the edge would create a cycle.
    pub fn add_dependency(&mut self, task: &TaskId, depends_on: &TaskId) -> bool {
        if !self.nodes.contains_key(task) || !self.nodes.contains_key(depends_on) {
            return false;
        }
        if task == depends_on {
            return false;
        }

        // Cycle check: would adding depends_on -> task create a path from task back to depends_on?
        if self.has_path(task, depends_on) {
            return false;
        }

        self.nodes
            .get_mut(task)
            .expect("invariant: task existence was verified above")
            .dependencies
            .insert(depends_on.clone());
        self.nodes
            .get_mut(depends_on)
            .expect("invariant: depends_on existence was verified above")
            .dependents
            .insert(task.clone());
        true
    }

    /// Checks if there is a path from `from` to `to` in the graph.
    #[must_use]
    fn has_path(&self, from: &TaskId, to: &TaskId) -> bool {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(from.clone());

        while let Some(current) = queue.pop_front() {
            if current == *to {
                return true;
            }
            if visited.insert(current.clone()) {
                if let Some(node) = self.nodes.get(&current) {
                    for dep in &node.dependents {
                        queue.push_back(dep.clone());
                    }
                }
            }
        }
        false
    }

    /// Returns the number of tasks in the graph.
    #[must_use]
    pub fn task_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns `true` if the graph is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns the status of a task.
    #[must_use]
    pub fn status(&self, task: &TaskId) -> Option<TaskStatus> {
        self.nodes.get(task).map(|n| n.status)
    }

    /// Sets the status of a task.
    pub fn set_status(&mut self, task: &TaskId, status: TaskStatus) -> bool {
        if let Some(node) = self.nodes.get_mut(task) {
            node.status = status;
            true
        } else {
            false
        }
    }

    /// Returns the set of tasks that are ready to execute (all deps completed).
    #[must_use]
    pub fn ready_tasks(&self) -> Vec<TaskId> {
        self.nodes
            .iter()
            .filter(|(_, node)| {
                node.status == TaskStatus::Pending
                    && node.dependencies.iter().all(|dep| {
                        self.nodes
                            .get(dep)
                            .is_some_and(|d| d.status == TaskStatus::Completed)
                    })
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Returns the direct dependencies of a task.
    #[must_use]
    pub fn dependencies(&self, task: &TaskId) -> Vec<TaskId> {
        self.nodes
            .get(task)
            .map(|n| n.dependencies.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Returns the direct dependents (downstream tasks) of a task.
    #[must_use]
    pub fn dependents(&self, task: &TaskId) -> Vec<TaskId> {
        self.nodes
            .get(task)
            .map(|n| n.dependents.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Returns a topological ordering of the graph, or `None` if there is a cycle.
    #[must_use]
    pub fn topological_sort(&self) -> Option<Vec<TaskId>> {
        let mut in_degree: HashMap<TaskId, usize> = HashMap::new();
        for (id, node) in &self.nodes {
            in_degree.entry(id.clone()).or_insert(0);
            for dep in &node.dependents {
                *in_degree.entry(dep.clone()).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<TaskId> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(id, _)| id.clone())
            .collect();

        let mut result = Vec::new();
        while let Some(id) = queue.pop_front() {
            result.push(id.clone());
            if let Some(node) = self.nodes.get(&id) {
                for dep in &node.dependents {
                    if let Some(d) = in_degree.get_mut(dep) {
                        *d -= 1;
                        if *d == 0 {
                            queue.push_back(dep.clone());
                        }
                    }
                }
            }
        }

        if result.len() == self.nodes.len() {
            Some(result)
        } else {
            None
        }
    }

    /// Returns all root tasks (tasks with no dependencies).
    #[must_use]
    pub fn root_tasks(&self) -> Vec<TaskId> {
        self.nodes
            .iter()
            .filter(|(_, n)| n.dependencies.is_empty())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Returns all leaf tasks (tasks with no dependents).
    #[must_use]
    pub fn leaf_tasks(&self) -> Vec<TaskId> {
        self.nodes
            .iter()
            .filter(|(_, n)| n.dependents.is_empty())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Counts tasks in each status.
    #[must_use]
    pub fn status_counts(&self) -> HashMap<TaskStatus, usize> {
        let mut counts = HashMap::new();
        for node in self.nodes.values() {
            *counts.entry(node.status).or_insert(0) += 1;
        }
        counts
    }

    /// Returns the critical path length (longest path through the graph in terms of task count).
    #[must_use]
    pub fn critical_path_length(&self) -> usize {
        let topo = match self.topological_sort() {
            Some(t) => t,
            None => return 0,
        };

        let mut longest: HashMap<&TaskId, usize> = HashMap::new();
        let mut max_len = 0usize;

        for id in &topo {
            let deps = self.dependencies(id);
            let max_dep = deps
                .iter()
                .filter_map(|d| longest.get(d))
                .copied()
                .max()
                .unwrap_or(0);
            let length = max_dep + 1;
            longest.insert(id, length);
            if length > max_len {
                max_len = length;
            }
        }

        max_len
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tid(s: &str) -> TaskId {
        TaskId::new(s)
    }

    #[test]
    fn test_task_id() {
        let id = tid("render-1");
        assert_eq!(id.as_str(), "render-1");
    }

    #[test]
    fn test_empty_graph() {
        let g = DependencyGraph::new();
        assert!(g.is_empty());
        assert_eq!(g.task_count(), 0);
    }

    #[test]
    fn test_add_task() {
        let mut g = DependencyGraph::new();
        assert!(g.add_task(tid("a"), "Task A"));
        assert_eq!(g.task_count(), 1);
    }

    #[test]
    fn test_add_duplicate_task() {
        let mut g = DependencyGraph::new();
        g.add_task(tid("a"), "Task A");
        assert!(!g.add_task(tid("a"), "Another A"));
    }

    #[test]
    fn test_add_dependency() {
        let mut g = DependencyGraph::new();
        g.add_task(tid("a"), "A");
        g.add_task(tid("b"), "B");
        assert!(g.add_dependency(&tid("b"), &tid("a")));
    }

    #[test]
    fn test_self_dependency_rejected() {
        let mut g = DependencyGraph::new();
        g.add_task(tid("a"), "A");
        assert!(!g.add_dependency(&tid("a"), &tid("a")));
    }

    #[test]
    fn test_cycle_rejected() {
        let mut g = DependencyGraph::new();
        g.add_task(tid("a"), "A");
        g.add_task(tid("b"), "B");
        g.add_dependency(&tid("b"), &tid("a"));
        // a -> b already, b -> a would create cycle
        assert!(!g.add_dependency(&tid("a"), &tid("b")));
    }

    #[test]
    fn test_ready_tasks_no_deps() {
        let mut g = DependencyGraph::new();
        g.add_task(tid("a"), "A");
        let ready = g.ready_tasks();
        assert_eq!(ready.len(), 1);
    }

    #[test]
    fn test_ready_tasks_with_deps() {
        let mut g = DependencyGraph::new();
        g.add_task(tid("a"), "A");
        g.add_task(tid("b"), "B depends on A");
        g.add_dependency(&tid("b"), &tid("a"));
        // Only a should be ready
        let ready = g.ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], tid("a"));
    }

    #[test]
    fn test_ready_after_completion() {
        let mut g = DependencyGraph::new();
        g.add_task(tid("a"), "A");
        g.add_task(tid("b"), "B");
        g.add_dependency(&tid("b"), &tid("a"));
        g.set_status(&tid("a"), TaskStatus::Completed);
        let ready = g.ready_tasks();
        assert!(ready.contains(&tid("b")));
    }

    #[test]
    fn test_topological_sort() {
        let mut g = DependencyGraph::new();
        g.add_task(tid("a"), "A");
        g.add_task(tid("b"), "B");
        g.add_task(tid("c"), "C");
        g.add_dependency(&tid("b"), &tid("a"));
        g.add_dependency(&tid("c"), &tid("b"));
        let topo = g.topological_sort().expect("should succeed in test");
        let pos_a = topo
            .iter()
            .position(|t| *t == tid("a"))
            .expect("should succeed in test");
        let pos_b = topo
            .iter()
            .position(|t| *t == tid("b"))
            .expect("should succeed in test");
        let pos_c = topo
            .iter()
            .position(|t| *t == tid("c"))
            .expect("should succeed in test");
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn test_root_tasks() {
        let mut g = DependencyGraph::new();
        g.add_task(tid("a"), "A");
        g.add_task(tid("b"), "B");
        g.add_dependency(&tid("b"), &tid("a"));
        let roots = g.root_tasks();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0], tid("a"));
    }

    #[test]
    fn test_leaf_tasks() {
        let mut g = DependencyGraph::new();
        g.add_task(tid("a"), "A");
        g.add_task(tid("b"), "B");
        g.add_dependency(&tid("b"), &tid("a"));
        let leaves = g.leaf_tasks();
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0], tid("b"));
    }

    #[test]
    fn test_critical_path_length() {
        let mut g = DependencyGraph::new();
        g.add_task(tid("a"), "A");
        g.add_task(tid("b"), "B");
        g.add_task(tid("c"), "C");
        g.add_dependency(&tid("b"), &tid("a"));
        g.add_dependency(&tid("c"), &tid("b"));
        assert_eq!(g.critical_path_length(), 3);
    }

    #[test]
    fn test_status_counts() {
        let mut g = DependencyGraph::new();
        g.add_task(tid("a"), "A");
        g.add_task(tid("b"), "B");
        g.set_status(&tid("a"), TaskStatus::Completed);
        let counts = g.status_counts();
        assert_eq!(*counts.get(&TaskStatus::Completed).unwrap_or(&0), 1);
        assert_eq!(*counts.get(&TaskStatus::Pending).unwrap_or(&0), 1);
    }
}
