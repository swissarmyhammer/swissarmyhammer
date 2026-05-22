//! File watching functionality for MCP server

use async_watcher::{notify::RecursiveMode, AsyncDebouncer, DebouncedEvent};
use rmcp::RoleServer;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use swissarmyhammer_common::{Pretty, Result, SwissArmyHammerError};
use swissarmyhammer_prompts::PromptResolver;
use tokio::sync::Mutex;

/// Common prompt file extensions
const PROMPT_EXTENSIONS: &[&str] = &["md", "yaml", "yml", "markdown"];

/// Compound prompt file extensions (checked first due to specificity)
const COMPOUND_PROMPT_EXTENSIONS: &[&str] =
    &["md.liquid", "markdown.liquid", "yaml.liquid", "yml.liquid"];

/// Check if a file has a compound extension (more specific check)
fn has_compound_extension<P: AsRef<Path>>(path: P) -> bool {
    let path_str = path.as_ref().to_string_lossy().to_lowercase();
    COMPOUND_PROMPT_EXTENSIONS.iter().any(|&ext| {
        let extension = format!(".{ext}");
        path_str.ends_with(&extension)
    })
}

/// Check if a file has a prompt extension
fn is_prompt_file<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        PROMPT_EXTENSIONS.contains(&ext_str.as_str())
    } else {
        false
    }
}

/// Check if a file is any kind of prompt file (simple or compound extension)
fn is_any_prompt_file<P: AsRef<Path>>(path: P) -> bool {
    has_compound_extension(&path) || is_prompt_file(path)
}

/// Callback trait for handling file system events
pub trait FileWatcherCallback: Send + Sync + 'static {
    /// Called when a relevant file change is detected
    fn on_file_changed(
        &self,
        paths: Vec<std::path::PathBuf>,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Called when the file watcher encounters an error
    fn on_error(&self, error: String) -> impl std::future::Future<Output = ()> + Send;
}

/// File watcher for monitoring prompt directories
pub struct FileWatcher {
    /// The async debouncer instance
    debouncer: Option<AsyncDebouncer<async_watcher::notify::RecommendedWatcher>>,
    /// Channel receiver for debounced events
    event_rx: Option<
        tokio::sync::mpsc::Receiver<
            std::result::Result<Vec<DebouncedEvent>, Vec<async_watcher::notify::Error>>,
        >,
    >,
    /// Handle to the background event processing task
    event_handle: Option<tokio::task::JoinHandle<()>>,
}

impl FileWatcher {
    /// Create a new file watcher instance.
    ///
    /// The file watcher starts in an inactive state. Call `start_watching()` to begin
    /// monitoring file system changes.
    ///
    /// # Example
    ///
    /// ```
    /// use swissarmyhammer_tools::mcp::file_watcher::FileWatcher;
    /// let mut watcher = FileWatcher::new();
    /// // watcher.start_watching(callback).await?;
    /// ```
    pub fn new() -> Self {
        Self {
            debouncer: None,
            event_rx: None,
            event_handle: None,
        }
    }

    /// Start watching prompt directories for changes.
    ///
    /// This replaces any previously-active watch on `self`. It is the
    /// `&mut self` convenience wrapper around [`FileWatcher::start`]: it builds
    /// a freshly-started watcher and moves it into `self`.
    ///
    /// Prefer [`FileWatcher::start`] when the watcher lives behind a shared
    /// lock (e.g. `Arc<Mutex<FileWatcher>>`): building off-lock keeps the slow
    /// macOS FSEvents `.watch()` registration out of the locked critical path,
    /// so concurrent shutdown does not block on it.
    pub async fn start_watching<C>(&mut self, callback: C) -> Result<()>
    where
        C: FileWatcherCallback + Clone,
    {
        // Stop existing watcher if running
        self.stop_watching();
        *self = Self::start(callback).await?;
        Ok(())
    }

