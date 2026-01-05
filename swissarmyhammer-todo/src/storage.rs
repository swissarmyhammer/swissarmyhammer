//! Storage backend for todo lists
//!
//! This module provides filesystem-based storage for todo lists using YAML format.
//! Todo lists are stored as `.todo.yaml` files in the `.swissarmyhammer/todo/` directory.
//!
//! ## Concurrency Safety
//!
//! This module uses file-based locking to ensure safe concurrent access to todo lists.
//! When multiple processes attempt to modify the same todo file simultaneously, they
//! acquire an exclusive lock on a separate `.lock` file to prevent race conditions.

use crate::error::{Result, TodoError};
use crate::types::{TodoId, TodoItem, TodoList};
use crate::utils::get_todo_directory;
use fs2::FileExt;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

/// Storage backend for todo list operations
pub struct TodoStorage {
    /// Base directory for todo files
    base_dir: PathBuf,
}

impl TodoStorage {
    /// Create a new TodoStorage with a custom directory
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Create a new TodoStorage using the default directory
    pub fn new_default() -> Result<Self> {
        let base_dir = get_todo_directory()?;
        Ok(Self::new(base_dir))
    }

    /// Create a new TodoStorage from an explicit working directory
    ///
    /// This creates a TodoStorage that will store todos in
    /// `{working_dir}/.swissarmyhammer/todo/`. This approach avoids
    /// reliance on environment variables or git root detection, making
    /// it ideal for tests and explicit directory control.
    ///
    /// **Important**: This method still requires the working directory to be
    /// within a Git repository. Todo operations are designed to work within
    /// version-controlled projects.
    ///
    /// # Arguments
    ///
    /// * `working_dir` - The base working directory (must be in a git repository)
    ///
    /// # Returns
    ///
    /// A `TodoStorage` instance configured to use the specified directory
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The working directory is not within a Git repository
    /// - The todo directory cannot be created
    pub fn new_with_working_dir(working_dir: PathBuf) -> Result<Self> {
        // Verify we're in a git repository by checking if we can find a git root
        use swissarmyhammer_common::utils::directory_utils::find_git_repository_root_from;

        // Try to find the git root from the working directory
        if find_git_repository_root_from(&working_dir).is_none() {
            return Err(TodoError::other(
                "Todo operations require a Git repository. Please run this command from within a Git repository.".to_string()
            ));
        }

        let todo_dir = working_dir.join(".swissarmyhammer").join("todo");
        fs::create_dir_all(&todo_dir)
            .map_err(|e| TodoError::other(format!("Failed to create todo directory: {e}")))?;
        Ok(Self::new(todo_dir))
    }

    /// Create a new todo item
    ///
    /// Returns the created item and the number of completed items that were garbage collected
    ///
    /// This method acquires an exclusive lock to prevent concurrent modifications.
    pub async fn create_todo_item(
        &self,
        task: String,
        context: Option<String>,
    ) -> Result<(TodoItem, usize)> {
        if task.trim().is_empty() {
            return Err(TodoError::EmptyTask);
        }

        let path = self.get_todo_file_path()?;

        // Acquire exclusive lock for the entire read-modify-write operation
        let _lock = self.acquire_lock(&path)?;

        // Load existing list or create new one
        let mut list = if path.exists() {
            self.load_todo_list(&path).await?
        } else {
            TodoList::new()
        };

        // Garbage collect completed todos before adding new item
        let gc_count = self.gc_completed_todos(&mut list)?;

        // Add new item
        let item = list.add_item(task, context);
        let new_item = item.clone();

        // Save the updated list
        self.save_todo_list(&path, &list).await?;

        // Lock is automatically released when _lock goes out of scope

        Ok((new_item, gc_count))
    }

