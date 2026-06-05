//! Shared per-session raw JSON-RPC transcript recording for ACP agents.
//!
//! This module consolidates what used to be two near-identical
//! `RawMessageManager` implementations (one in `claude-agent`, one in
//! `llama-agent`) into a single shared implementation.
//!
//! A [`RawMessageManager`] records every raw JSON-RPC frame flowing through an
//! ACP session to an append-only, line-delimited JSON file. Writes are
//! serialized through an mpsc channel onto a single background writer task, so
//! concurrent agents (a root agent and its subagents) can share one manager
//! without locking or interleaved output.
//!
//! Transcripts live under a per-session directory resolved by
//! [`acp_session_dir`] — `<XDG_STATE_HOME>/acp/<session-ulid>/raw.jsonl`. The
//! session ULID is globally unique and lexically time-sortable, so the global
//! state directory is collision-free across projects. The session-record store
//! reuses [`acp_session_dir`] so the raw trace (`raw.jsonl`) and the session
//! record (`session.json`) are siblings inside the same per-session directory.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

/// File name for the raw JSON-RPC transcript within a session directory.
const RAW_TRANSCRIPT_FILE: &str = "raw.jsonl";

/// Global registry of [`RawMessageManager`]s keyed by root session ULID.
///
/// This lets subagents look up and share their root agent's manager so every
/// agent in a session hierarchy writes to the same transcript file.
static RAW_MESSAGE_MANAGERS: LazyLock<Mutex<HashMap<String, RawMessageManager>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Resolve the per-session ACP state directory for the given session ULID.
///
/// Returns `<XDG_STATE_HOME>/acp/<session-ulid>/`, falling back to
/// `~/.local/state/acp/<session-ulid>/` when `$XDG_STATE_HOME` is unset. The
/// directory is created on demand.
///
/// The raw transcript (`raw.jsonl`) and the session record (`session.json`)
/// are siblings inside this directory — the session-record store reuses this
/// exact helper.
///
/// The session id is used as a single path component. It is treated as an
/// opaque string — its *format* is never validated as a ULID — but it must be
/// safe to use as one path segment: an id containing a path separator or a
/// `.`/`..` component would resolve outside `acp/`. Such ids are rejected via
/// [`session_path_component`]. This is path-safety validation, not format
/// validation, so it does not conflict with the opaque-id contract.
///
/// # Parameters
///
/// * `session_ulid` - The root session ULID. ULIDs are globally unique and
///   lexically time-sortable, so the global directory is collision-free across
///   projects.
///
/// # Errors
///
/// Returns an [`std::io::Error`] if the home directory cannot be determined,
/// the session id is not a safe single path component, or the directory cannot
/// be created.
pub fn acp_session_dir(session_ulid: &str) -> std::io::Result<PathBuf> {
    let component = session_path_component(session_ulid)?;
    let base = swissarmyhammer_directory::xdg_state_dir()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let dir = base.join("acp").join(component);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Resolve the per-session raw JSON-RPC transcript path (`raw.jsonl`).
///
/// This is the exact path [`RawMessageManager::new`] writes to: the
/// `raw.jsonl` sibling of the session record inside [`acp_session_dir`]. It is
/// exposed so callers that need the *path* without owning the writer — e.g. a
/// hook layer that hands the transcript location to a `.claude` command hook —
/// resolve it through the same helper rather than re-deriving the filename.
///
/// # Parameters
///
/// * `session_ulid` - The root session ULID identifying the transcript
///   directory.
///
/// # Errors
///
/// Returns an [`std::io::Error`] if the session directory cannot be resolved or
/// created (see [`acp_session_dir`]).
pub fn raw_transcript_path(session_ulid: &str) -> std::io::Result<PathBuf> {
    Ok(acp_session_dir(session_ulid)?.join(RAW_TRANSCRIPT_FILE))
}

/// Validate that a session id is safe to use as a single filesystem path
/// component, returning it unchanged when so.
///
/// A session id is rejected when it is empty, equal to `.` or `..`, or contains
/// a path separator (`/`, or `\` on Windows). Any such id could resolve a
/// session directory outside the `acp/` state root. Empty and `.`/`..` ids
/// would alias the `acp/` directory itself or its parent.
///
/// This validates *path safety*, not id *format*: a non-ULID id that is still a
/// single safe path component (e.g. `my-session`) passes. The session id
/// remains an opaque string.
///
/// # Parameters
///
/// * `session_id` - The session id to validate as a path component.
///
/// # Errors
///
/// Returns an [`std::io::Error`] with kind [`std::io::ErrorKind::InvalidInput`]
/// when the id cannot be safely used as a single path component.
fn session_path_component(session_id: &str) -> std::io::Result<&str> {
    let unsafe_component = session_id.is_empty()
        || session_id == "."
        || session_id == ".."
        || session_id.contains('/')
        || session_id.contains('\\');
    if unsafe_component {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("session id is not a safe path component: {session_id:?}"),
        ));
    }
    Ok(session_id)
}