    /// Build a fully-started `FileWatcher`, performing the slow FSEvents
    /// stream registration WITHOUT requiring `&mut self` or any shared lock,
    /// and WITHOUT pinning a tokio worker for the duration of the registration.
    ///
    /// WHY this is an associated constructor rather than `&mut self`: on macOS
    /// `debouncer.watcher().watch(path, Recursive)` blocks for seconds while it
    /// registers an FSEvents stream, and that registration serializes through
    /// the OS FSEvents subsystem. When the watcher lives behind a shared
    /// `Arc<Mutex<FileWatcher>>`, holding that lock across `.watch()` makes
    /// `stop_watching()` (shutdown) block for the full registration time.
    /// Building the watcher here — off any shared lock — lets the caller grab
    /// the lock only briefly to store the result, keeping shutdown fast.
    ///
    /// WHY the blocking build runs on a detached `std::thread`: the FSEvents
    /// `.watch()` call is synchronous and cannot be interrupted by `abort()`.
    /// If it ran inline on the background-startup tokio task, it would pin a
    /// tokio worker for the full ~4.5s registration. `abort()` would suspend
    /// the future but the worker stays blocked in `.watch()`, so when the test
    /// runtime is dropped, runtime teardown waits for that pinned worker —
    /// keeping in-process server tests at ~4.5s. Running the build on a
    /// dedicated `std::thread` and only `.await`ing the completion channel here
    /// keeps the async side cleanly abortable: nothing on the tokio runtime
    /// blocks on FSEvents, so an aborted startup never holds a worker and
    /// runtime teardown is immediate. The thread is short-lived (one per
    /// registration; concurrency bounded by the caller's parallelism). If the
    /// receiver is dropped (startup aborted), the built watcher is dropped on
    /// the thread, whose `Drop` detaches the debouncer teardown — so a late
    /// registration never resurrects a watcher and never blocks a tokio worker.
    ///
    /// Returns an inactive (no-debouncer) `FileWatcher` when there are no
    /// prompt directories to watch.
    pub async fn start<C>(callback: C) -> Result<Self>
    where
        C: FileWatcherCallback + Clone,
    {
        // Capture the current runtime handle so the off-runtime build thread
        // can drive the async debouncer construction (which internally spawns a
        // tokio task) and the event-processing task. `block_on`/`enter` on this
        // handle keeps those tasks on the existing runtime while the blocking
        // `.watch()` runs on the std::thread — off the tokio worker pool.
        let runtime = tokio::runtime::Handle::current();
        let (tx, rx) = tokio::sync::oneshot::channel::<Result<Self>>();

        // WHY std::thread (not spawn_blocking): spawn_blocking threads still
        // belong to the runtime's blocking pool and can delay runtime teardown;
        // a fully detached std::thread does not. The thread takes ownership of
        // everything it needs and reports back over the oneshot channel.
        std::thread::Builder::new()
            .name("fsevents-watch-register".to_string())
            .spawn(move || {
                let built = runtime.block_on(Self::build_blocking(callback));
                // If the receiver is gone (startup was aborted / shutdown ran),
                // the send fails and `built` is dropped here on this thread. Its
                // `Drop` detaches the debouncer teardown, so the abandoned
                // watcher is cleaned up without ever touching a tokio worker.
                let _ = tx.send(built);
            })
            .map_err(|e| SwissArmyHammerError::Other {
                message: format!("Failed to spawn FSEvents registration thread: {e}"),
            })?;

        // Await completion. This await is the only thing the background-startup
        // tokio task does for the registration, so aborting it suspends here
        // cleanly without pinning a worker in the synchronous `.watch()`.
        rx.await.map_err(|_| SwissArmyHammerError::Other {
            message: "FSEvents registration thread terminated before reporting a result"
                .to_string(),
        })?
    }

    /// Perform the blocking FSEvents registration and event-task spawn.
    ///
    /// This runs on the dedicated registration `std::thread` via `block_on`, so
    /// the synchronous `.watch()` call never occupies a tokio worker. The
    /// debouncer construction and event-processing task are spawned onto the
    /// captured runtime via the `block_on` runtime context.
    ///
    /// Returns an inactive (no-debouncer) `FileWatcher` when there are no
    /// prompt directories to watch.
    async fn build_blocking<C>(callback: C) -> Result<Self>
    where
        C: FileWatcherCallback + Clone,
    {
        tracing::info!("Starting file watching for prompt directories");

        // Get the directories to watch using the same logic as PromptResolver
        let resolver = PromptResolver::new();
        let watch_paths =
            resolver
                .get_prompt_directories()
                .map_err(|e| SwissArmyHammerError::Other {
                    message: e.to_string(),
                })?;

        tracing::info!(
            "Found {} directories to watch: {:?}",
            watch_paths.len(),
            watch_paths
        );

        // The resolver already returns only existing paths
        if watch_paths.is_empty() {
            tracing::warn!("No prompt directories found to watch");
            return Ok(Self::new());
        }

        // Create async debouncer with 500ms timeout and channel for events
        let (mut debouncer, event_rx) = AsyncDebouncer::new_with_channel(
            Duration::from_millis(500),
            None, // Use default tick rate
        )
        .await
        .map_err(|e| SwissArmyHammerError::Other {
            message: format!("Failed to create async debouncer: {}", e),
        })?;

        // Watch all directories. On macOS this `.watch()` call is the slow,
        // synchronous FSEvents stream registration. It runs on the dedicated
        // registration thread (not a tokio worker), so an aborted startup never
        // blocks runtime teardown on it.
        for path in &watch_paths {
            debouncer
                .watcher()
                .watch(path, RecursiveMode::Recursive)
                .map_err(|e| SwissArmyHammerError::Other {
                    message: format!("Failed to watch directory {path:?}: {}", e),
                })?;
            tracing::info!("Watching directory: {}", Pretty(&path));
        }

        // Spawn task to process events from async-watcher
        let mut event_rx_clone = event_rx;
        let handle = tokio::spawn(async move {
            while let Some(events_result) = event_rx_clone.recv().await {
                match events_result {
                    Ok(events) => {
                        #[derive(serde::Serialize, Debug)]
                        struct EventsInfo {
                            count: usize,
                            events: Vec<String>,
                        }
                        let events_info = EventsInfo {
                            count: events.len(),
                            events: events.iter().map(|e| format!("{:?}", e)).collect(),
                        };
                        tracing::debug!(
                            "📁 Debounced file system events: {}",
                            Pretty(&events_info)
                        );

                        // Filter for prompt files and collect all relevant paths
                        let relevant_paths: Vec<std::path::PathBuf> = events
                            .into_iter()
                            .flat_map(|event| event.event.paths)
                            .filter(|p| is_any_prompt_file(p))
                            .collect();

                        if !relevant_paths.is_empty() {
                            tracing::info!("📄 Prompt file changed: {}", Pretty(&relevant_paths));

                            // Notify callback about the change
                            if let Err(e) = callback.on_file_changed(relevant_paths).await {
                                tracing::error!("✗ File watcher callback failed: {}", e);
                                callback.on_error(format!("Callback failed: {e}")).await;
                            }
                        } else {
                            tracing::debug!("🚫 Ignoring non-prompt files in batch");
                        }
                    }
                    Err(errors) => {
                        for error in errors {
                            tracing::error!("✗ File watcher error: {}", error);
                            callback
                                .on_error(format!("File watcher error: {error}"))
                                .await;
                        }
                    }
                }
            }
            tracing::debug!("📁 File watcher task exiting");
        });

        // Return the fully-started watcher. `event_rx` was moved into the
        // event-processing task above, so it is intentionally not retained.
        Ok(Self {
            debouncer: Some(debouncer),
            event_rx: None,
            event_handle: Some(handle),
        })
    }

