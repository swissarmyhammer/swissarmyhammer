//! File watcher integration for the tree-sitter index
//!
//! This module provides file watching capabilities to keep the tree-sitter
//! index up-to-date as files change. Uses async-watcher with 500ms debounce.

use crate::error::{Result, TreeSitterError};
use crate::language::LanguageRegistry;
use async_watcher::{
    notify::{Event, EventKind, RecursiveMode},
    AsyncDebouncer,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Debounce duration for file events (500ms)
pub const DEBOUNCE_DURATION_MS: u64 = 500;

/// Callback trait for file change events
///
/// Implement this trait to handle file change notifications from the watcher.
pub trait WorkspaceWatcherCallback: Send + Sync + 'static {
    /// Called when files are added or modified
    fn on_files_changed(
        &self,
        paths: Vec<PathBuf>,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Called when files are deleted
    fn on_files_removed(&self, paths: Vec<PathBuf>)
        -> impl std::future::Future<Output = ()> + Send;

    /// Called on watcher errors
    fn on_error(&self, error: String) -> impl std::future::Future<Output = ()> + Send;
}

/// File watcher for monitoring file changes
///
/// Watches a directory for file changes and invokes callbacks accordingly.
/// Uses 500ms debounce to batch rapid file changes.
///
/// # Example
///
/// ```ignore
/// use swissarmyhammer_treesitter::WorkspaceWatcher;
///
/// struct MyCallback;
///
/// impl WorkspaceWatcherCallback for MyCallback {
///     async fn on_files_changed(&self, paths: Vec<PathBuf>) -> Result<()> {
///         for path in paths {
///             println!("File changed: {}", path.display());
///         }
///         Ok(())
///     }
///     async fn on_files_removed(&self, paths: Vec<PathBuf>) {
///         for path in paths {
///             println!("File removed: {}", path.display());
///         }
///     }
///     async fn on_error(&self, error: String) {
///         eprintln!("Error: {}", error);
///     }
/// }
///
/// let mut watcher = WorkspaceWatcher::new();
/// watcher.start("/path/to/project", MyCallback).await?;
///
/// // Later...
/// watcher.stop();
/// ```
pub struct WorkspaceWatcher {
    /// Handle to stop the watcher
    stop_handle: Option<tokio::task::JoinHandle<()>>,

    /// Root path being watched
    root_path: Option<PathBuf>,

    /// Whether the watcher is currently active
    is_watching: bool,
}

impl WorkspaceWatcher {
    /// Create a new file watcher (not yet watching)
    pub fn new() -> Self {
        Self {
            stop_handle: None,
            root_path: None,
            is_watching: false,
        }
    }

    /// Start watching from root path with callback
    ///
    /// Uses 500ms debounce to batch rapid file changes.
    pub async fn start<C>(&mut self, root_path: impl AsRef<Path>, callback: C) -> Result<()>
    where
        C: WorkspaceWatcherCallback + Clone + 'static,
    {
        let root_path = root_path.as_ref().to_path_buf();

        if !root_path.exists() {
            return Err(TreeSitterError::FileNotFound(root_path));
        }

        // Stop any existing watcher
        self.stop();

        let callback = Arc::new(callback);
        let root_clone = root_path.clone();

        // Create the async debouncer
        let (mut debouncer, mut file_events) =
            AsyncDebouncer::new_with_channel(Duration::from_millis(DEBOUNCE_DURATION_MS), None)
                .await
                .map_err(|e| TreeSitterError::watcher_error(e.to_string()))?;

        // Start watching
        debouncer
            .watcher()
            .watch(&root_path, RecursiveMode::Recursive)
            .map_err(|e| TreeSitterError::watcher_error(e.to_string()))?;

        // Spawn task to handle events
        let handle = tokio::spawn(async move {
            // Keep debouncer alive
            let _debouncer = debouncer;

            while let Some(result) = file_events.recv().await {
                match result {
                    Ok(debounced_events) => {
                        // Extract events from DebouncedEvent
                        let events: Vec<Event> =
                            debounced_events.into_iter().map(|de| de.event).collect();
                        let (changed, removed) = categorize_events(events);

                        if !changed.is_empty() {
                            if let Err(e) = callback.on_files_changed(changed).await {
                                callback.on_error(e.to_string()).await;
                            }
                        }

                        if !removed.is_empty() {
                            callback.on_files_removed(removed).await;
                        }
                    }
                    Err(errors) => {
                        for error in errors {
                            callback.on_error(error.to_string()).await;
                        }
                    }
                }
            }
        });

        self.stop_handle = Some(handle);
        self.root_path = Some(root_clone);
        self.is_watching = true;

        Ok(())
    }

    /// Stop watching
    pub fn stop(&mut self) {
        if let Some(handle) = self.stop_handle.take() {
            handle.abort();
        }
        self.is_watching = false;
    }

    /// Check if currently watching
    pub fn is_watching(&self) -> bool {
        self.is_watching
    }

    /// Get the root path being watched
    pub fn root_path(&self) -> Option<&Path> {
        self.root_path.as_deref()
    }
}

impl Default for WorkspaceWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WorkspaceWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Categorize notify events into changed and removed files
fn categorize_events(events: Vec<Event>) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut changed = Vec::new();
    let mut removed = Vec::new();
    let registry = LanguageRegistry::global();

    for event in events {
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                for path in event.paths {
                    if path.is_file() && registry.detect_language(&path).is_some() {
                        changed.push(path);
                    }
                }
            }
            EventKind::Remove(_) => {
                for path in event.paths {
                    if registry.detect_language(&path).is_some() {
                        removed.push(path);
                    }
                }
            }
            _ => {}
        }
    }

    // Deduplicate
    changed.sort();
    changed.dedup();
    removed.sort();
    removed.dedup();

    (changed, removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{setup_minimal_test_dir, TestWatcherCallback};
    use tempfile::TempDir;

    #[test]
    fn test_watcher_new() {
        let watcher = WorkspaceWatcher::new();
        assert!(!watcher.is_watching());
        assert!(watcher.root_path().is_none());
    }

    #[test]
    fn test_watcher_default() {
        let watcher = WorkspaceWatcher::default();
        assert!(!watcher.is_watching());
    }

    #[tokio::test]
    async fn test_watcher_start_and_stop() {
        let dir = setup_minimal_test_dir();
        let mut watcher = WorkspaceWatcher::new();
        let callback = TestWatcherCallback::new();

        // Start watching
        let result = watcher.start(dir.path(), callback).await;
        assert!(result.is_ok());
        assert!(watcher.is_watching());
        assert_eq!(watcher.root_path(), Some(dir.path()));

        // Stop watching
        watcher.stop();
        assert!(!watcher.is_watching());
    }

    #[tokio::test]
    async fn test_watcher_start_replaces_previous() {
        let dir1 = setup_minimal_test_dir();
        let dir2 = TempDir::new().unwrap();
        std::fs::write(dir2.path().join("lib.rs"), "pub fn foo() {}").unwrap();

        let mut watcher = WorkspaceWatcher::new();
        let callback = TestWatcherCallback::new();

        // Start watching dir1
        watcher.start(dir1.path(), callback.clone()).await.unwrap();
        assert_eq!(watcher.root_path(), Some(dir1.path()));

        // Start watching dir2 (should replace)
        watcher.start(dir2.path(), callback).await.unwrap();
        assert_eq!(watcher.root_path(), Some(dir2.path()));
    }

    #[test]
    fn test_categorize_events_create() {
        use async_watcher::notify::event::CreateKind;

        let dir = setup_minimal_test_dir();
        let path = dir.path().join("main.rs");

        let events = vec![Event {
            kind: EventKind::Create(CreateKind::File),
            paths: vec![path.clone()],
            attrs: Default::default(),
        }];

        let (changed, removed) = categorize_events(events);
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0], path);
        assert!(removed.is_empty());
    }

    #[test]
    fn test_categorize_events_modify() {
        use async_watcher::notify::event::{DataChange, ModifyKind};

        let dir = setup_minimal_test_dir();
        let path = dir.path().join("main.rs");

        let events = vec![Event {
            kind: EventKind::Modify(ModifyKind::Data(DataChange::Content)),
            paths: vec![path.clone()],
            attrs: Default::default(),
        }];

        let (changed, removed) = categorize_events(events);
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0], path);
        assert!(removed.is_empty());
    }

    #[test]
    fn test_categorize_events_remove() {
        use async_watcher::notify::event::RemoveKind;

        let path = PathBuf::from("/tmp/deleted.rs");

        let events = vec![Event {
            kind: EventKind::Remove(RemoveKind::File),
            paths: vec![path.clone()],
            attrs: Default::default(),
        }];

        let (changed, removed) = categorize_events(events);
        assert!(changed.is_empty());
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0], path);
    }

    #[test]
    fn test_categorize_events_deduplicates() {
        use async_watcher::notify::event::{DataChange, ModifyKind};

        let dir = setup_minimal_test_dir();
        let path = dir.path().join("main.rs");

        // Same file modified twice
        let events = vec![
            Event {
                kind: EventKind::Modify(ModifyKind::Data(DataChange::Content)),
                paths: vec![path.clone()],
                attrs: Default::default(),
            },
            Event {
                kind: EventKind::Modify(ModifyKind::Data(DataChange::Content)),
                paths: vec![path.clone()],
                attrs: Default::default(),
            },
        ];

        let (changed, _) = categorize_events(events);
        assert_eq!(changed.len(), 1); // Deduplicated
    }

    #[test]
    fn test_categorize_events_ignores_other_kinds() {
        use async_watcher::notify::event::AccessKind;

        let events = vec![Event {
            kind: EventKind::Access(AccessKind::Read),
            paths: vec![PathBuf::from("/tmp/file.rs")],
            attrs: Default::default(),
        }];

        let (changed, removed) = categorize_events(events);
        assert!(changed.is_empty());
        assert!(removed.is_empty());
    }

    #[tokio::test]
    async fn test_custom_callback_implementation() {
        let callback = TestWatcherCallback::new();

        // Test on_files_changed
        callback
            .on_files_changed(vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")])
            .await
            .unwrap();
        assert_eq!(callback.changed_count(), 2);

        // Test on_files_removed
        callback.on_files_removed(vec![PathBuf::from("c.rs")]).await;
        assert_eq!(callback.removed_count(), 1);

        // Test on_error
        callback.on_error("test".to_string()).await;
        assert_eq!(callback.error_count(), 1);
    }

    #[tokio::test]
    async fn test_watcher_start_nonexistent_path() {
        let mut watcher = WorkspaceWatcher::new();
        let callback = TestWatcherCallback::new();

        let result = watcher
            .start("/nonexistent/path/that/does/not/exist", callback)
            .await;

        assert!(result.is_err());
        assert!(!watcher.is_watching());
    }

    #[tokio::test]
    async fn test_watcher_stop() {
        let mut watcher = WorkspaceWatcher::new();
        watcher.stop(); // Should not panic even if not watching
        assert!(!watcher.is_watching());
    }

    #[tokio::test]
    async fn test_watcher_drop_stops_watching() {
        let dir = setup_minimal_test_dir();
        let callback = TestWatcherCallback::new();

        {
            let mut watcher = WorkspaceWatcher::new();
            watcher.start(dir.path(), callback).await.unwrap();
            assert!(watcher.is_watching());
            // Watcher is dropped here
        }

        // No panic means drop worked correctly
    }
}
