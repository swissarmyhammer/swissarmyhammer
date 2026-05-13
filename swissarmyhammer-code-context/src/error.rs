//! Error types for code context operations

use std::path::PathBuf;

use swissarmyhammer_leader_election::ElectionError;

/// Errors that can occur during code context operations
#[derive(Debug, thiserror::Error)]
pub enum CodeContextError {
    /// An IO operation failed
    #[error("IO error")]
    Io(#[from] std::io::Error),

    /// A database operation failed
    #[error("database error")]
    Database(#[from] rusqlite::Error),

    /// Leader election failed
    #[error("election error")]
    Election(#[from] ElectionError),

    /// Invalid regex pattern
    #[error("invalid regex pattern: {0}")]
    Pattern(String),

    /// LSP communication error
    #[error("LSP error: {0}")]
    LspError(String),

    /// Query execution error
    #[error("{0}")]
    QueryError(String),

    /// A write op was attempted from a follower (read-only) process.
    ///
    /// The code-context database is opened read-write only by the leader. When
    /// any other process opens the same workspace it joins as a follower with
    /// a read-only connection. Attempting a write op from that connection
    /// would surface as `attempt to write a readonly database` from SQLite;
    /// this typed error fires *before* the SQL runs so the user sees a
    /// useful explanation in the MCP payload instead of an opaque "database
    /// error".
    ///
    /// Produced by [`crate::CodeContextWorkspace::write_db`] — that is the single call
    /// site that signals this condition. Any write-side op (`rebuild_index`,
    /// `clear_status`, future writers) routes through `write_db()` and so
    /// receives this variant uniformly, hence the message stays op-agnostic.
    #[error(
        "the code-context database is held read-only by this process; \
         the writable connection is owned by the leader{leader_pid_display} \
         (db {db_path}) — usually an MCP server running for an agent session in {workspace_root}. \
         Stop that session and rerun the op through it.",
        leader_pid_display = match leader_pid {
            Some(pid) => format!(" (pid {})", pid),
            None => String::new(),
        },
        db_path = db_path.display(),
        workspace_root = workspace_root.display(),
    )]
    ReadOnlyFollower {
        /// PID of the process currently holding the leader lock, if known.
        ///
        /// Read from the leader-election lock file via
        /// [`swissarmyhammer_leader_election::peek_leader_pid`]. `None` means
        /// the file was missing, empty, or unparseable — the message degrades
        /// gracefully without it.
        leader_pid: Option<u32>,
        /// The workspace root whose code-context database is read-only.
        workspace_root: PathBuf,
        /// Path to the code-context SQLite database file.
        db_path: PathBuf,
    },
}
