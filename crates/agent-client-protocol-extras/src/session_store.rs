//! Agent-neutral session persistence for ACP `session/list`, `session/load`,
//! and `session/resume`.
//!
//! This module is the shared, backend-independent layer that powers the three
//! ACP session-lifecycle methods. It encodes the distinction between
//! `session/resume` and `session/load`:
//!
//! - `session/resume` — restore session state and return. MUST NOT replay
//!   history. The handler is [`ResumeStrategy::restore`] followed by a return.
//! - `session/load` — restore state, then replay the recorded conversation as
//!   `session/update` notifications, then return. The handler is
//!   [`ResumeStrategy::restore`], then replaying [`SessionRecord::updates`] to
//!   the client, then a return.
//!
//! `load` is `resume` plus replay. Both share state restoration; the shared
//! structure ([`SessionRecord`], [`SessionStore`], [`ResumeStrategy`]) is what
//! this module provides. The replay step lives in the caller's `session/load`
//! handler, not here — this layer only persists and restores.
//!
//! Records are serialized to `session.json` inside the per-session directory
//! resolved by [`acp_session_dir`](crate::raw_messages::acp_session_dir), so
//! the raw JSON-RPC transcript (`raw.jsonl`) and the session record
//! (`session.json`) are siblings.
//!
//! The session id is treated as an **opaque string** throughout — neither
//! [`SessionStore`] nor [`acp_session_dir`](crate::raw_messages::acp_session_dir)
//! parse or validate it as a ULID. Records produced by `new_session` carry
//! lexically time-sortable ULID ids, which is what makes directory-name sorting
//! double as stable cursor pagination, but the store never depends on that.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use agent_client_protocol::schema::{McpServer, SessionInfo, SessionUpdate};
use serde::{Deserialize, Serialize};

use crate::raw_messages::acp_session_dir;

/// File name for the persisted session record within a session directory.
const SESSION_RECORD_FILE: &str = "session.json";

/// Process-wide counter making each [`SessionStore::persist`] temp-file name
/// unique, even for concurrent persists of the same session from different
/// threads. The process id alone is identical across threads, so two threads
/// persisting the same session would otherwise race on the same temp path.
static PERSIST_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Sub-directory of `$XDG_STATE_HOME` that holds all per-session directories.
const ACP_STATE_SUBDIR: &str = "acp";

/// Errors raised while persisting, loading, or listing session records.
#[derive(Debug, thiserror::Error)]
pub enum SessionStoreError {
    /// An underlying filesystem operation failed.
    #[error("session store I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// A `session.json` file could not be (de)serialized.
    #[error("session record JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Result alias for fallible session-store operations.
pub type Result<T> = std::result::Result<T, SessionStoreError>;

/// An agent-neutral, serde-serializable snapshot of an ACP session.
///
/// A `SessionRecord` captures everything needed to restore a session and to
/// answer `session/list`, independent of which agent backend produced it. It
/// is persisted as `session.json` inside the session's
/// [`acp_session_dir`](crate::raw_messages::acp_session_dir).
///
/// The [`updates`](Self::updates) field is the ordered ACP `SessionUpdate`
/// stream for the session. Replaying it as `session/update` notifications is
/// exactly what turns a `session/resume` into a `session/load`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    /// Opaque session identifier. In practice a ULID string, but the store
    /// never parses or validates it — its only requirement is uniqueness.
    pub session_id: String,
    /// Working directory the session runs in. Absolute path.
    pub cwd: PathBuf,
    /// Optional human-readable title for the session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// RFC 3339 timestamp of the session's last activity.
    pub updated_at: String,
    /// MCP servers configured for the session.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_servers: Vec<McpServer>,
    /// Ordered ACP `SessionUpdate` stream. Replaying this as `session/update`
    /// notifications is the replay half of a `session/load`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub updates: Vec<SessionUpdate>,
}

