use crate::types::{Session, SessionError, SessionId};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Trait for session storage backends that provide persistent storage for sessions.
///
/// This trait defines the interface for storing, loading, and managing sessions
/// across application restarts. Implementations should provide atomic operations
/// and handle concurrent access safely.
///
/// # Examples
///
/// ```rust
/// use llama_agent::storage::{SessionStorage, FileSessionStorage};
/// use std::path::PathBuf;
///
/// let storage = FileSessionStorage::new(PathBuf::from(".llama-sessions"));
/// // Use storage methods to save/load sessions
/// ```
#[async_trait]
pub trait SessionStorage: Send + Sync {
    /// Save a session to persistent storage.
    ///
    /// This method should atomically write the session data to storage,
    /// ensuring data integrity even if the operation is interrupted.
    ///
    /// # Arguments
    /// * `session` - The session to save
    ///
    /// # Returns
    /// * `Ok(())` if the session was saved successfully
    /// * `Err(SessionError)` if the save operation failed
    async fn save_session(&self, session: &Session) -> Result<(), SessionError>;

    /// Load a session from persistent storage.
    ///
    /// # Arguments
    /// * `session_id` - The unique identifier of the session to load
    ///
    /// # Returns
    /// * `Ok(Some(session))` if the session was found and loaded successfully
    /// * `Ok(None)` if no session with the given ID exists
    /// * `Err(SessionError)` if the load operation failed
    async fn load_session(&self, session_id: &SessionId) -> Result<Option<Session>, SessionError>;

    /// Delete a session from persistent storage.
    ///
    /// # Arguments
    /// * `session_id` - The unique identifier of the session to delete
    ///
    /// # Returns
    /// * `Ok(true)` if the session was found and deleted successfully
    /// * `Ok(false)` if no session with the given ID exists
    /// * `Err(SessionError)` if the delete operation failed
    async fn delete_session(&self, session_id: &SessionId) -> Result<bool, SessionError>;

    /// List all session IDs currently stored.
    ///
    /// # Returns
    /// * `Ok(Vec<SessionId>)` containing all stored session IDs
    /// * `Err(SessionError)` if the list operation failed
    async fn list_sessions(&self) -> Result<Vec<SessionId>, SessionError>;

