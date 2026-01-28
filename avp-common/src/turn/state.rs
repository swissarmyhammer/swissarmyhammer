//! Turn state persistence with file locking.
//!
//! This module uses a SINGLE project-wide state file instead of per-session files.
//! All tool_use_ids are globally unique, so we don't need session-based separation.
//! This avoids an explosion of state files when subagents are spawned.
//!
//! The turn state tracks file changes between PreToolUse and PostToolUse hooks,
//! accumulating a list of changed files that is passed to Stop validators.

use crate::error::AvpError;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

/// State tracked during a turn for file change detection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TurnState {
    /// Pending file hashes, keyed by tool_use_id.
    /// Each tool_use_id maps to a set of file paths and their pre-execution hashes.
    /// A hash of `None` indicates the file did not exist before the tool ran.
    #[serde(default)]
    pub pending: HashMap<String, HashMap<PathBuf, Option<String>>>,

    /// Files that have been confirmed as changed during this turn.
    #[serde(default)]
    pub changed: Vec<PathBuf>,
}

impl TurnState {
    /// Create a new empty turn state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there are any changed files.
    pub fn has_changes(&self) -> bool {
        !self.changed.is_empty()
    }

    /// Get the list of changed files as strings.
    pub fn changed_files_as_strings(&self) -> Vec<String> {
        self.changed
            .iter()
            .map(|p| p.display().to_string())
            .collect()
    }
}

/// Name of the single project-wide turn state file.
const TURN_STATE_FILE: &str = "turn_state.yaml";
/// Name of the lock file for the turn state.
const TURN_STATE_LOCK: &str = "turn_state.yaml.lock";

/// Manages turn state persistence with file locking.
///
/// Uses a single project-wide state file instead of per-session files.
/// This avoids file explosion with subagents while still supporting
/// concurrent access via file locking.
pub struct TurnStateManager {
    /// Directory for turn state file (.avp/).
    state_dir: PathBuf,
}

impl TurnStateManager {
    /// Create a new TurnStateManager.
    ///
    /// # Arguments
    /// * `cwd` - The current working directory (project root).
    pub fn new(cwd: &Path) -> Self {
        let state_dir = cwd.join(".avp");
        Self { state_dir }
    }

