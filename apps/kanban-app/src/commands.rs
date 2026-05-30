// ┌──────────────────────────────────────────────────────────────────────────┐
// │ 🛑🛑🛑  STOP — READ THIS BEFORE ADDING A NEW #[tauri::command]  🛑🛑🛑 │
// │                                                                        │
// │  ALL state mutations MUST go through `dispatch_command` so they flow    │
// │  through the swissarmyhammer-commands system. This is REQUIRED for:     │
// │                                                                        │
// │    ✅  Undo / Redo support                                             │
// │    ✅  UIState persistence                                             │
// │    ✅  Event emission (ui-state-changed)                               │
// │    ✅  Command logging and observability                               │
// │                                                                        │
// │  Adding a new #[tauri::command] that mutates state BYPASSES all of     │
// │  this. If you think you need one, you almost certainly need a new      │
// │  command impl in swissarmyhammer-commands instead.                     │
// │                                                                        │
// │  Acceptable Tauri commands:                                            │
// │    • Read-only queries (get_board_data, list_entities, etc.)           │
// │    • OS-level operations (create_window, restore_windows, quit_app)    │
// │    • Transient UI plumbing (drag session start/cancel, context menus)  │
// │                                                                        │
// │  If in doubt, ask. Don't just add a quick invoke().                    │
// └──────────────────────────────────────────────────────────────────────────┘

//! Tauri commands for board operations.

use crate::menu;
use crate::state::{AppState, BoardHandle};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer_entity::Entity;
use swissarmyhammer_kanban::task_helpers::{
    enrich_all_task_entities, enrich_task_entity, retain_filtered_tasks, EntitySlugRegistry,
};
use swissarmyhammer_kanban::virtual_tags::default_virtual_tag_registry;
use tauri::menu::{ContextMenu, MenuBuilder};
use tauri::webview::WebviewWindowBuilder;
use tauri::{AppHandle, Emitter, Manager, State, Window};

/// The base application title shown in all window title bars.
pub const APP_TITLE: &str = "SwissArmyHammer";

/// Maximum number of mention-autocomplete suggestions returned per request.
///
/// Sized for the CM6 autocomplete popup: 20 rows is the comfortable visual
/// upper bound before the list needs scrolling, and it keeps the per-keystroke
/// payload small. Raising it would bloat the dropdown without helping the
/// user — they'll keep typing to narrow instead of scrolling.
const MENTION_AUTOCOMPLETE_LIMIT: usize = 20;

/// Default cap on `search_entities` results when the caller omits `limit`.
///
/// Fuzzy-matcher work is bounded by this limit, so a cautious default keeps
/// the global-search popover snappy under large boards. Callers wiring a UI
/// that wants more can pass an explicit `limit`.
const DEFAULT_SEARCH_RESULT_LIMIT: usize = 50;

/// Maximum accepted length for a `search_entities` query string.
///
/// Caps fuzzy-matcher work per call. 500 characters comfortably exceeds any
/// realistic user-typed query while keeping the worst-case per-request cost
/// bounded — longer strings almost always indicate paste bombs or a stuck
/// input loop, not a real search.
const MAX_SEARCH_QUERY_LENGTH: usize = 500;

/// Maximum length in bytes of a command identifier passed to `dispatch_command`.
///
/// Command names are ASCII-only dotted identifiers. 128 bytes is plenty for
/// the longest registered command (well under 50 chars today) while giving a
/// hard ceiling on the allocation work the dispatcher will spend validating
/// a garbage request. Pairs with the ASCII check below.
const MAX_COMMAND_LENGTH: usize = 128;

/// Default logical pixel dimensions for a newly created board window.
///
/// Sized to fit comfortably on a 1280×800 MacBook Air display while being
/// generous enough that `kanban-app`'s multi-column board shows at least
/// three swimlanes without horizontal scrolling. Persisted window geometry
/// overrides this on restore.
const INITIAL_WINDOW_WIDTH: f64 = 1200.0;
const INITIAL_WINDOW_HEIGHT: f64 = 800.0;

/// Generate a unique window label using ULID.
fn new_window_label() -> String {
    format!("board-{}", ulid::Ulid::new().to_string().to_lowercase())
}

/// Set the window title to reflect the currently loaded board.
///
/// When `board_name` is `Some`, the title becomes "SwissArmyHammer — project-name".
/// When `None`, the title resets to just "SwissArmyHammer".
fn update_window_title(app: &AppHandle, label: &str, board_name: Option<&str>) {
    let title = match board_name {
        Some(name) if !name.is_empty() => format!("{APP_TITLE} \u{2014} {name}"),
        _ => APP_TITLE.to_string(),
    };
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.set_title(&title);
    }
}

/// Gather window info from Tauri for dynamic commands.
///
/// Stays in the GUI crate because live window titles, visibility, and focus
/// state only exist on the Tauri runtime. The headless `DynamicSources`
/// builder in `swissarmyhammer_kanban::dynamic_sources` takes the result of
/// this function as a caller-supplied input; tests fabricate the list.
fn gather_windows(app: &tauri::AppHandle) -> Vec<swissarmyhammer_common::WindowInfo> {
    use swissarmyhammer_common::WindowInfo;
    app.webview_windows()
        .iter()
        .filter_map(|(label, w)| {
            let title = w.title().ok()?;
            if title.is_empty() || !w.is_visible().unwrap_or(false) {
                return None;
            }
            Some(WindowInfo {
                label: label.clone(),
                title,
                focused: w.is_focused().unwrap_or(false),
            })
        })
        .collect()
}

/// Resolve a board handle — by explicit path or falling back to active board.
///
/// When `board_path` is provided, canonicalizes it to match the key format
/// used by `AppState::boards` (which stores canonical paths from `open_board`).
/// The fallback to the raw path on canonicalize failure is safe: it simply
/// won't match any key, producing a clear "Board not open" error.
async fn resolve_handle(
    state: &AppState,
    board_path: Option<String>,
) -> Result<Arc<BoardHandle>, String> {
    if let Some(bp) = board_path {
        // Boards are keyed by canonical path; fall back to raw path if
        // canonicalize fails (will produce "Board not open" on mismatch).
        let canonical = PathBuf::from(&bp)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&bp));
        let boards = state.boards.read().await;
        boards
            .get(&canonical)
            .cloned()
            .ok_or_else(|| format!("Board not open: {}", bp))
    } else {
        state.active_handle().await.ok_or("No active board".into())
    }
}

/// List all currently open boards.
#[tauri::command]
pub async fn list_open_boards(state: State<'_, AppState>) -> Result<Value, String> {
    let boards = state.boards.read().await;
    let most_recent = state.ui_state.most_recent_board().map(PathBuf::from);

    let mut list: Vec<Value> = Vec::new();
    for (path, handle) in boards.iter() {
        let is_active = most_recent.as_ref() == Some(path);
        let name = swissarmyhammer_kanban::board_display_name(&handle.ctx)
            .await
            .unwrap_or_default();
        list.push(json!({
            "path": path.display().to_string(),
            "is_active": is_active,
            "name": name,
        }));
    }

    Ok(json!(list))
}

/// Return the full UIState as JSON for the frontend.
///
/// Returns a snapshot of all UIState fields including transient ones
/// (`palette_open`, `scope_chain`). The frontend uses this on mount
/// to initialise the UIStateProvider.
#[tauri::command]
pub async fn get_ui_state(state: State<'_, AppState>) -> Result<Value, String> {
    Ok(state.ui_state.to_json())
}

/// Get the field+entity schema for a given entity type.
///
/// Returns the EntityDef plus each resolved FieldDef, serialized as JSON.
#[tauri::command]
pub async fn get_entity_schema(
    state: State<'_, AppState>,
    entity_type: String,
    board_path: Option<String>,
) -> Result<Value, String> {
    let handle = resolve_handle(&state, board_path).await?;
    let ectx = handle
        .ctx
        .entity_context()
        .await
        .map_err(|e| e.to_string())?;
    let fields_ctx = ectx.fields();

    let entity_def = fields_ctx
        .get_entity(&entity_type)
        .ok_or_else(|| format!("Unknown entity type: {}", entity_type))?;

    let field_defs: Vec<Value> = fields_ctx
        .fields_for_entity(&entity_type)
        .iter()
        .map(|f| {
            serde_json::to_value(f)
                .map_err(|e| format!("failed to serialize field '{}': {}", f.name, e))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(json!({
        "entity": serde_json::to_value(entity_def).map_err(|e| e.to_string())?,
        "fields": field_defs,
    }))
}

/// List all registered entity type names.
///
/// Returns an array of entity type name strings discovered from the schema.
#[tauri::command]
pub async fn list_entity_types(
    state: State<'_, AppState>,
    board_path: Option<String>,
) -> Result<Value, String> {
    let handle = resolve_handle(&state, board_path).await?;
    let ectx = handle
        .ctx
        .entity_context()
        .await
        .map_err(|e| e.to_string())?;
    let fields_ctx = ectx.fields();
    let names: Vec<&str> = fields_ctx
        .all_entities()
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    Ok(json!(names))
}

/// Enrich task entities with computed fields and sort by position.
///
/// Loads columns to determine the terminal column, then batch-enriches tasks
/// with readiness, dependency, and virtual tag data. Finally sorts by
/// (column, ordinal) so the frontend can trust the order.
async fn enrich_and_sort_tasks(
    entities: &mut [Entity],
    ectx: &swissarmyhammer_entity::EntityContext,
    entity_type: &str,
) -> Result<(), String> {
    let mut columns = ectx
        .list("column")
        .await
        .map_err(|e| format!("list_entities({}): {}", entity_type, e))?;
    columns.sort_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0) as usize);
    let terminal_id = columns
        .last()
        .map(|c| c.id.to_string())
        .unwrap_or_else(|| "done".to_string());

    let registry = default_virtual_tag_registry();
    enrich_all_task_entities(entities, &terminal_id, registry);

    entities.sort_by(|a, b| {
        let col_a = a.get_str("position_column").unwrap_or("");
        let col_b = b.get_str("position_column").unwrap_or("");
        col_a.cmp(col_b).then_with(|| {
            let ord_a = a.get_str("position_ordinal").unwrap_or("a0");
            let ord_b = b.get_str("position_ordinal").unwrap_or("a0");
            ord_a.cmp(ord_b)
        })
    });
    Ok(())
}

/// List all entities of a given type, returning raw entity bags.
///
/// For tasks, enriches each entity with computed fields: `ready`, `blocked_by`,
/// `blocks`, and `progress_fraction`. Other entity types are returned as-is.
///
/// When `filter` is provided, parses it as a filter DSL expression and returns
/// only entities that match. Empty or whitespace-only filters are treated as
/// no filter (all entities returned).
///
/// Returns `{ entities: [...], count: N }`.
#[tauri::command]
pub async fn list_entities(
    state: State<'_, AppState>,
    entity_type: String,
    filter: Option<String>,
    board_path: Option<String>,
) -> Result<Value, String> {
    let handle = resolve_handle(&state, board_path).await?;
    let ectx = handle
        .ctx
        .entity_context()
        .await
        .map_err(|e| format!("list_entities({}): {}", entity_type, e))?;
    let mut entities = ectx
        .list(&entity_type)
        .await
        .map_err(|e| format!("list_entities({}): {}", entity_type, e))?;

    if entity_type == "task" {
        enrich_and_sort_tasks(&mut entities, &ectx, &entity_type).await?;
    }

    if let Some(filter_str) = filter.as_deref().filter(|f| !f.trim().is_empty()) {
        // Build the id-or-slug registry once so `$project`, `@user`, and
        // `^task` predicates resolve display-name slugs to entity ids.
        // For non-task entity types, `entities` wouldn't contribute a
        // useful task-title index, so we list tasks separately to keep
        // the registry complete.
        let projects = ectx
            .list("project")
            .await
            .map_err(|e| format!("list_entities({}): list(project): {}", entity_type, e))?;
        let actors = ectx
            .list("actor")
            .await
            .map_err(|e| format!("list_entities({}): list(actor): {}", entity_type, e))?;
        let tasks = if entity_type == "task" {
            // Reuse the already-loaded tasks to avoid a redundant list.
            None
        } else {
            Some(
                ectx.list("task")
                    .await
                    .map_err(|e| format!("list_entities({}): list(task): {}", entity_type, e))?,
            )
        };
        let task_slice: &[Entity] = tasks.as_deref().unwrap_or(&entities);
        let registry = EntitySlugRegistry::build(&projects, &actors, task_slice);
        retain_filtered_tasks(&mut entities, filter_str, &registry)?;
    }

    let json_entities: Vec<Value> = entities.iter().map(|e| e.to_json()).collect();
    Ok(json!({
        "entities": json_entities,
        "count": json_entities.len(),
    }))
}

/// Get a single entity by type and id, returning a raw entity bag.
///
/// For tasks, enriches with computed fields: `ready`, `blocked_by`, `blocks`,
/// and `progress_fraction`. Other entity types are returned as-is.
#[tauri::command]
pub async fn get_entity(
    state: State<'_, AppState>,
    entity_type: String,
    id: String,
    board_path: Option<String>,
) -> Result<Value, String> {
    let handle = resolve_handle(&state, board_path).await?;
    let ectx = handle
        .ctx
        .entity_context()
        .await
        .map_err(|e| format!("get_entity({}/{}): {}", entity_type, id, e))?;
    let mut entity = ectx
        .read(&entity_type, &id)
        .await
        .map_err(|e| format!("get_entity({}/{}): {}", entity_type, id, e))?;

    if entity_type == "task" {
        let all_tasks = ectx
            .list("task")
            .await
            .map_err(|e| format!("get_entity({}/{}): {}", entity_type, id, e))?;
        let mut columns = ectx
            .list("column")
            .await
            .map_err(|e| format!("get_entity({}/{}): {}", entity_type, id, e))?;
        columns.sort_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0) as usize);
        let terminal_id = columns
            .last()
            .map(|c| c.id.to_string())
            .unwrap_or_else(|| "done".to_string());
        let registry = default_virtual_tag_registry();
        enrich_task_entity(&mut entity, &all_tasks, &terminal_id, registry);
    }

    Ok(entity.to_json())
}

/// Search entities by display field for mention autocomplete.
///
/// Searches entities of the given type by their `mention_display_field`
/// (from the entity definition). Returns matches with id, display_name,
/// color, and avatar for CM6 autocomplete rendering.
///
/// Returns `[{id, display_name, color, avatar}]`.
#[tauri::command]
pub async fn search_mentions(
    state: State<'_, AppState>,
    entity_type: String,
    query: String,
    board_path: Option<String>,
) -> Result<Value, String> {
    let handle = resolve_handle(&state, board_path).await?;
    let ectx = handle
        .ctx
        .entity_context()
        .await
        .map_err(|e| e.to_string())?;

    let display_field = mention_display_field_for(&ectx, &entity_type)?;
    let entities = ectx.list(&entity_type).await.map_err(|e| e.to_string())?;
    let matches = filter_mention_candidates(&entities, &query, display_field);
    Ok(json!(matches))
}

