/// A thread-safe writer wrapper that ensures immediate flushing and disk synchronization for log files.
///
/// This struct wraps a `File` in `Arc<Mutex<>>` to provide thread-safe access while ensuring
/// that all writes are immediately flushed to the operating system and synced to disk.
///
/// # Thread Safety
///
/// Multiple threads can safely write to the same `FileWriterGuard` instance. Each write
/// operation acquires the mutex lock, writes the data, flushes the OS buffer, and
/// synchronizes to disk before releasing the lock.
pub struct FileWriterGuard {
    file: std::sync::Arc<std::sync::Mutex<std::fs::File>>,
}

impl FileWriterGuard {
    /// Creates a new `FileWriterGuard` wrapping the given file.
    pub fn new(file: std::sync::Arc<std::sync::Mutex<std::fs::File>>) -> Self {
        Self { file }
    }
}

impl std::io::Write for FileWriterGuard {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut file = self
            .file
            .lock()
            .expect("FileWriterGuard mutex was poisoned");
        let result = file.write(buf)?;
        file.flush()?;
        file.sync_all()?;
        Ok(result)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut file = self
            .file
            .lock()
            .expect("FileWriterGuard flush mutex was poisoned");
        file.flush()?;
        file.sync_all()?;
        Ok(())
    }
}
