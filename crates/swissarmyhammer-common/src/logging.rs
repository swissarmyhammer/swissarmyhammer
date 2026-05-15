//! Logging utilities shared across SwissArmyHammer CLI crates.
//!
//! Provides:
//!
//! - [`FileWriterGuard`], a thread-safe writer that flushes and syncs every
//!   write to disk. Used by MCP servers and CLIs to guarantee log data is
//!   immediately visible for debugging, even if the process crashes.
//! - [`open_log_file`], which resolves a per-CLI log file under a data
//!   directory and emits a consistent stderr warning on creation failure.
//! - [`init_file_tracing_with_fallback`], which installs a global tracing
//!   subscriber that writes to that log file (via `FileWriterGuard`) with a
//!   stderr fallback.
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

/// The log file name written inside each CLI's data directory.
///
/// Every CLI writes to `<data-dir>/mcp.log`, so this constant is centralized
/// here rather than duplicated in each wrapper.
pub const LOG_FILE_NAME: &str = "mcp.log";

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

/// Resolve the tracing log file under `root/<dir_name>/<LOG_FILE_NAME>`.
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
    let log_file_path = log_dir.join(LOG_FILE_NAME);
    match File::create(&log_file_path) {
        Ok(file) => (Some(file), None),
        Err(e) => (None, Some(e)),
    }
}

/// Install a global tracing subscriber with file-based logging and a stderr
/// fallback.
///
/// When [`open_log_file`] returns a file, logs are written to it via a
/// [`FileWriterGuard`] (flush + sync on every write). When it returns `None`
/// — whether from a silent absent-directory path or a creation failure —
/// tracing falls back to stderr using the same filter.
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
    let (file, error) = open_log_file(root, dir_name, policy);

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
        assert!(data_dir.join(LOG_FILE_NAME).exists());
    }

    #[test]
    fn open_log_file_auto_create_makes_dir_and_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();

        let (file, error) = open_log_file(root, ".shell", DirPolicy::AutoCreate);

        assert!(file.is_some());
        assert!(error.is_none());
        assert!(root.join(".shell").is_dir());
        assert!(root.join(".shell").join(LOG_FILE_NAME).exists());
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
