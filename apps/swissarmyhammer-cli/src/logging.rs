//! File-based tracing for the `sah` CLI / MCP server.
//!
//! In MCP (`serve`) mode, logs are written to `<sah-root>/mcp.log` using
//! [`FileWriterGuard`](swissarmyhammer_common::logging::FileWriterGuard) so
//! every message is flushed and synced to disk immediately — critical when
//! the host process dies before a normal shutdown. When the log file cannot
//! be created (or the CLI is not in MCP mode), tracing falls back to stderr.
//!
//! The public entry point is [`configure_logging`].

use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer_common::logging::FileWriterGuard;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

/// Determine the appropriate log level based on configuration flags.
///
/// This function centralizes the logic for determining the log level based on
/// verbose, debug, quiet, and MCP mode flags.
///
/// # Arguments
/// * `is_mcp_mode` - Whether MCP mode is active
/// * `verbose` - Whether verbose logging is enabled
/// * `debug` - Whether debug logging is enabled
/// * `quiet` - Whether quiet mode is enabled
///
/// # Returns
/// The appropriate tracing Level
fn determine_log_level(
    is_mcp_mode: bool,
    verbose: bool,
    debug: bool,
    quiet: bool,
) -> tracing::Level {
    use tracing::Level;

    if is_mcp_mode {
        Level::DEBUG // More verbose for MCP mode to help with debugging
    } else if quiet {
        Level::ERROR
    } else if debug {
        Level::DEBUG
    } else if verbose {
        Level::TRACE
    } else {
        Level::INFO
    }
}

/// Build an [`EnvFilter`] for the sah CLI at the specified log level.
///
/// Suppresses noisy rmcp internals at warn while letting the requested level
/// apply to everything else. Named `make_filter` to match the vocabulary
/// used by the other CLI `logging.rs` wrappers (kanban, shelltool,
/// code-context) — sah's signature differs because the level is computed
/// from `verbose`/`debug`/`quiet`/`is_mcp_mode` flags before the filter is
/// built, but the helper role is the same.
fn make_filter(log_level: tracing::Level) -> EnvFilter {
    EnvFilter::new(format!("rmcp=warn,{log_level}"))
}

/// Ensure the `.sah/` directory exists and return its absolute path.
///
/// Tries the git root first so MCP servers started from arbitrary subdirectories
/// log to a single, consistent location per repository; falls back to the
/// current working directory when we're not inside a git repo.
fn ensure_swissarmyhammer_dir() -> Result<PathBuf, std::io::Error> {
    use swissarmyhammer_common::SwissarmyhammerDirectory;

    let sah_dir = SwissarmyhammerDirectory::from_git_root()
        .or_else(|_| SwissarmyhammerDirectory::from_custom_root(std::env::current_dir()?))
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    Ok(sah_dir.root().to_path_buf())
}

/// Install the tracing subscriber with the given level and writer.
///
/// Consolidates the common pattern of creating an [`EnvFilter`] and building a
/// tracing registry with the specified writer. Disables ANSI colors because
/// log output is typically read from files or piped tools where escape
/// sequences are noise.
fn setup_logging_with_writer<W>(log_level: tracing::Level, writer: W)
where
    W: for<'a> tracing_subscriber::fmt::MakeWriter<'a> + Send + Sync + 'static,
{
    let filter = make_filter(log_level);
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_ansi(false),
        )
        .init();
}

/// The log file name written inside the sah data directory.
///
/// Named `LOG_FILE_NAME` to match the vocabulary used by the other CLI
/// `logging.rs` wrappers (kanban, shelltool, code-context). Sah overrides
/// this via the `SWISSARMYHAMMER_LOG_FILE` env var, which the shared helper
/// does not — the override machinery stays local here.
const LOG_FILE_NAME: &str = "mcp.log";

/// Validate that a log file name contains no path components.
///
/// Only bare file names are accepted — any name containing a path separator
/// (`/`, `\`), a parent-directory reference (`..`), or an absolute-path
/// designator is rejected. This prevents a caller-controlled
/// `SWISSARMYHAMMER_LOG_FILE` from escaping the sah data directory.
///
/// Returns the name unchanged on success; returns an [`std::io::Error`] of
/// kind [`InvalidInput`](std::io::ErrorKind::InvalidInput) on rejection.
fn validate_log_file_name(name: &str) -> Result<&str, std::io::Error> {
    let path = std::path::Path::new(name);

    // Reject empty, absolute, or anything that isn't a single `Normal`
    // component. This rules out `..`, `.`, bare `/`, Windows roots, etc.
    let mut components = path.components();
    let only = components.next();
    if components.next().is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "SWISSARMYHAMMER_LOG_FILE must be a bare file name (no path separators): {name:?}"
            ),
        ));
    }
    match only {
        Some(std::path::Component::Normal(_)) => Ok(name),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("SWISSARMYHAMMER_LOG_FILE is not a valid file name: {name:?}"),
        )),
    }
}

