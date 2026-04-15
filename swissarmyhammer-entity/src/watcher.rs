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
                                    } else if let Some((entity_type, filename)) =
                                        parse_attachment_path(&root_clone, &path)
                                    {
                                        handle_attachment_event(
                                            &cache,
                                            &entity_type,
                                            &filename,
                                            &kind,
                                            &path,
                                        );
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

/// Parse a filesystem path to extract (entity_type, filename) for an attachment.
///
/// Expected path pattern: `{root}/{type}s/.attachments/{filename}`. Exactly three
/// components relative to `root`: the entity directory (must end with `s`), the
/// literal `.attachments` directory, and the file itself. The file may have any
/// extension — attachments are arbitrary binary blobs.
///
/// Returns `None` for anything that is not an attachment path (wrong depth,
/// wrong middle directory, empty entity type, etc.).
fn parse_attachment_path(root: &Path, path: &Path) -> Option<(String, String)> {
    // Must be inside root
    let relative = path.strip_prefix(root).ok()?;

    // Must have exactly 3 components: "{type}s/.attachments/{filename}"
    let components: Vec<_> = relative.components().collect();
    if components.len() != 3 {
        return None;
    }

    let dir_name = components[0].as_os_str().to_str()?;
    let att_name = components[1].as_os_str().to_str()?;
    let file_name = components[2].as_os_str().to_str()?;

    // Middle segment must be the `.attachments` directory.
    if att_name != ".attachments" {
        return None;
    }

    // Entity directory name must end with `s` and have at least one
    // character before the `s` so we can derive the singular entity type.
    if !dir_name.ends_with('s') || dir_name.len() < 2 {
        return None;
    }
    let entity_type = &dir_name[..dir_name.len() - 1];

    Some((entity_type.to_string(), file_name.to_string()))
}

/// Handle a single entity-file event by routing to the appropriate cache operation.
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

/// Handle a single attachment-file event by emitting an `AttachmentChanged`
/// event on the cache's broadcast channel.
///
/// Attachments are not entities — they do not populate the cache map. This
/// routine only translates the filesystem event into a notification for
/// subscribers (e.g. a frontend bridge) to refresh thumbnails or badge counts.
///
/// `removed` is derived from both the event kind (any `Remove(_)`) and the
/// live filesystem state: a `Create`/`Modify` event whose path no longer
/// exists is treated as a removal (handles the create-then-delete temp-file
/// pattern identically to `handle_file_event`). `Access` and other event
/// kinds are ignored.
fn handle_attachment_event(
    cache: &EntityCache,
    entity_type: &str,
    filename: &str,
    kind: &EventKind,
    path: &Path,
) {
    let removed = match kind {
        EventKind::Remove(_) => true,
        EventKind::Create(_) | EventKind::Modify(_) => !path.exists(),
        _ => return,
    };
    tracing::debug!(entity_type, filename, removed, "attachment event");
    cache.send_attachment_event(entity_type, filename, removed);
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
        let ctx = Arc::new(EntityContext::new(&root, fields));
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

    // -------------------------------------------------------------------------
    // Tests for `parse_attachment_path` and attachment-file event handling.
    // -------------------------------------------------------------------------

    use crate::events::EntityEvent;

    #[test]
    fn test_parse_attachment_path_ok() {
        let root = Path::new("/root");
        let path = Path::new("/root/tasks/.attachments/01ABC-foo.png");
        let (entity_type, filename) = parse_attachment_path(root, path).unwrap();
        assert_eq!(entity_type, "task");
        assert_eq!(filename, "01ABC-foo.png");
    }

    #[test]
    fn test_parse_attachment_path_actor_avatar() {
        let root = Path::new("/root");
        let path = Path::new("/root/actors/.attachments/avatar.jpg");
        let (entity_type, filename) = parse_attachment_path(root, path).unwrap();
        assert_eq!(entity_type, "actor");
        assert_eq!(filename, "avatar.jpg");
    }

    #[test]
    fn test_parse_attachment_path_accepts_any_extension() {
        // `parse_attachment_path` must not filter by extension — attachments
        // can be any type (pdf, png, txt, no extension at all).
        let root = Path::new("/root");
        let cases = [
            "/root/tasks/.attachments/notes.txt",
            "/root/tasks/.attachments/report.pdf",
            "/root/tasks/.attachments/no-extension",
            "/root/tasks/.attachments/archive.tar.gz",
        ];
        for path_str in cases {
            let (entity_type, filename) = parse_attachment_path(root, Path::new(path_str)).unwrap();
            assert_eq!(entity_type, "task", "path {path_str}");
            assert!(!filename.is_empty(), "path {path_str}");
        }
    }

    #[test]
    fn test_parse_attachment_path_rejects_wrong_depth() {
        let root = Path::new("/root");
        // 2 components: wrong depth.
        assert!(parse_attachment_path(root, Path::new("/root/tasks/foo.png")).is_none());
        // 4 components: wrong depth.
        assert!(
            parse_attachment_path(root, Path::new("/root/tasks/.attachments/sub/foo.png"))
                .is_none()
        );
        // 1 component: wrong depth.
        assert!(parse_attachment_path(root, Path::new("/root/foo.png")).is_none());
    }

    #[test]
    fn test_parse_attachment_path_rejects_non_attachments_middle() {
        let root = Path::new("/root");
        // Middle segment is not `.attachments` — reject.
        assert!(parse_attachment_path(root, Path::new("/root/tasks/.trash/foo.png")).is_none());
        assert!(parse_attachment_path(root, Path::new("/root/tasks/.archive/foo.png")).is_none());
        assert!(parse_attachment_path(root, Path::new("/root/tasks/sub/foo.png")).is_none());
    }

    #[test]
    fn test_parse_attachment_path_rejects_non_plural_entity_dir() {
        let root = Path::new("/root");
        // Entity directory must end in `s` and be at least 2 chars long.
        assert!(parse_attachment_path(root, Path::new("/root/s/.attachments/foo.png")).is_none());
        assert!(parse_attachment_path(root, Path::new("/root/tag/.attachments/foo.png")).is_none());
    }

    #[test]
    fn test_parse_attachment_path_outside_root() {
        let root = Path::new("/root");
        assert!(
            parse_attachment_path(root, Path::new("/other/tasks/.attachments/foo.png")).is_none()
        );
    }

    #[test]
    fn test_parse_entity_path_still_rejects_attachments() {
        // Belt-and-suspenders: the entity parser must not accidentally accept
        // attachment paths now that a dedicated parser exists.
        let root = Path::new("/root");
        assert!(parse_entity_path(root, Path::new("/root/tasks/.attachments/01ABC.png")).is_none());
    }

    #[test]
    fn test_handle_attachment_event_create_emits_change() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let (dir, cache) = setup_cache().await;
            let mut rx = cache.subscribe();

            // Create the attachment file on disk so handle_attachment_event
            // sees it as "exists".
            let att_dir = dir.path().join("tasks").join(".attachments");
            std::fs::create_dir_all(&att_dir).unwrap();
            let path = att_dir.join("01ABC-foo.png");
            std::fs::write(&path, b"png-bytes").unwrap();

            handle_attachment_event(
                &cache,
                "task",
                "01ABC-foo.png",
                &EventKind::Create(notify::event::CreateKind::File),
                &path,
            );

            let event = rx.try_recv().unwrap();
            match event {
                EntityEvent::AttachmentChanged {
                    entity_type,
                    filename,
                    removed,
                } => {
                    assert_eq!(entity_type, "task");
                    assert_eq!(filename, "01ABC-foo.png");
                    assert!(!removed);
                }
                other => panic!("expected AttachmentChanged, got {other:?}"),
            }
        });
    }

    #[test]
    fn test_handle_attachment_event_remove_emits_change_with_removed_true() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let (_dir, cache) = setup_cache().await;
            let mut rx = cache.subscribe();

            // Path does not exist — Remove kind should still emit removed=true.
            let path = _dir.path().join("tasks/.attachments/gone.png");
            handle_attachment_event(
                &cache,
                "task",
                "gone.png",
                &EventKind::Remove(notify::event::RemoveKind::File),
                &path,
            );

            let event = rx.try_recv().unwrap();
            match event {
                EntityEvent::AttachmentChanged {
                    entity_type,
                    filename,
                    removed,
                } => {
                    assert_eq!(entity_type, "task");
                    assert_eq!(filename, "gone.png");
                    assert!(removed);
                }
                other => panic!("expected AttachmentChanged, got {other:?}"),
            }
        });
    }

    #[test]
    fn test_handle_attachment_event_create_missing_file_treated_as_removed() {
        // Create-then-immediately-delete temp-file pattern: the event says
        // Create but the path no longer exists. Should surface as removed=true.
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let (_dir, cache) = setup_cache().await;
            let mut rx = cache.subscribe();

            let path = _dir.path().join("tasks/.attachments/vanished.png");
            assert!(!path.exists());
            handle_attachment_event(
                &cache,
                "task",
                "vanished.png",
                &EventKind::Create(notify::event::CreateKind::File),
                &path,
            );

            let event = rx.try_recv().unwrap();
            match event {
                EntityEvent::AttachmentChanged { removed, .. } => assert!(removed),
                other => panic!("expected AttachmentChanged, got {other:?}"),
            }
        });
    }

    #[test]
    fn test_handle_attachment_event_access_kind_is_noop() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let (_dir, cache) = setup_cache().await;
            let mut rx = cache.subscribe();

            let path = _dir.path().join("tasks/.attachments/foo.png");
            handle_attachment_event(
                &cache,
                "task",
                "foo.png",
                &EventKind::Access(notify::event::AccessKind::Read),
                &path,
            );

            // No event should fire for Access kinds.
            assert!(rx.try_recv().is_err());
        });
    }

    #[tokio::test]
    async fn test_attachment_does_not_populate_cache() {
        let (_dir, cache) = setup_cache().await;

        // Seed one task in the cache so we have a baseline count.
        let mut task = Entity::new("task", "01EXISTING");
        task.set("title", json!("baseline"));
        cache.write(&task).await.unwrap();
        let before = cache.get_all("task").await.len();
        assert_eq!(before, 1);

        // Fire an attachment event — it must not add anything to the cache map.
        let path = _dir.path().join("tasks/.attachments/nope.png");
        handle_attachment_event(
            &cache,
            "task",
            "nope.png",
            &EventKind::Create(notify::event::CreateKind::File),
            &path,
        );

        let after = cache.get_all("task").await.len();
        assert_eq!(after, before, "attachment event must not touch cache map");
    }

    /// Integration test: a full `EntityWatcher` observing a temp `.kanban`
    /// directory must emit `AttachmentChanged { removed: false }` when a new
    /// attachment file appears — exercising `RecursiveMode::Recursive` into
    /// `.attachments/` subdirs and the `parse_attachment_path` dispatch.
    ///
    /// On macOS the notify backend reports event paths under `/private/var/…`
    /// while `tempfile::TempDir` returns paths under `/var/…`. We canonicalize
    /// the root we hand to the watcher so its `strip_prefix` in
    /// `parse_attachment_path` matches.
    #[tokio::test]
    async fn test_attachment_create_emits_event() {
        let (dir, cache) = setup_cache().await;
        let att_dir = dir.path().join("tasks").join(".attachments");
        std::fs::create_dir_all(&att_dir).unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let att_dir = std::fs::canonicalize(&att_dir).unwrap();

        let mut rx = cache.subscribe();
        let _watcher = EntityWatcher::start(root, Arc::clone(&cache)).unwrap();

        // Give the watcher a moment to register before we touch the filesystem.
        tokio::time::sleep(Duration::from_millis(200)).await;

        let path = att_dir.join("new.png");
        tokio::fs::write(&path, b"new").await.unwrap();

        // Drain events looking for AttachmentChanged within the debounce window.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut got = None;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
                Ok(Ok(EntityEvent::AttachmentChanged {
                    entity_type,
                    filename,
                    removed,
                })) if filename == "new.png" => {
                    got = Some((entity_type, filename, removed));
                    break;
                }
                Ok(Ok(_)) => continue, // ignore other events
                Ok(Err(_)) | Err(_) => continue,
            }
        }

        let (entity_type, filename, removed) = got.expect("expected AttachmentChanged within 10s");
        assert_eq!(entity_type, "task");
        assert_eq!(filename, "new.png");
        assert!(!removed);
    }

    /// Integration test: deleting an existing attachment file must emit
    /// `AttachmentChanged { removed: true }`.
    #[tokio::test]
    async fn test_attachment_remove_emits_event() {
        let (dir, cache) = setup_cache().await;
        let att_dir = dir.path().join("tasks").join(".attachments");
        std::fs::create_dir_all(&att_dir).unwrap();
        let path = att_dir.join("doomed.png");
        std::fs::write(&path, b"bye").unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let path = std::fs::canonicalize(&path).unwrap();

        let mut rx = cache.subscribe();
        let _watcher = EntityWatcher::start(root, Arc::clone(&cache)).unwrap();

        tokio::time::sleep(Duration::from_millis(200)).await;

        tokio::fs::remove_file(&path).await.unwrap();

        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        let mut got = None;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
                Ok(Ok(EntityEvent::AttachmentChanged {
                    entity_type,
                    filename,
                    removed,
                })) if filename == "doomed.png" => {
                    got = Some((entity_type, filename, removed));
                    break;
                }
                Ok(Ok(_)) => continue,
                Ok(Err(_)) | Err(_) => continue,
            }
        }

        let (entity_type, filename, removed) = got.expect("expected AttachmentChanged within 10s");
        assert_eq!(entity_type, "task");
        assert_eq!(filename, "doomed.png");
        assert!(removed);
    }
}
