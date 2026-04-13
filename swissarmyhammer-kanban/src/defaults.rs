//! Built-in field definitions and entity templates for kanban.
//!
//! Builtin YAML files are embedded from `builtin/definitions/` and
//! `builtin/entities/` at compile time via `include_dir!`. At runtime,
//! these are merged with local overrides from `.kanban/definitions/` and
//! `.kanban/entities/` to produce the full field registry.
//!
//! `KanbanLookup` implements `EntityLookup` for kanban entity stores,
//! enabling reference field validation to prune dangling IDs.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use include_dir::{include_dir, Dir};
use swissarmyhammer_entity::EntityContext;
use swissarmyhammer_fields::{ComputeEngine, EntityLookup, FieldsContext};

use crate::tag_parser;
use crate::task_helpers;

/// Builtin field definition YAML files, embedded at compile time.
///
/// Each builtin field uses a zero-padded sentinel ID (e.g. `00000000000000000000000001`)
/// that sorts before any real ULID. The last two characters encode the builtin field
/// code. See `builtin/definitions/*.yaml` for the full set.
static BUILTIN_DEFINITIONS: Dir = include_dir!("$CARGO_MANIFEST_DIR/builtin/definitions");

/// Builtin entity definition YAML files, embedded at compile time.
static BUILTIN_ENTITIES: Dir = include_dir!("$CARGO_MANIFEST_DIR/builtin/entities");

/// Builtin view definition YAML files, embedded at compile time.
static BUILTIN_VIEWS: Dir = include_dir!("$CARGO_MANIFEST_DIR/builtin/views");

/// Builtin actor entity YAML files, embedded at compile time.
static BUILTIN_ACTORS: Dir = include_dir!("$CARGO_MANIFEST_DIR/builtin/actors");

/// Load builtin field definitions as `(name, yaml_content)` pairs.
pub fn builtin_field_definitions() -> Vec<(&'static str, &'static str)> {
    BUILTIN_DEFINITIONS
        .files()
        .filter_map(|file| {
            let name = file.path().file_stem()?.to_str()?;
            let content = file.contents_utf8()?;
            Some((name, content))
        })
        .collect()
}

/// Load builtin entity definitions as `(name, yaml_content)` pairs.
pub fn builtin_entity_definitions() -> Vec<(&'static str, &'static str)> {
    BUILTIN_ENTITIES
        .files()
        .filter_map(|file| {
            let name = file.path().file_stem()?.to_str()?;
            let content = file.contents_utf8()?;
            Some((name, content))
        })
        .collect()
}

/// Load builtin view definitions as `(name, yaml_content)` pairs.
pub fn builtin_view_definitions() -> Vec<(&'static str, &'static str)> {
    BUILTIN_VIEWS
        .files()
        .filter_map(|file| {
            let name = file.path().file_stem()?.to_str()?;
            let content = file.contents_utf8()?;
            Some((name, content))
        })
        .collect()
}

/// Load builtin actor entity YAML as `(id, yaml_content)` pairs.
///
/// The file stem is the actor ID (e.g. `claude-code.yaml` → `"claude-code"`).
pub fn builtin_actor_entities() -> Vec<(&'static str, &'static str)> {
    BUILTIN_ACTORS
        .files()
        .filter_map(|file| {
            let name = file.path().file_stem()?.to_str()?;
            let content = file.contents_utf8()?;
            Some((name, content))
        })
        .collect()
}

/// Register the parse-body-tags derivation.
///
/// Extracts #tag patterns from the body field, filtered to only include
/// tags that actually exist as tag entities.
fn register_parse_body_tags(engine: &mut ComputeEngine) {
    engine.register_aggregate(
        "parse-body-tags",
        Box::new(|fields, query| {
            let body = fields
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Box::pin(async move {
                let parsed = tag_parser::parse_tags(&body);
                let existing_tags = query("tag").await;
                let known: std::collections::HashSet<&str> = existing_tags
                    .iter()
                    .filter_map(|t| t.get("tag_name").and_then(|v| v.as_str()))
                    .collect();
                let filtered: Vec<serde_json::Value> = parsed
                    .into_iter()
                    .filter(|slug| known.contains(slug.as_str()))
                    .map(serde_json::Value::String)
                    .collect();
                serde_json::Value::Array(filtered)
            })
        }),
    );
}

/// Register the parse-body-progress derivation.
///
/// Parses GFM task lists from body and computes total/completed/percent.
fn register_parse_body_progress(engine: &mut ComputeEngine) {
    engine.register(
        "parse-body-progress",
        Box::new(|fields| {
            let body = fields.get("body").and_then(|v| v.as_str()).unwrap_or("");
            let (total, completed) = task_helpers::parse_checklist_counts(body);
            let percent = if total > 0 {
                (completed as f64 / total as f64 * 100.0).round() as u32
            } else {
                0
            };
            let value = serde_json::json!({
                "total": total,
                "completed": completed,
                "percent": percent,
            });
            Box::pin(async move { value })
        }),
    );
}

/// Register the board-percent-complete derivation.
///
/// Counts done tasks (terminal column) vs total to produce a board-level
/// progress percentage.
fn register_board_percent_complete(engine: &mut ComputeEngine) {
    engine.register_aggregate(
        "board-percent-complete",
        Box::new(|_fields, query| {
            Box::pin(async move {
                let columns = query("column").await;
                let tasks = query("task").await;

                let terminal_id = columns
                    .iter()
                    .max_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0))
                    .and_then(|c| c.get("id").and_then(|v| v.as_str()))
                    .unwrap_or("done");

                let total = tasks.len();
                let done = tasks
                    .iter()
                    .filter(|t| {
                        t.get("position_column")
                            .and_then(|v| v.as_str())
                            .unwrap_or("todo")
                            == terminal_id
                    })
                    .count();
                let percent = if total > 0 {
                    (done as f64 / total as f64 * 100.0).round() as u32
                } else {
                    0
                };

                serde_json::json!({
                    "done": done,
                    "total": total,
                    "percent": percent,
                })
            })
        }),
    );
}

/// Extract the `timestamp` string from a serialized `ChangeEntry` JSON value.
fn changelog_timestamp(entry: &serde_json::Value) -> Option<&str> {
    entry.get("timestamp").and_then(|v| v.as_str())
}

/// Parse a unified diff patch and return the new value from the first `+` line
/// (excluding the `+++` diff header).
fn extract_text_diff_new_value(patch: &str) -> Option<String> {
    patch
        .lines()
        .find(|line| line.starts_with('+') && !line.starts_with("+++"))
        .map(|line| line[1..].trim_end().to_string())
}

