#![allow(dead_code)]
/// A thread-safe writer wrapper that ensures immediate flushing and disk synchronization for MCP logging.
///
/// This struct wraps a `File` in `Arc<Mutex<>>` to provide thread-safe access while ensuring
/// that all writes are immediately flushed to the operating system and synced to disk.
/// This behavior is critical for MCP (Model Context Protocol) servers where log data must
/// be immediately available for debugging purposes.
///
/// # Thread Safety
///
/// Multiple threads can safely write to the same `FileWriterGuard` instance. Each write
/// operation acquires the mutex lock, writes the data, flushes the OS buffer, and
/// synchronizes to disk before releasing the lock.
///
/// # Performance Considerations
///
/// This implementation prioritizes data reliability over performance by calling `sync_all()`
/// on every write operation. This ensures data is written to disk immediately but may
/// impact performance in high-throughput scenarios.
///
/// # Example
///
/// ```no_run
/// use std::sync::{Arc, Mutex};
/// use std::fs::File;
/// use swissarmyhammer_cli::logging::FileWriterGuard;
///
/// let file = File::create("log.txt").unwrap();
/// let shared_file = Arc::new(Mutex::new(file));
/// let mut guard = FileWriterGuard::new(shared_file);
///
/// // This write will be immediately flushed and synced to disk
/// guard.write_all(b"Log message\n").unwrap();
/// ```
pub struct FileWriterGuard {
    file: std::sync::Arc<std::sync::Mutex<std::fs::File>>,
}

impl FileWriterGuard {
    /// Creates a new `FileWriterGuard` wrapping the given file.
    ///
    /// # Arguments
    ///
    /// * `file` - A thread-safe reference to a file that will be written to
    ///
    /// # Returns
    ///
    /// A new `FileWriterGuard` instance that will ensure immediate flushing for all writes
    pub fn new(file: std::sync::Arc<std::sync::Mutex<std::fs::File>>) -> Self {
        Self { file }
    }
}

impl std::io::Write for FileWriterGuard {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut file = self.file.lock().expect("FileWriterGuard mutex was poisoned - this indicates a panic occurred while another thread held the lock");
        let result = file.write(buf)?;
        file.flush()?; // Flush immediately for MCP mode
                       // Ensure data is written to disk immediately
        file.sync_all()?;
        Ok(result)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut file = self.file.lock().expect("FileWriterGuard flush mutex was poisoned - this indicates a panic occurred while another thread held the lock");
        file.flush()?;
        file.sync_all()?; // Force sync to disk
        Ok(())
    }
}

// Implement MakeWriter for tracing-subscriber compatibility
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for FileWriterGuard {
    type Writer = FileWriterGuard;

    fn make_writer(&'a self) -> Self::Writer {
        FileWriterGuard::new(self.file.clone())
    }
}
