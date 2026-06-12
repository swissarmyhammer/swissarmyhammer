//! Logging utilities shared across SwissArmyHammer CLI crates.
//!
//! Provides:
//!
//! - [`FileWriterGuard`], a thread-safe writer that flushes and syncs every
//!   write to disk. Used by MCP servers and CLIs to guarantee log data is
//!   immediately visible for debugging, even if the process crashes.
//! - [`open_log_file`], which resolves a per-process log file
//!   (`mcp.<pid>.log`, see [`log_file_name`]) under a data directory so
//!   concurrent processes in one workspace never clobber each other's logs,
//!   and emits a consistent stderr warning on creation failure.
//! - [`init_file_tracing_with_fallback`], which installs a global tracing
//!   subscriber that writes to that log file (via `FileWriterGuard`) with a
//!   stderr fallback, and [`init_file_tracing_with_fallback_named`] for callers
//!   (e.g. the sah CLI's `SWISSARMYHAMMER_LOG_FILE` override) that resolve their
//!   own file name yet still want the shared open + fallback wiring.
//!
//! Together, these let each CLI's `logging.rs` shrink to a `make_filter`
//! helper plus a one-line entry point, keeping the "standard pattern"
//! consistent across crates.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// The base log file name from which each process derives its own file.
///
/// This is the stem source, not the on-disk file name: [`open_log_file`] does
/// NOT write to `<data-dir>/mcp.log` directly. Instead [`log_file_name`]
/// inserts the process id between the stem and extension, producing
/// `mcp.<pid>.log`, so concurrent processes sharing a workspace never target
/// the same file. The constant is centralized here rather than duplicated in
/// each wrapper.
pub const LOG_FILE_NAME: &str = "mcp.log";

/// Resolve the per-process log file name derived from [`LOG_FILE_NAME`].
///
/// The process id from [`std::process::id`] is inserted between the stem and
/// extension of [`LOG_FILE_NAME`], yielding e.g. `mcp.<pid>.log`. Each running
/// process therefore owns a distinct file name, so multiple processes logging
/// into the same data directory (for example, the parallel `sah serve`
/// instances a batch workflow spawns) never clobber one another's output.
///
/// The name is data-driven from [`LOG_FILE_NAME`] rather than re-hardcoding the
/// `mcp` stem: the stem and extension are taken from the constant, and an
/// extensionless or stemless constant degrades gracefully (the pid is appended
/// before any extension that exists).
pub fn log_file_name() -> String {
    let base = Path::new(LOG_FILE_NAME);
    let stem = base
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(LOG_FILE_NAME);
    let pid = std::process::id();
    match base.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("{stem}.{pid}.{ext}"),
        None => format!("{stem}.{pid}"),
    }
}

/// Policy for handling a missing data directory when opening a log file.
///
/// Different CLIs have different contracts about their data directories:
///
/// - `.kanban/` holds committed board data and must NOT be auto-created by
///   the logger — it is initialized through the normal kanban workflow.
/// - `.shell/` and `.code-context/` hold runtime data and are auto-created
///   on demand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirPolicy {
    /// The data directory must already exist. When absent, `open_log_file`
    /// returns `None` silently (no stderr warning) — the caller falls back to
    /// stderr. This matches the kanban workflow, where the directory signals
    /// whether logging to file is expected at all.
    MustExist,
    /// Auto-create the data directory if missing, emitting a stderr warning
    /// on filesystem failure. This matches runtime data directories that are
    /// created on demand.
    AutoCreate,
}

/// A thread-safe writer that ensures immediate flushing and disk synchronization.
///
/// Wraps a `File` in `Arc<Mutex<_>>` so the tracing subscriber can clone
/// writers across threads while guaranteeing each write is immediately flushed
/// and synced to disk.
///
/// # Thread Safety
///
/// Multiple threads can safely write through cloned `FileWriterGuard` instances.
/// Each write acquires the mutex, writes data, flushes the OS buffer, and calls
/// `sync_all` before releasing the lock.
///
/// # Performance
///
/// This implementation prioritizes data reliability over throughput by calling
/// `sync_all()` on every write. This ensures data reaches disk immediately but
/// may impact performance in high-throughput scenarios.
///
/// # Example
///
/// ```no_run
/// use std::sync::{Arc, Mutex};
/// use std::io::Write;
/// use swissarmyhammer_common::logging::FileWriterGuard;
///
/// let file = std::fs::File::create("log.txt").unwrap();
/// let shared = Arc::new(Mutex::new(file));
/// let mut guard = FileWriterGuard::new(shared);
///
/// // This write is immediately flushed and synced to disk.
/// guard.write_all(b"Log message\n").unwrap();
/// ```
pub struct FileWriterGuard {
    file: Arc<Mutex<File>>,
}