/// Configure the MCP-mode log file sink.
///
/// Creates `<sah-root>/mcp.log` (or a name overridden via the
/// `SWISSARMYHAMMER_LOG_FILE` env var), wraps the file in a
/// [`FileWriterGuard`] so every write is flushed and synced to disk, and
/// installs the global tracing subscriber. The `SAH_CLI_MODE=1` env var is
/// set to prevent the unified server from reconfiguring logging downstream.
///
/// The override env var is validated to contain only a bare file name — any
/// value with path separators, parent-directory references, or an absolute
/// path is rejected with an error so a hostile or misconfigured environment
/// can't redirect log output to an arbitrary location on disk.
///
/// Returns an error if the log directory cannot be located, the override
/// file name is invalid, or the file cannot be created; callers are expected
/// to fall back to stderr on failure.
fn setup_mcp_logging(
    log_level: tracing::Level,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Set flag to prevent unified server from also configuring logging
    std::env::set_var("SAH_CLI_MODE", "1");

    // In MCP mode, write logs to .sah/mcp.log for debugging
    let log_dir = ensure_swissarmyhammer_dir()?;

    let override_name = std::env::var("SWISSARMYHAMMER_LOG_FILE").ok();
    let log_file_name = match override_name.as_deref() {
        Some(name) => validate_log_file_name(name)?,
        None => LOG_FILE_NAME,
    };
    let log_file_path = log_dir.join(log_file_name);
    let file = std::fs::File::create(&log_file_path)?;

    let shared_file = Arc::new(std::sync::Mutex::new(file));
    setup_logging_with_writer(log_level, move || {
        let file = shared_file.clone();
        Box::new(FileWriterGuard::new(file)) as Box<dyn std::io::Write>
    });

    Ok(())
}

/// Install a stderr-backed tracing subscriber at the given level.
///
/// Reusable fallback used both when the caller never asked for file logging
/// and when file logging setup failed.
fn setup_stderr_logging(log_level: tracing::Level) {
    setup_logging_with_writer(log_level, std::io::stderr);
}

/// Configure logging for the CLI.
///
/// In MCP (`serve`) mode, logs are written to `<sah-root>/mcp.log` via
/// [`FileWriterGuard`] (flush + sync on every write). In all other cases —
/// and as the fallback when the log file cannot be opened — logs go to
/// stderr. A warning is logged when file logging setup fails so
/// the missing log file isn't surprising.
///
/// # Arguments
/// * `verbose` - Increases verbosity to `TRACE` in non-MCP mode
/// * `debug` - Increases verbosity to `DEBUG` in non-MCP mode
/// * `quiet` - Lowers verbosity to `ERROR` in non-MCP mode
/// * `is_mcp_mode` - When `true`, write to file at `DEBUG` and set
///   `SAH_CLI_MODE=1`
pub async fn configure_logging(verbose: bool, debug: bool, quiet: bool, is_mcp_mode: bool) {
    let log_level = determine_log_level(is_mcp_mode, verbose, debug, quiet);

    if is_mcp_mode {
        if let Err(e) = setup_mcp_logging(log_level) {
            // Setup stderr logging first so we can emit the warning through tracing
            setup_stderr_logging(log_level);
            tracing::warn!(
                "Could not setup MCP logging: {}. Falling back to stderr.",
                e
            );
        }
    } else {
        setup_stderr_logging(log_level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_log_file_name_accepts_bare_names() {
        assert_eq!(validate_log_file_name("mcp.log").unwrap(), "mcp.log");
        assert_eq!(
            validate_log_file_name("server-1.log").unwrap(),
            "server-1.log"
        );
        assert_eq!(validate_log_file_name("log.txt").unwrap(), "log.txt");
    }

    #[test]
    fn validate_log_file_name_rejects_parent_traversal() {
        // Classic traversal attempt.
        assert!(validate_log_file_name("../etc/passwd").is_err());
        // Sneakier form that's still rooted in a parent component.
        assert!(validate_log_file_name("..").is_err());
    }

    #[test]
    fn validate_log_file_name_rejects_separators() {
        assert!(validate_log_file_name("sub/dir.log").is_err());
        assert!(validate_log_file_name("a/b").is_err());
    }

    #[test]
    fn validate_log_file_name_rejects_absolute_paths() {
        assert!(validate_log_file_name("/etc/passwd").is_err());
        assert!(validate_log_file_name("/tmp/evil.log").is_err());
    }

    #[test]
    fn validate_log_file_name_rejects_dot_and_empty() {
        // `.` is a `CurDir` component, not `Normal` — must be rejected.
        assert!(validate_log_file_name(".").is_err());
        // Empty string parses to zero components.
        assert!(validate_log_file_name("").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn validate_log_file_name_rejects_windows_separators() {
        assert!(validate_log_file_name("sub\\dir.log").is_err());
        assert!(validate_log_file_name("C:\\evil.log").is_err());
    }
}