/// Resolve the `mention_display_field` for `entity_type` from the field
/// registry, defaulting to `"name"` when the entity definition doesn't set
/// one explicitly.
fn mention_display_field_for<'a>(
    ectx: &'a swissarmyhammer_entity::EntityContext,
    entity_type: &str,
) -> Result<&'a str, String> {
    let fields_ctx = ectx.fields();
    let entity_def = fields_ctx
        .get_entity(entity_type)
        .ok_or_else(|| format!("Unknown entity type: {}", entity_type))?;
    Ok(entity_def
        .mention_display_field
        .as_deref()
        .unwrap_or("name"))
}

/// Filter `entities` down to the first `MENTION_AUTOCOMPLETE_LIMIT` whose
/// display field or ID case-insensitively contains `query`, projecting each
/// surviving entity into the `{id, display_name, color, avatar}` shape the
/// CM6 autocomplete popup renders. An empty `query` returns the first
/// `MENTION_AUTOCOMPLETE_LIMIT` entities as-is.
fn filter_mention_candidates(entities: &[Entity], query: &str, display_field: &str) -> Vec<Value> {
    let query_lower = query.to_lowercase();
    entities
        .iter()
        .filter(|e| {
            if query_lower.is_empty() {
                return true;
            }
            let display = e.get_str(display_field).unwrap_or("");
            let id = e.id.as_str();
            display.to_lowercase().contains(&query_lower)
                || id.to_lowercase().contains(&query_lower)
        })
        .take(MENTION_AUTOCOMPLETE_LIMIT)
        .map(|e| {
            json!({
                "id": e.id,
                "display_name": e.get_str(display_field).unwrap_or(""),
                "color": e.get_str("color"),
                "avatar": e.get_str("avatar"),
            })
        })
        .collect()
}

/// Build a single search-result JSON row from an entity plus a score.
///
/// Resolves the display name using the entity schema's
/// `search_display_field`, falling back to `mention_display_field`,
/// then `"name"`, then `"title"`, and finally the entity ID. The
/// resulting shape is `{ entity_type, entity_id, display_name, score }`
/// — the element shape consumed by the frontend search presenter.
fn build_search_result_row(
    entity: &Entity,
    score: f64,
    fields_ctx: &swissarmyhammer_fields::FieldsContext,
) -> Value {
    let entity_type = entity.entity_type.as_str();

    // Resolve display field: search_display_field > mention_display_field > "name" > "title"
    let display_field = fields_ctx
        .get_entity(entity_type)
        .and_then(|def| {
            def.search_display_field
                .as_ref()
                .or(def.mention_display_field.as_ref())
                .map(|f| f.as_str())
        })
        .unwrap_or("name");

    let display_name = entity
        .get_str(display_field)
        .or_else(|| entity.get_str("name"))
        .or_else(|| entity.get_str("title"))
        .unwrap_or(entity.id.as_str());

    json!({
        "entity_type": entity_type,
        "entity_id": entity.id,
        "display_name": display_name,
        "score": score,
    })
}

/// Search all entities using the backend search index.
///
/// The backend owns the search strategy: currently fuzzy matching via
/// `EntitySearchIndex::search()`, switching to hybrid (fuzzy + semantic
/// embedding) when an embedder is configured. The frontend calls this
/// command and displays results — no search logic runs client-side.
///
/// For each result, resolves the display name using the entity's
/// `search_display_field` (falling back to `mention_display_field`,
/// then "name", then "title").
///
/// Returns `[{ entity_type, entity_id, display_name, score }]`.
#[tauri::command]
pub async fn search_entities(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
    board_path: Option<String>,
) -> Result<Value, String> {
    let handle = resolve_handle(&state, board_path).await?;
    let limit = limit.unwrap_or(DEFAULT_SEARCH_RESULT_LIMIT);

    // Cap query length to prevent excessive fuzzy matcher work
    if query.len() > MAX_SEARCH_QUERY_LENGTH {
        return Err(format!(
            "Search query too long (max {MAX_SEARCH_QUERY_LENGTH} characters)"
        ));
    }

    // Empty query returns empty results
    if query.trim().is_empty() {
        return Ok(json!([]));
    }

    let search_index = handle.search_index.read().await;
    // Strategy decision point: currently fuzzy-only. When a TextEmbedder is
    // stored on BoardHandle, switch to search_index.search_hybrid(&query,
    // &embedder, limit) which automatically picks fuzzy for short queries
    // and semantic for longer ones, with cross-strategy fallback.
    let results = search_index.search(&query, limit);

    // Resolve display names using entity schema
    let ectx = handle
        .ctx
        .entity_context()
        .await
        .map_err(|e| e.to_string())?;
    let fields_ctx = ectx.fields();

    let output: Vec<Value> = results
        .iter()
        .filter_map(|result| {
            let entity = search_index.get(&result.entity_id)?;
            Some(build_search_result_row(entity, result.score, fields_ctx))
        })
        .collect();

    Ok(json!(output))
}

/// Count tasks and ready tasks per column.
fn count_tasks_by_column(
    tasks: &[Entity],
    terminal_id: &str,
) -> (HashMap<String, usize>, HashMap<String, usize>) {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut ready_counts: HashMap<String, usize> = HashMap::new();
    for task in tasks {
        let col = task
            .get_str("position_column")
            .unwrap_or("todo")
            .to_string();
        *counts.entry(col.clone()).or_insert(0) += 1;
        if swissarmyhammer_kanban::task_helpers::task_is_ready(task, tasks, terminal_id) {
            *ready_counts.entry(col).or_insert(0) += 1;
        }
    }
    (counts, ready_counts)
}

/// Serialize columns with injected task_count and ready_count fields.
fn serialize_columns_with_counts(
    columns: &[Entity],
    counts: &HashMap<String, usize>,
    ready_counts: &HashMap<String, usize>,
) -> Vec<Value> {
    columns
        .iter()
        .map(|col| {
            let mut e = col.clone();
            e.set(
                "task_count",
                json!(counts.get(col.id.as_str()).copied().unwrap_or(0)),
            );
            e.set(
                "ready_count",
                json!(ready_counts.get(col.id.as_str()).copied().unwrap_or(0)),
            );
            e.to_json()
        })
        .collect()
}

/// Build the summary object for get_board_data.
fn build_board_summary(
    board: &Entity,
    total_tasks: usize,
    total_actors: usize,
    ready_counts: &HashMap<String, usize>,
) -> Value {
    let ready_tasks: usize = ready_counts.values().sum();
    let pc = board
        .get("percent_complete")
        .cloned()
        .unwrap_or(json!(null));
    let done_tasks = pc.get("done").and_then(|v| v.as_u64()).unwrap_or(0);
    let percent_complete = pc.get("percent").and_then(|v| v.as_u64()).unwrap_or(0);
    json!({
        "total_tasks": total_tasks,
        "total_actors": total_actors,
        "ready_tasks": ready_tasks,
        "blocked_tasks": total_tasks - ready_tasks,
        "done_tasks": done_tasks,
        "percent_complete": percent_complete,
    })
}

/// Get the board data with all entities as raw entity bags.
///
/// Columns and tags are returned as `Entity::to_json()` with
/// computed count fields injected. Tasks are NOT included (use `list_entities`
/// for that). A summary object provides aggregate counts.
#[tauri::command]
pub async fn get_board_data(
    state: State<'_, AppState>,
    board_path: Option<String>,
) -> Result<Value, String> {
    let handle = resolve_handle(&state, board_path).await?;
    let ectx = handle
        .ctx
        .entity_context()
        .await
        .map_err(|e| format!("get_board_data: {}", e))?;

    let BoardEntities {
        board,
        columns,
        tags,
        all_tasks,
        total_actors,
    } = load_board_entities(&ectx).await?;

    let terminal_id = columns.last().map(|c| c.id.as_str()).unwrap_or("done");
    let (counts, ready_counts) = count_tasks_by_column(&all_tasks, terminal_id);
    let columns_json = serialize_columns_with_counts(&columns, &counts, &ready_counts);
    let tags_json: Vec<Value> = tags.iter().map(|tag| tag.to_json()).collect();
    let summary = build_board_summary(&board, all_tasks.len(), total_actors, &ready_counts);
    let virtual_tag_meta = virtual_tag_meta_json();

    Ok(json!({
        "board": board.to_json(),
        "columns": columns_json,
        "tags": tags_json,
        "virtual_tag_meta": virtual_tag_meta,
        "summary": summary,
    }))
}

/// Every entity set `get_board_data` needs, loaded in one place so the
/// command body can focus on projecting them into the response shape.
struct BoardEntities {
    board: swissarmyhammer_entity::Entity,
    columns: Vec<swissarmyhammer_entity::Entity>,
    tags: Vec<swissarmyhammer_entity::Entity>,
    all_tasks: Vec<swissarmyhammer_entity::Entity>,
    total_actors: usize,
}

/// Load every entity collection `get_board_data` needs from the entity
/// context: the board row, sorted columns, tags, all tasks, and the actor
/// count (used by the summary). Columns come back pre-sorted by `order`.
async fn load_board_entities(
    ectx: &swissarmyhammer_entity::EntityContext,
) -> Result<BoardEntities, String> {
    let board = ectx
        .read("board", "board")
        .await
        .map_err(|e| format!("get_board_data: {}", e))?;
    let mut columns = ectx
        .list("column")
        .await
        .map_err(|e| format!("get_board_data: {}", e))?;
    columns.sort_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0) as usize);
    let tags = ectx
        .list("tag")
        .await
        .map_err(|e| format!("get_board_data: {}", e))?;
    let all_tasks = ectx
        .list("task")
        .await
        .map_err(|e| format!("get_board_data: {}", e))?;
    let total_actors = ectx
        .list("actor")
        .await
        .map_err(|e| format!("get_board_data: {}", e))?
        .len();
    Ok(BoardEntities {
        board,
        columns,
        tags,
        all_tasks,
        total_actors,
    })
}

/// Serialize every virtual tag from the default registry into the JSON
/// shape the frontend `BoardData.virtualTagMeta` wants.
fn virtual_tag_meta_json() -> Vec<Value> {
    default_virtual_tag_registry()
        .metadata()
        .into_iter()
        .map(|m| json!({ "slug": m.slug, "color": m.color, "description": m.description }))
        .collect()
}

/// Quit the application.
#[tauri::command]
pub async fn quit_app(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}

/// Open a folder picker to create a new board.
#[tauri::command]
pub async fn new_board_dialog(app: AppHandle) -> Result<(), String> {
    menu::trigger_new_board(&app);
    Ok(())
}

/// Open a folder picker to open an existing board.
#[tauri::command]
pub async fn open_board_dialog(app: AppHandle) -> Result<(), String> {
    menu::trigger_open_board(&app);
    Ok(())
}

/// Create a new window, optionally opening a specific board.
///
/// If `board_path` is not provided, uses the currently active board.
/// The window label and board path are persisted to `windows`
/// so the window can be restored at the same position on restart.
/// Internal helper for creating a window, callable from dispatch side effects.
async fn create_window_internal(app: &AppHandle, state: &AppState) {
    if let Err(e) = create_window_impl(app, state, None, None, None).await {
        tracing::error!("create_window_internal failed: {e}");
    }
}

/// Tauri command: create a new webview window.
#[tauri::command]
pub async fn create_window(
    app: AppHandle,
    state: State<'_, AppState>,
    board_path: Option<String>,
) -> Result<Value, String> {
    create_window_impl(&app, &state, board_path, None, None).await
}

/// Window position and size used when creating or restoring a board window.
///
/// Populated from persisted `UIState` on app start, then passed to
/// [`create_window_impl`]. All fields are in logical pixels relative to the
/// primary display's top-left origin, matching what Tauri's window APIs
/// return from `outer_position()` / `outer_size()`.
pub struct WindowGeometry {
    /// Logical-pixel x coordinate of the window's top-left corner.
    pub x: i32,
    /// Logical-pixel y coordinate of the window's top-left corner.
    pub y: i32,
    /// Window width in logical pixels.
    pub width: u32,
    /// Window height in logical pixels.
    pub height: u32,
    /// Whether the window was maximized when the geometry was captured.
    pub maximized: bool,
}

/// Resolve which board path a newly-created window should display.
///
/// Precedence: explicit `board_path` argument wins; otherwise falls back
/// to the most-recently-focused board from `UIState`, and finally to any
/// currently-open board. Returns `None` only when no board is available
/// at all — a window with no board still renders (empty state).
fn resolve_window_board_path(state: &AppState, board_path: Option<String>) -> Option<String> {
    match board_path {
        Some(bp) => Some(bp),
        None => state.ui_state.most_recent_board().or_else(|| {
            let boards = state.boards.try_read().ok();
            boards.and_then(|b| b.keys().next().map(|p| p.display().to_string()))
        }),
    }
}

/// Apply saved geometry (position, size, maximized) to a freshly built
/// webview window.
///
/// This is the window-restore path: on startup we rebuild windows that
/// were open last session and push their saved geometry back onto the
/// OS window. When `geometry` is `None` (new window), the OS default
/// placement is preserved and this function is a no-op.
fn apply_saved_geometry(window: &tauri::WebviewWindow, geometry: Option<&WindowGeometry>) {
    let Some(geo) = geometry else { return };
    let _ = window.set_size(tauri::PhysicalSize::new(geo.width, geo.height));
    let _ = window.set_position(tauri::PhysicalPosition::new(geo.x, geo.y));
    if geo.maximized {
        let _ = window.maximize();
    }
}

/// Persist a window's geometry into `UIState` so the window can be
/// restored on next launch.
///
/// Uses the provided `geometry` directly when present (restore path,
/// avoiding a race with OS placement), otherwise reads the actual
/// position/size from the live window after the OS has placed it
/// (new-window path). Failures to read live geometry are silently
/// ignored — best-effort persistence.
fn persist_window_geometry(
    state: &AppState,
    label: &str,
    window: &tauri::WebviewWindow,
    geometry: Option<&WindowGeometry>,
) {
    if let Some(geo) = geometry {
        state.ui_state.save_window_geometry(
            label,
            geo.x,
            geo.y,
            geo.width,
            geo.height,
            geo.maximized,
        );
    } else if let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) {
        let maximized = window.is_maximized().unwrap_or(false);
        state.ui_state.save_window_geometry(
            label,
            pos.x,
            pos.y,
            size.width,
            size.height,
            maximized,
        );
    }
}

/// Build a new (still hidden) webview window pointed at the given board path.
///
/// Constructs the `index.html?window=board&board=...` URL and builds
/// the `WebviewWindowBuilder` with the app's default size and title.
/// The returned window is `visible(false)` — the caller is responsible
/// for applying geometry, then calling `show()` and `set_focus()`.
/// Returns `Err` only when the underlying Tauri `build()` fails.
fn build_window_for_board(
    app: &AppHandle,
    label: &str,
    resolved_path: Option<&str>,
) -> Result<tauri::WebviewWindow, String> {
    let mut url = String::from("index.html?window=board");
    if let Some(bp) = resolved_path {
        url.push_str("&board=");
        url.push_str(&urlencoding::encode(bp));
    }

    let window = WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::App(url.into()))
        .title(APP_TITLE)
        .visible(false)
        .inner_size(INITIAL_WINDOW_WIDTH, INITIAL_WINDOW_HEIGHT)
        .resizable(true)
        .disable_drag_drop_handler()
        .build()
        .map_err(|e| format!("Failed to create window: {e}"))?;

    Ok(window)
}

