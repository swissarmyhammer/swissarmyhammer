//! File-system watcher for the `.kanban` directory.
//!
//! Watches entity directories for changes using the `notify` crate.
//! Uses SHA-256 content hashing to detect real changes — our own writes
//! update the hash cache, so only external modifications trigger events.
//!
//! Emits entity-level and field-level change events:
//! - `entity-created` — new entity file appeared
//! - `entity-removed` — entity file deleted
//! - `entity-field-changed` — specific field(s) on an entity changed

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// File extensions that represent entity state (not logs).
const ENTITY_EXTENSIONS: &[&str] = &["yaml", "yml", "md"];

/// An event emitted when an entity or field changes.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
#[allow(clippy::enum_variant_names)]
pub enum WatchEvent {
    /// A new entity file appeared.
    #[serde(rename = "entity-created")]
    EntityCreated {
        entity_type: String,
        id: String,
        fields: HashMap<String, serde_json::Value>,
    },
    /// An entity file was deleted.
    #[serde(rename = "entity-removed")]
    EntityRemoved { entity_type: String, id: String },
    /// One or more fields on an entity changed.
    #[serde(rename = "entity-field-changed")]
    EntityFieldChanged {
        entity_type: String,
        id: String,
        changes: Vec<FieldChange>,
        /// Full entity state (including computed fields) when available.
        /// Populated by `dispatch_command` after reading through the entity
        /// context; `None` for raw watcher events.
        #[serde(skip_serializing_if = "Option::is_none")]
        fields: Option<HashMap<String, serde_json::Value>>,
    },
    /// An attachment file was created, modified, or deleted.
    ///
    /// Emitted when files inside `.attachments/` subdirectories change,
    /// allowing the frontend to update thumbnail previews and badge counts.
    #[serde(rename = "attachment-changed")]
    AttachmentChanged {
        /// The entity type that owns the attachment (e.g. "task").
        entity_type: String,
        /// The stored filename (e.g. "01ABC-screenshot.png").
        filename: String,
        /// Whether the file was removed (true) or created/modified (false).
        removed: bool,
    },
}

/// Wrapper that pairs a `WatchEvent` with the board it belongs to.
///
/// When emitted to the frontend, the JSON includes all fields from the inner
/// event (via `#[serde(flatten)]`) plus a `board_path` string so listeners
/// can filter events for their active board.
#[derive(Debug, Clone, Serialize)]
pub struct BoardWatchEvent {
    #[serde(flatten)]
    pub event: WatchEvent,
    pub board_path: String,
}

/// A single field-level change.
#[derive(Debug, Clone, Serialize)]
pub struct FieldChange {
    pub field: String,
    pub value: serde_json::Value,
}

/// Apply a single `WatchEvent` to an `EntitySearchIndex`.
///
/// Reconstructs an `Entity` from the event fields and calls `update` or
/// `remove` on the index. Used from both the file-watcher callback and the
/// `dispatch_command` post-write path to avoid duplicated sync logic.
pub fn sync_search_index(
    idx: &mut swissarmyhammer_entity_search::EntitySearchIndex,
    evt: &WatchEvent,
) {
    match evt {
        WatchEvent::EntityCreated {
            entity_type,
            id,
            fields,
        } => {
            let mut entity = swissarmyhammer_entity::Entity::new(entity_type.as_str(), id.as_str());
            for (k, v) in fields {
                entity.set(k, v.clone());
            }
            idx.update(entity);
        }
        WatchEvent::EntityFieldChanged {
            entity_type,
            id,
            fields,
            ..
        } => {
            if let Some(fields) = fields {
                // Merge into existing entity to preserve fields not in this event
                idx.merge_fields(entity_type, id, fields);
            }
        }
        WatchEvent::EntityRemoved { id, .. } => {
            idx.remove(id);
        }
        WatchEvent::AttachmentChanged { .. } => {
            // Attachment file changes don't affect the search index.
            // They are forwarded to the frontend for UI updates only.
        }
    }
}

/// Handle to a running file watcher. Dropping this stops the watcher.
pub struct BoardWatcher {
    _watcher: RecommendedWatcher,
    _cancel: tokio::sync::oneshot::Sender<()>,
}

/// Cached state for a single entity file.
#[derive(Debug, Clone)]
pub(crate) struct CachedEntity {
    hash: Vec<u8>,
    fields: HashMap<String, serde_json::Value>,
}

/// Content + field cache shared between the write-side (our own writes)
/// and the read-side (filesystem event handler).
pub type EntityCache = Arc<Mutex<HashMap<PathBuf, CachedEntity>>>;

/// Create a new entity cache, pre-populated by scanning entity files on disk.
///
/// `store_roots` is the list of directories to scan, obtained from
/// `StoreContext::watched_roots()`. The kanban root directory is also scanned
/// for root-level entity files (e.g. `board.yaml`).
pub fn new_entity_cache(kanban_root: &Path, store_roots: &[PathBuf]) -> EntityCache {
    let mut map = HashMap::new();
    for dir in store_roots {
        if dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if is_entity_file(&path) {
                        let canonical = path.canonicalize().unwrap_or(path);
                        if let Some(cached) = cache_file(&canonical) {
                            map.insert(canonical, cached);
                        }
                    }
                }
            }
        }
    }
    // Also scan root-level entity files (e.g. board.yaml)
    if let Ok(entries) = std::fs::read_dir(kanban_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_entity_file(&path) {
                let canonical = path.canonicalize().unwrap_or(path);
                if let Some(cached) = cache_file(&canonical) {
                    map.insert(canonical, cached);
                }
            }
        }
    }
    Arc::new(Mutex::new(map))
}

/// Update the cache after our own write to a file.
///
/// Call this from command execution paths so the watcher knows
/// the content we just wrote and doesn't treat it as an external change.
#[cfg(test)]
pub fn update_cache(cache: &EntityCache, path: &Path) {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if let Some(cached) = cache_file(&canonical) {
        if let Ok(mut map) = cache.lock() {
            map.insert(canonical, cached);
        }
    }
}

/// Scan all entity files in the registered store directories, compare against
/// the cache, emit events for anything that changed, and update the cache.
///
/// `store_roots` is the list of directories to scan, obtained from
/// `StoreContext::watched_roots()`. The kanban root directory is also scanned
/// for root-level entity files (e.g. `board.yaml`).
///
/// This is used after our own command execution to produce immediate granular
/// events without waiting for the file watcher's debounce. It also updates
/// the cache so the watcher won't double-fire for these writes.
///
/// Returns the list of events emitted.
pub fn flush_and_emit(
    kanban_root: &Path,
    store_roots: &[PathBuf],
    cache: &EntityCache,
) -> Vec<WatchEvent> {
    let kanban_root = kanban_root
        .canonicalize()
        .unwrap_or_else(|_| kanban_root.to_path_buf());
    let mut events = Vec::new();

    // Collect all current entity file paths on disk
    let mut disk_paths: HashSet<PathBuf> = HashSet::new();
    for dir in store_roots {
        if dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if is_entity_file(&path) {
                        let canonical = path.canonicalize().unwrap_or(path);
                        disk_paths.insert(canonical);
                    }
                }
            }
        }
    }
    // Root-level entity files
    if let Ok(entries) = std::fs::read_dir(&kanban_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_entity_file(&path) {
                let canonical = path.canonicalize().unwrap_or(path);
                disk_paths.insert(canonical);
            }
        }
    }

    // Check each disk file against the cache
    for path in &disk_paths {
        if let Some(evt) = resolve_change(path, cache, &kanban_root) {
            events.push(evt);
        }
    }

    // Check for removals: cached paths no longer on disk
    let cached_paths: Vec<PathBuf> = {
        let map = cache.lock().unwrap();
        map.keys().cloned().collect()
    };
    for path in cached_paths {
        if !disk_paths.contains(&path) {
            if let Some(evt) = resolve_removal(&path, cache, &kanban_root) {
                events.push(evt);
            }
        }
    }

    events
}

/// Categorized filesystem event: which paths had which kind of event.
#[derive(Debug)]
enum FsAction {
    Changed(PathBuf),
    Removed(PathBuf),
}

