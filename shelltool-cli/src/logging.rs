//! File-based tracing for the shelltool CLI / MCP server.
//!
//! Logs are written to `<ShellConfig::DIR_NAME>/mcp.log` (conventionally
//! `.shell/mcp.log`) under the current working directory, falling back to
//! stderr when the log directory or file cannot be created. Unlike kanban's
//! committed `.kanban/` data directory, the `.shell/` directory holds runtime
//! data and is auto-created on demand.
//!
//! This module is a thin wrapper around
//! [`swissarmyhammer_common::logging::init_file_tracing_with_fallback`] — it
//! only defines a shelltool-specific filter and entry point; the shared
//! helper owns the file-layer construction, stderr fallback, and
//! warning-on-failure format.

use swissarmyhammer_common::logging::{init_file_tracing_with_fallback, DirPolicy};
use swissarmyhammer_directory::{DirectoryConfig, ShellConfig};
use tracing_subscriber::EnvFilter;

/// Build an `EnvFilter` for the shelltool CLI.
///
/// In debug mode, enables debug-level output for shelltool-specific crates.
/// Otherwise, defers to the `RUST_LOG` environment variable, defaulting to
/// `rmcp=warn,debug` (suppress noisy rmcp internals while keeping everything
/// else at debug level).
fn make_filter(debug: bool) -> EnvFilter {
    if debug {
        EnvFilter::new("shelltool=debug,swissarmyhammer_tools=debug,swissarmyhammer_shell=debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("rmcp=warn,debug"))
    }
}

/// Initialize the tracing subscriber for the shelltool CLI.
///
/// Logs are written to `<cwd>/<ShellConfig::DIR_NAME>/mcp.log` via the shared
/// [`FileWriterGuard`] pattern (flush + sync on every write). The data
/// directory is auto-created on demand — `.shell/` holds runtime data (not
/// committed state), so creation is the expected behavior. On any filesystem
/// failure (permissions, read-only FS, disk full), a warning is emitted to
/// stderr and tracing falls back to stderr.
///
/// [`FileWriterGuard`]: swissarmyhammer_common::logging::FileWriterGuard
///
/// # Arguments
///
/// * `debug` - When `true`, enables debug-level tracing for shelltool crates.
pub fn init_tracing(debug: bool) {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    if let Some(e) = init_file_tracing_with_fallback(
        make_filter(debug),
        &cwd,
        ShellConfig::DIR_NAME,
        DirPolicy::AutoCreate,
    ) {
        tracing::warn!(
            "Could not setup shelltool file logging: {}. Falling back to stderr.",
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

    #[test]
    fn open_log_file_creates_log_dir_and_file() {
        let tmp = TempDir::new().expect("create tempdir");
        let root = tmp.path();

        let (file, error) = open_log_file(root, ShellConfig::DIR_NAME, DirPolicy::AutoCreate);

        assert!(
            file.is_some(),
            "open_log_file should return Some when the dir can be created"
        );
        assert!(
            error.is_none(),
            "open_log_file should not report an error when creation succeeds"
        );
        assert!(
            root.join(ShellConfig::DIR_NAME).is_dir(),
            "open_log_file should have created the shelltool data directory"
        );
        assert!(
            root.join(ShellConfig::DIR_NAME)
                .join(LOG_FILE_NAME)
                .exists(),
            "mcp.log should have been created inside the shelltool data directory"
        );
    }

    /// End-to-end test for `init_tracing`.
    ///
    /// `init_tracing` installs a global subscriber via `.init()`, which can
    /// only succeed once per test binary — subsequent calls panic. This test
    /// therefore exercises the auto-create path (the happy branch) in a
    /// single call and uses `#[serial]` to keep the `CurrentDirGuard`'s CWD
    /// change from racing with other tests in this crate.
    #[test]
    #[serial]
    fn init_tracing_creates_mcp_log_under_shell_dir() {
        let tmp = TempDir::new().unwrap();

        let _cwd_guard = CurrentDirGuard::new(tmp.path()).unwrap();

        // `init_tracing` may only be called once per test binary. Running
        // this under `#[serial]` ensures it's the single authoritative call.
        init_tracing(false);

        assert!(
            tmp.path()
                .join(ShellConfig::DIR_NAME)
                .join(LOG_FILE_NAME)
                .exists(),
            "init_tracing should create <ShellConfig::DIR_NAME>/mcp.log under cwd"
        );
    }
}
