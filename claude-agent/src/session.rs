//! Session management system for tracking conversation contexts and state

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use swissarmyhammer_common::SwissarmyhammerDirectory;
use ulid::Ulid;

/// Session identifier - raw ULID that can be used with Claude CLI
///
/// # Format
/// Raw ULID (no prefix): `01ARZ3NDEKTSV4RRFFQ69G5FAV`
///
/// # Claude CLI Compatibility
/// ULIDs are valid UUIDs and can be passed to `claude --session-id <uuid>`
///
/// # Properties
/// 1. Unique identifier for conversation context
/// 2. Must persist across session loads
/// 3. Used in session/prompt, session/cancel, session/load
/// 4. URL-safe and filesystem-safe
/// 5. Sortable by creation time
/// 6. 128-bit entropy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(Ulid);

impl SessionId {
    /// Create a new session ID
    pub fn new() -> Self {
        Self(Ulid::new())
    }

    /// Parse a session ID from string
    ///
    /// # Format
    /// Expects raw ULID string: `01ARZ3NDEKTSV4RRFFQ69G5FAV`
    ///
    /// # Errors
    /// Returns error if:
    /// - Invalid ULID format
    /// - Empty string
    pub fn parse(s: &str) -> Result<Self, SessionIdError> {
        if s.is_empty() {
            return Err(SessionIdError::Empty);
        }

        match Ulid::from_string(s) {
            Ok(ulid) => Ok(Self(ulid)),
            Err(e) => Err(SessionIdError::InvalidUlid {
                provided: s.to_string(),
                error: e.to_string(),
            }),
        }
    }

    /// Get the underlying ULID
    pub fn as_ulid(&self) -> Ulid {
        self.0
    }

    /// Get the ULID string representation
    ///
    /// Returns the raw ULID string.
    ///
    /// # Example
    /// ```ignore
    /// let session_id = SessionId::new();
    /// let ulid_str = session_id.ulid_string(); // "01ARZ3NDEKTSV4RRFFQ69G5FAV"
    /// ```
    pub fn ulid_string(&self) -> String {
        self.0.to_string()
    }

    /// Convert ULID to UUID format for Claude CLI compatibility
    ///
    /// Claude CLI's --session-id flag requires standard UUID format (8-4-4-4-12 hex digits).
    /// This converts the ULID's 128-bit value to UUID string representation.
    ///
    /// # Example
    /// ```ignore
    /// let session_id = SessionId::new();
    /// let uuid_str = session_id.to_uuid_string(); // "550e8400-e29b-41d4-a716-446655440000"
    /// ```
    pub fn to_uuid_string(&self) -> String {
        // ULID is 128 bits, same as UUID
        // Convert ULID to bytes array then format as UUID
        let bytes = self.0.to_bytes();
        format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5],
            bytes[6], bytes[7],
            bytes[8], bytes[9],
            bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
        )
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for SessionId {
    type Err = SessionIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl From<Ulid> for SessionId {
    fn from(ulid: Ulid) -> Self {
        Self(ulid)
    }
}

impl Serialize for SessionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SessionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Errors that can occur when parsing session IDs
#[derive(Debug, Clone, thiserror::Error)]
pub enum SessionIdError {
    /// Session ID is an empty string
    ///
    /// This error occurs when attempting to parse an empty string as a session ID.
    /// Provide a properly formatted ULID session ID.
    #[error("Session ID cannot be empty")]
    Empty,

    /// The ULID is malformed
    ///
    /// This error occurs when the provided string is not a valid ULID.
    /// ULIDs must be exactly 26 characters using Crockford's Base32 encoding
    /// (0-9, A-Z excluding I, L, O, U).
    ///
    /// # Example
    /// - Invalid: `INVALID` (too short)
    /// - Invalid: `01ARZ3NDEKTSV4RRFFQ69G5FAV!!!` (invalid characters)
    /// - Valid: `01ARZ3NDEKTSV4RRFFQ69G5FAV`
    #[error("Invalid ULID format in session ID '{provided}': {error}")]
    InvalidUlid { provided: String, error: String },
}

/// A conversation session containing context and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub created_at: SystemTime,
    pub last_accessed: SystemTime,
    pub context: Vec<Message>,
    pub client_capabilities: Option<agent_client_protocol::ClientCapabilities>,
    pub mcp_servers: Vec<String>,
    /// Working directory for this session (ACP requirement - must be absolute path)
    pub cwd: PathBuf,
    /// Available commands that can be invoked during this session
    pub available_commands: Vec<agent_client_protocol::AvailableCommand>,
    /// Number of language model requests made in the current turn
    pub turn_request_count: u64,
    /// Total tokens consumed in the current turn (input + output)
    pub turn_token_count: u64,
    /// Current session mode identifier for ACP current mode updates
    pub current_mode: Option<String>,
}

impl Session {
    /// Create a new session with the given ID and working directory
    ///
    /// # Arguments
    /// * `id` - Unique session identifier (ULID)
    /// * `cwd` - Working directory for the session (must be absolute path as per ACP spec)
    ///
    /// # Panics
    /// This function will panic if the working directory is not absolute, as this violates
    /// the ACP specification requirement that sessions must have absolute working directories.
    pub fn new(id: SessionId, cwd: PathBuf) -> Self {
        // ACP requires absolute working directory - validate this at session creation
        if !cwd.is_absolute() {
            panic!(
                "Session working directory must be absolute path (ACP requirement), got: {}",
                cwd.display()
            );
        }

        let now = SystemTime::now();
        Self {
            id,
            created_at: now,
            last_accessed: now,
            context: Vec::new(),
            client_capabilities: None,
            mcp_servers: Vec::new(),
            cwd,
            available_commands: Vec::new(),
            turn_request_count: 0,
            turn_token_count: 0,
            current_mode: None,
        }
    }