/// Extract the new value from a single `FieldChange` JSON value, dispatching on `kind`.
fn field_change_new_value(fc: &serde_json::Value) -> Option<String> {
    match fc.get("kind")?.as_str()? {
        "set" => fc.get("value").and_then(|v| v.as_str()).map(String::from),
        "changed" => fc
            .get("new_value")
            .and_then(|v| v.as_str())
            .map(String::from),
        "text_diff" => extract_text_diff_new_value(fc.get("forward_patch")?.as_str()?),
        _ => None,
    }
}

/// Return true if `change` is a `(field_name, FieldChange)` pair for `position_column`.
fn is_position_column_change(change: &serde_json::Value) -> bool {
    let Some(pair) = change.as_array() else {
        return false;
    };
    pair.len() == 2 && pair[0].as_str() == Some("position_column")
}

/// Extract the new value of `position_column` from a changelog entry's changes.
///
/// Handles three field-change kinds:
/// - `set`: the initial value on entity creation.
/// - `changed`: old/new JSON values (non-string diff path — included for robustness).
/// - `text_diff`: a unified diff patch used for string field updates. The new value
///   is the line starting with `+` (excluding the diff header `+++`).
fn extract_position_column(entry: &serde_json::Value) -> Option<String> {
    let changes = entry.get("changes")?.as_array()?;
    changes
        .iter()
        .filter(|change| is_position_column_change(change))
        .find_map(|change| field_change_new_value(&change.as_array()?[1]))
}

/// Register the derive-created derivation.
///
/// Resolves the entity's creation timestamp by consulting sources in order
/// of decreasing authority:
///
/// 1. First changelog entry with `op: "create"` — the authoritative signal
///    written by the current `StoreHandle` write path.
/// 2. First changelog entry regardless of op — covers histories where the
///    create entry was lost or never written.
/// 3. `_file_created`, an RFC 3339 timestamp derived from the entity file's
///    filesystem metadata (injected by `EntityContext::apply_compute_with_query`
///    when this field declares `depends_on: ["_file_created"]`) — the backstop
///    for tasks dropped into `.kanban/tasks/` by hand or written via the
///    legacy `io::write_entity` path.
/// 4. `Value::Null` — no signal is available.
fn register_derive_created(engine: &mut ComputeEngine) {
    engine.register(
        "derive-created",
        Box::new(|fields| {
            let changelog = fields
                .get("_changelog")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let file_created = fields
                .get("_file_created")
                .and_then(|v| v.as_str())
                .map(String::from);
            Box::pin(async move {
                let ts = changelog
                    .iter()
                    .find(|e| e.get("op").and_then(|v| v.as_str()) == Some("create"))
                    .and_then(changelog_timestamp)
                    .map(String::from)
                    .or_else(|| {
                        changelog
                            .first()
                            .and_then(changelog_timestamp)
                            .map(String::from)
                    })
                    .or(file_created);
                match ts {
                    Some(s) => serde_json::Value::String(s),
                    None => serde_json::Value::Null,
                }
            })
        }),
    );
}

/// Register the derive-updated derivation.
///
/// Returns the timestamp of the last changelog entry.
/// Returns null for an empty changelog.
fn register_derive_updated(engine: &mut ComputeEngine) {
    engine.register(
        "derive-updated",
        Box::new(|fields| {
            let changelog = fields
                .get("_changelog")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            Box::pin(async move {
                match changelog.last().and_then(changelog_timestamp) {
                    Some(s) => serde_json::Value::String(s.to_string()),
                    None => serde_json::Value::Null,
                }
            })
        }),
    );
}

/// Read the `_changelog` injected field as an owned array of JSON values.
fn take_changelog(fields: &HashMap<String, serde_json::Value>) -> Vec<serde_json::Value> {
    fields
        .get("_changelog")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

/// Pick a column id by applying `order_key` across the column entities.
///
/// Used to locate the first (lowest-order) or terminal (highest-order) column.
/// Returns `fallback` if the column list is empty or lacks an id.
fn pick_column_id<F, K>(
    columns: &[HashMap<String, serde_json::Value>],
    order_key: F,
    fallback: &str,
) -> String
where
    F: Fn(&HashMap<String, serde_json::Value>) -> K,
    K: Ord,
{
    columns
        .iter()
        .max_by_key(|c| order_key(c))
        .and_then(|c| c.get("id").and_then(|v| v.as_str()))
        .unwrap_or(fallback)
        .to_string()
}

/// Wrap an `Option<String>` timestamp into a `serde_json::Value`.
fn timestamp_value(ts: Option<String>) -> serde_json::Value {
    ts.map(serde_json::Value::String)
        .unwrap_or(serde_json::Value::Null)
}

/// Find the timestamp of the first changelog entry whose `position_column` change
/// moved the task out of the first (lowest-order) column.
fn find_started_timestamp(
    changelog: &[serde_json::Value],
    first_column_id: &str,
) -> Option<String> {
    changelog
        .iter()
        .find(|entry| {
            extract_position_column(entry)
                .map(|col| col != first_column_id)
                .unwrap_or(false)
        })
        .and_then(|entry| changelog_timestamp(entry).map(String::from))
}

/// Determine whether the task is currently in the terminal column by examining
/// the most recent `position_column` change in the changelog. If that change
/// moved to the terminal column, return its timestamp; otherwise return None.
fn find_completed_timestamp(changelog: &[serde_json::Value], terminal_id: &str) -> Option<String> {
    let last_move = changelog
        .iter()
        .rev()
        .find_map(|entry| extract_position_column(entry).map(|col| (entry, col)))?;
    let (entry, col) = last_move;
    if col == terminal_id {
        changelog_timestamp(entry).map(String::from)
    } else {
        None
    }
}

/// Register the derive-started derivation.
///
/// Scans the changelog for the first entry where `position_column` changed
/// to a column that is not the first (lowest-order) column. Uses
/// `register_aggregate` to query column entities for ordering.
/// Returns null if the task was never moved out of the first column.
fn register_derive_started(engine: &mut ComputeEngine) {
    engine.register_aggregate(
        "derive-started",
        Box::new(|fields, query| {
            let changelog = take_changelog(fields);
            Box::pin(async move {
                let columns = query("column").await;
                let first_column_id = pick_column_id(
                    &columns,
                    |c| {
                        std::cmp::Reverse(
                            c.get("order").and_then(|v| v.as_u64()).unwrap_or(u64::MAX),
                        )
                    },
                    "todo",
                );
                timestamp_value(find_started_timestamp(&changelog, &first_column_id))
            })
        }),
    );
}

/// Register the derive-completed derivation.
///
/// Scans the changelog to determine whether the task is currently in the
/// terminal (highest-order) column. If the last `position_column` change
/// set the value to the terminal column, returns that entry's timestamp.
/// If the task was later moved out of done, returns null.
/// Uses `register_aggregate` to query column entities for ordering.
fn register_derive_completed(engine: &mut ComputeEngine) {
    engine.register_aggregate(
        "derive-completed",
        Box::new(|fields, query| {
            let changelog = take_changelog(fields);
            Box::pin(async move {
                let columns = query("column").await;
                let terminal_id = pick_column_id(
                    &columns,
                    |c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0),
                    "done",
                );
                timestamp_value(find_completed_timestamp(&changelog, &terminal_id))
            })
        }),
    );
}