    /// Get a specific todo item by ID or the next incomplete item
    pub async fn get_todo_item(&self, item_identifier: &str) -> Result<Option<TodoItem>> {
        let path = self.get_todo_file_path()?;

        if !path.exists() {
            return Ok(None);
        }

        let list = self.load_todo_list(&path).await?;

        if item_identifier == "next" {
            Ok(list.get_next_incomplete().cloned())
        } else {
            let id = TodoId::from_string(item_identifier.to_string())?;
            Ok(list.find_item(&id).cloned())
        }
    }

    /// Mark a todo item as complete
    ///
    /// This method acquires an exclusive lock to prevent concurrent modifications.
    pub async fn mark_todo_complete(&self, id: &TodoId) -> Result<()> {
        let path = self.get_todo_file_path()?;

        if !path.exists() {
            return Err(TodoError::TodoListNotFound("todo".to_string()));
        }

        // Acquire exclusive lock for the entire read-modify-write operation
        let _lock = self.acquire_lock(&path)?;

        let mut list = self.load_todo_list(&path).await?;

        // Find and mark the item complete
        let item = list
            .find_item_mut(id)
            .ok_or_else(|| TodoError::TodoItemNotFound(id.to_string(), "todo".to_string()))?;

        item.mark_complete();

        // Check if all items are complete
        if list.all_complete() {
            // Delete the file if all tasks are complete
            fs::remove_file(&path).map_err(|e| {
                TodoError::other(format!("Failed to delete completed todo list: {e}"))
            })?;

            // Also clean up the lock file
            let lock_path = self.get_lock_file_path(&path);
            if lock_path.exists() {
                let _ = fs::remove_file(&lock_path); // Ignore errors cleaning up lock file
            }
        } else {
            // Save the updated list
            self.save_todo_list(&path, &list).await?;
        }

        // Lock is automatically released when _lock goes out of scope

        Ok(())
    }

    /// Get all todo items
    pub async fn get_todo_list(&self) -> Result<Option<TodoList>> {
        let path = self.get_todo_file_path()?;

        if !path.exists() {
            return Ok(None);
        }

        let list = self.load_todo_list(&path).await?;
        Ok(Some(list))
    }

    /// Remove completed todos from the list to prevent accumulation
    ///
    /// This garbage collection method removes all completed todo items,
    /// keeping only incomplete tasks. It's called automatically when
    /// creating new todos to maintain a clean, focused list.
    ///
    /// Returns the number of completed items that were removed.
    fn gc_completed_todos(&self, list: &mut TodoList) -> Result<usize> {
        let original_count = list.todo.len();
        let completed_count = list.complete_count();

        // Remove all completed items
        list.todo.retain(|item| !item.done);

        if completed_count > 0 {
            tracing::debug!(
                "Garbage collected {} completed todo(s), {} remaining",
                completed_count,
                list.todo.len()
            );
        }

        // Verify we removed exactly the completed items
        debug_assert_eq!(
            list.todo.len(),
            original_count - completed_count,
            "Garbage collection should remove exactly the completed items"
        );

        Ok(completed_count)
    }

    /// Get the path for the single todo file
    fn get_todo_file_path(&self) -> Result<PathBuf> {
        Ok(self.base_dir.join("todo.yaml"))
    }

    /// Get the lock file path for a given todo file
    fn get_lock_file_path(&self, todo_path: &Path) -> PathBuf {
        todo_path.with_extension("yaml.lock")
    }