    /// Add a message to the session context
    pub fn add_message(&mut self, message: Message) {
        self.context.push(message);
        self.last_accessed = SystemTime::now();
    }

    /// Update the last accessed time
    pub fn update_access_time(&mut self) {
        self.last_accessed = SystemTime::now();
    }

    /// Update available commands for this session
    pub fn update_available_commands(
        &mut self,
        commands: Vec<agent_client_protocol::AvailableCommand>,
    ) {
        self.available_commands = commands;
        self.last_accessed = SystemTime::now();
    }

    /// Check if available commands have changed from the given set
    pub fn has_available_commands_changed(
        &self,
        new_commands: &[agent_client_protocol::AvailableCommand],
    ) -> bool {
        if self.available_commands.len() != new_commands.len() {
            return true;
        }

        // Compare each command by all fields
        for (existing, new) in self.available_commands.iter().zip(new_commands.iter()) {
            if existing.name != new.name
                || existing.description != new.description
                || existing.input != new.input
                || existing.meta != new.meta
            {
                return true;
            }
        }

        false
    }

    /// Reset turn counters for a new turn
    pub fn reset_turn_counters(&mut self) {
        self.turn_request_count = 0;
        self.turn_token_count = 0;
        self.last_accessed = SystemTime::now();
    }

    /// Increment the turn request count and return the new value
    pub fn increment_turn_requests(&mut self) -> u64 {
        self.turn_request_count += 1;
        self.last_accessed = SystemTime::now();
        self.turn_request_count
    }

    /// Add tokens to the current turn count and return the new total
    pub fn add_turn_tokens(&mut self, tokens: u64) -> u64 {
        self.turn_token_count += tokens;
        self.last_accessed = SystemTime::now();
        self.turn_token_count
    }

    /// Get the current turn request count
    pub fn get_turn_request_count(&self) -> u64 {
        self.turn_request_count
    }

    /// Get the current turn token count
    pub fn get_turn_token_count(&self) -> u64 {
        self.turn_token_count
    }
}

/// Session event storing ACP SessionUpdate with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub timestamp: SystemTime,
    pub update: agent_client_protocol::SessionUpdate,
}

impl Message {
    /// Create message from SessionUpdate
    pub fn from_update(update: agent_client_protocol::SessionUpdate) -> Self {
        Self {
            timestamp: SystemTime::now(),
            update,
        }
    }

    /// Create text message from role and content string
    ///
    /// Convenience constructor that wraps text in appropriate SessionUpdate variant
    pub fn new(role: MessageRole, content: String) -> Self {
        use agent_client_protocol::{ContentBlock, SessionUpdate, TextContent};

        let update = match role {
            MessageRole::User => {
                let text_content = TextContent::new(content);
                let content_block = ContentBlock::Text(text_content);
                SessionUpdate::UserMessageChunk(agent_client_protocol::ContentChunk::new(
                    content_block,
                ))
            }
            MessageRole::Assistant => {
                let text_content = TextContent::new(content);
                let content_block = ContentBlock::Text(text_content);
                SessionUpdate::AgentMessageChunk(agent_client_protocol::ContentChunk::new(
                    content_block,
                ))
            }
            MessageRole::System => {
                let text_content = TextContent::new(format!("[System] {}", content));
                let content_block = ContentBlock::Text(text_content);
                SessionUpdate::AgentMessageChunk(agent_client_protocol::ContentChunk::new(
                    content_block,
                ))
            }
        };

        Self::from_update(update)
    }
}

/// Message role for simple text message construction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Thread-safe session manager
#[derive(Debug)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    cleanup_interval: Duration,
    max_session_age: Duration,
    storage_path: Option<PathBuf>,
}