    /// Stop file watching
    pub fn stop_watching(&mut self) {
        // Drop the debouncer (which stops watching automatically).
        //
        // WHY a detached thread: dropping the `notify` `RecommendedWatcher`
        // inside the `AsyncDebouncer` blocks for ~5s on macOS FSEvents stream
        // teardown, and that teardown serializes through the OS FSEvents
        // subsystem — so under load (many watchers torn down at once) it queues
        // and dominates server `shutdown()` time. Moving the drop onto a
        // short-lived `std::thread` lets the caller return immediately while the
        // OS teardown completes off the critical path. `std::thread::spawn`
        // (not `tokio::task::spawn_blocking`) because `stop_watching` runs from
        // both async callers and `Drop`, which can execute outside a runtime.
        // Watch behavior is unchanged — only teardown timing moves off-path.
        if let Some(debouncer) = self.debouncer.take() {
            std::thread::spawn(move || drop(debouncer));
            tracing::debug!("📁 Async debouncer teardown detached");
        }

        // Close the event channel
        if let Some(_event_rx) = self.event_rx.take() {
            // Dropping the receiver will cause the sender to fail and the task to exit
        }

        // Abort the event processing task
        if let Some(handle) = self.event_handle.take() {
            handle.abort();
            tracing::debug!("📁 File watcher event task aborted");
        }
    }
}

impl FileWatcher {
    /// Test-only accessor: reports whether the watcher is currently inactive
    /// (no debouncer installed). Used by server-level tests to assert that a
    /// late, post-shutdown store was suppressed without exposing internals.
    #[cfg(test)]
    pub(crate) fn debouncer_is_none_for_test(&self) -> bool {
        self.debouncer.is_none()
    }
}

impl Default for FileWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.stop_watching();
    }
}

/// Callback implementation for file watcher that handles prompt reloading
#[derive(Clone)]
pub struct McpFileWatcherCallback {
    server: super::McpServer,
    peer: rmcp::Peer<RoleServer>,
}

impl McpFileWatcherCallback {
    /// Create a new file watcher callback with the given server and peer
    pub fn new(server: super::McpServer, peer: rmcp::Peer<RoleServer>) -> Self {
        Self { server, peer }
    }
}

impl FileWatcherCallback for McpFileWatcherCallback {
    async fn on_file_changed(&self, paths: Vec<std::path::PathBuf>) -> Result<()> {
        tracing::info!("📄 Prompt file changed: {}", Pretty(&paths));

        // Reload the library and check if content actually changed
        let has_changes = match self.server.reload_prompts().await {
            Ok(changed) => changed,
            Err(e) => {
                tracing::error!("✗ Failed to reload prompts: {}", e);
                return Err(e);
            }
        };
        tracing::info!("✓ Prompts reloaded successfully");

        // Only send notification to client if content actually changed
        if has_changes {
            let peer_clone = self.peer.clone();
            tokio::spawn(async move {
                match peer_clone.notify_prompt_list_changed().await {
                    Ok(_) => {
                        tracing::info!("📢 Sent prompts/listChanged notification to client");
                    }
                    Err(e) => {
                        tracing::error!("✗ Failed to send notification: {}", e);
                    }
                }
            });
        } else {
            tracing::info!("⏭️  Skipped notification (no content changes)");
        }

        Ok(())
    }

    async fn on_error(&self, error: String) {
        tracing::error!("✗ File watcher error: {}", error);
    }
}

/// Retry an async operation with exponential backoff
///
/// # Arguments
///
/// * `max_retries` - Maximum number of retry attempts
/// * `initial_backoff_ms` - Initial backoff duration in milliseconds
/// * `is_retryable` - Function to determine if an error is retryable
/// * `operation` - The async operation to retry
///
/// # Returns
///
/// Returns the result of the operation or the last error encountered
async fn retry_with_backoff<F, Fut, T, E>(
    max_retries: u32,
    initial_backoff_ms: u64,
    is_retryable: fn(&E) -> bool,
    mut operation: F,
) -> std::result::Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<T, E>>,
    E: std::fmt::Display,
{
    let mut backoff_ms = initial_backoff_ms;
    let mut last_error = None;

    for attempt in 1..=max_retries {
        match operation().await {
            Ok(value) => {
                if attempt > 1 {
                    tracing::info!("✓ Operation succeeded on attempt {}", attempt);
                }
                return Ok(value);
            }
            Err(e) => {
                let should_retry = attempt < max_retries && is_retryable(&e);

                if should_retry {
                    tracing::warn!(
                        "⚠️ Attempt {} failed, retrying in {}ms: {}",
                        attempt,
                        backoff_ms,
                        e
                    );

                    tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                    backoff_ms *= 2; // Exponential backoff
                    last_error = Some(e);
                } else {
                    return Err(e);
                }
            }
        }
    }

    Err(last_error.unwrap())
}

