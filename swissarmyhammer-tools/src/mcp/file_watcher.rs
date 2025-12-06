//! File watching functionality for MCP server

use async_watcher::{notify::RecursiveMode, AsyncDebouncer, DebouncedEvent};
use rmcp::RoleServer;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use swissarmyhammer_common::{Result, SwissArmyHammerError};
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

    /// Start watching prompt directories for changes
    pub async fn start_watching<C>(&mut self, callback: C) -> Result<()>
    where
        C: FileWatcherCallback + Clone,
    {
        // Stop existing watcher if running
        self.stop_watching();

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
            return Ok(());
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

        // Watch all directories
        for path in &watch_paths {
            debouncer
                .watcher()
                .watch(path, RecursiveMode::Recursive)
                .map_err(|e| SwissArmyHammerError::Other {
                    message: format!("Failed to watch directory {path:?}: {}", e),
                })?;
            tracing::info!("Watching directory: {:?}", path);
        }

        // Spawn task to process events from async-watcher
        let mut event_rx_clone = event_rx;
        let handle = tokio::spawn(async move {
            while let Some(events_result) = event_rx_clone.recv().await {
                match events_result {
                    Ok(events) => {
                        tracing::debug!("📁 Debounced file system events: {:?}", events);

                        // Filter for prompt files and collect all relevant paths
                        let relevant_paths: Vec<std::path::PathBuf> = events
                            .into_iter()
                            .flat_map(|event| event.event.paths)
                            .filter(|p| is_any_prompt_file(p))
                            .collect();

                        if !relevant_paths.is_empty() {
                            tracing::info!("📄 Prompt file changed: {:?}", relevant_paths);

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

        // Store the debouncer and event handler
        self.debouncer = Some(debouncer);
        self.event_handle = Some(handle);

        Ok(())
    }

    /// Stop file watching
    pub fn stop_watching(&mut self) {
        // Drop the debouncer (which stops watching automatically)
        if let Some(_debouncer) = self.debouncer.take() {
            tracing::debug!("📁 Async debouncer stopped");
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
        tracing::info!("📄 Prompt file changed: {:?}", paths);

        // Reload the library
        if let Err(e) = self.server.reload_prompts().await {
            tracing::error!("✗ Failed to reload prompts: {}", e);
            return Err(e);
        }
        tracing::info!("✓ Prompts reloaded successfully");

        // Send notification to client about prompt list change
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
