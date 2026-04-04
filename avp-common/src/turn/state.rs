//! Turn state persistence with file locking.
//!
//! Each session gets its own state file under `.avp/turn_state/<session_id>.yaml`.
//! This isolates subagent state from parent sessions, preventing race conditions
//! where a subagent's cleanup would wipe the parent's tracked changes.
//!
//! State is cleaned at SessionStart (not Stop), preserving debug evidence and
//! matching the lifecycle of sidecar diff files.
//!
//! The turn state tracks file changes between PreToolUse and PostToolUse hooks,
//! accumulating a list of changed files that is passed to Stop validators.

use crate::error::AvpError;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

/// Subdirectory under `.avp/` for session-scoped sidecar diff files.
const TURN_DIFFS_DIR: &str = "turn_diffs";

/// Subdirectory under `.avp/` for session-scoped pre-content sidecar files.
/// Layout: `turn_pre/<session_id>/<tool_use_id>/<encoded_path>.pre`
///
/// A sentinel file `<encoded_path>.none` marks files that did not exist
/// before the tool ran (new-file case). When a `.pre` file is present the
/// content is the raw bytes of the file before the tool modified it.
const TURN_PRE_DIR: &str = "turn_pre";

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

/// Subdirectory under `.avp/` for per-session turn state files.
const TURN_STATE_DIR: &str = "turn_state";

/// Manages turn state persistence with file locking.
///
/// Each session gets its own state file under `.avp/turn_state/<session_id>.yaml`,
/// isolating subagent state from parent sessions. File locking ensures safe
/// concurrent access within a session.
pub struct TurnStateManager {
    /// Directory for turn state file (.avp/).
    state_dir: PathBuf,
}

impl TurnStateManager {
    /// Sanitize an identifier (session_id or tool_use_id) to prevent path traversal.
    ///
    /// Replaces any character that is not alphanumeric, hyphen, or underscore
    /// with an underscore. This ensures the value is safe to use as a single
    /// path component and cannot contain `..`, `/`, or absolute path prefixes.
    fn sanitize_id(id: &str) -> String {
        id.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }

    /// Create a new TurnStateManager.
    ///
    /// # Arguments
    /// * `cwd` - The current working directory (project root).
    pub fn new(cwd: &Path) -> Self {
        let state_dir = cwd.join(".avp");
        Self { state_dir }
    }

    /// Load turn state for a session, creating empty state if none exists.
    ///
    /// Each session has its own state file under `.avp/turn_state/<session_id>.yaml`.
    pub fn load(&self, session_id: &str) -> Result<TurnState, AvpError> {
        let state_path = self.state_path(session_id);

        // Acquire lock for reading
        let _lock = self.acquire_lock(session_id)?;

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

        let state: TurnState = serde_yaml_ng::from_str(&content).map_err(|e| {
            AvpError::Context(format!(
                "Failed to parse turn state '{}': {}",
                state_path.display(),
                e
            ))
        })?;

        Ok(state)
    }

    /// Save turn state for a session.
    ///
    /// Each session has its own state file under `.avp/turn_state/<session_id>.yaml`.
    pub fn save(&self, session_id: &str, state: &TurnState) -> Result<(), AvpError> {
        let state_path = self.state_path(session_id);

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
        let _lock = self.acquire_lock(session_id)?;

        let content = serde_yaml_ng::to_string(state)
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

        tracing::trace!(session_id, "Saved turn state");
        Ok(())
    }

    /// Clear turn state for a session.
    ///
    /// Removes only the specified session's state file; other sessions are untouched.
    pub fn clear(&self, session_id: &str) -> Result<(), AvpError> {
        let state_path = self.state_path(session_id);
        let lock_path = self.lock_path(session_id);

        // Acquire lock before clearing
        let _lock = self.acquire_lock(session_id)?;

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
            tracing::debug!(session_id, "Cleared turn state");
        }