/// Start watching the registered store directories for external changes.
///
/// `store_roots` is the list of directories to watch, obtained from
/// `StoreContext::watched_roots()`. The kanban root is also watched for
/// root-level entity files (e.g. `board.yaml`).
///
/// Returns a `BoardWatcher` handle — dropping it stops the watcher.
/// Hash comparison happens at debounce time (not event-receive time),
/// so `update_cache` calls made after writing but before the debounce
/// fires will suppress the event.
pub fn start_watching<F>(
    kanban_root: PathBuf,
    store_roots: Vec<PathBuf>,
    entity_cache: EntityCache,
    on_event: F,
) -> Result<BoardWatcher, String>
where
    F: Fn(WatchEvent) + Send + Sync + 'static,
{
    // Canonicalize root so paths match between cache and notify events
    let kanban_root = kanban_root.canonicalize().unwrap_or(kanban_root);

    let on_event = Arc::new(on_event);
    let (tx, mut rx) = mpsc::channel::<Event>(256);

    let watcher_tx = tx.clone();
    let mut watcher = RecommendedWatcher::new(
        move |result: Result<Event, notify::Error>| {
            if let Ok(event) = result {
                let _ = watcher_tx.blocking_send(event);
            }
        },
        Config::default(),
    )
    .map_err(|e| format!("Failed to create file watcher: {e}"))?;

    // Watch each registered store directory
    for dir in &store_roots {
        if dir.is_dir() {
            watcher
                .watch(dir, RecursiveMode::NonRecursive)
                .map_err(|e| format!("Failed to watch {}: {e}", dir.display()))?;

            // Also watch .attachments/ inside entity directories for attachment file changes
            let att_dir = dir.join(".attachments");
            if att_dir.is_dir() {
                watcher
                    .watch(&att_dir, RecursiveMode::NonRecursive)
                    .map_err(|e| format!("Failed to watch {}: {e}", att_dir.display()))?;
            }
        }
    }

    // Watch root for board.yaml etc.
    watcher
        .watch(&kanban_root, RecursiveMode::NonRecursive)
        .map_err(|e| format!("Failed to watch {}: {e}", kanban_root.display()))?;

    let (cancel_tx, mut cancel_rx) = tokio::sync::oneshot::channel::<()>();

    let cache = entity_cache;
    let root = kanban_root.clone();
    tokio::spawn(async move {
        use tokio::time::{sleep, Duration, Instant};

        let debounce = Duration::from_millis(200);
        // Collect raw filesystem paths that had events, not resolved changes.
        // We defer hash comparison to debounce time so update_cache() calls
        // that happen between the fs event and the debounce will suppress
        // our own writes.
        let mut pending_changed: HashSet<PathBuf> = HashSet::new();
        let mut pending_removed: HashSet<PathBuf> = HashSet::new();
        let mut last_emit = Instant::now() - debounce;

        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Some(event) => {
                            for action in classify_event(&event, &root) {
                                match action {
                                    FsAction::Changed(p) => {
                                        pending_removed.remove(&p);
                                        pending_changed.insert(p);
                                    }
                                    FsAction::Removed(p) => {
                                        pending_changed.remove(&p);
                                        pending_removed.insert(p);
                                    }
                                }
                            }
                        }
                        None => break,
                    }
                }
                _ = sleep(debounce), if !pending_changed.is_empty() || !pending_removed.is_empty() => {
                    let now = Instant::now();
                    if now.duration_since(last_emit) >= debounce {
                        // Now resolve changes against the cache (at debounce time)
                        let mut events = Vec::new();

                        for path in pending_changed.drain() {
                            if path.exists() {
                                if let Some(evt) = resolve_change(&path, &cache, &root) {
                                    events.push(evt);
                                }
                            } else {
                                // File was in pending_changed but no longer exists —
                                // treat as removal (some platforms emit Modify on dir
                                // instead of Remove on file).
                                if let Some(evt) = resolve_removal(&path, &cache, &root) {
                                    events.push(evt);
                                }
                            }
                        }
                        for path in pending_removed.drain() {
                            if let Some(evt) = resolve_removal(&path, &cache, &root) {
                                events.push(evt);
                            }
                        }

                        // Deduplicate by (entity_type, id/filename)
                        let mut seen: HashMap<(String, String), WatchEvent> = HashMap::new();
                        for evt in events {
                            let key = match &evt {
                                WatchEvent::EntityCreated { entity_type, id, .. } |
                                WatchEvent::EntityRemoved { entity_type, id } |
                                WatchEvent::EntityFieldChanged { entity_type, id, .. } => {
                                    (entity_type.clone(), id.clone())
                                }
                                WatchEvent::AttachmentChanged { entity_type, filename, .. } => {
                                    (format!("attachment:{}", entity_type), filename.clone())
                                }
                            };
                            seen.insert(key, evt);
                        }
                        for evt in seen.into_values() {
                            tracing::info!(event = ?evt, "file watcher: emitting event");
                            (on_event)(evt);
                        }
                        last_emit = now;
                    }
                }
                _ = &mut cancel_rx => {
                    tracing::debug!("file watcher: cancelled");
                    break;
                }
            }
        }
    });

    Ok(BoardWatcher {
        _watcher: watcher,
        _cancel: cancel_tx,
    })
}

/// Classify a raw filesystem event into actions for entity files.
///
/// Canonicalizes paths to handle macOS symlink differences
/// (e.g. `/var/folders` vs `/private/var/folders`).
fn classify_event(event: &Event, kanban_root: &Path) -> Vec<FsAction> {
    let mut actions = Vec::new();
    for path in &event.paths {
        let is_entity = is_entity_file(path);
        let is_att = is_attachment(path);

        if !is_entity && !is_att {
            tracing::trace!(path = %path.display(), kind = ?event.kind, "classify_event: not entity/attachment, skipping");
            continue;
        }

        tracing::info!(
            path = %path.display(),
            kind = ?event.kind,
            is_entity,
            is_att,
            "classify_event: filesystem event received"
        );

        // For removals, the file may not exist, so canonicalize the parent + filename
        let canonical = if path.exists() {
            path.canonicalize().unwrap_or_else(|_| path.clone())
        } else {
            // Canonicalize parent, re-append filename
            match (path.parent(), path.file_name()) {
                (Some(parent), Some(name)) => parent
                    .canonicalize()
                    .unwrap_or_else(|_| parent.to_path_buf())
                    .join(name),
                _ => path.clone(),
            }
        };

        if is_entity && path_to_entity(&canonical, kanban_root).is_none() {
            // Not a recognized entity file
            if !is_att {
                continue;
            }
        }

        if is_att && path_to_attachment(&canonical, kanban_root).is_none() {
            continue;
        }

        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Access(_) => {
                // For Changed actions, resolve_change will check
                // if the file still exists at debounce time. If not,
                // it gets promoted to a removal.
                actions.push(FsAction::Changed(canonical));
            }
            EventKind::Remove(_) => {
                actions.push(FsAction::Removed(canonical));
            }
            _ => {
                // Catch-all: treat any other event as a potential change.
                // The debounce phase will determine if it's real.
                actions.push(FsAction::Changed(canonical));
            }
        }
    }
    actions
}

/// Resolve a changed file path into a WatchEvent at debounce time.
///
/// Reads the file now, computes hash, compares against the cache.
/// Returns None if the content matches the cache (our own write).
fn resolve_change(path: &Path, cache: &EntityCache, kanban_root: &Path) -> Option<WatchEvent> {
    // Check if this is an attachment file first
    if is_attachment(path) {
        if let Some((entity_type, filename)) = path_to_attachment(path, kanban_root) {
            return Some(WatchEvent::AttachmentChanged {
                entity_type,
                filename,
                removed: false,
            });
        }
    }

    let (entity_type, id) = path_to_entity(path, kanban_root)?;
    let new_cached = cache_file(path)?;

    let mut map = cache.lock().unwrap();
    match map.get(path) {
        Some(old) if old.hash == new_cached.hash => {
            // Same content — our own write or no real change
            tracing::debug!(
                entity_type = %entity_type,
                id = %id,
                "resolve_change: hash unchanged, suppressing event"
            );
            None
        }
        Some(old) => {
            // Content changed — diff fields
            let changes = diff_fields(&old.fields, &new_cached.fields);
            map.insert(path.to_path_buf(), new_cached);
            if changes.is_empty() {
                None
            } else {
                tracing::debug!(
                    entity_type = %entity_type,
                    id = %id,
                    changed_fields = changes.len(),
                    "file watcher: fields changed"
                );
                Some(WatchEvent::EntityFieldChanged {
                    entity_type,
                    id,
                    changes,
                    fields: None,
                })
            }
        }
        None => {
            // New file — entity created
            tracing::debug!(
                entity_type = %entity_type,
                id = %id,
                "file watcher: new entity"
            );
            let fields = new_cached.fields.clone();
            map.insert(path.to_path_buf(), new_cached);
            Some(WatchEvent::EntityCreated {
                entity_type,
                id,
                fields,
            })
        }
    }
}

/// Resolve a removed file path into a WatchEvent at debounce time.
fn resolve_removal(path: &Path, cache: &EntityCache, kanban_root: &Path) -> Option<WatchEvent> {
    // Check if this is an attachment file first
    if is_attachment(path) {
        if let Some((entity_type, filename)) = path_to_attachment(path, kanban_root) {
            return Some(WatchEvent::AttachmentChanged {
                entity_type,
                filename,
                removed: true,
            });
        }
    }

    let (entity_type, id) = path_to_entity(path, kanban_root)?;
    let was_tracked = {
        let mut map = cache.lock().unwrap();
        map.remove(path).is_some()
    };
    if was_tracked {
        tracing::debug!(
            entity_type = %entity_type,
            id = %id,
            "file watcher: entity removed"
        );
        Some(WatchEvent::EntityRemoved { entity_type, id })
    } else {
        None
    }
}

/// Diff two field maps and return a list of changed fields.
fn diff_fields(
    old: &HashMap<String, serde_json::Value>,
    new: &HashMap<String, serde_json::Value>,
) -> Vec<FieldChange> {
    let mut changes = Vec::new();

    for (key, new_val) in new {
        match old.get(key) {
            Some(old_val) if old_val == new_val => {}
            _ => {
                changes.push(FieldChange {
                    field: key.clone(),
                    value: new_val.clone(),
                });
            }
        }
    }

    for key in old.keys() {
        if !new.contains_key(key) {
            changes.push(FieldChange {
                field: key.clone(),
                value: serde_json::Value::Null,
            });
        }
    }

    changes
}

/// Read a file and produce its cached representation (hash + parsed fields).
fn cache_file(path: &Path) -> Option<CachedEntity> {
    let content = std::fs::read_to_string(path).ok()?;
    let hash = {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hasher.finalize().to_vec()
    };
    let fields = parse_entity_file(path, &content)?;
    Some(CachedEntity { hash, fields })
}

/// Parse an entity file into a flat field map.
///
/// Uses a two-step YAML → JSON conversion to avoid serde_yaml_ng/serde_json
/// deserialization mismatches.
fn parse_entity_file(path: &Path, content: &str) -> Option<HashMap<String, serde_json::Value>> {
    let ext = path.extension()?.to_str()?;
    match ext {
        "md" => parse_frontmatter_body(content),
        "yaml" | "yml" => parse_plain_yaml(content),
        _ => None,
    }
}

/// Convert a serde_yaml_ng::Value to serde_json::Value via re-serialization.
fn yaml_to_json(yaml: serde_yaml_ng::Value) -> Option<serde_json::Value> {
    // Serialize yaml to JSON string, then parse back as serde_json::Value.
    // This correctly handles all type conversions.
    let json_str = serde_json::to_string(&yaml).ok()?;
    serde_json::from_str(&json_str).ok()
}

/// Parse YAML frontmatter + markdown body.
fn parse_frontmatter_body(content: &str) -> Option<HashMap<String, serde_json::Value>> {
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return None;
    }
    let frontmatter = parts[1].trim();
    let body = parts[2].strip_prefix('\n').unwrap_or(parts[2]);

    let yaml_val: serde_yaml_ng::Value = serde_yaml_ng::from_str(frontmatter).ok()?;
    let json_val = yaml_to_json(yaml_val)?;

    let mut fields = json_to_field_map(json_val)?;

    fields.insert(
        "body".to_string(),
        serde_json::Value::String(body.to_string()),
    );
    Some(fields)
}

