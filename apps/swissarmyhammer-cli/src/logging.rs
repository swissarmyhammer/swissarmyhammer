//! File-based tracing for the `sah` CLI / MCP server.
//!
//! In MCP (`serve`) mode, logs are written under `<sah-root>/.sah/` to a
//! per-process file (`mcp.<pid>.log`, from
//! [`log_file_name`](swissarmyhammer_common::logging::log_file_name)) using
//! [`FileWriterGuard`](swissarmyhammer_common::logging::FileWriterGuard) so
//! every message is flushed and synced to disk immediately — critical when
//! the host process dies before a normal shutdown. The per-process name means
//! several concurrent `sah serve` processes in one workspace (for example the
//! parallel servers a batch workflow spawns) never truncate or interleave one
//! another's log. When the log file cannot be created (or the CLI is not in MCP
//! mode), tracing falls back to stderr.
//!
//! This wrapper reuses the shared
//! [`init_file_tracing_with_fallback_named`](swissarmyhammer_common::logging::init_file_tracing_with_fallback_named)
//! chokepoint rather than opening the file itself; it only adds sah's filter and
//! its `SWISSARMYHAMMER_LOG_FILE` override (honored verbatim).
//!
//! The public entry point is [`configure_logging`].

use std::path::{Path, PathBuf};
use swissarmyhammer_common::directory::{DirectoryConfig, SwissarmyhammerConfig};
use swissarmyhammer_common::logging::{
    init_file_tracing_with_fallback_named, log_file_name, DirPolicy,
};
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

/// Resolve the parent directory that holds the `.sah/` data directory.
///
/// Tries the git root first so MCP servers started from arbitrary subdirectories
/// log to a single, consistent location per repository; falls back to the
/// current working directory when we're not inside a git repo.
///
/// Returns the directory *containing* `.sah/` (e.g. the repository root), NOT
/// the `.sah/` path itself: the shared
/// [`init_file_tracing_with_fallback_named`] chokepoint joins this root with
/// [`SwissarmyhammerConfig::DIR_NAME`] and auto-creates the data directory, so
/// the `.sah/`-creation responsibility lives entirely in the shared helper and
/// is not duplicated here.
fn swissarmyhammer_root() -> Result<PathBuf, std::io::Error> {
    use swissarmyhammer_common::SwissarmyhammerDirectory;

    let sah_dir = SwissarmyhammerDirectory::from_git_root()
        .or_else(|_| SwissarmyhammerDirectory::from_custom_root(std::env::current_dir()?))
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    // `root()` is the `.sah/` directory; its parent is the location the shared
    // logging chokepoint expects to join `.sah/` onto.
    let sah_path = sah_dir.root();
    sah_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| std::io::Error::other("resolved .sah directory has no parent"))
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

