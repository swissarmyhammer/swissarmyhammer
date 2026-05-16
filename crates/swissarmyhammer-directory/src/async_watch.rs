//! Shared `async-watcher` plumbing for filesystem watchers in this crate.
//!
//! This internal module isolates the mechanical concerns of driving
//! [`async_watcher`] — creating the debouncer, registering directory roots for
//! recursive watching, pumping debounced batches onto a channel, and tearing
//! everything down cleanly on drop. Higher-level watchers (such as
//! [`crate::watcher::Watcher`]) build their domain logic on top of this rather
//! than reimplementing the boilerplate.
//!
//! The helper is deliberately layout-agnostic: it knows nothing about layers,
//! plugins, or `FileSource`. It simply emits the raw debounced filesystem
//! events for whatever directories it was told to watch.

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_watcher::notify::RecursiveMode;
use async_watcher::{AsyncDebouncer, DebouncedEvent};
use tokio::sync::mpsc;

use crate::error::{DirectoryError, Result};

/// Default debounce window for directory watchers.
///
/// Multiple filesystem events for paths under the same watched tree that occur
/// within this window are collapsed by [`async_watcher`] into a single batch.
/// The value is a real but bounded delay: long enough to coalesce a multi-file
/// save, short enough to keep hot-reload latency low.
pub(crate) const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(300);

/// A debounced, recursive watcher over a set of directory roots.
///
/// `DebouncedDirWatcher` owns an [`AsyncDebouncer`] and a background task that
/// forwards each debounced batch of [`DebouncedEvent`]s onto an `mpsc` channel.
/// Dropping the watcher stops the debouncer and aborts the pump task, so no
/// further events are delivered after the value goes out of scope.
///
/// This type is internal to the crate; it is the shared substrate for the
/// public [`crate::watcher::Watcher`] and any future watchers.
pub(crate) struct DebouncedDirWatcher {
    /// The async debouncer; kept alive so the OS watch handles persist.
    ///
    /// Wrapped in `Option` purely so [`Drop`] can move it out and stop it
    /// without blocking.
    debouncer: Option<AsyncDebouncer<async_watcher::notify::RecommendedWatcher>>,
    /// Handle to the background task pumping debounced batches onto the channel.
    pump: Option<tokio::task::JoinHandle<()>>,
}

impl DebouncedDirWatcher {
    /// Start a debounced recursive watch over `roots`.
    ///
    /// Each root is watched recursively. The returned channel receiver yields
    /// one `Vec<DebouncedEvent>` per debounced batch; filesystem errors raised
    /// by the underlying watcher are logged and dropped rather than surfaced on
    /// the channel, since callers of directory watchers generally cannot act on
    /// transient watch errors.
    ///
    /// # Parameters
    ///
    /// * `roots` - Directory paths to watch recursively. Each must already
    ///   exist on disk; non-existent roots cause an error.
    /// * `debounce` - The debounce window passed to [`async_watcher`].
    ///
    /// # Returns
    ///
    /// On success, the live `DebouncedDirWatcher` and a receiver of debounced
    /// event batches.
    ///
    /// # Errors
    ///
    /// Returns [`DirectoryError::Other`] if the debouncer cannot be created or
    /// if any root cannot be registered for watching (for example, because the
    /// path does not exist).
    pub(crate) async fn start(
        roots: &[PathBuf],
        debounce: Duration,
    ) -> Result<(Self, mpsc::Receiver<Vec<DebouncedEvent>>)> {
        let (mut debouncer, mut raw_rx) = AsyncDebouncer::new_with_channel(debounce, None)
            .await
            .map_err(|e| DirectoryError::Other {
                message: format!("failed to create filesystem debouncer: {e}"),
            })?;

        for root in roots {
            debouncer
                .watcher()
                .watch(root, RecursiveMode::Recursive)
                .map_err(|e| DirectoryError::Other {
                    message: format!("failed to watch directory '{}': {e}", root.display()),
                })?;
            tracing::debug!("watching directory: {}", root.display());
        }

        // The channel is bounded but small: debounced batches are infrequent
        // relative to the rate a consumer drains them.
        let (batch_tx, batch_rx) = mpsc::channel(16);

        let pump = tokio::spawn(async move {
            while let Some(batch) = raw_rx.recv().await {
                match batch {
                    Ok(events) if !events.is_empty() => {
                        // If the consumer has dropped its receiver, stop pumping.
                        if batch_tx.send(events).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(errors) => {
                        for error in errors {
                            tracing::warn!("filesystem watch error: {error}");
                        }
                    }
                }
            }
            tracing::debug!("debounced directory watch pump exiting");
        });

        Ok((
            Self {
                debouncer: Some(debouncer),
                pump: Some(pump),
            },
            batch_rx,
        ))
    }
}

impl Drop for DebouncedDirWatcher {
    /// Stop watching and abort the pump task.
    ///
    /// Dropping the [`AsyncDebouncer`] signals its background loop to stop
    /// without blocking; aborting the pump task ensures no further batches are
    /// forwarded once the watcher is gone.
    fn drop(&mut self) {
        if let Some(_debouncer) = self.debouncer.take() {
            // Dropping the debouncer flags its internal loop to stop.
        }
        if let Some(pump) = self.pump.take() {
            pump.abort();
        }
    }
}

/// Classify the top-level entry name that a changed `path` belongs to.
///
/// Given a filesystem `path` and the `root` of a watched layer, this returns
/// the first path component *under* `root` — i.e. the top-level entry within
/// the watched subdirectory (such as a plugin directory name). A change to the
/// root itself, or to a path outside the root, yields `None`.
///
/// # Parameters
///
/// * `root` - The watched layer's subdirectory root (e.g. `<project>/.sah/plugins`
///   or, for a custom root, `<project>/plugins`).
/// * `path` - An absolute path reported by the filesystem watcher.
pub(crate) fn top_level_entry(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let first = relative.components().next()?;
    match first {
        std::path::Component::Normal(name) => name.to_str().map(|s| s.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `top_level_entry` returns the first component beneath the watched root.
    #[test]
    fn top_level_entry_extracts_first_component() {
        let root = Path::new("/tmp/project/plugins");
        let path = Path::new("/tmp/project/plugins/weather/src/main.rs");
        assert_eq!(top_level_entry(root, path), Some("weather".to_string()));
    }

    /// `top_level_entry` returns the entry name even for a direct child file.
    #[test]
    fn top_level_entry_handles_direct_child() {
        let root = Path::new("/tmp/project/plugins");
        let path = Path::new("/tmp/project/plugins/manifest.json");
        assert_eq!(
            top_level_entry(root, path),
            Some("manifest.json".to_string())
        );
    }

    /// A path that is not under the root yields `None`.
    #[test]
    fn top_level_entry_rejects_path_outside_root() {
        let root = Path::new("/tmp/project/plugins");
        let path = Path::new("/tmp/other/file.rs");
        assert_eq!(top_level_entry(root, path), None);
    }

    /// A change to the root directory itself yields `None`.
    #[test]
    fn top_level_entry_rejects_root_itself() {
        let root = Path::new("/tmp/project/plugins");
        assert_eq!(top_level_entry(root, root), None);
    }
}