/// Set a window's title from the board entity's display name.
///
/// Canonicalizes the board path, looks up the handle, reads the board
/// entity's `name` field, and delegates to `update_window_title`. A
/// missing handle (e.g. the board is not currently open in this app
/// instance) is a no-op — the window keeps its default title.
async fn apply_board_title(app: &AppHandle, state: &AppState, label: &str, board_path: &str) {
    let board_path = std::path::PathBuf::from(board_path);
    let canonical = board_path
        .canonicalize()
        .unwrap_or_else(|_| board_path.clone());
    let boards = state.boards.read().await;
    if let Some(handle) = boards.get(&canonical) {
        let name = swissarmyhammer_kanban::board_display_name(&handle.ctx).await;
        update_window_title(app, label, name.as_deref());
    }
}

/// Create a new webview window.
///
/// This is the single code path for all window creation — both user-initiated
/// (`window.new`) and startup restore. Every window created through this
/// function gets logged, persisted to UIState, and shows up in the command log.
///
/// - `board_path`: board to display. Falls back to most recent / first open.
/// - `label`: reuse a saved window label (for restore). `None` generates a new ULID.
/// - `geometry`: apply saved position/size (for restore). `None` uses OS defaults.
///
/// When `rebuild_menu` is true, rebuilds the native menu after creation.
/// Pass false when calling from `setup()` (via `block_on`) to avoid a
/// tokio deadlock — `rebuild_menu` uses `blocking_read` which panics
/// inside an existing `block_on`. Caller should rebuild the menu once
/// after all windows are created.
pub async fn create_window_impl(
    app: &AppHandle,
    state: &AppState,
    board_path: Option<String>,
    label: Option<String>,
    geometry: Option<WindowGeometry>,
) -> Result<Value, String> {
    let app = app.clone();
    let label = label.unwrap_or_else(new_window_label);
    tracing::info!(board_path = ?board_path, label = %label, "create_window called");

    let resolved_path = resolve_window_board_path(state, board_path);

    let window = build_window_for_board(&app, &label, resolved_path.as_deref())?;

    apply_saved_geometry(&window, geometry.as_ref());

    let _ = window.show();
    let _ = window.set_focus();

    // Persist window→board mapping AND geometry so the window can be
    // restored even if the user quits without moving it.
    if let Some(ref bp) = resolved_path {
        tracing::info!(
            label = %label,
            board_path = %bp,
            "persisting window state to UIState"
        );
        state.ui_state.set_window_board(&label, bp);
        persist_window_geometry(state, &label, &window, geometry.as_ref());
        apply_board_title(&app, state, &label, bp).await;
    }

    // Menu rebuild is handled by the frontend dispatching ui.setFocus
    // when the new window mounts — no explicit rebuild needed here.

    Ok(json!({
        "label": label,
        "board_path": resolved_path,
    }))
}