/// File watcher operations for MCP server
pub struct McpFileWatcher {
    file_watcher: Arc<Mutex<FileWatcher>>,
}

impl McpFileWatcher {
    /// Create a new MCP file watcher with the given file watcher instance
    pub fn new(file_watcher: Arc<Mutex<FileWatcher>>) -> Self {
        Self { file_watcher }
    }

    /// Start watching prompt directories for file changes.
    ///
    /// When files change, the server will automatically reload prompts and
    /// send notifications to the MCP client.
    ///
    /// # Arguments
    ///
    /// * `server` - The MCP server instance
    /// * `peer` - The MCP peer connection for sending notifications
    ///
    /// # Returns
    ///
    /// * `Result<()>` - Ok if watching starts successfully, error otherwise
    ///
    /// # Errors
    ///
    /// Returns an error if file watching cannot be initialized.
    pub async fn start_file_watching(
        &self,
        server: super::McpServer,
        peer: rmcp::Peer<RoleServer>,
    ) -> Result<()> {
        const MAX_RETRIES: u32 = 3;
        const INITIAL_BACKOFF_MS: u64 = 100;

        // Create callback that handles file changes and notifications
        let callback = McpFileWatcherCallback::new(server, peer);

        // Use retry logic to handle transient file system errors
        retry_with_backoff(
            MAX_RETRIES,
            INITIAL_BACKOFF_MS,
            Self::is_retryable_fs_error,
            || async {
                let mut watcher = self.file_watcher.lock().await;
                watcher.start_watching(callback.clone()).await
            },
        )
        .await
    }

    /// Stop watching prompt directories for file changes.
    ///
    /// This should be called when the MCP server is shutting down.
    pub async fn stop_file_watching(&self) {
        let mut watcher = self.file_watcher.lock().await;
        watcher.stop_watching();
    }

