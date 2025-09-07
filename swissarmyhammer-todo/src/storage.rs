//! Storage backend for todo lists
//!
//! This module provides filesystem-based storage for todo lists using YAML format.
//! Todo lists are stored as `.todo.yaml` files in the `.swissarmyhammer/todo/` directory.

use crate::error::{Result, TodoError};
use crate::types::{TodoId, TodoItem, TodoList};
use crate::utils::{get_todo_directory, validate_todo_list_name};
use std::fs;
use std::path::PathBuf;

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

    /// Create a new todo item in the specified list
    pub async fn create_todo_item(
        &self,
        todo_list: &str,
        task: String,
        context: Option<String>,
    ) -> Result<TodoItem> {
        validate_todo_list_name(todo_list)?;

        if task.trim().is_empty() {
            return Err(TodoError::EmptyTask);
        }

        let path = self.get_list_path(todo_list)?;

        // Load existing list or create new one
        let mut list = if path.exists() {
            self.load_todo_list(&path).await?
        } else {
            TodoList::new()
        };

        // Add new item
        let item = list.add_item(task, context);
        let new_item = item.clone();

        // Save the updated list
        self.save_todo_list(&path, &list).await?;

        Ok(new_item)
    }

    /// Get a specific todo item by ID or the next incomplete item
    pub async fn get_todo_item(
        &self,
        todo_list: &str,
        item_identifier: &str,
    ) -> Result<Option<TodoItem>> {
        validate_todo_list_name(todo_list)?;

        let path = self.get_list_path(todo_list)?;

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
    pub async fn mark_todo_complete(&self, todo_list: &str, id: &TodoId) -> Result<()> {
        validate_todo_list_name(todo_list)?;

        let path = self.get_list_path(todo_list)?;

        if !path.exists() {
            return Err(TodoError::TodoListNotFound(todo_list.to_string()));
        }

        let mut list = self.load_todo_list(&path).await?;

        // Find and mark the item complete
        let item = list
            .find_item_mut(id)
            .ok_or_else(|| TodoError::TodoItemNotFound(id.to_string(), todo_list.to_string()))?;

        item.mark_complete();

        // Check if all items are complete
        if list.all_complete() {
            // Delete the file if all tasks are complete
            fs::remove_file(&path).map_err(|e| {
                TodoError::other(format!(
                    "Failed to delete completed todo list '{todo_list}': {e}"
                ))
            })?;
        } else {
            // Save the updated list
            self.save_todo_list(&path, &list).await?;
        }

        Ok(())
    }

    /// Get all todo items from a list
    pub async fn get_todo_list(&self, todo_list: &str) -> Result<Option<TodoList>> {
        validate_todo_list_name(todo_list)?;

        let path = self.get_list_path(todo_list)?;

        if !path.exists() {
            return Ok(None);
        }

        let list = self.load_todo_list(&path).await?;
        Ok(Some(list))
    }

    /// List all todo list names
    pub async fn list_todo_lists(&self) -> Result<Vec<String>> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let entries = fs::read_dir(&self.base_dir)
            .map_err(|e| TodoError::other(format!("Failed to read todo directory: {e}")))?;

        let mut lists = Vec::new();

        for entry in entries {
            let entry = entry.map_err(|e| {
                TodoError::other(format!("Failed to read todo directory entry: {e}"))
            })?;

            if let Some(file_name) = entry.file_name().to_str() {
                if file_name.ends_with(".todo.yaml") {
                    let list_name = file_name.strip_suffix(".todo.yaml").unwrap();
                    lists.push(list_name.to_string());
                }
            }
        }

        lists.sort();
        Ok(lists)
    }

    /// Get the path for a todo list file
    fn get_list_path(&self, todo_list: &str) -> Result<PathBuf> {
        validate_todo_list_name(todo_list)?;
        Ok(self.base_dir.join(format!("{todo_list}.todo.yaml")))
    }

    /// Load a todo list from a YAML file
    async fn load_todo_list(&self, path: &PathBuf) -> Result<TodoList> {
        let content = fs::read_to_string(path).map_err(|e| {
            TodoError::other(format!(
                "Failed to read todo list file '{}': {}",
                path.display(),
                e
            ))
        })?;

        let list: TodoList = serde_yaml::from_str(&content).map_err(|e| {
            TodoError::other(format!(
                "Failed to parse todo list file '{}': {}",
                path.display(),
                e
            ))
        })?;

        Ok(list)
    }

    /// Save a todo list to a YAML file
    async fn save_todo_list(&self, path: &PathBuf, list: &TodoList) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                TodoError::other(format!(
                    "Failed to create todo directory '{}': {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        let content = serde_yaml::to_string(list)
            .map_err(|e| TodoError::other(format!("Failed to serialize todo list: {e}")))?;

        fs::write(path, content).map_err(|e| {
            TodoError::other(format!(
                "Failed to write todo list file '{}': {}",
                path.display(),
                e
            ))
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_todo_item() {
        // Create a temporary directory for todo storage instead of using default
        let temp_dir = tempfile::TempDir::new().unwrap();
        let todo_dir = temp_dir.path().join("todo");
        fs::create_dir_all(&todo_dir).unwrap();
        let storage = TodoStorage::new(todo_dir);

        let test_list = format!("test_create_{}", ulid::Ulid::new());
        let item = storage
            .create_todo_item(
                &test_list,
                "Test task".to_string(),
                Some("Test context".to_string()),
            )
            .await
            .unwrap();

        assert_eq!(item.task, "Test task");
        assert_eq!(item.context, Some("Test context".to_string()));
        assert!(!item.done);
    }

    #[tokio::test]
    async fn test_get_next_todo_item() {
        // Create a temporary directory for todo storage instead of using default
        let temp_dir = tempfile::TempDir::new().unwrap();
        let todo_dir = temp_dir.path().join("todo");
        fs::create_dir_all(&todo_dir).unwrap();
        let storage = TodoStorage::new(todo_dir);

        // Create two items with unique test list name
        let test_list = format!("test_get_next_{}", ulid::Ulid::new());
        let item1 = storage
            .create_todo_item(&test_list, "Task 1".to_string(), None)
            .await
            .unwrap();

        storage
            .create_todo_item(&test_list, "Task 2".to_string(), None)
            .await
            .unwrap();

        // Get next should return first item
        let next = storage
            .get_todo_item(&test_list, "next")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(next.id, item1.id);
        assert_eq!(next.task, "Task 1");
    }

    #[tokio::test]
    async fn test_mark_complete() {
        // Create a temporary directory for todo storage instead of using default
        let temp_dir = tempfile::TempDir::new().unwrap();
        let todo_dir = temp_dir.path().join("todo");
        fs::create_dir_all(&todo_dir).unwrap();
        let storage = TodoStorage::new(todo_dir);

        let test_list = format!("test_mark_complete_{}", ulid::Ulid::new());
        let item = storage
            .create_todo_item(&test_list, "Test task".to_string(), None)
            .await
            .unwrap();

        // Mark complete
        storage
            .mark_todo_complete(&test_list, &item.id)
            .await
            .unwrap();

        // Since all items are complete, the list should be deleted
        let result = storage.get_todo_list(&test_list).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_mark_complete_partial() {
        // Create a temporary directory for todo storage instead of using default
        let temp_dir = tempfile::TempDir::new().unwrap();
        let todo_dir = temp_dir.path().join("todo");
        fs::create_dir_all(&todo_dir).unwrap();
        let storage = TodoStorage::new(todo_dir);

        let test_list = format!("test_mark_partial_{}", ulid::Ulid::new());
        let item1 = storage
            .create_todo_item(&test_list, "Task 1".to_string(), None)
            .await
            .unwrap();

        storage
            .create_todo_item(&test_list, "Task 2".to_string(), None)
            .await
            .unwrap();

        // Mark first item complete
        storage
            .mark_todo_complete(&test_list, &item1.id)
            .await
            .unwrap();

        // List should still exist with one incomplete item
        let list = storage.get_todo_list(&test_list).await.unwrap().unwrap();
        assert_eq!(list.incomplete_count(), 1);
        assert_eq!(list.complete_count(), 1);

        // Next should return the second task
        let next = storage
            .get_todo_item(&test_list, "next")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(next.task, "Task 2");
    }

    #[tokio::test]
    async fn test_get_specific_item() {
        // Create a temporary directory for todo storage instead of using default
        let temp_dir = tempfile::TempDir::new().unwrap();
        let todo_dir = temp_dir.path().join("todo");
        fs::create_dir_all(&todo_dir).unwrap();
        let storage = TodoStorage::new(todo_dir);

        let test_list = format!("test_get_specific_{}", ulid::Ulid::new());
        let item = storage
            .create_todo_item(&test_list, "Test task".to_string(), None)
            .await
            .unwrap();

        // Get specific item by ID
        let retrieved = storage
            .get_todo_item(&test_list, item.id.as_str())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.id, item.id);
        assert_eq!(retrieved.task, "Test task");
    }

    #[tokio::test]
    async fn test_list_todo_lists() {
        // Use a completely isolated temp directory
        let temp_dir = tempfile::TempDir::new().unwrap();
        let storage = TodoStorage::new(temp_dir.path().to_path_buf());

        let list1 = format!("test_list1_{}", ulid::Ulid::new());
        let list2 = format!("test_list2_{}", ulid::Ulid::new());

        storage
            .create_todo_item(&list1, "Task 1".to_string(), None)
            .await
            .unwrap();

        storage
            .create_todo_item(&list2, "Task 2".to_string(), None)
            .await
            .unwrap();

        let lists = storage.list_todo_lists().await.unwrap();
        assert_eq!(lists.len(), 2);
        assert!(lists.contains(&list1));
        assert!(lists.contains(&list2));
    }

    #[tokio::test]
    async fn test_validation_errors() {
        // Create a temporary directory for todo storage instead of using default
        let temp_dir = tempfile::TempDir::new().unwrap();
        let todo_dir = temp_dir.path().join("todo");
        fs::create_dir_all(&todo_dir).unwrap();
        let storage = TodoStorage::new(todo_dir);

        // Empty todo list name
        let result = storage.create_todo_item("", "Task".to_string(), None).await;
        assert!(result.is_err());

        // Empty task
        let result = storage
            .create_todo_item("test_list", "".to_string(), None)
            .await;
        assert!(result.is_err());

        // Invalid todo list name
        let result = storage
            .create_todo_item("invalid/name", "Task".to_string(), None)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_nonexistent_list() {
        // Create a temporary directory for todo storage instead of using default
        let temp_dir = tempfile::TempDir::new().unwrap();
        let todo_dir = temp_dir.path().join("todo");
        fs::create_dir_all(&todo_dir).unwrap();
        let storage = TodoStorage::new(todo_dir);

        // Get from nonexistent list
        let result = storage.get_todo_item("nonexistent", "next").await.unwrap();
        assert!(result.is_none());

        // Mark complete in nonexistent list
        let id = TodoId::new();
        let result = storage.mark_todo_complete("nonexistent", &id).await;
        assert!(result.is_err());
    }
}
