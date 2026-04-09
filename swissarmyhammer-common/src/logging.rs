//! Logging utilities shared across SwissArmyHammer CLI crates.
//!
//! Provides [`FileWriterGuard`], a thread-safe writer that flushes and syncs
//! every write to disk. Used by MCP servers and CLIs to guarantee log data is
//! immediately visible for debugging, even if the process crashes.

use std::fs::File;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

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
        let mut file = self
            .file
            .lock()
            .expect("FileWriterGuard mutex was poisoned");
        let n = file.write(buf)?;
        file.flush()?;
        file.sync_all()?;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut file = self
            .file
            .lock()
            .expect("FileWriterGuard mutex was poisoned");
        file.flush()?;
        file.sync_all()
    }
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
}
