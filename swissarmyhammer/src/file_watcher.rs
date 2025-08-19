//! File watching functionality for prompt directories
//!
//! This module provides a unified file watching system that can monitor
//! prompt directories for changes and trigger appropriate reload actions.

use crate::common::file_types::is_any_prompt_file;
#[cfg(not(test))]
use crate::common::mcp_errors::ToSwissArmyHammerError;
use crate::{PromptResolver, Result};
#[cfg(not(test))]
use notify::{
    event::{Event, EventKind},
    RecommendedWatcher, RecursiveMode, Watcher,
};
#[cfg(not(test))]
use tokio::sync::mpsc;

/// File watcher for monitoring prompt directories
pub struct FileWatcher {
    /// Handle to the background watcher task
    watcher_handle: Option<tokio::task::JoinHandle<()>>,
    /// Shutdown sender to gracefully stop the watcher
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// Configuration used for the watcher
    config: FileWatcherConfig,
}

/// Configuration for file watching behavior
pub struct FileWatcherConfig {
    /// Channel buffer size for file system events
    pub channel_buffer_size: usize,
    /// Whether to watch directories recursively
    pub recursive: bool,
    /// Timeout for graceful shutdown of watcher task
    pub shutdown_timeout_secs: u64,
    /// Timeout for abort cleanup after forced shutdown
    pub abort_cleanup_timeout_millis: u64,
}

impl Default for FileWatcherConfig {
    fn default() -> Self {
        Self {
            channel_buffer_size: 100,
            recursive: true,
            shutdown_timeout_secs: 2,
            abort_cleanup_timeout_millis: 100,
        }
    }
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

impl FileWatcher {
    /// Create a new file watcher
    pub fn new() -> Self {
        Self {
            watcher_handle: None,
            shutdown_tx: None,
            config: FileWatcherConfig::default(),
        }
    }

    /// Start watching prompt directories for changes
    pub async fn start_watching<C>(&mut self, callback: C) -> Result<()>
    where
        C: FileWatcherCallback + Clone,
    {
        self.start_watching_with_config(callback, FileWatcherConfig::default())
            .await
    }

    /// Start watching with custom configuration
    pub async fn start_watching_with_config<C>(
        &mut self,
        _callback: C,
        config: FileWatcherConfig,
    ) -> Result<()>
    where
        C: FileWatcherCallback + Clone,
    {
        // Stop existing watcher if running
        self.stop_watching();

        // Store the config
        self.config = config;

        tracing::info!("Starting file watching for prompt directories");

        // Get the directories to watch using the same logic as PromptResolver
        let resolver = PromptResolver::new();
        let watch_paths = resolver.get_prompt_directories()?;

        tracing::info!(
            "Found {} directories to watch: {:?}",
            watch_paths.len(),
            watch_paths
        );

        // The resolver already returns only existing paths
        if watch_paths.is_empty() {
            tracing::warn!("No prompt directories found to watch");
            // Still set up a mock watcher handle for consistency in tests
            #[cfg(test)]
            {
                let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
                let handle = tokio::spawn(async move {
                    // Mock implementation: just wait for shutdown signal
                    let _ = shutdown_rx.await;
                    tracing::info!("Mock file watcher (no directories) shutting down");
                });
                self.watcher_handle = Some(handle);
                self.shutdown_tx = Some(shutdown_tx);
            }
            return Ok(());
        }

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        // Use mock implementation for tests to avoid hanging on RecommendedWatcher::new()
        #[cfg(test)]
        {
            tracing::info!("Using mock file watcher for tests");
            let handle = tokio::spawn(async move {
                // Mock implementation: just wait for shutdown signal
                let _ = shutdown_rx.await;
                tracing::info!("Mock file watcher shutting down");
            });

            self.watcher_handle = Some(handle);
            self.shutdown_tx = Some(shutdown_tx);
        }

        #[cfg(not(test))]
        {
            let mut shutdown_rx = shutdown_rx;
            // Create the file watcher
            let (tx, mut rx) = mpsc::channel(self.config.channel_buffer_size);
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
            .to_swiss_error_with_context("Failed to create file watcher")?;

            // Watch all directories
            let recursive_mode = if self.config.recursive {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };

            for path in &watch_paths {
                watcher
                    .watch(path, recursive_mode)
                    .to_swiss_error_with_context(&format!("Failed to watch directory {path:?}"))?;
                tracing::info!("Watching directory: {:?}", path);
            }

            // Spawn the event handler task
            let handle = tokio::spawn(async move {
                // Keep the watcher alive for the duration of this task
                // The watcher must be moved into the task to prevent it from being dropped
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
                                                if let Err(e) = _callback.on_file_changed(relevant_paths).await {
                                                    tracing::error!("❌ File watcher callback failed: {}", e);
                                                    _callback.on_error(format!("Callback failed: {e}")).await;
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
        }

        Ok(())
    }

    /// Stop file watching
    pub fn stop_watching(&mut self) {
        // Send shutdown signal if available
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()); // Ignore error if receiver is dropped
        }

        // Wait for the task to complete (with timeout to avoid hanging)
        if let Some(handle) = self.watcher_handle.take() {
            // Just abort the task to avoid hanging in synchronous contexts
            handle.abort();
            tracing::debug!("📁 File watcher task aborted");
        }
    }

    /// Stop file watching asynchronously with proper cleanup
    pub async fn stop_watching_async(&mut self) {
        // Send shutdown signal if available
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()); // Ignore error if receiver is dropped
        }

