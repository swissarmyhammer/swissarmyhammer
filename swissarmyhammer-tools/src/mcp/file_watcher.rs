//! File watching functionality for MCP server

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rmcp::RoleServer;
use std::sync::Arc;
use swissarmyhammer::common::file_types::is_any_prompt_file;
use swissarmyhammer::PromptResolver;
use swissarmyhammer_common::{SwissArmyHammerError, Result};
use tokio::sync::{mpsc, Mutex};


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
    /// Handle to the background watcher task
    watcher_handle: Option<tokio::task::JoinHandle<()>>,
    /// Shutdown sender to gracefully stop the watcher
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
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
    /// let mut watcher = FileWatcher::new();
    /// // watcher.start_watching(callback).await?;
    /// ```
    pub fn new() -> Self {
        Self {
            watcher_handle: None,
            shutdown_tx: None,
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
        let watch_paths = resolver.get_prompt_directories()
            .map_err(|e| SwissArmyHammerError::Other { message: e.to_string() })?;

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

        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

        // Create the file watcher
        let (tx, mut rx) = mpsc::channel(100);
        let mut watcher = RecommendedWatcher::new(
            move |result: std::result::Result<Event, notify::Error>| {
                if let Ok(event) = result {
                    if let Err(e) = tx.blocking_send(event) {
                        tracing::error!("Failed to send file watch event: {}", e);
                    }
                }
            },
            notify::Config::default(),
        )
        .map_err(|e| SwissArmyHammerError::Other { message: format!("Failed to create file watcher: {}", e) })?;

        // Watch all directories
        for path in &watch_paths {
            watcher
                .watch(path, RecursiveMode::Recursive)
                .map_err(|e| SwissArmyHammerError::Other { message: format!("Failed to watch directory {path:?}: {}", e) })?;
            tracing::info!("Watching directory: {:?}", path);
        }

        // Spawn the event handler task
        let handle = tokio::spawn(async move {
            // Keep the watcher alive for the duration of this task
            let _watcher = watcher;

            loop {
                tokio::select! {
                    // Handle file system events
                    event = rx.recv() => {
                        match event {
                            Some(event) => {
                                tracing::debug!("📁 File system event: {:?}", event);

                                // Check if this is a relevant event
                                match event.kind {
                                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                                        // Check if it's a prompt file
                                        let relevant_paths: Vec<std::path::PathBuf> = event
                                            .paths
                                            .iter()
                                            .filter(|p| is_any_prompt_file(p))
                                            .cloned()
                                            .collect();

                                        if !relevant_paths.is_empty() {
                                            tracing::info!("📄 Prompt file changed: {:?}", relevant_paths);

                                            // Notify callback about the change
                                            if let Err(e) = callback.on_file_changed(relevant_paths).await {
                                                tracing::error!("❌ File watcher callback failed: {}", e);
                                                callback.on_error(format!("Callback failed: {e}")).await;
                                            }
                                        } else {
                                            tracing::debug!("🚫 Ignoring non-prompt file: {:?}", event.paths);
                                        }
                                    }
                                    _ => {
                                        tracing::debug!("🚫 Ignoring event type: {:?}", event.kind);
                                    }
                                }
                            }
                            None => {
                                // Channel closed, exit loop
                                tracing::debug!("📁 File watch channel closed, stopping watcher");
                                break;
                            }
                        }
                    }
                    // Handle shutdown signal
                    _ = &mut shutdown_rx => {
                        tracing::debug!("📁 Received shutdown signal, stopping file watcher");
                        break;
                    }
                }
            }
            tracing::debug!("📁 File watcher task exiting");
        });

        // Store the handle and shutdown sender
        self.watcher_handle = Some(handle);
        self.shutdown_tx = Some(shutdown_tx);

        Ok(())
    }

    /// Stop file watching
    pub fn stop_watching(&mut self) {
        // Send shutdown signal if available
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()); // Ignore error if receiver is dropped
        }

        // Wait for the task to complete
        if let Some(handle) = self.watcher_handle.take() {
            handle.abort();
            tracing::debug!("📁 File watcher task aborted");
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
            tracing::error!("❌ Failed to reload prompts: {}", e);
            return Err(e);
        }
        tracing::info!("✅ Prompts reloaded successfully");

        // Send notification to client about prompt list change
        let peer_clone = self.peer.clone();
        tokio::spawn(async move {
            match peer_clone.notify_prompt_list_changed().await {
                Ok(_) => {
                    tracing::info!("📢 Sent prompts/listChanged notification to client");
                }
                Err(e) => {
                    tracing::error!("❌ Failed to send notification: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn on_error(&self, error: String) {
        tracing::error!("❌ File watcher error: {}", error);
    }
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

        let mut last_error = None;
        let mut backoff_ms = INITIAL_BACKOFF_MS;

        for attempt in 1..=MAX_RETRIES {
            // Start watching using the file watcher module
            let result = {
                let mut watcher = self.file_watcher.lock().await;
                watcher.start_watching(callback.clone()).await
            };

            match result {
                Ok(()) => {
                    if attempt > 1 {
                        tracing::info!(
                            "✅ File watcher started successfully on attempt {}",
                            attempt
                        );
                    }
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);

                    if attempt < MAX_RETRIES
                        && Self::is_retryable_fs_error(last_error.as_ref().unwrap())
                    {
                        tracing::warn!(
                            "⚠️ File watcher initialization attempt {} failed, retrying in {}ms: {}",
                            attempt,
                            backoff_ms,
                            last_error.as_ref().unwrap()
                        );

                        tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                        backoff_ms *= 2; // Exponential backoff
                    } else {
                        break;
                    }
                }
            }
        }

        Err(last_error.unwrap())
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