/// Parse plain YAML into a flat field map.
fn parse_plain_yaml(content: &str) -> Option<HashMap<String, serde_json::Value>> {
    let yaml_val: serde_yaml_ng::Value = serde_yaml_ng::from_str(content).ok()?;
    let json_val = yaml_to_json(yaml_val)?;
    json_to_field_map(json_val)
}

/// Convert a JSON value (expected object) to a flat field map.
/// Flattens one level of nesting (e.g. `position: {column: x}` → `position_column: x`).
fn json_to_field_map(value: serde_json::Value) -> Option<HashMap<String, serde_json::Value>> {
    let map = value.as_object()?;
    let mut fields = HashMap::new();

    for (key, val) in map {
        if let serde_json::Value::Object(nested) = val {
            for (sub_key, sub_val) in nested {
                fields.insert(format!("{}_{}", key, sub_key), sub_val.clone());
            }
        } else {
            fields.insert(key.clone(), val.clone());
        }
    }

    Some(fields)
}

/// Map a file path to (entity_type, id).
///
/// The entity type is derived from the parent directory name by stripping
/// a trailing 's' (e.g. `tasks` → `task`, `perspectives` → `perspective`).
/// Files directly in the kanban root (e.g. `board.yaml`) use the stem as both
/// entity type and id.
fn path_to_entity(path: &Path, kanban_root: &Path) -> Option<(String, String)> {
    let stem = path.file_stem()?.to_str()?;

    if path.parent()? == kanban_root {
        return Some((stem.to_string(), stem.to_string()));
    }

    let parent = path.parent()?;

    // Only handle direct children of the kanban root (one directory deep)
    if parent.parent()? != kanban_root {
        return None;
    }

    let parent_name = parent.file_name()?.to_str()?;

    // Skip hidden directories (e.g. .attachments handled separately)
    if parent_name.starts_with('.') {
        return None;
    }

    let entity_type = parent_name.strip_suffix('s').unwrap_or(parent_name);

    Some((entity_type.to_string(), stem.to_string()))
}

/// Check if a path is an entity file we should track.
fn is_entity_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ENTITY_EXTENSIONS.contains(&ext))
}

/// Check if a path is an attachment file inside a `.attachments/` directory.
///
/// Attachment files live at `<kanban_root>/<entity_type>s/.attachments/<filename>`.
/// Any extension is accepted. Temp files (starting with `.tmp_`) and the
/// `.trash/` subdirectory are excluded.
fn is_attachment(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    let Some(parent_name) = parent.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if parent_name != ".attachments" {
        return false;
    }
    // Exclude temp files and directories
    let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    !filename.starts_with(".tmp_") && !filename.starts_with('.')
}