impl FileWriterGuard {
    /// Create a new guard wrapping the given shared file handle.
    pub fn new(file: Arc<Mutex<File>>) -> Self {
        Self { file }
    }
}

impl Write for FileWriterGuard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Best-effort write path: if a previous writer panicked and left the
        // mutex poisoned, recover the inner guard instead of propagating the
        // poison. Double-panicking from a logging sink on an already-unwinding
        // thread would just hide the original failure — we'd rather write the
        // log line.
        let mut file = self.file.lock().unwrap_or_else(|e| e.into_inner());
        let n = file.write(buf)?;
        file.flush()?;
        file.sync_all()?;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut file = self.file.lock().unwrap_or_else(|e| e.into_inner());
        file.flush()?;
        file.sync_all()
    }
}

/// Resolve a fresh, per-process tracing log file under `root/<dir_name>/`.
///
/// The on-disk file name comes from [`log_file_name`], i.e. `mcp.<pid>.log`,
/// NOT the bare [`LOG_FILE_NAME`]. This is the centralized chokepoint every CLI
/// (sah, kanban, shell, code-context) inherits: because each process owns a
/// distinct file name, multiple processes logging into the same data directory
/// never truncate or interleave one another's output. The file is still
/// created fresh (truncate-on-create) — but only this process ever owns that
/// name, so fresh-per-run semantics and bounded per-process growth are
/// preserved without the shared-file clobbering.
///
/// The directory-creation behavior is controlled by `policy`:
///
/// - [`DirPolicy::MustExist`]: returns `(None, None)` silently when the directory
///   does not already exist — the caller falls back to stderr. If the
///   directory exists but the log file cannot be created, returns the error.
/// - [`DirPolicy::AutoCreate`]: creates the directory on demand. Returns any
///   filesystem error so the caller can decide how to handle it.
///
/// # Arguments
///
/// * `root` - The directory to look under for the data directory
///   (typically the current working directory).
/// * `dir_name` - The name of the data directory (e.g. `.kanban`, `.shell`).
/// * `policy` - Whether the directory may be auto-created.
///
/// # Returns
///
/// A tuple of `(Option<File>, Option<std::io::Error>)`. The file is present
/// if setup succeeded; the error is present if setup failed after initial
/// directory checks (so the caller can emit a warning if appropriate).
pub fn open_log_file(
    root: &Path,
    dir_name: &str,
    policy: DirPolicy,
) -> (Option<File>, Option<std::io::Error>) {
    open_log_file_named(root, dir_name, policy, &log_file_name())
}

/// Open a log file with an explicit `file_name` under `root/<dir_name>/`.
///
/// Shared implementation behind [`open_log_file`], which supplies the
/// per-process [`log_file_name`]. Exposed so callers that resolve their own
/// file name — for example the sah CLI, which honors an explicit
/// `SWISSARMYHAMMER_LOG_FILE` override verbatim while defaulting to the
/// per-process [`log_file_name`] — can reuse this exact chokepoint instead of
/// re-implementing `File::create`. Also lets the no-clobber property be
/// exercised with two distinct, deterministic names within a single test
/// process (which necessarily shares one pid). `policy` controls directory
/// creation exactly as documented on [`open_log_file`].
pub fn open_log_file_named(
    root: &Path,
    dir_name: &str,
    policy: DirPolicy,
    file_name: &str,
) -> (Option<File>, Option<std::io::Error>) {
    let log_dir = root.join(dir_name);
    match policy {
        DirPolicy::MustExist => {
            if !log_dir.is_dir() {
                // Absent-directory path is silent by design: the missing
                // directory is the caller's contract for "don't log to file".
                return (None, None);
            }
        }
        DirPolicy::AutoCreate => {
            if let Err(e) = std::fs::create_dir_all(&log_dir) {
                return (None, Some(e));
            }
        }
    }
    let log_file_path = log_dir.join(file_name);
    match File::create(&log_file_path) {
        Ok(file) => (Some(file), None),
        Err(e) => (None, Some(e)),
    }
}

