//! Todo management handlers for ClaudeAgent
//!
//! This module contains methods for managing todo items within agent sessions,
//! including creation, retrieval, completion marking, and synchronization with
//! persistent storage.

use crate::ClaudeAgent;

impl ClaudeAgent {
    /// Get todo storage for a session
    ///
    /// Creates or retrieves a TodoStorage instance for the given session,
    /// using the session's working directory as the storage location.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    ///
    /// # Returns
    ///
    /// Returns the TodoStorage instance for the session
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The session ID is invalid
    /// - The session does not exist
    /// - The todo storage cannot be created
    pub async fn get_todo_storage(
        &self,
        session_id: &str,
    ) -> crate::Result<swissarmyhammer_todo::TodoStorage> {
        // Get the session to access its working directory
        let session_id_parsed =
            session_id
                .to_string()
                .parse()
                .map_err(|e: crate::session::SessionIdError| {
                    crate::error::AgentError::Session(format!("Invalid session ID: {}", e))
                })?;
        let session = self
            .session_manager
            .get_session(&session_id_parsed)
            .map_err(|_e| {
                crate::error::AgentError::Session(format!("Session not found: {}", session_id))
            })?
            .ok_or_else(|| {
                crate::error::AgentError::Session(format!("Session not found: {}", session_id))
            })?;

        // Create TodoStorage using the session's working directory
        let todo_storage = swissarmyhammer_todo::TodoStorage::new_with_working_dir(session.cwd)
            .map_err(|e| {
                crate::error::AgentError::Internal(format!("Failed to create todo storage: {}", e))
            })?;

        Ok(todo_storage)
    }

    /// Create a new todo item for a session
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `task` - The task description
    /// * `context` - Optional context or implementation notes
    ///
    /// # Returns
    ///
    /// Returns a tuple containing:
    /// - The created TodoItem
    /// - The number of completed items that were garbage collected
    ///
    /// # Errors
    ///
    /// Returns an error if the todo item cannot be created
    pub async fn create_todo(
        &self,
        session_id: &str,
        task: String,
        context: Option<String>,
    ) -> crate::Result<(swissarmyhammer_todo::TodoItem, usize)> {
        let storage = self.get_todo_storage(session_id).await?;
        storage.create_todo_item(task, context).await.map_err(|e| {
            crate::error::AgentError::Internal(format!("Failed to create todo item: {}", e))
        })
    }

    /// Get a specific todo item by ID or the next incomplete item
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `item_identifier` - Either a ULID string or "next" for the next incomplete item
    ///
    /// # Returns
    ///
    /// Returns the todo item if found, or None if not found or no incomplete items exist
    ///
    /// # Errors
    ///
    /// Returns an error if the todo item cannot be retrieved
    pub async fn get_todo_item(
        &self,
        session_id: &str,
        item_identifier: &str,
    ) -> crate::Result<Option<swissarmyhammer_todo::TodoItem>> {
        let storage = self.get_todo_storage(session_id).await?;
        storage.get_todo_item(item_identifier).await.map_err(|e| {
            crate::error::AgentError::Internal(format!("Failed to get todo item: {}", e))
        })
    }

    /// Mark a todo item as complete
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    /// * `id` - The todo item ID
    ///
    /// # Errors
    ///
    /// Returns an error if the todo item cannot be marked as complete
    pub async fn mark_todo_complete(
        &self,
        session_id: &str,
        id: &swissarmyhammer_todo::TodoId,
    ) -> crate::Result<()> {
        let storage = self.get_todo_storage(session_id).await?;
        storage.mark_todo_complete(id).await.map_err(|e| {
            crate::error::AgentError::Internal(format!("Failed to mark todo complete: {}", e))
        })
    }

    /// Get all todo items for a session
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    ///
    /// # Returns
    ///
    /// Returns the complete todo list if it exists, or None if no todos exist
    ///
    /// # Errors
    ///
    /// Returns an error if the todo list cannot be retrieved
    pub async fn get_todo_list(
        &self,
        session_id: &str,
    ) -> crate::Result<Option<swissarmyhammer_todo::TodoList>> {
        let storage = self.get_todo_storage(session_id).await?;
        storage.get_todo_list().await.map_err(|e| {
            crate::error::AgentError::Internal(format!("Failed to get todo list: {}", e))
        })
    }

    /// Sync session todos with TodoStorage
    ///
    /// Loads todos from TodoStorage and updates the session's todos vector with the IDs
    /// of all incomplete todo items. This ensures the session's todo list is in sync
    /// with the persistent storage.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The session does not exist
    /// - The todo storage cannot be accessed
    /// - The session cannot be updated
    pub async fn sync_session_todos(&self, session_id: &str) -> crate::Result<()> {
        // Get the todo list from storage
        let storage = self.get_todo_storage(session_id).await?;
        let todo_list = storage.get_todo_list().await.map_err(|e| {
            crate::error::AgentError::Internal(format!("Failed to get todo list: {}", e))
        })?;

        // Extract incomplete todo IDs (id is TodoId type, use as_str().to_string())
        let todo_ids: Vec<String> = if let Some(list) = todo_list {
            list.todo
                .iter()
                .filter(|item| !item.done)
                .map(|item| item.id.as_str().to_string())
                .collect()
        } else {
            Vec::new()
        };

        // Update the session's todos vector
        let session_id_parsed: crate::session::SessionId =
            session_id
                .to_string()
                .parse()
                .map_err(|e: crate::session::SessionIdError| {
                    crate::error::AgentError::Session(format!("Invalid session ID: {}", e))
                })?;
        self.session_manager
            .update_session(&session_id_parsed, |session| {
                session.todos = todo_ids;
            })?;

        Ok(())
    }
}
