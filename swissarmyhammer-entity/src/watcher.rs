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
    fn parse_entity_path_project() {
        let root = Path::new("/project/.kanban");
        let path = Path::new("/project/.kanban/projects/feature.yaml");
        let (t, id) = parse_entity_path(root, path).unwrap();
        assert_eq!(t, "project");
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

    #[test]
    fn parse_entity_path_single_char_dir_no_s() {
        let root = Path::new("/project/.kanban");
        // "x" does not end with 's' (len < 2 after removing 's') -> None
        let path = Path::new("/project/.kanban/s/foo.yaml");
        // "s" has len 1 which is < 2 after removing 's'
        assert!(parse_entity_path(root, path).is_none());
    }

    // -------------------------------------------------------------------------
    // Tests for `handle_file_event` and `EntityWatcher` drop
    // -------------------------------------------------------------------------

    use crate::cache::EntityCache;
    use crate::context::EntityContext;
    use crate::entity::Entity;
    use crate::test_utils::test_fields_context;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Helper: build an EntityCache backed by a temp directory.
    async fn setup_cache() -> (TempDir, Arc<EntityCache>) {
        let fields = test_fields_context();
        let temp = TempDir::new().unwrap();
        let root = temp.path().to_path_buf();
        std::fs::create_dir_all(root.join("tags")).unwrap();
        std::fs::create_dir_all(root.join("tasks")).unwrap();
        let ctx = EntityContext::new(&root, fields);
        let cache = Arc::new(EntityCache::new(ctx));
        (temp, cache)
    }

    #[tokio::test]
    async fn handle_file_event_create_refreshes_cache() {
        let (_dir, cache) = setup_cache().await;

        // Write a tag via the cache so it exists on disk
        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        cache.write(&tag).await.unwrap();

        // Evict from cache to simulate external edit scenario
        cache.evict("tag", "t1").await;
        assert!(cache.get("tag", "t1").await.is_none());

        // Simulate a Create event — should re-read from disk
        let path = _dir.path().join("tags").join("t1.yaml");
        handle_file_event(
            &cache,
            "tag",
            "t1",
            &EventKind::Create(notify::event::CreateKind::File),
            &path,
        )
        .await;

        // Entity should now be back in cache
        let cached = cache.get("tag", "t1").await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().get_str("tag_name"), Some("Bug"));
    }

    #[tokio::test]
    async fn handle_file_event_modify_refreshes_cache() {
        let (_dir, cache) = setup_cache().await;

        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));
        cache.write(&tag).await.unwrap();

        // Modify the file on disk directly (bypass cache)
        let path = _dir.path().join("tags").join("t1.yaml");
        tokio::fs::write(&path, "tag_name: Updated\ncolor: '#00ff00'\n")
            .await
            .unwrap();

        // Simulate a Modify event
        handle_file_event(
            &cache,
            "tag",
            "t1",
            &EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content,
            )),
            &path,
        )
        .await;

        // Cache should reflect the on-disk changes
        let cached = cache.get("tag", "t1").await.unwrap();
        assert_eq!(cached.get_str("tag_name"), Some("Updated"));
    }

    #[tokio::test]
    async fn handle_file_event_remove_evicts_cache() {
        let (_dir, cache) = setup_cache().await;

        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        cache.write(&tag).await.unwrap();
        assert!(cache.get("tag", "t1").await.is_some());

        // Simulate a Remove event
        let path = _dir.path().join("tags").join("t1.yaml");
        handle_file_event(
            &cache,
            "tag",
            "t1",
            &EventKind::Remove(notify::event::RemoveKind::File),
            &path,
        )
        .await;

        // Entity should be evicted from cache
        assert!(cache.get("tag", "t1").await.is_none());
    }

    #[tokio::test]
    async fn handle_file_event_create_nonexistent_evicts() {
        let (_dir, cache) = setup_cache().await;

        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        cache.write(&tag).await.unwrap();

        // Delete the file on disk but simulate a Create event for it
        let path = _dir.path().join("tags").join("t1.yaml");
        tokio::fs::remove_file(&path).await.unwrap();

        handle_file_event(
            &cache,
            "tag",
            "t1",
            &EventKind::Create(notify::event::CreateKind::File),
            &path,
        )
        .await;

        // Should be evicted (file doesn't exist on Create event)
        assert!(cache.get("tag", "t1").await.is_none());
    }

    #[tokio::test]
    async fn handle_file_event_other_kind_is_noop() {
        let (_dir, cache) = setup_cache().await;

        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        cache.write(&tag).await.unwrap();

        // An "Other" event kind should be a no-op
        let path = _dir.path().join("tags").join("t1.yaml");
        handle_file_event(&cache, "tag", "t1", &EventKind::Other, &path).await;

        // Entity should still be in cache
        assert!(cache.get("tag", "t1").await.is_some());
    }

    #[test]
    fn parse_entity_path_ignores_hidden_dirs() {
        let root = Path::new("/project/.kanban");
        // Files inside hidden directories like .trash or .archive should be ignored
        // because they have 3 components: dir/.trash/file
        let path = Path::new("/project/.kanban/tasks/.trash/01ABC.md");
        assert!(parse_entity_path(root, path).is_none());
    }

    #[test]
    fn parse_entity_path_ignores_archive_dir() {
        let root = Path::new("/project/.kanban");
        let path = Path::new("/project/.kanban/tasks/.archive/01ABC.md");
        assert!(parse_entity_path(root, path).is_none());
    }

    #[test]
    fn parse_entity_path_ignores_attachments_dir() {
        let root = Path::new("/project/.kanban");
        let path = Path::new("/project/.kanban/tasks/.attachments/01ABC-photo.png");
        assert!(parse_entity_path(root, path).is_none());
    }

    #[test]
    fn parse_entity_path_dir_name_too_short() {
        let root = Path::new("/project/.kanban");
        // Single-char dir "x" does not end with 's'
        let path = Path::new("/project/.kanban/x/foo.yaml");
        assert!(parse_entity_path(root, path).is_none());
    }

    #[tokio::test]
    async fn handle_file_event_access_kind_is_noop() {
        let (_dir, cache) = setup_cache().await;

        let mut tag = Entity::new("tag", "t1");
        tag.set("tag_name", json!("Bug"));
        cache.write(&tag).await.unwrap();

        // An Access event kind should be a no-op
        let path = _dir.path().join("tags").join("t1.yaml");
        handle_file_event(
            &cache,
            "tag",
            "t1",
            &EventKind::Access(notify::event::AccessKind::Read),
            &path,
        )
        .await;

        // Entity should still be in cache unchanged
        let cached = cache.get("tag", "t1").await.unwrap();
        assert_eq!(cached.get_str("tag_name"), Some("Bug"));
    }

    #[tokio::test]
    async fn entity_watcher_drop_sends_shutdown() {
        let (_dir, cache) = setup_cache().await;

        // Start the watcher and drop it immediately
        let watcher = EntityWatcher::start(_dir.path().to_path_buf(), cache);
        assert!(watcher.is_ok());

        // Drop should send shutdown signal without panic
        drop(watcher.unwrap());
    }
}