impl SessionRecord {
    /// Create a new record with no title, no MCP servers, and no updates.
    ///
    /// # Parameters
    ///
    /// * `session_id` - Opaque session identifier (typically a ULID string).
    /// * `cwd` - Absolute working directory for the session.
    /// * `updated_at` - RFC 3339 timestamp of the last activity.
    #[must_use]
    pub fn new(
        session_id: impl Into<String>,
        cwd: impl Into<PathBuf>,
        updated_at: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            cwd: cwd.into(),
            title: None,
            updated_at: updated_at.into(),
            mcp_servers: Vec::new(),
            updates: Vec::new(),
        }
    }

    /// Project this record into the ACP [`SessionInfo`] shape returned by
    /// `session/list`. Drops the heavy [`updates`](Self::updates) stream and
    /// the MCP server configuration, keeping only the listing-relevant fields.
    #[must_use]
    pub fn to_session_info(&self) -> SessionInfo {
        SessionInfo::new(self.session_id.clone(), self.cwd.clone())
            .title(self.title.clone())
            .updated_at(self.updated_at.clone())
    }
}

/// One page of a [`SessionStore::list`] call: the page of [`SessionInfo`]s and
/// an optional opaque cursor for the next page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionListPage {
    /// The session listings on this page.
    pub sessions: Vec<SessionInfo>,
    /// Opaque cursor for the next page, or `None` when this is the last page.
    /// When present, pass it back as `cursor` to fetch the following page.
    pub next_cursor: Option<String>,
}

/// Persistent store for [`SessionRecord`]s, one `session.json` per session
/// directory under `$XDG_STATE_HOME/acp/`.
///
/// The store keys everything on the opaque session-id string. Because session
/// directories are named by ULID — globally unique and lexically
/// time-sortable — listing them in sorted order yields a chronological
/// ordering, and a directory name doubles as a stable pagination cursor for
/// free. The store does not depend on the id actually being a ULID; it only
/// sorts the names lexically.
#[derive(Debug, Default, Clone, Copy)]
pub struct SessionStore;

impl SessionStore {
    /// Create a new session store.
    ///
    /// The store is stateless — it resolves directories on every call — so
    /// construction is free and the value is trivially [`Copy`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Persist a [`SessionRecord`] to its `session.json`.
    ///
    /// The record is written atomically: it is serialized to a temporary file
    /// in the same directory and then renamed over the destination, so a
    /// reader never observes a half-written `session.json`. A crash mid-write
    /// leaves the previous record (or no record) intact.
    ///
    /// The temp-file name is unique per call — process id plus a
    /// process-wide atomic counter — so two concurrent `persist` calls for the
    /// same session (from any threads, in or across processes) write to
    /// distinct temp files and never corrupt each other's content before the
    /// rename. The final rename onto `session.json` is last-writer-wins.
    ///
    /// # Errors
    ///
    /// Returns [`SessionStoreError`] if the session directory cannot be
    /// resolved, the record cannot be serialized, or the temporary file cannot
    /// be written or renamed.
    pub fn persist(&self, record: &SessionRecord) -> Result<()> {
        let dir = acp_session_dir(&record.session_id)?;
        let destination = dir.join(SESSION_RECORD_FILE);
        let json = serde_json::to_vec_pretty(record)?;

        // Write to a uniquely-named temp file in the same directory so the
        // rename is atomic (same-filesystem) and concurrent persists of the
        // same session do not clobber each other's temp file. The counter
        // disambiguates threads within one process; the pid disambiguates
        // processes.
        let unique = PERSIST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp = dir.join(format!(
            "{SESSION_RECORD_FILE}.{}.{unique}.tmp",
            std::process::id()
        ));
        std::fs::write(&temp, &json)?;
        std::fs::rename(&temp, &destination)?;
        Ok(())
    }

    /// Load the [`SessionRecord`] for an opaque session id.
    ///
    /// Returns `Ok(None)` when the session has no persisted `session.json`
    /// (either the directory or the file is absent).
    ///
    /// # Errors
    ///
    /// Returns [`SessionStoreError`] if the session directory cannot be
    /// resolved, the file exists but cannot be read, or its contents are not
    /// valid `SessionRecord` JSON.
    pub fn load(&self, session_id: &str) -> Result<Option<SessionRecord>> {
        let path = acp_session_dir(session_id)?.join(SESSION_RECORD_FILE);
        read_record(&path)
    }