/// List all view definitions, returning a JSON array.
#[tauri::command]
pub async fn list_views(
    state: State<'_, AppState>,
    board_path: Option<String>,
) -> Result<Value, String> {
    let handle = resolve_handle(&state, board_path).await?;
    let views_lock = handle.ctx.views().ok_or("Views not initialized")?;
    let views = views_lock.read().await;

    let views_json: Vec<Value> = views
        .all_views()
        .iter()
        .map(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(json!(views_json))
}

// ---------------------------------------------------------------------------
// get_undo_state — read-only query for undo/redo availability
// ---------------------------------------------------------------------------

/// Return the current undo/redo availability for the active board.
///
/// Returns `{ "can_undo": bool, "can_redo": bool }`. If no board is open,
/// both are `false`.
#[tauri::command]
pub async fn get_undo_state(
    state: State<'_, AppState>,
    board_path: Option<String>,
) -> Result<Value, String> {
    let handle = match resolve_handle(&state, board_path).await {
        Ok(h) => h,
        Err(_) => return Ok(json!({ "can_undo": false, "can_redo": false })),
    };

    let sc = &handle.store_context;
    Ok(json!({
        "can_undo": sc.can_undo().await,
        "can_redo": sc.can_redo().await,
    }))
}

// ---------------------------------------------------------------------------
// log_command — lightweight log entry for commands that execute in the frontend
// ---------------------------------------------------------------------------

/// Log a command that was executed locally in the frontend.
///
/// Commands with a local `execute` handler never reach `dispatch_command`,
/// so the frontend calls this to ensure every command appears in the
/// unified Rust log.
#[tauri::command]
pub async fn log_command(cmd: String, target: Option<String>) {
    tracing::info!(cmd = %cmd, target = ?target, "command");
}

// ---------------------------------------------------------------------------
// save_dropped_file — write HTML5 drop bytes to a temp file
// ---------------------------------------------------------------------------

/// Receive file bytes from an HTML5 drop event and write to a temp file.
/// Returns the absolute path so the frontend can pass it to attachment copy.
#[tauri::command]
pub async fn save_dropped_file(filename: String, data: Vec<u8>) -> Result<String, String> {
    use std::io::Write;
    let safe = filename.replace(['/', '\\', '\0'], "_");
    let tmp_dir = std::env::temp_dir().join("kanban-drops");
    std::fs::create_dir_all(&tmp_dir).map_err(|e| e.to_string())?;
    let path = tmp_dir.join(format!("{}-{}", ulid::Ulid::new(), safe));
    let mut f = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    f.write_all(&data).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

// ---------------------------------------------------------------------------
// dispatch_command — unified command dispatcher via Command trait
// ---------------------------------------------------------------------------

/// Unified command dispatcher that routes a `cmd` string through the
/// `Command` trait system.
///
/// Looks up the command definition in the registry, resolves the scope
/// chain, checks availability, and executes via the trait implementation.
///
/// This is the Tauri entry point — it delegates to `dispatch_command_internal`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn dispatch_command(
    app: AppHandle,
    state: State<'_, AppState>,
    cmd: String,
    scope_chain: Option<Vec<String>>,
    target: Option<String>,
    args: Option<Value>,
    board_path: Option<String>,
) -> Result<Value, String> {
    dispatch_command_internal(&app, &state, &cmd, scope_chain, target, args, board_path).await
}

/// Maximum number of prefix rewrites before we reject the command.
const MAX_REWRITE_DEPTH: u8 = 1;

/// Rewrite result from the dynamic prefix loop.
struct RewriteResult {
    cmd: String,
    args: Option<Value>,
    board_path: Option<String>,
    /// If set, return this value immediately (e.g. for window.focus side-effects).
    early_return: Option<Value>,
}

/// Handle `window.focus:*` — pure OS side-effect with no undo/UIState implications.
fn handle_window_focus(app: &AppHandle, label: &str) -> Value {
    tracing::info!(label = %label, "window.focus — bringing window to front");
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    json!({ "WindowFocus": label })
}

/// Match a dynamic command prefix and return (new_cmd, arg_key, arg_value, updates_board_path).
///
/// The two remaining dynamic prefixes:
///
///   * `board.switch:{path}` — rewrites to `file.switchBoard` with `path`
///     lifted into args and additionally propagated as the effective board
///     path so downstream multi-window targeting switches boards in lockstep
///     with the command.
///   * `entity.add:{type}` — rewrites to the canonical `entity.add` with
///     `entity_type` moved into the arg bag; `AddEntityCmd` reads
///     `entity_type` from args and forwards every other arg as a field
///     override, so adding a new entity type needs zero Rust changes here.
///
/// `view.switch:{id}` and `perspective.goto:{id}` were retired in
/// 01KPZMXXEXKVE3RNPA4XJP0105 — the emit_* helpers in
/// `swissarmyhammer_kanban::scope_commands` now produce `view.set` /
/// `perspective.switch` rows directly with the view / perspective id
/// pre-filled in `args`, so no rewrite hop is needed.
fn match_dynamic_prefix(
    cmd: &str,
) -> Result<Option<(&'static str, &'static str, String, bool)>, String> {
    if let Some(suffix) = cmd.strip_prefix("board.switch:") {
        if suffix.contains("..") || !std::path::Path::new(suffix).is_absolute() {
            return Err(format!("Invalid board path in command: {:?}", suffix));
        }
        Ok(Some(("file.switchBoard", "path", suffix.to_string(), true)))
    } else if let Some(suffix) = cmd.strip_prefix("entity.add:") {
        if suffix.is_empty() {
            return Err(format!("Missing entity type in command: {:?}", cmd));
        }
        Ok(Some((
            "entity.add",
            "entity_type",
            suffix.to_string(),
            false,
        )))
    } else {
        Ok(None)
    }
}

/// Rewrite dynamic palette command prefixes to their canonical forms.
///
/// Handles `window.focus:*` (pure side-effect, returns early),
/// `board.switch:*`, and `entity.add:*` by stripping the prefix and
/// injecting the suffix as an arg. Preserves all input validation
/// (ASCII-only, `MAX_COMMAND_LENGTH`-byte limit, bounded rewrite depth).
///
/// `view.switch:{id}` and `perspective.goto:{id}` were retired in
/// 01KPZMXXEXKVE3RNPA4XJP0105 — the palette now emits `view.set` /
/// `perspective.switch` directly with args pre-filled, so no rewrite is
/// needed.
fn rewrite_dynamic_prefix(
    app: &AppHandle,
    cmd: &str,
    args: Option<Value>,
    board_path: Option<String>,
    target: &Option<String>,
    scope_chain: &Option<Vec<String>>,
) -> Result<RewriteResult, String> {
    if cmd.is_empty() || cmd.len() > MAX_COMMAND_LENGTH || !cmd.is_ascii() {
        return Err(format!("Invalid command ID: {:?}", cmd));
    }
    let mut effective_cmd = cmd.to_owned();
    let mut effective_args = args;
    let mut effective_board_path = board_path;

    for depth in 0..=MAX_REWRITE_DEPTH {
        tracing::info!(cmd = %effective_cmd, target = ?target, args = ?effective_args,
            scope_chain = ?scope_chain, board_path = ?effective_board_path, "command");

        if effective_cmd.starts_with("window.focus:") {
            return Ok(run_window_focus(
                app,
                effective_cmd,
                effective_args,
                effective_board_path,
            ));
        }
        let Some(rewrite) = match_dynamic_prefix(&effective_cmd)? else {
            break;
        };
        if depth >= MAX_REWRITE_DEPTH {
            return Err(format!(
                "Command rewrite depth exceeded for: {effective_cmd}"
            ));
        }
        apply_prefix_rewrite(
            rewrite,
            &mut effective_cmd,
            &mut effective_args,
            &mut effective_board_path,
        );
    }

    Ok(RewriteResult {
        cmd: effective_cmd,
        args: effective_args,
        board_path: effective_board_path,
        early_return: None,
    })
}

/// Execute the `window.focus:*` side-effect and package it as a
/// `RewriteResult` whose `early_return` is populated so the dispatcher
/// short-circuits the normal pipeline.
fn run_window_focus(
    app: &AppHandle,
    effective_cmd: String,
    effective_args: Option<Value>,
    effective_board_path: Option<String>,
) -> RewriteResult {
    let label = effective_cmd
        .strip_prefix("window.focus:")
        .expect("caller checked prefix")
        .to_owned();
    let result = handle_window_focus(app, &label);
    RewriteResult {
        cmd: effective_cmd,
        args: effective_args,
        board_path: effective_board_path,
        early_return: Some(result),
    }
}

/// Apply one `match_dynamic_prefix` result: rewrite `effective_cmd` to the
/// canonical form, merge the dynamic argument into `effective_args`, and
/// (when the prefix implies a board change) also rewrite
/// `effective_board_path` so downstream multi-window targeting is correct.
fn apply_prefix_rewrite(
    rewrite: (&'static str, &'static str, String, bool),
    effective_cmd: &mut String,
    effective_args: &mut Option<Value>,
    effective_board_path: &mut Option<String>,
) {
    let (new_cmd, arg_key, arg_val, update_bp) = rewrite;
    let mut merged = match effective_args.take() {
        Some(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    merged.insert(arg_key.into(), Value::String(arg_val.clone()));
    if update_bp {
        *effective_board_path = Some(arg_val);
    }
    *effective_cmd = new_cmd.to_owned();
    *effective_args = Some(Value::Object(merged));
}

/// Internal dispatch — single path for all state-mutating command execution.
///
/// This is the single path for all state-mutating command execution.
/// It handles: command lookup, context building, execution, undo tracking,
/// entity flush, event emission, and UIState change broadcasting.
///
/// Dynamic prefix commands (`board.switch:*`, `entity.add:*`) are rewritten
/// to their canonical command IDs via a single-pass loop. The rewrite is
/// limited to one iteration (`MAX_REWRITE_DEPTH`) to prevent unbounded
/// recursion from malformed command chains like `board.switch:board.switch:…`.
///
/// `view.switch:{id}` and `perspective.goto:{id}` used to travel this path
/// and were retired in 01KPZMXXEXKVE3RNPA4XJP0105 — the palette now emits
/// `view.set` / `perspective.switch` directly with args pre-filled.
///
/// The `window.focus:*` prefix is a pure side-effect (unminimize + focus) that
/// returns early without entering the standard result-processing pipeline.
/// This is intentional: window focus is OS-level and has no undo, UIState, or
/// entity implications.
///
/// # Parameters
/// - `app` - Tauri application handle for event emission
/// - `state` - Application state with command registry, impls, and board handles
/// - `cmd` - Command ID string (e.g. "task.move")
/// - `scope_chain` - Optional explicit scope; falls back to stored UIState focus
/// - `target` - Optional target entity ID
/// - `args` - Optional JSON object of command arguments
/// - `board_path` - Optional board path for multi-window targeting
pub(crate) async fn dispatch_command_internal(
    app: &AppHandle,
    state: &AppState,
    cmd: &str,
    scope_chain: Option<Vec<String>>,
    target: Option<String>,
    args: Option<Value>,
    board_path: Option<String>,
) -> Result<Value, String> {
    // Rewrite the remaining dynamic prefixes (`window.focus:*`,
    // `board.switch:*`, `entity.add:*`) to canonical commands with merged
    // args. Also validates command ID. `view.switch:*` /
    // `perspective.goto:*` are emitted as `view.set` / `perspective.switch`
    // directly (see 01KPZMXXEXKVE3RNPA4XJP0105) so no rewrite hop applies.
    let rw = rewrite_dynamic_prefix(app, cmd, args, board_path, &target, &scope_chain)?;
    if let Some(result) = rw.early_return {
        return Ok(result);
    }
    let scope = scope_chain.unwrap_or_else(|| state.ui_state.scope_chain());
    tracing::debug!(scope = ?scope, "resolved scope chain");
    let effective_cmd = rw.cmd;
    let undoable = lookup_undoable(state, &effective_cmd).await?;
    let (ctx, active_handle) = build_dispatch_context(
        state,
        app,
        effective_cmd.clone(),
        rw.args,
        scope,
        target,
        rw.board_path,
    )
    .await;
    let result = dispatch_via_service_or_fallback(state, app, &effective_cmd, &ctx, active_handle.as_ref())
        .await?;
    tracing::info!(cmd = %effective_cmd, undoable, result = %result, "command completed");

    // Undo stack push is handled automatically inside EntityContext::write()/delete()
    // (wired in the entity crate). No need to push at the dispatch level.
    apply_post_command_side_effects(
        app,
        state,
        &effective_cmd,
        undoable,
        active_handle.as_ref(),
        &result,
    )
    .await;
    menu::update_menu_enabled_state(state);

    Ok(json!({ "result": result, "undoable": undoable }))
}

/// Hybrid dispatch: try the new `CommandService` first; fall back to the
/// legacy `command_impls` path on "unknown command".
///
/// The new path scopes `CURRENT_STORE_CTX` and `CURRENT_ENTITY_BOARD_SERVICES`
/// around `CommandService::dispatch`, so the in-process `store`/`entity` MCP
/// surfaces route per-call to the active board's substrate. Errors that are
/// NOT "no registration for this id" propagate as the dispatcher's error; the
/// fallback only fires when the command isn't registered in the service yet.
///
/// During the hybrid window (sub-stage 2e) `command_impls` is the source of
/// truth for ~all commands — only the 7 builtin command-plugin ids that the
/// new path already registers will land on the new branch. Once Stage 4
/// retires `command_impls`, this helper collapses to just the new path.
async fn dispatch_via_service_or_fallback(
    state: &AppState,
    app: &AppHandle,
    effective_cmd: &str,
    ctx: &swissarmyhammer_commands::CommandContext,
    active_handle: Option<&Arc<BoardHandle>>,
) -> Result<Value, String> {
    let new_result =
        try_dispatch_via_command_service(state, app, effective_cmd, ctx, active_handle).await;
    match new_result {
        Some(Ok(value)) => Ok(value),
        Some(Err(err)) if is_unknown_command_error(&err) => {
            tracing::debug!(
                cmd = %effective_cmd,
                "command not registered in CommandService — falling back to command_impls"
            );
            execute_registered_command(state, effective_cmd, ctx).await
        }
        Some(Err(err)) => Err(format!("Command failed: {err}")),
        None => execute_registered_command(state, effective_cmd, ctx).await,
    }
}

/// Attempt the new path. Returns `None` when no `CommandService` is wired
/// (test fixtures / failed bootstrap); the caller falls back unconditionally
/// in that case.
async fn try_dispatch_via_command_service(
    state: &AppState,
    app: &AppHandle,
    effective_cmd: &str,
    ctx: &swissarmyhammer_commands::CommandContext,
    active_handle: Option<&Arc<BoardHandle>>,
) -> Option<Result<Value, rmcp::ErrorData>> {
    let service = state.plugin_platform.lock().await.command_service()?;

    let req = swissarmyhammer_command_service::ExecuteCommand {
        id: effective_cmd.to_string(),
        ctx: swissarmyhammer_command_service::CommandContext {
            scope_chain: ctx.scope_chain.clone(),
            target: ctx.target.clone(),
            args: ctx.args.clone(),
        },
        force: None,
    };

    // Build the per-board entity-services bundle. When no board is active
    // (e.g. command fired before a board is open), drop the entity_ctx-bound
    // services entirely — the entity tool surface will then return its
    // "no board scoped" structured error rather than panic.
    let entity_services = match active_handle {
        Some(handle) => {
            let entity_ctx = match handle.ctx.entity_context().await {
                Ok(ectx) => ectx,
                Err(e) => {
                    tracing::warn!(
                        cmd = %effective_cmd,
                        error = %e,
                        "failed to resolve entity_context for new-path dispatch"
                    );
                    return Some(Err(rmcp::ErrorData::internal_error(
                        format!("entity_context unavailable: {e}"),
                        None,
                    )));
                }
            };
            Some(swissarmyhammer_entity_mcp::server::EntityBoardServices {
                entity_ctx,
                kanban: Some(Arc::clone(&handle.ctx)),
                // Wire the same TauriClipboardProvider the legacy build_dispatch_context
                // installs as a ClipboardProviderExt — the entity MCP clipboard ops
                // (cut/copy/paste) read it via the resolver.
                clipboard: Some(Arc::new(crate::state::TauriClipboardProvider::new(
                    app.clone(),
                ))),
                ui_state: Some(Arc::clone(&state.ui_state)),
            })
        }
        None => None,
    };

    let store_ctx = active_handle.map(|h| Arc::clone(&h.store_context));

    // Capture `service` by move into the dispatch future so the lock guard
    // above doesn't need to outlive the await.
    let dispatched = async move {
        service
            .dispatch(swissarmyhammer_plugin::CallerId::HostInternal, req)
            .await
    };

    // Scope both task-locals around the dispatch when we have them; otherwise
    // run dispatch bare and let the resolvers return their structured
    // "not scoped" errors for ops that need them.
    let raw = match (store_ctx, entity_services) {
        (Some(sctx), Some(esvc)) => {
            swissarmyhammer_kanban::command_seam::scope_store_context(
                sctx,
                swissarmyhammer_entity_mcp::server::scope_entity_board_services(esvc, dispatched),
            )
            .await
        }
        (Some(sctx), None) => {
            swissarmyhammer_kanban::command_seam::scope_store_context(sctx, dispatched).await
        }
        (None, Some(esvc)) => {
            swissarmyhammer_entity_mcp::server::scope_entity_board_services(esvc, dispatched).await
        }
        (None, None) => dispatched.await,
    };

    // The service wraps successful results as `{ "ok": true, "result": <value> }`.
    // Unwrap to match the legacy `execute_registered_command` contract
    // (which returns just the inner result value).
    Some(raw.map(|v| {
        v.get("result").cloned().unwrap_or(v)
    }))
}

/// True iff `err` is the `CommandService`'s `UnknownCommand` error.
///
/// `command_error_to_mcp` in `swissarmyhammer-command-service` projects every
/// `CommandError` variant into `err.data` as `{ "kind": "<VariantName>", ... }`
/// — see `command_error_data` in service.rs. So the structured discriminant
/// is reliable and we don't have to grep the human-readable `message`.
fn is_unknown_command_error(err: &rmcp::ErrorData) -> bool {
    err.data
        .as_ref()
        .and_then(|d| d.get("kind"))
        .and_then(|k| k.as_str())
        == Some("UnknownCommand")
}

/// Look up `effective_cmd`'s implementation, check availability, and execute
/// it. Errors unify the "no impl", "not available", and "execute failed"
/// paths into the `String` return type so the caller can `?` them.
async fn execute_registered_command(
    state: &AppState,
    effective_cmd: &str,
    ctx: &swissarmyhammer_commands::CommandContext,
) -> Result<Value, String> {
    let cmd_impl = state
        .command_impls
        .get(effective_cmd)
        .ok_or_else(|| format!("No implementation for command: {}", effective_cmd))?;
    if !cmd_impl.available(ctx) {
        tracing::warn!(cmd = %effective_cmd, "command not available in current context");
        return Err(format!("Command not available: {}", effective_cmd));
    }
    tracing::debug!(cmd = %effective_cmd, "executing command");
    cmd_impl.execute(ctx).await.map_err(|e| {
        tracing::error!(cmd = %effective_cmd, error = %e, "command execution failed");
        format!("Command failed: {e}")
    })
}

/// Run every post-execute side-effect — board management, drag events,
/// UI-state snapshot emit, menu rebuild, and flush/undo-redo sync — in the
/// fixed order the dispatcher relied on before this was extracted.
///
/// Split out so `dispatch_command_internal` stays under the project's
/// function-length budget; semantics are identical to calling each helper
/// in this exact order.
async fn apply_post_command_side_effects(
    app: &AppHandle,
    state: &AppState,
    effective_cmd: &str,
    undoable: bool,
    active_handle: Option<&Arc<BoardHandle>>,
    result: &Value,
) {
    handle_board_switch_result(app, state, effective_cmd, result).await;
    handle_board_close_result(app, state, effective_cmd, result).await;
    handle_ui_trigger_results(app, state, result).await;
    handle_drag_events(app, state, active_handle, result).await;
    emit_ui_state_change_if_needed(app, state, result);
    maybe_rebuild_menu_after_cmd(app, effective_cmd, result).await;
    flush_and_sync_after_command(app, state, effective_cmd, undoable, active_handle).await;
}

/// Read `undoable` for a command from the registry without holding the read
/// guard across the subsequent async `execute` call.
async fn lookup_undoable(state: &AppState, effective_cmd: &str) -> Result<bool, String> {
    let registry = state.commands_registry.read().await;
    let cmd_def = registry
        .get(effective_cmd)
        .ok_or_else(|| format!("Unknown command: {}", effective_cmd))?;
    Ok(cmd_def.undoable)
}

/// Build a `CommandContext` for `effective_cmd`, attach the relevant board
/// and clipboard extensions, and resolve the active board handle (if any)
/// so the caller can reuse it for post-execute flushing.
///
/// The handle resolution prefers `effective_board_path` (multi-window
/// targeting) and falls back to the `store:` moniker in the scope chain.
async fn build_dispatch_context(
    state: &AppState,
    app: &AppHandle,
    effective_cmd: String,
    effective_args: Option<Value>,
    scope: Vec<String>,
    target: Option<String>,
    effective_board_path: Option<String>,
) -> (
    swissarmyhammer_commands::CommandContext,
    Option<Arc<BoardHandle>>,
) {
    let args_map: HashMap<String, Value> = match effective_args {
        Some(Value::Object(map)) => map.into_iter().collect(),
        _ => HashMap::new(),
    };
    let mut ctx =
        swissarmyhammer_commands::CommandContext::new(effective_cmd, scope, target, args_map);
    ctx = ctx.with_ui_state(Arc::clone(&state.ui_state));

    let resolved_board_path =
        effective_board_path.or_else(|| ctx.resolve_store_path().map(|s| s.to_string()));
    let active_handle = resolve_handle(state, resolved_board_path).await.ok();
    if let Some(ref handle) = active_handle {
        ctx.set_extension(Arc::clone(&handle.ctx));
        if let Ok(ectx_arc) = handle.ctx.entity_context().await {
            ctx.set_extension(ectx_arc);
        }
        ctx.set_extension(Arc::clone(&handle.store_context));
    }

    // Inject ClipboardProvider so commands can read/write the system clipboard.
    // Wrapped in ClipboardProviderExt (a sized newtype) for CommandContext storage.
    let clipboard_ext = swissarmyhammer_kanban::clipboard::ClipboardProviderExt(Arc::new(
        crate::state::TauriClipboardProvider::new(app.clone()),
    ));
    ctx.set_extension(Arc::new(clipboard_ext));

    (ctx, active_handle)
}

/// Apply the `BoardSwitch` side-effect: open the target board, persist the
/// window→board mapping, refresh the window title, and emit `board-changed`.
///
/// Only the Tauri layer can manage `BoardHandle`s, so although `file.switchBoard`
/// already updated `UIState`, we still need to run this side-effect here.
async fn handle_board_switch_result(
    app: &AppHandle,
    state: &AppState,
    effective_cmd: &str,
    result: &Value,
) {
    let Some(board_switch) = result.get("BoardSwitch") else {
        return;
    };
    let Some(path_str) = board_switch.get("path").and_then(|v| v.as_str()) else {
        return;
    };
    let board_path = std::path::PathBuf::from(path_str);
    let label = board_switch
        .get("window_label")
        .and_then(|v| v.as_str())
        .unwrap_or("main");

    match state.open_board(&board_path, Some(app.clone())).await {
        Ok(canonical) => {
            state
                .ui_state
                .set_window_board(label, &canonical.display().to_string());
            let boards = state.boards.read().await;
            if let Some(handle) = boards.get(&canonical) {
                let name = swissarmyhammer_kanban::board_display_name(&handle.ctx).await;
                update_window_title(app, label, name.as_deref());
            }
        }
        Err(e) => {
            tracing::error!(cmd = %effective_cmd, path = %path_str, error = %e, "BoardSwitch: failed to open board");
        }
    }
    let _ = app.emit("board-changed", ());
}

/// Apply the `BoardClose` side-effect: drop the board handle (if this window
/// was the last viewer), close the requesting window, and emit
/// `board-changed`. Keeps the window open when it's the only visible window,
/// so the user is never left staring at a closed app.
async fn handle_board_close_result(
    app: &AppHandle,
    state: &AppState,
    effective_cmd: &str,
    result: &Value,
) {
    let Some(board_close) = result.get("BoardClose") else {
        return;
    };
    let Some(path_str) = board_close.get("path").and_then(|v| v.as_str()) else {
        return;
    };
    let requesting_label = board_close
        .get("window_label")
        .and_then(|v| v.as_str())
        .unwrap_or("main")
        .to_string();

    drop_or_detach_board(state, effective_cmd, path_str, &requesting_label).await;
    close_or_retitle_window(app, &requesting_label);
    let _ = app.emit("board-changed", ());
}

/// Drop the board handle when this is the last window showing it; otherwise
/// just clear the requesting window's assignment so other windows keep
/// running.
async fn drop_or_detach_board(
    state: &AppState,
    effective_cmd: &str,
    path_str: &str,
    requesting_label: &str,
) {
    let windows_showing: Vec<String> = state
        .ui_state
        .all_window_boards()
        .into_iter()
        .filter(|(_, bp)| bp == path_str)
        .map(|(label, _)| label)
        .collect();

    if windows_showing.len() <= 1 {
        let target = std::path::PathBuf::from(path_str);
        if let Err(e) = state.close_board(&target).await {
            tracing::error!(cmd = %effective_cmd, path = %path_str, error = %e, "BoardClose: failed to close board");
        }
        state.ui_state.remove_open_board(path_str);
    } else {
        state.ui_state.set_window_board(requesting_label, "");
    }
}

/// Close the requesting window unless it's the last visible window — in
/// which case keep it open with a cleared title so the user is not left
/// staring at a closed app.
fn close_or_retitle_window(app: &AppHandle, requesting_label: &str) {
    let visible_windows: Vec<_> = app
        .webview_windows()
        .into_iter()
        .filter(|(label, w)| label != "quick-capture" && w.is_visible().unwrap_or(false))
        .collect();

    if visible_windows.len() > 1 {
        if let Some(win) = app.get_webview_window(requesting_label) {
            let _ = win.close();
        }
    } else {
        update_window_title(app, requesting_label, None);
    }
}

/// Apply UI-triggering command results: file dialogs, new-window creation,
/// app quit, and UI-broadcast envelopes (e.g. focus signals). Each result
/// variant is dispatched to a dedicated handler so the side-effect for each
/// shape lives in one place and can be tested or changed in isolation.
async fn handle_ui_trigger_results(app: &AppHandle, state: &AppState, result: &Value) {
    handle_new_board_dialog(app, result);
    handle_open_board_dialog(app, result);
    handle_create_window(app, state, result).await;
    handle_quit(app, result);
}

/// Trigger the native "new board" file dialog when the command emitted a
/// `NewBoardDialog` result envelope. Fire-and-forget — the dialog drives any
/// follow-up command on completion.
fn handle_new_board_dialog(app: &AppHandle, result: &Value) {
    if result.get("NewBoardDialog").is_some() {
        menu::trigger_new_board(app);
    }
}

/// Trigger the native "open board" file dialog when the command emitted an
/// `OpenBoardDialog` result envelope. Fire-and-forget — the dialog drives any
/// follow-up command on completion.
fn handle_open_board_dialog(app: &AppHandle, result: &Value) {
    if result.get("OpenBoardDialog").is_some() {
        menu::trigger_open_board(app);
    }
}

/// Spawn a new Tauri window when the command emitted a `CreateWindow` result.
/// Delegates to `create_window_internal`, which resolves the board path and
/// applies any persisted geometry.
async fn handle_create_window(app: &AppHandle, state: &AppState, result: &Value) {
    if result.get("CreateWindow").is_some() {
        create_window_internal(app, state).await;
    }
}

/// Exit the Tauri app when the command emitted a `quit` result envelope.
fn handle_quit(app: &AppHandle, result: &Value) {
    if result.get("quit").is_some() {
        app.exit(0);
    }
}

/// Emit all drag-session events. `DragStart`/`DragCancel` are simple
/// forwarding emits; `DragComplete` delegates to `handle_drag_complete`
/// which flushes the affected boards and emits `drag-session-completed`.
async fn handle_drag_events(
    app: &AppHandle,
    state: &AppState,
    active_handle: Option<&Arc<BoardHandle>>,
    result: &Value,
) {
    handle_drag_start(app, result);
    handle_drag_cancel(app, result);
    if let Some(drag_complete) = result.get("DragComplete") {
        handle_drag_complete(app, state, active_handle, drag_complete).await;
    }
}

/// Forward the `DragStart` payload to the frontend on the
/// `drag-session-active` channel. No-op when the result does not carry a
/// `DragStart` envelope.
fn handle_drag_start(app: &AppHandle, result: &Value) {
    if let Some(drag_start) = result.get("DragStart") {
        let _ = app.emit("drag-session-active", drag_start);
    }
}

/// Forward the `DragCancel` payload to the frontend on the
/// `drag-session-cancelled` channel. No-op when the result does not carry a
/// `DragCancel` envelope.
fn handle_drag_cancel(app: &AppHandle, result: &Value) {
    if let Some(drag_cancel) = result.get("DragCancel") {
        let _ = app.emit("drag-session-cancelled", drag_cancel);
    }
}

/// Handle the `drag.complete` side-effects: same-board flushes the single
/// board (the task.move already ran inside `DragCompleteCmd`), cross-board
/// routes through `transfer_task` and flushes both boards. Always emits
/// `drag-session-completed` with a success flag.
///
/// Same-board: `undoable=false` on `drag.complete`, so the regular
/// post-command flush at the bottom of `dispatch_command_internal` would
/// skip this board — we flush explicitly here to ship the entity events.
async fn handle_drag_complete(
    app: &AppHandle,
    state: &AppState,
    active_handle: Option<&Arc<BoardHandle>>,
    drag_complete: &Value,
) {
    let session_id = drag_complete
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let cross_board = drag_complete
        .get("cross_board")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let transfer_ok = if cross_board {
        perform_cross_board_drag_transfer(app, state, drag_complete).await
    } else {
        // Same-board drag: writes already went through `EntityCache::write`,
        // which emits `EntityChanged` synchronously on its broadcast channel.
        // The bridge subscriber forwards those to Tauri — no extra flush
        // needed here.
        let _ = active_handle;
        true
    };

    let _ = app.emit(
        "drag-session-completed",
        json!({
            "session_id": session_id,
            "success": transfer_ok,
        }),
    );
}

/// Parsed parameters for a cross-board drag transfer. The drag-complete
/// payload is a loosely-typed `Value` from the frontend; this struct is the
/// owned, typed shape that `perform_cross_board_drag_transfer` works with.
struct CrossBoardDragParams {
    source_path: String,
    target_path: String,
    task_id: String,
    target_column: String,
    drop_index: Option<u64>,
    before_id: Option<String>,
    after_id: Option<String>,
    copy_mode: bool,
}

impl CrossBoardDragParams {
    /// Extract every cross-board drag parameter from the `DragComplete`
    /// payload. Missing string fields default to empty so `transfer_task`'s
    /// own validation can produce a consistent error message.
    fn from_value(drag_complete: &Value) -> Self {
        let get_str = |key: &str| {
            drag_complete
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let get_opt_str = |key: &str| {
            drag_complete
                .get(key)
                .and_then(|v| v.as_str())
                .map(str::to_string)
        };
        Self {
            source_path: get_str("source_board_path"),
            target_path: get_str("target_board_path"),
            task_id: get_str("task_id"),
            target_column: get_str("target_column"),
            drop_index: drag_complete.get("drop_index").and_then(|v| v.as_u64()),
            before_id: get_opt_str("before_id"),
            after_id: get_opt_str("after_id"),
            copy_mode: drag_complete
                .get("copy_mode")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }
    }
}

/// Cross-board drag: resolve both source and target board handles, run
/// `transfer_task`, and flush both boards (only the source when the drag
/// was a copy, so entity events propagate to every listening window).
async fn perform_cross_board_drag_transfer(
    app: &AppHandle,
    state: &AppState,
    drag_complete: &Value,
) -> bool {
    let params = CrossBoardDragParams::from_value(drag_complete);
    let source_handle = resolve_handle(state, Some(params.source_path.clone())).await;
    let target_handle = resolve_handle(state, Some(params.target_path.clone())).await;
    let (src, tgt) = match (source_handle, target_handle) {
        (Ok(src), Ok(tgt)) => (src, tgt),
        _ => {
            tracing::error!(
                "drag.complete: failed to resolve board handles for cross-board transfer"
            );
            return false;
        }
    };

    let transfer_result = swissarmyhammer_kanban::cross_board::transfer_task(
        &src.ctx,
        &tgt.ctx,
        &params.task_id,
        &params.target_column,
        params.drop_index,
        params.before_id.as_deref(),
        params.after_id.as_deref(),
        params.copy_mode,
    )
    .await;

    let ok = transfer_result.is_ok();
    // Cross-board writes go through each board's `EntityCache`, which emits
    // events synchronously. The bridge subscriber for each board already
    // forwarded them to Tauri — no extra flush needed here.
    let _ = (app, tgt, src);
    ok
}

/// Emit a fresh `ui-state-changed` event when the command either returned a
/// `UIStateChange` result envelope or mutated board open/close state (which
/// is not typed as a `UIStateChange` but still affects what the React
/// `UIStateProvider` renders).
///
/// The emitted payload is a wrapper of the form
/// `{ "kind": "<discriminator>", "state": <full UIState snapshot> }`.
/// `kind` names which slice of UI state changed — one of the seven
/// `UIStateChange` variants plus the two board result shapes — so the
/// frontend can skip `setState` for events it doesn't care about (e.g.
/// every `ui.setFocus` arrow-key fires a `scope_chain` event; the
/// frontend owns that slice via `FocusedScopeContext` and ignores the
/// echo). No UI-specific policy lives here — the backend just tells the
/// truth about which change it made.
fn emit_ui_state_change_if_needed(app: &AppHandle, state: &AppState, result: &Value) {
    let Some(kind) = ui_state_change_kind(result) else {
        return;
    };
    let _ = app.emit(
        "ui-state-changed",
        serde_json::json!({ "kind": kind, "state": state.ui_state.to_json() }),
    );
}

/// Classify a command result into a `ui-state-changed` discriminator, or
/// `None` if the result does not trigger a UI state event.
///
/// Returns the `kind` string used on the wire:
/// - One per `UIStateChange` variant (`scope_chain`, `palette_open`,
///   `keymap_mode`, `inspector_stack`, `active_view`,
///   `active_perspective`, `app_mode`, `inspector_width`,
///   `perspective_switch`).
/// - `board_switch` / `board_close` for the two board result shapes,
///   which are not typed as `UIStateChange` but still mutate what the
///   `UIStateProvider` renders.
fn ui_state_change_kind(result: &Value) -> Option<&'static str> {
    if let Ok(change) =
        serde_json::from_value::<swissarmyhammer_ui_state::UIStateChange>(result.clone())
    {
        return Some(match change {
            swissarmyhammer_ui_state::UIStateChange::ScopeChain(_) => "scope_chain",
            swissarmyhammer_ui_state::UIStateChange::PaletteOpen(_) => "palette_open",
            swissarmyhammer_ui_state::UIStateChange::KeymapMode(_) => "keymap_mode",
            swissarmyhammer_ui_state::UIStateChange::InspectorStack(_) => "inspector_stack",
            swissarmyhammer_ui_state::UIStateChange::ActiveView(_) => "active_view",
            swissarmyhammer_ui_state::UIStateChange::ActivePerspective(_) => "active_perspective",
            swissarmyhammer_ui_state::UIStateChange::AppMode(_) => "app_mode",
            swissarmyhammer_ui_state::UIStateChange::InspectorWidth { .. } => "inspector_width",
            // `PerspectiveSwitch` is the atomic id+filtered-ids update emitted
            // by `perspective.switch`. We classify it as `perspective_switch`
            // so the frontend can register its own debounce/skip policy
            // independently of the legacy `active_perspective` kind (which
            // covered id-only mutations).
            swissarmyhammer_ui_state::UIStateChange::PerspectiveSwitch { .. } => {
                "perspective_switch"
            }
        });
    }
    if result.get("BoardSwitch").is_some() {
        return Some("board_switch");
    }
    if result.get("BoardClose").is_some() {
        return Some("board_close");
    }
    None
}

/// Rebuild the native menu after commands whose effects change what items
/// are enabled or their accelerator mappings: keymap mode changes, focus
/// changes, and board switch/close.
async fn maybe_rebuild_menu_after_cmd(app: &AppHandle, effective_cmd: &str, result: &Value) {
    if effective_cmd.starts_with("settings.keymap.")
        || effective_cmd == "ui.setFocus"
        || result.get("BoardSwitch").is_some()
        || result.get("BoardClose").is_some()
    {
        menu::rebuild_menu_async(app).await;
    }
}

/// Post-command sync: refresh cached undo/redo flags and window titles
/// after a mutating command ran.
///
/// Entity changes no longer need an explicit "flush" step here: writes
/// flow through `EntityCache::write`, which emits `EntityChanged` events
/// on its broadcast channel synchronously. The bridge task (started in
/// `BoardHandle::start_watcher`) already forwarded those to Tauri. The
/// work that remains is app-level: undo/redo flags depend on the store
/// context, and window titles reflect the (possibly just-renamed) board
/// entity.
///
/// Runs for undoable commands **and** for `app.undo`/`app.redo`: both
/// mutate entities on disk but are themselves non-undoable, so they'd
/// otherwise skip this sync.
async fn flush_and_sync_after_command(
    app: &AppHandle,
    state: &AppState,
    effective_cmd: &str,
    undoable: bool,
    active_handle: Option<&Arc<BoardHandle>>,
) {
    let needs_sync = undoable || effective_cmd == "app.undo" || effective_cmd == "app.redo";
    if !needs_sync {
        return;
    }
    let Some(handle) = active_handle else {
        return;
    };
    state.ui_state.set_undo_redo_state(
        handle.store_context.can_undo().await,
        handle.store_context.can_redo().await,
    );
    refresh_board_window_titles(app, state, handle).await;
}

/// Update every window title that currently points at `handle`'s board to
/// match the board entity's display name. Catches board renames, undo/redo
/// of name changes, etc.
async fn refresh_board_window_titles(app: &AppHandle, state: &AppState, handle: &BoardHandle) {
    let display_name = swissarmyhammer_kanban::board_display_name(&handle.ctx).await;
    let canonical = handle
        .ctx
        .root()
        .canonicalize()
        .unwrap_or_else(|_| handle.ctx.root().to_path_buf());
    let board_path_str = canonical.display().to_string();
    for (label, wbp) in state.ui_state.all_window_boards() {
        if wbp == board_path_str {
            update_window_title(app, &label, display_name.as_deref());
        }
    }
}

// ---------------------------------------------------------------------------
// list_commands_for_scope — backend-driven command resolution
// ---------------------------------------------------------------------------

/// Build a `DynamicSources` for the current app state (views, boards,
/// windows, perspectives).
///
/// This is a thin Tauri-side shim around the headless
/// [`swissarmyhammer_kanban::dynamic_sources::build_dynamic_sources`]
/// entry point. All assembly logic (views/boards/perspectives + active
/// view kind) lives in the kanban crate so it can be exercised without
/// Tauri scaffolding; this shim contributes the one input that can only
/// come from the GUI runtime — live window titles/visibility/focus.
async fn build_dynamic_sources(
    app: &AppHandle,
    state: &AppState,
    active_handle: Option<&crate::state::BoardHandle>,
) -> swissarmyhammer_kanban::scope_commands::DynamicSources {
    use swissarmyhammer_kanban::dynamic_sources::{
        build_dynamic_sources as build_headless, DynamicSourcesInputs,
    };
    let windows = gather_windows(app);
    // Project the `HashMap<PathBuf, Arc<BoardHandle>>` down to
    // `HashMap<PathBuf, Arc<KanbanContext>>` — the headless builder only
    // needs the context, not the full handle (entity cache, bridge task,
    // search index are irrelevant to dynamic-source assembly).
    let boards_guard = state.boards.read().await;
    let open_board_ctxs: std::collections::HashMap<
        PathBuf,
        Arc<swissarmyhammer_kanban::KanbanContext>,
    > = boards_guard
        .iter()
        .map(|(path, handle)| (path.clone(), Arc::clone(&handle.ctx)))
        .collect();
    drop(boards_guard);
    let active_ctx = active_handle.map(|h| h.ctx.as_ref());
    build_headless(DynamicSourcesInputs {
        ui_state: &state.ui_state,
        active_ctx,
        open_board_ctxs: &open_board_ctxs,
        active_window_label: Some("main"),
        windows,
        ai_models: gather_ai_models(),
    })
    .await
}

/// Enumerate the selectable AI models and project them onto the
/// kanban-side [`AiModelInfo`] shape the `ai.models` options resolver
/// consumes.
///
/// The model set is discovered by `swissarmyhammer-config`'s
/// `ModelManager` — a GUI-runtime concern the pure-domain kanban crate
/// does not depend on, so the enumeration happens here and the
/// projected list is threaded into `DynamicSources`. The full
/// [`crate::ai::models::Model`] carries richer metadata (`kind`,
/// `available`, `hint`); the picker only needs `id` + `label`.
///
/// On enumeration failure the model picker degrades to an empty list
/// (the same graceful-degradation the resolver already tolerates) — a
/// transient agent-discovery error must not break `list_commands_for_scope`.
fn gather_ai_models() -> Vec<swissarmyhammer_kanban::commands::options_resolvers::AiModelInfo> {
    use swissarmyhammer_kanban::commands::options_resolvers::AiModelInfo;
    match crate::ai::models::ai_list_models() {
        Ok(models) => models
            .into_iter()
            .map(|m| AiModelInfo {
                id: m.id,
                label: m.label,
            })
            .collect(),
        Err(e) => {
            tracing::warn!(error = %e, "ai model enumeration failed; ai.model picker will be empty");
            Vec::new()
        }
    }
}

/// Emit `info`-level telemetry about the resolved-command list so a
/// "no entity.add" bug is diagnosable from logs alone.
fn log_scope_inputs(
    scope_chain: &[String],
    context_menu: Option<bool>,
    dynamic: &swissarmyhammer_kanban::scope_commands::DynamicSources,
) {
    let views_with_entity_type = dynamic
        .views
        .iter()
        .filter(|v| v.entity_type.as_deref().is_some_and(|s| !s.is_empty()))
        .count();
    tracing::info!(
        scope_chain = ?scope_chain,
        context_menu = ?context_menu,
        views_count = dynamic.views.len(),
        views_with_entity_type,
        boards_count = dynamic.boards.len(),
        windows_count = dynamic.windows.len(),
        perspectives_count = dynamic.perspectives.len(),
        "list_commands_for_scope"
    );
}

fn log_scope_result(result: &[swissarmyhammer_kanban::scope_commands::ResolvedCommand]) {
    if !tracing::enabled!(tracing::Level::INFO) {
        return;
    }
    let mut by_group: HashMap<&str, usize> = HashMap::new();
    for cmd in result {
        *by_group.entry(cmd.group.as_str()).or_default() += 1;
    }
    let entity_add_ids: Vec<&str> = result
        .iter()
        .filter(|c| c.id.starts_with("entity.add:"))
        .map(|c| c.id.as_str())
        .collect();
    tracing::info!(
        total = result.len(),
        by_group = ?by_group,
        entity_add_ids = ?entity_add_ids,
        "list_commands_for_scope result"
    );
}

#[tauri::command]
pub async fn list_commands_for_scope(
    app: AppHandle,
    state: State<'_, AppState>,
    scope_chain: Vec<String>,
    context_menu: Option<bool>,
) -> Result<Value, String> {
    let active_handle = state.active_handle().await;
    let registry = state.commands_registry.read().await;
    let dynamic = build_dynamic_sources(&app, &state, active_handle.as_deref()).await;
    log_scope_inputs(&scope_chain, context_menu, &dynamic);

    // Thread the active context through `commands_for_scope_with_context`
    // so the call carries BOTH the entity schemas (via `fields`) AND the
    // options resolver registry (via `options_registry`) — sourced from the
    // same context object. The earlier direct call to `commands_for_scope`
    // passed `None` for the options registry, which silently disabled the
    // enrichment pass and left every picker (Group By, View, Sort, etc.)
    // with `options: None` — the empty-popover bug tracked in kanban task
    // 01KRGW1DYD0T05PSTEDPT5D076 (iteration 4).
    let result = swissarmyhammer_kanban::scope_commands::commands_for_scope_with_context(
        &scope_chain,
        &registry,
        &state.command_impls,
        active_handle.as_ref().map(|h| h.ctx.as_ref()),
        &state.ui_state,
        context_menu == Some(true),
        Some(&dynamic),
    );

    log_scope_result(&result);
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// show_context_menu — generic native context menu
// ---------------------------------------------------------------------------

/// A single item in a generic context menu.
///
/// Each item is self-contained: it carries the command ID, target, and scope
/// chain needed for dispatch. The frontend sends all dispatch info upfront;
/// when the user selects an item, Rust emits a `context-menu-command` event
/// so the frontend routes it through `useDispatchCommand`.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct ContextMenuItem {
    /// Display name shown in the menu.
    pub name: String,
    /// Command ID to dispatch (e.g. "entity.copy"). Empty for separators.
    #[serde(default)]
    pub cmd: String,
    /// Optional target moniker (e.g. "task:01ABC").
    #[serde(default)]
    pub target: Option<String>,
    /// Scope chain from the right-click point.
    #[serde(default)]
    pub scope_chain: Vec<String>,
    /// Whether this item is a separator.
    #[serde(default)]
    pub separator: bool,
}

/// Show a native context menu with the given items.
///
/// Each item carries its full dispatch info (cmd, target, scope_chain).
/// When the user selects an item, `handle_menu_event` parses the JSON-encoded
/// ID and emits a `context-menu-command` event so the frontend routes it
/// through `useDispatchCommand` for busy tracking and scope resolution.
#[tauri::command]
pub async fn show_context_menu(
    app: AppHandle,
    window: Window,
    items: Vec<ContextMenuItem>,
) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }

    // Encode each item's dispatch info as JSON into the native menu item ID.
    // When the user selects an item, handle_menu_event parses the JSON and
    // dispatches directly — no lookup table needed.
    let mut builder = MenuBuilder::new(&app);
    for item in &items {
        if item.separator {
            builder = builder.separator();
        } else {
            let encoded = serde_json::to_string(&item).unwrap_or_default();
            builder = builder.text(encoded, &item.name);
        }
    }

    let menu = builder.build().map_err(|e| e.to_string())?;
    menu.popup(window)
        .map_err(|e: tauri::Error| e.to_string())?;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Spatial-navigation commands.
//
// These wire the headless `swissarmyhammer-focus` kernel into Tauri. Each
// command derives a `WindowLabel` from its `tauri::Window` parameter, locks
// the registry and per-window focus state held in `AppState`, performs its
// kernel call, and (where the kernel returns a `FocusChangedEvent`) emits
// `focus-changed` so the React `SpatialFocusProvider` can update its claim
// listeners.
//
// These commands intentionally bypass `dispatch_command` per the rule at the
// top of this file: they are transient UI plumbing — not business state
// mutations. The headless kernel owns the model; these commands only forward
// register / focus / navigate calls and surface the resulting events.
//
// ## Lock ordering
//
// Every spatial command that touches both the registry and per-window focus
// holds the locks in the same order: **registry first, state second**. The
// helper [`with_spatial`] enforces this so callers cannot deadlock by
// accident. The unregister command intentionally takes both locks together
// for the duration of the transaction so observers (focus listeners,
// fallback computations) cannot see a half-applied unregister.
// ─────────────────────────────────────────────────────────────────────────────

use swissarmyhammer_focus::{
    Direction, FocusChangedEvent, FocusLayer, FullyQualifiedMoniker, LayerName, NavSnapshot, Rect,
    SegmentMoniker, SpatialRegistry, SpatialState, WindowLabel,
};

/// Tauri event name for spatial focus changes — mirrors the listener
/// registered in `kanban-app/ui/src/lib/spatial-focus-context.tsx`.
const FOCUS_CHANGED_EVENT: &str = "focus-changed";

/// Derive a [`WindowLabel`] newtype from a Tauri window handle.
///
/// `tauri::Window::label()` returns the borrowed string the user-space
/// constructor mints (`"main"`, `"board-<ulid>"`, `"quick-capture"`, …);
/// the kernel speaks in newtypes, so we wrap it at the boundary. Every
/// spatial command funnels through this helper so a stray `String` cannot
/// leak into the kernel surface.
fn window_label_from(window: &Window) -> WindowLabel {
    WindowLabel::from_string(window.label())
}

/// Acquire both spatial locks in canonical order and run `f` with mutable
/// access.
///
/// Order is `spatial_registry` then `spatial_state` for any command that
/// holds both at once. Centralizing the order here means every adapter
/// inherits it for free and cannot accidentally lock-invert.
async fn with_spatial<R, F>(state: &State<'_, AppState>, f: F) -> R
where
    F: FnOnce(&mut SpatialRegistry, &mut SpatialState) -> R,
{
    let mut registry = state.spatial_registry.lock().await;
    let mut spatial_state = state.spatial_state.lock().await;
    f(&mut registry, &mut spatial_state)
}

/// Forward a kernel-produced [`FocusChangedEvent`] to the React side of
/// the **one** window the focus change belongs to.
///
/// Emits with `emit_to(event.window_label, …)` rather than the broadcast
/// `window.emit(…)`. The distinction is load-bearing: `FullyQualifiedMoniker`s
/// are NOT unique across windows — every window's root layer is `/window`,
/// so a card is `/window/.../task:Z` in every window showing that board.
/// A broadcast would reach every webview, and each window's claim registry
/// would match the identically-keyed scope and light it up — the "jump
/// highlights all windows" bug. The focus map (`SpatialState::focus_by_window`)
/// is per-window, so `event.window_label` is the sole correct recipient;
/// targeting it confines the highlight to the window that actually moved
/// focus.
fn emit_focus_changed(window: &Window, event: &FocusChangedEvent) -> Result<(), String> {
    window
        .emit_to(event.window_label.as_str(), FOCUS_CHANGED_EVENT, event)
        .map_err(|e| format!("failed to emit {FOCUS_CHANGED_EVENT}: {e}"))
}

// ── Pure inner logic, factored out of the Tauri commands so unit tests can
// drive the same code paths against `&mut SpatialRegistry, &mut SpatialState`
// without spinning up Tauri. The Tauri commands below are thin wrappers that
// derive the [`WindowLabel`] from `tauri::Window`, lock the registry/state in
// canonical order, dispatch to one of these helpers, and emit the resulting
// `FocusChangedEvent` (when any) on the calling window.

/// Push a layer into the registry under the given owning window.
///
/// `window_label` is derived from the calling `tauri::Window` in the
/// command wrapper; the layer's owning window cannot be supplied by the
/// React side because Tauri webviews are server-tracked, not client-known.
fn spatial_push_layer_inner(
    registry: &mut SpatialRegistry,
    fq: FullyQualifiedMoniker,
    segment: SegmentMoniker,
    name: LayerName,
    parent: Option<FullyQualifiedMoniker>,
    window_label: WindowLabel,
) {
    registry.push_layer(FocusLayer {
        fq,
        segment,
        name,
        parent,
        window_label,
        last_focused: None,
    });
}

/// Move focus to the scope at `fq`.
///
/// The kernel records ancestry from the snapshot's `parent_zone` chain —
/// mid-life `navOverride` and `parent_zone` changes propagate without a
/// re-register. The registry is consulted only for the focused entry's
/// owning window, segment, and layer-ancestor chain.
///
/// `snapshot` is `None` only during the transient unmount window where
/// the focused scope's React-side layer registry has already torn down;
/// the kernel drops the commit silently in that case rather than guess
/// at scope ancestry without a snapshot.
///
/// Delegates to [`SpatialState::focus`]. Returns
/// `Some(FocusChangedEvent)` when focus actually moved and `None`
/// otherwise (no snapshot, window unknown, already focused, or the FQM
/// is missing from `snapshot.scopes`). We forward only actual
/// transitions so claim listeners don't see redundant events.
#[tauri::command]
pub async fn spatial_focus(
    window: Window,
    state: State<'_, AppState>,
    fq: FullyQualifiedMoniker,
    snapshot: Option<NavSnapshot>,
) -> Result<(), String> {
    let Some(snapshot) = snapshot else {
        tracing::debug!(
            op = "spatial_focus",
            focused_fq = %fq,
            "snapshot=None — dropping focus commit (transient unmount race)"
        );
        return Ok(());
    };
    let label = window_label_from(&window);
    let event = with_spatial(&state, |registry, spatial_state| {
        spatial_state.focus(registry, &snapshot, fq.clone(), Some(label))
    })
    .await;

    if let Some(event) = event {
        emit_focus_changed(&window, &event)?;
    }
    Ok(())
}

/// Clear focus for the calling window.
///
/// Explicit-clear counterpart of [`spatial_focus`]. The React side calls this when the user
/// (or a component-level handler) wants to drop focus altogether — for
/// example `setFocus(null)` from `entity-focus-context.tsx`. Routing
/// the clear through the kernel preserves the architectural invariant
/// from card `01KQD0WK54G0FRD7SZVZASA9ST`: the React entity-focus
/// store is a pure projection of `focus-changed` events. Without this
/// command, `setFocus(null)` would have to mutate the React store
/// synchronously to clear focus, which is exactly the kernel/React
/// drift the card was filed to eliminate.
///
/// Delegates to [`SpatialState::clear_focus`], which removes the
/// per-window focus slot and returns a `Some(prev) → None`
/// [`FocusChangedEvent`] when focus was actually cleared. We forward
/// the event so the React-side bridge writes the entity-focus store
/// to `null` and dispatches `ui.setFocus` with an empty scope chain.
/// When the window had no prior focus, the kernel returns `None` and
/// no event is emitted (idempotent).
#[tauri::command]
pub async fn spatial_clear_focus(window: Window, state: State<'_, AppState>) -> Result<(), String> {
    let label = window_label_from(&window);
    let event = with_spatial(&state, |_registry, spatial_state| {
        spatial_state.clear_focus(&label)
    })
    .await;

    if let Some(event) = event {
        emit_focus_changed(&window, &event)?;
    }
    Ok(())
}

/// Move focus relative to `focused_fq` in `direction`.
///
/// Pathfinding runs against the snapshot; the registry is consulted only
/// for the focused entry's segment / window and the target's commit
/// metadata. When `snapshot` is `None` (the transient unmount window),
/// the call drops silently — the React-side layer registry has torn
/// down and there is no live geometry to navigate over.
///
/// Returns `Ok(())` whether or not focus actually moved — under the
/// no-silent-dropout contract, the kernel always returns a moniker; if
/// it equals the focused moniker (semantic "stay put" or torn-state
/// echo), the inner method short-circuits via the
/// "already focused → no event" check in `SpatialState::focus` and
/// nothing is emitted. Same outcome when the resolved moniker doesn't
/// own any registered scope.
#[tauri::command]
pub async fn spatial_navigate(
    window: Window,
    state: State<'_, AppState>,
    focused_fq: FullyQualifiedMoniker,
    direction: Direction,
    snapshot: Option<NavSnapshot>,
) -> Result<(), String> {
    let Some(snapshot) = snapshot else {
        tracing::debug!(
            op = "spatial_navigate",
            focused_fq = %focused_fq,
            direction = ?direction,
            "snapshot=None — dropping navigation (transient unmount race)"
        );
        return Ok(());
    };
    let label = window_label_from(&window);
    let event = with_spatial(&state, |registry, spatial_state| {
        spatial_state.navigate(
            registry,
            &snapshot,
            focused_fq.clone(),
            direction,
            Some(label),
        )
    })
    .await;

    if let Some(event) = event {
        emit_focus_changed(&window, &event)?;
    }
    Ok(())
}

/// React to the focused scope unmounting on the React side.
///
/// Called from the React-side layer registry's deletion path when the
/// scope being unmounted is the currently focused FQM in this window.
/// React supplies the lost FQM, its `parent_zone`, owning layer FQM, and
/// last-known bounding rect alongside a snapshot whose `scopes` set has
/// already had the lost FQM removed — the kernel's fallback walk reads
/// from the snapshot only, so no registry mutation around the lost
/// entry's metadata is required.
#[tauri::command]
pub async fn spatial_focus_lost(
    window: Window,
    state: State<'_, AppState>,
    focused_fq: FullyQualifiedMoniker,
    lost_parent_zone: Option<FullyQualifiedMoniker>,
    lost_layer_fq: FullyQualifiedMoniker,
    lost_rect: Rect,
    snapshot: NavSnapshot,
) -> Result<(), String> {
    let label = window_label_from(&window);
    let event = with_spatial(&state, |registry, spatial_state| {
        spatial_state.focus_lost(
            registry,
            &snapshot,
            &focused_fq,
            lost_parent_zone.as_ref(),
            &lost_layer_fq,
            lost_rect,
            Some(label),
        )
    })
    .await;

    if let Some(event) = event {
        emit_focus_changed(&window, &event)?;
    }
    Ok(())
}

/// Push a new layer onto the registry.
///
/// Layers form a per-window forest: the window root has `parent = None`;
/// inspector / dialog / palette overlays are stacked under their parent.
/// `key` is the stable mount identifier; `name` is the layer role
/// (`"window"`, `"inspector"`, `"dialog"`, `"palette"`); `parent` ties
/// the layer to its stacking parent (`None` for a window root).
///
/// The owning window is taken from the calling `tauri::Window` — every
/// layer in a forest path back to a root shares the same window label,
/// and the registry uses that to bound spatial nav and fallback
/// resolution to a single window.
#[tauri::command]
pub async fn spatial_push_layer(
    window: Window,
    state: State<'_, AppState>,
    fq: FullyQualifiedMoniker,
    segment: SegmentMoniker,
    name: LayerName,
    parent: Option<FullyQualifiedMoniker>,
) -> Result<(), String> {
    let window_label = window_label_from(&window);
    with_spatial(&state, |registry, _spatial_state| {
        spatial_push_layer_inner(registry, fq, segment, name, parent, window_label);
    })
    .await;
    Ok(())
}

/// Pop a previously-pushed layer and return the focus-restoration target.
///
/// Reads the popped layer's `last_focused` slot before removal and
/// returns it so the caller can issue a follow-up `spatial_focus` to
/// commit the restoration through the snapshot path. The kernel does
/// not mutate `focus_by_window` or emit a `focus-changed` event from
/// this command — focus restoration is a two-step round-trip that
/// keeps every commit on the snapshot-driven path.
///
/// Returns `None` when the layer is unknown or has no recorded
/// `last_focused`. The React side treats `None` as "leave focus
/// as-is".
///
/// The registry side is a single `remove_layer` call; descendant scope
/// entries are dropped by the React side beforehand, so no GC pass is
/// needed here.
#[tauri::command]
pub async fn spatial_pop_layer(
    state: State<'_, AppState>,
    fq: FullyQualifiedMoniker,
) -> Result<Option<FullyQualifiedMoniker>, String> {
    let next_fq = with_spatial(&state, |registry, _spatial_state| {
        let next_fq = registry.layer(&fq).and_then(|l| l.last_focused.clone());
        registry.remove_layer(&fq);
        next_fq
    })
    .await;
    Ok(next_fq)
}

/// Compute the FQM to focus when the user drills *into* the scope at
/// `fq`.
///
/// Returns the result of [`swissarmyhammer_focus::drill_in`]: the
/// scope's recorded `last_focused_by_fq` target if it is still in
/// `snapshot`, else the topmost-then-leftmost child by rect, else
/// `focused_fq` (semantic no-op).
///
/// Pure query — does not mutate focus state and does not emit a
/// `focus-changed` event; the React side calls `setFocus(moniker)` on
/// the result. Returns `Ok(focused_fq)` when `snapshot` is `None`
/// (transient unmount window; the React side has nothing live to drill
/// into).
#[tauri::command]
pub async fn spatial_drill_in(
    _window: Window,
    state: State<'_, AppState>,
    fq: FullyQualifiedMoniker,
    focused_fq: FullyQualifiedMoniker,
    snapshot: Option<NavSnapshot>,
) -> Result<FullyQualifiedMoniker, String> {
    let Some(snapshot) = snapshot else {
        tracing::debug!(
            op = "spatial_drill_in",
            focused_fq = %focused_fq,
            target_fq = %fq,
            "snapshot=None — returning focused_fq (transient unmount race)"
        );
        return Ok(focused_fq);
    };
    let next_fq = with_spatial(&state, |registry, _spatial_state| {
        let view = swissarmyhammer_focus::IndexedSnapshot::new(&snapshot);
        swissarmyhammer_focus::drill_in(&view, registry, fq, &focused_fq)
    })
    .await;
    Ok(next_fq)
}

/// Compute the FQM to focus when the user drills *out of* the scope at
/// `fq`.
///
/// Returns the result of [`swissarmyhammer_focus::drill_out`]: the
/// scope's `parent_zone` if it is still in `snapshot`, else
/// `focused_fq` (the React side compares against `focused_fq` to fall
/// through to `app.dismiss`).
#[tauri::command]
pub async fn spatial_drill_out(
    _window: Window,
    state: State<'_, AppState>,
    fq: FullyQualifiedMoniker,
    focused_fq: FullyQualifiedMoniker,
    snapshot: Option<NavSnapshot>,
) -> Result<FullyQualifiedMoniker, String> {
    let Some(snapshot) = snapshot else {
        tracing::debug!(
            op = "spatial_drill_out",
            focused_fq = %focused_fq,
            target_fq = %fq,
            "snapshot=None — returning focused_fq (transient unmount race)"
        );
        return Ok(focused_fq);
    };
    let next_fq = with_spatial(&state, |_registry, _spatial_state| {
        let view = swissarmyhammer_focus::IndexedSnapshot::new(&snapshot);
        swissarmyhammer_focus::drill_out(&view, fq, &focused_fq)
    })
    .await;
    Ok(next_fq)
}

/// Generate `count` distinct, prefix-free key codes for the Jump-To
/// overlay (vim-sneak / AceJump-style labels).
///
/// Pure pass-through to
/// [`swissarmyhammer_focus::generate_sneak_codes`] — no state mutation,
/// no I/O. The frontend calls this once when the overlay opens and uses
/// the resulting codes to label visible scopes.
///
/// # Errors
///
/// Returns the stringified
/// [`swissarmyhammer_focus::SneakError`] when `count` exceeds
/// the maximum capacity of the alphabet (currently 529 — `23²`). In
/// practice this means an upstream bug, since the overlay never shows
/// hundreds of targets.
#[tauri::command]
pub fn generate_jump_codes(count: usize) -> Result<Vec<String>, String> {
    swissarmyhammer_focus::generate_sneak_codes(count).map_err(|e| e.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Generic MCP transport: `command_tool_call` + `mcp_subscribe`.
//
// The frontend has no in-process JS MCP client — the host runs the MCP server
// in Rust. These two handlers are the seam: `command_tool_call` is the request
// path the webview uses to reach the `command` operation tool exposed on the
// host's `commands` MCP module, and `mcp_subscribe` is the bootstrap that
// pumps the host's `NotificationBridge` into Tauri events the frontend
// `listen(...)`s on (one event per MCP `notifications/*` method name).
//
// See `apps/kanban-app/ui/src/lib/mcp-transport.ts` and
// `apps/kanban-app/ui/src/lib/mcp-notifications.ts` for the frontend ends.
// ─────────────────────────────────────────────────────────────────────────────

/// Tracks whether the `mcp_subscribe` bridge→Tauri-event pump has been
/// spawned, so a second `mcp_subscribe` call is a no-op (the bridge is one
/// shared broadcast channel; one forwarder fans out to every Tauri listener).
static MCP_SUBSCRIBE_STARTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Generic MCP `tools/call` from the webview onto the host's `commands` module.
///
/// The frontend's `callCommandTool(op, params)` lowers to
/// `invoke("command_tool_call", { tool: "command", op, params })`. We route it
/// as `host.call(HostInternal, "commands", tool, { op, ...params })`: the
/// `commands` MCP module (installed by `install_app_command_services`) hosts a
/// single operation tool named `"command"` that dispatches on the `op` verb
/// (`"list command"`, `"available command"`, etc.). The tool's structured
/// result is returned verbatim to the webview.
///
/// Errors from the platform / tool are stringified per this file's convention
/// (see `dispatch_command`, `get_entity`, …).
#[tauri::command]
pub async fn command_tool_call(
    state: State<'_, AppState>,
    tool: String,
    op: String,
    params: Option<Value>,
) -> Result<Value, String> {
    // Merge `op` into the params object so the operation tool sees the same
    // `{ op, ...params }` shape an external MCP client would send.
    let mut input = match params {
        Some(Value::Object(map)) => map,
        Some(other) => {
            return Err(format!(
                "command_tool_call: `params` must be a JSON object, got {other:?}",
            ));
        }
        None => serde_json::Map::new(),
    };
    input.insert("op".to_string(), Value::String(op));

    let platform = state.plugin_platform.lock().await;
    platform
        .host()
        .call(
            swissarmyhammer_plugin::CallerId::HostInternal,
            swissarmyhammer_command_service::bootstrap::COMMANDS_MODULE_ID,
            &tool,
            Value::Object(input),
        )
        .await
        .map_err(|e| e.to_string())
}

/// Start the `NotificationBridge` → Tauri-event forwarder once.
///
/// Idempotent: a second call is a no-op so the frontend can invoke this on
/// every webview mount without double-spawning the pump. The bridge is one
/// shared `broadcast` channel — one in-process subscriber fans out via
/// `app.emit(method, params)` to every Tauri `listen(method, ...)` in the
/// webview. The event name is the MCP notification `method` verbatim
/// (e.g. `"notifications/store/changed"`,
/// `"notifications/commands/changed"`), matching the constants in
/// `mcp-notifications.ts` / `mcp-transport.ts`.
///
/// The forwarder task lives for the app's lifetime; the frontend does not
/// unsubscribe.
#[tauri::command]
pub async fn mcp_subscribe(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    if MCP_SUBSCRIBE_STARTED.swap(true, Ordering::SeqCst) {
        // Pump already running — every Tauri `listen(...)` already receives.
        return Ok(());
    }

    let bridge = state.plugin_platform.lock().await.host().notification_bridge();
    let mut subscription = bridge.subscribe();

    tauri::async_runtime::spawn(async move {
        loop {
            match subscription.recv().await {
                Ok(notification) => {
                    // Emit under the MCP notification `method` so the
                    // frontend's `listen("notifications/store/changed", …)`
                    // (and siblings) wire up unchanged. The payload is the
                    // notification's `params` object — what every
                    // `subscribe*` helper in `mcp-notifications.ts` types as
                    // its `event.payload`.
                    if let Err(e) = app.emit(&notification.method, &notification.params) {
                        tracing::warn!(
                            method = %notification.method,
                            error = %e,
                            "mcp_subscribe: failed to emit MCP notification as Tauri event"
                        );
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(
                        skipped,
                        "mcp_subscribe: notification forwarder lagged; \
                         webview may need to resync"
                    );
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    // Bridge dropped — only happens at app teardown.
                    return;
                }
            }
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::generate_jump_codes;

    /// `generate_jump_codes` round-trips through the same code path the
    /// Tauri runtime invokes — identical output to the underlying Rust
    /// impl, errors flattened to `String`.
    #[test]
    fn generate_jump_codes_matches_rust_impl_for_known_count() {
        let count = 30usize;
        let via_command = generate_jump_codes(count).expect("count=30 must succeed");
        let via_rust = swissarmyhammer_focus::generate_sneak_codes(count)
            .expect("count=30 must succeed via direct call");
        assert_eq!(via_command, via_rust);
        assert_eq!(via_command.len(), count);
    }

    /// `generate_jump_codes` flattens [`SneakError`] to a `String`
    /// preserving the canonical error message.
    #[test]
    fn generate_jump_codes_flattens_too_many_targets_error() {
        let over = swissarmyhammer_focus::MAX_SNEAK_CODES + 1;
        let err = generate_jump_codes(over).expect_err("over-capacity must fail");
        assert!(
            err.contains("too many jump targets"),
            "unexpected error message: {err:?}",
        );
    }

    /// Verifies that store_name and id are correctly extracted from ChangeEvent
    /// payloads, and that events with missing fields are identified.
    #[test]
    fn store_name_extraction_from_change_event() {
        let event = swissarmyhammer_store::ChangeEvent::new(
            "item-created",
            serde_json::json!({
                "store": "task",
                "id": "01ABC"
            }),
        );
        let store_name = event
            .payload()
            .get("store")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let id = event
            .payload()
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert_eq!(store_name, "task");
        assert_eq!(id, "01ABC");
    }

    /// Events missing store or id should be detected so they can be dropped.
    #[test]
    fn store_name_extraction_missing_fields() {
        let event = swissarmyhammer_store::ChangeEvent::new("item-changed", serde_json::json!({}));
        let store_name = event
            .payload()
            .get("store")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let id = event
            .payload()
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(store_name.is_empty());
        assert!(id.is_empty());
    }

    /// Events with null values for store/id should also be treated as empty.
    #[test]
    fn store_name_extraction_null_values() {
        let event = swissarmyhammer_store::ChangeEvent::new(
            "item-changed",
            serde_json::json!({
                "store": null,
                "id": null
            }),
        );
        let store_name = event
            .payload()
            .get("store")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let id = event
            .payload()
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(store_name.is_empty());
        assert!(id.is_empty());
    }

    // ── retain_filtered_tasks delegation test ─────────────────────
    //
    // Filter DSL evaluation (`#tag`, `@user`, `^ref`, `$project`) lives in
    // `swissarmyhammer_kanban::task_helpers` as `TaskFilterAdapter` /
    // `retain_filtered_tasks`. The kanban-app crate consumes the shared
    // implementation via `retain_filtered_tasks` in `list_entities` so a
    // single DSL evaluator backs both `list_entities` and the
    // `perspective.switch` command. Full adapter coverage (tag / assignee /
    // ref / project, slug fallback, every operator) lives in
    // `swissarmyhammer-kanban/src/task_helpers.rs` — this test only pins the
    // delegation so a future refactor cannot silently re-introduce a
    // duplicate evaluator in this crate.

    use swissarmyhammer_entity::Entity;
    use swissarmyhammer_kanban::task_helpers::{retain_filtered_tasks, EntitySlugRegistry};

    /// Build a task entity with the given filter_tags, assignees, and depends_on.
    fn make_entity(
        id: &str,
        filter_tags: &[&str],
        assignees: &[&str],
        depends_on: &[&str],
    ) -> Entity {
        let mut e = Entity::new("task", id);
        e.set("filter_tags", serde_json::json!(filter_tags));
        e.set("assignees", serde_json::json!(assignees));
        e.set("depends_on", serde_json::json!(depends_on));
        e
    }

    #[test]
    fn list_entities_filter_path_delegates_to_kanban_crate() {
        // Two tasks, one tagged `#bug`. The shared `retain_filtered_tasks`
        // must keep only the bug-tagged entity.
        let mut entities = vec![
            make_entity("t1", &["bug", "READY"], &["will"], &[]),
            make_entity("t2", &["feature"], &["alice"], &[]),
        ];
        let registry = EntitySlugRegistry::empty();
        retain_filtered_tasks(&mut entities, "#bug", &registry).unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].id.as_ref(), "t1");
    }

    // ── filter_mention_candidates tests ────────────────────────────

    use super::filter_mention_candidates;

    /// Build a project entity fixture with an id and a name.
    ///
    /// Simulates the shape returned by `EntityContext::list("project")` where
    /// the project's `id` is free-form text that may differ from
    /// `slugify(name)`. Used to verify `filter_mention_candidates` ships the
    /// raw id and display name separately to the frontend.
    fn make_project(id: &str, name: &str) -> Entity {
        let mut e = Entity::new("project", id);
        e.set("name", serde_json::json!(name));
        e
    }

    /// Build a tag entity fixture where the id and the tag_name are identical.
    ///
    /// Tag ids are already slug-shaped by convention, so `id == tag_name`.
    /// Used to verify filter_mention_candidates treats tag and project
    /// shapes symmetrically at the API level even though the frontend
    /// sources slugs differently based on the entity's `mention_slug_field`.
    fn make_tag(id: &str) -> Entity {
        let mut e = Entity::new("tag", id);
        e.set("tag_name", serde_json::json!(id));
        e
    }

    #[test]
    fn filter_mention_candidates_ships_project_id_and_display_name_separately() {
        // A project with a free-form id distinct from slugify(name) is the
        // regression case: the frontend mention text must come from `id`
        // so the `$AUTH-Migration` filter atom matches tasks whose stored
        // `project` field equals `AUTH-Migration`. filter_mention_candidates
        // is the pinch point that ships both fields to the UI.
        let project = make_project("AUTH-Migration", "Auth Migration System");
        let matches = filter_mention_candidates(&[project], "", "name");
        assert_eq!(matches.len(), 1);
        let row = &matches[0];
        assert_eq!(
            row.get("id").and_then(|v| v.as_str()),
            Some("AUTH-Migration")
        );
        assert_eq!(
            row.get("display_name").and_then(|v| v.as_str()),
            Some("Auth Migration System")
        );
    }

    // ── match_dynamic_prefix tests ────────────────────────────────

    use super::match_dynamic_prefix;

    #[test]
    fn match_dynamic_prefix_rewrites_entity_add_to_canonical_command() {
        // `entity.add:task` rewrites to canonical `entity.add` with the
        // type moved into the arg bag under `entity_type`. The dispatcher
        // then routes to the generic `AddEntityCmd`.
        let (new_cmd, arg_key, arg_val, updates_bp) =
            match_dynamic_prefix("entity.add:task").unwrap().unwrap();
        assert_eq!(new_cmd, "entity.add");
        assert_eq!(arg_key, "entity_type");
        assert_eq!(arg_val, "task");
        assert!(!updates_bp);
    }

    #[test]
    fn match_dynamic_prefix_entity_add_requires_type_suffix() {
        // An empty suffix is a malformed command — reject it rather than
        // dispatch to `entity.add` with `entity_type: ""` which would
        // then fail availability checks.
        assert!(match_dynamic_prefix("entity.add:").is_err());
    }

    #[test]
    fn match_dynamic_prefix_passes_through_non_prefix_commands() {
        // Non-prefix commands (like `entity.add` without a suffix) are
        // NOT rewritten — the rewriter returns Ok(None) so the dispatcher
        // can fall through to the registry.
        assert!(match_dynamic_prefix("entity.add").unwrap().is_none());
        assert!(match_dynamic_prefix("task.add").unwrap().is_none());
    }

    /// Regression guard for 01KPZMXXEXKVE3RNPA4XJP0105: the palette emits
    /// `view.set` / `perspective.switch` directly with pre-filled `args` —
    /// there is no longer a `view.switch:*` or `perspective.goto:*` rewrite
    /// branch. If an input string still carries one of the legacy prefixes
    /// it must fall through to `Ok(None)` so the dispatcher surfaces it as
    /// an unknown command rather than silently translating it.
    #[test]
    fn match_dynamic_prefix_no_longer_rewrites_view_switch_or_perspective_goto() {
        assert!(
            match_dynamic_prefix("view.switch:board-view")
                .unwrap()
                .is_none(),
            "view.switch:* rewrite was retired in 01KPZMXXEXKVE3RNPA4XJP0105"
        );
        assert!(
            match_dynamic_prefix("perspective.goto:sprint-01")
                .unwrap()
                .is_none(),
            "perspective.goto:* rewrite was retired in 01KPZMXXEXKVE3RNPA4XJP0105"
        );
    }

    #[test]
    fn filter_mention_candidates_tag_id_and_display_name_match() {
        // Tags are the control case: id == tag_name, so the emitted row
        // has identical `id` and `display_name`. This guards against a
        // regression where the shape becomes asymmetric by accident.
        let tag = make_tag("bug");
        let matches = filter_mention_candidates(&[tag], "", "tag_name");
        assert_eq!(matches.len(), 1);
        let row = &matches[0];
        assert_eq!(row.get("id").and_then(|v| v.as_str()), Some("bug"));
        assert_eq!(
            row.get("display_name").and_then(|v| v.as_str()),
            Some("bug")
        );
    }

    // ── ui_state_change_kind tests ─────────────────────────────────
    //
    // These guard the wire-format contract for `ui-state-changed` events:
    // every payload carries a `kind` discriminator so the frontend can skip
    // `setState` for slices it owns (notably `scope_chain`, which echoes
    // back from every `ui.setFocus` call and would otherwise cascade
    // re-renders through every `useUIState()` consumer).

    use super::ui_state_change_kind;
    use swissarmyhammer_ui_state::UIStateChange;

    #[test]
    fn ui_state_change_kind_scope_chain() {
        let value = serde_json::to_value(UIStateChange::ScopeChain(vec!["board:main".to_string()]))
            .unwrap();
        assert_eq!(ui_state_change_kind(&value), Some("scope_chain"));
    }

    #[test]
    fn ui_state_change_kind_palette_open() {
        let value = serde_json::to_value(UIStateChange::PaletteOpen(true)).unwrap();
        assert_eq!(ui_state_change_kind(&value), Some("palette_open"));
    }

    #[test]
    fn ui_state_change_kind_keymap_mode() {
        let value = serde_json::to_value(UIStateChange::KeymapMode("vim".into())).unwrap();
        assert_eq!(ui_state_change_kind(&value), Some("keymap_mode"));
    }

    #[test]
    fn ui_state_change_kind_inspector_stack() {
        let value =
            serde_json::to_value(UIStateChange::InspectorStack(vec!["task:1".into()])).unwrap();
        assert_eq!(ui_state_change_kind(&value), Some("inspector_stack"));
    }

    #[test]
    fn ui_state_change_kind_active_view() {
        let value = serde_json::to_value(UIStateChange::ActiveView("board".into())).unwrap();
        assert_eq!(ui_state_change_kind(&value), Some("active_view"));
    }

    #[test]
    fn ui_state_change_kind_active_perspective() {
        let value =
            serde_json::to_value(UIStateChange::ActivePerspective("sprint-01".into())).unwrap();
        assert_eq!(ui_state_change_kind(&value), Some("active_perspective"));
    }

    #[test]
    fn ui_state_change_kind_app_mode() {
        let value = serde_json::to_value(UIStateChange::AppMode("normal".into())).unwrap();
        assert_eq!(ui_state_change_kind(&value), Some("app_mode"));
    }

    #[test]
    fn ui_state_change_kind_inspector_width() {
        // Pinned for the resizable-inspector pipeline. The frontend
        // listens for `inspector_width` events to learn the persisted
        // width set in another window or session.
        let value = serde_json::to_value(UIStateChange::InspectorWidth {
            window_label: "main".into(),
            width: 540,
        })
        .unwrap();
        assert_eq!(ui_state_change_kind(&value), Some("inspector_width"));
    }

    #[test]
    fn ui_state_change_kind_perspective_switch() {
        // Pinned for the `perspective.switch` command introduced in
        // 01KP3ERHEDP86C2JYYR7NM1593. The atomic id + filtered_task_ids
        // update must serialize to the `perspective_switch` wire kind so
        // the frontend can apply its own debounce/skip policy
        // independently of the legacy `active_perspective` id-only kind.
        let value = serde_json::to_value(UIStateChange::PerspectiveSwitch {
            perspective_id: "p1".into(),
            filtered_task_ids: vec!["t1".into(), "t2".into()],
        })
        .unwrap();
        assert_eq!(ui_state_change_kind(&value), Some("perspective_switch"));
    }

    #[test]
    fn ui_state_change_kind_board_switch() {
        // BoardSwitch is not typed as a UIStateChange — it's a side-effect
        // result shape. Detected by the presence of the `BoardSwitch` key.
        let value = serde_json::json!({
            "BoardSwitch": {
                "path": "/boards/my-board",
                "window_label": "main",
            }
        });
        assert_eq!(ui_state_change_kind(&value), Some("board_switch"));
    }

    #[test]
    fn ui_state_change_kind_board_close() {
        // Same shape as BoardSwitch — detected by the `BoardClose` key.
        let value = serde_json::json!({
            "BoardClose": {
                "path": "/boards/my-board",
            }
        });
        assert_eq!(ui_state_change_kind(&value), Some("board_close"));
    }

    #[test]
    fn ui_state_change_kind_unrelated_result_is_none() {
        // Results that are neither a UIStateChange nor a board side-effect
        // must NOT trigger a ui-state-changed emit. Null, plain strings,
        // and arbitrary objects all fall through to None.
        assert_eq!(ui_state_change_kind(&serde_json::Value::Null), None);
        assert_eq!(ui_state_change_kind(&serde_json::json!("ok")), None);
        assert_eq!(
            ui_state_change_kind(&serde_json::json!({ "some_other_key": 1 })),
            None
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Spatial-navigation command tests.
//
// These exercise the inner functions extracted from each `#[tauri::command]`
// shell. We can't construct a `tauri::Window` or `State<'_, AppState>` in a
// unit test without a Tauri runtime — so the inner helpers operate directly
// on `&mut SpatialRegistry, &mut SpatialState`, which is exactly the
// signature these tests want.
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod spatial_command_tests {
    use super::*;
    use std::collections::HashMap;
    use swissarmyhammer_focus::{Pixels, SnapshotScope};

    fn rect_at(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect {
            x: Pixels::new(x),
            y: Pixels::new(y),
            width: Pixels::new(w),
            height: Pixels::new(h),
        }
    }

    /// Push a window-root layer into `registry`, returning the layer FQM
    /// for use as `snapshot.layer_fq`.
    fn push_root_layer(
        registry: &mut SpatialRegistry,
        window: &str,
        layer_segment: &str,
    ) -> FullyQualifiedMoniker {
        let segment = SegmentMoniker::from_string(layer_segment);
        let fq = FullyQualifiedMoniker::root(&segment);
        spatial_push_layer_inner(
            registry,
            fq.clone(),
            segment,
            LayerName::from_string("window"),
            None,
            WindowLabel::from_string(window),
        );
        fq
    }

    /// Build a snapshot scope at the composed FQM `<layer_fq>/<segment>`
    /// with the given rect.
    fn snapshot_leaf(
        layer_fq: &FullyQualifiedMoniker,
        segment_str: &str,
        rect: Rect,
    ) -> (FullyQualifiedMoniker, SnapshotScope) {
        let segment = SegmentMoniker::from_string(segment_str);
        let fq = FullyQualifiedMoniker::compose(layer_fq, &segment);
        (
            fq.clone(),
            SnapshotScope {
                fq,
                rect,
                parent_zone: None,
                nav_override: HashMap::new(),
                focusable: true,
            },
        )
    }

    /// `spatial_focus` invokes the snapshot-driven focus path and the
    /// kernel returns a `FocusChangedEvent` carrying the focused window,
    /// FQM, and segment.
    #[test]
    fn spatial_focus_emits_focus_changed_event() {
        let mut registry = SpatialRegistry::new();
        let mut state = SpatialState::new();
        let layer = push_root_layer(&mut registry, "main", "L");
        let (fq, scope) = snapshot_leaf(&layer, "task:01", rect_at(0.0, 0.0, 10.0, 10.0));
        let snapshot = NavSnapshot {
            layer_fq: layer,
            scopes: vec![scope],
        };

        let event = state
            .focus(&mut registry, &snapshot, fq.clone())
            .expect("focus emits an event for a snapshot scope");

        assert_eq!(event.window_label, WindowLabel::from_string("main"));
        assert_eq!(event.prev_fq, None);
        assert_eq!(event.next_fq, Some(fq));
        assert_eq!(
            event.next_segment,
            Some(SegmentMoniker::from_string("task:01"))
        );
    }

    /// `spatial_navigate` lands on the snapshot-determined target and
    /// emits the matching `FocusChangedEvent`.
    #[test]
    fn spatial_navigate_with_snapshot_resolves_target() {
        let mut registry = SpatialRegistry::new();
        let mut state = SpatialState::new();
        let layer = push_root_layer(&mut registry, "main", "L");

        let (top, top_scope) = snapshot_leaf(&layer, "task:top", rect_at(0.0, 0.0, 10.0, 10.0));
        let (bottom, bottom_scope) =
            snapshot_leaf(&layer, "task:bottom", rect_at(0.0, 20.0, 10.0, 10.0));

        let snapshot = NavSnapshot {
            layer_fq: layer,
            scopes: vec![top_scope, bottom_scope],
        };

        state
            .focus(&mut registry, &snapshot, top.clone())
            .expect("focus top");

        let event = state
            .navigate(&mut registry, &snapshot, top, Direction::Down)
            .expect("Down resolves to bottom");

        assert_eq!(event.next_fq, Some(bottom));
    }

    /// `spatial_push_layer_inner` derives `window_label` from the calling
    /// command and stores the layer under that label so `root_for_window`
    /// can find it.
    #[test]
    fn spatial_push_layer_associates_window_label() {
        let mut registry = SpatialRegistry::new();
        let segment = SegmentMoniker::from_string("L1");
        let fq = FullyQualifiedMoniker::root(&segment);
        spatial_push_layer_inner(
            &mut registry,
            fq.clone(),
            segment,
            LayerName::from_string("window"),
            None,
            WindowLabel::from_string("board-abc"),
        );

        let root = registry
            .root_for_window(&WindowLabel::from_string("board-abc"))
            .expect("root layer registered for the window");
        assert_eq!(root.fq, fq);
        assert_eq!(root.window_label, WindowLabel::from_string("board-abc"));
    }

    /// `spatial_pop_layer` removes the layer from the registry.
    #[test]
    fn spatial_pop_layer_removes_layer() {
        let mut registry = SpatialRegistry::new();
        let layer = push_root_layer(&mut registry, "main", "L");
        assert!(registry.layer(&layer).is_some());

        registry.remove_layer(&layer);
        assert!(registry.layer(&layer).is_none());
    }

    /// When `fq` is missing from `snapshot.scopes`, `focus` drops the
    /// commit and returns `None`.
    #[test]
    fn focus_drops_when_fq_missing_from_snapshot() {
        let mut registry = SpatialRegistry::new();
        let layer = push_root_layer(&mut registry, "main", "L");
        let leaf = FullyQualifiedMoniker::compose(&layer, &SegmentMoniker::from_string("leaf:01"));

        let empty_snapshot = NavSnapshot {
            layer_fq: layer,
            scopes: vec![],
        };

        let mut state = SpatialState::new();
        let event = state.focus(&mut registry, &empty_snapshot, leaf);
        assert!(
            event.is_none(),
            "focus must drop when target is absent from snapshot"
        );
    }

    /// `focus` populates `last_focused_by_fq` for every ancestor in the
    /// snapshot's `parent_zone` chain.
    #[test]
    fn focus_records_last_focused_for_ancestor_zones() {
        let mut registry = SpatialRegistry::new();
        let layer = push_root_layer(&mut registry, "main", "L");
        let zone = FullyQualifiedMoniker::compose(&layer, &SegmentMoniker::from_string("zone:01"));
        let leaf = FullyQualifiedMoniker::compose(&zone, &SegmentMoniker::from_string("leaf:01"));

        let snapshot = NavSnapshot {
            layer_fq: layer,
            scopes: vec![
                SnapshotScope {
                    fq: zone.clone(),
                    rect: rect_at(0.0, 0.0, 100.0, 100.0),
                    parent_zone: None,
                    nav_override: HashMap::new(),
                    focusable: true,
                },
                SnapshotScope {
                    fq: leaf.clone(),
                    rect: rect_at(10.0, 10.0, 10.0, 10.0),
                    parent_zone: Some(zone.clone()),
                    nav_override: HashMap::new(),
                    focusable: true,
                },
            ],
        };

        let mut state = SpatialState::new();
        state
            .focus(&mut registry, &snapshot, leaf.clone())
            .expect("focus emits");

        assert_eq!(
            registry.last_focused_by_fq.get(&zone),
            Some(&leaf),
            "ancestor zone records the focused descendant",
        );
    }
}
