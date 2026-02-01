//! KanbanContext - I/O primitives for kanban storage
//!
//! The context provides access to storage and utilities. No business logic methods,
//! just data access primitives. Commands do all the work.

use crate::error::{KanbanError, Result};
use crate::types::{Board, LogEntry, Task, TaskId};
use fs2::FileExt;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Context passed to every command - provides access, not logic
pub struct KanbanContext {
    /// Path to the .kanban directory
    root: PathBuf,
}

impl KanbanContext {
    /// Create a new context for the given .kanban directory
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Create a context by finding the .kanban directory from a starting path
    pub fn find(start: impl AsRef<Path>) -> Result<Self> {
        let mut current = start.as_ref().to_path_buf();

        loop {
            let kanban_dir = current.join(".kanban");
            if kanban_dir.is_dir() {
                return Ok(Self::new(kanban_dir));
            }

            if !current.pop() {
                return Err(KanbanError::NotInitialized {
                    path: start.as_ref().to_path_buf(),
                });
            }
        }
    }

    // =========================================================================
    // Path helpers
    // =========================================================================

    /// Get the root .kanban directory
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Path to board.json
    pub fn board_path(&self) -> PathBuf {
        self.root.join("board.json")
    }

    /// Path to tasks directory
    pub fn tasks_dir(&self) -> PathBuf {
        self.root.join("tasks")
    }

    /// Path to a task's JSON file
    pub fn task_path(&self, id: &TaskId) -> PathBuf {
        self.root.join("tasks").join(format!("{}.json", id))
    }

    /// Path to a task's log file
    pub fn task_log_path(&self, id: &TaskId) -> PathBuf {
        self.root.join("tasks").join(format!("{}.jsonl", id))
    }

    /// Path to the activity directory
    pub fn activity_dir(&self) -> PathBuf {
        self.root.join("activity")
    }

    /// Path to the current activity log
    pub fn activity_path(&self) -> PathBuf {
        self.root.join("activity").join("current.jsonl")
    }

    /// Path to the lock file
    pub fn lock_path(&self) -> PathBuf {
        self.root.join(".lock")
    }

    // =========================================================================
    // Directory initialization
    // =========================================================================

    /// Check if the board is initialized
    pub fn is_initialized(&self) -> bool {
        self.board_path().exists()
    }

    /// Create the directory structure for a new board
    pub async fn create_directories(&self) -> Result<()> {
        fs::create_dir_all(self.tasks_dir()).await?;
        fs::create_dir_all(self.activity_dir()).await?;
        Ok(())
    }

    // =========================================================================
    // Board I/O
    // =========================================================================

    /// Read the board file
    pub async fn read_board(&self) -> Result<Board> {
        let path = self.board_path();
        if !path.exists() {
            return Err(KanbanError::NotInitialized {
                path: self.root.clone(),
            });
        }

        let content = fs::read_to_string(&path).await?;
        let board: Board = serde_json::from_str(&content)?;
        Ok(board)
    }

    /// Write the board file (atomic write via temp file)
    pub async fn write_board(&self, board: &Board) -> Result<()> {
        let path = self.board_path();
        let content = serde_json::to_string_pretty(board)?;
        atomic_write(&path, content.as_bytes()).await
    }

    // =========================================================================
    // Task I/O
    // =========================================================================

    /// Read a task file
    pub async fn read_task(&self, id: &TaskId) -> Result<Task> {
        let path = self.task_path(id);
        if !path.exists() {
            return Err(KanbanError::TaskNotFound { id: id.to_string() });
        }

        let content = fs::read_to_string(&path).await?;
        let task: Task = serde_json::from_str(&content)?;
        Ok(task)
    }

    /// Write a task file (atomic write via temp file)
    pub async fn write_task(&self, task: &Task) -> Result<()> {
        let path = self.task_path(&task.id);
        let content = serde_json::to_string_pretty(task)?;
        atomic_write(&path, content.as_bytes()).await
    }

    /// Delete a task file and its log
    pub async fn delete_task_file(&self, id: &TaskId) -> Result<()> {
        let task_path = self.task_path(id);
        let log_path = self.task_log_path(id);

        if task_path.exists() {
            fs::remove_file(&task_path).await?;
        }
        if log_path.exists() {
            fs::remove_file(&log_path).await?;
        }

        Ok(())
    }