    /// List persisted sessions, newest first, with cursor-based pagination.
    ///
    /// Scans `$XDG_STATE_HOME/acp/`, reads each session's `session.json`, and
    /// returns up to `page_size` [`SessionInfo`]s. Directory names are sorted
    /// in descending lexical order so the most recent ULID-named sessions come
    /// first.
    ///
    /// # Parameters
    ///
    /// * `cwd_filter` - When `Some`, only sessions whose `cwd` matches exactly
    ///   are returned.
    /// * `cursor` - When `Some`, listing resumes strictly after the session
    ///   directory named by the cursor (the opaque token from a previous
    ///   page's [`SessionListPage::next_cursor`]).
    /// * `page_size` - Maximum number of sessions on the returned page. A
    ///   `page_size` of `0` yields an empty page.
    ///
    /// # Errors
    ///
    /// Returns [`SessionStoreError`] if the `acp` state directory cannot be
    /// resolved or scanned, or a `session.json` exists but is unreadable or
    /// malformed.
    pub fn list(
        &self,
        cwd_filter: Option<&Path>,
        cursor: Option<&str>,
        page_size: usize,
    ) -> Result<SessionListPage> {
        let session_ids = self.sorted_session_ids()?;

        // Resume strictly after the cursor's session id. `session_ids` is in
        // descending order, so ids `>= cursor` form the already-seen prefix;
        // the first id `< cursor` is where the next page begins. A cursor
        // naming a session that no longer exists still works — the partition
        // point is computed purely from the ordering.
        let start = match cursor {
            Some(c) => session_ids.partition_point(|id| id.as_str() >= c),
            None => 0,
        };

        let mut sessions = Vec::new();
        let mut next_cursor = None;
        for id in session_ids.iter().skip(start) {
            if sessions.len() == page_size {
                // There is at least one more matching-or-not entry beyond the
                // page. Hand back the last emitted session's id as the cursor
                // to resume after it. A `page_size` of 0 produces an empty
                // page with no last session, so there is no cursor to emit —
                // `sessions_cursor` returns `None` in that case.
                next_cursor = sessions_cursor(&sessions);
                break;
            }
            let path = self.session_record_path(id)?;
            let Some(record) = read_record(&path)? else {
                continue;
            };
            if let Some(filter) = cwd_filter {
                if record.cwd != filter {
                    continue;
                }
            }
            sessions.push(record.to_session_info());
        }

        Ok(SessionListPage {
            sessions,
            next_cursor,
        })
    }

    /// Resolve the `acp` state directory and return its session-id entries
    /// sorted in descending lexical order (newest ULID first).
    fn sorted_session_ids(&self) -> Result<Vec<String>> {
        let state_dir = swissarmyhammer_directory::xdg_state_dir()
            .map_err(|e| std::io::Error::other(e.to_string()))?
            .join(ACP_STATE_SUBDIR);

        let mut ids = Vec::new();
        match std::fs::read_dir(&state_dir) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry?;
                    if entry.file_type()?.is_dir() {
                        if let Some(name) = entry.file_name().to_str() {
                            ids.push(name.to_string());
                        }
                    }
                }
            }
            // No `acp` directory yet means no sessions — an empty list, not an
            // error.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        ids.sort_unstable_by(|a, b| b.cmp(a));
        Ok(ids)
    }

    /// Resolve the `session.json` path for an opaque session id.
    fn session_record_path(&self, session_id: &str) -> Result<PathBuf> {
        Ok(acp_session_dir(session_id)?.join(SESSION_RECORD_FILE))
    }
}

/// Read a `session.json` file, returning `Ok(None)` when the file is absent.
fn read_record(path: &Path) -> Result<Option<SessionRecord>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Derive the next-page cursor from the last session on the current page.
///
/// The cursor is the last emitted session's id; the next `list` call resumes
/// strictly after it in descending order. Returns `None` for an empty page
/// (e.g. a `page_size` of 0), since there is no session to resume after — a
/// `Some("")` cursor would be meaningless.
fn sessions_cursor(page: &[SessionInfo]) -> Option<String> {
    page.last().map(|info| info.session_id.0.to_string())
}

