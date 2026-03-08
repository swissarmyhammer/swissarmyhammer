//! Typed error types for LSP server management.
//!
//! [`LspError`] replaces bare `String` errors throughout the crate, giving
//! callers structured variants they can match on (e.g. to distinguish a
//! missing binary from a handshake timeout).

use std::time::Duration;

/// Errors that can occur during LSP server lifecycle management.
#[derive(Debug, thiserror::Error)]
pub enum LspError {
    /// The LSP binary was not found on `$PATH`.
    #[error("binary not found: {command}")]
    BinaryNotFound {
        /// The command that was looked up.
        command: String,
        /// Human-readable install instructions.
        install_hint: String,
    },

    /// `Command::spawn()` returned an I/O error.
    #[error("failed to spawn LSP server: {0}")]
    SpawnFailed(#[from] std::io::Error),

    /// The LSP `initialize` handshake did not complete successfully.
    #[error("initialize handshake failed: {0}")]
    HandshakeFailed(String),

    /// The `initialize` handshake did not finish within the allowed window.
    #[error("initialize timed out after {0:?}")]
    Timeout(Duration),

    /// Graceful shutdown (`shutdown` + `exit`) failed.
    #[error("shutdown failed: {0}")]
    ShutdownFailed(String),

    /// An operation was attempted on a daemon that is not running.
    #[error("server not running")]
    NotRunning,

    /// A JSON-RPC framing or encoding error.
    #[error("JSON-RPC error: {0}")]
    JsonRpc(String),

    /// Project detection failed for the workspace.
    #[error("project detection failed: {0}")]
    ProjectDetection(String),

    /// No managed daemon exists for the given command name.
    #[error("no daemon found for command: {0}")]
    DaemonNotFound(String),
}