/// Install a global tracing subscriber with file-based logging and a stderr
/// fallback.
///
/// When [`open_log_file`] returns a file, logs are written to it via a
/// [`FileWriterGuard`] (flush + sync on every write). The file is per-process
/// (`mcp.<pid>.log`, see [`log_file_name`]), so several processes initialized
/// against the same data directory each log to their own file and remain
/// independently readable. When [`open_log_file`] returns `None` — whether from
/// a silent absent-directory path or a creation failure — tracing falls back to
/// stderr using the same filter.
///
/// Both layers use `with_target(false)` and `with_ansi(false)` to keep log
/// output consistent across file and stderr destinations and free of ANSI
/// escape sequences that files don't render.
///
/// This function calls `.init()` on the tracing registry, so it can only be
/// called once per process.
///
/// # Arguments
///
/// * `filter` - The `EnvFilter` to apply to every emitted record.
/// * `root` - The directory to look under for the data directory
///   (typically the current working directory).
/// * `dir_name` - The name of the data directory (e.g. `.kanban`, `.shell`).
/// * `policy` - Whether the directory may be auto-created.
///
/// # Returns
///
/// An `Option<std::io::Error>` that is present if file logging setup failed
/// after initial directory checks. Callers can use this to emit a warning
/// through tracing after the subscriber is initialized.
pub fn init_file_tracing_with_fallback(
    filter: EnvFilter,
    root: &Path,
    dir_name: &str,
    policy: DirPolicy,
) -> Option<std::io::Error> {
    init_file_tracing_with_fallback_named(filter, root, dir_name, policy, &log_file_name())
}

