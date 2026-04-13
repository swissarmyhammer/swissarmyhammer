//! File-based tracing for the kanban MCP server.
//!
//! Logs to `.kanban/mcp.log` when a `.kanban/` directory exists in the current
//! working directory, falling back to stderr otherwise. The `.kanban/` directory
//! holds committed board data and must NOT be auto-created by the logger.
//!
//! This module is a thin wrapper around
//! [`swissarmyhammer_common::logging::init_file_tracing_with_fallback`] — it
//! only defines a kanban-specific filter and entry point; the shared helper
//! owns the file-layer construction, stderr fallback, and warning-on-failure
//! format.

use swissarmyhammer_common::logging::{init_file_tracing_with_fallback, DirPolicy};
use tracing_subscriber::EnvFilter;

/// The kanban data directory name.
const KANBAN_DIR_NAME: &str = ".kanban";

/// Build an `EnvFilter` for the kanban CLI.
///
/// In debug mode, enables debug-level output for kanban-specific crates.
/// Otherwise, defers to the `RUST_LOG` environment variable, defaulting to
/// `rmcp=warn,debug` (suppress noisy rmcp internals while keeping everything
/// else at debug level).
fn make_filter(debug: bool) -> EnvFilter {
    if debug {
        EnvFilter::new("kanban_cli=debug,swissarmyhammer_kanban=debug,swissarmyhammer_tools=debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("rmcp=warn,debug"))
    }
}

/// Initialize the tracing subscriber for the kanban CLI.
///
/// When a `.kanban/` directory exists in the current working directory, logs are
/// written to `.kanban/mcp.log` using the shared [`FileWriterGuard`] pattern
/// (flush + sync on every write). When the directory is absent, logging
/// silently falls back to stderr — the missing directory is the expected
/// "no file logging" signal, not an error. When the directory exists but the
/// log file cannot be created, a warning is logged through tracing before falling
/// back.
///
/// The `.kanban/` directory is never auto-created — it holds committed board
/// data and must be initialized through the normal kanban workflow.
///
/// [`FileWriterGuard`]: swissarmyhammer_common::logging::FileWriterGuard
///
/// # Arguments
///
/// * `debug` - When `true`, enables debug-level tracing for kanban crates.
pub fn init_tracing(debug: bool) {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    if let Some(e) = init_file_tracing_with_fallback(
        make_filter(debug),
        &cwd,
        KANBAN_DIR_NAME,
        DirPolicy::MustExist,
    ) {
        tracing::warn!(
            "Could not setup kanban file logging: {}. Falling back to stderr.",
            e
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use swissarmyhammer_common::logging::{open_log_file, LOG_FILE_NAME};
    use swissarmyhammer_common::test_utils::CurrentDirGuard;
    use tempfile::TempDir;

    /// End-to-end test for `init_tracing`.
    ///
    /// `init_tracing` installs a global subscriber via `.init()`, which can only
    /// succeed once per test binary — subsequent calls panic. This test therefore
    /// exercises the `.kanban/`-present path (the more interesting branch) in a
    /// single call and uses `#[serial]` to keep the `CurrentDirGuard`'s CWD
    /// change from racing with other tests in this crate.
    ///
    /// The "no auto-create when `.kanban/` is absent" invariant is covered
    /// by the shared helper's unit tests in `swissarmyhammer-common`, which
    /// exercise the same `open_log_file` that `init_tracing` delegates to via
    /// `DirPolicy::MustExist`.
    #[test]
    #[serial]
    fn init_tracing_creates_mcp_log_when_kanban_dir_exists() {
        let tmp = TempDir::new().unwrap();
        let kanban_dir = tmp.path().join(KANBAN_DIR_NAME);
        std::fs::create_dir(&kanban_dir).unwrap();

        let _cwd_guard = CurrentDirGuard::new(tmp.path()).unwrap();

        // `init_tracing` may only be called once per test binary. Running this
        // under `#[serial]` ensures it's the single authoritative call.
        init_tracing(false);

        assert!(
            kanban_dir.join(LOG_FILE_NAME).exists(),
            "init_tracing should create .kanban/mcp.log when .kanban/ exists"
        );
    }

    /// Regression: the kanban logger must never auto-create `.kanban/` — that
    /// directory holds committed board data and is only created through the
    /// normal kanban workflow. This exercises the `DirPolicy::MustExist` path
    /// directly via the shared helper.
    #[test]
    fn open_log_file_does_not_auto_create_kanban_dir() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let (file, error) = open_log_file(root, KANBAN_DIR_NAME, DirPolicy::MustExist);

        assert!(
            file.is_none(),
            "open_log_file must return None when .kanban/ is absent"
        );
        assert!(
            error.is_none(),
            "open_log_file must not report an error when directory is absent (silent fallback)"
        );
        assert!(
            !root.join(KANBAN_DIR_NAME).exists(),
            "open_log_file must not auto-create .kanban/"
        );
    }
}