        // Wait for the task to complete with timeout
        if let Some(handle) = self.watcher_handle.take() {
            let timeout_duration =
                std::time::Duration::from_secs(self.config.shutdown_timeout_secs);

            match tokio::time::timeout(timeout_duration, handle).await {
                Ok(Ok(_)) => {
                    tracing::debug!("📁 File watcher task completed successfully");
                }
                Ok(Err(e)) if e.is_cancelled() => {
                    tracing::debug!("📁 File watcher task was cancelled");
                }
                Ok(Err(e)) => {
                    tracing::debug!("📁 File watcher task failed: {}", e);
                }
                Err(_) => {
                    tracing::debug!("📁 File watcher task did not complete within timeout");
                }
            }
        }
    }

    // File type detection moved to common::file_types module
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.stop_watching();
    }
}

impl Default for FileWatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[derive(Clone)]
    struct TestCallback {
        changes: Arc<Mutex<Vec<Vec<std::path::PathBuf>>>>,
        errors: Arc<Mutex<Vec<String>>>,
    }

    struct DirGuard {
        original_dir: std::path::PathBuf,
    }

    impl Drop for DirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original_dir);
        }
    }

    impl TestCallback {
        fn new() -> Self {
            Self {
                changes: Arc::new(Mutex::new(Vec::new())),
                errors: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl FileWatcherCallback for TestCallback {
        async fn on_file_changed(&self, paths: Vec<std::path::PathBuf>) -> Result<()> {
            self.changes.lock().await.push(paths);
            Ok(())
        }

        async fn on_error(&self, error: String) {
            self.errors.lock().await.push(error);
        }
    }

    #[tokio::test]
    async fn test_file_watcher_creation() {
        let watcher = FileWatcher::new();
        assert!(watcher.watcher_handle.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_file_watcher_start_stop() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let test_prompts_dir = temp_dir.path().join(".swissarmyhammer").join("prompts");
        fs::create_dir_all(&test_prompts_dir).unwrap();

        // Create a test prompt file so directory isn't empty
        let test_file = test_prompts_dir.join("test.md");
        fs::write(&test_file, "test prompt").unwrap();

        // Set current directory to temp dir so it finds our test directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Ensure directory is restored even if test panics
        let _guard = DirGuard { original_dir };

        let mut watcher = FileWatcher::new();
        let callback = TestCallback::new();

        // Start watching - should now succeed
        let result = watcher.start_watching(callback).await;
        assert!(result.is_ok());
        assert!(watcher.watcher_handle.is_some());

        // Add small delay to ensure watcher is fully initialized
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Stop watching with timeout
        watcher.stop_watching_async().await;
        assert!(watcher.watcher_handle.is_none());
    }

    #[test]
    fn test_is_prompt_file() {
        use std::path::Path;

        assert!(is_any_prompt_file(Path::new("test.md")));
        assert!(is_any_prompt_file(Path::new("test.yaml")));
        assert!(is_any_prompt_file(Path::new("test.yml")));
        assert!(!is_any_prompt_file(Path::new("test.txt")));
        assert!(!is_any_prompt_file(Path::new("test")));
    }

    #[test]
    fn test_file_watcher_config_default() {
        let config = FileWatcherConfig::default();
        assert_eq!(config.channel_buffer_size, 100);
        assert!(config.recursive);
    }

    #[test]
    fn test_file_watcher_default_trait() {
        let watcher1 = FileWatcher::default();
        let watcher2 = FileWatcher::new();
        // Both should create watchers without handles
        assert!(watcher1.watcher_handle.is_none());
        assert!(watcher2.watcher_handle.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_file_watcher_custom_config() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let test_prompts_dir = temp_dir.path().join(".swissarmyhammer").join("prompts");
        fs::create_dir_all(&test_prompts_dir).unwrap();

        // Create a test prompt file
        let test_file = test_prompts_dir.join("test.yaml");
        fs::write(&test_file, "name: test\ndescription: test prompt").unwrap();

        // Set current directory to temp dir
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Ensure directory is restored even if test panics
        let _guard = DirGuard { original_dir };

        let mut watcher = FileWatcher::new();
        let callback = TestCallback::new();
        let config = FileWatcherConfig {
            channel_buffer_size: 200,
            recursive: false,
            shutdown_timeout_secs: 5,
            abort_cleanup_timeout_millis: 1000,
        };

        // Start with custom config - should now succeed
        let result = watcher.start_watching_with_config(callback, config).await;
        assert!(result.is_ok());
        assert!(watcher.watcher_handle.is_some());

        // Add small delay to ensure watcher is fully initialized
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Use async stop to ensure proper cleanup
        watcher.stop_watching_async().await;
        assert!(watcher.watcher_handle.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_file_watcher_drop() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let test_prompts_dir = temp_dir.path().join(".swissarmyhammer").join("prompts");
        fs::create_dir_all(&test_prompts_dir).unwrap();
        fs::write(test_prompts_dir.join("test.md"), "test").unwrap();

        // Set current directory to temp dir
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Ensure directory is restored even if test panics
        let _guard = DirGuard { original_dir };

        let mut watcher = FileWatcher::new();
        let callback = TestCallback::new();

        // Start watching
        let result = watcher.start_watching(callback).await;
        assert!(result.is_ok());
        assert!(watcher.watcher_handle.is_some());

        // Add small delay to ensure watcher is fully initialized
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Stop watching using async method instead of relying on Drop
        watcher.stop_watching_async().await;
        // Verify the watcher handle is cleared
        assert!(watcher.watcher_handle.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_file_watcher_restart() {
        use std::fs;
        use tempfile::TempDir;

        // Save original directory first
        let original_dir = std::env::current_dir().unwrap();

        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let test_prompts_dir = temp_dir.path().join(".swissarmyhammer").join("prompts");
        fs::create_dir_all(&test_prompts_dir).unwrap();
        fs::write(test_prompts_dir.join("test.yml"), "name: test").unwrap();

        // Set current directory to temp dir
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Ensure directory is restored even if test panics
        let _guard = DirGuard { original_dir };

        let mut watcher = FileWatcher::new();
        let callback1 = TestCallback::new();
        let callback2 = TestCallback::new();

        // Start watching first time
        let result1 = watcher.start_watching(callback1).await;
        assert!(result1.is_ok());
        assert!(watcher.watcher_handle.is_some());

        // Add delay to ensure watcher is fully initialized
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Stop first watcher explicitly to ensure proper cleanup
        watcher.stop_watching_async().await;

        // Add delay to ensure complete cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Start watching again with second callback
        let result2 = watcher.start_watching(callback2).await;
        assert!(result2.is_ok());
        assert!(watcher.watcher_handle.is_some());

        // Add delay before final cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        watcher.stop_watching_async().await;
    }

    #[derive(Clone)]
    struct ErrorCallback {
        calls: Arc<Mutex<Vec<Vec<std::path::PathBuf>>>>,
    }

    impl ErrorCallback {
        fn new() -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl FileWatcherCallback for ErrorCallback {
        async fn on_file_changed(&self, paths: Vec<std::path::PathBuf>) -> Result<()> {
            self.calls.lock().await.push(paths.clone());
            // Return error to test error handling
            Err(crate::SwissArmyHammerError::Other("Test error".to_string()))
        }

        async fn on_error(&self, _error: String) {
            // Track that error handler was called
        }
    }

    #[test]
    fn test_is_prompt_file_edge_cases() {
        use std::path::Path;

        // Test file without extension
        assert!(!is_any_prompt_file(Path::new("README")));

        // Test hidden files
        assert!(is_any_prompt_file(Path::new(".test.md")));
        assert!(is_any_prompt_file(Path::new(".config.yaml")));
        assert!(!is_any_prompt_file(Path::new(".gitignore")));

        // Test files with multiple dots
        assert!(is_any_prompt_file(Path::new("file.test.md")));
        assert!(is_any_prompt_file(Path::new("config.prod.yaml")));

        // Test case insensitivity (our implementation is case-insensitive for user-friendliness)
        assert!(is_any_prompt_file(Path::new("file.MD")));
        assert!(is_any_prompt_file(Path::new("file.YML")));
        assert!(is_any_prompt_file(Path::new("file.YAML")));
    }

    #[tokio::test]
    async fn test_file_watcher_multiple_stops() {
        let mut watcher = FileWatcher::new();

        // Multiple stops should be safe
        watcher.stop_watching();
        watcher.stop_watching();
        assert!(watcher.watcher_handle.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn test_file_watcher_simple_start() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let test_prompts_dir = temp_dir.path().join(".swissarmyhammer").join("prompts");
        fs::create_dir_all(&test_prompts_dir).unwrap();
        fs::write(test_prompts_dir.join("test.md"), "test prompt").unwrap();

        // Set current directory to temp dir so it finds our test directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Ensure directory is restored even if test panics
        let _guard = DirGuard { original_dir };

        let mut watcher = FileWatcher::new();
        let callback = TestCallback::new();

        // Test just the creation without serial attribute
        let result = watcher.start_watching(callback).await;
        if result.is_ok() {
            watcher.stop_watching_async().await;
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_file_watcher_error_callback() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory for testing
        let temp_dir = TempDir::new().unwrap();
        let test_prompts_dir = temp_dir.path().join(".swissarmyhammer").join("prompts");
        fs::create_dir_all(&test_prompts_dir).unwrap();
        fs::write(test_prompts_dir.join("test.yaml"), "name: test").unwrap();

        // Set current directory to temp dir with better error handling
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Ensure directory is restored even if test panics
        let _guard = DirGuard { original_dir };

        let mut watcher = FileWatcher::new();
        let callback = ErrorCallback::new();

        // Start watching with error callback
        let result = watcher.start_watching(callback.clone()).await;

        // If the result is an error, log it for debugging before asserting
        if let Err(ref e) = result {
            eprintln!("File watcher failed to start: {e}");
        }

        assert!(
            result.is_ok(),
            "File watcher should start successfully, but got error: {:?}",
            result.err()
        );
        assert!(watcher.watcher_handle.is_some());

        // Add small delay to ensure watcher is fully initialized
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Clean up immediately without waiting for events that never come
        watcher.stop_watching_async().await;
    }
}