/// Install a global tracing subscriber writing to an explicitly named log file,
/// with a stderr fallback.
///
/// Identical to [`init_file_tracing_with_fallback`] except the on-disk file
/// name is supplied by the caller rather than defaulting to the per-process
/// [`log_file_name`]. This is the single chokepoint a CLI reuses when it
/// resolves its own log file name — for example the sah CLI, which defaults to
/// the per-process [`log_file_name`] but honors an explicit
/// `SWISSARMYHAMMER_LOG_FILE` override verbatim — so no caller re-implements the
/// `File::create` + `FileWriterGuard` + stderr-fallback wiring.
///
/// Callers that want the per-process default should call
/// [`init_file_tracing_with_fallback`]; passing a stable name here forfeits the
/// concurrent-process no-clobber guarantee for that name.
///
/// # Arguments
///
/// * `filter` - The `EnvFilter` to apply to every emitted record.
/// * `root` - The directory to look under for the data directory.
/// * `dir_name` - The name of the data directory (e.g. `.kanban`, `.sah`).
/// * `policy` - Whether the directory may be auto-created.
/// * `file_name` - The bare log file name to create inside the data directory.
pub fn init_file_tracing_with_fallback_named(
    filter: EnvFilter,
    root: &Path,
    dir_name: &str,
    policy: DirPolicy,
    file_name: &str,
) -> Option<std::io::Error> {
    let (file, error) = open_log_file_named(root, dir_name, policy, file_name);

    if let Some(file) = file {
        let shared_file = Arc::new(Mutex::new(file));
        let file_layer = tracing_subscriber::fmt::layer()
            .with_target(false)
            .with_ansi(false)
            .with_writer(move || {
                let file = shared_file.clone();
                Box::new(FileWriterGuard::new(file)) as Box<dyn Write>
            })
            .with_filter(filter);
        tracing_subscriber::registry().with(file_layer).init();
        return None;
    }

    // Fallback: write to stderr when the log file cannot be opened.
    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .with_filter(filter);
    tracing_subscriber::registry().with(stderr_layer).init();

    // Return the error if one was captured, so callers can emit a warning
    error
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn write_flushes_to_disk() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let file = std::fs::File::create(&path).unwrap();
        let shared = Arc::new(Mutex::new(file));

        let mut guard = FileWriterGuard::new(shared);
        guard.write_all(b"hello").unwrap();

        // Read back immediately -- sync_all guarantees it's on disk.
        let mut contents = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert_eq!(contents, "hello");
    }

    #[test]
    fn flush_syncs_to_disk() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let file = std::fs::File::create(&path).unwrap();
        let shared = Arc::new(Mutex::new(file));

        let mut guard = FileWriterGuard::new(shared);
        guard.write_all(b"world").unwrap();
        guard.flush().unwrap();

        let mut contents = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert_eq!(contents, "world");
    }

    #[test]
    fn write_recovers_from_poisoned_mutex() {
        // Poison the mutex by panicking while holding the lock, then verify a
        // subsequent write via FileWriterGuard still succeeds instead of
        // double-panicking.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let file = std::fs::File::create(&path).unwrap();
        let shared = Arc::new(Mutex::new(file));

        let poisoner = Arc::clone(&shared);
        let _ = std::thread::spawn(move || {
            let _lock = poisoner.lock().unwrap();
            panic!("intentional panic to poison the mutex");
        })
        .join();

        assert!(
            shared.is_poisoned(),
            "precondition: the mutex should be poisoned for this test"
        );

        let mut guard = FileWriterGuard::new(Arc::clone(&shared));
        guard.write_all(b"after poison").unwrap();

        let mut contents = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert_eq!(contents, "after poison");
    }

    #[test]
    fn open_log_file_must_exist_returns_none_when_dir_absent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();

        let (file, error) = open_log_file(root, ".kanban", DirPolicy::MustExist);

        assert!(file.is_none());
        assert!(error.is_none());
        assert!(
            !root.join(".kanban").exists(),
            "MustExist policy must not auto-create the directory"
        );
    }

    #[test]
    fn open_log_file_must_exist_creates_log_file_when_dir_exists() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let data_dir = root.join(".kanban");
        std::fs::create_dir(&data_dir).unwrap();

        let (file, error) = open_log_file(root, ".kanban", DirPolicy::MustExist);

        assert!(file.is_some());
        assert!(error.is_none());
        // The on-disk name is per-process (`mcp.<pid>.log`), not the bare stem.
        assert!(data_dir.join(log_file_name()).exists());
    }

    #[test]
    fn open_log_file_auto_create_makes_dir_and_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();

        let (file, error) = open_log_file(root, ".shell", DirPolicy::AutoCreate);

        assert!(file.is_some());
        assert!(error.is_none());
        assert!(root.join(".shell").is_dir());
        // The on-disk name is per-process (`mcp.<pid>.log`), not the bare stem.
        assert!(root.join(".shell").join(log_file_name()).exists());
    }

    #[test]
    fn log_file_name_is_per_process() {
        // The resolved log file name must embed this process's pid so that
        // concurrent processes sharing a workspace never target the same path.
        let name = log_file_name();
        assert_eq!(name, format!("mcp.{}.log", std::process::id()));
    }

    #[test]
    fn concurrent_opens_do_not_clobber_each_other() {
        // Two "processes" opening a log against the SAME directory must each
        // get their own file; the first's content must survive the second's
        // open. With the old `File::create` on a single shared `mcp.log` path,
        // process B's open would truncate process A's file and this would fail.
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();

        let (file_a, err_a) =
            open_log_file_named(root, ".shell", DirPolicy::AutoCreate, "mcp.1001.log");
        assert!(err_a.is_none());
        let mut file_a = file_a.expect("process A log file");
        file_a.write_all(b"line from process A\n").unwrap();
        file_a.flush().unwrap();

        let (file_b, err_b) =
            open_log_file_named(root, ".shell", DirPolicy::AutoCreate, "mcp.1002.log");
        assert!(err_b.is_none());
        let mut file_b = file_b.expect("process B log file");
        file_b.write_all(b"line from process B\n").unwrap();
        file_b.flush().unwrap();

        // Process A's marker must still be present after process B opened.
        let mut a_contents = String::new();
        std::fs::File::open(root.join(".shell").join("mcp.1001.log"))
            .unwrap()
            .read_to_string(&mut a_contents)
            .unwrap();
        assert!(
            a_contents.contains("line from process A"),
            "process A's log line was clobbered by process B's open: {a_contents:?}"
        );

        // And process B's file is independently readable with its own content.
        let mut b_contents = String::new();
        std::fs::File::open(root.join(".shell").join("mcp.1002.log"))
            .unwrap()
            .read_to_string(&mut b_contents)
            .unwrap();
        assert!(b_contents.contains("line from process B"));
    }

    #[test]
    fn open_log_file_auto_create_returns_error_when_dir_cannot_be_created() {
        // Nest the would-be data dir under a path component that exists as a
        // regular file — `create_dir_all` cannot create a directory inside a
        // non-directory, so this reliably triggers the error branch without
        // needing filesystem permission tricks.
        let tmp = tempfile::TempDir::new().unwrap();
        let blocker = tmp.path().join("blocker");
        std::fs::write(&blocker, b"not a dir").unwrap();

        let (file, error) = open_log_file(&blocker, ".shell", DirPolicy::AutoCreate);

        assert!(file.is_none());
        assert!(error.is_some());
    }
}