/// Restores an agent's generation state from a persisted [`SessionRecord`].
///
/// This trait is the agent-specific seam of the session-resume layer. The
/// shared code calls [`restore`](Self::restore) for both `session/resume` and
/// `session/load`; the only difference between the two handlers is that
/// `session/load` additionally replays [`SessionRecord::updates`] to the
/// client after `restore` returns.
///
/// Per-agent implementations live in later cards — for example, the Claude
/// backend shells out to `claude --resume`, and the llama backend re-renders
/// the conversation through its chat template.
#[async_trait::async_trait]
pub trait ResumeStrategy: Send + Sync {
    /// Restore the agent's generation state from `record`.
    ///
    /// Restores state only — it MUST NOT replay history to the client. The
    /// `session/load` replay step is the caller's responsibility and is not
    /// part of this contract.
    ///
    /// # Errors
    ///
    /// Returns [`SessionStoreError`] when the agent state cannot be restored
    /// from the record.
    async fn restore(&self, record: &SessionRecord) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// Run `body` with `XDG_STATE_HOME` pointed at a fresh temp directory,
    /// restoring the previous value afterwards.
    ///
    /// Serialized at the call site with `#[serial]`: this mutates the
    /// process-global `XDG_STATE_HOME` env var.
    fn with_temp_state<R>(body: impl FnOnce() -> R) -> R {
        let temp = tempfile::tempdir().unwrap();
        // SAFETY: callers are `#[serial]`, so no other thread reads or writes
        // the env var concurrently; the previous value is restored below.
        let previous = std::env::var_os("XDG_STATE_HOME");
        std::env::set_var("XDG_STATE_HOME", temp.path());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(body));
        match previous {
            Some(value) => std::env::set_var("XDG_STATE_HOME", value),
            None => std::env::remove_var("XDG_STATE_HOME"),
        }
        match result {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    /// Build a record with a fixed cwd and timestamp for the given id.
    fn record_at(id: &str, cwd: &str) -> SessionRecord {
        SessionRecord::new(id, cwd, "2026-05-18T12:00:00Z")
    }

    /// Persisting a record then loading it back yields an identical record.
    #[test]
    #[serial]
    fn persist_then_load_round_trips() {
        with_temp_state(|| {
            let store = SessionStore::new();
            let mut record = record_at("01ARZ3NDEKTSV4RRFFQ69G5FAV", "/work/project");
            record.title = Some("Round trip".to_string());
            record.updates.push(SessionUpdate::AgentMessageChunk(
                agent_client_protocol::schema::ContentChunk::new(
                    agent_client_protocol::schema::ContentBlock::from("hello".to_string()),
                ),
            ));

            store.persist(&record).unwrap();
            let loaded = store
                .load("01ARZ3NDEKTSV4RRFFQ69G5FAV")
                .unwrap()
                .expect("record should be present after persist");

            assert_eq!(loaded, record);
        });
    }

    /// Persisting twice overwrites the prior record atomically.
    #[test]
    #[serial]
    fn persist_overwrites_existing_record() {
        with_temp_state(|| {
            let store = SessionStore::new();
            let id = "01ARZ3NDEKTSV4RRFFQ69G5FB0";

            store.persist(&record_at(id, "/work/a")).unwrap();
            let mut updated = record_at(id, "/work/a");
            updated.title = Some("Second".to_string());
            store.persist(&updated).unwrap();

            let loaded = store.load(id).unwrap().unwrap();
            assert_eq!(loaded.title.as_deref(), Some("Second"));
        });
    }

    /// `load` returns `None` for a session that was never persisted.
    #[test]
    #[serial]
    fn load_missing_session_returns_none() {
        with_temp_state(|| {
            let store = SessionStore::new();
            assert!(store
                .load("01ARZ3NDEKTSV4RRFFQ69MISSING")
                .unwrap()
                .is_none());
        });
    }

    /// `list` with no cwd filter returns every persisted session, newest
    /// (highest ULID) first.
    #[test]
    #[serial]
    fn list_without_filter_returns_all_newest_first() {
        with_temp_state(|| {
            let store = SessionStore::new();
            // Ascending ids; expect descending order back.
            for id in ["01AAA0000000000000000000A0", "01BBB0000000000000000000B0"] {
                store.persist(&record_at(id, "/work/x")).unwrap();
            }

            let page = store.list(None, None, 10).unwrap();
            let ids: Vec<String> = page
                .sessions
                .iter()
                .map(|s| s.session_id.0.to_string())
                .collect();

            assert_eq!(
                ids,
                vec![
                    "01BBB0000000000000000000B0".to_string(),
                    "01AAA0000000000000000000A0".to_string(),
                ]
            );
            assert!(page.next_cursor.is_none());
        });
    }

    /// `list` with a cwd filter returns only sessions whose cwd matches.
    #[test]
    #[serial]
    fn list_with_cwd_filter_returns_only_matches() {
        with_temp_state(|| {
            let store = SessionStore::new();
            store
                .persist(&record_at("01CCC0000000000000000000C0", "/work/keep"))
                .unwrap();
            store
                .persist(&record_at("01DDD0000000000000000000D0", "/work/skip"))
                .unwrap();
            store
                .persist(&record_at("01EEE0000000000000000000E0", "/work/keep"))
                .unwrap();

            let page = store.list(Some(Path::new("/work/keep")), None, 10).unwrap();
            let ids: Vec<String> = page
                .sessions
                .iter()
                .map(|s| s.session_id.0.to_string())
                .collect();

            assert_eq!(
                ids,
                vec![
                    "01EEE0000000000000000000E0".to_string(),
                    "01CCC0000000000000000000C0".to_string(),
                ]
            );
        });
    }

    /// `list` paginates: each page honours `page_size`, the cursor resumes
    /// after the last returned session, and the final page has no cursor.
    #[test]
    #[serial]
    fn list_paginates_with_cursor() {
        with_temp_state(|| {
            let store = SessionStore::new();
            let ids = [
                "01AAA0000000000000000000A0",
                "01BBB0000000000000000000B0",
                "01CCC0000000000000000000C0",
                "01DDD0000000000000000000D0",
                "01EEE0000000000000000000E0",
            ];
            for id in ids {
                store.persist(&record_at(id, "/work/p")).unwrap();
            }

            // Page 1: two newest sessions, cursor present.
            let page1 = store.list(None, None, 2).unwrap();
            let page1_ids: Vec<String> = page1
                .sessions
                .iter()
                .map(|s| s.session_id.0.to_string())
                .collect();
            assert_eq!(
                page1_ids,
                vec![
                    "01EEE0000000000000000000E0".to_string(),
                    "01DDD0000000000000000000D0".to_string(),
                ]
            );
            let cursor1 = page1.next_cursor.expect("first page should have a cursor");

            // Page 2: next two, cursor still present.
            let page2 = store.list(None, Some(&cursor1), 2).unwrap();
            let page2_ids: Vec<String> = page2
                .sessions
                .iter()
                .map(|s| s.session_id.0.to_string())
                .collect();
            assert_eq!(
                page2_ids,
                vec![
                    "01CCC0000000000000000000C0".to_string(),
                    "01BBB0000000000000000000B0".to_string(),
                ]
            );
            let cursor2 = page2.next_cursor.expect("second page should have a cursor");

            // Page 3: final session, no cursor.
            let page3 = store.list(None, Some(&cursor2), 2).unwrap();
            let page3_ids: Vec<String> = page3
                .sessions
                .iter()
                .map(|s| s.session_id.0.to_string())
                .collect();
            assert_eq!(page3_ids, vec!["01AAA0000000000000000000A0".to_string()]);
            assert!(page3.next_cursor.is_none());
        });
    }

    /// A `page_size` of zero yields an empty page even when sessions exist,
    /// and emits no cursor — an empty page has no session to resume after.
    #[test]
    #[serial]
    fn list_with_zero_page_size_is_empty() {
        with_temp_state(|| {
            let store = SessionStore::new();
            store
                .persist(&record_at("01AAA0000000000000000000A0", "/work/z"))
                .unwrap();

            let page = store.list(None, None, 0).unwrap();
            assert!(page.sessions.is_empty());
            assert!(
                page.next_cursor.is_none(),
                "an empty page must not emit a cursor"
            );
        });
    }

    /// `list` paginates correctly across a page boundary while a `cwd_filter`
    /// is in effect: the cursor advances by *scanned* sessions (not just
    /// matched ones), so following it skips the already-scanned prefix even
    /// when the trailing sessions are all filtered out.
    #[test]
    #[serial]
    fn list_paginates_with_cursor_and_cwd_filter() {
        with_temp_state(|| {
            let store = SessionStore::new();
            // Interleave matching (`/work/keep`) and non-matching (`/work/skip`)
            // sessions. Descending scan order is E, D, C, B, A.
            store
                .persist(&record_at("01AAA0000000000000000000A0", "/work/keep"))
                .unwrap();
            store
                .persist(&record_at("01BBB0000000000000000000B0", "/work/keep"))
                .unwrap();
            store
                .persist(&record_at("01CCC0000000000000000000C0", "/work/keep"))
                .unwrap();
            store
                .persist(&record_at("01DDD0000000000000000000D0", "/work/skip"))
                .unwrap();
            store
                .persist(&record_at("01EEE0000000000000000000E0", "/work/skip"))
                .unwrap();

            let keep = Path::new("/work/keep");

            // Page 1: scan E, D, C. E and D are filtered out; C fills the
            // single-session page. The cursor is C, even though the scan
            // crossed two non-matching sessions to reach it.
            let page1 = store.list(Some(keep), None, 1).unwrap();
            let page1_ids: Vec<String> = page1
                .sessions
                .iter()
                .map(|s| s.session_id.0.to_string())
                .collect();
            assert_eq!(page1_ids, vec!["01CCC0000000000000000000C0".to_string()]);
            let cursor1 = page1
                .next_cursor
                .expect("a full page should carry a cursor");
            assert_eq!(cursor1, "01CCC0000000000000000000C0");

            // Page 2: resume strictly after C — scan B, then A. B fills the
            // page; A remains, so a cursor is emitted.
            let page2 = store.list(Some(keep), Some(&cursor1), 1).unwrap();
            let page2_ids: Vec<String> = page2
                .sessions
                .iter()
                .map(|s| s.session_id.0.to_string())
                .collect();
            assert_eq!(page2_ids, vec!["01BBB0000000000000000000B0".to_string()]);
            let cursor2 = page2
                .next_cursor
                .expect("a full page should carry a cursor");

            // Page 3: resume after B — only A remains and it matches. It is
            // the final page, so no cursor.
            let page3 = store.list(Some(keep), Some(&cursor2), 1).unwrap();
            let page3_ids: Vec<String> = page3
                .sessions
                .iter()
                .map(|s| s.session_id.0.to_string())
                .collect();
            assert_eq!(page3_ids, vec!["01AAA0000000000000000000A0".to_string()]);
            assert!(page3.next_cursor.is_none());
        });
    }

    /// When a `cwd_filter` leaves the trailing sessions all filtered out, the
    /// cursor from the last full page points at an effectively-empty next
    /// page: following it yields zero sessions and no further cursor.
    #[test]
    #[serial]
    fn list_cursor_into_all_filtered_tail_is_empty() {
        with_temp_state(|| {
            let store = SessionStore::new();
            // Descending scan order C, B, A: C matches, B and A do not.
            store
                .persist(&record_at("01AAA0000000000000000000A0", "/work/skip"))
                .unwrap();
            store
                .persist(&record_at("01BBB0000000000000000000B0", "/work/skip"))
                .unwrap();
            store
                .persist(&record_at("01CCC0000000000000000000C0", "/work/keep"))
                .unwrap();

            let keep = Path::new("/work/keep");

            // Page 1: C fills the page. Two non-matching sessions (B, A) still
            // lie beyond it, so a cursor is emitted even though they will all
            // be filtered out.
            let page1 = store.list(Some(keep), None, 1).unwrap();
            let page1_ids: Vec<String> = page1
                .sessions
                .iter()
                .map(|s| s.session_id.0.to_string())
                .collect();
            assert_eq!(page1_ids, vec!["01CCC0000000000000000000C0".to_string()]);
            let cursor1 = page1
                .next_cursor
                .expect("a full page with trailing entries carries a cursor");

            // Page 2: the cursor lands in the all-filtered tail — B and A are
            // scanned and dropped, yielding an empty page and no cursor.
            let page2 = store.list(Some(keep), Some(&cursor1), 1).unwrap();
            assert!(page2.sessions.is_empty());
            assert!(page2.next_cursor.is_none());
        });
    }

    /// `list` against an absent `acp` state directory is an empty page, not an
    /// error.
    #[test]
    #[serial]
    fn list_with_no_state_dir_is_empty() {
        with_temp_state(|| {
            let store = SessionStore::new();
            let page = store.list(None, None, 10).unwrap();
            assert!(page.sessions.is_empty());
            assert!(page.next_cursor.is_none());
        });
    }

    /// `to_session_info` carries the listing-relevant fields and drops the
    /// heavy `updates` stream.
    #[test]
    fn to_session_info_projects_listing_fields() {
        let mut record = record_at("01ARZ3NDEKTSV4RRFFQ69G5FAV", "/work/project");
        record.title = Some("Titled".to_string());

        let info = record.to_session_info();
        assert_eq!(info.session_id.0.as_ref(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert_eq!(info.cwd, PathBuf::from("/work/project"));
        assert_eq!(info.title.as_deref(), Some("Titled"));
        assert_eq!(info.updated_at.as_deref(), Some("2026-05-18T12:00:00Z"));
    }
}