    /// Check if an error is a retryable file system error
    fn is_retryable_fs_error(error: &SwissArmyHammerError) -> bool {
        // Check for common transient file system errors
        if let SwissArmyHammerError::Io(io_err) = error {
            matches!(
                io_err.kind(),
                std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::Interrupted
                    | std::io::ErrorKind::WouldBlock
                    | std::io::ErrorKind::UnexpectedEof
            )
        } else {
            // Also retry if the error message contains certain patterns
            let error_str = error.to_string().to_lowercase();
            error_str.contains("temporarily unavailable")
                || error_str.contains("resource busy")
                || error_str.contains("locked")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    // ---------------------------------------------------------------------------
    // FileWatcher lifecycle tests
    // ---------------------------------------------------------------------------

    /// Test that `FileWatcher::new()` starts inactive — no task handle or debouncer.
    #[tokio::test]
    async fn test_file_watcher_new_is_inactive() {
        let watcher = FileWatcher::new();
        // A freshly created watcher has no running handles
        assert!(watcher.debouncer.is_none());
        assert!(watcher.event_rx.is_none());
        assert!(watcher.event_handle.is_none());
    }

    /// Test that `FileWatcher::default()` produces the same inactive state as `new()`.
    #[tokio::test]
    async fn test_file_watcher_default_is_inactive() {
        let watcher = FileWatcher::default();
        assert!(watcher.debouncer.is_none());
        assert!(watcher.event_rx.is_none());
        assert!(watcher.event_handle.is_none());
    }

    /// Test that calling `stop_watching()` on an already-stopped watcher is a no-op.
    #[tokio::test]
    async fn test_stop_watching_when_not_started_is_noop() {
        let mut watcher = FileWatcher::new();
        // Should not panic
        watcher.stop_watching();
        watcher.stop_watching(); // Idempotent
    }

    /// Test that dropping a `FileWatcher` calls `stop_watching()` via `Drop`.
    #[tokio::test]
    async fn test_file_watcher_drop_does_not_panic() {
        let watcher = FileWatcher::new();
        drop(watcher); // Should not panic
    }

    // ---------------------------------------------------------------------------
    // Extension-detection helpers
    // ---------------------------------------------------------------------------

    /// Test that `is_prompt_file` correctly identifies prompt file extensions.
    #[test]
    fn test_is_prompt_file_with_prompt_extensions() {
        assert!(is_prompt_file(std::path::Path::new("prompt.md")));
        assert!(is_prompt_file(std::path::Path::new("prompt.yaml")));
        assert!(is_prompt_file(std::path::Path::new("prompt.yml")));
        assert!(is_prompt_file(std::path::Path::new("prompt.markdown")));
    }

    /// Test that `is_prompt_file` rejects non-prompt extensions.
    #[test]
    fn test_is_prompt_file_with_non_prompt_extensions() {
        assert!(!is_prompt_file(std::path::Path::new("file.rs")));
        assert!(!is_prompt_file(std::path::Path::new("file.txt")));
        assert!(!is_prompt_file(std::path::Path::new("file.json")));
        assert!(!is_prompt_file(std::path::Path::new("noext")));
    }

    /// Test that `has_compound_extension` detects `.md.liquid` and similar.
    #[test]
    fn test_has_compound_extension_detects_liquid_variants() {
        assert!(has_compound_extension(std::path::Path::new(
            "prompt.md.liquid"
        )));
        assert!(has_compound_extension(std::path::Path::new(
            "prompt.yaml.liquid"
        )));
        assert!(has_compound_extension(std::path::Path::new(
            "prompt.yml.liquid"
        )));
        assert!(has_compound_extension(std::path::Path::new(
            "prompt.markdown.liquid"
        )));
    }

    /// Test that `has_compound_extension` rejects plain extensions.
    #[test]
    fn test_has_compound_extension_rejects_plain_extensions() {
        assert!(!has_compound_extension(std::path::Path::new("prompt.md")));
        assert!(!has_compound_extension(std::path::Path::new("file.rs")));
    }

    /// Test that `is_any_prompt_file` accepts both simple and compound extensions.
    #[test]
    fn test_is_any_prompt_file_combines_checks() {
        assert!(is_any_prompt_file(std::path::Path::new("a.md")));
        assert!(is_any_prompt_file(std::path::Path::new("a.yaml.liquid")));
        assert!(!is_any_prompt_file(std::path::Path::new("a.rs")));
    }

    // ---------------------------------------------------------------------------
    // FileWatcherCallback mock and behaviour tests
    // ---------------------------------------------------------------------------

    /// A minimal mock implementation of `FileWatcherCallback` that records calls.
    #[derive(Clone)]
    struct MockCallback {
        changed_count: Arc<AtomicUsize>,
        error_count: Arc<AtomicUsize>,
        /// When `true`, `on_file_changed` returns an error.
        fail_on_change: Arc<AtomicBool>,
    }

    impl MockCallback {
        fn new() -> Self {
            Self {
                changed_count: Arc::new(AtomicUsize::new(0)),
                error_count: Arc::new(AtomicUsize::new(0)),
                fail_on_change: Arc::new(AtomicBool::new(false)),
            }
        }

        fn changed_count(&self) -> usize {
            self.changed_count.load(Ordering::SeqCst)
        }

        fn error_count(&self) -> usize {
            self.error_count.load(Ordering::SeqCst)
        }

        fn set_fail_on_change(&self, fail: bool) {
            self.fail_on_change.store(fail, Ordering::SeqCst);
        }
    }

    impl FileWatcherCallback for MockCallback {
        async fn on_file_changed(&self, _paths: Vec<std::path::PathBuf>) -> Result<()> {
            self.changed_count.fetch_add(1, Ordering::SeqCst);
            if self.fail_on_change.load(Ordering::SeqCst) {
                return Err(SwissArmyHammerError::Other {
                    message: "mock failure".to_string(),
                });
            }
            Ok(())
        }

        async fn on_error(&self, _error: String) {
            self.error_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Test that `on_file_changed` increments the changed counter.
    #[tokio::test]
    async fn test_mock_callback_on_file_changed_increments_counter() {
        let cb = MockCallback::new();
        cb.on_file_changed(vec![]).await.unwrap();
        assert_eq!(cb.changed_count(), 1);
        cb.on_file_changed(vec![]).await.unwrap();
        assert_eq!(cb.changed_count(), 2);
    }

    /// Test that `on_error` increments the error counter.
    #[tokio::test]
    async fn test_mock_callback_on_error_increments_counter() {
        let cb = MockCallback::new();
        cb.on_error("some error".to_string()).await;
        assert_eq!(cb.error_count(), 1);
    }

    /// Test that `on_file_changed` returns an error when `fail_on_change` is set.
    #[tokio::test]
    async fn test_mock_callback_on_file_changed_can_fail() {
        let cb = MockCallback::new();
        cb.set_fail_on_change(true);
        let result = cb.on_file_changed(vec![]).await;
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------------------
    // retry_with_backoff tests (exercised via the function directly in the module)
    // ---------------------------------------------------------------------------

    /// Test that `retry_with_backoff` succeeds on the first attempt.
    #[tokio::test]
    async fn test_retry_with_backoff_succeeds_immediately() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();

        let result: std::result::Result<u32, String> = retry_with_backoff(
            3,
            1, // 1ms backoff
            |_e: &String| true,
            move || {
                let n = count_clone.fetch_add(1, Ordering::SeqCst);
                let _ = n;
                async { Ok::<u32, String>(42) }
            },
        )
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    /// Test that `retry_with_backoff` retries on transient errors and eventually succeeds.
    #[tokio::test]
    async fn test_retry_with_backoff_retries_and_succeeds() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();

        let result: std::result::Result<u32, String> = retry_with_backoff(
            3,
            1, // 1ms initial backoff
            |_e: &String| true,
            move || {
                let count = count_clone.clone();
                async move {
                    let n = count.fetch_add(1, Ordering::SeqCst);
                    if n < 2 {
                        Err("transient error".to_string())
                    } else {
                        Ok(99)
                    }
                }
            },
        )
        .await;

        assert_eq!(result.unwrap(), 99);
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    /// Test that `retry_with_backoff` stops retrying when `is_retryable` returns false.
    #[tokio::test]
    async fn test_retry_with_backoff_stops_on_non_retryable_error() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();

        let result: std::result::Result<u32, String> = retry_with_backoff(
            5,
            1,
            |_e: &String| false, // never retryable
            move || {
                let count = count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Err("permanent error".to_string())
                }
            },
        )
        .await;

        assert!(result.is_err());
        // Should only try once since not retryable
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    /// Test that `retry_with_backoff` exhausts retries and returns the last error.
    #[tokio::test]
    async fn test_retry_with_backoff_exhausts_retries() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();

        let result: std::result::Result<u32, String> = retry_with_backoff(
            3,
            1,
            |_e: &String| true, // always retryable
            move || {
                let count = count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Err("always fails".to_string())
                }
            },
        )
        .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "always fails");
        // max_retries = 3, last attempt returns Err directly (not via last_error)
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    // ---------------------------------------------------------------------------
    // McpFileWatcher::is_retryable_fs_error tests
    // ---------------------------------------------------------------------------

    /// Test that IO `TimedOut` errors are retryable.
    #[test]
    fn test_is_retryable_fs_error_timed_out() {
        let err = SwissArmyHammerError::Io(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "timed out",
        ));
        assert!(McpFileWatcher::is_retryable_fs_error(&err));
    }

    /// Test that IO `Interrupted` errors are retryable.
    #[test]
    fn test_is_retryable_fs_error_interrupted() {
        let err = SwissArmyHammerError::Io(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            "interrupted",
        ));
        assert!(McpFileWatcher::is_retryable_fs_error(&err));
    }

