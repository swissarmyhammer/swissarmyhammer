//! Shared test utilities for swissarmyhammer-treesitter
//!
//! This module provides common test helpers to avoid duplication across test modules.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

use crate::error::Result;
use crate::index::{IndexContext, IndexStatus, ScanResult};
use crate::watcher::IndexWatcherCallback;

/// Create a temporary directory with common test files
///
/// Creates:
/// - main.rs: `fn main() {}`
/// - lib.rs: `pub fn hello() {}`
/// - config.json: `{"key": "value"}`
/// - README.md: `# Hello`
/// - unsupported.xyz: `unknown`
pub fn setup_test_dir() -> TempDir {
    let dir = TempDir::new().unwrap();

    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
    std::fs::write(dir.path().join("lib.rs"), "pub fn hello() {}").unwrap();
    std::fs::write(dir.path().join("config.json"), r#"{"key": "value"}"#).unwrap();
    std::fs::write(dir.path().join("README.md"), "# Hello").unwrap();
    std::fs::write(dir.path().join("unsupported.xyz"), "unknown").unwrap();

    dir
}

/// Create a minimal temporary directory with just one Rust file
///
/// Creates:
/// - main.rs: `fn main() {}`
pub fn setup_minimal_test_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
    dir
}

/// Collects IndexStatus updates from progress callbacks
///
/// # Example
///
/// ```ignore
/// let collector = ProgressCollector::new();
/// let mut context = IndexContext::new(path).with_progress(collector.callback());
/// context.scan().await?;
/// let updates = collector.updates();
/// ```
pub struct ProgressCollector {
    updates: Arc<Mutex<Vec<IndexStatus>>>,
}

impl ProgressCollector {
    /// Create a new progress collector
    pub fn new() -> Self {
        Self {
            updates: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get a callback function that collects status updates
    pub fn callback(&self) -> impl Fn(IndexStatus) + Send + Sync + 'static {
        let updates = self.updates.clone();
        move |status| {
            updates.lock().unwrap().push(status);
        }
    }

    /// Get all collected status updates
    pub fn updates(&self) -> Vec<IndexStatus> {
        self.updates.lock().unwrap().clone()
    }

    /// Get the number of updates collected
    pub fn count(&self) -> usize {
        self.updates.lock().unwrap().len()
    }
}

impl Default for ProgressCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Test callback for IndexWatcher that tracks call counts
///
/// # Example
///
/// ```ignore
/// let callback = TestWatcherCallback::new();
/// watcher.start(path, callback.clone()).await?;
/// // ... trigger file changes ...
/// assert!(callback.changed_count() > 0);
/// ```
#[derive(Clone)]
pub struct TestWatcherCallback {
    changed_count: Arc<AtomicUsize>,
    removed_count: Arc<AtomicUsize>,
    error_count: Arc<AtomicUsize>,
}

impl TestWatcherCallback {
    /// Create a new test callback with all counts at zero
    pub fn new() -> Self {
        Self {
            changed_count: Arc::new(AtomicUsize::new(0)),
            removed_count: Arc::new(AtomicUsize::new(0)),
            error_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Get the number of files changed
    pub fn changed_count(&self) -> usize {
        self.changed_count.load(Ordering::SeqCst)
    }

    /// Get the number of files removed
    pub fn removed_count(&self) -> usize {
        self.removed_count.load(Ordering::SeqCst)
    }

    /// Get the number of errors
    pub fn error_count(&self) -> usize {
        self.error_count.load(Ordering::SeqCst)
    }
}

impl Default for TestWatcherCallback {
    fn default() -> Self {
        Self::new()
    }
}

impl IndexWatcherCallback for TestWatcherCallback {
    async fn on_files_changed(&self, paths: Vec<PathBuf>) -> Result<()> {
        self.changed_count.fetch_add(paths.len(), Ordering::SeqCst);
        Ok(())
    }

    async fn on_files_removed(&self, paths: Vec<PathBuf>) {
        self.removed_count.fetch_add(paths.len(), Ordering::SeqCst);
    }

    async fn on_error(&self, _error: String) {
        self.error_count.fetch_add(1, Ordering::SeqCst);
    }
}

// Note: CallCounter functionality is provided by ProgressCollector::count()
// Use ProgressCollector when you need to track callback invocations.

/// Result of running a progress test setup
///
/// Contains all the data needed to verify progress callback behavior.
#[allow(dead_code)]
pub struct ProgressTestResult {
    /// The temporary directory (must be kept alive for the duration of the test)
    pub dir: TempDir,
    /// The scanned index context
    pub context: IndexContext,
    /// The scan result
    pub scan_result: ScanResult,
    /// All progress updates received during scan
    pub updates: Vec<IndexStatus>,
}

/// Run a standard progress callback test setup
///
/// Creates a test directory, sets up an IndexContext with a progress collector,
/// scans, and returns all the results for assertions.
///
/// # Example
///
/// ```ignore
/// let result = run_progress_test().await;
/// assert!(result.updates.len() >= 3);
/// assert!(result.updates.last().unwrap().is_ready);
/// ```
pub async fn run_progress_test() -> ProgressTestResult {
    let dir = setup_test_dir();
    let collector = ProgressCollector::new();

    let mut context = IndexContext::new(dir.path()).with_progress(collector.callback());
    let scan_result = context.scan().await.unwrap();
    let updates = collector.updates();

    ProgressTestResult {
        dir,
        context,
        scan_result,
        updates,
    }
}