/// Manager for recording raw JSON-RPC messages to a per-session transcript.
///
/// Manages centralized recording of raw JSON-RPC frames from multiple agents
/// (a root agent and its subagents) to a single file. This ensures a complete
/// transcript of all message traffic without race conditions or truncation.
///
/// Writes are serialized through an mpsc channel onto a single background
/// writer task. Because there is exactly one writer, the file needs no lock.
///
/// The manager is cheap to [`Clone`] — clones share the same channel and
/// therefore the same writer task and transcript file.
#[derive(Debug, Clone)]
pub struct RawMessageManager {
    /// Channel for sending raw JSON-RPC messages to the writer task.
    sender: tokio::sync::mpsc::UnboundedSender<String>,
}

impl RawMessageManager {
    /// Create a new raw message manager for the given session ULID.
    ///
    /// Resolves the per-session directory via [`acp_session_dir`], opens
    /// `raw.jsonl` in append mode, and spawns a background task that writes
    /// queued messages sequentially, flushing after each write.
    ///
    /// The constructor takes the session ULID rather than a fixed path: the
    /// manager is created at `new_session` time, once the ULID is known, not
    /// at agent construction time.
    ///
    /// # Parameters
    ///
    /// * `session_ulid` - The root session ULID identifying the transcript
    ///   directory.
    ///
    /// # Returns
    ///
    /// A `RawMessageManager` that can be cloned and shared across agents.
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] if the session directory cannot be
    /// resolved or created, or the transcript file cannot be opened.
    pub fn new(session_ulid: &str) -> std::io::Result<Self> {
        Self::with_path(raw_transcript_path(session_ulid)?)
    }

    /// Create a raw message manager that writes to an explicit file path.
    ///
    /// This is the path-based core used by [`RawMessageManager::new`] and is
    /// exposed primarily for tests that need a transcript at a known location.
    ///
    /// # Parameters
    ///
    /// * `path` - Path to the transcript file (created/appended to).
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::Error`] if the file cannot be opened in append
    /// mode.
    pub fn with_path(path: PathBuf) -> std::io::Result<Self> {
        use std::fs::OpenOptions;
        use std::io::Write;

        // Open the file in append mode so subagents sharing this manager all
        // append to the same transcript without truncating each other.
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;

        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<String>();

        // Single writer task: no lock needed because there is one owner of the
        // file handle. The task exits when every sender clone is dropped.
        tokio::task::spawn(async move {
            while let Some(message) = receiver.recv().await {
                if let Err(e) = writeln!(file, "{message}") {
                    tracing::warn!("Failed to write raw message to file: {e}");
                }
                // Flush after each write so the transcript is durable even if
                // the process is killed mid-session.
                if let Err(e) = file.flush() {
                    tracing::warn!("Failed to flush raw message file: {e}");
                }
            }
        });

        Ok(Self { sender })
    }

    /// Register a manager for a root session ULID so subagents can share it.
    ///
    /// # Parameters
    ///
    /// * `session_ulid` - The root session ULID to key the manager under.
    /// * `manager` - The manager to register.
    pub fn register(session_ulid: String, manager: RawMessageManager) {
        if let Ok(mut registry) = RAW_MESSAGE_MANAGERS.lock() {
            registry.insert(session_ulid, manager);
        }
    }

    /// Look up a previously registered manager by root session ULID.
    ///
    /// Returns `None` if no manager is registered for the ULID.
    pub fn lookup(session_ulid: &str) -> Option<RawMessageManager> {
        RAW_MESSAGE_MANAGERS
            .lock()
            .ok()
            .and_then(|registry| registry.get(session_ulid).cloned())
    }

    /// Record a raw JSON-RPC message.
    ///
    /// Queues the message for the writer task to append. Non-blocking — returns
    /// immediately after enqueuing.
    ///
    /// # Parameters
    ///
    /// * `message` - The raw JSON-RPC message string to record.
    pub fn record(&self, message: String) {
        if let Err(e) = self.sender.send(message) {
            tracing::warn!("Failed to send raw message to recorder: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;

    /// Messages recorded through the manager are written and flushed to the
    /// transcript file in order, and can be read back.
    #[tokio::test]
    async fn test_raw_message_manager_write_flush_read_back() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("test_raw_messages.jsonl");

        let manager = RawMessageManager::with_path(test_file.clone()).unwrap();

        manager.record(r#"{"type":"init","session":"test1"}"#.to_string());
        manager.record(r#"{"type":"prompt","content":"hello"}"#.to_string());

        // Drop the manager so the writer task drains and exits.
        drop(manager);

        // Give the writer task time to flush the queued messages.
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        assert!(test_file.exists(), "Transcript file was not created");

        let contents = fs::read_to_string(&test_file).unwrap();
        assert!(contents.contains(r#"{"type":"init","session":"test1"}"#));
        assert!(contents.contains(r#"{"type":"prompt","content":"hello"}"#));
    }

    /// `acp_session_dir` resolves under `$XDG_STATE_HOME/acp/<ulid>` and
    /// creates the directory on demand.
    ///
    /// Serialized: mutates the process-global `XDG_STATE_HOME` env var.
    #[test]
    #[serial]
    fn test_acp_session_dir_resolves_and_creates() {
        let temp = tempfile::tempdir().unwrap();
        let xdg_state = temp.path().join("state");

        // SAFETY: single-threaded test; restores the env var afterwards.
        let previous = std::env::var_os("XDG_STATE_HOME");
        std::env::set_var("XDG_STATE_HOME", &xdg_state);
        let dir = acp_session_dir("01ARZ3NDEKTSV4RRFFQ69G5FAV");
        match previous {
            Some(value) => std::env::set_var("XDG_STATE_HOME", value),
            None => std::env::remove_var("XDG_STATE_HOME"),
        }

        let dir = dir.unwrap();
        assert_eq!(
            dir,
            xdg_state.join("acp").join("01ARZ3NDEKTSV4RRFFQ69G5FAV")
        );
        assert!(
            dir.is_dir(),
            "session directory should be created on demand"
        );
    }

    /// A manager built from a session ULID writes its transcript to
    /// `<acp-session-dir>/raw.jsonl`.
    ///
    /// Serialized: mutates the process-global `XDG_STATE_HOME` env var.
    #[tokio::test]
    #[serial]
    async fn test_new_writes_to_session_raw_jsonl() {
        let temp = tempfile::tempdir().unwrap();
        let xdg_state = temp.path().join("state");
        let ulid = "01ARZ3NDEKTSV4RRFFQ69G5FB0";

        let previous = std::env::var_os("XDG_STATE_HOME");
        std::env::set_var("XDG_STATE_HOME", &xdg_state);
        let manager = RawMessageManager::new(ulid);
        match previous {
            Some(value) => std::env::set_var("XDG_STATE_HOME", value),
            None => std::env::remove_var("XDG_STATE_HOME"),
        }

        let manager = manager.unwrap();
        manager.record(r#"{"type":"init"}"#.to_string());
        drop(manager);

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let expected = xdg_state.join("acp").join(ulid).join("raw.jsonl");
        assert!(
            expected.exists(),
            "transcript should be at <session-dir>/raw.jsonl"
        );
        let contents = fs::read_to_string(&expected).unwrap();
        assert!(contents.contains(r#"{"type":"init"}"#));
    }

    /// A registered manager can be looked up by root session ULID, and the
    /// lookup result shares the same transcript as the original.
    #[tokio::test]
    async fn test_register_and_lookup_share_transcript() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("registry_transcript.jsonl");
        let session_ulid = "01ARZ3NDEKTSV4RRFFQ69G5REG".to_string();

        let manager = RawMessageManager::with_path(test_file.clone()).unwrap();
        RawMessageManager::register(session_ulid.clone(), manager.clone());

        let looked_up =
            RawMessageManager::lookup(&session_ulid).expect("manager should be registered");

        // The root manager and the looked-up clone write to the same file.
        manager.record(r#"{"agent":"root"}"#.to_string());
        looked_up.record(r#"{"agent":"subagent"}"#.to_string());
        drop(manager);
        drop(looked_up);

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let contents = fs::read_to_string(&test_file).unwrap();
        assert!(contents.contains(r#"{"agent":"root"}"#));
        assert!(contents.contains(r#"{"agent":"subagent"}"#));
    }

    /// `lookup` returns `None` for a ULID that was never registered.
    #[test]
    fn test_lookup_unregistered_returns_none() {
        assert!(RawMessageManager::lookup("01ARZ3NDEKTSV4RRFFQ69NEVER0").is_none());
    }

    /// `acp_session_dir` rejects session ids that are not a safe single path
    /// component, so a client-influenced id cannot escape the `acp/` root.
    ///
    /// Serialized: mutates the process-global `XDG_STATE_HOME` env var.
    #[test]
    #[serial]
    fn test_acp_session_dir_rejects_path_escaping_ids() {
        let temp = tempfile::tempdir().unwrap();
        let xdg_state = temp.path().join("state");

        // SAFETY: single-threaded test; restores the env var afterwards.
        let previous = std::env::var_os("XDG_STATE_HOME");
        std::env::set_var("XDG_STATE_HOME", &xdg_state);
        let results: Vec<(&str, std::io::Result<PathBuf>)> =
            ["../../etc", "..", ".", "", "nested/child", "a\\b"]
                .into_iter()
                .map(|id| (id, acp_session_dir(id)))
                .collect();
        match previous {
            Some(value) => std::env::set_var("XDG_STATE_HOME", value),
            None => std::env::remove_var("XDG_STATE_HOME"),
        }

        for (id, result) in results {
            let err = result
                .err()
                .unwrap_or_else(|| panic!("path-escaping id {id:?} should be rejected"));
            assert_eq!(
                err.kind(),
                std::io::ErrorKind::InvalidInput,
                "id {id:?} should be rejected as invalid input"
            );
        }
    }

    /// `acp_session_dir` accepts a non-ULID id that is still a single safe path
    /// component — path-safety validation does not reject ids on format.
    ///
    /// Serialized: mutates the process-global `XDG_STATE_HOME` env var.
    #[test]
    #[serial]
    fn test_acp_session_dir_accepts_safe_non_ulid_id() {
        let temp = tempfile::tempdir().unwrap();
        let xdg_state = temp.path().join("state");

        // SAFETY: single-threaded test; restores the env var afterwards.
        let previous = std::env::var_os("XDG_STATE_HOME");
        std::env::set_var("XDG_STATE_HOME", &xdg_state);
        let dir = acp_session_dir("my-session-id");
        match previous {
            Some(value) => std::env::set_var("XDG_STATE_HOME", value),
            None => std::env::remove_var("XDG_STATE_HOME"),
        }

        let dir = dir.expect("a safe single-component id should be accepted");
        assert_eq!(dir, xdg_state.join("acp").join("my-session-id"));
        assert!(dir.is_dir());
    }
}