    /// Test that IO `WouldBlock` errors are retryable.
    #[test]
    fn test_is_retryable_fs_error_would_block() {
        let err = SwissArmyHammerError::Io(std::io::Error::new(
            std::io::ErrorKind::WouldBlock,
            "would block",
        ));
        assert!(McpFileWatcher::is_retryable_fs_error(&err));
    }

    /// Test that IO `UnexpectedEof` errors are retryable.
    #[test]
    fn test_is_retryable_fs_error_unexpected_eof() {
        let err = SwissArmyHammerError::Io(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "eof",
        ));
        assert!(McpFileWatcher::is_retryable_fs_error(&err));
    }

    /// Test that non-transient IO errors are not retryable.
    #[test]
    fn test_is_retryable_fs_error_not_found_is_not_retryable() {
        let err = SwissArmyHammerError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        ));
        assert!(!McpFileWatcher::is_retryable_fs_error(&err));
    }

    /// Test that errors containing "temporarily unavailable" message are retryable.
    #[test]
    fn test_is_retryable_fs_error_temporarily_unavailable_message() {
        let err = SwissArmyHammerError::Other {
            message: "resource temporarily unavailable".to_string(),
        };
        assert!(McpFileWatcher::is_retryable_fs_error(&err));
    }

    /// Test that errors containing "resource busy" message are retryable.
    #[test]
    fn test_is_retryable_fs_error_resource_busy_message() {
        let err = SwissArmyHammerError::Other {
            message: "resource busy right now".to_string(),
        };
        assert!(McpFileWatcher::is_retryable_fs_error(&err));
    }

    /// Test that errors containing "locked" message are retryable.
    #[test]
    fn test_is_retryable_fs_error_locked_message() {
        let err = SwissArmyHammerError::Other {
            message: "file is locked".to_string(),
        };
        assert!(McpFileWatcher::is_retryable_fs_error(&err));
    }

    /// Test that unrelated Other errors are not retryable.
    #[test]
    fn test_is_retryable_fs_error_other_is_not_retryable() {
        let err = SwissArmyHammerError::Other {
            message: "some other error".to_string(),
        };
        assert!(!McpFileWatcher::is_retryable_fs_error(&err));
    }

    // ---------------------------------------------------------------------------
    // McpFileWatcher lifecycle tests
    // ---------------------------------------------------------------------------

    /// Test that `McpFileWatcher::new()` creates an instance wrapping the given watcher.
    #[tokio::test]
    async fn test_mcp_file_watcher_new() {
        let inner = Arc::new(tokio::sync::Mutex::new(FileWatcher::new()));
        let mcp_watcher = McpFileWatcher::new(inner.clone());
        // stop_file_watching should be a no-op when watcher was never started
        mcp_watcher.stop_file_watching().await;
    }

    /// Test that `stop_file_watching()` is idempotent — safe to call multiple times.
    #[tokio::test]
    async fn test_mcp_file_watcher_stop_is_idempotent() {
        let inner = Arc::new(tokio::sync::Mutex::new(FileWatcher::new()));
        let mcp_watcher = McpFileWatcher::new(inner);
        mcp_watcher.stop_file_watching().await;
        mcp_watcher.stop_file_watching().await; // Second call should not panic
    }

    // ---------------------------------------------------------------------------
    // Extension detection edge cases
    // ---------------------------------------------------------------------------

    /// Test case-insensitive extension matching for prompt files.
    #[test]
    fn test_is_prompt_file_case_insensitive() {
        assert!(is_prompt_file(std::path::Path::new("prompt.MD")));
        assert!(is_prompt_file(std::path::Path::new("prompt.Yaml")));
        assert!(is_prompt_file(std::path::Path::new("prompt.YML")));
        assert!(is_prompt_file(std::path::Path::new("prompt.MARKDOWN")));
    }

    /// Test case-insensitive compound extension matching.
    #[test]
    fn test_has_compound_extension_case_insensitive() {
        assert!(has_compound_extension(std::path::Path::new(
            "prompt.MD.Liquid"
        )));
        assert!(has_compound_extension(std::path::Path::new(
            "PROMPT.YAML.LIQUID"
        )));
    }

    /// Test that `is_prompt_file` handles paths with directories.
    #[test]
    fn test_is_prompt_file_with_directory_path() {
        assert!(is_prompt_file(std::path::Path::new("/some/dir/prompt.md")));
        assert!(is_prompt_file(std::path::Path::new(
            "relative/path/to/file.yaml"
        )));
        assert!(!is_prompt_file(std::path::Path::new("/some/dir/file.txt")));
    }

    /// Test that `has_compound_extension` handles paths with directories.
    #[test]
    fn test_has_compound_extension_with_directory_path() {
        assert!(has_compound_extension(std::path::Path::new(
            "/some/dir/prompt.md.liquid"
        )));
        assert!(!has_compound_extension(std::path::Path::new(
            "/some/dir/prompt.md"
        )));
    }

    /// Test `is_any_prompt_file` with compound extensions takes priority.
    #[test]
    fn test_is_any_prompt_file_compound_priority() {
        // Compound extension files should match (even though .liquid is not a simple prompt ext)
        assert!(is_any_prompt_file(std::path::Path::new("a.yml.liquid")));
        assert!(is_any_prompt_file(std::path::Path::new(
            "a.markdown.liquid"
        )));
    }

    /// Test `is_prompt_file` with empty path.
    #[test]
    fn test_is_prompt_file_empty_path() {
        assert!(!is_prompt_file(std::path::Path::new("")));
    }

    /// Test `has_compound_extension` with empty path.
    #[test]
    fn test_has_compound_extension_empty_path() {
        assert!(!has_compound_extension(std::path::Path::new("")));
    }

    /// Test `is_any_prompt_file` with a dotfile (no real extension).
    #[test]
    fn test_is_any_prompt_file_dotfile() {
        assert!(!is_any_prompt_file(std::path::Path::new(".hidden")));
    }

    // ---------------------------------------------------------------------------
    // FileWatcher start_watching with callback
    // ---------------------------------------------------------------------------

    /// Test that `start_watching` succeeds with a mock callback and sets up the debouncer.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_file_watcher_start_watching_sets_up_debouncer() {
        let cb = MockCallback::new();
        let mut watcher = FileWatcher::new();

        // start_watching may succeed or fail depending on the environment's
        // prompt directories. We test both code paths.
        let result = watcher.start_watching(cb).await;
        if result.is_ok() {
            // If it succeeded, the debouncer should be set
            assert!(watcher.debouncer.is_some());
            assert!(watcher.event_handle.is_some());
        }
        // Either way, stop_watching should be safe
        watcher.stop_watching();
        assert!(watcher.debouncer.is_none());
        assert!(watcher.event_handle.is_none());
    }

    /// Teardown-promptness guard: `stop_watching()` must return to the caller
    /// in well under 1s even when an active `AsyncDebouncer` is watching a real
    /// directory.
    ///
    /// On macOS, dropping the `notify` `RecommendedWatcher` inside the
    /// `AsyncDebouncer` blocks for ~5s on FSEvents stream teardown and
    /// serializes across processes under load — the dominant cost of an
    /// in-process MCP server `shutdown()`. `stop_watching()` detaches that drop
    /// to a background thread, so the caller must return promptly. This test
    /// fails (takes ~5s) if the detach regresses back to a synchronous drop.
    ///
    /// The debouncer is constructed and watched directly (rather than via
    /// `start_watching`, which depends on environment prompt directories) so
    /// the guard reliably exercises a live FSEvents stream teardown.
    #[tokio::test]
    async fn test_stop_watching_returns_promptly() {
        let temp = tempfile::TempDir::new().unwrap();

        let (mut debouncer, _event_rx) =
            AsyncDebouncer::new_with_channel(Duration::from_millis(500), None)
                .await
                .unwrap();
        debouncer
            .watcher()
            .watch(temp.path(), RecursiveMode::Recursive)
            .unwrap();

        let mut watcher = FileWatcher::new();
        watcher.debouncer = Some(debouncer);

        let start = std::time::Instant::now();
        watcher.stop_watching();
        let elapsed = start.elapsed();

        assert!(
            elapsed < Duration::from_secs(1),
            "stop_watching blocked the caller for {elapsed:?}; teardown must be detached"
        );
        assert!(watcher.debouncer.is_none());
    }

    /// Off-lock-build guard: `FileWatcher::start` builds a started watcher
    /// WITHOUT requiring the shared `Arc<Mutex<FileWatcher>>` lock. This is
    /// what frees server `shutdown()` from blocking behind an in-flight
    /// startup: the slow macOS FSEvents `.watch()` registration runs off-lock.
    ///
    /// The test holds the shared lock for the entire `start` build. If `start`
    /// ever tried to take that lock it would deadlock and the test would hang
    /// forever (caught by the nextest slow/leak timeout). Completing at all is
    /// the guarantee. We deliberately do NOT bound the build's wall time: the
    /// FSEvents registration itself takes seconds under load, but that latency
    /// is independent of the lock — the point here is only that it is not held.
    #[tokio::test]
    async fn test_start_builds_without_shared_lock() {
        let shared: Arc<Mutex<FileWatcher>> = Arc::new(Mutex::new(FileWatcher::new()));

        // Hold the shared lock for the whole build to prove `start` never needs it.
        let _guard = shared.lock().await;

        // Exercise the real off-lock builder while still holding the lock. If
        // `start` needed `shared`, this `.await` would never return.
        let built = FileWatcher::start(MockCallback::new()).await;

        assert!(built.is_ok(), "start should build a watcher off-lock");
    }

    /// Test that calling `start_watching` twice replaces the previous watcher.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_file_watcher_start_watching_replaces_previous() {
        let cb = MockCallback::new();
        let mut watcher = FileWatcher::new();

        let _ = watcher.start_watching(cb.clone()).await;
        // Start again — should replace previous
        let _ = watcher.start_watching(cb).await;
        // Should not panic; stop should clean up
        watcher.stop_watching();
    }

    // ---------------------------------------------------------------------------
    // retry_with_backoff edge cases
    // ---------------------------------------------------------------------------

    /// Test that `retry_with_backoff` with max_retries=1 tries exactly once.
    #[tokio::test]
    async fn test_retry_with_backoff_single_attempt() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();

        let result: std::result::Result<u32, String> = retry_with_backoff(
            1,
            1,
            |_e: &String| true,
            move || {
                let count = count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Err("only attempt".to_string())
                }
            },
        )
        .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "only attempt");
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    /// Test that backoff increases exponentially by checking timing is fast.
    #[tokio::test]
    async fn test_retry_with_backoff_completes_in_reasonable_time() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = call_count.clone();

        let start = std::time::Instant::now();
        let _result: std::result::Result<u32, String> = retry_with_backoff(
            3,
            1,
            |_e: &String| true,
            move || {
                let count = count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Err("fail".to_string())
                }
            },
        )
        .await;

        let elapsed = start.elapsed();
        // With 1ms initial backoff: 1ms + 2ms = 3ms total backoff
        // Should complete well within 1 second
        assert!(
            elapsed < std::time::Duration::from_secs(1),
            "Retry took too long: {:?}",
            elapsed
        );
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    // ---------------------------------------------------------------------------
    // McpFileWatcher constructor variations
    // ---------------------------------------------------------------------------

    /// Test that `McpFileWatcher` wraps a shared file watcher.
    #[tokio::test]
    async fn test_mcp_file_watcher_shared_state() {
        let inner = Arc::new(tokio::sync::Mutex::new(FileWatcher::new()));
        let inner_clone = inner.clone();
        let _mcp_watcher = McpFileWatcher::new(inner);

        // The inner watcher should still be accessible
        let guard = inner_clone.lock().await;
        assert!(guard.debouncer.is_none());
    }

    // ---------------------------------------------------------------------------
    // McpFileWatcher::is_retryable_fs_error additional edge cases
    // ---------------------------------------------------------------------------

    /// Test that mixed-case message matching works for retryable errors.
    #[test]
    fn test_is_retryable_fs_error_case_insensitive_message() {
        let err = SwissArmyHammerError::Other {
            message: "RESOURCE TEMPORARILY UNAVAILABLE".to_string(),
        };
        assert!(McpFileWatcher::is_retryable_fs_error(&err));

        let err = SwissArmyHammerError::Other {
            message: "File Is Locked By Another Process".to_string(),
        };
        assert!(McpFileWatcher::is_retryable_fs_error(&err));
    }

    /// Test that PermissionDenied IO errors are NOT retryable.
    #[test]
    fn test_is_retryable_fs_error_permission_denied_not_retryable() {
        let err = SwissArmyHammerError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "permission denied",
        ));
        assert!(!McpFileWatcher::is_retryable_fs_error(&err));
    }

    /// Test that ConnectionRefused IO errors are NOT retryable.
    #[test]
    fn test_is_retryable_fs_error_connection_refused_not_retryable() {
        let err = SwissArmyHammerError::Io(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "connection refused",
        ));
        assert!(!McpFileWatcher::is_retryable_fs_error(&err));
    }

    // ---------------------------------------------------------------------------
    // PROMPT_EXTENSIONS / COMPOUND_PROMPT_EXTENSIONS constant tests
    // ---------------------------------------------------------------------------

    /// Verify that the expected prompt extensions are all present.
    #[test]
    fn test_prompt_extensions_contains_expected() {
        assert!(PROMPT_EXTENSIONS.contains(&"md"));
        assert!(PROMPT_EXTENSIONS.contains(&"yaml"));
        assert!(PROMPT_EXTENSIONS.contains(&"yml"));
        assert!(PROMPT_EXTENSIONS.contains(&"markdown"));
    }

    /// Verify that the expected compound extensions are all present.
    #[test]
    fn test_compound_prompt_extensions_contains_expected() {
        assert!(COMPOUND_PROMPT_EXTENSIONS.contains(&"md.liquid"));
        assert!(COMPOUND_PROMPT_EXTENSIONS.contains(&"markdown.liquid"));
        assert!(COMPOUND_PROMPT_EXTENSIONS.contains(&"yaml.liquid"));
        assert!(COMPOUND_PROMPT_EXTENSIONS.contains(&"yml.liquid"));
    }
}