/// Extract entity type and filename from an attachment path.
///
/// Given `<kanban_root>/tasks/.attachments/01ABC-screenshot.png`, returns
/// `Some(("task", "01ABC-screenshot.png"))`.
fn path_to_attachment(path: &Path, kanban_root: &Path) -> Option<(String, String)> {
    // Path structure: kanban_root / <entity_type>s / .attachments / <filename>
    let att_dir = path.parent()?; // .attachments/
    let entity_dir = att_dir.parent()?; // tasks/
    let entity_dir_name = entity_dir.file_name()?.to_str()?;

    // Verify this is under kanban_root
    if entity_dir.parent()? != kanban_root {
        return None;
    }

    let entity_type = entity_dir_name.strip_suffix('s').unwrap_or(entity_dir_name);
    let filename = path.file_name()?.to_str()?;

    Some((entity_type.to_string(), filename.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    /// Standard subdirectories for test boards (mirrors production entity types).
    const TEST_SUBDIRS: &[&str] = &[
        "tasks",
        "tags",
        "columns",
        "swimlanes",
        "actors",
        "boards",
        "views",
        "perspectives",
    ];

    fn setup_kanban_dir() -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let kanban = tmp.path().join(".kanban");
        for subdir in TEST_SUBDIRS {
            std::fs::create_dir_all(kanban.join(subdir)).unwrap();
        }
        // Canonicalize to match what the cache and watcher use internally
        let kanban = kanban.canonicalize().unwrap();
        (tmp, kanban)
    }

    /// Build the store roots list from a kanban directory (mirrors what
    /// StoreContext::watched_roots() returns in production).
    fn store_roots(kanban: &Path) -> Vec<PathBuf> {
        TEST_SUBDIRS
            .iter()
            .map(|s| kanban.join(s))
            .filter(|p| p.is_dir())
            .collect()
    }

    // =========================================================================
    // Unit tests — parsing, diffing, path mapping
    // =========================================================================

    #[test]
    fn test_is_entity_file() {
        assert!(is_entity_file(Path::new("/foo/tasks/abc.yaml")));
        assert!(is_entity_file(Path::new("/foo/tasks/abc.yml")));
        assert!(is_entity_file(Path::new("/foo/tasks/abc.md")));
        assert!(!is_entity_file(Path::new("/foo/tasks/abc.jsonl")));
        assert!(!is_entity_file(Path::new("/foo/.lock")));
    }

    #[test]
    fn test_path_to_entity() {
        let root = PathBuf::from("/project/.kanban");

        assert_eq!(
            path_to_entity(Path::new("/project/.kanban/tasks/ABC123.md"), &root),
            Some(("task".to_string(), "ABC123".to_string()))
        );
        assert_eq!(
            path_to_entity(Path::new("/project/.kanban/tags/bug.yaml"), &root),
            Some(("tag".to_string(), "bug".to_string()))
        );
        assert_eq!(
            path_to_entity(Path::new("/project/.kanban/columns/todo.yaml"), &root),
            Some(("column".to_string(), "todo".to_string()))
        );
        assert_eq!(
            path_to_entity(Path::new("/project/.kanban/board.yaml"), &root),
            Some(("board".to_string(), "board".to_string()))
        );
    }

    #[test]
    fn test_is_attachment() {
        assert!(is_attachment(Path::new(
            "/project/.kanban/tasks/.attachments/01ABC-screenshot.png"
        )));
        assert!(is_attachment(Path::new(
            "/project/.kanban/tasks/.attachments/01XYZ-report.pdf"
        )));
        // Temp files should be excluded
        assert!(!is_attachment(Path::new(
            "/project/.kanban/tasks/.attachments/.tmp_01ABC"
        )));
        // Hidden files should be excluded
        assert!(!is_attachment(Path::new(
            "/project/.kanban/tasks/.attachments/.trash"
        )));
        // Regular entity files are not attachments
        assert!(!is_attachment(Path::new("/project/.kanban/tasks/abc.yaml")));
        // Files not in .attachments/ are not attachments
        assert!(!is_attachment(Path::new(
            "/project/.kanban/tasks/other/file.png"
        )));
    }

    #[test]
    fn test_path_to_attachment() {
        let root = PathBuf::from("/project/.kanban");

        assert_eq!(
            path_to_attachment(
                Path::new("/project/.kanban/tasks/.attachments/01ABC-screenshot.png"),
                &root
            ),
            Some(("task".to_string(), "01ABC-screenshot.png".to_string()))
        );
        assert_eq!(
            path_to_attachment(
                Path::new("/project/.kanban/actors/.attachments/avatar.jpg"),
                &root
            ),
            Some(("actor".to_string(), "avatar.jpg".to_string()))
        );
        // Wrong root should return None
        assert_eq!(
            path_to_attachment(
                Path::new("/other/.kanban/tasks/.attachments/file.png"),
                &root
            ),
            None
        );
    }

    #[test]
    fn test_parse_plain_yaml() {
        let content = "tag_name: bug\ncolor: \"ff0000\"\norder: 5\n";
        let fields = parse_plain_yaml(content).unwrap();
        assert_eq!(fields.get("tag_name").unwrap(), &serde_json::json!("bug"));
        assert_eq!(fields.get("color").unwrap(), &serde_json::json!("ff0000"));
        assert_eq!(fields.get("order").unwrap(), &serde_json::json!(5));
    }

    #[test]
    fn test_parse_plain_yaml_flattens_nested() {
        let content = "name: To Do\nmetadata:\n  color: red\n  icon: star\n";
        let fields = parse_plain_yaml(content).unwrap();
        assert_eq!(fields.get("name").unwrap(), &serde_json::json!("To Do"));
        assert_eq!(
            fields.get("metadata_color").unwrap(),
            &serde_json::json!("red")
        );
        assert_eq!(
            fields.get("metadata_icon").unwrap(),
            &serde_json::json!("star")
        );
        assert!(!fields.contains_key("metadata"));
    }

    #[test]
    fn test_parse_frontmatter_body() {
        let content = "---\ntitle: My Task\nassignees: []\n---\nBody text here\n";
        let fields = parse_frontmatter_body(content).unwrap();
        assert_eq!(fields.get("title").unwrap(), &serde_json::json!("My Task"));
        assert_eq!(
            fields.get("body").unwrap(),
            &serde_json::json!("Body text here\n")
        );
        assert_eq!(fields.get("assignees").unwrap(), &serde_json::json!([]));
    }

    #[test]
    fn test_parse_frontmatter_flattens_nested() {
        let content = "---\ntitle: T\nposition:\n  column: todo\n  ordinal: a0\n---\nBody\n";
        let fields = parse_frontmatter_body(content).unwrap();
        assert_eq!(
            fields.get("position_column").unwrap(),
            &serde_json::json!("todo")
        );
        assert_eq!(
            fields.get("position_ordinal").unwrap(),
            &serde_json::json!("a0")
        );
        assert!(!fields.contains_key("position"));
    }

    #[test]
    fn test_diff_fields_no_change() {
        let old: HashMap<String, serde_json::Value> =
            [("a".into(), serde_json::json!("x"))].into_iter().collect();
        let new = old.clone();
        assert!(diff_fields(&old, &new).is_empty());
    }

    #[test]
    fn test_diff_fields_modified() {
        let old: HashMap<String, serde_json::Value> = [("a".into(), serde_json::json!("old"))]
            .into_iter()
            .collect();
        let new: HashMap<String, serde_json::Value> = [("a".into(), serde_json::json!("new"))]
            .into_iter()
            .collect();
        let changes = diff_fields(&old, &new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "a");
        assert_eq!(changes[0].value, serde_json::json!("new"));
    }

    #[test]
    fn test_diff_fields_added() {
        let old: HashMap<String, serde_json::Value> = HashMap::new();
        let new: HashMap<String, serde_json::Value> =
            [("b".into(), serde_json::json!(42))].into_iter().collect();
        let changes = diff_fields(&old, &new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "b");
    }

    #[test]
    fn test_diff_fields_removed() {
        let old: HashMap<String, serde_json::Value> = [("c".into(), serde_json::json!("gone"))]
            .into_iter()
            .collect();
        let new: HashMap<String, serde_json::Value> = HashMap::new();
        let changes = diff_fields(&old, &new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "c");
        assert_eq!(changes[0].value, serde_json::Value::Null);
    }

    #[test]
    fn test_cache_population() {
        let (_tmp, kanban) = setup_kanban_dir();

        std::fs::write(kanban.join("tasks/abc.md"), "---\ntitle: Test\n---\nBody\n").unwrap();
        std::fs::write(kanban.join("tags/bug.yaml"), "tag_name: Bug\n").unwrap();
        std::fs::write(kanban.join("tasks/abc.jsonl"), "log\n").unwrap(); // not entity

        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);
        let map = cache.lock().unwrap();
        assert_eq!(map.len(), 2);
        assert!(map.contains_key(&kanban.join("tasks/abc.md")));
        assert!(map.contains_key(&kanban.join("tags/bug.yaml")));
    }

    // =========================================================================
    // Resolve-level tests (no watcher, just resolve_change/resolve_removal)
    // =========================================================================

    #[test]
    fn test_resolve_change_field_level() {
        let (_tmp, kanban) = setup_kanban_dir();
        let path = kanban.join("tags/bug.yaml");
        std::fs::write(&path, "tag_name: Bug\ncolor: \"ff0000\"\n").unwrap();

        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);

        // Same content → None
        assert!(resolve_change(&path, &cache, &kanban).is_none());

        // Change one field
        std::fs::write(&path, "tag_name: Bug\ncolor: \"00ff00\"\n").unwrap();
        let evt = resolve_change(&path, &cache, &kanban);
        match evt {
            Some(WatchEvent::EntityFieldChanged {
                entity_type,
                id,
                changes,
                ..
            }) => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "bug");
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0].field, "color");
                assert_eq!(changes[0].value, serde_json::json!("00ff00"));
            }
            _ => panic!("expected EntityFieldChanged, got {:?}", evt),
        }
    }

    #[test]
    fn test_resolve_change_new_entity() {
        let (_tmp, kanban) = setup_kanban_dir();
        let cache = Arc::new(Mutex::new(HashMap::new())); // empty

        let path = kanban.join("tags/new.yaml");
        std::fs::write(&path, "tag_name: New\ncolor: \"abcdef\"\n").unwrap();

        let evt = resolve_change(&path, &cache, &kanban);
        match evt {
            Some(WatchEvent::EntityCreated {
                entity_type,
                id,
                fields,
            }) => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "new");
                assert_eq!(fields.get("tag_name").unwrap(), &serde_json::json!("New"));
            }
            _ => panic!("expected EntityCreated, got {:?}", evt),
        }
    }

    #[test]
    fn test_resolve_removal() {
        let (_tmp, kanban) = setup_kanban_dir();
        let path = kanban.join("tags/old.yaml");
        std::fs::write(&path, "tag_name: old\n").unwrap();

        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);

        let evt = resolve_removal(&path, &cache, &kanban);
        match evt {
            Some(WatchEvent::EntityRemoved { entity_type, id }) => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "old");
            }
            _ => panic!("expected EntityRemoved, got {:?}", evt),
        }
    }

    // =========================================================================
    // Integration tests — full watcher lifecycle
    // =========================================================================

    #[tokio::test]
    async fn test_watcher_emits_field_changes() {
        let (_tmp, kanban) = setup_kanban_dir();

        let task_path = kanban.join("tasks/test.md");
        std::fs::write(&task_path, "---\ntitle: Original\n---\nBody\n").unwrap();

        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);
        let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let _watcher = start_watching(kanban.clone(), roots.clone(), cache.clone(), move |evt| {
            events_clone.lock().unwrap().push(evt);
        })
        .expect("start_watching should succeed");

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // External edit: change title
        std::fs::write(&task_path, "---\ntitle: Modified\n---\nBody\n").unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        let captured = events.lock().unwrap();
        assert!(!captured.is_empty(), "should have events");
        match &captured[0] {
            WatchEvent::EntityFieldChanged {
                entity_type,
                id,
                changes,
                ..
            } => {
                assert_eq!(entity_type, "task");
                assert_eq!(id, "test");
                assert!(changes.iter().any(|c| c.field == "title"));
            }
            other => panic!("expected EntityFieldChanged, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_watcher_ignores_own_writes() {
        let (_tmp, kanban) = setup_kanban_dir();

        let tag_path = kanban.join("tags/test.yaml");
        std::fs::write(&tag_path, "tag_name: Original\n").unwrap();

        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);
        let change_count = Arc::new(AtomicUsize::new(0));
        let count_clone = change_count.clone();

        let _watcher = start_watching(kanban.clone(), roots.clone(), cache.clone(), move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        })
        .expect("start_watching should succeed");

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Simulate our own write: write file then immediately update cache.
        // The debounce (200ms) ensures the watcher hasn't resolved yet.
        std::fs::write(&tag_path, "tag_name: Updated\n").unwrap();
        update_cache(&cache, &tag_path);

        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        assert_eq!(
            change_count.load(Ordering::SeqCst),
            0,
            "should NOT fire for our own writes"
        );
    }

    #[tokio::test]
    async fn test_watcher_detects_new_file() {
        let (_tmp, kanban) = setup_kanban_dir();

        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);
        let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let _watcher = start_watching(kanban.clone(), roots.clone(), cache.clone(), move |evt| {
            events_clone.lock().unwrap().push(evt);
        })
        .expect("start_watching should succeed");

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        std::fs::write(
            kanban.join("tags/new-tag.yaml"),
            "tag_name: new\ncolor: \"ff0000\"\n",
        )
        .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        let captured = events.lock().unwrap();
        assert!(!captured.is_empty());
        match &captured[0] {
            WatchEvent::EntityCreated {
                entity_type,
                id,
                fields,
            } => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "new-tag");
                assert_eq!(fields.get("tag_name").unwrap(), &serde_json::json!("new"));
            }
            other => panic!("expected EntityCreated, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_watcher_detects_deletion() {
        let (_tmp, kanban) = setup_kanban_dir();

        let tag_path = kanban.join("tags/doomed.yaml");
        std::fs::write(&tag_path, "tag_name: doomed\n").unwrap();

        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);
        let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let _watcher = start_watching(kanban.clone(), roots.clone(), cache.clone(), move |evt| {
            events_clone.lock().unwrap().push(evt);
        })
        .expect("start_watching should succeed");

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        std::fs::remove_file(&tag_path).unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        let captured = events.lock().unwrap();
        assert!(!captured.is_empty());
        match &captured[0] {
            WatchEvent::EntityRemoved { entity_type, id } => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "doomed");
            }
            other => panic!("expected EntityRemoved, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_watcher_stops_on_drop() {
        let (_tmp, kanban) = setup_kanban_dir();
        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);
        let change_count = Arc::new(AtomicUsize::new(0));
        let count_clone = change_count.clone();

        let watcher = start_watching(kanban.clone(), roots.clone(), cache.clone(), move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        })
        .expect("start_watching should succeed");

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        drop(watcher);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        std::fs::write(kanban.join("tasks/ghost.md"), "---\ntitle: G\n---\n").unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        assert_eq!(change_count.load(Ordering::SeqCst), 0);
    }

    // =========================================================================
    // flush_and_emit tests
    // =========================================================================

    #[test]
    fn test_flush_and_emit_detects_new_file() {
        let (_tmp, kanban) = setup_kanban_dir();
        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);

        // Add a file after cache was built
        std::fs::write(kanban.join("tags/new.yaml"), "tag_name: New\n").unwrap();

        let events = flush_and_emit(&kanban, &roots, &cache);
        assert_eq!(events.len(), 1);
        match &events[0] {
            WatchEvent::EntityCreated {
                entity_type, id, ..
            } => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "new");
            }
            other => panic!("expected EntityCreated, got {:?}", other),
        }

        // Second flush should produce no events (cache updated)
        let events2 = flush_and_emit(&kanban, &roots, &cache);
        assert!(
            events2.is_empty(),
            "should produce no events on second flush"
        );
    }

    #[test]
    fn test_flush_and_emit_detects_field_change() {
        let (_tmp, kanban) = setup_kanban_dir();
        std::fs::write(
            kanban.join("tags/bug.yaml"),
            "tag_name: Bug\ncolor: \"ff0000\"\n",
        )
        .unwrap();
        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);

        // Modify a field
        std::fs::write(
            kanban.join("tags/bug.yaml"),
            "tag_name: Bug\ncolor: \"00ff00\"\n",
        )
        .unwrap();

        let events = flush_and_emit(&kanban, &roots, &cache);
        assert_eq!(events.len(), 1);
        match &events[0] {
            WatchEvent::EntityFieldChanged {
                entity_type,
                id,
                changes,
                ..
            } => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "bug");
                assert_eq!(changes.len(), 1);
                assert_eq!(changes[0].field, "color");
            }
            other => panic!("expected EntityFieldChanged, got {:?}", other),
        }
    }

    #[test]
    fn test_flush_and_emit_detects_removal() {
        let (_tmp, kanban) = setup_kanban_dir();
        std::fs::write(kanban.join("tags/old.yaml"), "tag_name: old\n").unwrap();
        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);

        std::fs::remove_file(kanban.join("tags/old.yaml")).unwrap();

        let events = flush_and_emit(&kanban, &roots, &cache);
        assert_eq!(events.len(), 1);
        match &events[0] {
            WatchEvent::EntityRemoved { entity_type, id } => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "old");
            }
            other => panic!("expected EntityRemoved, got {:?}", other),
        }
    }

    #[test]
    fn test_flush_and_emit_no_changes() {
        let (_tmp, kanban) = setup_kanban_dir();
        std::fs::write(kanban.join("tags/bug.yaml"), "tag_name: Bug\n").unwrap();
        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);

        let events = flush_and_emit(&kanban, &roots, &cache);
        assert!(
            events.is_empty(),
            "should produce no events when nothing changed"
        );
    }

    #[test]
    fn test_flush_and_emit_body_change_triggers_event() {
        // When a task body changes (which would cause derived fields like
        // tags and progress to change), flush_and_emit should detect the
        // body change. Computed fields are enriched by dispatch_command.
        let (_tmp, kanban) = setup_kanban_dir();
        std::fs::write(
            kanban.join("tasks/task1.md"),
            "---\ntitle: My Task\nassignees: []\n---\nOriginal body\n",
        )
        .unwrap();
        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);

        // Edit the body to include a #tag reference
        std::fs::write(
            kanban.join("tasks/task1.md"),
            "---\ntitle: My Task\nassignees: []\n---\nBody with #bug tag\n",
        )
        .unwrap();

        let events = flush_and_emit(&kanban, &roots, &cache);
        assert_eq!(events.len(), 1);
        match &events[0] {
            WatchEvent::EntityFieldChanged {
                entity_type,
                id,
                changes,
                fields,
            } => {
                assert_eq!(entity_type, "task");
                assert_eq!(id, "task1");
                // Raw body field should be detected as changed
                assert!(
                    changes.iter().any(|c| c.field == "body"),
                    "should detect body field change, got: {:?}",
                    changes.iter().map(|c| &c.field).collect::<Vec<_>>()
                );
                // fields is None for raw watcher events (enriched by dispatch_command)
                assert!(fields.is_none());
            }
            other => panic!("expected EntityFieldChanged, got {:?}", other),
        }
    }

    #[test]
    fn test_flush_and_emit_progress_body_change() {
        // GFM task list changes in body should be detected, enabling
        // the progress computed field to be re-derived.
        let (_tmp, kanban) = setup_kanban_dir();
        std::fs::write(
            kanban.join("tasks/task1.md"),
            "---\ntitle: Checklist\nassignees: []\n---\n- [ ] Step 1\n- [ ] Step 2\n",
        )
        .unwrap();
        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);

        // Check off one item
        std::fs::write(
            kanban.join("tasks/task1.md"),
            "---\ntitle: Checklist\nassignees: []\n---\n- [x] Step 1\n- [ ] Step 2\n",
        )
        .unwrap();

        let events = flush_and_emit(&kanban, &roots, &cache);
        assert_eq!(events.len(), 1);
        match &events[0] {
            WatchEvent::EntityFieldChanged {
                entity_type,
                id,
                changes,
                ..
            } => {
                assert_eq!(entity_type, "task");
                assert_eq!(id, "task1");
                assert!(
                    changes.iter().any(|c| c.field == "body"),
                    "should detect body change for progress re-derivation"
                );
            }
            other => panic!("expected EntityFieldChanged, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_flush_and_emit_suppresses_watcher() {
        let (_tmp, kanban) = setup_kanban_dir();
        std::fs::write(kanban.join("tags/test.yaml"), "tag_name: Original\n").unwrap();

        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);
        let change_count = Arc::new(AtomicUsize::new(0));
        let count_clone = change_count.clone();

        let _watcher = start_watching(kanban.clone(), roots.clone(), cache.clone(), move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        })
        .expect("start_watching should succeed");

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Write a file and immediately flush_and_emit (simulates command execution).
        // This updates the cache, so the watcher's debounce should NOT fire.
        std::fs::write(kanban.join("tags/test.yaml"), "tag_name: Updated\n").unwrap();
        let events = flush_and_emit(&kanban, &roots, &cache);
        assert_eq!(events.len(), 1); // flush_and_emit returns the event

        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        assert_eq!(
            change_count.load(Ordering::SeqCst),
            0,
            "watcher should NOT fire because flush_and_emit updated the cache"
        );
    }

    // =========================================================================
    // sync_search_index tests
    // =========================================================================

    #[test]
    fn test_sync_search_index_entity_created() {
        let mut idx = swissarmyhammer_entity_search::EntitySearchIndex::new();
        let mut fields = HashMap::new();
        fields.insert("title".to_string(), serde_json::json!("My Task"));
        fields.insert("status".to_string(), serde_json::json!("open"));

        let evt = WatchEvent::EntityCreated {
            entity_type: "task".to_string(),
            id: "ABC123".to_string(),
            fields,
        };
        sync_search_index(&mut idx, &evt);

        // Verify entity was added to the index
        let results = idx.search("My Task", 10);
        assert!(!results.is_empty(), "entity should be findable after sync");
        assert_eq!(results[0].entity_id, "ABC123");
    }

    #[test]
    fn test_sync_search_index_entity_field_changed_with_fields() {
        let mut idx = swissarmyhammer_entity_search::EntitySearchIndex::new();

        // First, add an entity
        let mut initial_fields = HashMap::new();
        initial_fields.insert("title".to_string(), serde_json::json!("Original"));
        let create_evt = WatchEvent::EntityCreated {
            entity_type: "task".to_string(),
            id: "T1".to_string(),
            fields: initial_fields,
        };
        sync_search_index(&mut idx, &create_evt);

        // Now update it with field changes (with fields present)
        let mut updated_fields = HashMap::new();
        updated_fields.insert("title".to_string(), serde_json::json!("Updated Title"));
        let change_evt = WatchEvent::EntityFieldChanged {
            entity_type: "task".to_string(),
            id: "T1".to_string(),
            changes: vec![FieldChange {
                field: "title".to_string(),
                value: serde_json::json!("Updated Title"),
            }],
            fields: Some(updated_fields),
        };
        sync_search_index(&mut idx, &change_evt);

        let results = idx.search("Updated Title", 10);
        assert!(!results.is_empty(), "updated entity should be findable");
    }

    #[test]
    fn test_sync_search_index_entity_field_changed_without_fields() {
        let mut idx = swissarmyhammer_entity_search::EntitySearchIndex::new();

        // Add an entity first
        let mut fields = HashMap::new();
        fields.insert("title".to_string(), serde_json::json!("Task"));
        let create_evt = WatchEvent::EntityCreated {
            entity_type: "task".to_string(),
            id: "T2".to_string(),
            fields,
        };
        sync_search_index(&mut idx, &create_evt);

        // EntityFieldChanged with fields: None should be a no-op
        let change_evt = WatchEvent::EntityFieldChanged {
            entity_type: "task".to_string(),
            id: "T2".to_string(),
            changes: vec![FieldChange {
                field: "title".to_string(),
                value: serde_json::json!("New"),
            }],
            fields: None,
        };
        sync_search_index(&mut idx, &change_evt);

        // Entity still in index with original title
        let results = idx.search("Task", 10);
        assert!(!results.is_empty(), "entity should still exist");
    }

    #[test]
    fn test_sync_search_index_entity_removed() {
        let mut idx = swissarmyhammer_entity_search::EntitySearchIndex::new();

        let mut fields = HashMap::new();
        fields.insert("title".to_string(), serde_json::json!("Doomed"));
        let create_evt = WatchEvent::EntityCreated {
            entity_type: "task".to_string(),
            id: "DOOM".to_string(),
            fields,
        };
        sync_search_index(&mut idx, &create_evt);

        let remove_evt = WatchEvent::EntityRemoved {
            entity_type: "task".to_string(),
            id: "DOOM".to_string(),
        };
        sync_search_index(&mut idx, &remove_evt);

        let results = idx.search("Doomed", 10);
        assert!(
            results.is_empty() || results.iter().all(|r| r.entity_id != "DOOM"),
            "removed entity should not be findable"
        );
    }

    #[test]
    fn test_sync_search_index_attachment_changed_noop() {
        let mut idx = swissarmyhammer_entity_search::EntitySearchIndex::new();

        // AttachmentChanged events should be silently ignored
        let evt = WatchEvent::AttachmentChanged {
            entity_type: "task".to_string(),
            filename: "screenshot.png".to_string(),
            removed: false,
        };
        sync_search_index(&mut idx, &evt);
        // No panic, no entities added — just a no-op
    }

    // =========================================================================
    // classify_event tests
    // =========================================================================

    #[test]
    fn test_classify_event_create_entity() {
        let (_tmp, kanban) = setup_kanban_dir();
        let path = kanban.join("tasks/abc.yaml");
        std::fs::write(&path, "title: test\n").unwrap();

        let event = Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![path.clone()],
            attrs: Default::default(),
        };

        let actions = classify_event(&event, &kanban);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], FsAction::Changed(_)));
    }

    #[test]
    fn test_classify_event_modify_entity() {
        let (_tmp, kanban) = setup_kanban_dir();
        let path = kanban.join("tags/bug.yaml");
        std::fs::write(&path, "tag_name: bug\n").unwrap();

        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content,
            )),
            paths: vec![path.clone()],
            attrs: Default::default(),
        };

        let actions = classify_event(&event, &kanban);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], FsAction::Changed(_)));
    }

    #[test]
    fn test_classify_event_remove_entity() {
        let (_tmp, kanban) = setup_kanban_dir();
        // File doesn't need to exist for Remove events
        let path = kanban.join("tags/gone.yaml");

        let event = Event {
            kind: EventKind::Remove(notify::event::RemoveKind::File),
            paths: vec![path.clone()],
            attrs: Default::default(),
        };

        let actions = classify_event(&event, &kanban);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], FsAction::Removed(_)));
    }

    #[test]
    fn test_classify_event_other_kind_treated_as_changed() {
        let (_tmp, kanban) = setup_kanban_dir();
        let path = kanban.join("tasks/abc.yaml");
        std::fs::write(&path, "title: test\n").unwrap();

        let event = Event {
            kind: EventKind::Other,
            paths: vec![path.clone()],
            attrs: Default::default(),
        };

        let actions = classify_event(&event, &kanban);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], FsAction::Changed(_)));
    }

    #[test]
    fn test_classify_event_ignores_non_entity_non_attachment() {
        let (_tmp, kanban) = setup_kanban_dir();
        let path = kanban.join("tasks/abc.jsonl");
        std::fs::write(&path, "log\n").unwrap();

        let event = Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![path.clone()],
            attrs: Default::default(),
        };

        let actions = classify_event(&event, &kanban);
        assert!(actions.is_empty(), "should ignore non-entity files");
    }

    #[test]
    fn test_classify_event_entity_in_unrecognized_subdir() {
        let (_tmp, kanban) = setup_kanban_dir();
        let weird_dir = kanban.join("unknown");
        std::fs::create_dir_all(&weird_dir).unwrap();
        let path = weird_dir.join("foo.yaml");
        std::fs::write(&path, "name: foo\n").unwrap();

        let event = Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![path.clone()],
            attrs: Default::default(),
        };

        let actions = classify_event(&event, &kanban);
        assert!(
            actions.is_empty(),
            "should skip entity files in unrecognized subdirectories"
        );
    }

    #[test]
    fn test_classify_event_attachment_file() {
        let (_tmp, kanban) = setup_kanban_dir();
        let att_dir = kanban.join("tasks/.attachments");
        std::fs::create_dir_all(&att_dir).unwrap();
        let path = att_dir.join("screenshot.png");
        std::fs::write(&path, "PNG data").unwrap();

        let event = Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![path.clone()],
            attrs: Default::default(),
        };

        let actions = classify_event(&event, &kanban);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], FsAction::Changed(_)));
    }

    #[test]
    fn test_classify_event_attachment_removal() {
        let (_tmp, kanban) = setup_kanban_dir();
        let att_dir = kanban.join("tasks/.attachments");
        std::fs::create_dir_all(&att_dir).unwrap();
        // File doesn't need to exist for Remove events
        let path = att_dir.join("deleted.png");

        let event = Event {
            kind: EventKind::Remove(notify::event::RemoveKind::File),
            paths: vec![path.clone()],
            attrs: Default::default(),
        };

        let actions = classify_event(&event, &kanban);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], FsAction::Removed(_)));
    }

    #[test]
    fn test_classify_event_multiple_paths() {
        let (_tmp, kanban) = setup_kanban_dir();
        let path1 = kanban.join("tasks/t1.yaml");
        let path2 = kanban.join("tags/bug.yaml");
        std::fs::write(&path1, "title: t1\n").unwrap();
        std::fs::write(&path2, "tag_name: bug\n").unwrap();

        let event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content,
            )),
            paths: vec![path1, path2],
            attrs: Default::default(),
        };

        let actions = classify_event(&event, &kanban);
        assert_eq!(actions.len(), 2, "should produce one action per path");
    }

    // =========================================================================
    // resolve_change / resolve_removal for attachments
    // =========================================================================

    #[test]
    fn test_resolve_change_attachment() {
        let (_tmp, kanban) = setup_kanban_dir();
        let att_dir = kanban.join("tasks/.attachments");
        std::fs::create_dir_all(&att_dir).unwrap();
        let path = att_dir.join("screenshot.png");
        std::fs::write(&path, "PNG data").unwrap();

        let cache = new_entity_cache(&kanban);
        let evt = resolve_change(&path.canonicalize().unwrap(), &cache, &kanban);
        match evt {
            Some(WatchEvent::AttachmentChanged {
                entity_type,
                filename,
                removed,
            }) => {
                assert_eq!(entity_type, "task");
                assert_eq!(filename, "screenshot.png");
                assert!(!removed);
            }
            other => panic!("expected AttachmentChanged, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_removal_attachment() {
        let (_tmp, kanban) = setup_kanban_dir();
        let att_dir = kanban.join("tasks/.attachments");
        std::fs::create_dir_all(&att_dir).unwrap();
        // Create and then remove the file
        let path = att_dir.join("deleted.png");
        std::fs::write(&path, "data").unwrap();
        let canonical = path.canonicalize().unwrap();
        std::fs::remove_file(&path).unwrap();

        let cache = new_entity_cache(&kanban);
        let evt = resolve_removal(&canonical, &cache, &kanban);
        match evt {
            Some(WatchEvent::AttachmentChanged {
                entity_type,
                filename,
                removed,
            }) => {
                assert_eq!(entity_type, "task");
                assert_eq!(filename, "deleted.png");
                assert!(removed);
            }
            other => panic!("expected AttachmentChanged removed, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_removal_untracked_entity() {
        let (_tmp, kanban) = setup_kanban_dir();
        // Don't create the file first, so it's never in the cache
        let path = kanban.join("tags/phantom.yaml");

        let cache = Arc::new(Mutex::new(HashMap::new()));
        let evt = resolve_removal(&path, &cache, &kanban);
        assert!(
            evt.is_none(),
            "should return None for untracked (never-cached) entity"
        );
    }

    // =========================================================================
    // Root-level entity file tests
    // =========================================================================

    #[test]
    fn test_cache_population_root_level_files() {
        let (_tmp, kanban) = setup_kanban_dir();
        std::fs::write(kanban.join("board.yaml"), "name: My Board\n").unwrap();

        let cache = new_entity_cache(&kanban);
        let map = cache.lock().unwrap();
        assert!(
            map.contains_key(&kanban.join("board.yaml")),
            "root-level entity files should be cached"
        );
    }

    #[test]
    fn test_flush_and_emit_root_level_files() {
        let (_tmp, kanban) = setup_kanban_dir();
        let cache = new_entity_cache(&kanban);

        // Add a root-level entity file after cache was built
        std::fs::write(kanban.join("board.yaml"), "name: My Board\n").unwrap();

        let events = flush_and_emit(&kanban, &cache);
        assert_eq!(events.len(), 1);
        match &events[0] {
            WatchEvent::EntityCreated {
                entity_type,
                id,
                fields,
            } => {
                assert_eq!(entity_type, "board");
                assert_eq!(id, "board");
                assert_eq!(fields.get("name").unwrap(), &serde_json::json!("My Board"));
            }
            other => panic!("expected EntityCreated for root file, got {:?}", other),
        }
    }

    // =========================================================================
    // update_cache tests
    // =========================================================================

    #[test]
    fn test_update_cache_suppresses_change_detection() {
        let (_tmp, kanban) = setup_kanban_dir();
        let path = kanban.join("tags/test.yaml");
        std::fs::write(&path, "tag_name: Original\n").unwrap();

        let cache = new_entity_cache(&kanban);

        // Write new content, then update cache
        std::fs::write(&path, "tag_name: Updated\n").unwrap();
        update_cache(&cache, &path);

        // Now resolve_change should return None (cache matches)
        assert!(
            resolve_change(&path, &cache, &kanban).is_none(),
            "should not detect change after update_cache"
        );
    }

    // =========================================================================
    // parse_entity_file edge cases
    // =========================================================================

    #[test]
    fn test_parse_entity_file_unknown_extension() {
        let result = parse_entity_file(Path::new("/foo/bar.txt"), "some content");
        assert!(result.is_none(), "unknown extension should return None");
    }

    #[test]
    fn test_parse_entity_file_yaml_extension() {
        let result = parse_entity_file(Path::new("/foo/bar.yaml"), "name: test\n");
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().get("name").unwrap(),
            &serde_json::json!("test")
        );
    }

    #[test]
    fn test_parse_entity_file_yml_extension() {
        let result = parse_entity_file(Path::new("/foo/bar.yml"), "name: test\n");
        assert!(result.is_some());
    }

    #[test]
    fn test_parse_entity_file_md_extension() {
        let result = parse_entity_file(Path::new("/foo/bar.md"), "---\ntitle: T\n---\nBody\n");
        assert!(result.is_some());
        let fields = result.unwrap();
        assert_eq!(fields.get("title").unwrap(), &serde_json::json!("T"));
        assert!(fields.contains_key("body"));
    }

    #[test]
    fn test_parse_frontmatter_body_no_frontmatter() {
        let result = parse_frontmatter_body("Just plain text with no frontmatter");
        assert!(result.is_none(), "no --- delimiters should return None");
    }

    #[test]
    fn test_parse_plain_yaml_scalar_returns_none() {
        // A bare string is valid YAML but not an object — json_to_field_map returns None
        let result = parse_plain_yaml("just a string");
        assert!(
            result.is_none(),
            "non-object YAML should return None from json_to_field_map"
        );
    }

    // =========================================================================
    // path_to_entity edge cases
    // =========================================================================

    #[test]
    fn test_path_to_entity_unrecognized_subdir() {
        let root = PathBuf::from("/project/.kanban");
        assert_eq!(
            path_to_entity(Path::new("/project/.kanban/unknown/abc.yaml"), &root),
            None,
            "files in unrecognized subdirectories should return None"
        );
    }

    #[test]
    fn test_path_to_entity_swimlanes_strips_s() {
        let root = PathBuf::from("/project/.kanban");
        assert_eq!(
            path_to_entity(Path::new("/project/.kanban/swimlanes/main.yaml"), &root),
            Some(("swimlane".to_string(), "main".to_string()))
        );
    }

    #[test]
    fn test_path_to_entity_views_strips_s() {
        let root = PathBuf::from("/project/.kanban");
        assert_eq!(
            path_to_entity(Path::new("/project/.kanban/views/board.yaml"), &root),
            Some(("view".to_string(), "board".to_string()))
        );
    }

    #[test]
    fn test_path_to_entity_actors_strips_s() {
        let root = PathBuf::from("/project/.kanban");
        assert_eq!(
            path_to_entity(Path::new("/project/.kanban/actors/alice.yaml"), &root),
            Some(("actor".to_string(), "alice".to_string()))
        );
    }

    #[test]
    fn test_path_to_entity_boards_strips_s() {
        let root = PathBuf::from("/project/.kanban");
        assert_eq!(
            path_to_entity(Path::new("/project/.kanban/boards/main.yaml"), &root),
            Some(("board".to_string(), "main".to_string()))
        );
    }

    // =========================================================================
    // is_attachment edge cases
    // =========================================================================

    #[test]
    fn test_is_attachment_no_parent() {
        // A bare filename with no parent
        assert!(!is_attachment(Path::new("file.png")));
    }

    // =========================================================================
    // WatchEvent and BoardWatchEvent serialization tests
    // =========================================================================

    #[test]
    fn test_watch_event_serialization_entity_created() {
        let mut fields = HashMap::new();
        fields.insert("title".to_string(), serde_json::json!("Test"));
        let evt = WatchEvent::EntityCreated {
            entity_type: "task".to_string(),
            id: "T1".to_string(),
            fields,
        };
        let json = serde_json::to_value(&evt).unwrap();
        assert_eq!(json["kind"], "entity-created");
        assert_eq!(json["entity_type"], "task");
    }

    #[test]
    fn test_watch_event_serialization_entity_removed() {
        let evt = WatchEvent::EntityRemoved {
            entity_type: "tag".to_string(),
            id: "bug".to_string(),
        };
        let json = serde_json::to_value(&evt).unwrap();
        assert_eq!(json["kind"], "entity-removed");
    }

    #[test]
    fn test_watch_event_serialization_field_changed_no_fields() {
        let evt = WatchEvent::EntityFieldChanged {
            entity_type: "task".to_string(),
            id: "T1".to_string(),
            changes: vec![FieldChange {
                field: "title".to_string(),
                value: serde_json::json!("New"),
            }],
            fields: None,
        };
        let json = serde_json::to_value(&evt).unwrap();
        assert_eq!(json["kind"], "entity-field-changed");
        // fields should be absent (skip_serializing_if = None)
        assert!(json.get("fields").is_none());
    }

    #[test]
    fn test_watch_event_serialization_attachment_changed() {
        let evt = WatchEvent::AttachmentChanged {
            entity_type: "task".to_string(),
            filename: "screenshot.png".to_string(),
            removed: false,
        };
        let json = serde_json::to_value(&evt).unwrap();
        assert_eq!(json["kind"], "attachment-changed");
        assert_eq!(json["filename"], "screenshot.png");
    }

    #[test]
    fn test_board_watch_event_flattens() {
        let evt = BoardWatchEvent {
            event: WatchEvent::EntityRemoved {
                entity_type: "tag".to_string(),
                id: "bug".to_string(),
            },
            board_path: "/path/to/board".to_string(),
        };
        let json = serde_json::to_value(&evt).unwrap();
        assert_eq!(json["kind"], "entity-removed");
        assert_eq!(json["board_path"], "/path/to/board");
    }

    // =========================================================================
    // yaml_to_json edge cases
    // =========================================================================

    #[test]
    fn test_yaml_to_json_valid() {
        let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str("name: test\ncount: 42").unwrap();
        let json = yaml_to_json(yaml).unwrap();
        assert_eq!(json["name"], "test");
        assert_eq!(json["count"], 42);
    }

    #[test]
    fn test_json_to_field_map_non_object() {
        // json_to_field_map should return None for non-object values
        let val = serde_json::json!("just a string");
        assert!(json_to_field_map(val).is_none());
    }

    // =========================================================================
    // resolve_change with same hash (no real change)
    // =========================================================================

    #[test]
    fn test_resolve_change_same_hash_but_different_fields_returns_none() {
        let (_tmp, kanban) = setup_kanban_dir();
        let path = kanban.join("tags/stable.yaml");
        std::fs::write(&path, "tag_name: Stable\ncolor: \"aaa\"\n").unwrap();

        let cache = new_entity_cache(&kanban);

        // Re-read the same file — same hash, no change
        let evt = resolve_change(&path, &cache, &kanban);
        assert!(evt.is_none(), "same content should produce no event");
    }

    // =========================================================================
    // flush_and_emit with root-level field changes
    // =========================================================================

    #[test]
    fn test_flush_and_emit_root_level_field_change() {
        let (_tmp, kanban) = setup_kanban_dir();
        std::fs::write(kanban.join("board.yaml"), "name: Old Name\n").unwrap();
        let cache = new_entity_cache(&kanban);

        std::fs::write(kanban.join("board.yaml"), "name: New Name\n").unwrap();

        let events = flush_and_emit(&kanban, &cache);
        assert_eq!(events.len(), 1);
        match &events[0] {
            WatchEvent::EntityFieldChanged {
                entity_type,
                id,
                changes,
                ..
            } => {
                assert_eq!(entity_type, "board");
                assert_eq!(id, "board");
                assert!(changes.iter().any(|c| c.field == "name"));
            }
            other => panic!("expected EntityFieldChanged, got {:?}", other),
        }
    }

    #[test]
    fn test_flush_and_emit_root_level_removal() {
        let (_tmp, kanban) = setup_kanban_dir();
        std::fs::write(kanban.join("board.yaml"), "name: Board\n").unwrap();
        let cache = new_entity_cache(&kanban);

        std::fs::remove_file(kanban.join("board.yaml")).unwrap();

        let events = flush_and_emit(&kanban, &cache);
        assert_eq!(events.len(), 1);
        match &events[0] {
            WatchEvent::EntityRemoved { entity_type, id } => {
                assert_eq!(entity_type, "board");
                assert_eq!(id, "board");
            }
            other => panic!("expected EntityRemoved, got {:?}", other),
        }
    }

    // =========================================================================
    // Watcher with .attachments/ directory present
    // =========================================================================

    #[tokio::test]
    async fn test_watcher_with_attachments_dir() {
        let (_tmp, kanban) = setup_kanban_dir();
        // Create .attachments/ dirs so the watcher registers them
        let att_dir = kanban.join("tasks/.attachments");
        std::fs::create_dir_all(&att_dir).unwrap();

        let cache = new_entity_cache(&kanban);
        let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let _watcher = start_watching(kanban.clone(), cache.clone(), move |evt| {
            events_clone.lock().unwrap().push(evt);
        })
        .expect("start_watching should succeed with .attachments/ dir");

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Create an attachment file
        std::fs::write(att_dir.join("screenshot.png"), "PNG data").unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        let captured = events.lock().unwrap();
        assert!(!captured.is_empty(), "should emit attachment event");
        assert!(
            captured.iter().any(|e| matches!(
                e,
                WatchEvent::AttachmentChanged {
                    entity_type,
                    filename,
                    removed,
                } if entity_type == "task" && filename == "screenshot.png" && !removed
            )),
            "should have AttachmentChanged event, got: {:?}",
            *captured
        );
    }

    #[tokio::test]
    async fn test_watcher_detects_attachment_deletion() {
        let (_tmp, kanban) = setup_kanban_dir();
        let att_dir = kanban.join("tasks/.attachments");
        std::fs::create_dir_all(&att_dir).unwrap();
        let att_file = att_dir.join("to-delete.png");
        std::fs::write(&att_file, "data").unwrap();

        let cache = new_entity_cache(&kanban);
        let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let _watcher = start_watching(kanban.clone(), cache.clone(), move |evt| {
            events_clone.lock().unwrap().push(evt);
        })
        .expect("start_watching should succeed");

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        std::fs::remove_file(&att_file).unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        let captured = events.lock().unwrap();
        assert!(
            !captured.is_empty(),
            "should emit events for attachment deletion"
        );
        assert!(
            captured.iter().any(|e| matches!(
                e,
                WatchEvent::AttachmentChanged {
                    entity_type,
                    filename,
                    removed,
                } if entity_type == "task" && filename == "to-delete.png" && *removed
            )),
            "should have AttachmentChanged removed event, got: {:?}",
            *captured
        );
    }

    // =========================================================================
    // resolve_change edge case: hash changed but fields identical
    // =========================================================================

    #[test]
    fn test_resolve_change_hash_differs_but_fields_same() {
        // When a file's hash changes (e.g., trailing whitespace) but the parsed
        // fields are identical, resolve_change should return None.
        let (_tmp, kanban) = setup_kanban_dir();
        let path = kanban.join("tags/ws.yaml");
        std::fs::write(&path, "tag_name: WS\n").unwrap();

        let cache = new_entity_cache(&kanban);

        // Rewrite with trailing whitespace — different hash, same parsed fields
        std::fs::write(&path, "tag_name: WS\n\n").unwrap();

        let evt = resolve_change(&path, &cache, &kanban);
        assert!(
            evt.is_none(),
            "should return None when hash changes but fields are identical"
        );
    }

    // =========================================================================
    // classify_event: removed file (path doesn't exist, canonicalize parent)
    // =========================================================================

    #[test]
    fn test_classify_event_removed_file_canonicalizes_parent() {
        let (_tmp, kanban) = setup_kanban_dir();
        // File doesn't exist — classify_event should canonicalize parent dir
        let path = kanban.join("tasks/removed.yaml");

        let event = Event {
            kind: EventKind::Remove(notify::event::RemoveKind::File),
            paths: vec![path],
            attrs: Default::default(),
        };

        let actions = classify_event(&event, &kanban);
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            FsAction::Removed(p) => {
                // Should have canonicalized the parent portion
                assert!(p.ends_with("tasks/removed.yaml"));
            }
            other => panic!("expected Removed, got {:?}", other),
        }
    }

    // =========================================================================
    // new_entity_cache with multiple subdirs containing files
    // =========================================================================

    #[test]
    fn test_cache_population_multiple_subdirs() {
        let (_tmp, kanban) = setup_kanban_dir();
        std::fs::write(kanban.join("tasks/t1.md"), "---\ntitle: T1\n---\nBody\n").unwrap();
        std::fs::write(kanban.join("tags/bug.yaml"), "tag_name: Bug\n").unwrap();
        std::fs::write(kanban.join("columns/todo.yaml"), "name: To Do\n").unwrap();
        std::fs::write(kanban.join("swimlanes/main.yaml"), "name: Main\n").unwrap();
        std::fs::write(kanban.join("actors/alice.yaml"), "name: Alice\n").unwrap();
        std::fs::write(kanban.join("views/board.yaml"), "name: Board\n").unwrap();
        std::fs::write(kanban.join("board.yaml"), "name: My Board\n").unwrap();

        let cache = new_entity_cache(&kanban);
        let map = cache.lock().unwrap();
        assert_eq!(map.len(), 7, "should cache files from all subdirs + root");
    }

    // =========================================================================
    // flush_and_emit: scan root-level + subdirs together
    // =========================================================================

    #[test]
    fn test_flush_and_emit_scans_all_subdirs() {
        let (_tmp, kanban) = setup_kanban_dir();
        std::fs::write(kanban.join("tasks/t1.md"), "---\ntitle: T1\n---\nBody\n").unwrap();
        std::fs::write(kanban.join("tags/bug.yaml"), "tag_name: Bug\n").unwrap();
        let cache = new_entity_cache(&kanban);

        // Add new files in different subdirs after cache
        std::fs::write(kanban.join("columns/done.yaml"), "name: Done\n").unwrap();
        std::fs::write(kanban.join("board.yaml"), "name: Board\n").unwrap();

        let events = flush_and_emit(&kanban, &cache);
        assert_eq!(
            events.len(),
            2,
            "should detect new files in subdirs and root"
        );
    }

    /// Reproduces the drag-and-drop bug: changing position_ordinal in a task .md
    /// file must be detected by flush_and_emit as an EntityFieldChanged event.
    /// Without this, the frontend never learns the card moved.
    #[test]
    fn test_flush_and_emit_detects_task_position_ordinal_change() {
        let (_tmp, kanban) = setup_kanban_dir();
        // Write initial task with position_ordinal: 7e80
        std::fs::write(
            kanban.join("tasks/task-c.md"),
            "---\ntitle: Card C\nposition_column: todo\nposition_ordinal: 7e80\n---\n",
        )
        .unwrap();
        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);

        // Simulate task.move changing ordinal from 7e80 to 7cc0 (moved before card B)
        std::fs::write(
            kanban.join("tasks/task-c.md"),
            "---\ntitle: Card C\nposition_column: todo\nposition_ordinal: 7cc0\n---\n",
        )
        .unwrap();

        let events = flush_and_emit(&kanban, &roots, &cache);
        assert_eq!(events.len(), 1, "should detect the ordinal change");
        match &events[0] {
            WatchEvent::EntityFieldChanged {
                entity_type,
                id,
                changes,
                ..
            } => {
                assert_eq!(entity_type, "task");
                assert_eq!(id, "task-c");
                assert!(
                    changes.iter().any(|c| c.field == "position_ordinal"),
                    "changed field should be position_ordinal, got: {:?}",
                    changes.iter().map(|c| &c.field).collect::<Vec<_>>()
                );
            }
            other => panic!("expected EntityFieldChanged, got {:?}", other),
        }
    }

    /// Verify that a perspective store registered via store_roots causes
    /// perspective file changes to be detected by the watcher.
    ///
    /// This test uses store_roots directly (as StoreContext::watched_roots()
    /// would return) to confirm that perspectives are included automatically
    /// without special-casing.
    #[tokio::test]
    async fn test_watcher_detects_perspective_file_changes() {
        let (_tmp, kanban) = setup_kanban_dir();
        // perspectives/ is already created by setup_kanban_dir via TEST_SUBDIRS

        let perspective_path = kanban.join("perspectives/myview.yaml");
        std::fs::write(&perspective_path, "name: My View\ntype: kanban\n").unwrap();

        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);
        let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let _watcher = start_watching(kanban.clone(), roots.clone(), cache.clone(), move |evt| {
            events_clone.lock().unwrap().push(evt);
        })
        .expect("start_watching should succeed");

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // External edit to a perspective file
        std::fs::write(&perspective_path, "name: My View\ntype: table\n").unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        let captured = events.lock().unwrap();
        assert!(
            !captured.is_empty(),
            "should have events for perspective file change"
        );
        match &captured[0] {
            WatchEvent::EntityFieldChanged {
                entity_type, id, ..
            } => {
                assert_eq!(entity_type, "perspective");
                assert_eq!(id, "myview");
            }
            WatchEvent::EntityCreated {
                entity_type, id, ..
            } => {
                // Also acceptable if watcher sees it as new
                assert_eq!(entity_type, "perspective");
                assert_eq!(id, "myview");
            }
            other => panic!("expected perspective event, got {:?}", other),
        }
    }

    /// Verify that flush_and_emit detects new perspective files.
    #[test]
    fn test_flush_and_emit_detects_perspective_file() {
        let (_tmp, kanban) = setup_kanban_dir();
        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);

        // Add a perspective file after cache was built
        std::fs::write(
            kanban.join("perspectives/board-view.yaml"),
            "name: Board View\ntype: kanban\n",
        )
        .unwrap();

        let events = flush_and_emit(&kanban, &roots, &cache);
        assert_eq!(events.len(), 1);
        match &events[0] {
            WatchEvent::EntityCreated {
                entity_type, id, ..
            } => {
                assert_eq!(entity_type, "perspective");
                assert_eq!(id, "board-view");
            }
            other => panic!("expected EntityCreated for perspective, got {:?}", other),
        }
    }

    // =========================================================================
    // Named integration tests required by acceptance criteria
    // These test the full start_watching → external write → callback pipeline.
    // =========================================================================

    /// Integration test: external write to an existing entity file while
    /// `start_watching` is running fires `EntityFieldChanged` with the correct
    /// `changes` vec containing the modified field name and value.
    #[tokio::test]
    async fn test_start_watching_field_change_event() {
        let (_tmp, kanban) = setup_kanban_dir();

        let tag_path = kanban.join("tags/mytag.yaml");
        std::fs::write(&tag_path, "tag_name: Original\ncolor: \"ff0000\"\n").unwrap();

        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);
        let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let _watcher = start_watching(kanban.clone(), roots.clone(), cache.clone(), move |evt| {
            events_clone.lock().unwrap().push(evt);
        })
        .expect("start_watching should succeed");

        // Allow the watcher to initialize before writing
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // External write: modify a single field
        std::fs::write(&tag_path, "tag_name: Original\ncolor: \"00ff00\"\n").unwrap();

        // Wait for the debounce to fire (debounce is 200ms, sleep 600ms)
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        let captured = events.lock().unwrap();
        assert!(!captured.is_empty(), "watcher should have fired an event");
        match &captured[0] {
            WatchEvent::EntityFieldChanged {
                entity_type,
                id,
                changes,
                ..
            } => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "mytag");
                assert!(
                    changes.iter().any(|c| c.field == "color"),
                    "changes should contain 'color', got: {:?}",
                    changes.iter().map(|c| &c.field).collect::<Vec<_>>()
                );
                let color_change = changes.iter().find(|c| c.field == "color").unwrap();
                assert_eq!(color_change.value, serde_json::json!("00ff00"));
            }
            other => panic!("expected EntityFieldChanged, got {:?}", other),
        }
    }

    /// Integration test: creating a new YAML entity file while `start_watching`
    /// is running fires `EntityCreated` with the correct entity type, id, and fields.
    #[tokio::test]
    async fn test_start_watching_creates_entity_event() {
        let (_tmp, kanban) = setup_kanban_dir();

        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);
        let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let _watcher = start_watching(kanban.clone(), roots.clone(), cache.clone(), move |evt| {
            events_clone.lock().unwrap().push(evt);
        })
        .expect("start_watching should succeed");

        // Allow the watcher to initialize before writing
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // External write: create a brand-new entity file
        std::fs::write(
            kanban.join("tasks/new-task.md"),
            "---\ntitle: New Task\nassignees: []\n---\nTask body\n",
        )
        .unwrap();

        // Wait for the debounce to fire
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        let captured = events.lock().unwrap();
        assert!(!captured.is_empty(), "watcher should have fired an event");
        match &captured[0] {
            WatchEvent::EntityCreated {
                entity_type,
                id,
                fields,
            } => {
                assert_eq!(entity_type, "task");
                assert_eq!(id, "new-task");
                assert_eq!(fields.get("title").unwrap(), &serde_json::json!("New Task"));
            }
            other => panic!("expected EntityCreated, got {:?}", other),
        }
    }

    /// Integration test: deleting an entity file while `start_watching` is
    /// running fires `EntityRemoved` with the correct entity type and id.
    #[tokio::test]
    async fn test_start_watching_removes_entity_event() {
        let (_tmp, kanban) = setup_kanban_dir();

        let tag_path = kanban.join("tags/deleteme.yaml");
        std::fs::write(&tag_path, "tag_name: deleteme\n").unwrap();

        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);
        let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let _watcher = start_watching(kanban.clone(), roots.clone(), cache.clone(), move |evt| {
            events_clone.lock().unwrap().push(evt);
        })
        .expect("start_watching should succeed");

        // Allow the watcher to initialize before deleting
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // External removal of the entity file
        std::fs::remove_file(&tag_path).unwrap();

        // Wait for the debounce to fire
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        let captured = events.lock().unwrap();
        assert!(!captured.is_empty(), "watcher should have fired an event");
        match &captured[0] {
            WatchEvent::EntityRemoved { entity_type, id } => {
                assert_eq!(entity_type, "tag");
                assert_eq!(id, "deleteme");
            }
            other => panic!("expected EntityRemoved, got {:?}", other),
        }
    }

    /// Integration test: writing a file through "our own" code path (update
    /// cache first, before the debounce fires) does NOT trigger a watcher event.
    ///
    /// This verifies the full async suppression pipeline:
    /// write file → update_cache → debounce fires → hash unchanged → no event.
    #[tokio::test]
    async fn test_start_watching_suppresses_own_writes() {
        let (_tmp, kanban) = setup_kanban_dir();

        let tag_path = kanban.join("tags/own-write.yaml");
        std::fs::write(&tag_path, "tag_name: Original\n").unwrap();

        let roots = store_roots(&kanban);
        let cache = new_entity_cache(&kanban, &roots);
        let event_count = Arc::new(AtomicUsize::new(0));
        let count_clone = event_count.clone();

        let _watcher = start_watching(kanban.clone(), roots.clone(), cache.clone(), move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        })
        .expect("start_watching should succeed");

        // Allow the watcher to initialize
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Simulate our own write: write the file and immediately update the
        // cache. The debounce (200ms) hasn't fired yet, so when it does, the
        // hash will match and no event will be emitted.
        std::fs::write(&tag_path, "tag_name: Updated\n").unwrap();
        update_cache(&cache, &tag_path);

        // Wait longer than the debounce to confirm no event fires
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        assert_eq!(
            event_count.load(Ordering::SeqCst),
            0,
            "watcher must NOT fire for writes that update the cache first"
        );
    }
}
