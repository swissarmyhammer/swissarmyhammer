//! File watching functionality for MCP server

use rmcp::RoleServer;
use std::sync::Arc;
use swissarmyhammer::file_watcher::{FileWatcher, FileWatcherCallback};
use swissarmyhammer::{Result, SwissArmyHammerError};
use tokio::sync::Mutex;

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

/// Get the workflow runs directory path
pub fn get_workflow_runs_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        })
        .join(".swissarmyhammer")
        .join("workflow-runs")
}