/// Parse a timestamp from `fields` by name.
///
/// Accepts either RFC 3339 datetimes (e.g. `2026-04-12T10:23:00Z`) or bare
/// calendar dates (`YYYY-MM-DD` — used by `due` and `scheduled`). Bare dates
/// are interpreted as midnight UTC of that day. Returns `None` if the field
/// is missing, null, not a string, or not a parseable date/datetime.
fn parse_status_date_input(
    fields: &HashMap<String, serde_json::Value>,
    name: &str,
) -> Option<chrono::DateTime<chrono::Utc>> {
    let raw = fields.get(name)?.as_str()?;
    if raw.is_empty() {
        return None;
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return Some(dt.with_timezone(&chrono::Utc));
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        let naive = date.and_hms_opt(0, 0, 0)?;
        return Some(chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            naive,
            chrono::Utc,
        ));
    }
    None
}

/// Render a `DateTime<Utc>` back as an RFC 3339 string for the output payload.
fn format_status_date_timestamp(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Look up a raw timestamp string for `name` in `fields`, preserving the original
/// serialization (date vs datetime). Falls back to formatting `parsed` if the raw
/// string is unavailable — keeps output consistent regardless of input shape.
fn status_date_raw_string(
    fields: &HashMap<String, serde_json::Value>,
    name: &str,
    parsed: chrono::DateTime<chrono::Utc>,
) -> String {
    fields
        .get(name)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| format_status_date_timestamp(parsed))
}

/// Compute the tagged `status_date` payload for a task given its resolved date
/// fields and a reference "now" instant.
///
/// Applies the priority ladder (first match wins):
/// 1. `completed` set → `kind: "completed"`
/// 2. `due` set AND in the past → `kind: "overdue"`
/// 3. `started` set → `kind: "started"`
/// 4. `scheduled` set AND in the future → `kind: "scheduled"`
/// 5. `created` set → `kind: "created"` (fallback)
///
/// The `timestamp` in the result preserves the original input string form
/// (date or datetime). Returns `Value::Null` if no ladder rule matches.
///
/// Pure — accepts `now` explicitly so it can be tested with a fixed instant.
fn compute_status_date(
    fields: &HashMap<String, serde_json::Value>,
    now: chrono::DateTime<chrono::Utc>,
) -> serde_json::Value {
    if let Some(ts) = parse_status_date_input(fields, "completed") {
        return serde_json::json!({
            "kind": "completed",
            "timestamp": status_date_raw_string(fields, "completed", ts),
        });
    }
    if let Some(ts) = parse_status_date_input(fields, "due") {
        if ts < now {
            return serde_json::json!({
                "kind": "overdue",
                "timestamp": status_date_raw_string(fields, "due", ts),
            });
        }
    }
    if let Some(ts) = parse_status_date_input(fields, "started") {
        return serde_json::json!({
            "kind": "started",
            "timestamp": status_date_raw_string(fields, "started", ts),
        });
    }
    if let Some(ts) = parse_status_date_input(fields, "scheduled") {
        if ts > now {
            return serde_json::json!({
                "kind": "scheduled",
                "timestamp": status_date_raw_string(fields, "scheduled", ts),
            });
        }
    }
    if let Some(ts) = parse_status_date_input(fields, "created") {
        return serde_json::json!({
            "kind": "created",
            "timestamp": status_date_raw_string(fields, "created", ts),
        });
    }
    serde_json::Value::Null
}

/// Register the derive-status-date derivation.
///
/// Reads already-resolved `completed`, `started`, `due`, `scheduled`, and
/// `created` values from the entity's field map (they appear earlier in
/// task.yaml's field list, so the enrichment pipeline has already derived
/// them by the time this runs) and produces a tagged `{ kind, timestamp }`
/// payload, or `null` if no priority ladder rule matches.
fn register_derive_status_date(engine: &mut ComputeEngine) {
    engine.register(
        "derive-status-date",
        Box::new(|fields| {
            let value = compute_status_date(fields, chrono::Utc::now());
            Box::pin(async move { value })
        }),
    );
}

/// Build a ComputeEngine with all kanban derivation functions registered.
pub fn kanban_compute_engine() -> ComputeEngine {
    let mut engine = ComputeEngine::new();

    register_parse_body_tags(&mut engine);
    register_parse_body_progress(&mut engine);
    register_board_percent_complete(&mut engine);

    // compute-virtual-tags: stub — returns empty array.
    // Populated by the enrichment pipeline in a later card.
    engine.register(
        "compute-virtual-tags",
        Box::new(|_fields| Box::pin(async { serde_json::Value::Array(vec![]) })),
    );

    // compute-filter-tags: stub — returns empty array.
    // Will compute tags ∪ virtual_tags once the enrichment pipeline lands.
    engine.register(
        "compute-filter-tags",
        Box::new(|_fields| Box::pin(async { serde_json::Value::Array(vec![]) })),
    );

    register_derive_created(&mut engine);
    register_derive_updated(&mut engine);
    register_derive_started(&mut engine);
    register_derive_completed(&mut engine);
    register_derive_status_date(&mut engine);

    engine
}

/// Entity types supported by kanban lookup.
const KNOWN_ENTITY_TYPES: &[&str] = &["task", "tag", "actor", "column", "attachment", "project"];

/// Entity lookup backed by kanban file storage.
///
/// Uses a bare `EntityContext` (no engines) to avoid circular dependency:
/// engines → lookup → engines. Validation lookups use raw I/O only.
pub struct KanbanLookup {
    root: PathBuf,
    fields: Arc<FieldsContext>,
}

impl KanbanLookup {
    /// Create a lookup from a root path and fields context.
    pub fn new(root: impl Into<PathBuf>, fields: Arc<FieldsContext>) -> Self {
        Self {
            root: root.into(),
            fields,
        }
    }

    /// Build a bare EntityContext (no engines) for raw I/O.
    fn bare_entity_context(&self) -> EntityContext {
        EntityContext::new(&self.root, Arc::clone(&self.fields))
    }
}

#[async_trait]
impl EntityLookup for KanbanLookup {
    async fn get(&self, entity_type: &str, id: &str) -> Option<serde_json::Value> {
        if !KNOWN_ENTITY_TYPES.contains(&entity_type) {
            return None;
        }
        let ectx = self.bare_entity_context();
        ectx.read(entity_type, id).await.ok().map(|e| e.to_json())
    }

