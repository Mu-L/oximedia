//! Workflow state persistence using `SQLite`.

use crate::error::{Result, WorkflowError};
use crate::task::{Task, TaskId, TaskState};
use crate::workflow::{Workflow, WorkflowId, WorkflowState};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Row};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info};

/// Workflow persistence manager.
pub struct PersistenceManager {
    pool: Arc<Pool<SqliteConnectionManager>>,
}

impl PersistenceManager {
    /// Create a new persistence manager.
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let manager = SqliteConnectionManager::file(db_path);
        let pool = Pool::new(manager).map_err(|e| {
            WorkflowError::Database(rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })?;

        let persistence = Self {
            pool: Arc::new(pool),
        };

        persistence.initialize_schema()?;
        Ok(persistence)
    }

    /// Create an in-memory persistence manager.
    pub fn in_memory() -> Result<Self> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::new(manager).map_err(|e| {
            WorkflowError::Database(rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })?;

        let persistence = Self {
            pool: Arc::new(pool),
        };

        persistence.initialize_schema()?;
        Ok(persistence)
    }

    fn initialize_schema(&self) -> Result<()> {
        let conn = self.pool.get().map_err(|e| {
            WorkflowError::Database(rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })?;

        conn.execute_batch(
            r"
            CREATE TABLE IF NOT EXISTS workflows (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                state TEXT NOT NULL,
                config TEXT NOT NULL,
                metadata TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL,
                name TEXT NOT NULL,
                task_type TEXT NOT NULL,
                state TEXT NOT NULL,
                priority INTEGER NOT NULL,
                retry_policy TEXT NOT NULL,
                timeout_secs INTEGER NOT NULL,
                dependencies TEXT,
                metadata TEXT,
                retry_count INTEGER NOT NULL DEFAULT 0,
                conditions TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS edges (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                workflow_id TEXT NOT NULL,
                from_task TEXT NOT NULL,
                to_task TEXT NOT NULL,
                condition TEXT,
                FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS task_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL,
                workflow_id TEXT NOT NULL,
                status TEXT NOT NULL,
                data TEXT,
                error TEXT,
                duration_ms INTEGER NOT NULL,
                outputs TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
                FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS execution_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                workflow_id TEXT NOT NULL,
                state TEXT NOT NULL,
                started_at TEXT NOT NULL,
                completed_at TEXT,
                duration_ms INTEGER,
                error TEXT,
                FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_tasks_workflow ON tasks(workflow_id);
            CREATE INDEX IF NOT EXISTS idx_tasks_state ON tasks(state);
            CREATE INDEX IF NOT EXISTS idx_edges_workflow ON edges(workflow_id);
            CREATE INDEX IF NOT EXISTS idx_task_results_workflow ON task_results(workflow_id);
            CREATE INDEX IF NOT EXISTS idx_execution_history_workflow ON execution_history(workflow_id);
            ",
        )?;

        info!("Database schema initialized");
        Ok(())
    }

    /// Save a workflow to the database.
    pub fn save_workflow(&self, workflow: &Workflow) -> Result<()> {
        let conn = self.pool.get().map_err(|e| {
            WorkflowError::Database(rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })?;

        let now = Utc::now().to_rfc3339();
        let config_json = serde_json::to_string(&workflow.config)?;
        let metadata_json = serde_json::to_string(&workflow.metadata)?;

        conn.execute(
            "INSERT OR REPLACE INTO workflows (id, name, description, state, config, metadata, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                workflow.id.to_string(),
                workflow.name,
                workflow.description,
                format!("{:?}", workflow.state),
                config_json,
                metadata_json,
                now,
                now,
            ],
        )?;

        // Save tasks
        for task in workflow.tasks.values() {
            self.save_task(workflow.id, task)?;
        }

        // Save edges
        conn.execute(
            "DELETE FROM edges WHERE workflow_id = ?1",
            params![workflow.id.to_string()],
        )?;

        for edge in &workflow.edges {
            conn.execute(
                "INSERT INTO edges (workflow_id, from_task, to_task, condition) VALUES (?1, ?2, ?3, ?4)",
                params![
                    workflow.id.to_string(),
                    edge.from.to_string(),
                    edge.to.to_string(),
                    edge.condition,
                ],
            )?;
        }

        debug!("Saved workflow: {}", workflow.id);
        Ok(())
    }

    /// Load a workflow from the database.
    pub fn load_workflow(&self, workflow_id: WorkflowId) -> Result<Workflow> {
        let conn = self.pool.get().map_err(|e| {
            WorkflowError::Database(rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, name, description, state, config, metadata FROM workflows WHERE id = ?1",
        )?;

        let workflow = stmt.query_row(params![workflow_id.to_string()], |row| {
            let config_json: String = row.get(4)?;
            let metadata_json: String = row.get(5)?;

            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                config_json,
                metadata_json,
            ))
        })?;

        let config = serde_json::from_str(&workflow.4)?;
        let metadata = serde_json::from_str(&workflow.5)?;
        let state = self.parse_workflow_state(&workflow.3);

        let mut result = Workflow {
            id: workflow_id,
            name: workflow.1,
            description: workflow.2,
            tasks: Default::default(),
            edges: Vec::new(),
            config,
            state,
            metadata,
        };

        // Load tasks
        let tasks = self.load_tasks(workflow_id)?;
        for task in tasks {
            result.tasks.insert(task.id, task);
        }

        // Load edges
        result.edges = self.load_edges(workflow_id)?;

        debug!("Loaded workflow: {}", workflow_id);
        Ok(result)
    }

    fn save_task(&self, workflow_id: WorkflowId, task: &Task) -> Result<()> {
        let conn = self.pool.get().map_err(|e| {
            WorkflowError::Database(rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })?;

        let now = Utc::now().to_rfc3339();
        let task_type_json = serde_json::to_string(&task.task_type)?;
        let retry_policy_json = serde_json::to_string(&task.retry)?;
        let dependencies_json = serde_json::to_string(&task.dependencies)?;
        let metadata_json = serde_json::to_string(&task.metadata)?;
        let conditions_json = serde_json::to_string(&task.conditions)?;

        conn.execute(
            "INSERT OR REPLACE INTO tasks
             (id, workflow_id, name, task_type, state, priority, retry_policy, timeout_secs,
              dependencies, metadata, retry_count, conditions, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                task.id.to_string(),
                workflow_id.to_string(),
                task.name,
                task_type_json,
                format!("{:?}", task.state),
                task.priority as i32,
                retry_policy_json,
                task.timeout.as_secs() as i64,
                dependencies_json,
                metadata_json,
                task.retry_count,
                conditions_json,
                now,
                now,
            ],
        )?;

        Ok(())
    }

    fn load_tasks(&self, workflow_id: WorkflowId) -> Result<Vec<Task>> {
        let conn = self.pool.get().map_err(|e| {
            WorkflowError::Database(rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })?;

        let mut stmt = conn.prepare(
            "SELECT id, name, task_type, state, priority, retry_policy, timeout_secs,
                    dependencies, metadata, retry_count, conditions
             FROM tasks WHERE workflow_id = ?1",
        )?;

        let tasks = stmt
            .query_map(params![workflow_id.to_string()], |row| {
                self.task_from_row(row)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tasks)
    }

    fn task_from_row(&self, row: &Row) -> rusqlite::Result<Task> {
        let id_str: String = row.get(0)?;
        let id = uuid::Uuid::parse_str(&id_str)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let task_type_json: String = row.get(2)?;
        let task_type: crate::task::TaskType = serde_json::from_str(&task_type_json)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let state_str: String = row.get(3)?;
        let state = self.parse_task_state(&state_str);

        let priority_int: i32 = row.get(4)?;
        let priority = self.parse_task_priority(priority_int);

        let retry_policy_json: String = row.get(5)?;
        let retry = serde_json::from_str(&retry_policy_json)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let timeout_secs: i64 = row.get(6)?;
        let timeout = std::time::Duration::from_secs(u64::try_from(timeout_secs).unwrap_or(3600));

        let dependencies_json: String = row.get(7)?;
        let dependencies = serde_json::from_str(&dependencies_json)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let metadata_json: String = row.get(8)?;
        let metadata = serde_json::from_str(&metadata_json)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let retry_count: u32 = row.get(9)?;

        let conditions_json: String = row.get(10)?;
        let conditions = serde_json::from_str(&conditions_json)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        Ok(Task {
            id: TaskId::from(id),
            name: row.get(1)?,
            task_type,
            state,
            priority,
            retry,
            timeout,
            dependencies,
            metadata,
            retry_count,
            conditions,
        })
    }

    fn load_edges(&self, workflow_id: WorkflowId) -> Result<Vec<crate::workflow::Edge>> {
        let conn = self.pool.get().map_err(|e| {
            WorkflowError::Database(rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })?;

        let mut stmt =
            conn.prepare("SELECT from_task, to_task, condition FROM edges WHERE workflow_id = ?1")?;

        let edges = stmt
            .query_map(params![workflow_id.to_string()], |row| {
                let from_str: String = row.get(0)?;
                let to_str: String = row.get(1)?;
                let condition: Option<String> = row.get(2)?;

                let from_uuid = uuid::Uuid::parse_str(&from_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                let to_uuid = uuid::Uuid::parse_str(&to_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                Ok(crate::workflow::Edge {
                    from: TaskId::from(from_uuid),
                    to: TaskId::from(to_uuid),
                    condition,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(edges)
    }

    /// List all workflows.
    pub fn list_workflows(&self) -> Result<Vec<WorkflowId>> {
        let conn = self.pool.get().map_err(|e| {
            WorkflowError::Database(rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })?;

        let mut stmt = conn.prepare("SELECT id FROM workflows")?;
        let workflows = stmt
            .query_map([], |row| {
                let id_str: String = row.get(0)?;
                let uuid = uuid::Uuid::parse_str(&id_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                Ok(WorkflowId::from(uuid))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(workflows)
    }

    /// Delete a workflow.
    pub fn delete_workflow(&self, workflow_id: WorkflowId) -> Result<()> {
        let conn = self.pool.get().map_err(|e| {
            WorkflowError::Database(rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
        })?;

        conn.execute(
            "DELETE FROM workflows WHERE id = ?1",
            params![workflow_id.to_string()],
        )?;

        debug!("Deleted workflow: {}", workflow_id);
        Ok(())
    }

    fn parse_workflow_state(&self, state_str: &str) -> WorkflowState {
        match state_str {
            "Created" => WorkflowState::Created,
            "Scheduled" => WorkflowState::Scheduled,
            "Running" => WorkflowState::Running,
            "Paused" => WorkflowState::Paused,
            "Completed" => WorkflowState::Completed,
            "Failed" => WorkflowState::Failed,
            "Cancelled" => WorkflowState::Cancelled,
            _ => WorkflowState::Created,
        }
    }

    fn parse_task_state(&self, state_str: &str) -> TaskState {
        match state_str {
            "Pending" => TaskState::Pending,
            "Queued" => TaskState::Queued,
            "Running" => TaskState::Running,
            "Completed" => TaskState::Completed,
            "Failed" => TaskState::Failed,
            "Cancelled" => TaskState::Cancelled,
            "Waiting" => TaskState::Waiting,
            "Retrying" => TaskState::Retrying,
            "Skipped" => TaskState::Skipped,
            _ => TaskState::Pending,
        }
    }

    fn parse_task_priority(&self, priority: i32) -> crate::task::TaskPriority {
        match priority {
            0 => crate::task::TaskPriority::Low,
            1 => crate::task::TaskPriority::Normal,
            2 => crate::task::TaskPriority::High,
            3 => crate::task::TaskPriority::Critical,
            _ => crate::task::TaskPriority::Normal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::{Task, TaskType};
    use std::time::Duration;

    #[test]
    fn test_persistence_creation() {
        let persistence = PersistenceManager::in_memory();
        assert!(persistence.is_ok());
    }

    #[test]
    fn test_save_and_load_workflow() {
        let persistence = PersistenceManager::in_memory().expect("should succeed in test");
        let mut workflow = Workflow::new("test-workflow");

        let task = Task::new(
            "test-task",
            TaskType::Wait {
                duration: Duration::from_secs(10),
            },
        );
        workflow.add_task(task);

        persistence
            .save_workflow(&workflow)
            .expect("should succeed in test");
        let loaded = persistence
            .load_workflow(workflow.id)
            .expect("should succeed in test");

        assert_eq!(loaded.id, workflow.id);
        assert_eq!(loaded.name, workflow.name);
        assert_eq!(loaded.tasks.len(), 1);
    }

    #[test]
    fn test_list_workflows() {
        let persistence = PersistenceManager::in_memory().expect("should succeed in test");

        let workflow1 = Workflow::new("workflow1");
        let workflow2 = Workflow::new("workflow2");

        persistence
            .save_workflow(&workflow1)
            .expect("should succeed in test");
        persistence
            .save_workflow(&workflow2)
            .expect("should succeed in test");

        let workflows = persistence
            .list_workflows()
            .expect("should succeed in test");
        assert_eq!(workflows.len(), 2);
    }

    #[test]
    fn test_delete_workflow() {
        let persistence = PersistenceManager::in_memory().expect("should succeed in test");
        let workflow = Workflow::new("test-workflow");

        persistence
            .save_workflow(&workflow)
            .expect("should succeed in test");
        assert!(persistence.load_workflow(workflow.id).is_ok());

        persistence
            .delete_workflow(workflow.id)
            .expect("should succeed in test");
        assert!(persistence.load_workflow(workflow.id).is_err());
    }

    #[test]
    fn test_save_workflow_with_edges() {
        let persistence = PersistenceManager::in_memory().expect("should succeed in test");
        let mut workflow = Workflow::new("test-workflow");

        let task1 = Task::new(
            "task1",
            TaskType::Wait {
                duration: Duration::from_secs(1),
            },
        );
        let task2 = Task::new(
            "task2",
            TaskType::Wait {
                duration: Duration::from_secs(1),
            },
        );

        let id1 = workflow.add_task(task1);
        let id2 = workflow.add_task(task2);
        workflow.add_edge(id1, id2).expect("should succeed in test");

        persistence
            .save_workflow(&workflow)
            .expect("should succeed in test");
        let loaded = persistence
            .load_workflow(workflow.id)
            .expect("should succeed in test");

        assert_eq!(loaded.edges.len(), 1);
        assert_eq!(loaded.edges[0].from, id1);
        assert_eq!(loaded.edges[0].to, id2);
    }
}
