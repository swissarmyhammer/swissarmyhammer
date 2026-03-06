//! File watcher for concurrent access with hash-based change detection.
//!
//! Watches a `.kanban/` directory recursively for external file changes and
//! routes them through the `EntityCache` event system. The cache handles
//! hash comparison and event emission, so duplicate/unchanged writes are
//! automatically suppressed.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing;

use crate::cache::EntityCache;

/// Handle to a running file watcher. Dropping it stops the watcher.
pub struct EntityWatcher {
    _watcher: RecommendedWatcher,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl EntityWatcher {
    /// Start watching the given root directory for entity file changes.
    ///
    /// The root should be the `.kanban/` directory. Entity files are expected
    /// at `{root}/{type}s/{id}.(yaml|md)`.
    ///
    /// The watcher debounces rapid changes and routes events through the
    /// `EntityCache`, which handles hash comparison and event emission.
    pub fn start(root: PathBuf, cache: Arc<EntityCache>) -> Result<Self, notify::Error> {
        let (tx, mut rx) = mpsc::channel::<notify::Result<Event>>(256);
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.blocking_send(res);
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )?;

        watcher.watch(&root, RecursiveMode::Recursive)?;

        let root_clone = root.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        tracing::debug!("entity watcher shutting down");
                        break;
                    }
                    event = rx.recv() => {
                        match event {
                            Some(Ok(ev)) => {
                                // Small debounce window to batch rapid changes
                                tokio::time::sleep(Duration::from_millis(50)).await;

                                // Collect all paths from this event
                                let mut all_events: Vec<(PathBuf, EventKind)> = ev
                                    .paths
                                    .into_iter()
                                    .map(|p| (p, ev.kind))
                                    .collect();

                                // Drain any additional events that arrived during sleep
                                while let Ok(more) = rx.try_recv() {
                                    if let Ok(more_ev) = more {
                                        all_events.extend(
                                            more_ev.paths.into_iter().map(|p| (p, more_ev.kind)),
                                        );
                                    }
                                }

                                // Deduplicate by path, keeping the last event kind per path
                                let mut seen = HashSet::new();
                                let mut deduped: Vec<(PathBuf, EventKind)> = Vec::new();
                                for (path, kind) in all_events.into_iter().rev() {
                                    if seen.insert(path.clone()) {
                                        deduped.push((path, kind));
                                    }
                                }

                                for (path, kind) in deduped {
                                    if let Some((entity_type, id)) =
                                        parse_entity_path(&root_clone, &path)
                                    {
                                        handle_file_event(&cache, &entity_type, &id, &kind, &path)
                                            .await;
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                tracing::warn!(error = %e, "file watcher error");
                            }
                            None => break, // channel closed
                        }
                    }
                }
            }
        });

        Ok(Self {
            _watcher: watcher,
            shutdown: Some(shutdown_tx),
        })
    }
}