    async fn list(&self, entity_type: &str) -> Vec<serde_json::Value> {
        if !KNOWN_ENTITY_TYPES.contains(&entity_type) {
            return Vec::new();
        }
        let ectx = self.bare_entity_context();
        ectx.list(entity_type)
            .await
            .unwrap_or_default()
            .iter()
            .map(|e| e.to_json())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use swissarmyhammer_fields::{EntityDef, FieldDef, FieldName};

    #[test]
    fn builtin_view_definitions_load() {
        let defs = builtin_view_definitions();
        assert!(
            !defs.is_empty(),
            "expected at least 1 builtin view definition"
        );
    }

    #[test]
    fn builtin_views_parse_as_view_def() {
        for (name, yaml) in builtin_view_definitions() {
            let result: Result<swissarmyhammer_views::ViewDef, _> = serde_yaml_ng::from_str(yaml);
            assert!(
                result.is_ok(),
                "Failed to parse view '{}': {}",
                name,
                result.unwrap_err()
            );
        }
    }

    #[test]
    fn builtin_board_view_exists() {
        let defs = builtin_view_definitions();
        let (_, yaml) = defs.iter().find(|(n, _)| *n == "board").unwrap();
        let view: swissarmyhammer_views::ViewDef = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(view.name, "Board");
        assert_eq!(view.kind, swissarmyhammer_views::ViewKind::Board);
        assert!(view.entity_type.as_deref() == Some("task"));
        assert!(!view.card_fields.is_empty());
        assert!(!view.commands.is_empty());
    }

    #[test]
    fn builtin_field_definitions_load() {
        let defs = builtin_field_definitions();
        assert_eq!(defs.len(), 30, "expected 30 builtin field definitions");
    }

    #[test]
    fn builtin_entity_definitions_load() {
        let defs = builtin_entity_definitions();
        assert_eq!(defs.len(), 7, "expected 7 builtin entity definitions");
    }

    #[test]
    fn builtin_fields_parse_as_field_def() {
        for (name, yaml) in builtin_field_definitions() {
            let result: Result<FieldDef, _> = serde_yaml_ng::from_str(yaml);
            assert!(
                result.is_ok(),
                "Failed to parse field '{}': {}",
                name,
                result.unwrap_err()
            );
        }
    }

    #[test]
    fn builtin_entities_parse_as_entity_def() {
        for (name, yaml) in builtin_entity_definitions() {
            let result: Result<EntityDef, _> = serde_yaml_ng::from_str(yaml);
            assert!(
                result.is_ok(),
                "Failed to parse entity '{}': {}",
                name,
                result.unwrap_err()
            );
        }
    }

    #[test]
    fn builtin_field_names_are_unique() {
        let defs = builtin_field_definitions();
        let mut names: Vec<_> = defs
            .iter()
            .map(|(_, yaml)| {
                let def: FieldDef = serde_yaml_ng::from_str(yaml).unwrap();
                def.name
            })
            .collect();
        let orig_len = names.len();
        names.sort();
        names.dedup();
        assert_eq!(orig_len, names.len(), "duplicate field names in builtins");
    }

    #[test]
    fn builtin_field_ulids_are_unique() {
        let defs = builtin_field_definitions();
        let mut ids: Vec<_> = defs
            .iter()
            .map(|(_, yaml)| {
                let def: FieldDef = serde_yaml_ng::from_str(yaml).unwrap();
                def.id
            })
            .collect();
        let orig_len = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(orig_len, ids.len(), "duplicate ULIDs in builtins");
    }

    #[test]
    fn builtin_task_entity_has_expected_fields() {
        let defs = builtin_entity_definitions();
        let (_, yaml) = defs.iter().find(|(n, _)| *n == "task").unwrap();
        let entity: EntityDef = serde_yaml_ng::from_str(yaml).unwrap();

        assert_eq!(entity.name, "task");
        assert_eq!(entity.body_field, Some("body".into()));
        assert_eq!(entity.mention_prefix, Some("^".to_string()));
        assert_eq!(entity.mention_display_field, Some("title".into()));
        assert!(entity.fields.iter().any(|f| f == "title"));
        assert!(entity.fields.iter().any(|f| f == "position_column"));
        assert!(entity.fields.iter().any(|f| f == "position_ordinal"));
        assert!(entity.fields.iter().any(|f| f == "attachments"));
        assert!(entity.fields.iter().any(|f| f == "progress"));
        assert!(entity.fields.iter().any(|f| f == "project"));
        // Date fields
        assert!(entity.fields.iter().any(|f| f == "due"));
        assert!(entity.fields.iter().any(|f| f == "scheduled"));
        assert!(entity.fields.iter().any(|f| f == "created"));
        assert!(entity.fields.iter().any(|f| f == "updated"));
        assert!(entity.fields.iter().any(|f| f == "started"));
        assert!(entity.fields.iter().any(|f| f == "completed"));
    }

    #[test]
    fn builtin_board_entity_exists() {
        let defs = builtin_entity_definitions();
        let (_, yaml) = defs.iter().find(|(n, _)| *n == "board").unwrap();
        let entity: EntityDef = serde_yaml_ng::from_str(yaml).unwrap();

        assert_eq!(entity.name, "board");
        assert!(entity.fields.iter().any(|f| f == "name"));
        assert!(entity.fields.iter().any(|f| f == "description"));
    }

    #[test]
    fn builtin_entity_fields_reference_existing_field_defs() {
        let field_defs = builtin_field_definitions();
        let field_names: Vec<FieldName> = field_defs
            .iter()
            .map(|(_, yaml)| {
                let def: FieldDef = serde_yaml_ng::from_str(yaml).unwrap();
                def.name
            })
            .collect();

        let entity_defs = builtin_entity_definitions();
        for (ename, eyaml) in &entity_defs {
            let entity: EntityDef = serde_yaml_ng::from_str(eyaml).unwrap();
            for field_ref in &entity.fields {
                assert!(
                    field_names.contains(field_ref),
                    "Entity '{}' references field '{}' which has no builtin definition",
                    ename,
                    field_ref
                );
            }
        }
    }

    #[test]
    fn from_yaml_sources_builds_valid_context() {
        let defs = builtin_field_definitions();
        let entities = builtin_entity_definitions();

        let ctx = swissarmyhammer_fields::FieldsContext::from_yaml_sources(
            std::path::PathBuf::from("/tmp/test"),
            &defs,
            &entities,
        )
        .unwrap();

        assert_eq!(ctx.all_fields().len(), 30);
        assert_eq!(ctx.all_entities().len(), 7);
        assert!(ctx.get_field_by_name("title").is_some());
        assert!(ctx.get_entity("task").is_some());
        assert_eq!(ctx.fields_for_entity("task").len(), 19);
    }

    #[test]
    fn builtin_attachment_field_round_trips_through_yaml() {
        let defs = builtin_field_definitions();
        let entities = builtin_entity_definitions();

        let ctx = swissarmyhammer_fields::FieldsContext::from_yaml_sources(
            std::path::PathBuf::from("/tmp/test"),
            &defs,
            &entities,
        )
        .unwrap();

        let field = ctx
            .get_field_by_name("attachments")
            .expect("builtin 'attachments' field should exist in FieldsContext");

        match &field.type_ {
            swissarmyhammer_fields::FieldType::Attachment {
                max_bytes,
                multiple,
            } => {
                assert!(multiple, "attachments field should have multiple: true");
                assert_eq!(
                    *max_bytes, 104_857_600,
                    "attachments max_bytes should be 100 MiB"
                );
            }
            other => panic!("expected FieldType::Attachment, got {:?}", other),
        }
    }

    #[test]
    fn kanban_compute_engine_registers_all_derivations() {
        let engine = kanban_compute_engine();
        assert!(engine.has("parse-body-tags"));
        assert!(engine.has("parse-body-progress"));
        assert!(engine.has("derive-status-date"));
    }

    /// Helper: build a query function that returns known tags.
    fn tag_query(
        tag_names: Vec<&'static str>,
    ) -> std::sync::Arc<swissarmyhammer_fields::EntityQueryFn> {
        std::sync::Arc::new(Box::new(move |entity_type: &str| {
            let names = tag_names.clone();
            let entity_type = entity_type.to_string();
            Box::pin(async move {
                if entity_type == "tag" {
                    names
                        .iter()
                        .map(|n| {
                            let mut m = HashMap::new();
                            m.insert(
                                "tag_name".to_string(),
                                serde_json::Value::String(n.to_string()),
                            );
                            m
                        })
                        .collect()
                } else {
                    Vec::new()
                }
            })
        }))
    }

    #[tokio::test]
    async fn parse_body_tags_derivation() {
        let engine = kanban_compute_engine();
        let field = swissarmyhammer_fields::FieldDef {
            id: swissarmyhammer_fields::FieldDefId::new(),
            name: "tags".into(),
            description: None,
            type_: swissarmyhammer_fields::FieldType::Computed {
                derive: "parse-body-tags".to_string(),
                depends_on: vec![],
                entity: None,
                commit_display_names: false,
            },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
            groupable: None,
        };

        let mut fields = HashMap::new();
        fields.insert(
            "body".to_string(),
            serde_json::json!("Fix the #bug in #login module"),
        );

        let query = tag_query(vec!["bug", "login"]);
        let result = engine.derive(&field, &fields, Some(&query)).await.unwrap();
        let tags: Vec<String> = serde_json::from_value(result).unwrap();
        assert_eq!(tags, vec!["bug", "login"]);
    }

    #[tokio::test]
    async fn parse_body_tags_filters_nonexistent() {
        let engine = kanban_compute_engine();
        let field = swissarmyhammer_fields::FieldDef {
            id: swissarmyhammer_fields::FieldDefId::new(),
            name: "tags".into(),
            description: None,
            type_: swissarmyhammer_fields::FieldType::Computed {
                derive: "parse-body-tags".to_string(),
                depends_on: vec![],
                entity: None,
                commit_display_names: false,
            },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
            groupable: None,
        };

        let mut fields = HashMap::new();
        fields.insert(
            "body".to_string(),
            serde_json::json!("Fix #bug, and #tag, not real #valid here"),
        );

        // "bug," and "tag," are parsed as-is (with comma) — neither matches "bug" or "valid".
        // Only #valid (followed by space) parses cleanly and matches the known tag.
        let query = tag_query(vec!["valid"]);
        let result = engine.derive(&field, &fields, Some(&query)).await.unwrap();
        let tags: Vec<String> = serde_json::from_value(result).unwrap();
        assert_eq!(tags, vec!["valid"]);
    }

    #[tokio::test]
    async fn parse_body_progress_derivation() {
        let engine = kanban_compute_engine();
        let field = swissarmyhammer_fields::FieldDef {
            id: swissarmyhammer_fields::FieldDefId::new(),
            name: "progress".into(),
            description: None,
            type_: swissarmyhammer_fields::FieldType::Computed {
                derive: "parse-body-progress".to_string(),
                depends_on: vec![],
                entity: None,
                commit_display_names: false,
            },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
            groupable: None,
        };

        let mut fields = HashMap::new();
        fields.insert(
            "body".to_string(),
            serde_json::json!("Tasks:\n- [x] First\n- [ ] Second\n- [x] Third\n- [ ] Fourth"),
        );

        let result = engine.derive(&field, &fields, None).await.unwrap();
        assert_eq!(result["total"], 4);
        assert_eq!(result["completed"], 2);
        assert_eq!(result["percent"], 50);
    }

    // parse_checklist_counts tests live in task_helpers module

    #[test]
    fn all_builtin_computed_fields_have_registered_derivations() {
        let engine = kanban_compute_engine();
        let defs = builtin_field_definitions();

        for (filename, yaml) in &defs {
            let field: swissarmyhammer_fields::FieldDef = serde_yaml_ng::from_str(yaml).unwrap();
            if let swissarmyhammer_fields::FieldType::Computed { derive, .. } = &field.type_ {
                assert!(
                    engine.has(derive),
                    "Builtin computed field '{}' (file: {}) references derive '{}' which is not registered in kanban_compute_engine()",
                    field.name, filename, derive
                );
            }
        }
    }

    // ---- Date derivation tests ----

    /// Build a mock changelog entry as JSON with the given op, timestamp, and
    /// optional position_column change.
    fn mock_changelog_entry(
        op: &str,
        timestamp: &str,
        position_change: Option<(&str, &str)>,
    ) -> serde_json::Value {
        let mut changes = vec![serde_json::json!(["title", {"kind": "set", "value": "Test"}])];
        if let Some((kind, value_or_patch)) = position_change {
            match kind {
                "set" => {
                    changes.push(
                        serde_json::json!(["position_column", {"kind": "set", "value": value_or_patch}]),
                    );
                }
                "text_diff" => {
                    // value_or_patch is "old->new", parse it
                    let parts: Vec<&str> = value_or_patch.splitn(2, "->").collect();
                    let (old, new) = (parts[0], parts[1]);
                    let forward = format!(
                        "--- original\n+++ modified\n@@ -1 +1 @@\n-{}\n\\ No newline at end of file\n+{}\n\\ No newline at end of file\n",
                        old, new
                    );
                    let reverse = format!(
                        "--- original\n+++ modified\n@@ -1 +1 @@\n-{}\n\\ No newline at end of file\n+{}\n\\ No newline at end of file\n",
                        new, old
                    );
                    changes.push(serde_json::json!(["position_column", {
                        "kind": "text_diff",
                        "forward_patch": forward,
                        "reverse_patch": reverse,
                    }]));
                }
                "changed" => {
                    let parts: Vec<&str> = value_or_patch.splitn(2, "->").collect();
                    changes.push(serde_json::json!(["position_column", {
                        "kind": "changed",
                        "old_value": parts[0],
                        "new_value": parts[1],
                    }]));
                }
                _ => {}
            }
        }
        serde_json::json!({
            "id": "01TEST000000000000000000XX",
            "timestamp": timestamp,
            "entity_type": "task",
            "entity_id": "01TASK0000000000000000000X",
            "op": op,
            "changes": changes,
        })
    }

    /// Build a query function that returns standard kanban columns.
    fn column_query(
        columns: Vec<(&'static str, u64)>,
    ) -> std::sync::Arc<swissarmyhammer_fields::EntityQueryFn> {
        std::sync::Arc::new(Box::new(move |entity_type: &str| {
            let cols = columns.clone();
            let entity_type = entity_type.to_string();
            Box::pin(async move {
                if entity_type == "column" {
                    cols.iter()
                        .map(|(id, order)| {
                            let mut m = HashMap::new();
                            m.insert("id".to_string(), serde_json::json!(id));
                            m.insert("order".to_string(), serde_json::json!(order));
                            m
                        })
                        .collect()
                } else {
                    Vec::new()
                }
            })
        }))
    }

    fn make_date_field(derive: &str) -> swissarmyhammer_fields::FieldDef {
        swissarmyhammer_fields::FieldDef {
            id: swissarmyhammer_fields::FieldDefId::new(),
            name: derive.replace("derive-", "").into(),
            description: None,
            type_: swissarmyhammer_fields::FieldType::Computed {
                derive: derive.to_string(),
                depends_on: vec!["_changelog".to_string()],
                entity: None,
                commit_display_names: false,
            },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
            groupable: None,
        }
    }

    #[tokio::test]
    async fn derive_created_returns_first_create_timestamp() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-created");
        let mut fields = HashMap::new();
        fields.insert(
            "_changelog".to_string(),
            serde_json::json!([
                mock_changelog_entry("create", "2026-01-01T10:00:00Z", Some(("set", "todo"))),
                mock_changelog_entry("update", "2026-01-02T10:00:00Z", None),
            ]),
        );
        let result = engine.derive(&field, &fields, None).await.unwrap();
        assert_eq!(result, serde_json::json!("2026-01-01T10:00:00Z"));
    }

    #[tokio::test]
    async fn derive_created_falls_back_to_first_entry() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-created");
        let mut fields = HashMap::new();
        // No entry has op "create"
        fields.insert(
            "_changelog".to_string(),
            serde_json::json!([
                mock_changelog_entry("update", "2026-02-01T10:00:00Z", None),
                mock_changelog_entry("update", "2026-02-02T10:00:00Z", None),
            ]),
        );
        let result = engine.derive(&field, &fields, None).await.unwrap();
        assert_eq!(result, serde_json::json!("2026-02-01T10:00:00Z"));
    }