        // Clean up lock file after state is cleared
        // Drop lock first by ending scope
        drop(_lock);

        if lock_path.exists() {
            let _ = fs::remove_file(&lock_path); // Ignore errors cleaning up lock file
        }

        Ok(())
    }

    // ── Sidecar diff file methods ─────────────────────────────────────

    /// Get the directory for a session's sidecar diff files.
    fn diffs_dir(&self, session_id: &str) -> PathBuf {
        self.state_dir
            .join(TURN_DIFFS_DIR)
            .join(Self::sanitize_id(session_id))
    }

    /// Encode a file path as a sidecar diff filename.
    ///
    /// Uses percent-encoding so the mapping is reversible for any path:
    /// `%` is encoded as `%25`, then `/` is encoded as `%2F`.
    /// A `.diff` suffix is appended.
    fn encode_diff_filename(path: &Path) -> String {
        let s = path.display().to_string();
        // Encode `%` first (so literal `%` in paths survives the round-trip),
        // then encode `/` as `%2F`.
        let encoded = s.replace('%', "%25").replace('/', "%2F");
        format!("{}.diff", encoded)
    }

    /// Decode a sidecar diff filename back to a file path.
    ///
    /// Strips the `.diff` suffix then reverses the percent-encoding:
    /// `%2F` becomes `/`, then `%25` becomes `%`.
    fn decode_diff_filename(filename: &str) -> Option<String> {
        let stem = filename.strip_suffix(".diff")?;
        // Decode in reverse order: `/` first, then `%`.
        let decoded = stem.replace("%2F", "/").replace("%25", "%");
        Some(decoded)
    }

    /// Write a diff for a file into the session's sidecar directory.
    ///
    /// Creates the session directory if needed. Last-writer-wins for the same path.
    pub fn write_diff(
        &self,
        session_id: &str,
        path: &Path,
        diff_text: &str,
    ) -> Result<(), AvpError> {
        let dir = self.diffs_dir(session_id);
        fs::create_dir_all(&dir).map_err(|e| {
            AvpError::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to create diffs directory '{}': {}",
                    dir.display(),
                    e
                ),
            ))
        })?;

        let filename = Self::encode_diff_filename(path);
        let diff_path = dir.join(&filename);

        fs::write(&diff_path, diff_text).map_err(|e| {
            AvpError::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to write diff '{}': {}", diff_path.display(), e),
            ))
        })?;

        tracing::trace!("Wrote diff sidecar: {}", diff_path.display());
        Ok(())
    }

    /// Load a single diff for a file from the session's sidecar directory.
    ///
    /// Returns `None` if the diff file does not exist.
    pub fn load_diff(&self, session_id: &str, path: &Path) -> Option<String> {
        let dir = self.diffs_dir(session_id);
        let filename = Self::encode_diff_filename(path);
        let diff_path = dir.join(&filename);

        fs::read_to_string(&diff_path).ok()
    }

    /// Load all diffs for a session from its sidecar directory.
    ///
    /// Returns a map of original file path (as string) to diff text.
    /// Returns an empty map if the directory does not exist.
    pub fn load_all_diffs(&self, session_id: &str) -> HashMap<String, String> {
        let dir = self.diffs_dir(session_id);
        let mut result = HashMap::new();

        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => return result,
        };

        for entry in entries.flatten() {
            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy();
            if !filename_str.ends_with(".diff") {
                continue;
            }
            if let Some(decoded_path) = Self::decode_diff_filename(&filename_str) {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    result.insert(decoded_path, content);
                }
            }
        }

        result
    }

    /// Clear all diffs for a session by removing its sidecar directory.
    ///
    /// Only removes the specified session's directory; other sessions are untouched.
    pub fn clear_diffs(&self, session_id: &str) -> Result<(), AvpError> {
        let dir = self.diffs_dir(session_id);
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| {
                AvpError::Io(std::io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to remove diffs directory '{}': {}",
                        dir.display(),
                        e
                    ),
                ))
            })?;
            tracing::debug!("Cleared diffs for session {}", session_id);
        }
        Ok(())
    }

    // ── Sidecar pre-content file methods ──────────────────────────────

    /// Get the directory for a session/tool_use_id's pre-content sidecar files.
    fn pre_content_dir(&self, session_id: &str, tool_use_id: &str) -> PathBuf {
        self.state_dir
            .join(TURN_PRE_DIR)
            .join(Self::sanitize_id(session_id))
            .join(Self::sanitize_id(tool_use_id))
    }

    /// Encode a file path as a pre-content sidecar filename.
    ///
    /// Reuses the same percent-encoding as diffs but with a `.pre` suffix.
    fn encode_pre_filename(path: &Path) -> String {
        let s = path.display().to_string();
        let encoded = s.replace('%', "%25").replace('/', "%2F");
        format!("{encoded}.pre")
    }

    /// Encode a path as a sentinel filename for "file did not exist" (new-file case).
    fn encode_pre_none_filename(path: &Path) -> String {
        let s = path.display().to_string();
        let encoded = s.replace('%', "%25").replace('/', "%2F");
        format!("{encoded}.none")
    }

    /// Decode a pre-content sidecar filename back to a file path.
    ///
    /// Accepts both `.pre` and `.none` suffixes.
    fn decode_pre_filename(filename: &str) -> Option<(String, bool)> {
        if let Some(stem) = filename.strip_suffix(".pre") {
            let decoded = stem.replace("%2F", "/").replace("%25", "%");
            Some((decoded, false))
        } else if let Some(stem) = filename.strip_suffix(".none") {
            let decoded = stem.replace("%2F", "/").replace("%25", "%");
            Some((decoded, true))
        } else {
            None
        }
    }

    /// Write pre-execution file content for a path to a sidecar file on disk.
    ///
    /// Content of `None` means the file did not exist before the tool ran;
    /// a `.none` sentinel is written instead of a `.pre` data file.
    ///
    /// This persists across process boundaries so that PostToolUse (which
    /// runs in a different process) can read content that PreToolUse stashed.
    pub fn write_pre_content(
        &self,
        session_id: &str,
        tool_use_id: &str,
        path: &Path,
        content: Option<&[u8]>,
    ) -> Result<(), AvpError> {
        let dir = self.pre_content_dir(session_id, tool_use_id);
        fs::create_dir_all(&dir).map_err(|e| {
            AvpError::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "Failed to create pre-content directory '{}': {}",
                    dir.display(),
                    e
                ),
            ))
        })?;

        match content {
            Some(bytes) => {
                let filename = Self::encode_pre_filename(path);
                let file_path = dir.join(&filename);
                fs::write(&file_path, bytes).map_err(|e| {
                    AvpError::Io(std::io::Error::new(
                        e.kind(),
                        format!(
                            "Failed to write pre-content '{}': {}",
                            file_path.display(),
                            e
                        ),
                    ))
                })?;
                tracing::trace!("Wrote pre-content sidecar: {}", file_path.display());
            }
            None => {
                let filename = Self::encode_pre_none_filename(path);
                let file_path = dir.join(&filename);
                fs::write(&file_path, b"").map_err(|e| {
                    AvpError::Io(std::io::Error::new(
                        e.kind(),
                        format!(
                            "Failed to write pre-content sentinel '{}': {}",
                            file_path.display(),
                            e
                        ),
                    ))
                })?;
                tracing::trace!("Wrote pre-content sentinel: {}", file_path.display());
            }
        }

        Ok(())
    }

    /// Take (read and remove) all pre-content for a session/tool_use_id.
    ///
    /// Returns a map of path -> `Option<Vec<u8>>` where `None` means the file
    /// did not exist before the tool ran. Removes the sidecar directory after
    /// reading so the data is consumed exactly once.
    ///
    /// Returns `None` if no pre-content was stashed for this tool_use_id.
    pub fn take_pre_content(
        &self,
        session_id: &str,
        tool_use_id: &str,
    ) -> Option<HashMap<PathBuf, Option<Vec<u8>>>> {
        let dir = self.pre_content_dir(session_id, tool_use_id);
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => return None,
        };

        let mut result = HashMap::new();
        for entry in entries.flatten() {
            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy();
            if let Some((decoded_path, is_none)) = Self::decode_pre_filename(&filename_str) {
                let path = PathBuf::from(decoded_path);
                if is_none {
                    result.insert(path, None);
                } else {
                    let content = fs::read(entry.path()).ok();
                    result.insert(path, content);
                }
            }
        }

        // Clean up the tool_use_id directory
        let _ = fs::remove_dir_all(&dir);

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    /// Clear all pre-content sidecar files for a session.
    ///
    /// Removes the entire `turn_pre/<session_id>/` directory.
    pub fn clear_pre_content(&self, session_id: &str) -> Result<(), AvpError> {
        let dir = self
            .state_dir
            .join(TURN_PRE_DIR)
            .join(Self::sanitize_id(session_id));
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| {
                AvpError::Io(std::io::Error::new(
                    e.kind(),
                    format!(
                        "Failed to remove pre-content directory '{}': {}",
                        dir.display(),
                        e
                    ),
                ))
            })?;
            tracing::debug!("Cleared pre-content for session {}", session_id);
        }
        Ok(())
    }

    // ── Turn state file methods ─────────────────────────────────────

    /// Get the path to a session's state file.
    fn state_path(&self, session_id: &str) -> PathBuf {
        let safe_id = Self::sanitize_id(session_id);
        self.state_dir
            .join(TURN_STATE_DIR)
            .join(format!("{}.yaml", safe_id))
    }

    /// Get the path to a session's lock file.
    fn lock_path(&self, session_id: &str) -> PathBuf {
        let safe_id = Self::sanitize_id(session_id);
        self.state_dir
            .join(TURN_STATE_DIR)
            .join(format!("{}.yaml.lock", safe_id))
    }

    /// Acquire an exclusive lock for a session's state file.
    ///
    /// Returns a File handle that holds the lock. The lock is automatically
    /// released when the File handle is dropped (RAII pattern).
    fn acquire_lock(&self, session_id: &str) -> Result<File, AvpError> {
        let lock_path = self.lock_path(session_id);

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

        manager.save("session-1", &state).unwrap();

        // Same session_id returns the state
        let loaded = manager.load("session-1").unwrap();
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

        // Clear only affects the specified session
        manager.clear("session-1").unwrap();

        let loaded = manager.load("session-1").unwrap();
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
    fn test_two_sessions_write_read_independently() {
        let (manager, _temp_dir) = setup_test_manager();

        // Save from "session-1"
        let mut state1 = TurnState::new();
        state1.changed.push(PathBuf::from("/file1.rs"));
        manager.save("session-1", &state1).unwrap();

        // Save from "session-2"
        let mut state2 = TurnState::new();
        state2.changed.push(PathBuf::from("/file2.rs"));
        manager.save("session-2", &state2).unwrap();

        // Each session sees only its own data
        let loaded1 = manager.load("session-1").unwrap();
        assert_eq!(loaded1.changed.len(), 1);
        assert_eq!(loaded1.changed[0], PathBuf::from("/file1.rs"));

        let loaded2 = manager.load("session-2").unwrap();
        assert_eq!(loaded2.changed.len(), 1);
        assert_eq!(loaded2.changed[0], PathBuf::from("/file2.rs"));
    }

    #[test]
    fn test_clear_for_session_a_does_not_affect_session_b() {
        let (manager, _temp_dir) = setup_test_manager();

        // Both sessions save state
        let mut state1 = TurnState::new();
        state1.changed.push(PathBuf::from("/file1.rs"));
        manager.save("session-a", &state1).unwrap();

        let mut state2 = TurnState::new();
        state2.changed.push(PathBuf::from("/file2.rs"));
        manager.save("session-b", &state2).unwrap();

        // Clear session-a
        manager.clear("session-a").unwrap();

        // session-a is empty
        let loaded_a = manager.load("session-a").unwrap();
        assert!(loaded_a.changed.is_empty());

        // session-b is untouched
        let loaded_b = manager.load("session-b").unwrap();
        assert_eq!(loaded_b.changed.len(), 1);
        assert_eq!(loaded_b.changed[0], PathBuf::from("/file2.rs"));
    }

    // ── Sidecar pre-content tests ────────────────────────────────────

    #[test]
    fn test_write_and_take_pre_content_roundtrip() {
        let (manager, _temp_dir) = setup_test_manager();

        let path = PathBuf::from("/test/file.rs");
        let content = b"fn main() {}";

        manager
            .write_pre_content("session-1", "tool-1", &path, Some(content))
            .unwrap();

        let taken = manager.take_pre_content("session-1", "tool-1");
        assert!(taken.is_some());
        let map = taken.unwrap();
        assert_eq!(map.get(&path).unwrap().as_ref().unwrap(), content);

        // Second take returns None (consumed / directory removed)
        assert!(manager.take_pre_content("session-1", "tool-1").is_none());
    }

    #[test]
    fn test_take_pre_content_none_for_new_file() {
        let (manager, _temp_dir) = setup_test_manager();

        let path = PathBuf::from("/test/new_file.rs");
        manager
            .write_pre_content("session-1", "tool-1", &path, None)
            .unwrap();

        let taken = manager.take_pre_content("session-1", "tool-1").unwrap();
        assert!(taken.get(&path).unwrap().is_none());
    }

    #[test]
    fn test_take_pre_content_cleans_up_sidecar_files() {
        let (manager, _temp_dir) = setup_test_manager();

        let path = PathBuf::from("/test/file.rs");
        manager
            .write_pre_content("session-1", "tool-1", &path, Some(b"data"))
            .unwrap();

        // Verify directory exists before take
        let dir = manager.pre_content_dir("session-1", "tool-1");
        assert!(dir.exists());

        let _ = manager.take_pre_content("session-1", "tool-1");

        // Directory should be gone after take
        assert!(!dir.exists());
    }

    #[test]
    fn test_clear_pre_content_removes_session_dir() {
        let (manager, _temp_dir) = setup_test_manager();

        manager
            .write_pre_content("session-1", "tool-1", &PathBuf::from("/a.rs"), Some(b"a"))
            .unwrap();
        manager
            .write_pre_content("session-1", "tool-2", &PathBuf::from("/b.rs"), Some(b"b"))
            .unwrap();

        manager.clear_pre_content("session-1").unwrap();

        // Both tool_use_ids are gone
        assert!(manager.take_pre_content("session-1", "tool-1").is_none());
        assert!(manager.take_pre_content("session-1", "tool-2").is_none());
    }

    #[test]
    fn test_clear_pre_content_does_not_affect_other_session() {
        let (manager, _temp_dir) = setup_test_manager();

        manager
            .write_pre_content("session-a", "tool-1", &PathBuf::from("/a.rs"), Some(b"a"))
            .unwrap();
        manager
            .write_pre_content("session-b", "tool-2", &PathBuf::from("/b.rs"), Some(b"b"))
            .unwrap();

        manager.clear_pre_content("session-a").unwrap();

        // session-a is gone
        assert!(manager.take_pre_content("session-a", "tool-1").is_none());

        // session-b is untouched
        let taken = manager.take_pre_content("session-b", "tool-2");
        assert!(taken.is_some());
        let map = taken.unwrap();
        assert_eq!(
            map.get(&PathBuf::from("/b.rs")).unwrap().as_ref().unwrap(),
            b"b"
        );
    }

    #[test]
    fn test_pre_content_survives_across_manager_instances() {
        // Simulates separate processes: process A writes, process B reads.
        let temp_dir = TempDir::new().unwrap();

        // Process A: PreToolUse writes pre-content
        let manager_a = TurnStateManager::new(temp_dir.path());
        let path = PathBuf::from("/test/file.rs");
        let content = b"original content";
        manager_a
            .write_pre_content("session-1", "tool-1", &path, Some(content))
            .unwrap();

        // Process B: PostToolUse reads pre-content from a NEW manager instance
        let manager_b = TurnStateManager::new(temp_dir.path());
        let taken = manager_b.take_pre_content("session-1", "tool-1");
        assert!(taken.is_some());
        let map = taken.unwrap();
        assert_eq!(map.get(&path).unwrap().as_ref().unwrap(), content);
    }

    #[test]
    fn test_pre_content_multiple_files_per_tool() {
        let (manager, _temp_dir) = setup_test_manager();

        let path1 = PathBuf::from("/test/a.rs");
        let path2 = PathBuf::from("/test/b.rs");
        manager
            .write_pre_content("s1", "t1", &path1, Some(b"content-a"))
            .unwrap();
        manager
            .write_pre_content("s1", "t1", &path2, Some(b"content-b"))
            .unwrap();

        let taken = manager.take_pre_content("s1", "t1").unwrap();
        assert_eq!(taken.len(), 2);
        assert_eq!(
            taken.get(&path1).unwrap().as_ref().unwrap(),
            &b"content-a"[..]
        );
        assert_eq!(
            taken.get(&path2).unwrap().as_ref().unwrap(),
            &b"content-b"[..]
        );
    }

    #[test]
    fn test_encode_decode_pre_filename() {
        let path = Path::new("/src/lib/foo.rs");
        let encoded = TurnStateManager::encode_pre_filename(path);
        assert_eq!(encoded, "%2Fsrc%2Flib%2Ffoo.rs.pre");

        let (decoded, is_none) = TurnStateManager::decode_pre_filename(&encoded).unwrap();
        assert_eq!(decoded, "/src/lib/foo.rs");
        assert!(!is_none);
    }

    #[test]
    fn test_encode_decode_pre_none_filename() {
        let path = Path::new("/src/new_file.rs");
        let encoded = TurnStateManager::encode_pre_none_filename(path);
        assert_eq!(encoded, "%2Fsrc%2Fnew_file.rs.none");

        let (decoded, is_none) = TurnStateManager::decode_pre_filename(&encoded).unwrap();
        assert_eq!(decoded, "/src/new_file.rs");
        assert!(is_none);
    }

    #[test]
    fn test_decode_pre_filename_rejects_no_suffix() {
        assert!(TurnStateManager::decode_pre_filename("no_suffix").is_none());
    }

    // ── Sidecar diff tests ──────────────────────────────────────────

    #[test]
    fn test_write_diff_and_load_diff_roundtrip() {
        let (manager, _temp_dir) = setup_test_manager();
        let path = Path::new("/src/main.rs");
        let diff_text = "--- /src/main.rs\n+++ /src/main.rs\n@@ -1 +1 @@\n-old\n+new\n";

        manager.write_diff("session-1", path, diff_text).unwrap();

        let loaded = manager.load_diff("session-1", path);
        assert_eq!(loaded.as_deref(), Some(diff_text));
    }

    #[test]
    fn test_write_diff_and_load_all_diffs_roundtrip() {
        let (manager, _temp_dir) = setup_test_manager();

        manager
            .write_diff("session-1", Path::new("/src/main.rs"), "diff-main")
            .unwrap();
        manager
            .write_diff("session-1", Path::new("/src/lib/foo.rs"), "diff-foo")
            .unwrap();

        let all = manager.load_all_diffs("session-1");
        assert_eq!(all.len(), 2);
        assert_eq!(all.get("/src/main.rs").unwrap(), "diff-main");
        assert_eq!(all.get("/src/lib/foo.rs").unwrap(), "diff-foo");
    }

    #[test]
    fn test_write_diff_overwrite_keeps_latest() {
        let (manager, _temp_dir) = setup_test_manager();
        let path = Path::new("/src/main.rs");

        manager.write_diff("session-1", path, "first").unwrap();
        manager.write_diff("session-1", path, "second").unwrap();

        let loaded = manager.load_diff("session-1", path);
        assert_eq!(loaded.as_deref(), Some("second"));
    }

    #[test]
    fn test_clear_diffs_removes_only_session() {
        let (manager, _temp_dir) = setup_test_manager();

        manager
            .write_diff("session-1", Path::new("/a.rs"), "diff-a")
            .unwrap();
        manager
            .write_diff("session-2", Path::new("/b.rs"), "diff-b")
            .unwrap();

        manager.clear_diffs("session-1").unwrap();

        // session-1's diffs gone
        assert!(manager.load_all_diffs("session-1").is_empty());

        // session-2's diffs intact
        let s2 = manager.load_all_diffs("session-2");
        assert_eq!(s2.len(), 1);
        assert_eq!(s2.get("/b.rs").unwrap(), "diff-b");
    }

    #[test]
    fn test_two_sessions_isolated() {
        let (manager, _temp_dir) = setup_test_manager();

        manager
            .write_diff("parent", Path::new("/src/main.rs"), "parent-diff")
            .unwrap();
        manager
            .write_diff("child", Path::new("/tests/test.rs"), "child-diff")
            .unwrap();

        let parent_diffs = manager.load_all_diffs("parent");
        assert_eq!(parent_diffs.len(), 1);
        assert!(parent_diffs.contains_key("/src/main.rs"));

        let child_diffs = manager.load_all_diffs("child");
        assert_eq!(child_diffs.len(), 1);
        assert!(child_diffs.contains_key("/tests/test.rs"));
    }

    #[test]
    fn test_load_all_diffs_empty_dir() {
        let (manager, _temp_dir) = setup_test_manager();
        let diffs = manager.load_all_diffs("nonexistent-session");
        assert!(diffs.is_empty());
    }

    #[test]
    fn test_encode_decode_diff_filename() {
        let path = Path::new("/src/lib/foo.rs");
        let encoded = TurnStateManager::encode_diff_filename(path);
        assert_eq!(encoded, "%2Fsrc%2Flib%2Ffoo.rs.diff");

        let decoded = TurnStateManager::decode_diff_filename(&encoded);
        assert_eq!(decoded.as_deref(), Some("/src/lib/foo.rs"));
    }

    #[test]
    fn test_encode_decode_diff_filename_double_underscores() {
        // Paths with `__` must round-trip correctly (this was the original bug).
        let path = Path::new("/src/__init__.py");
        let encoded = TurnStateManager::encode_diff_filename(path);
        let decoded = TurnStateManager::decode_diff_filename(&encoded);
        assert_eq!(decoded.as_deref(), Some("/src/__init__.py"));
    }

    #[test]
    fn test_encode_decode_diff_filename_percent_sign() {
        // Paths containing a literal `%` must also round-trip.
        let path = Path::new("/src/100%done.rs");
        let encoded = TurnStateManager::encode_diff_filename(path);
        let decoded = TurnStateManager::decode_diff_filename(&encoded);
        assert_eq!(decoded.as_deref(), Some("/src/100%done.rs"));
    }

    #[test]
    fn test_decode_diff_filename_rejects_no_suffix() {
        assert_eq!(TurnStateManager::decode_diff_filename("no_suffix"), None);
    }
}