impl Drop for EntityWatcher {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

/// Parse a filesystem path to extract (entity_type, id).
///
/// Expected path pattern: `{root}/{type}s/{id}.(yaml|md)`
/// Returns `None` for non-entity files (changelog `.jsonl`, `activity/` dir, etc.)
fn parse_entity_path(root: &Path, path: &Path) -> Option<(String, String)> {
    // Must be inside root
    let relative = path.strip_prefix(root).ok()?;

    // Must have exactly 2 components: "{type}s/{filename}"
    let components: Vec<_> = relative.components().collect();
    if components.len() != 2 {
        return None;
    }

    let dir_name = components[0].as_os_str().to_str()?;
    let file_name = components[1].as_os_str().to_str()?;

    // Extract entity type by removing trailing 's' from directory name
    if !dir_name.ends_with('s') || dir_name.len() < 2 {
        return None;
    }
    let entity_type = &dir_name[..dir_name.len() - 1];

    // Must be .yaml or .md file
    let extension = Path::new(file_name).extension()?.to_str()?;
    if extension != "yaml" && extension != "md" {
        return None;
    }

    // Extract id from filename (strip extension)
    let id = Path::new(file_name).file_stem()?.to_str()?;

    Some((entity_type.to_string(), id.to_string()))
}

/// Handle a single file event by routing to the appropriate cache operation.
async fn handle_file_event(
    cache: &EntityCache,
    entity_type: &str,
    id: &str,
    kind: &EventKind,
    path: &Path,
) {
    match kind {
        EventKind::Create(_) | EventKind::Modify(_) => {
            if path.exists() {
                match cache.refresh_from_disk(entity_type, id).await {
                    Ok(changed) => {
                        if changed {
                            tracing::debug!(entity_type, id, "entity updated from disk");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(entity_type, id, error = %e, "failed to refresh entity from disk");
                    }
                }
            } else {
                // File was created then immediately deleted (temp file pattern).
                // Remove from cache if it was there.
                cache.evict(entity_type, id).await;
            }
        }
        EventKind::Remove(_) => {
            cache.evict(entity_type, id).await;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_entity_path_task_md() {
        let root = Path::new("/project/.kanban");
        let path = Path::new("/project/.kanban/tasks/01ABC.md");
        let (t, id) = parse_entity_path(root, path).unwrap();
        assert_eq!(t, "task");
        assert_eq!(id, "01ABC");
    }

    #[test]
    fn parse_entity_path_tag_yaml() {
        let root = Path::new("/project/.kanban");
        let path = Path::new("/project/.kanban/tags/bug.yaml");
        let (t, id) = parse_entity_path(root, path).unwrap();
        assert_eq!(t, "tag");
        assert_eq!(id, "bug");
    }

    #[test]
    fn parse_entity_path_column() {
        let root = Path::new("/project/.kanban");
        let path = Path::new("/project/.kanban/columns/todo.yaml");
        let (t, id) = parse_entity_path(root, path).unwrap();
        assert_eq!(t, "column");
        assert_eq!(id, "todo");
    }

    #[test]
    fn parse_entity_path_ignores_changelog() {
        let root = Path::new("/project/.kanban");
        let path = Path::new("/project/.kanban/activity/changelog.jsonl");
        assert!(parse_entity_path(root, path).is_none());
    }

    #[test]
    fn parse_entity_path_ignores_nested() {
        let root = Path::new("/project/.kanban");
        let path = Path::new("/project/.kanban/tasks/sub/deep.yaml");
        assert!(parse_entity_path(root, path).is_none());
    }

    #[test]
    fn parse_entity_path_ignores_root_files() {
        let root = Path::new("/project/.kanban");
        let path = Path::new("/project/.kanban/config.yaml");
        assert!(parse_entity_path(root, path).is_none());
    }

    #[test]
    fn parse_entity_path_ignores_non_entity_extensions() {
        let root = Path::new("/project/.kanban");
        let path = Path::new("/project/.kanban/tasks/01ABC.json");
        assert!(parse_entity_path(root, path).is_none());
    }

    #[test]
    fn parse_entity_path_outside_root() {
        let root = Path::new("/project/.kanban");
        let path = Path::new("/other/tasks/01ABC.yaml");
        assert!(parse_entity_path(root, path).is_none());
    }

    #[test]
    fn parse_entity_path_swimlane() {
        let root = Path::new("/project/.kanban");
        let path = Path::new("/project/.kanban/swimlanes/feature.yaml");
        let (t, id) = parse_entity_path(root, path).unwrap();
        assert_eq!(t, "swimlane");
        assert_eq!(id, "feature");
    }

    #[test]
    fn parse_entity_path_actor() {
        let root = Path::new("/project/.kanban");
        let path = Path::new("/project/.kanban/actors/alice.yaml");
        let (t, id) = parse_entity_path(root, path).unwrap();
        assert_eq!(t, "actor");
        assert_eq!(id, "alice");
    }

    #[test]
    fn parse_entity_path_board() {
        let root = Path::new("/project/.kanban");
        let path = Path::new("/project/.kanban/boards/main.yaml");
        let (t, id) = parse_entity_path(root, path).unwrap();
        assert_eq!(t, "board");
        assert_eq!(id, "main");
    }

    #[test]
    fn parse_entity_path_ignores_jsonl() {
        let root = Path::new("/project/.kanban");
        let path = Path::new("/project/.kanban/tasks/01ABC.jsonl");
        assert!(parse_entity_path(root, path).is_none());
    }
}
