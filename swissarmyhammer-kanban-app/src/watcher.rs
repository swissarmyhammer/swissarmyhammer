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

/// Directories within `.kanban/` that contain entity files we care about.
const WATCHED_SUBDIRS: &[&str] = &[
    "tasks",
    "tags",
    "columns",
    "swimlanes",
    "actors",
    "boards",
    "views",
    "attachments",
];

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
}

/// A single field-level change.
#[derive(Debug, Clone, Serialize)]
pub struct FieldChange {
    pub field: String,
    pub value: serde_json::Value,
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
pub fn new_entity_cache(kanban_root: &Path) -> EntityCache {
    let mut map = HashMap::new();
    for subdir in WATCHED_SUBDIRS {
        let dir = kanban_root.join(subdir);
        if dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
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

/// Scan all entity files in a `.kanban` directory, compare against the cache,
/// emit events for anything that changed, and update the cache.
///
/// This is used after our own command execution to produce immediate granular
/// events without waiting for the file watcher's debounce. It also updates
/// the cache so the watcher won't double-fire for these writes.
///
/// Returns the list of events emitted.
pub fn flush_and_emit(kanban_root: &Path, cache: &EntityCache) -> Vec<WatchEvent> {
    let kanban_root = kanban_root
        .canonicalize()
        .unwrap_or_else(|_| kanban_root.to_path_buf());
    let mut events = Vec::new();

    // Collect all current entity file paths on disk
    let mut disk_paths: HashSet<PathBuf> = HashSet::new();
    for subdir in WATCHED_SUBDIRS {
        let dir = kanban_root.join(subdir);
        if dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
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

/// Start watching the `.kanban` directory for external changes.
///
/// Returns a `BoardWatcher` handle — dropping it stops the watcher.
/// Hash comparison happens at debounce time (not event-receive time),
/// so `update_cache` calls made after writing but before the debounce
/// fires will suppress the event.
pub fn start_watching<F>(
    kanban_root: PathBuf,
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

    // Watch each entity subdirectory
    for subdir in WATCHED_SUBDIRS {
        let dir = kanban_root.join(subdir);
        if dir.is_dir() {
            watcher
                .watch(&dir, RecursiveMode::NonRecursive)
                .map_err(|e| format!("Failed to watch {}: {e}", dir.display()))?;
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

                        // Deduplicate by (entity_type, id)
                        let mut seen: HashMap<(String, String), WatchEvent> = HashMap::new();
                        for evt in events {
                            let key = match &evt {
                                WatchEvent::EntityCreated { entity_type, id, .. } |
                                WatchEvent::EntityRemoved { entity_type, id } |
                                WatchEvent::EntityFieldChanged { entity_type, id, .. } => {
                                    (entity_type.clone(), id.clone())
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
        if !is_entity_file(path) {
            continue;
        }
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
        if path_to_entity(&canonical, kanban_root).is_none() {
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
    let (entity_type, id) = path_to_entity(path, kanban_root)?;
    let new_cached = cache_file(path)?;

    let mut map = cache.lock().unwrap();
    match map.get(path) {
        Some(old) if old.hash == new_cached.hash => {
            // Same content — our own write or no real change
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
fn path_to_entity(path: &Path, kanban_root: &Path) -> Option<(String, String)> {
    let stem = path.file_stem()?.to_str()?;

    if path.parent()? == kanban_root {
        return Some((stem.to_string(), stem.to_string()));
    }

    let parent_name = path.parent()?.file_name()?.to_str()?;

    if !WATCHED_SUBDIRS.contains(&parent_name) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    fn setup_kanban_dir() -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let kanban = tmp.path().join(".kanban");
        for subdir in WATCHED_SUBDIRS {
            std::fs::create_dir_all(kanban.join(subdir)).unwrap();
        }
        // Canonicalize to match what the cache and watcher use internally
        let kanban = kanban.canonicalize().unwrap();
        (tmp, kanban)
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

        let cache = new_entity_cache(&kanban);
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

        let cache = new_entity_cache(&kanban);

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

        let cache = new_entity_cache(&kanban);

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

        let cache = new_entity_cache(&kanban);
        let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let _watcher = start_watching(kanban.clone(), cache.clone(), move |evt| {
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

        let cache = new_entity_cache(&kanban);
        let change_count = Arc::new(AtomicUsize::new(0));
        let count_clone = change_count.clone();

        let _watcher = start_watching(kanban.clone(), cache.clone(), move |_| {
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

        let cache = new_entity_cache(&kanban);
        let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let _watcher = start_watching(kanban.clone(), cache.clone(), move |evt| {
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

        let cache = new_entity_cache(&kanban);
        let events: Arc<Mutex<Vec<WatchEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let _watcher = start_watching(kanban.clone(), cache.clone(), move |evt| {
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
        let cache = new_entity_cache(&kanban);
        let change_count = Arc::new(AtomicUsize::new(0));
        let count_clone = change_count.clone();

        let watcher = start_watching(kanban.clone(), cache.clone(), move |_| {
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
        let cache = new_entity_cache(&kanban);

        // Add a file after cache was built
        std::fs::write(kanban.join("tags/new.yaml"), "tag_name: New\n").unwrap();

        let events = flush_and_emit(&kanban, &cache);
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
        let events2 = flush_and_emit(&kanban, &cache);
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
        let cache = new_entity_cache(&kanban);

        // Modify a field
        std::fs::write(
            kanban.join("tags/bug.yaml"),
            "tag_name: Bug\ncolor: \"00ff00\"\n",
        )
        .unwrap();

        let events = flush_and_emit(&kanban, &cache);
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
        let cache = new_entity_cache(&kanban);

        std::fs::remove_file(kanban.join("tags/old.yaml")).unwrap();

        let events = flush_and_emit(&kanban, &cache);
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
        let cache = new_entity_cache(&kanban);

        let events = flush_and_emit(&kanban, &cache);
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
        let cache = new_entity_cache(&kanban);

        // Edit the body to include a #tag reference
        std::fs::write(
            kanban.join("tasks/task1.md"),
            "---\ntitle: My Task\nassignees: []\n---\nBody with #bug tag\n",
        )
        .unwrap();

        let events = flush_and_emit(&kanban, &cache);
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
        let cache = new_entity_cache(&kanban);

        // Check off one item
        std::fs::write(
            kanban.join("tasks/task1.md"),
            "---\ntitle: Checklist\nassignees: []\n---\n- [x] Step 1\n- [ ] Step 2\n",
        )
        .unwrap();

        let events = flush_and_emit(&kanban, &cache);
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

        let cache = new_entity_cache(&kanban);
        let change_count = Arc::new(AtomicUsize::new(0));
        let count_clone = change_count.clone();

        let _watcher = start_watching(kanban.clone(), cache.clone(), move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        })
        .expect("start_watching should succeed");

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Write a file and immediately flush_and_emit (simulates command execution).
        // This updates the cache, so the watcher's debounce should NOT fire.
        std::fs::write(kanban.join("tags/test.yaml"), "tag_name: Updated\n").unwrap();
        let events = flush_and_emit(&kanban, &cache);
        assert_eq!(events.len(), 1); // flush_and_emit returns the event

        tokio::time::sleep(std::time::Duration::from_millis(600)).await;

        assert_eq!(
            change_count.load(Ordering::SeqCst),
            0,
            "watcher should NOT fire because flush_and_emit updated the cache"
        );
    }
}