    /// Clean up expired sessions based on TTL (time-to-live).
    ///
    /// Sessions that haven't been updated within the specified TTL hours
    /// will be permanently deleted from storage.
    ///
    /// # Arguments
    /// * `ttl_hours` - Number of hours after which sessions are considered expired.
    ///   Use 0 to disable cleanup.
    ///
    /// # Returns
    /// * `Ok(count)` where count is the number of sessions that were cleaned up
    /// * `Err(SessionError)` if the cleanup operation failed
    async fn cleanup_expired(&self, ttl_hours: u32) -> Result<usize, SessionError>;
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionMetadata {
    session_id: SessionId,
    last_saved: SystemTime,
    message_count: usize,
    token_count: usize,
    file_path: PathBuf,
}

/// File-based implementation of SessionStorage that stores sessions as JSON files.
///
/// This implementation stores each session as a separate JSON file in a designated
/// directory, with an additional metadata.json file for session indexing and cleanup.
/// All write operations are atomic using temporary files and rename operations.
///
/// # File Structure
/// ```
/// .llama-sessions/
/// ├── {session_id_1}.json    # Session data
/// ├── {session_id_2}.json
/// └── metadata.json          # Session metadata for indexing
/// ```
///
/// # Examples
///
/// ```rust
/// use llama_agent::storage::FileSessionStorage;
/// use std::path::PathBuf;
///
/// // Create storage in default directory (.llama-sessions)
/// let storage = FileSessionStorage::default();
///
/// // Create storage in custom directory
/// let storage = FileSessionStorage::new(PathBuf::from("/custom/path"));
/// ```
pub struct FileSessionStorage {
    storage_dir: PathBuf,
    metadata_file: PathBuf,
    metadata_lock: Arc<Mutex<()>>,
}

impl FileSessionStorage {
    /// Create a new FileSessionStorage with the specified storage directory.
    ///
    /// The directory will be created automatically when the first session is saved.
    /// A metadata.json file will be maintained in this directory for session indexing.
    ///
    /// # Arguments
    /// * `storage_dir` - Path to the directory where sessions will be stored
    ///
    /// # Examples
    /// ```rust
    /// use llama_agent::storage::FileSessionStorage;
    /// use std::path::PathBuf;
    ///
    /// let storage = FileSessionStorage::new(PathBuf::from(".llama-sessions"));
    /// ```
    pub fn new(storage_dir: PathBuf) -> Self {
        let metadata_file = storage_dir.join("metadata.json");
        Self {
            storage_dir,
            metadata_file,
            metadata_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Ensure the storage directory exists, creating it if necessary.
    ///
    /// This method is called automatically by save operations, but can be
    /// called explicitly to pre-create the directory structure.
    ///
    /// # Returns
    /// * `Ok(())` if the directory exists or was created successfully
    /// * `Err(SessionError)` if directory creation failed
    pub async fn ensure_storage_dir(&self) -> Result<(), SessionError> {
        if !self.storage_dir.exists() {
            fs::create_dir_all(&self.storage_dir).await.map_err(|e| {
                SessionError::InvalidState(format!(
                    "Failed to create session storage directory {}: {}",
                    self.storage_dir.display(),
                    e
                ))
            })?;
            info!(
                "Created session storage directory: {}",
                self.storage_dir.display()
            );
        }
        Ok(())
    }

    async fn load_metadata(&self) -> Result<HashMap<SessionId, SessionMetadata>, SessionError> {
        if !self.metadata_file.exists() {
            return Ok(HashMap::new());
        }

        let content = fs::read_to_string(&self.metadata_file).await.map_err(|e| {
            SessionError::InvalidState(format!("Failed to read metadata file: {}", e))
        })?;

        let metadata_vec: Vec<SessionMetadata> = serde_json::from_str(&content).map_err(|e| {
            SessionError::InvalidState(format!("Failed to parse metadata file: {}", e))
        })?;

        Ok(metadata_vec
            .into_iter()
            .map(|meta| (meta.session_id, meta))
            .collect())
    }

    async fn save_metadata(
        &self,
        metadata: &HashMap<SessionId, SessionMetadata>,
    ) -> Result<(), SessionError> {
        // Acquire lock to prevent concurrent metadata writes
        let _lock = self.metadata_lock.lock().await;

        self.save_metadata_locked(metadata).await
    }

    /// Internal method to save metadata - must be called with metadata_lock held
    async fn save_metadata_locked(
        &self,
        metadata: &HashMap<SessionId, SessionMetadata>,
    ) -> Result<(), SessionError> {
        self.ensure_storage_dir().await?;

        let metadata_vec: Vec<&SessionMetadata> = metadata.values().collect();
        let json_content = serde_json::to_string_pretty(&metadata_vec).map_err(|e| {
            SessionError::InvalidState(format!("Failed to serialize metadata: {}", e))
        })?;

        // Atomic write using temporary file with unique name in same directory
        // Use ULID which is guaranteed to be unique and sortable
        let temp_file = self
            .storage_dir
            .join(format!("metadata.tmp.{}", ulid::Ulid::new()));

        debug!("Creating temp metadata file: {}", temp_file.display());

        let mut file = fs::File::create(&temp_file).await.map_err(|e| {
            SessionError::InvalidState(format!(
                "Failed to create temp metadata file {} (storage_dir: {}): {}",
                temp_file.display(),
                self.storage_dir.display(),
                e
            ))
        })?;

        file.write_all(json_content.as_bytes())
            .await
            .map_err(|e| SessionError::InvalidState(format!("Failed to write metadata: {}", e)))?;

        file.flush()
            .await
            .map_err(|e| SessionError::InvalidState(format!("Failed to flush metadata: {}", e)))?;

        file.sync_all()
            .await
            .map_err(|e| SessionError::InvalidState(format!("Failed to sync metadata: {}", e)))?;

        drop(file);

        // Verify temp file exists before rename
        if !temp_file.exists() {
            return Err(SessionError::InvalidState(format!(
                "Temp metadata file {} does not exist before rename (storage_dir: {}, metadata_file: {})",
                temp_file.display(),
                self.storage_dir.display(),
                self.metadata_file.display()
            )));
        }

        debug!(
            "Renaming {} to {}",
            temp_file.display(),
            self.metadata_file.display()
        );

        fs::rename(&temp_file, &self.metadata_file)
            .await
            .map_err(|e| {
                SessionError::InvalidState(format!(
                    "Failed to rename metadata file from {} to {} (storage_dir: {}): {}",
                    temp_file.display(),
                    self.metadata_file.display(),
                    self.storage_dir.display(),
                    e
                ))
            })?;

        Ok(())
    }

    fn session_file_path(&self, session_id: &SessionId) -> PathBuf {
        self.storage_dir.join(format!("{}.json", session_id))
    }
}

#[async_trait]
impl SessionStorage for FileSessionStorage {
    async fn save_session(&self, session: &Session) -> Result<(), SessionError> {
        self.ensure_storage_dir().await?;

        let session_file = self.session_file_path(&session.id);
        let session_json = serde_json::to_string_pretty(session).map_err(|e| {
            SessionError::InvalidState(format!("Failed to serialize session: {}", e))
        })?;

        // Atomic write using temporary file
        let temp_file = session_file.with_extension("tmp");
        let mut file = fs::File::create(&temp_file).await.map_err(|e| {
            SessionError::InvalidState(format!("Failed to create temp session file: {}", e))
        })?;

        file.write_all(session_json.as_bytes())
            .await
            .map_err(|e| SessionError::InvalidState(format!("Failed to write session: {}", e)))?;

        file.flush()
            .await
            .map_err(|e| SessionError::InvalidState(format!("Failed to flush session: {}", e)))?;

        drop(file);

        fs::rename(&temp_file, &session_file).await.map_err(|e| {
            SessionError::InvalidState(format!("Failed to rename session file: {}", e))
        })?;

        // Update metadata with lock held
        {
            let _lock = self.metadata_lock.lock().await;
            let mut metadata = self.load_metadata().await?;
            let usage = session.token_usage();
            metadata.insert(
                session.id,
                SessionMetadata {
                    session_id: session.id,
                    last_saved: SystemTime::now(),
                    message_count: session.messages.len(),
                    token_count: usage.total,
                    file_path: session_file,
                },
            );

            // Call save_metadata_locked since we already hold the lock
            self.save_metadata_locked(&metadata).await?;
        }

        debug!("Saved session {} to disk", session.id);
        Ok(())
    }

    async fn load_session(&self, session_id: &SessionId) -> Result<Option<Session>, SessionError> {
        let session_file = self.session_file_path(session_id);

        if !session_file.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&session_file).await.map_err(|e| {
            SessionError::InvalidState(format!("Failed to read session file: {}", e))
        })?;

        let session: Session = serde_json::from_str(&content).map_err(|e| {
            SessionError::InvalidState(format!("Failed to parse session file: {}", e))
        })?;

        debug!("Loaded session {} from disk", session_id);
        Ok(Some(session))
    }

    async fn delete_session(&self, session_id: &SessionId) -> Result<bool, SessionError> {
        let session_file = self.session_file_path(session_id);

        if !session_file.exists() {
            return Ok(false);
        }

        fs::remove_file(&session_file).await.map_err(|e| {
            SessionError::InvalidState(format!("Failed to delete session file: {}", e))
        })?;

        // Update metadata
        let mut metadata = self.load_metadata().await?;
        let removed = metadata.remove(session_id).is_some();
        if removed {
            self.save_metadata(&metadata).await?;
        }

        debug!("Deleted session {} from disk", session_id);
        Ok(true)
    }

    async fn list_sessions(&self) -> Result<Vec<SessionId>, SessionError> {
        let metadata = self.load_metadata().await?;
        Ok(metadata.keys().cloned().collect())
    }

    async fn cleanup_expired(&self, ttl_hours: u32) -> Result<usize, SessionError> {
        if ttl_hours == 0 {
            return Ok(0); // No cleanup when TTL is 0
        }

        let metadata = self.load_metadata().await?;
        let ttl_duration = std::time::Duration::from_secs(ttl_hours as u64 * 3600);
        let now = SystemTime::now();
        let mut cleaned_count = 0;

        let mut updated_metadata = metadata;

        // Find expired sessions
        let expired_sessions: Vec<SessionId> = updated_metadata
            .iter()
            .filter_map(|(session_id, meta)| {
                if let Ok(elapsed) = now.duration_since(meta.last_saved) {
                    if elapsed > ttl_duration {
                        Some(*session_id)
                    } else {
                        None
                    }
                } else {
                    Some(*session_id) // Invalid timestamp, remove it
                }
            })
            .collect();

        // Remove expired sessions
        for session_id in expired_sessions {
            if self.delete_session(&session_id).await? {
                updated_metadata.remove(&session_id);
                cleaned_count += 1;
            }
        }

        if cleaned_count > 0 {
            self.save_metadata(&updated_metadata).await?;
            info!("Cleaned up {} expired sessions", cleaned_count);
        }

        Ok(cleaned_count)
    }
}

impl Default for FileSessionStorage {
    fn default() -> Self {
        Self::new(PathBuf::from(".llama-sessions"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Message, MessageRole};
    use tempfile::TempDir;

    fn create_test_session() -> Session {
        Session {
            cwd: std::path::PathBuf::from("/tmp"),
            id: SessionId::new(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "Hello, world!".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            }],
            mcp_servers: Vec::new(),
            available_tools: Vec::new(),
            available_prompts: Vec::new(),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            compaction_history: Vec::new(),
            transcript_path: None,
            context_state: None,

            todos: Vec::new(),

            available_commands: Vec::new(),
            current_mode: None,

            client_capabilities: None,
            cached_message_count: 0,
            cached_token_count: 0,
        }
    }

    #[tokio::test]
    async fn test_file_session_storage_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileSessionStorage::new(temp_dir.path().to_path_buf());

        let session = create_test_session();
        let session_id = session.id;

        // Save session
        storage.save_session(&session).await.unwrap();

        // Load session
        let loaded_session = storage.load_session(&session_id).await.unwrap();
        assert!(loaded_session.is_some());

        let loaded = loaded_session.unwrap();
        assert_eq!(loaded.id, session_id);
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].content, "Hello, world!");
    }

    #[tokio::test]
    async fn test_file_session_storage_delete() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileSessionStorage::new(temp_dir.path().to_path_buf());

        let session = create_test_session();
        let session_id = session.id;

        // Save and delete
        storage.save_session(&session).await.unwrap();
        let deleted = storage.delete_session(&session_id).await.unwrap();
        assert!(deleted);

        // Try to load deleted session
        let loaded = storage.load_session(&session_id).await.unwrap();
        assert!(loaded.is_none());

        // Try to delete again
        let deleted_again = storage.delete_session(&session_id).await.unwrap();
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn test_file_session_storage_list_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileSessionStorage::new(temp_dir.path().to_path_buf());

        let session1 = create_test_session();
        let session2 = create_test_session();

        storage.save_session(&session1).await.unwrap();
        storage.save_session(&session2).await.unwrap();

        let sessions = storage.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains(&session1.id));
        assert!(sessions.contains(&session2.id));
    }

    #[tokio::test]
    async fn test_file_session_storage_nonexistent_session() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileSessionStorage::new(temp_dir.path().to_path_buf());

        let session_id = SessionId::new();
        let loaded = storage.load_session(&session_id).await.unwrap();
        assert!(loaded.is_none());
    }
}