    /// Load turn state, creating empty state if none exists.
    ///
    /// Note: The session_id parameter is kept for API compatibility but
    /// is ignored - all sessions share the same project-wide state file.
    pub fn load(&self, _session_id: &str) -> Result<TurnState, AvpError> {
        let state_path = self.state_path();

        // Acquire lock for reading
        let _lock = self.acquire_lock()?;

        if !state_path.exists() {
            return Ok(TurnState::new());
        }

        let content = fs::read_to_string(&state_path).map_err(|e| {
            AvpError::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to read turn state '{}': {}",
                    state_path.display(),
                    e
                ),
            ))
        })?;

        let state: TurnState = serde_yaml::from_str(&content).map_err(|e| {
            AvpError::Context(format!(
                "Failed to parse turn state '{}': {}",
                state_path.display(),
                e
            ))
        })?;

        Ok(state)
    }

    /// Save turn state.
    ///
    /// Note: The session_id parameter is kept for API compatibility but
    /// is ignored - all sessions share the same project-wide state file.
    pub fn save(&self, _session_id: &str, state: &TurnState) -> Result<(), AvpError> {
        let state_path = self.state_path();

        // Ensure directory exists
        if let Some(parent) = state_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AvpError::Io(std::io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to create turn state directory '{}': {}",
                        parent.display(),
                        e
                    ),
                ))
            })?;
        }

        // Acquire lock for writing
        let _lock = self.acquire_lock()?;

        let content = serde_yaml::to_string(state)
            .map_err(|e| AvpError::Context(format!("Failed to serialize turn state: {}", e)))?;

        fs::write(&state_path, content).map_err(|e| {
            AvpError::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to write turn state '{}': {}",
                    state_path.display(),
                    e
                ),
            ))
        })?;

        tracing::trace!("Saved turn state");
        Ok(())
    }

    /// Clear turn state.
    ///
    /// Note: The session_id parameter is kept for API compatibility but
    /// is ignored - clears the single project-wide state file.
    pub fn clear(&self, _session_id: &str) -> Result<(), AvpError> {
        let state_path = self.state_path();
        let lock_path = self.lock_path();

        // Acquire lock before clearing
        let _lock = self.acquire_lock()?;

        if state_path.exists() {
            fs::remove_file(&state_path).map_err(|e| {
                AvpError::Io(std::io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to remove turn state '{}': {}",
                        state_path.display(),
                        e
                    ),
                ))
            })?;
            tracing::debug!("Cleared turn state");
        }

        // Clean up lock file after state is cleared
        // Drop lock first by ending scope
        drop(_lock);

        if lock_path.exists() {
            let _ = fs::remove_file(&lock_path); // Ignore errors cleaning up lock file
        }

        Ok(())
    }

    /// Get the path to the single state file.
    fn state_path(&self) -> PathBuf {
        self.state_dir.join(TURN_STATE_FILE)
    }

    /// Get the path to the lock file.
    fn lock_path(&self) -> PathBuf {
        self.state_dir.join(TURN_STATE_LOCK)
    }

    /// Acquire an exclusive lock for the state file.
    ///
    /// Returns a File handle that holds the lock. The lock is automatically
    /// released when the File handle is dropped (RAII pattern).
    fn acquire_lock(&self) -> Result<File, AvpError> {
        let lock_path = self.lock_path();

        // Ensure parent directory exists for lock file
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AvpError::Io(std::io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to create turn state lock directory '{}': {}",
                        parent.display(),
                        e
                    ),
                ))
            })?;
        }

        // Create or open the lock file
        let lock_file = File::create(&lock_path).map_err(|e| {
            AvpError::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to create turn state lock file '{}': {}",
                    lock_path.display(),
                    e
                ),
            ))
        })?;

        // Acquire exclusive lock (blocks until available)
        lock_file.lock_exclusive().map_err(|e| {
            AvpError::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to acquire turn state lock '{}': {}",
                    lock_path.display(),
                    e
                ),
            ))
        })?;

        tracing::trace!("Acquired turn state lock");

        Ok(lock_file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_manager() -> (TurnStateManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let manager = TurnStateManager::new(temp_dir.path());
        (manager, temp_dir)
    }

    #[test]
    fn test_load_nonexistent_returns_empty() {
        let (manager, _temp_dir) = setup_test_manager();
        // session_id is ignored - uses single project-wide file
        let state = manager.load("any-session").unwrap();
        assert!(state.pending.is_empty());
        assert!(state.changed.is_empty());
    }

    #[test]
    fn test_save_and_load() {
        let (manager, _temp_dir) = setup_test_manager();

        let mut state = TurnState::new();
        state.changed.push(PathBuf::from("/path/to/file.rs"));

        let mut pending_hashes = HashMap::new();
        pending_hashes.insert(
            PathBuf::from("/path/to/other.rs"),
            Some("sha256:abc123".to_string()),
        );
        state.pending.insert("tool_123".to_string(), pending_hashes);

        // session_id is ignored - uses single project-wide file
        manager.save("session-1", &state).unwrap();

        // Loading with different session_id returns same state
        let loaded = manager.load("session-2").unwrap();
        assert_eq!(loaded.changed.len(), 1);
        assert_eq!(loaded.changed[0], PathBuf::from("/path/to/file.rs"));
        assert!(loaded.pending.contains_key("tool_123"));
    }

    #[test]
    fn test_clear() {
        let (manager, _temp_dir) = setup_test_manager();

        let mut state = TurnState::new();
        state.changed.push(PathBuf::from("/path/to/file.rs"));
        manager.save("session-1", &state).unwrap();

        // Clear with any session_id clears the single state file
        manager.clear("session-2").unwrap();

        let loaded = manager.load("session-3").unwrap();
        assert!(loaded.pending.is_empty());
        assert!(loaded.changed.is_empty());
    }

    #[test]
    fn test_has_changes() {
        let mut state = TurnState::new();
        assert!(!state.has_changes());

        state.changed.push(PathBuf::from("/path/to/file.rs"));
        assert!(state.has_changes());
    }

    #[test]
    fn test_changed_files_as_strings() {
        let mut state = TurnState::new();
        state.changed.push(PathBuf::from("/path/to/file.rs"));
        state.changed.push(PathBuf::from("/path/to/other.rs"));

        let strings = state.changed_files_as_strings();
        assert_eq!(strings.len(), 2);
        assert!(strings.contains(&"/path/to/file.rs".to_string()));
        assert!(strings.contains(&"/path/to/other.rs".to_string()));
    }

    #[test]
    fn test_single_file_shared_across_sessions() {
        let (manager, _temp_dir) = setup_test_manager();

        // Save from "session-1"
        let mut state = TurnState::new();
        state.changed.push(PathBuf::from("/file1.rs"));
        manager.save("session-1", &state).unwrap();

        // Load from "session-2" - should see the same data
        let loaded = manager.load("session-2").unwrap();
        assert_eq!(loaded.changed.len(), 1);
        assert_eq!(loaded.changed[0], PathBuf::from("/file1.rs"));

        // Update from "session-2"
        let mut state = loaded;
        state.changed.push(PathBuf::from("/file2.rs"));
        manager.save("session-2", &state).unwrap();

        // Load from "session-1" - should see both files
        let loaded = manager.load("session-1").unwrap();
        assert_eq!(loaded.changed.len(), 2);
    }
}