    /// Acquire an exclusive lock on the todo file
    ///
    /// Returns a File handle that holds the lock. The lock is automatically
    /// released when the File handle is dropped (RAII pattern).
    ///
    /// This uses a separate `.lock` file to avoid conflicts with the actual data file.
    fn acquire_lock(&self, todo_path: &Path) -> Result<File> {
        let lock_path = self.get_lock_file_path(todo_path);

        // Ensure parent directory exists for lock file
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| self.fs_error("create todo lock directory", parent, e))?;
        }

        // Create or open the lock file
        let lock_file = File::create(&lock_path)
            .map_err(|e| self.fs_error("create lock file", &lock_path, e))?;

        // Acquire exclusive lock (blocks until available)
        lock_file
            .lock_exclusive()
            .map_err(|e| self.fs_error("acquire lock", &lock_path, e))?;

        tracing::debug!("Acquired lock on {}", lock_path.display());

        Ok(lock_file)
    }

    /// Helper method for filesystem error handling
    fn fs_error(&self, operation: &str, path: &Path, error: std::io::Error) -> TodoError {
        TodoError::other(format!(
            "Failed to {} '{}': {}",
            operation,
            path.display(),
            error
        ))
    }

    /// Load a todo list from a YAML file
    async fn load_todo_list(&self, path: &PathBuf) -> Result<TodoList> {
        let content =
            fs::read_to_string(path).map_err(|e| self.fs_error("read todo list file", path, e))?;

        // Check if the YAML content is missing timestamp fields (old format)
        let has_timestamps = content.contains("created_at:") && content.contains("updated_at:");

        let list: TodoList = serde_yaml::from_str(&content).map_err(|e| {
            TodoError::other(format!(
                "Failed to parse todo list file '{}': {}",
                path.display(),
                e
            ))
        })?;

        // Warn if we loaded an old format file
        if !has_timestamps && !list.todo.is_empty() {
            tracing::warn!(
                "Loaded todo list from '{}' with old format (missing timestamps). \
                Timestamps have been set to current time. The file will be updated \
                with timestamps on next save.",
                path.display()
            );
        }

        Ok(list)
    }

    /// Save a todo list to a YAML file
    async fn save_todo_list(&self, path: &PathBuf, list: &TodoList) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| self.fs_error("create todo directory", parent, e))?;
        }

        let content = serde_yaml::to_string(list)
            .map_err(|e| TodoError::other(format!("Failed to serialize todo list: {e}")))?;

        fs::write(path, content).map_err(|e| self.fs_error("write todo list file", path, e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper function to set up test storage with a temporary directory
    fn setup_test_storage() -> (TodoStorage, tempfile::TempDir) {
        let temp_dir = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(&temp_dir).unwrap();
        let storage = TodoStorage::new(temp_dir.path().to_path_buf());
        (storage, temp_dir)
    }

    /// Helper function to assert todo list counts
    fn assert_todo_counts(list: &TodoList, total: usize, complete: usize, incomplete: usize) {
        assert_eq!(list.todo.len(), total);
        assert_eq!(list.complete_count(), complete);
        assert_eq!(list.incomplete_count(), incomplete);
    }

    /// Helper function to assert timestamp bounds for a todo item
    ///
    /// Verifies that created_at and updated_at are within the expected time range.
    /// If `expect_equal` is true, also verifies that both timestamps are equal.
    fn assert_timestamp_bounds(
        item: &TodoItem,
        before: chrono::DateTime<chrono::Utc>,
        after: chrono::DateTime<chrono::Utc>,
        expect_equal: bool,
    ) {
        // Verify created_at is set and within reasonable bounds
        assert!(item.created_at >= before);
        assert!(item.created_at <= after);

        // Verify updated_at is set and within reasonable bounds
        assert!(item.updated_at >= before);
        assert!(item.updated_at <= after);

        // Optionally verify both timestamps are equal
        if expect_equal {
            assert_eq!(item.created_at, item.updated_at);
        }
    }

    #[tokio::test]
    async fn test_create_todo_item() {
        let (storage, _temp_dir) = setup_test_storage();

        let (item, gc_count) = storage
            .create_todo_item("Test task".to_string(), Some("Test context".to_string()))
            .await
            .unwrap();

        assert_eq!(item.task, "Test task");
        assert_eq!(item.context, Some("Test context".to_string()));
        assert!(!item.done);
        assert_eq!(gc_count, 0);
    }

    #[tokio::test]
    async fn test_todo_item_timestamps_on_creation() {
        use chrono::Utc;

        let (storage, _temp_dir) = setup_test_storage();

        let before = Utc::now();
        let (item, _gc_count) = storage
            .create_todo_item("Test task".to_string(), None)
            .await
            .unwrap();
        let after = Utc::now();

        // Verify timestamps are set correctly at creation
        assert_timestamp_bounds(&item, before, after, true);
    }

    #[tokio::test]
    async fn test_todo_item_timestamps_persist() {
        use chrono::Utc;

        let (storage, _temp_dir) = setup_test_storage();

        // Create an item
        let before = Utc::now();
        let (item, _gc_count) = storage
            .create_todo_item("Test task".to_string(), None)
            .await
            .unwrap();
        let after = Utc::now();

        let original_created_at = item.created_at;
        let original_updated_at = item.updated_at;

        // Retrieve the item by ID
        let retrieved = storage
            .get_todo_item(item.id.as_str())
            .await
            .unwrap()
            .unwrap();

        // Verify timestamps are preserved after persistence and retrieval
        assert_eq!(retrieved.created_at, original_created_at);
        assert_eq!(retrieved.updated_at, original_updated_at);

        // Verify timestamps are within bounds and equal
        assert_timestamp_bounds(&retrieved, before, after, true);
    }

    #[tokio::test]
    async fn test_get_next_todo_item() {
        let (storage, _temp_dir) = setup_test_storage();

        // Create two items
        let (item1, _gc_count) = storage
            .create_todo_item("Task 1".to_string(), None)
            .await
            .unwrap();

        storage
            .create_todo_item("Task 2".to_string(), None)
            .await
            .unwrap();

        // Get next should return first item
        let next = storage.get_todo_item("next").await.unwrap().unwrap();

        assert_eq!(next.id, item1.id);
        assert_eq!(next.task, "Task 1");
    }

    #[tokio::test]
    async fn test_mark_complete() {
        let (storage, _temp_dir) = setup_test_storage();

        let (item, _gc_count) = storage
            .create_todo_item("Test task".to_string(), None)
            .await
            .unwrap();

        // Mark complete
        storage.mark_todo_complete(&item.id).await.unwrap();

        // Since all items are complete, the list should be deleted
        let result = storage.get_todo_list().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_mark_complete_partial() {
        let (storage, _temp_dir) = setup_test_storage();

        let (item1, _gc_count) = storage
            .create_todo_item("Task 1".to_string(), None)
            .await
            .unwrap();

        storage
            .create_todo_item("Task 2".to_string(), None)
            .await
            .unwrap();

        // Mark first item complete
        storage.mark_todo_complete(&item1.id).await.unwrap();

        // List should still exist with one incomplete item
        let list = storage.get_todo_list().await.unwrap().unwrap();
        assert_todo_counts(&list, 2, 1, 1);

        // Next should return the second task
        let next = storage.get_todo_item("next").await.unwrap().unwrap();
        assert_eq!(next.task, "Task 2");
    }

    #[tokio::test]
    async fn test_get_specific_item() {
        let (storage, _temp_dir) = setup_test_storage();

        let (item, _gc_count) = storage
            .create_todo_item("Test task".to_string(), None)
            .await
            .unwrap();

        // Get specific item by ID
        let retrieved = storage
            .get_todo_item(item.id.as_str())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.id, item.id);
        assert_eq!(retrieved.task, "Test task");
    }

    #[tokio::test]
    async fn test_validation_errors() {
        let (storage, _temp_dir) = setup_test_storage();

        // Empty task
        let result = storage.create_todo_item("".to_string(), None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_nonexistent_todo_file() {
        let (storage, _temp_dir) = setup_test_storage();

        // Get from nonexistent todo file
        let result = storage.get_todo_item("next").await.unwrap();
        assert!(result.is_none());

        // Mark complete in nonexistent todo file
        let id = TodoId::new();
        let result = storage.mark_todo_complete(&id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_backward_compatibility_old_yaml_format() {
        use chrono::Utc;

        let (storage, temp_dir) = setup_test_storage();

        // Create an old format YAML file (without timestamps)
        let old_yaml = r#"todo:
- id: 01K68A5EJJ61XP2W1T5VDP2XBC
  task: Check system status
  context: null
  done: false
- id: 01K68A5EJJ61XP2W1T5VDP2XBD
  task: Another task
  context: Some context
  done: true
"#;

        let todo_file = temp_dir.path().join("todo.yaml");
        fs::write(&todo_file, old_yaml).unwrap();

        // Load the old format file
        let before_load = Utc::now();
        let list = storage.load_todo_list(&todo_file).await.unwrap();
        let after_load = Utc::now();

        // Verify the list was loaded successfully
        assert_eq!(list.todo.len(), 2);

        // Verify first item
        assert_eq!(list.todo[0].task, "Check system status");
        assert_eq!(list.todo[0].context, None);
        assert!(!list.todo[0].done);

        // Verify timestamps were added with default values (current time)
        assert_timestamp_bounds(&list.todo[0], before_load, after_load, false);

        // Verify second item
        assert_eq!(list.todo[1].task, "Another task");
        assert_eq!(list.todo[1].context, Some("Some context".to_string()));
        assert!(list.todo[1].done);

        // Verify timestamps were added
        assert_timestamp_bounds(&list.todo[1], before_load, after_load, false);

        // Now save the list and verify it has timestamps in the YAML
        storage.save_todo_list(&todo_file, &list).await.unwrap();

        let new_content = fs::read_to_string(&todo_file).unwrap();
        assert!(new_content.contains("created_at:"));
        assert!(new_content.contains("updated_at:"));
    }

    #[tokio::test]
    async fn test_gc_completed_todos_empty_list() {
        let (storage, _temp_dir) = setup_test_storage();

        let mut list = TodoList::new();

        // GC on empty list should not error
        storage.gc_completed_todos(&mut list).unwrap();

        assert_eq!(list.todo.len(), 0);
    }

    #[tokio::test]
    async fn test_gc_completed_todos_no_completed() {
        let (storage, _temp_dir) = setup_test_storage();

        let mut list = TodoList::new();
        list.add_item("Task 1".to_string(), None);
        list.add_item("Task 2".to_string(), None);
        list.add_item("Task 3".to_string(), None);

        // GC with no completed items should not remove anything
        storage.gc_completed_todos(&mut list).unwrap();

        assert_eq!(list.todo.len(), 3);
        assert_eq!(list.incomplete_count(), 3);
    }

    #[tokio::test]
    async fn test_gc_completed_todos_all_completed() {
        let (storage, _temp_dir) = setup_test_storage();

        let mut list = TodoList::new();
        list.add_item("Task 1".to_string(), None);
        list.add_item("Task 2".to_string(), None);
        list.add_item("Task 3".to_string(), None);

        // Mark all as complete
        for item in &mut list.todo {
            item.mark_complete();
        }

        assert_eq!(list.complete_count(), 3);

        // GC should remove all items
        storage.gc_completed_todos(&mut list).unwrap();

        assert_todo_counts(&list, 0, 0, 0);
    }

    #[tokio::test]
    async fn test_gc_completed_todos_mixed() {
        let (storage, _temp_dir) = setup_test_storage();

        let mut list = TodoList::new();
        list.add_item("Task 1".to_string(), None);
        list.add_item("Task 2".to_string(), None);
        list.add_item("Task 3".to_string(), None);
        list.add_item("Task 4".to_string(), None);
        list.add_item("Task 5".to_string(), None);

        // Mark some as complete (index 1 and 3)
        list.todo[1].mark_complete();
        list.todo[3].mark_complete();

        assert_eq!(list.complete_count(), 2);
        assert_eq!(list.incomplete_count(), 3);

        // Store the tasks of incomplete items
        let incomplete_tasks: Vec<String> = list
            .todo
            .iter()
            .filter(|item| !item.done)
            .map(|item| item.task.clone())
            .collect();

        // GC should remove only completed items
        storage.gc_completed_todos(&mut list).unwrap();

        assert_todo_counts(&list, 3, 0, 3);

        // Verify the remaining tasks are the ones that were incomplete
        let remaining_tasks: Vec<String> = list.todo.iter().map(|item| item.task.clone()).collect();

        assert_eq!(remaining_tasks, incomplete_tasks);
        assert_eq!(remaining_tasks, vec!["Task 1", "Task 3", "Task 5"]);
    }

    #[tokio::test]
    async fn test_gc_completed_todos_called_on_create() {
        let (storage, _temp_dir) = setup_test_storage();

        // Create three items
        let (item1, _gc_count) = storage
            .create_todo_item("Task 1".to_string(), None)
            .await
            .unwrap();
        let (item2, _gc_count) = storage
            .create_todo_item("Task 2".to_string(), None)
            .await
            .unwrap();
        let (item3, _gc_count) = storage
            .create_todo_item("Task 3".to_string(), None)
            .await
            .unwrap();

        // Mark first two as complete
        storage.mark_todo_complete(&item1.id).await.unwrap();
        storage.mark_todo_complete(&item2.id).await.unwrap();

        // Verify we have 1 incomplete and 2 complete
        let list = storage.get_todo_list().await.unwrap().unwrap();
        assert_todo_counts(&list, 3, 2, 1);

        // Create a new item - this should trigger GC
        let (item4, _gc_count) = storage
            .create_todo_item("Task 4".to_string(), None)
            .await
            .unwrap();

        // Reload the list and verify completed items were removed
        let list = storage.get_todo_list().await.unwrap().unwrap();
        assert_todo_counts(&list, 2, 0, 2);

        // Verify the correct items remain
        let remaining_ids: Vec<String> = list.todo.iter().map(|item| item.id.to_string()).collect();

        assert!(
            remaining_ids.contains(&item3.id.to_string()),
            "Task 3 should remain"
        );
        assert!(
            remaining_ids.contains(&item4.id.to_string()),
            "Task 4 should remain"
        );
        assert!(
            !remaining_ids.contains(&item1.id.to_string()),
            "Task 1 should be GC'd"
        );
        assert!(
            !remaining_ids.contains(&item2.id.to_string()),
            "Task 2 should be GC'd"
        );
    }

    #[tokio::test]
    async fn test_gc_preserves_order_of_incomplete_items() {
        let (storage, _temp_dir) = setup_test_storage();

        let mut list = TodoList::new();
        list.add_item("A".to_string(), None);
        list.add_item("B".to_string(), None);
        list.add_item("C".to_string(), None);
        list.add_item("D".to_string(), None);
        list.add_item("E".to_string(), None);

        // Mark B and D as complete
        list.todo[1].mark_complete();
        list.todo[3].mark_complete();

        // GC should preserve order of incomplete items
        storage.gc_completed_todos(&mut list).unwrap();

        assert_eq!(list.todo.len(), 3);
        assert_eq!(list.todo[0].task, "A");
        assert_eq!(list.todo[1].task, "C");
        assert_eq!(list.todo[2].task, "E");
    }

    #[tokio::test]
    async fn test_gc_preserves_timestamps() {
        let (storage, _temp_dir) = setup_test_storage();

        let mut list = TodoList::new();
        list.add_item("Task 1".to_string(), None);
        list.add_item("Task 2".to_string(), None);

        let task1_created_at = list.todo[0].created_at;
        let task1_updated_at = list.todo[0].updated_at;

        // Mark second item as complete
        list.todo[1].mark_complete();

        // GC should not affect timestamps of remaining items
        storage.gc_completed_todos(&mut list).unwrap();

        assert_eq!(list.todo.len(), 1);
        assert_eq!(list.todo[0].created_at, task1_created_at);
        assert_eq!(list.todo[0].updated_at, task1_updated_at);
    }
}