impl SessionManager {
    /// Create a new session manager with default settings
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            cleanup_interval: Duration::from_secs(300), // 5 minutes
            max_session_age: Duration::from_secs(3600), // 1 hour
            storage_path: Self::default_storage_path(),
        }
    }

    /// Create a new session manager with custom cleanup settings
    pub fn with_cleanup_settings(cleanup_interval: Duration, max_session_age: Duration) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            cleanup_interval,
            max_session_age,
            storage_path: Self::default_storage_path(),
        }
    }

    /// Set the storage path for sessions
    ///
    /// This allows tests and custom configurations to specify where session files are stored.
    /// If set to None, sessions will only be kept in memory.
    pub fn with_storage_path(mut self, path: Option<PathBuf>) -> Self {
        self.storage_path = path;
        self
    }

    /// Get the default storage path for sessions
    fn default_storage_path() -> Option<PathBuf> {
        std::env::current_dir().ok().map(|cwd| {
            cwd.join(SwissarmyhammerDirectory::dir_name())
                .join("sessions")
        })
    }

    /// Ensure the storage directory exists
    fn ensure_storage_directory(&self) -> crate::Result<PathBuf> {
        if let Some(storage_path) = &self.storage_path {
            std::fs::create_dir_all(storage_path).map_err(|e| {
                crate::AgentError::Session(format!(
                    "Failed to create session storage directory: {}",
                    e
                ))
            })?;
            Ok(storage_path.clone())
        } else {
            Err(crate::AgentError::Session(
                "No storage path configured".to_string(),
            ))
        }
    }

    /// Get the file path for a session
    fn session_file_path(&self, session_id: &SessionId) -> crate::Result<PathBuf> {
        let storage_path = self.ensure_storage_directory()?;
        Ok(storage_path.join(format!("{}.json", session_id)))
    }

    /// Save a session to disk
    fn save_session_to_disk(&self, session: &Session) -> crate::Result<()> {
        let file_path = self.session_file_path(&session.id)?;
        let json = serde_json::to_string_pretty(session).map_err(|e| {
            crate::AgentError::Session(format!("Failed to serialize session: {}", e))
        })?;
        std::fs::write(&file_path, json).map_err(|e| {
            crate::AgentError::Session(format!("Failed to write session to disk: {}", e))
        })?;
        tracing::debug!("Saved session {} to disk", session.id);
        Ok(())
    }

    /// Load a session from disk
    fn load_session_from_disk(&self, session_id: &SessionId) -> crate::Result<Option<Session>> {
        let file_path = self.session_file_path(session_id)?;

        if !file_path.exists() {
            return Ok(None);
        }

        let json = std::fs::read_to_string(&file_path).map_err(|e| {
            crate::AgentError::Session(format!("Failed to read session from disk: {}", e))
        })?;

        let session: Session = serde_json::from_str(&json).map_err(|e| {
            crate::AgentError::Session(format!("Failed to deserialize session: {}", e))
        })?;

        tracing::debug!("Loaded session {} from disk", session_id);
        Ok(Some(session))
    }

    /// Delete a session file from disk
    fn delete_session_from_disk(&self, session_id: &SessionId) -> crate::Result<()> {
        let file_path = self.session_file_path(session_id)?;

        if file_path.exists() {
            std::fs::remove_file(&file_path).map_err(|e| {
                crate::AgentError::Session(format!("Failed to delete session from disk: {}", e))
            })?;
            tracing::debug!("Deleted session {} from disk", session_id);
        }

        Ok(())
    }

    /// Create a new session with specified working directory and return its ID
    ///
    /// # Arguments
    /// * `cwd` - Working directory for the session (must be absolute path as per ACP spec)
    /// * `client_capabilities` - Optional client capabilities
    ///
    /// # Errors
    /// Returns error if:
    /// - Working directory validation fails
    /// - Session storage write lock cannot be acquired
    pub fn create_session(
        &self,
        cwd: PathBuf,
        client_capabilities: Option<agent_client_protocol::ClientCapabilities>,
    ) -> crate::Result<SessionId> {
        // Validate working directory before creating session
        crate::session_validation::validate_working_directory(&cwd).map_err(|e| {
            crate::AgentError::Session(format!("Working directory validation failed: {}", e))
        })?;

        let session_id = SessionId::new();
        let mut session = Session::new(session_id, cwd);
        session.client_capabilities = client_capabilities;

        // Save to disk first
        self.save_session_to_disk(&session)?;

        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| crate::AgentError::Session("Failed to acquire write lock".to_string()))?;

        sessions.insert(session_id, session);
        tracing::debug!("Created new session: {}", session_id);
        Ok(session_id)
    }

    /// Get a session by ID
    pub fn get_session(&self, session_id: &SessionId) -> crate::Result<Option<Session>> {
        // Fast path: check in-memory cache with read lock
        {
            let sessions = self.sessions.read().map_err(|_| {
                crate::AgentError::Session("Failed to acquire read lock".to_string())
            })?;

            if let Some(session) = sessions.get(session_id) {
                return Ok(Some(session.clone()));
            }
        } // Release read lock immediately

        // Slow path: load from disk if storage is configured
        if let Some(session) = self.load_session_from_disk(session_id)? {
            // Cache the loaded session in memory
            let mut sessions = self.sessions.write().map_err(|_| {
                crate::AgentError::Session("Failed to acquire write lock".to_string())
            })?;

            // Double-check in case another thread loaded it while we were acquiring write lock
            if let Some(existing) = sessions.get(session_id) {
                return Ok(Some(existing.clone()));
            }

            sessions.insert(*session_id, session.clone());
            tracing::debug!("Loaded session {} from disk into memory", session_id);
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }

    /// Update a session using a closure
    pub fn update_session<F>(&self, session_id: &SessionId, updater: F) -> crate::Result<()>
    where
        F: FnOnce(&mut Session),
    {
        // First ensure the session is loaded into memory
        self.get_session(session_id)?;

        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| crate::AgentError::Session("Failed to acquire write lock".to_string()))?;

        if let Some(session) = sessions.get_mut(session_id) {
            updater(session);
            session.update_access_time();

            // Save updated session to disk
            let session_clone = session.clone();
            drop(sessions); // Release lock before disk I/O
            self.save_session_to_disk(&session_clone)?;

            tracing::trace!("Updated session: {}", session_id);
        } else {
            tracing::warn!("Attempted to update non-existent session: {}", session_id);
        }

        Ok(())
    }

    /// Remove a session and return it if it existed
    ///
    /// Note: This method only removes the session data. To properly clean up all
    /// associated resources (including terminal processes), use `remove_session_with_cleanup`
    /// which accepts a `TerminalManager`.
    pub fn remove_session(&self, session_id: &SessionId) -> crate::Result<Option<Session>> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| crate::AgentError::Session("Failed to acquire write lock".to_string()))?;

        let removed = sessions.remove(session_id);

        // Also remove from disk
        drop(sessions); // Release lock before disk I/O
        let _ = self.delete_session_from_disk(session_id); // Best effort deletion

        if removed.is_some() {
            tracing::debug!("Removed session: {}", session_id);
        }
        Ok(removed)
    }

    /// Remove a session and clean up all associated resources including terminals
    ///
    /// This is the preferred method for removing sessions as it ensures proper cleanup
    /// of terminal processes associated with the session. It:
    /// 1. Cleans up all terminal processes for the session
    /// 2. Removes the session from memory and disk
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID to remove
    /// * `terminal_manager` - Terminal manager for cleaning up associated terminals
    ///
    /// # Returns
    ///
    /// Returns the removed session if it existed, or None if not found
    pub async fn remove_session_with_cleanup(
        &self,
        session_id: &SessionId,
        terminal_manager: &crate::terminal_manager::TerminalManager,
    ) -> crate::Result<Option<Session>> {
        // First clean up all terminals associated with this session
        let session_id_str = session_id.to_string();
        match terminal_manager
            .cleanup_session_terminals(&session_id_str)
            .await
        {
            Ok(count) => {
                if count > 0 {
                    tracing::info!(
                        "Cleaned up {} terminal(s) before removing session {}",
                        count,
                        session_id
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to clean up terminals for session {}: {}",
                    session_id,
                    e
                );
                // Continue with session removal even if terminal cleanup fails
            }
        }

        // Now remove the session
        self.remove_session(session_id)
    }

    /// List all session IDs (from both memory and disk)
    pub fn list_sessions(&self) -> crate::Result<Vec<SessionId>> {
        let mut session_ids = std::collections::HashSet::new();

        // Get in-memory sessions
        let sessions = self
            .sessions
            .read()
            .map_err(|_| crate::AgentError::Session("Failed to acquire read lock".to_string()))?;

        for id in sessions.keys() {
            session_ids.insert(*id);
        }

        drop(sessions); // Release lock before disk I/O

        // Get sessions from disk
        if let Some(storage_path) = &self.storage_path {
            if storage_path.exists() {
                if let Ok(entries) = std::fs::read_dir(storage_path) {
                    for entry in entries.flatten() {
                        if let Some(file_name) = entry.file_name().to_str() {
                            if file_name.ends_with(".json") {
                                if let Some(id_str) = file_name.strip_suffix(".json") {
                                    if let Ok(session_id) = SessionId::parse(id_str) {
                                        session_ids.insert(session_id);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(session_ids.into_iter().collect())
    }

    /// Get the number of active sessions
    pub fn session_count(&self) -> crate::Result<usize> {
        let sessions = self
            .sessions
            .read()
            .map_err(|_| crate::AgentError::Session("Failed to acquire read lock".to_string()))?;

        Ok(sessions.len())
    }

    /// Update available commands for a session and return whether an update was sent
    /// Returns true if commands changed and update was needed, false if no change
    pub fn update_available_commands(
        &self,
        session_id: &SessionId,
        commands: Vec<agent_client_protocol::AvailableCommand>,
    ) -> crate::Result<bool> {
        // First ensure the session is loaded into memory
        self.get_session(session_id)?;

        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| crate::AgentError::Session("Failed to acquire write lock".to_string()))?;

        if let Some(session) = sessions.get_mut(session_id) {
            let commands_changed = session.has_available_commands_changed(&commands);
            if commands_changed {
                session.update_available_commands(commands);

                // Save updated session to disk
                let session_clone = session.clone();
                drop(sessions); // Release lock before disk I/O
                self.save_session_to_disk(&session_clone)?;

                tracing::debug!("Updated available commands for session: {}", session_id);
                Ok(true)
            } else {
                tracing::trace!("Available commands unchanged for session: {}", session_id);
                Ok(false)
            }
        } else {
            tracing::warn!(
                "Attempted to update commands for non-existent session: {}",
                session_id
            );
            Ok(false)
        }
    }

    /// Start the cleanup task that removes expired sessions
    pub async fn start_cleanup_task(self: Arc<Self>) {
        let manager = Arc::clone(&self);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(manager.cleanup_interval);

            tracing::info!(
                "Session cleanup task started with interval: {:?}",
                manager.cleanup_interval
            );

            loop {
                interval.tick().await;
                if let Err(e) = manager.cleanup_expired_sessions().await {
                    tracing::error!("Session cleanup failed: {}", e);
                }
            }
        });
    }

    /// Clean up expired sessions
    async fn cleanup_expired_sessions(&self) -> crate::Result<()> {
        let now = SystemTime::now();
        let mut expired_sessions = Vec::new();

        // Find expired sessions
        {
            let sessions = self.sessions.read().map_err(|_| {
                crate::AgentError::Session("Failed to acquire read lock".to_string())
            })?;

            for (id, session) in sessions.iter() {
                if let Ok(age) = now.duration_since(session.last_accessed) {
                    if age > self.max_session_age {
                        expired_sessions.push(*id);
                    }
                }
            }
        }

        // Remove expired sessions
        for session_id in expired_sessions {
            tracing::info!("Cleaning up expired session: {}", session_id);
            self.remove_session(&session_id)?;
        }

        Ok(())
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // SessionId tests
    #[test]
    fn test_session_id_new() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();

        // Should be different
        assert_ne!(id1, id2);

        // Should have correct format (raw ULID, 26 characters)
        let id_str = id1.to_string();
        assert_eq!(id_str.len(), 26); // 26-char ULID
    }

    #[test]
    fn test_session_id_parse_valid() {
        let valid_id = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let result = SessionId::parse(valid_id);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_string(), valid_id);
    }

    #[test]
    fn test_session_id_parse_invalid_ulid() {
        let invalid_id = "invalid-ulid-format";
        let result = SessionId::parse(invalid_id);
        assert!(result.is_err());

        match result {
            Err(SessionIdError::InvalidUlid { .. }) => {}
            _ => panic!("Expected InvalidUlid error"),
        }
    }

    #[test]
    fn test_session_id_parse_empty() {
        let result = SessionId::parse("");
        assert!(result.is_err());

        match result {
            Err(SessionIdError::Empty) => {}
            _ => panic!("Expected Empty error"),
        }
    }

    #[test]
    fn test_session_id_serialization() {
        let session_id = SessionId::new();
        let serialized = serde_json::to_string(&session_id).unwrap();

        // Should be able to deserialize back
        let deserialized: SessionId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(session_id, deserialized);
    }

    #[test]
    fn test_session_id_from_ulid() {
        let ulid = Ulid::new();
        let session_id = SessionId::from(ulid);

        assert_eq!(session_id.as_ulid(), ulid);
        assert_eq!(session_id.to_string(), ulid.to_string());
    }

    #[test]
    fn test_session_id_display() {
        let ulid = Ulid::from_string("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();
        let session_id = SessionId::from(ulid);

        assert_eq!(session_id.to_string(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    }

    #[test]
    fn test_session_id_from_str() {
        use std::str::FromStr;

        let valid_id = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let session_id = SessionId::from_str(valid_id).unwrap();

        assert_eq!(session_id.to_string(), valid_id);
    }

    #[test]
    fn test_session_id_url_safe() {
        let session_id = SessionId::new();
        let id_str = session_id.to_string();

        // Check that it only contains URL-safe characters (ULIDs use alphanumeric only)
        assert!(id_str.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    // Session tests
    #[test]
    fn test_session_creation() {
        let session_id = SessionId::new();
        let cwd = std::env::current_dir().unwrap();
        let session = Session::new(session_id, cwd.clone());

        assert_eq!(session.id, session_id);
        assert_eq!(session.cwd, cwd);
        assert!(session.context.is_empty());
        assert!(session.client_capabilities.is_none());
        assert!(session.mcp_servers.is_empty());
    }

    #[test]
    fn test_message_creation() {
        let message = Message::new(MessageRole::User, "Hello".to_string());

        // Verify it creates appropriate SessionUpdate
        if let agent_client_protocol::SessionUpdate::UserMessageChunk(chunk) = message.update {
            if let agent_client_protocol::ContentBlock::Text(text) = chunk.content {
                assert_eq!(text.text, "Hello");
            } else {
                panic!("Expected Text content");
            }
        } else {
            panic!("Expected UserMessageChunk");
        }
    }

    #[test]
    fn test_session_add_message() {
        let session_id = SessionId::new();
        let cwd = std::env::current_dir().unwrap();
        let mut session = Session::new(session_id, cwd);
        let initial_time = session.last_accessed;

        // Small delay to ensure time difference
        std::thread::sleep(Duration::from_millis(1));

        let message = Message::new(MessageRole::User, "Hello".to_string());
        session.add_message(message);

        assert_eq!(session.context.len(), 1);
        assert!(session.last_accessed > initial_time);

        // Verify message is stored
        if let agent_client_protocol::SessionUpdate::UserMessageChunk(chunk) =
            &session.context[0].update
        {
            if let agent_client_protocol::ContentBlock::Text(text) = &chunk.content {
                assert_eq!(text.text, "Hello");
            } else {
                panic!("Expected Text content");
            }
        }
    }

    #[test]
    fn test_session_manager_creation() {
        let manager = SessionManager::new();
        assert_eq!(manager.cleanup_interval, Duration::from_secs(300));
        assert_eq!(manager.max_session_age, Duration::from_secs(3600));
    }

    #[test]
    fn test_session_manager_with_custom_settings() {
        let cleanup_interval = Duration::from_secs(60);
        let max_age = Duration::from_secs(1800);
        let manager = SessionManager::with_cleanup_settings(cleanup_interval, max_age);

        assert_eq!(manager.cleanup_interval, cleanup_interval);
        assert_eq!(manager.max_session_age, max_age);
    }

    #[test]
    fn test_create_and_get_session() {
        let manager = SessionManager::new();
        let cwd = std::env::current_dir().unwrap();

        let session_id = manager.create_session(cwd.clone(), None).unwrap();
        let session = manager.get_session(&session_id).unwrap();

        assert!(session.is_some());
        let session = session.unwrap();
        assert_eq!(session.id, session_id);
        assert_eq!(session.cwd, cwd);
    }

    #[test]
    fn test_get_nonexistent_session() {
        let manager = SessionManager::new();
        let nonexistent_id = SessionId::new();

        let session = manager.get_session(&nonexistent_id).unwrap();
        assert!(session.is_none());
    }

    #[test]
    fn test_update_session() {
        let manager = SessionManager::new();
        let cwd = std::env::current_dir().unwrap();
        let session_id = manager.create_session(cwd, None).unwrap();

        let message = Message::new(MessageRole::User, "Hello".to_string());

        manager
            .update_session(&session_id, |session| {
                session.add_message(message.clone());
            })
            .unwrap();

        let session = manager.get_session(&session_id).unwrap().unwrap();
        assert_eq!(session.context.len(), 1);

        // Verify message content
        if let agent_client_protocol::SessionUpdate::UserMessageChunk(chunk) =
            &session.context[0].update
        {
            if let agent_client_protocol::ContentBlock::Text(text) = &chunk.content {
                assert_eq!(text.text, "Hello");
            } else {
                panic!("Expected Text content");
            }
        }
    }

    #[test]
    fn test_update_nonexistent_session() {
        let manager = SessionManager::new();
        let nonexistent_id = SessionId::new();

        // Should not panic when trying to update a non-existent session
        let result = manager.update_session(&nonexistent_id, |session| {
            session.add_message(Message::new(MessageRole::User, "test".to_string()));
        });

        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_session() {
        let manager = SessionManager::new();
        let cwd = std::env::current_dir().unwrap();
        let session_id = manager.create_session(cwd, None).unwrap();

        // Verify session exists
        assert!(manager.get_session(&session_id).unwrap().is_some());

        // Remove session
        let removed = manager.remove_session(&session_id).unwrap();
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, session_id);

        // Verify session no longer exists
        assert!(manager.get_session(&session_id).unwrap().is_none());
    }

    #[tokio::test]
    async fn test_remove_session_with_cleanup() {
        use crate::terminal_manager::{TerminalCreateParams, TerminalManager};

        let manager = SessionManager::new();
        let terminal_manager = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        let session_id = manager.create_session(cwd, None).unwrap();
        let session_id_str = session_id.to_string();

        // Create a terminal associated with this session
        let params = TerminalCreateParams {
            session_id: session_id_str.clone(),
            command: "echo".to_string(),
            args: Some(vec!["test".to_string()]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let terminal_id = terminal_manager
            .create_terminal_with_command(&manager, params)
            .await
            .unwrap();

        // Verify terminal exists
        {
            let terminals = terminal_manager.terminals.read().await;
            assert!(terminals.contains_key(&terminal_id));
        }

        // Remove session with cleanup
        let removed = manager
            .remove_session_with_cleanup(&session_id, &terminal_manager)
            .await
            .unwrap();
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, session_id);

        // Verify session no longer exists
        assert!(manager.get_session(&session_id).unwrap().is_none());

        // Verify terminal was cleaned up
        {
            let terminals = terminal_manager.terminals.read().await;
            assert!(!terminals.contains_key(&terminal_id));
        }
    }

    #[tokio::test]
    async fn test_remove_session_with_cleanup_multiple_terminals() {
        use crate::terminal_manager::{TerminalCreateParams, TerminalManager};

        let manager = SessionManager::new();
        let terminal_manager = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        let session_id = manager.create_session(cwd, None).unwrap();
        let session_id_str = session_id.to_string();

        // Create multiple terminals associated with this session
        let mut terminal_ids = Vec::new();
        for i in 0..3 {
            let params = TerminalCreateParams {
                session_id: session_id_str.clone(),
                command: "echo".to_string(),
                args: Some(vec![format!("test{}", i)]),
                env: None,
                cwd: None,
                output_byte_limit: None,
            };

            let terminal_id = terminal_manager
                .create_terminal_with_command(&manager, params)
                .await
                .unwrap();
            terminal_ids.push(terminal_id);
        }

        // Verify all terminals exist
        {
            let terminals = terminal_manager.terminals.read().await;
            for terminal_id in &terminal_ids {
                assert!(terminals.contains_key(terminal_id));
            }
        }

        // Remove session with cleanup
        let removed = manager
            .remove_session_with_cleanup(&session_id, &terminal_manager)
            .await
            .unwrap();
        assert!(removed.is_some());

        // Verify all terminals were cleaned up
        {
            let terminals = terminal_manager.terminals.read().await;
            for terminal_id in &terminal_ids {
                assert!(!terminals.contains_key(terminal_id));
            }
        }
    }

    #[tokio::test]
    async fn test_remove_session_with_cleanup_no_terminals() {
        use crate::terminal_manager::TerminalManager;

        let manager = SessionManager::new();
        let terminal_manager = TerminalManager::new();
        let cwd = std::env::current_dir().unwrap();
        let session_id = manager.create_session(cwd, None).unwrap();

        // Remove session with cleanup (no terminals to clean up)
        let removed = manager
            .remove_session_with_cleanup(&session_id, &terminal_manager)
            .await
            .unwrap();
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, session_id);

        // Verify session no longer exists
        assert!(manager.get_session(&session_id).unwrap().is_none());
    }

    #[test]
    fn test_remove_nonexistent_session() {
        let manager = SessionManager::new();
        let nonexistent_id = SessionId::new();

        let removed = manager.remove_session(&nonexistent_id).unwrap();
        assert!(removed.is_none());
    }

    #[test]
    fn test_list_sessions() {
        let manager = SessionManager::new();
        let cwd = std::env::current_dir().unwrap();

        // Initially empty
        let sessions = manager.list_sessions().unwrap();
        assert_eq!(sessions.len(), 0);

        // Create some sessions
        let id1 = manager.create_session(cwd.clone(), None).unwrap();
        let id2 = manager.create_session(cwd, None).unwrap();

        let sessions = manager.list_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains(&id1));
        assert!(sessions.contains(&id2));
    }

    #[test]
    fn test_session_count() {
        let manager = SessionManager::new();
        let cwd = std::env::current_dir().unwrap();

        assert_eq!(manager.session_count().unwrap(), 0);

        manager.create_session(cwd.clone(), None).unwrap();
        assert_eq!(manager.session_count().unwrap(), 1);

        manager.create_session(cwd, None).unwrap();
        assert_eq!(manager.session_count().unwrap(), 2);
    }

    #[tokio::test]
    async fn test_cleanup_expired_sessions() {
        // Create manager with very short expiration time
        let manager = Arc::new(SessionManager::with_cleanup_settings(
            Duration::from_millis(100),
            Duration::from_millis(50), // 50ms max age
        ));

        // Create a session
        let cwd = std::env::current_dir().unwrap();
        let session_id = manager.create_session(cwd, None).unwrap();
        assert_eq!(manager.session_count().unwrap(), 1);

        // Wait for session to expire
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Manually trigger cleanup
        manager.cleanup_expired_sessions().await.unwrap();

        // Session should be removed
        assert_eq!(manager.session_count().unwrap(), 0);
        assert!(manager.get_session(&session_id).unwrap().is_none());
    }

    #[tokio::test]
    async fn test_cleanup_task_startup() {
        let manager = Arc::new(SessionManager::new());

        // This should not panic or block
        manager.clone().start_cleanup_task().await;

        // Give the task a moment to start
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    #[test]
    #[should_panic(expected = "Session working directory must be absolute path")]
    fn test_session_creation_with_relative_path_panics() {
        let session_id = SessionId::new();
        let relative_path = PathBuf::from("./relative/path");
        let _session = Session::new(session_id, relative_path);
    }

    #[test]
    fn test_create_session_with_invalid_working_directory() {
        let manager = SessionManager::new();
        let invalid_path = PathBuf::from("/nonexistent/directory");

        let result = manager.create_session(invalid_path, None);
        assert!(result.is_err());

        if let Err(crate::AgentError::Session(msg)) = result {
            assert!(msg.contains("Working directory validation failed"));
        } else {
            panic!("Expected Session error with working directory validation message");
        }
    }

    #[test]
    fn test_session_stores_working_directory() {
        let manager = SessionManager::new();
        let cwd = std::env::current_dir().unwrap();

        let session_id = manager.create_session(cwd.clone(), None).unwrap();
        let session = manager.get_session(&session_id).unwrap().unwrap();

        assert_eq!(session.cwd, cwd);
    }

    #[test]
    fn test_working_directory_validation_during_session_creation() {
        let manager = SessionManager::new();
        let non_absolute_path = PathBuf::from("relative/path");

        let result = manager.create_session(non_absolute_path, None);
        assert!(result.is_err());

        if let Err(crate::AgentError::Session(msg)) = result {
            assert!(msg.contains("Working directory validation failed"));
            assert!(msg.contains("must be absolute"));
        } else {
            panic!("Expected Session error with absolute path requirement");
        }
    }

    #[test]
    fn test_working_directory_preserved_across_session_operations() {
        let manager = SessionManager::new();
        let cwd = std::env::current_dir().unwrap();

        let session_id = manager.create_session(cwd.clone(), None).unwrap();

        // Add a message to the session
        manager
            .update_session(&session_id, |session| {
                session.add_message(Message::new(MessageRole::User, "test".to_string()));
            })
            .unwrap();

        // Retrieve session and verify working directory is preserved
        let session = manager.get_session(&session_id).unwrap().unwrap();
        assert_eq!(session.cwd, cwd);
        assert_eq!(session.context.len(), 1);
    }

    #[test]
    fn test_different_sessions_can_have_different_working_directories() {
        let manager = SessionManager::new();
        let cwd1 = std::env::current_dir().unwrap();
        let cwd2 = std::env::temp_dir();

        let session_id1 = manager.create_session(cwd1.clone(), None).unwrap();
        let session_id2 = manager.create_session(cwd2.clone(), None).unwrap();

        let session1 = manager.get_session(&session_id1).unwrap().unwrap();
        let session2 = manager.get_session(&session_id2).unwrap().unwrap();

        assert_eq!(session1.cwd, cwd1);
        assert_eq!(session2.cwd, cwd2);
        assert_ne!(session1.cwd, session2.cwd);
    }

    #[test]
    fn test_session_serialization_includes_working_directory() {
        let session_id = SessionId::new();
        let cwd = std::env::current_dir().unwrap();
        let session = Session::new(session_id, cwd.clone());

        // Test serialization
        let serialized = serde_json::to_string(&session).unwrap();
        assert!(serialized.contains(&cwd.to_string_lossy().to_string()));

        // Test deserialization
        let deserialized: Session = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.id, session_id);
        assert_eq!(deserialized.cwd, cwd);
    }

    #[cfg(unix)]
    #[test]
    fn test_unix_absolute_path_validation() {
        let manager = SessionManager::new();
        let unix_absolute = PathBuf::from("/tmp");

        let result = manager.create_session(unix_absolute.clone(), None);
        assert!(result.is_ok());

        let session_id = result.unwrap();
        let session = manager.get_session(&session_id).unwrap().unwrap();
        assert_eq!(session.cwd, unix_absolute);
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_absolute_path_validation() {
        let manager = SessionManager::new();
        let windows_absolute = PathBuf::from("C:\\temp");

        // This test assumes C:\temp exists on Windows systems
        // In real scenarios, we'd use a guaranteed existing path
        if windows_absolute.exists() {
            let result = manager.create_session(windows_absolute.clone(), None);
            assert!(result.is_ok());

            let session_id = result.unwrap();
            let session = manager.get_session(&session_id).unwrap().unwrap();
            assert_eq!(session.cwd, windows_absolute);
        }
    }

    #[test]
    fn test_working_directory_must_exist() {
        let manager = SessionManager::new();
        let non_existent = PathBuf::from("/this/path/definitely/does/not/exist/nowhere");

        let result = manager.create_session(non_existent, None);
        assert!(result.is_err());

        if let Err(crate::AgentError::Session(msg)) = result {
            assert!(msg.contains("Working directory validation failed"));
        } else {
            panic!("Expected Session error for non-existent directory");
        }
    }

    #[test]
    fn test_session_has_available_commands_field() {
        let session_id = SessionId::new();
        let cwd = std::env::current_dir().unwrap();
        let session = Session::new(session_id, cwd);

        // Session should have an available_commands field
        assert_eq!(session.available_commands.len(), 0);
    }

    #[test]
    fn test_session_update_available_commands() {
        let session_id = SessionId::new();
        let cwd = std::env::current_dir().unwrap();
        let mut session = Session::new(session_id, cwd);

        let commands = vec![
            agent_client_protocol::AvailableCommand::new(
                "create_plan",
                "Create an execution plan for complex tasks",
            ),
            agent_client_protocol::AvailableCommand::new(
                "research_codebase",
                "Research and analyze the codebase structure",
            ),
        ];

        session.update_available_commands(commands.clone());
        assert_eq!(session.available_commands.len(), 2);
        assert_eq!(session.available_commands[0].name, "create_plan");
        assert_eq!(session.available_commands[1].name, "research_codebase");
    }

    #[test]
    fn test_session_detect_available_commands_changes() {
        let session_id = SessionId::new();
        let cwd = std::env::current_dir().unwrap();
        let mut session = Session::new(session_id, cwd);

        let initial_commands = vec![agent_client_protocol::AvailableCommand::new(
            "create_plan",
            "Create an execution plan for complex tasks",
        )];

        // Set initial commands
        session.update_available_commands(initial_commands.clone());
        assert!(!session.has_available_commands_changed(&initial_commands));

        // Change commands - should detect difference
        let updated_commands = vec![agent_client_protocol::AvailableCommand::new(
            "research_codebase",
            "Research and analyze the codebase structure",
        )];

        assert!(session.has_available_commands_changed(&updated_commands));
    }

    #[test]
    fn test_session_manager_send_available_commands_update() {
        let manager = SessionManager::new();
        let cwd = std::env::current_dir().unwrap();
        let session_id = manager.create_session(cwd, None).unwrap();

        let commands = vec![agent_client_protocol::AvailableCommand::new(
            "create_plan",
            "Create an execution plan for complex tasks",
        )];

        // This should update session and return whether an update was sent
        let update_sent = manager
            .update_available_commands(&session_id, commands)
            .unwrap();
        assert!(update_sent);

        // Same commands again - should not send update
        let commands = vec![agent_client_protocol::AvailableCommand::new(
            "create_plan",
            "Create an execution plan for complex tasks",
        )];
        let update_sent = manager
            .update_available_commands(&session_id, commands)
            .unwrap();
        assert!(!update_sent);
    }

    #[test]
    fn test_session_detect_meta_field_changes() {
        let session_id = SessionId::new();
        let cwd = std::env::current_dir().unwrap();
        let mut session = Session::new(session_id, cwd);

        let mut initial_meta = serde_json::Map::new();
        initial_meta.insert("version".to_string(), serde_json::json!("1.0"));
        let initial_commands = vec![agent_client_protocol::AvailableCommand::new(
            "create_plan",
            "Create an execution plan",
        )
        .meta(initial_meta)];

        session.update_available_commands(initial_commands.clone());
        assert!(!session.has_available_commands_changed(&initial_commands));

        // Change meta field - should detect difference
        let mut updated_meta = serde_json::Map::new();
        updated_meta.insert("version".to_string(), serde_json::json!("2.0"));
        let updated_commands = vec![agent_client_protocol::AvailableCommand::new(
            "create_plan",
            "Create an execution plan",
        )
        .meta(updated_meta)];

        assert!(session.has_available_commands_changed(&updated_commands));
    }

    #[test]
    fn test_session_detect_input_field_changes() {
        let session_id = SessionId::new();
        let cwd = std::env::current_dir().unwrap();
        let mut session = Session::new(session_id, cwd);

        let input_schema = agent_client_protocol::AvailableCommandInput::Unstructured(
            agent_client_protocol::UnstructuredCommandInput::new(
                "Enter task description".to_string(),
            ),
        );

        let initial_commands = vec![agent_client_protocol::AvailableCommand::new(
            "create_plan",
            "Create an execution plan",
        )
        .input(input_schema.clone())];

        session.update_available_commands(initial_commands.clone());
        assert!(!session.has_available_commands_changed(&initial_commands));

        // Change input field - should detect difference
        let updated_input_schema = agent_client_protocol::AvailableCommandInput::Unstructured(
            agent_client_protocol::UnstructuredCommandInput::new(
                "Enter task with priority".to_string(),
            ),
        );

        let updated_commands = vec![agent_client_protocol::AvailableCommand::new(
            "create_plan",
            "Create an execution plan",
        )
        .input(updated_input_schema)];

        assert!(session.has_available_commands_changed(&updated_commands));
    }

    #[test]
    fn test_session_detect_meta_none_to_some_change() {
        let session_id = SessionId::new();
        let cwd = std::env::current_dir().unwrap();
        let mut session = Session::new(session_id, cwd);

        let initial_commands = vec![agent_client_protocol::AvailableCommand::new(
            "create_plan",
            "Create an execution plan",
        )];

        session.update_available_commands(initial_commands.clone());
        assert!(!session.has_available_commands_changed(&initial_commands));

        // Add meta field - should detect difference
        let mut meta_map = serde_json::Map::new();
        meta_map.insert("new".to_string(), serde_json::json!("field"));
        let updated_commands = vec![agent_client_protocol::AvailableCommand::new(
            "create_plan",
            "Create an execution plan",
        )
        .meta(meta_map)];

        assert!(session.has_available_commands_changed(&updated_commands));
    }
}
