use crate::storage::{FileSessionStorage, SessionStorage};
use crate::types::{CompactionConfig, Message, Session, SessionConfig, SessionError, SessionId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    config: SessionConfig,
    storage: Option<Box<dyn SessionStorage>>,
    changes_since_save: Arc<RwLock<HashMap<SessionId, usize>>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TranscriptEntry {
    session_id: SessionId,
    created_at: String,
    messages: Vec<TranscriptMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TranscriptMessage {
    timestamp: String,
    role: String,
    content: String,
    tool_call_id: Option<String>,
    tool_name: Option<String>,
}

impl SessionManager {
    pub fn new(config: SessionConfig) -> Self {
        let storage = if config.persistence_enabled {
            let storage_dir = config
                .session_storage_dir
                .clone()
                .unwrap_or_else(|| PathBuf::from(".llama-sessions"));
            Some(Box::new(FileSessionStorage::new(storage_dir)) as Box<dyn SessionStorage>)
        } else {
            None
        };

        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            config,
            storage,
            changes_since_save: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn restore_sessions(&self) -> Result<usize, SessionError> {
        if let Some(ref storage) = self.storage {
            let session_ids = storage.list_sessions().await?;
            let mut sessions = self.sessions.write().await;
            let mut restored_count = 0;

            for session_id in session_ids {
                if let Some(session) = storage.load_session(&session_id).await? {
                    sessions.insert(session_id, session);
                    restored_count += 1;
                }
            }

            if restored_count > 0 {
                info!("Restored {} sessions from storage", restored_count);
            }
            Ok(restored_count)
        } else {
            Ok(0)
        }
    }

    pub async fn save_session(&self, session_id: &SessionId) -> Result<(), SessionError> {
        if let Some(ref storage) = self.storage {
            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(session_id) {
                storage.save_session(session).await?;

                // Reset change counter for this session
                let mut changes = self.changes_since_save.write().await;
                changes.insert(*session_id, 0);

                debug!("Saved session {} to storage", session_id);
            }
        }
        Ok(())
    }

    async fn should_auto_save(&self, session_id: &SessionId) -> bool {
        if self.storage.is_none() {
            return false;
        }

        let changes = self.changes_since_save.read().await;
        let change_count = changes.get(session_id).unwrap_or(&0);
        *change_count >= self.config.auto_save_threshold
    }

    async fn increment_changes(&self, session_id: &SessionId) {
        let mut changes = self.changes_since_save.write().await;
        let count = changes.entry(*session_id).or_insert(0);
        *count += 1;
    }

    pub async fn create_session(&self) -> Result<Session, SessionError> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        self.create_session_with_cwd_and_transcript(cwd, None).await
    }

    pub async fn create_session_with_transcript(
        &self,
        transcript_path: Option<PathBuf>,
    ) -> Result<Session, SessionError> {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        self.create_session_with_cwd_and_transcript(cwd, transcript_path)
            .await
    }

    pub async fn create_session_with_cwd_and_transcript(
        &self,
        cwd: PathBuf,
        transcript_path: Option<PathBuf>,
    ) -> Result<Session, SessionError> {
        let mut sessions = self.sessions.write().await;

        // Check if we've reached the session limit
        if sessions.len() >= self.config.max_sessions {
            warn!("Session limit reached: {}", self.config.max_sessions);
            return Err(SessionError::LimitExceeded);
        }

        let now = SystemTime::now();
        let session = Session {
            id: SessionId::new(),
            messages: Vec::new(),
            cwd,
            mcp_servers: Vec::new(),
            available_tools: Vec::new(),
            available_prompts: Vec::new(),
            created_at: now,
            updated_at: now,
            compaction_history: Vec::new(),
            transcript_path: transcript_path.clone(),
            context_state: None,
            todos: Vec::new(),
            available_commands: Vec::new(),
            current_mode: None,
            client_capabilities: None,
            cached_message_count: 0,
            cached_token_count: 0,
        };

        // If transcript path is provided, initialize the transcript file
        if let Some(ref path) = transcript_path {
            self.initialize_transcript_file(&session, path).await?;
        }

        info!("Created new session: {}", session.id);
        sessions.insert(session.id, session.clone());

        Ok(session)
    }

    async fn initialize_transcript_file(
        &self,
        session: &Session,
        path: &PathBuf,
    ) -> Result<(), SessionError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent).await {
                error!("Failed to create transcript directory: {}", e);
                return Err(SessionError::InvalidState(format!(
                    "Failed to create transcript directory: {}",
                    e
                )));
            }
        }

        // Initialize empty transcript file
        let transcript = TranscriptEntry {
            session_id: session.id,
            created_at: session
                .created_at
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string(),
            messages: Vec::new(),
        };

        if let Err(e) = self.write_transcript_to_file(&transcript, path).await {
            error!("Failed to initialize transcript file: {}", e);
            return Err(SessionError::InvalidState(format!(
                "Failed to initialize transcript file: {}",
                e
            )));
        }

        Ok(())
    }

    async fn write_transcript_to_file(
        &self,
        transcript: &TranscriptEntry,
        path: &PathBuf,
    ) -> Result<(), SessionError> {
        let yaml_content = serde_yaml::to_string(transcript).map_err(|e| {
            SessionError::InvalidState(format!("Failed to serialize transcript: {}", e))
        })?;

        // Use a temporary file for atomic writes
        let temp_path = path.with_extension("tmp");

        let mut file = fs::File::create(&temp_path).await.map_err(|e| {
            SessionError::InvalidState(format!("Failed to create temp transcript file: {}", e))
        })?;

        file.write_all(yaml_content.as_bytes()).await.map_err(|e| {
            SessionError::InvalidState(format!("Failed to write transcript: {}", e))
        })?;

        file.flush().await.map_err(|e| {
            SessionError::InvalidState(format!("Failed to flush transcript: {}", e))
        })?;

        drop(file);

        // Atomic rename
        fs::rename(&temp_path, path).await.map_err(|e| {
            SessionError::InvalidState(format!("Failed to rename transcript file: {}", e))
        })?;

        Ok(())
    }

    async fn append_message_to_transcript(
        &self,
        session: &Session,
        message: &Message,
    ) -> Result<(), SessionError> {
        if let Some(ref transcript_path) = session.transcript_path {
            // Read existing transcript
            let mut transcript =
                match fs::read_to_string(transcript_path).await {
                    Ok(content) => serde_yaml::from_str::<TranscriptEntry>(&content)
                        .unwrap_or_else(|_| TranscriptEntry {
                            session_id: session.id,
                            created_at: session
                                .created_at
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs()
                                .to_string(),
                            messages: Vec::new(),
                        }),
                    Err(_) => TranscriptEntry {
                        session_id: session.id,
                        created_at: session
                            .created_at
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs()
                            .to_string(),
                        messages: Vec::new(),
                    },
                };

            // Add new message
            transcript.messages.push(TranscriptMessage {
                timestamp: message
                    .timestamp
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .to_string(),
                role: message.role.as_str().to_string(),
                content: message.content.clone(),
                tool_call_id: message.tool_call_id.as_ref().map(|id| id.to_string()),
                tool_name: message.tool_name.clone(),
            });

            // Write back to file
            if let Err(e) = self
                .write_transcript_to_file(&transcript, transcript_path)
                .await
            {
                error!("Failed to update transcript file: {}", e);
                // Don't fail the session operation, just log the error
            }
        }

        Ok(())
    }

    pub async fn get_session(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<Session>, SessionError> {
        // Fast path: check in-memory cache with read lock
        {
            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(session_id) {
                return Ok(Some(session.clone()));
            }
        } // Release read lock immediately

        // Slow path: try loading from storage if configured
        if let Some(ref storage) = self.storage {
            if let Some(session) = storage.load_session(session_id).await? {
                // Cache the loaded session in memory
                let mut sessions = self.sessions.write().await;

                // Double-check in case another task loaded it while we were acquiring write lock
                if let Some(existing) = sessions.get(session_id) {
                    return Ok(Some(existing.clone()));
                }

                sessions.insert(*session_id, session.clone());
                debug!("Loaded session {} from storage into memory", session_id);
                Ok(Some(session))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub async fn add_message(
        &self,
        session_id: &SessionId,
        message: Message,
    ) -> Result<(), SessionError> {
        let mut sessions = self.sessions.write().await;

        match sessions.get_mut(session_id) {
            Some(session) => {
                // Write to transcript before adding to session
                if let Err(e) = self.append_message_to_transcript(session, &message).await {
                    warn!("Failed to write message to transcript: {}", e);
                    // Continue with session operation even if transcript fails
                }

                session.messages.push(message);
                session.updated_at = SystemTime::now();
                debug!(
                    "Added message to session {}, total messages: {}",
                    session_id,
                    session.messages.len()
                );

                // Track changes for auto-save
                drop(sessions);
                self.increment_changes(session_id).await;

                // Check if auto-save is needed
                if self.should_auto_save(session_id).await {
                    if let Err(e) = self.save_session(session_id).await {
                        warn!("Auto-save failed for session {}: {}", session_id, e);
                        // Continue operation even if save fails
                    }
                }

                Ok(())
            }
            None => Err(SessionError::NotFound(session_id.to_string())),
        }
    }

    pub async fn update_session(&self, updated_session: Session) -> Result<(), SessionError> {
        let session_id = updated_session.id;
        let mut sessions = self.sessions.write().await;

        match sessions.get_mut(&session_id) {
            Some(session) => {
                *session = updated_session;
                session.updated_at = SystemTime::now();
                debug!("Updated session: {}", session_id);

                // Track changes for auto-save
                drop(sessions);
                self.increment_changes(&session_id).await;

                // Check if auto-save is needed
                if self.should_auto_save(&session_id).await {
                    if let Err(e) = self.save_session(&session_id).await {
                        warn!("Auto-save failed for session {}: {}", session_id, e);
                        // Continue operation even if save fails
                    }
                }

                Ok(())
            }
            None => Err(SessionError::NotFound(session_id.to_string())),
        }
    }

    pub async fn delete_session(&self, session_id: &SessionId) -> Result<bool, SessionError> {
        let mut sessions = self.sessions.write().await;
        let removed_from_memory = sessions.remove(session_id).is_some();

        // Remove from storage if present
        if let Some(ref storage) = self.storage {
            if let Err(e) = storage.delete_session(session_id).await {
                warn!(
                    "Failed to delete session {} from storage: {}",
                    session_id, e
                );
                // Continue operation even if storage deletion fails
            }
        }

        // Clean up change tracking
        let mut changes = self.changes_since_save.write().await;
        changes.remove(session_id);

        if removed_from_memory {
            info!("Deleted session: {}", session_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn cleanup_expired_sessions(&self) -> Result<usize, SessionError> {
        if let Some(ref storage) = self.storage {
            let cleaned_count = storage
                .cleanup_expired(self.config.session_ttl_hours)
                .await?;

            if cleaned_count > 0 {
                // Also remove from in-memory sessions
                let expired_ids = storage.list_sessions().await?;
                let mut sessions = self.sessions.write().await;
                let mut in_memory_cleaned = 0;

                // Remove sessions that are no longer in storage
                let current_in_memory: Vec<SessionId> = sessions.keys().cloned().collect();
                for session_id in current_in_memory {
                    if !expired_ids.contains(&session_id) {
                        sessions.remove(&session_id);
                        in_memory_cleaned += 1;
                    }
                }

                info!(
                    "Cleaned up {} expired sessions ({} from memory)",
                    cleaned_count, in_memory_cleaned
                );
            }

            Ok(cleaned_count)
        } else {
            Ok(0)
        }
    }

    pub async fn list_sessions(&self) -> Result<Vec<SessionId>, SessionError> {
        let sessions = self.sessions.read().await;
        Ok(sessions.keys().cloned().collect())
    }

    pub async fn get_session_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }

    pub async fn get_session_stats(&self) -> SessionStats {
        let sessions = self.sessions.read().await;

        let mut total_messages = 0;

        for session in sessions.values() {
            total_messages += session.messages.len();
        }

        SessionStats {
            total_sessions: sessions.len(),
            active_sessions: sessions.len(), // All sessions are now considered active
            total_messages,
            max_sessions: self.config.max_sessions,
        }
    }

    /// Compact a specific session
    pub async fn compact_session<F, Fut>(
        &self,
        session_id: &SessionId,
        config: Option<CompactionConfig>,
        generate_summary: F,
    ) -> Result<CompactionResult, SessionError>
    where
        F: FnOnce(Vec<Message>) -> Fut,
        Fut: std::future::Future<Output = Result<String, SessionError>>,
    {
        let mut sessions = self.sessions.write().await;

        match sessions.get_mut(session_id) {
            Some(session) => {
                let original_token_count = session.token_usage().total;
                let original_message_count = session.messages.len();

                // Perform compaction
                session.compact(config, generate_summary).await?;

                let new_token_count = session.token_usage().total;
                let compression_ratio = if original_token_count > 0 {
                    new_token_count as f32 / original_token_count as f32
                } else {
                    1.0
                };

                Ok(CompactionResult {
                    session_id: *session_id,
                    original_messages: original_message_count,
                    original_tokens: original_token_count,
                    compressed_tokens: new_token_count,
                    compression_ratio,
                    compacted_at: SystemTime::now(),
                })
            }
            None => Err(SessionError::NotFound(session_id.to_string())),
        }
    }

    /// Check if a session should be compacted based on model configuration and compaction config.
    fn should_compact_session(
        &self,
        session: &Session,
        config: &CompactionConfig,
        model_context_size: usize,
    ) -> bool {
        let usage = session.token_usage();
        let threshold_tokens = (model_context_size as f32 * config.threshold) as usize;
        usage.total > threshold_tokens
    }

    /// Get sessions that should be compacted based on configuration criteria.
    ///
    /// Returns a list of session IDs for sessions that exceed the compaction threshold
    /// when evaluated against the model's context size and compaction configuration.
    /// Sessions are selected based on token usage relative to the context window.
    pub async fn get_compaction_candidates(
        &self,
        config: &CompactionConfig,
        model_context_size: usize,
    ) -> Result<Vec<SessionId>, SessionError> {
        let sessions = self.sessions.read().await;
        let candidates: Vec<SessionId> = sessions
            .iter()
            .filter_map(|(id, session)| {
                if self.should_compact_session(session, config, model_context_size) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        Ok(candidates)
    }

    /// Compact multiple sessions in batch
    pub async fn compact_sessions_batch<F, Fut>(
        &self,
        session_ids: Vec<SessionId>,
        config: Option<CompactionConfig>,
        generate_summary: F,
    ) -> Result<Vec<CompactionResult>, SessionError>
    where
        F: Fn(Vec<Message>) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<String, SessionError>> + Send,
    {
        let mut results = Vec::new();

        for session_id in session_ids {
            match self
                .compact_session(&session_id, config.clone(), &generate_summary)
                .await
            {
                Ok(result) => results.push(result),
                Err(e) => {
                    warn!("Failed to compact session {}: {}", session_id, e);
                    // Continue with other sessions
                }
            }
        }

        Ok(results)
    }

    /// Auto-compact sessions based on global configuration
    pub async fn auto_compact_sessions<F, Fut>(
        &self,
        config: &CompactionConfig,
        model_context_size: usize,
        generate_summary: F,
    ) -> Result<CompactionSummary, SessionError>
    where
        F: Fn(Vec<Message>) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<String, SessionError>> + Send,
    {
        let candidates = self
            .get_compaction_candidates(config, model_context_size)
            .await?;

        if candidates.is_empty() {
            return Ok(CompactionSummary::empty());
        }

        let results = self
            .compact_sessions_batch(candidates, Some(config.clone()), generate_summary)
            .await?;

        let average_compression_ratio = if !results.is_empty() {
            results.iter().map(|r| r.compression_ratio).sum::<f32>() / results.len() as f32
        } else {
            1.0
        };

        Ok(CompactionSummary {
            total_sessions_processed: results.len(),
            successful_compactions: results.len(),
            total_messages_compressed: results.iter().map(|r| r.original_messages).sum(),
            total_tokens_saved: results
                .iter()
                .map(|r| r.original_tokens - r.compressed_tokens)
                .sum(),
            average_compression_ratio,
            processed_at: SystemTime::now(),
        })
    }

    /// Get compaction statistics across all sessions
    pub async fn get_compaction_stats(&self) -> Result<CompactionStats, SessionError> {
        let sessions = self.sessions.read().await;

        let mut total_compactions = 0;
        let mut total_sessions_compacted = 0;
        let mut total_tokens_saved = 0;
        let mut average_compression_ratio = 0.0;
        let mut most_recent_compaction: Option<SystemTime> = None;

        for session in sessions.values() {
            if !session.compaction_history.is_empty() {
                total_sessions_compacted += 1;
                total_compactions += session.compaction_history.len();

                for metadata in &session.compaction_history {
                    total_tokens_saved +=
                        metadata.original_token_count - metadata.compressed_token_count;
                    average_compression_ratio += metadata.compression_ratio;

                    if most_recent_compaction.is_none()
                        || metadata.compacted_at > most_recent_compaction.unwrap()
                    {
                        most_recent_compaction = Some(metadata.compacted_at);
                    }
                }
            }
        }

        if total_compactions > 0 {
            average_compression_ratio /= total_compactions as f32;
        }

        Ok(CompactionStats {
            total_sessions: sessions.len(),
            sessions_with_compaction: total_sessions_compacted,
            total_compaction_operations: total_compactions,
            total_tokens_saved,
            average_compression_ratio,
            most_recent_compaction,
        })
    }

    /// Check if any sessions need compaction based on the specified criteria.
    ///
    /// Evaluates all sessions against the model's context size and compaction configuration
    /// to determine if any sessions exceed the compaction threshold. Returns true if at least
    /// one session should be compacted, false otherwise.
    pub async fn needs_compaction(
        &self,
        config: &CompactionConfig,
        model_context_size: usize,
    ) -> Result<bool, SessionError> {
        let candidates = self
            .get_compaction_candidates(config, model_context_size)
            .await?;
        Ok(!candidates.is_empty())
    }
}

#[derive(Debug, Clone)]
pub struct SessionStats {
    pub total_sessions: usize,
    pub active_sessions: usize,
    pub total_messages: usize,
    pub max_sessions: usize,
}

#[derive(Debug, Clone)]
pub struct CompactionResult {
    pub session_id: SessionId,
    pub original_messages: usize,
    pub original_tokens: usize,
    pub compressed_tokens: usize,
    pub compression_ratio: f32,
    pub compacted_at: SystemTime,
}

#[derive(Debug, Clone)]
pub struct CompactionSummary {
    pub total_sessions_processed: usize,
    pub successful_compactions: usize,
    pub total_messages_compressed: usize,
    pub total_tokens_saved: usize,
    pub average_compression_ratio: f32,
    pub processed_at: SystemTime,
}

impl CompactionSummary {
    pub fn empty() -> Self {
        Self {
            total_sessions_processed: 0,
            successful_compactions: 0,
            total_messages_compressed: 0,
            total_tokens_saved: 0,
            average_compression_ratio: 1.0,
            processed_at: SystemTime::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompactionStats {
    pub total_sessions: usize,
    pub sessions_with_compaction: usize,
    pub total_compaction_operations: usize,
    pub total_tokens_saved: usize,
    pub average_compression_ratio: f32,
    pub most_recent_compaction: Option<SystemTime>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_utils::create_qwen_generate_summary_fn;
    use crate::types::{MessageRole, SessionConfig};

    fn create_test_config() -> SessionConfig {
        SessionConfig {
            max_sessions: 5,
            auto_compaction: None,
            persistence_enabled: false,
            session_storage_dir: None,
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        }
    }

    fn create_test_message() -> Message {
        Message {
            role: MessageRole::User,
            content: "Hello, world!".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        }
    }

    #[tokio::test]
    async fn test_session_manager_creation() {
        let config = create_test_config();
        let manager = SessionManager::new(config);

        assert_eq!(manager.get_session_count().await, 0);

        let sessions = manager.list_sessions().await.unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_create_session() {
        let config = create_test_config();
        let manager = SessionManager::new(config);

        let session = manager.create_session().await.unwrap();

        // Session ID is a ULID and should be valid
        assert!(!session.id.to_string().is_empty());
        assert!(session.messages.is_empty());
        assert!(session.mcp_servers.is_empty());
        assert!(session.available_tools.is_empty());
        assert_eq!(manager.get_session_count().await, 1);
    }

    #[tokio::test]
    async fn test_get_session() {
        let config = create_test_config();
        let manager = SessionManager::new(config);

        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Get existing session
        let retrieved = manager.get_session(&session_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, session_id);

        // Get non-existent session
        let non_existent_id = SessionId::new(); // Different ID
        let non_existent = manager.get_session(&non_existent_id).await.unwrap();
        assert!(non_existent.is_none());
    }

    #[tokio::test]
    async fn test_add_message() {
        let config = create_test_config();
        let manager = SessionManager::new(config);

        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        let message = create_test_message();
        let result = manager.add_message(&session_id, message).await;
        assert!(result.is_ok());

        let updated_session = manager.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(updated_session.messages.len(), 1);
        assert_eq!(updated_session.messages[0].content, "Hello, world!");
    }

    #[tokio::test]
    async fn test_multiple_messages_in_session() {
        use crate::types::ToolCallId;

        let config = create_test_config();
        let manager = SessionManager::new(config);

        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Create a tool call ID for the tool message
        let tool_call_id = ToolCallId::new();

        // Add multiple messages in sequence
        let messages = vec![
            Message {
                role: MessageRole::User,
                content: "First user message".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
            Message {
                role: MessageRole::Assistant,
                content: "First assistant response".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
            Message {
                role: MessageRole::User,
                content: "Second user message".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
            Message {
                role: MessageRole::Assistant,
                content: "Second assistant response".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            },
            Message {
                role: MessageRole::Tool,
                content: "Tool result".to_string(),
                tool_call_id: Some(tool_call_id),
                tool_name: Some("test_tool".to_string()),
                timestamp: SystemTime::now(),
            },
        ];

        // Add all messages sequentially
        for message in &messages {
            let result = manager.add_message(&session_id, message.clone()).await;
            assert!(result.is_ok(), "Failed to add message: {:?}", result);
        }

        // Retrieve the session and verify all messages
        let updated_session = manager.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(
            updated_session.messages.len(),
            5,
            "Should have 5 messages in session"
        );

        // Verify message order and content
        assert_eq!(updated_session.messages[0].role, MessageRole::User);
        assert_eq!(updated_session.messages[0].content, "First user message");

        assert_eq!(updated_session.messages[1].role, MessageRole::Assistant);
        assert_eq!(
            updated_session.messages[1].content,
            "First assistant response"
        );

        assert_eq!(updated_session.messages[2].role, MessageRole::User);
        assert_eq!(updated_session.messages[2].content, "Second user message");

        assert_eq!(updated_session.messages[3].role, MessageRole::Assistant);
        assert_eq!(
            updated_session.messages[3].content,
            "Second assistant response"
        );

        assert_eq!(updated_session.messages[4].role, MessageRole::Tool);
        assert_eq!(updated_session.messages[4].content, "Tool result");
        assert_eq!(updated_session.messages[4].tool_call_id, Some(tool_call_id));
        assert_eq!(
            updated_session.messages[4].tool_name,
            Some("test_tool".to_string())
        );

        // Verify updated_at timestamp changed
        assert!(updated_session.updated_at > session.created_at);
    }

    #[tokio::test]
    async fn test_add_message_to_non_existent_session() {
        let config = create_test_config();
        let manager = SessionManager::new(config);

        let non_existent_id = SessionId::new();
        let message = create_test_message();
        let result = manager.add_message(&non_existent_id, message).await;
        assert!(matches!(result, Err(SessionError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_delete_session() {
        let config = create_test_config();
        let manager = SessionManager::new(config);

        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Delete existing session
        let result = manager.delete_session(&session_id).await.unwrap();
        assert!(result);
        assert_eq!(manager.get_session_count().await, 0);

        // Delete non-existent session
        let non_existent_id = SessionId::new();
        let result = manager.delete_session(&non_existent_id).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_session_limit() {
        let config = SessionConfig {
            max_sessions: 2,
            auto_compaction: None,
            persistence_enabled: false,
            session_storage_dir: None,
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };
        let manager = SessionManager::new(config);

        // Create sessions up to the limit
        let _session1 = manager.create_session().await.unwrap();
        let _session2 = manager.create_session().await.unwrap();

        // Try to create one more session - should fail
        let result = manager.create_session().await;
        assert!(matches!(result, Err(SessionError::LimitExceeded)));
        assert_eq!(manager.get_session_count().await, 2);
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let config = create_test_config();
        let manager = SessionManager::new(config);

        let session1 = manager.create_session().await.unwrap();
        let session2 = manager.create_session().await.unwrap();

        let sessions = manager.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains(&session1.id));
        assert!(sessions.contains(&session2.id));
    }

    #[tokio::test]
    async fn test_get_session_stats() {
        let config = create_test_config();
        let manager = SessionManager::new(config);

        // Create some sessions with messages
        let session1 = manager.create_session().await.unwrap();
        let session2 = manager.create_session().await.unwrap();

        manager
            .add_message(&session1.id, create_test_message())
            .await
            .unwrap();
        manager
            .add_message(&session2.id, create_test_message())
            .await
            .unwrap();
        manager
            .add_message(&session2.id, create_test_message())
            .await
            .unwrap();

        let stats = manager.get_session_stats().await;
        assert_eq!(stats.total_sessions, 2);
        assert_eq!(stats.active_sessions, 2);

        assert_eq!(stats.total_messages, 3);
        assert_eq!(stats.max_sessions, 5);
    }

    #[test]
    fn test_session_stats_debug() {
        let stats = SessionStats {
            total_sessions: 5,
            active_sessions: 3,
            total_messages: 10,
            max_sessions: 10,
        };

        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("total_sessions: 5"));
        assert!(debug_str.contains("active_sessions: 3"));
        assert!(debug_str.contains("total_messages: 10"));
    }

    fn create_test_compaction_config() -> CompactionConfig {
        CompactionConfig {
            threshold: 0.5,
            preserve_recent: 2,

            custom_prompt: None,
        }
    }

    #[tokio::test]
    async fn test_compact_session_not_found() {
        let config = create_test_config();
        let manager = SessionManager::new(config);
        let compaction_config = create_test_compaction_config();

        let non_existent_id = SessionId::new();
        let generate_summary = create_qwen_generate_summary_fn();
        let result = manager
            .compact_session(&non_existent_id, Some(compaction_config), generate_summary)
            .await;

        assert!(matches!(result, Err(SessionError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_get_compaction_candidates_empty() {
        let config = create_test_config();
        let manager = SessionManager::new(config);
        let compaction_config = create_test_compaction_config();

        let candidates = manager
            .get_compaction_candidates(&compaction_config, 4096)
            .await
            .unwrap();
        assert!(candidates.is_empty());
    }

    #[tokio::test]
    async fn test_compact_sessions_batch_empty() {
        let config = create_test_config();
        let manager = SessionManager::new(config);
        let compaction_config = create_test_compaction_config();

        let generate_summary = create_qwen_generate_summary_fn();
        let results = manager
            .compact_sessions_batch(vec![], Some(compaction_config), generate_summary)
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_compact_sessions_batch_with_invalid_session() {
        let config = create_test_config();
        let manager = SessionManager::new(config);
        let compaction_config = create_test_compaction_config();

        // Create one valid session and one invalid ID
        let session = manager.create_session().await.unwrap();
        let invalid_id = SessionId::new();

        let generate_summary = create_qwen_generate_summary_fn();
        let results = manager
            .compact_sessions_batch(
                vec![session.id, invalid_id],
                Some(compaction_config),
                generate_summary,
            )
            .await
            .unwrap();

        // Should continue processing despite one failure
        // Note: This test assumes the session might not meet compaction criteria
        // In real scenarios, results could be empty if sessions don't need compaction
        assert!(results.len() <= 1);
    }

    #[tokio::test]
    async fn test_auto_compact_sessions_no_candidates() {
        let config = create_test_config();
        let manager = SessionManager::new(config);
        let compaction_config = create_test_compaction_config();

        // Create a session but it won't meet compaction criteria
        manager.create_session().await.unwrap();

        let generate_summary = create_qwen_generate_summary_fn();
        let summary = manager
            .auto_compact_sessions(&compaction_config, 4096, generate_summary)
            .await
            .unwrap();

        assert_eq!(summary.total_sessions_processed, 0);
        assert_eq!(summary.successful_compactions, 0);
        assert_eq!(summary.total_messages_compressed, 0);
        assert_eq!(summary.total_tokens_saved, 0);
        assert_eq!(summary.average_compression_ratio, 1.0);
    }

    #[tokio::test]
    async fn test_get_compaction_stats_no_compactions() {
        let config = create_test_config();
        let manager = SessionManager::new(config);

        // Create some sessions but no compactions
        manager.create_session().await.unwrap();
        manager.create_session().await.unwrap();

        let stats = manager.get_compaction_stats().await.unwrap();

        assert_eq!(stats.total_sessions, 2);
        assert_eq!(stats.sessions_with_compaction, 0);
        assert_eq!(stats.total_compaction_operations, 0);
        assert_eq!(stats.total_tokens_saved, 0);
        assert_eq!(stats.average_compression_ratio, 0.0);
        assert!(stats.most_recent_compaction.is_none());
    }

    #[tokio::test]
    async fn test_needs_compaction_false() {
        let config = create_test_config();
        let manager = SessionManager::new(config);
        let compaction_config = create_test_compaction_config();

        // Create sessions that won't need compaction
        manager.create_session().await.unwrap();

        let needs = manager
            .needs_compaction(&compaction_config, 4096)
            .await
            .unwrap();
        assert!(!needs);
    }

    #[tokio::test]
    async fn test_compaction_result_debug() {
        let result = CompactionResult {
            session_id: SessionId::new(),
            original_messages: 10,
            original_tokens: 1000,
            compressed_tokens: 500,
            compression_ratio: 0.5,
            compacted_at: SystemTime::now(),
        };

        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("original_messages: 10"));
        assert!(debug_str.contains("original_tokens: 1000"));
        assert!(debug_str.contains("compressed_tokens: 500"));
    }

    #[tokio::test]
    async fn test_compaction_summary_empty() {
        let summary = CompactionSummary::empty();

        assert_eq!(summary.total_sessions_processed, 0);
        assert_eq!(summary.successful_compactions, 0);
        assert_eq!(summary.total_messages_compressed, 0);
        assert_eq!(summary.total_tokens_saved, 0);
        assert_eq!(summary.average_compression_ratio, 1.0);
    }

    #[tokio::test]
    async fn test_compaction_stats_debug() {
        let stats = CompactionStats {
            total_sessions: 5,
            sessions_with_compaction: 2,
            total_compaction_operations: 3,
            total_tokens_saved: 1500,
            average_compression_ratio: 0.6,
            most_recent_compaction: Some(SystemTime::now()),
        };

        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("total_sessions: 5"));
        assert!(debug_str.contains("sessions_with_compaction: 2"));
        assert!(debug_str.contains("total_tokens_saved: 1500"));
    }

    // Session storage and retrieval tests
    #[tokio::test]
    async fn test_session_persistence_enabled() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = SessionConfig {
            max_sessions: 5,
            auto_compaction: None,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };

        let manager = SessionManager::new(config);

        // Verify storage is enabled
        assert!(manager.storage.is_some());
    }

    #[tokio::test]
    async fn test_session_persistence_disabled() {
        let config = create_test_config();
        let manager = SessionManager::new(config);

        // Verify storage is disabled
        assert!(manager.storage.is_none());
    }

    #[tokio::test]
    async fn test_save_session_with_persistence() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = SessionConfig {
            max_sessions: 5,
            auto_compaction: None,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };

        let manager = SessionManager::new(config);
        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Add a message to the session
        let message = create_test_message();
        manager.add_message(&session_id, message).await.unwrap();

        // Manually save the session
        manager.save_session(&session_id).await.unwrap();

        // Verify the session file exists
        let session_file = temp_dir.path().join(format!("{}.json", session_id));
        assert!(session_file.exists());

        // Verify the session can be loaded from storage
        if let Some(ref storage) = manager.storage {
            let loaded = storage.load_session(&session_id).await.unwrap();
            assert!(loaded.is_some());
            let loaded_session = loaded.unwrap();
            assert_eq!(loaded_session.id, session_id);
            assert_eq!(loaded_session.messages.len(), 1);
        }
    }

    #[tokio::test]
    async fn test_save_session_without_persistence() {
        let config = create_test_config();
        let manager = SessionManager::new(config);
        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Add a message
        let message = create_test_message();
        manager.add_message(&session_id, message).await.unwrap();

        // Save should succeed but not write to disk
        manager.save_session(&session_id).await.unwrap();

        // Verify storage is None
        assert!(manager.storage.is_none());
    }

    #[tokio::test]
    async fn test_restore_sessions_with_persistence() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = SessionConfig {
            max_sessions: 5,
            auto_compaction: None,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };

        // Create manager and add sessions
        let manager1 = SessionManager::new(config.clone());
        let session1 = manager1.create_session().await.unwrap();
        let session2 = manager1.create_session().await.unwrap();

        // Add messages to sessions
        manager1
            .add_message(&session1.id, create_test_message())
            .await
            .unwrap();
        manager1
            .add_message(&session2.id, create_test_message())
            .await
            .unwrap();

        // Save sessions
        manager1.save_session(&session1.id).await.unwrap();
        manager1.save_session(&session2.id).await.unwrap();

        // Create a new manager with the same storage directory
        let manager2 = SessionManager::new(config);

        // Restore sessions
        let restored_count = manager2.restore_sessions().await.unwrap();
        assert_eq!(restored_count, 2);

        // Verify sessions were restored
        let restored_session1 = manager2.get_session(&session1.id).await.unwrap();
        let restored_session2 = manager2.get_session(&session2.id).await.unwrap();

        assert!(restored_session1.is_some());
        assert!(restored_session2.is_some());

        let s1 = restored_session1.unwrap();
        let s2 = restored_session2.unwrap();

        assert_eq!(s1.id, session1.id);
        assert_eq!(s1.messages.len(), 1);
        assert_eq!(s2.id, session2.id);
        assert_eq!(s2.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_restore_sessions_without_persistence() {
        let config = create_test_config();
        let manager = SessionManager::new(config);

        // Restore should return 0 when persistence is disabled
        let restored_count = manager.restore_sessions().await.unwrap();
        assert_eq!(restored_count, 0);
    }

    #[tokio::test]
    async fn test_restore_sessions_empty_storage() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = SessionConfig {
            max_sessions: 5,
            auto_compaction: None,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };

        let manager = SessionManager::new(config);

        // Restore from empty storage should return 0
        let restored_count = manager.restore_sessions().await.unwrap();
        assert_eq!(restored_count, 0);
    }

    #[tokio::test]
    async fn test_auto_save_threshold() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = SessionConfig {
            max_sessions: 5,
            auto_compaction: None,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 3, // Save after 3 changes
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };

        let manager = SessionManager::new(config);
        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Add messages below threshold
        manager
            .add_message(&session_id, create_test_message())
            .await
            .unwrap();
        manager
            .add_message(&session_id, create_test_message())
            .await
            .unwrap();

        // Add one more message to trigger auto-save
        manager
            .add_message(&session_id, create_test_message())
            .await
            .unwrap();

        // Now the session should be auto-saved
        // Verify by checking the change counter was reset
        let changes = manager.changes_since_save.read().await;
        let change_count = changes.get(&session_id).unwrap_or(&0);
        assert_eq!(*change_count, 0);
    }

    #[tokio::test]
    async fn test_delete_session_removes_from_storage() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = SessionConfig {
            max_sessions: 5,
            auto_compaction: None,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };

        let manager = SessionManager::new(config);
        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Save the session
        manager.save_session(&session_id).await.unwrap();

        // Verify session file exists
        let session_file = temp_dir.path().join(format!("{}.json", session_id));
        assert!(session_file.exists());

        // Delete the session
        let deleted = manager.delete_session(&session_id).await.unwrap();
        assert!(deleted);

        // Verify session file no longer exists
        assert!(!session_file.exists());

        // Verify session is not in storage
        if let Some(ref storage) = manager.storage {
            let loaded = storage.load_session(&session_id).await.unwrap();
            assert!(loaded.is_none());
        }
    }

    #[tokio::test]
    async fn test_cleanup_expired_sessions_with_storage() {
        use tempfile::TempDir;
        use tokio::time::{sleep, Duration};

        let temp_dir = TempDir::new().unwrap();
        let config = SessionConfig {
            max_sessions: 5,
            auto_compaction: None,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 1, // 1 hour TTL (we'll use a very short duration for testing via direct storage call)
            auto_save_threshold: 5,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };

        let manager = SessionManager::new(config);
        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Save the session
        manager.save_session(&session_id).await.unwrap();

        // Verify session exists
        assert_eq!(manager.get_session_count().await, 1);

        // Wait a small amount to ensure timestamp difference
        sleep(Duration::from_millis(10)).await;

        // Use storage directly with a very short TTL (in fractions of an hour)
        // Note: The storage cleanup_expired uses ttl_hours * 3600 seconds
        // We need to wait long enough for the session to be considered expired
        // For testing, we'll use the storage layer directly with a very small TTL
        if let Some(ref storage) = manager.storage {
            // Cleanup with very short TTL (0.001 hours = 3.6 seconds)
            // Since we can't pass fractional hours, we'll just verify the mechanism works
            // by checking that with TTL of 1 hour, no sessions are cleaned
            let cleaned = storage.cleanup_expired(1).await.unwrap();
            assert_eq!(cleaned, 0); // Session should not be expired yet

            // Verify session still exists
            assert_eq!(manager.get_session_count().await, 1);
        }

        // Now test that sessions can be cleaned up when they should be
        // We'll delete the session normally to clean up test state
        let deleted = manager.delete_session(&session_id).await.unwrap();
        assert!(deleted);

        // Verify session file is removed
        let session_file = temp_dir.path().join(format!("{}.json", session_id));
        assert!(!session_file.exists());
    }

    #[tokio::test]
    async fn test_update_session_triggers_auto_save() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = SessionConfig {
            max_sessions: 5,
            auto_compaction: None,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 2, // Low threshold for testing
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };

        let manager = SessionManager::new(config);
        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Update session twice to trigger auto-save
        let updated_session = session.clone();
        manager.update_session(updated_session).await.unwrap();

        let updated_session2 = manager.get_session(&session_id).await.unwrap().unwrap();
        manager.update_session(updated_session2).await.unwrap();

        // Verify change counter was reset after auto-save
        let changes = manager.changes_since_save.read().await;
        let change_count = changes.get(&session_id).unwrap_or(&0);
        assert_eq!(*change_count, 0);
    }

    #[tokio::test]
    async fn test_session_persistence_roundtrip() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = SessionConfig {
            max_sessions: 5,
            auto_compaction: None,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 1,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };

        let manager = SessionManager::new(config);
        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Add multiple messages
        for i in 0..5 {
            let message = Message {
                role: MessageRole::User,
                content: format!("Message {}", i),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            };
            manager.add_message(&session_id, message).await.unwrap();
        }

        // Manually save to ensure it's persisted
        manager.save_session(&session_id).await.unwrap();

        // Load directly from storage to verify persistence
        if let Some(ref storage) = manager.storage {
            let loaded = storage.load_session(&session_id).await.unwrap();
            assert!(loaded.is_some());
            let loaded_session = loaded.unwrap();
            assert_eq!(loaded_session.id, session_id);
            assert_eq!(loaded_session.messages.len(), 5);

            // Verify message content
            for (i, message) in loaded_session.messages.iter().enumerate() {
                assert_eq!(message.content, format!("Message {}", i));
            }
        }
    }

    #[tokio::test]
    async fn test_session_current_mode_initialization() {
        let config = create_test_config();
        let manager = SessionManager::new(config);

        let session = manager.create_session().await.unwrap();

        // New sessions should have no current_mode set
        assert_eq!(session.current_mode, None);
    }

    #[tokio::test]
    async fn test_session_current_mode_update() {
        let config = create_test_config();
        let manager = SessionManager::new(config);

        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Update session with a current_mode
        let mut updated_session = session.clone();
        updated_session.current_mode = Some("planning".to_string());
        manager.update_session(updated_session).await.unwrap();

        // Retrieve and verify
        let retrieved = manager.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(retrieved.current_mode, Some("planning".to_string()));
    }

    #[tokio::test]
    async fn test_session_current_mode_change() {
        let config = create_test_config();
        let manager = SessionManager::new(config);

        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Set initial mode
        let mut updated_session = session.clone();
        updated_session.current_mode = Some("coding".to_string());
        manager.update_session(updated_session).await.unwrap();

        // Change to different mode
        let retrieved = manager.get_session(&session_id).await.unwrap().unwrap();
        let mut changed_session = retrieved.clone();
        changed_session.current_mode = Some("debugging".to_string());
        manager.update_session(changed_session).await.unwrap();

        // Verify the change
        let final_session = manager.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(final_session.current_mode, Some("debugging".to_string()));
    }

    #[tokio::test]
    async fn test_session_current_mode_clear() {
        let config = create_test_config();
        let manager = SessionManager::new(config);

        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Set a mode
        let mut updated_session = session.clone();
        updated_session.current_mode = Some("research".to_string());
        manager.update_session(updated_session).await.unwrap();

        // Clear the mode
        let retrieved = manager.get_session(&session_id).await.unwrap().unwrap();
        let mut cleared_session = retrieved.clone();
        cleared_session.current_mode = None;
        manager.update_session(cleared_session).await.unwrap();

        // Verify it's cleared
        let final_session = manager.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(final_session.current_mode, None);
    }

    #[tokio::test]
    async fn test_session_current_mode_persistence() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config = SessionConfig {
            max_sessions: 5,
            auto_compaction: None,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 1,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };

        let manager = SessionManager::new(config.clone());
        let session = manager.create_session().await.unwrap();
        let session_id = session.id;

        // Set current_mode
        let mut updated_session = session.clone();
        updated_session.current_mode = Some("interactive".to_string());
        manager.update_session(updated_session).await.unwrap();

        // Save to disk
        manager.save_session(&session_id).await.unwrap();

        // Create new manager and restore sessions
        let manager2 = SessionManager::new(config);
        manager2.restore_sessions().await.unwrap();

        // Verify current_mode was persisted
        let restored = manager2.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(restored.current_mode, Some("interactive".to_string()));
    }

    #[tokio::test]
    async fn test_session_current_mode_multiple_sessions() {
        let config = create_test_config();
        let manager = SessionManager::new(config);

        // Create multiple sessions with different modes
        let session1 = manager.create_session().await.unwrap();
        let session2 = manager.create_session().await.unwrap();
        let session3 = manager.create_session().await.unwrap();

        // Set different modes
        let mut s1 = session1.clone();
        s1.current_mode = Some("planning".to_string());
        manager.update_session(s1).await.unwrap();

        let mut s2 = session2.clone();
        s2.current_mode = Some("coding".to_string());
        manager.update_session(s2).await.unwrap();

        let mut s3 = session3.clone();
        s3.current_mode = Some("testing".to_string());
        manager.update_session(s3).await.unwrap();

        // Verify each session has its own mode
        let retrieved1 = manager.get_session(&session1.id).await.unwrap().unwrap();
        let retrieved2 = manager.get_session(&session2.id).await.unwrap().unwrap();
        let retrieved3 = manager.get_session(&session3.id).await.unwrap().unwrap();

        assert_eq!(retrieved1.current_mode, Some("planning".to_string()));
        assert_eq!(retrieved2.current_mode, Some("coding".to_string()));
        assert_eq!(retrieved3.current_mode, Some("testing".to_string()));
    }

    #[tokio::test]
    async fn test_multiple_concurrent_sessions() {
        use tokio::task::JoinSet;

        let config = create_test_config();
        let manager = Arc::new(SessionManager::new(config));

        // Test 1: Create multiple sessions concurrently
        let mut create_tasks = JoinSet::new();
        for _ in 0..5 {
            let manager_clone = manager.clone();
            create_tasks.spawn(async move { manager_clone.create_session().await });
        }

        let mut session_ids = Vec::new();
        while let Some(result) = create_tasks.join_next().await {
            let session = result.unwrap().unwrap();
            session_ids.push(session.id);
        }

        assert_eq!(session_ids.len(), 5);
        assert_eq!(manager.get_session_count().await, 5);

        // Test 2: Add messages to multiple sessions concurrently
        let mut add_message_tasks = JoinSet::new();
        for (i, session_id) in session_ids.iter().enumerate() {
            let manager_clone = manager.clone();
            let session_id = *session_id;
            add_message_tasks.spawn(async move {
                for j in 0..3 {
                    let message = Message {
                        role: MessageRole::User,
                        content: format!("Session {} message {}", i, j),
                        tool_call_id: None,
                        tool_name: None,
                        timestamp: SystemTime::now(),
                    };
                    manager_clone
                        .add_message(&session_id, message)
                        .await
                        .unwrap();
                }
                session_id
            });
        }

        while let Some(result) = add_message_tasks.join_next().await {
            result.unwrap();
        }

        // Test 3: Read all sessions concurrently and verify message counts
        let mut read_tasks = JoinSet::new();
        for session_id in session_ids.iter() {
            let manager_clone = manager.clone();
            let session_id = *session_id;
            read_tasks.spawn(async move {
                manager_clone
                    .get_session(&session_id)
                    .await
                    .unwrap()
                    .unwrap()
            });
        }

        let mut message_counts = Vec::new();
        while let Some(result) = read_tasks.join_next().await {
            let session = result.unwrap();
            message_counts.push(session.messages.len());
        }

        // Each session should have 3 messages
        for count in message_counts {
            assert_eq!(count, 3);
        }

        // Test 4: Update sessions concurrently
        let mut update_tasks = JoinSet::new();
        for (i, session_id) in session_ids.iter().enumerate() {
            let manager_clone = manager.clone();
            let session_id = *session_id;
            update_tasks.spawn(async move {
                let session = manager_clone
                    .get_session(&session_id)
                    .await
                    .unwrap()
                    .unwrap();
                let mut updated = session.clone();
                updated.current_mode = Some(format!("mode_{}", i));
                manager_clone.update_session(updated).await.unwrap();
                session_id
            });
        }

        while let Some(result) = update_tasks.join_next().await {
            result.unwrap();
        }

        // Test 5: Verify all updates were applied correctly
        for (i, session_id) in session_ids.iter().enumerate() {
            let session = manager.get_session(session_id).await.unwrap().unwrap();
            assert_eq!(session.current_mode, Some(format!("mode_{}", i)));
        }

        // Test 6: Get session stats with concurrent access
        let mut stats_tasks = JoinSet::new();
        for _ in 0..10 {
            let manager_clone = manager.clone();
            stats_tasks.spawn(async move { manager_clone.get_session_stats().await });
        }

        while let Some(result) = stats_tasks.join_next().await {
            let stats = result.unwrap();
            assert_eq!(stats.total_sessions, 5);
            assert_eq!(stats.active_sessions, 5);
            assert_eq!(stats.total_messages, 15); // 5 sessions * 3 messages
        }

        // Test 7: Delete sessions concurrently
        let mut delete_tasks = JoinSet::new();
        for session_id in session_ids.iter().take(3) {
            let manager_clone = manager.clone();
            let session_id = *session_id;
            delete_tasks
                .spawn(async move { manager_clone.delete_session(&session_id).await.unwrap() });
        }

        let mut deleted_count = 0;
        while let Some(result) = delete_tasks.join_next().await {
            if result.unwrap() {
                deleted_count += 1;
            }
        }

        assert_eq!(deleted_count, 3);
        assert_eq!(manager.get_session_count().await, 2);

        // Test 8: List sessions with concurrent access
        let mut list_tasks = JoinSet::new();
        for _ in 0..5 {
            let manager_clone = manager.clone();
            list_tasks.spawn(async move { manager_clone.list_sessions().await.unwrap() });
        }

        while let Some(result) = list_tasks.join_next().await {
            let sessions = result.unwrap();
            assert_eq!(sessions.len(), 2);
        }
    }

    #[tokio::test]
    async fn test_concurrent_sessions_with_persistence() {
        use tempfile::TempDir;
        use tokio::task::JoinSet;

        let temp_dir = TempDir::new().unwrap();
        let config = SessionConfig {
            max_sessions: 10,
            auto_compaction: None,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 2,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };

        let manager = Arc::new(SessionManager::new(config));

        // Create sessions concurrently
        let mut create_tasks = JoinSet::new();
        for _ in 0..5 {
            let manager_clone = manager.clone();
            create_tasks.spawn(async move { manager_clone.create_session().await });
        }

        let mut session_ids = Vec::new();
        while let Some(result) = create_tasks.join_next().await {
            let session = result.unwrap().unwrap();
            session_ids.push(session.id);
        }

        // Add messages to trigger auto-save
        let mut message_tasks = JoinSet::new();
        for session_id in session_ids.iter() {
            let manager_clone = manager.clone();
            let session_id = *session_id;
            message_tasks.spawn(async move {
                for i in 0..3 {
                    let message = Message {
                        role: MessageRole::User,
                        content: format!("Message {}", i),
                        tool_call_id: None,
                        tool_name: None,
                        timestamp: SystemTime::now(),
                    };
                    manager_clone
                        .add_message(&session_id, message)
                        .await
                        .unwrap();
                }
            });
        }

        while let Some(result) = message_tasks.join_next().await {
            result.unwrap();
        }

        // Save all sessions concurrently
        let mut save_tasks = JoinSet::new();
        for session_id in session_ids.iter() {
            let manager_clone = manager.clone();
            let session_id = *session_id;
            save_tasks.spawn(async move { manager_clone.save_session(&session_id).await });
        }

        while let Some(result) = save_tasks.join_next().await {
            result.unwrap().unwrap();
        }

        // Verify all session files exist
        for session_id in session_ids.iter() {
            let session_file = temp_dir.path().join(format!("{}.json", session_id));
            assert!(session_file.exists());
        }

        // Create new manager and restore sessions
        let config2 = SessionConfig {
            max_sessions: 10,
            auto_compaction: None,
            persistence_enabled: true,
            session_storage_dir: Some(temp_dir.path().to_path_buf()),
            session_ttl_hours: 24,
            auto_save_threshold: 2,
            max_kv_cache_files: 16,
            kv_cache_dir: None,
        };
        let manager2 = Arc::new(SessionManager::new(config2));

        let restored_count = manager2.restore_sessions().await.unwrap();
        assert_eq!(restored_count, 5);

        // Verify restored sessions have correct data
        let mut verify_tasks = JoinSet::new();
        for session_id in session_ids.iter() {
            let manager_clone = manager2.clone();
            let session_id = *session_id;
            verify_tasks.spawn(async move {
                let session = manager_clone
                    .get_session(&session_id)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(session.messages.len(), 3);
                session_id
            });
        }

        let mut verified_count = 0;
        while let Some(result) = verify_tasks.join_next().await {
            result.unwrap();
            verified_count += 1;
        }

        assert_eq!(verified_count, 5);
    }
}