/// Resolve the log file name `sah serve` writes inside the `.sah/` directory.
///
/// - **Default (no override):** the shared per-process name from
///   [`log_file_name`] — `mcp.<pid>.log`. This is what makes two concurrent
///   `sah serve` processes in one workspace stop clobbering each other: each
///   owns a distinct file name, so neither truncates the other's log.
/// - **Explicit override:** if `SWISSARMYHAMMER_LOG_FILE` is set, its value is
///   honored *verbatim* (after [`validate_log_file_name`] rejects anything that
///   is not a bare file name). An explicitly requested path is never pid-mangled
///   — only the default workspace log gets the per-process name.
///
/// Returns an [`std::io::Error`] of kind
/// [`InvalidInput`](std::io::ErrorKind::InvalidInput) when an override is set
/// but is not a valid bare file name.
fn resolve_log_file_name() -> Result<String, std::io::Error> {
    match std::env::var("SWISSARMYHAMMER_LOG_FILE").ok() {
        Some(name) => validate_log_file_name(&name).map(str::to_owned),
        None => Ok(log_file_name()),
    }
}

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
/// Routes through the shared
/// [`init_file_tracing_with_fallback_named`] chokepoint so `sah serve` reuses
/// the same file-opening + [`FileWriterGuard`] + stderr-fallback wiring as every
/// other CLI (kanban, shelltool, code-context) — there is no separate
/// `File::create` here. The on-disk file is created under `<sah-root>/.sah/`
/// using the name from [`resolve_log_file_name`]: the per-process
/// `mcp.<pid>.log` by default (so concurrent serves in one workspace never
/// clobber each other), or an explicit `SWISSARMYHAMMER_LOG_FILE` honored
/// verbatim.
///
/// The `SAH_CLI_MODE=1` env var is set to prevent the unified server from
/// reconfiguring logging downstream.
///
/// The override env var is validated to contain only a bare file name — any
/// value with path separators, parent-directory references, or an absolute
/// path is rejected with an error so a hostile or misconfigured environment
/// can't redirect log output to an arbitrary location on disk.
///
/// Returns an error if the log directory cannot be located or the override
/// file name is invalid; callers are expected to fall back to stderr on
/// failure. A failure to create the log file itself does NOT error here — the
/// shared chokepoint falls back to stderr internally and reports the error,
/// which is logged as a warning.
fn setup_mcp_logging(
    log_level: tracing::Level,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Set flag to prevent unified server from also configuring logging
    std::env::set_var("SAH_CLI_MODE", "1");

    // Resolve the log file name (per-process default or explicit override)
    // before touching the filesystem so an invalid override fails fast.
    let file_name = resolve_log_file_name()?;

    // The directory containing `.sah/`; the shared chokepoint joins and
    // auto-creates the `.sah/` data directory itself.
    let root = swissarmyhammer_root()?;

    if let Some(e) = init_file_tracing_with_fallback_named(
        make_filter(log_level),
        &root,
        SwissarmyhammerConfig::DIR_NAME,
        DirPolicy::AutoCreate,
        &file_name,
    ) {
        tracing::warn!(
            "Could not setup MCP log file: {}. Falling back to stderr.",
            e
        );
    }

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
/// In MCP (`serve`) mode, logs are written under `<sah-root>/.sah/` to the
/// per-process `mcp.<pid>.log` (or an explicit `SWISSARMYHAMMER_LOG_FILE`) via
/// the shared `FileWriterGuard` chokepoint (flush + sync on every write). In all
/// other cases — and as the fallback when the log file cannot be opened — logs
/// go to stderr. A warning is logged when file logging setup fails so the
/// missing log file isn't surprising.
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

    #[test]
    #[serial_test::serial(env)]
    fn resolve_log_file_name_defaults_to_per_process_name() {
        // With no override set, the default log file name must come from the
        // shared per-process helper (`mcp.<pid>.log`) so that two concurrent
        // `sah serve` processes in one workspace never clobber each other.
        let _guard = EnvVarGuard::unset("SWISSARMYHAMMER_LOG_FILE");
        assert_eq!(
            resolve_log_file_name().unwrap(),
            swissarmyhammer_common::logging::log_file_name()
        );
    }

    #[test]
    #[serial_test::serial(env)]
    fn resolve_log_file_name_honors_explicit_override_verbatim() {
        // An explicit SWISSARMYHAMMER_LOG_FILE must be honored as-is — NOT
        // pid-mangled — so a caller that asks for a specific file gets it.
        let _guard = EnvVarGuard::set("SWISSARMYHAMMER_LOG_FILE", "explicit.log");
        assert_eq!(resolve_log_file_name().unwrap(), "explicit.log");
    }

    #[test]
    #[serial_test::serial(env)]
    fn resolve_log_file_name_rejects_path_override() {
        let _guard = EnvVarGuard::set("SWISSARMYHAMMER_LOG_FILE", "../escape.log");
        assert!(resolve_log_file_name().is_err());
    }

    /// Save/restore a single env var around a test so the override-sensitive
    /// `resolve_log_file_name` cases don't leak into one another.
    struct EnvVarGuard {
        key: &'static str,
        prev: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let prev = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, prev }
        }

        fn unset(key: &'static str) -> Self {
            let prev = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, prev }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }
}