    #[tokio::test]
    async fn derive_created_empty_changelog_returns_null() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-created");
        let mut fields = HashMap::new();
        fields.insert("_changelog".to_string(), serde_json::json!([]));
        let result = engine.derive(&field, &fields, None).await.unwrap();
        assert_eq!(result, serde_json::Value::Null);
    }

    #[tokio::test]
    async fn derive_created_falls_back_to_file_created_when_changelog_empty() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-created");
        let mut fields = HashMap::new();
        fields.insert("_changelog".to_string(), serde_json::json!([]));
        fields.insert(
            "_file_created".to_string(),
            serde_json::json!("2026-04-01T12:00:00Z"),
        );
        let result = engine.derive(&field, &fields, None).await.unwrap();
        assert_eq!(result, serde_json::json!("2026-04-01T12:00:00Z"));
    }

    #[tokio::test]
    async fn derive_created_prefers_changelog_over_file_created() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-created");
        let mut fields = HashMap::new();
        fields.insert(
            "_changelog".to_string(),
            serde_json::json!([mock_changelog_entry(
                "create",
                "2026-01-01T10:00:00Z",
                Some(("set", "todo"))
            )]),
        );
        fields.insert(
            "_file_created".to_string(),
            serde_json::json!("2026-04-01T12:00:00Z"),
        );
        let result = engine.derive(&field, &fields, None).await.unwrap();
        assert_eq!(result, serde_json::json!("2026-01-01T10:00:00Z"));
    }

    #[tokio::test]
    async fn derive_updated_returns_last_timestamp() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-updated");
        let mut fields = HashMap::new();
        fields.insert(
            "_changelog".to_string(),
            serde_json::json!([
                mock_changelog_entry("create", "2026-01-01T10:00:00Z", None),
                mock_changelog_entry("update", "2026-01-05T14:30:00Z", None),
                mock_changelog_entry("update", "2026-01-10T09:00:00Z", None),
            ]),
        );
        let result = engine.derive(&field, &fields, None).await.unwrap();
        assert_eq!(result, serde_json::json!("2026-01-10T09:00:00Z"));
    }

    #[tokio::test]
    async fn derive_updated_empty_changelog_returns_null() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-updated");
        let mut fields = HashMap::new();
        fields.insert("_changelog".to_string(), serde_json::json!([]));
        let result = engine.derive(&field, &fields, None).await.unwrap();
        assert_eq!(result, serde_json::Value::Null);
    }

    #[tokio::test]
    async fn derive_started_todo_to_doing() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-started");
        let query = column_query(vec![("todo", 0), ("doing", 1), ("done", 2)]);
        let mut fields = HashMap::new();
        fields.insert(
            "_changelog".to_string(),
            serde_json::json!([
                mock_changelog_entry("create", "2026-01-01T10:00:00Z", Some(("set", "todo"))),
                mock_changelog_entry(
                    "update",
                    "2026-01-05T14:00:00Z",
                    Some(("text_diff", "todo->doing"))
                ),
            ]),
        );
        let result = engine.derive(&field, &fields, Some(&query)).await.unwrap();
        assert_eq!(result, serde_json::json!("2026-01-05T14:00:00Z"));
    }

    #[tokio::test]
    async fn derive_started_never_moved_returns_null() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-started");
        let query = column_query(vec![("todo", 0), ("doing", 1), ("done", 2)]);
        let mut fields = HashMap::new();
        fields.insert(
            "_changelog".to_string(),
            serde_json::json!([mock_changelog_entry(
                "create",
                "2026-01-01T10:00:00Z",
                Some(("set", "todo"))
            ),]),
        );
        let result = engine.derive(&field, &fields, Some(&query)).await.unwrap();
        assert_eq!(result, serde_json::Value::Null);
    }

    #[tokio::test]
    async fn derive_started_empty_changelog_returns_null() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-started");
        let query = column_query(vec![("todo", 0), ("doing", 1), ("done", 2)]);
        let mut fields = HashMap::new();
        fields.insert("_changelog".to_string(), serde_json::json!([]));
        let result = engine.derive(&field, &fields, Some(&query)).await.unwrap();
        assert_eq!(result, serde_json::Value::Null);
    }

    #[tokio::test]
    async fn derive_completed_doing_to_done() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-completed");
        let query = column_query(vec![("todo", 0), ("doing", 1), ("done", 2)]);
        let mut fields = HashMap::new();
        fields.insert(
            "_changelog".to_string(),
            serde_json::json!([
                mock_changelog_entry("create", "2026-01-01T10:00:00Z", Some(("set", "todo"))),
                mock_changelog_entry(
                    "update",
                    "2026-01-05T14:00:00Z",
                    Some(("text_diff", "todo->doing"))
                ),
                mock_changelog_entry(
                    "update",
                    "2026-01-10T16:00:00Z",
                    Some(("text_diff", "doing->done"))
                ),
            ]),
        );
        let result = engine.derive(&field, &fields, Some(&query)).await.unwrap();
        assert_eq!(result, serde_json::json!("2026-01-10T16:00:00Z"));
    }

    #[tokio::test]
    async fn derive_completed_reopened_returns_null() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-completed");
        let query = column_query(vec![("todo", 0), ("doing", 1), ("done", 2)]);
        let mut fields = HashMap::new();
        fields.insert(
            "_changelog".to_string(),
            serde_json::json!([
                mock_changelog_entry("create", "2026-01-01T10:00:00Z", Some(("set", "todo"))),
                mock_changelog_entry(
                    "update",
                    "2026-01-05T14:00:00Z",
                    Some(("text_diff", "todo->doing"))
                ),
                mock_changelog_entry(
                    "update",
                    "2026-01-10T16:00:00Z",
                    Some(("text_diff", "doing->done"))
                ),
                mock_changelog_entry(
                    "update",
                    "2026-01-12T09:00:00Z",
                    Some(("text_diff", "done->doing"))
                ),
            ]),
        );
        let result = engine.derive(&field, &fields, Some(&query)).await.unwrap();
        assert_eq!(result, serde_json::Value::Null);
    }

    #[tokio::test]
    async fn derive_completed_bounce_returns_last_done_timestamp() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-completed");
        let query = column_query(vec![("todo", 0), ("doing", 1), ("done", 2)]);
        let mut fields = HashMap::new();
        fields.insert(
            "_changelog".to_string(),
            serde_json::json!([
                mock_changelog_entry("create", "2026-01-01T10:00:00Z", Some(("set", "todo"))),
                mock_changelog_entry(
                    "update",
                    "2026-01-05T14:00:00Z",
                    Some(("text_diff", "todo->doing"))
                ),
                mock_changelog_entry(
                    "update",
                    "2026-01-10T16:00:00Z",
                    Some(("text_diff", "doing->done"))
                ),
                mock_changelog_entry(
                    "update",
                    "2026-01-12T09:00:00Z",
                    Some(("text_diff", "done->doing"))
                ),
                mock_changelog_entry(
                    "update",
                    "2026-01-15T11:00:00Z",
                    Some(("text_diff", "doing->done"))
                ),
            ]),
        );
        let result = engine.derive(&field, &fields, Some(&query)).await.unwrap();
        assert_eq!(result, serde_json::json!("2026-01-15T11:00:00Z"));
    }

    #[tokio::test]
    async fn derive_completed_empty_changelog_returns_null() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-completed");
        let query = column_query(vec![("todo", 0), ("doing", 1), ("done", 2)]);
        let mut fields = HashMap::new();
        fields.insert("_changelog".to_string(), serde_json::json!([]));
        let result = engine.derive(&field, &fields, Some(&query)).await.unwrap();
        assert_eq!(result, serde_json::Value::Null);
    }

    #[tokio::test]
    async fn derive_started_with_changed_kind() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-started");
        let query = column_query(vec![("todo", 0), ("doing", 1), ("done", 2)]);
        let mut fields = HashMap::new();
        // Test with "changed" kind (non-string diff path)
        fields.insert(
            "_changelog".to_string(),
            serde_json::json!([
                mock_changelog_entry("create", "2026-01-01T10:00:00Z", Some(("set", "todo"))),
                mock_changelog_entry(
                    "update",
                    "2026-01-05T14:00:00Z",
                    Some(("changed", "todo->doing"))
                ),
            ]),
        );
        let result = engine.derive(&field, &fields, Some(&query)).await.unwrap();
        assert_eq!(result, serde_json::json!("2026-01-05T14:00:00Z"));
    }

    #[tokio::test]
    async fn derive_created_no_changelog_field_returns_null() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-created");
        let fields = HashMap::new(); // No _changelog at all
        let result = engine.derive(&field, &fields, None).await.unwrap();
        assert_eq!(result, serde_json::Value::Null);
    }

    #[tokio::test]
    async fn derive_updated_no_changelog_field_returns_null() {
        let engine = kanban_compute_engine();
        let field = make_date_field("derive-updated");
        let fields = HashMap::new(); // No _changelog at all
        let result = engine.derive(&field, &fields, None).await.unwrap();
        assert_eq!(result, serde_json::Value::Null);
    }

    #[tokio::test]
    async fn parse_body_progress_empty_body() {
        let engine = kanban_compute_engine();
        let field = swissarmyhammer_fields::FieldDef {
            id: swissarmyhammer_fields::FieldDefId::new(),
            name: "progress".into(),
            description: None,
            type_: swissarmyhammer_fields::FieldType::Computed {
                derive: "parse-body-progress".to_string(),
                depends_on: vec![],
                entity: None,
                commit_display_names: false,
            },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
            groupable: None,
        };

        let fields = HashMap::new(); // No body field

        let result = engine.derive(&field, &fields, None).await.unwrap();
        assert_eq!(result["total"], 0);
        assert_eq!(result["completed"], 0);
        assert_eq!(result["percent"], 0);
    }

    // ---- status_date derivation tests ----

    /// Fixed "now" used across `derive_status_date_*` tests for deterministic
    /// past/future comparisons.
    fn status_date_now() -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339("2026-04-12T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    #[test]
    fn derive_status_date_prefers_completed() {
        let mut fields = HashMap::new();
        fields.insert(
            "completed".to_string(),
            serde_json::json!("2026-04-10T00:00:00Z"),
        );
        fields.insert(
            "started".to_string(),
            serde_json::json!("2026-04-05T00:00:00Z"),
        );
        fields.insert("due".to_string(), serde_json::json!("2026-04-01"));
        fields.insert("scheduled".to_string(), serde_json::json!("2026-05-01"));
        fields.insert(
            "created".to_string(),
            serde_json::json!("2026-03-15T08:00:00Z"),
        );

        let result = compute_status_date(&fields, status_date_now());
        assert_eq!(result["kind"], "completed");
        assert_eq!(result["timestamp"], "2026-04-10T00:00:00Z");
    }

    #[test]
    fn derive_status_date_overdue_when_due_past() {
        let mut fields = HashMap::new();
        fields.insert("due".to_string(), serde_json::json!("2026-04-01"));
        fields.insert(
            "created".to_string(),
            serde_json::json!("2026-03-15T08:00:00Z"),
        );

        let result = compute_status_date(&fields, status_date_now());
        assert_eq!(result["kind"], "overdue");
        // Preserves original calendar-date form.
        assert_eq!(result["timestamp"], "2026-04-01");
    }

    #[test]
    fn derive_status_date_started_over_future_due() {
        let mut fields = HashMap::new();
        // Due is in the future → does NOT trigger overdue.
        fields.insert("due".to_string(), serde_json::json!("2026-05-01"));
        fields.insert(
            "started".to_string(),
            serde_json::json!("2026-04-10T00:00:00Z"),
        );
        fields.insert(
            "created".to_string(),
            serde_json::json!("2026-03-15T08:00:00Z"),
        );

        let result = compute_status_date(&fields, status_date_now());
        assert_eq!(result["kind"], "started");
        assert_eq!(result["timestamp"], "2026-04-10T00:00:00Z");
    }

    #[test]
    fn derive_status_date_scheduled_when_future() {
        let mut fields = HashMap::new();
        fields.insert("scheduled".to_string(), serde_json::json!("2026-05-01"));
        fields.insert(
            "created".to_string(),
            serde_json::json!("2026-03-15T08:00:00Z"),
        );

        let result = compute_status_date(&fields, status_date_now());
        assert_eq!(result["kind"], "scheduled");
        assert_eq!(result["timestamp"], "2026-05-01");
    }

    #[test]
    fn derive_status_date_created_fallback() {
        let mut fields = HashMap::new();
        fields.insert(
            "created".to_string(),
            serde_json::json!("2026-03-15T08:00:00Z"),
        );

        let result = compute_status_date(&fields, status_date_now());
        assert_eq!(result["kind"], "created");
        assert_eq!(result["timestamp"], "2026-03-15T08:00:00Z");
    }

    #[test]
    fn derive_status_date_empty_inputs_returns_null() {
        let fields = HashMap::new();
        let result = compute_status_date(&fields, status_date_now());
        assert_eq!(result, serde_json::Value::Null);
    }

    #[test]
    fn derive_status_date_due_future_no_started_falls_through() {
        let mut fields = HashMap::new();
        // due in the future, no started/scheduled — must NOT produce overdue.
        fields.insert("due".to_string(), serde_json::json!("2026-05-01"));
        fields.insert(
            "created".to_string(),
            serde_json::json!("2026-03-15T08:00:00Z"),
        );

        let result = compute_status_date(&fields, status_date_now());
        assert_eq!(result["kind"], "created");
        assert_eq!(result["timestamp"], "2026-03-15T08:00:00Z");
    }

    /// Smoke test: the registered derivation dispatches to compute_status_date and
    /// yields the expected tagged value via the ComputeEngine.
    #[tokio::test]
    async fn derive_status_date_registered_runs_via_engine() {
        let engine = kanban_compute_engine();
        let field = swissarmyhammer_fields::FieldDef {
            id: swissarmyhammer_fields::FieldDefId::new(),
            name: "status_date".into(),
            description: None,
            type_: swissarmyhammer_fields::FieldType::Computed {
                derive: "derive-status-date".to_string(),
                depends_on: vec![
                    "completed".to_string(),
                    "started".to_string(),
                    "due".to_string(),
                    "scheduled".to_string(),
                    "created".to_string(),
                ],
                entity: None,
                commit_display_names: false,
            },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
            groupable: None,
        };
        let mut fields = HashMap::new();
        fields.insert(
            "completed".to_string(),
            serde_json::json!("2026-04-10T00:00:00Z"),
        );

        let result = engine.derive(&field, &fields, None).await.unwrap();
        assert_eq!(result["kind"], "completed");
        assert_eq!(result["timestamp"], "2026-04-10T00:00:00Z");
    }

    /// Integration test: when `derive_all` processes the task entity's field
    /// list, status_date must appear after its depends_on so the prior fields
    /// (completed/started/created) are already resolved in the map by the
    /// time status_date runs. Uses the real builtin task entity's field list.
    #[tokio::test]
    async fn derive_status_date_resolves_after_its_dependencies_in_task_order() {
        let defs = builtin_field_definitions();
        let entities = builtin_entity_definitions();
        let ctx = swissarmyhammer_fields::FieldsContext::from_yaml_sources(
            std::path::PathBuf::from("/tmp/test-status-date-order"),
            &defs,
            &entities,
        )
        .unwrap();

        let engine = kanban_compute_engine();
        let task_fields: Vec<_> = ctx.fields_for_entity("task").into_iter().cloned().collect();

        // Find status_date's position and its depends_on names.
        let status_idx = task_fields
            .iter()
            .position(|f| f.name.as_str() == "status_date")
            .expect("status_date must be in task fields");

        let depends_on = match &task_fields[status_idx].type_ {
            swissarmyhammer_fields::FieldType::Computed { depends_on, .. } => depends_on.clone(),
            _ => panic!("status_date must be a computed field"),
        };

        // Each non-internal dependency must be positioned earlier than status_date
        // in the task fields list, so derive_all has already resolved it.
        for dep in &depends_on {
            if dep.starts_with('_') {
                continue; // internal injected fields like _changelog
            }
            let dep_idx = task_fields
                .iter()
                .position(|f| f.name.as_str() == dep.as_str())
                .unwrap_or_else(|| panic!("dep {dep} not in task fields"));
            assert!(
                dep_idx < status_idx,
                "status_date depends on {dep} but {dep} appears at index {dep_idx} \
                 after status_date at index {status_idx} — derive_all would see \
                 {dep} unresolved"
            );
        }

        // Smoke-test the actual pipeline: drive derive_all over a minimal
        // changelog that marks the task completed, and confirm status_date
        // resolves to the completed timestamp via the real field list.
        let mut fields = HashMap::new();
        fields.insert(
            "_changelog".to_string(),
            serde_json::json!([
                mock_changelog_entry("create", "2026-01-01T10:00:00Z", Some(("set", "todo"))),
                mock_changelog_entry(
                    "update",
                    "2026-02-01T14:00:00Z",
                    Some(("text_diff", "todo->doing"))
                ),
                mock_changelog_entry(
                    "update",
                    "2026-04-10T00:00:00Z",
                    Some(("text_diff", "doing->done"))
                ),
            ]),
        );

        let query = column_query(vec![("todo", 0), ("doing", 1), ("done", 2)]);
        engine
            .derive_all(&mut fields, &task_fields, Some(&query))
            .await
            .unwrap();

        let resolved = fields.get("status_date").expect("status_date resolved");
        assert_eq!(resolved["kind"], "completed");
        assert_eq!(resolved["timestamp"], "2026-04-10T00:00:00Z");
    }
}