    /// List all task IDs by reading the tasks directory
    pub async fn list_task_ids(&self) -> Result<Vec<TaskId>> {
        let tasks_dir = self.tasks_dir();
        if !tasks_dir.exists() {
            return Ok(Vec::new());
        }

        let mut ids = Vec::new();
        let mut entries = fs::read_dir(&tasks_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    ids.push(TaskId::from_string(stem));
                }
            }
        }

        Ok(ids)
    }

    /// Read all tasks
    pub async fn read_all_tasks(&self) -> Result<Vec<Task>> {
        let ids = self.list_task_ids().await?;
        let mut tasks = Vec::with_capacity(ids.len());

        for id in ids {
            tasks.push(self.read_task(&id).await?);
        }

        Ok(tasks)
    }

    // =========================================================================
    // Activity logging
    // =========================================================================

    /// Append a log entry to the global activity log
    pub async fn append_activity(&self, entry: &LogEntry) -> Result<()> {
        self.append_log(&self.activity_path(), entry).await
    }

    /// Append a log entry to a task's log
    pub async fn append_task_log(&self, task_id: &TaskId, entry: &LogEntry) -> Result<()> {
        self.append_log(&self.task_log_path(task_id), entry).await
    }

    /// Append a log entry to a JSONL file
    async fn append_log(&self, path: &Path, entry: &LogEntry) -> Result<()> {
        let mut line = serde_json::to_string(entry)?;
        line.push('\n');

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;

        file.write_all(line.as_bytes()).await?;
        file.flush().await?;

        Ok(())
    }

    /// Read activity log entries (from current.jsonl)
    pub async fn read_activity(&self, limit: Option<usize>) -> Result<Vec<LogEntry>> {
        let path = self.activity_path();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&path).await?;
        let mut entries: Vec<LogEntry> = content
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();

        // Reverse to get newest first
        entries.reverse();

        if let Some(limit) = limit {
            entries.truncate(limit);
        }

        Ok(entries)
    }

    // =========================================================================
    // Locking
    // =========================================================================

    /// Try to acquire an exclusive lock (non-blocking)
    pub async fn lock(&self) -> Result<KanbanLock> {
        let lock_path = self.lock_path();

        // Ensure parent directory exists
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&lock_path)?;

        // Non-blocking lock attempt
        match file.try_lock_exclusive() {
            Ok(()) => Ok(KanbanLock {
                file,
                path: lock_path,
            }),
            Err(_) => Err(KanbanError::LockBusy),
        }
    }
}

/// RAII lock guard - releases on drop
pub struct KanbanLock {
    file: std::fs::File,
    #[allow(dead_code)]
    path: PathBuf,
}

impl Drop for KanbanLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// Atomic write via temp file and rename
async fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    // Write to temp file in same directory
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, content).await?;

    // Rename (atomic on same filesystem)
    fs::rename(&temp_path, path).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        std::fs::create_dir_all(&kanban_dir).unwrap();
        let ctx = KanbanContext::new(kanban_dir);
        ctx.create_directories().await.unwrap();
        (temp, ctx)
    }

    #[tokio::test]
    async fn test_paths() {
        let (temp, ctx) = setup().await;
        let root = temp.path().join(".kanban");

        assert_eq!(ctx.root(), root);
        assert_eq!(ctx.board_path(), root.join("board.json"));
        assert_eq!(ctx.tasks_dir(), root.join("tasks"));
    }

    #[tokio::test]
    async fn test_board_io() {
        let (_temp, ctx) = setup().await;

        let board = Board::new("Test Board");
        ctx.write_board(&board).await.unwrap();

        let loaded = ctx.read_board().await.unwrap();
        assert_eq!(loaded.name, "Test Board");
    }

    #[tokio::test]
    async fn test_task_io() {
        use crate::types::{ColumnId, Ordinal, Position};

        let (_temp, ctx) = setup().await;

        // Initialize board first
        let board = Board::new("Test");
        ctx.write_board(&board).await.unwrap();

        let task = Task::new(
            "Test Task",
            Position::new(ColumnId::from_string("todo"), None, Ordinal::first()),
        );
        let task_id = task.id.clone();

        ctx.write_task(&task).await.unwrap();

        let loaded = ctx.read_task(&task_id).await.unwrap();
        assert_eq!(loaded.title, "Test Task");

        // List tasks
        let ids = ctx.list_task_ids().await.unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], task_id);

        // Delete
        ctx.delete_task_file(&task_id).await.unwrap();
        let ids = ctx.list_task_ids().await.unwrap();
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn test_locking() {
        let (_temp, ctx) = setup().await;

        // First lock should succeed
        let lock1 = ctx.lock().await.unwrap();

        // Second lock should fail (busy)
        let result = ctx.lock().await;
        assert!(matches!(result, Err(KanbanError::LockBusy)));

        // After dropping, should be able to lock again
        drop(lock1);
        let _lock2 = ctx.lock().await.unwrap();
    }
}
