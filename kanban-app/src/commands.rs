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
use swissarmyhammer_filter_expr::FilterContext;
use swissarmyhammer_kanban::task_helpers::{enrich_all_task_entities, enrich_task_entity};
use swissarmyhammer_kanban::virtual_tags::default_virtual_tag_registry;
use tauri::menu::{ContextMenu, MenuBuilder};
use tauri::webview::WebviewWindowBuilder;
use tauri::{AppHandle, Emitter, Manager, State, Window};

/// The base application title shown in all window title bars.
pub const APP_TITLE: &str = "SwissArmyHammer";

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

/// Gather view info from an optional board handle for dynamic commands.
fn gather_views(
    handle: Option<&BoardHandle>,
) -> Vec<swissarmyhammer_kanban::scope_commands::ViewInfo> {
    use swissarmyhammer_kanban::scope_commands::ViewInfo;
    let Some(handle) = handle else { return vec![] };
    let Some(views_lock) = handle.ctx.views() else {
        return vec![];
    };
    let Ok(vc) = views_lock.try_read() else {
        return vec![];
    };
    vc.all_views()
        .iter()
        .map(|v| ViewInfo {
            id: v.id.clone(),
            name: v.name.clone(),
        })
        .collect()
}

/// Gather open board info from UIState for dynamic commands.
async fn gather_boards(
    ui_state: &swissarmyhammer_commands::UIState,
    boards: &tokio::sync::RwLock<
        std::collections::HashMap<std::path::PathBuf, std::sync::Arc<BoardHandle>>,
    >,
) -> Vec<swissarmyhammer_kanban::scope_commands::BoardInfo> {
    use swissarmyhammer_kanban::scope_commands::BoardInfo;
    let open_paths = ui_state.open_boards();
    let boards_lock = boards.read().await;
    let mut result = Vec::new();
    for path in &open_paths {
        let p = std::path::Path::new(path);
        let dir_name = p
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Board")
            .to_string();
        let entity_name = match boards_lock.get(p) {
            Some(handle) => board_display_name(handle)
                .await
                .unwrap_or_else(|| dir_name.clone()),
            None => dir_name.clone(),
        };
        let context_name = boards_lock
            .get(p)
            .map(|h| h.ctx.name().to_string())
            .unwrap_or_else(|| dir_name.clone());
        result.push(BoardInfo {
            path: path.clone(),
            name: dir_name,
            entity_name,
            context_name,
        });
    }
    result
}

/// Gather window info from Tauri for dynamic commands.
fn gather_windows(
    app: &tauri::AppHandle,
) -> Vec<swissarmyhammer_kanban::scope_commands::WindowInfo> {
    use swissarmyhammer_kanban::scope_commands::WindowInfo;
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

/// Resolve the active view kind (e.g. "board", "grid") from the UIState and views context.
fn resolve_active_view_kind(
    handle: Option<&BoardHandle>,
    ui_state: &swissarmyhammer_commands::UIState,
) -> Option<String> {
    let handle = handle?;
    let active_id = ui_state.active_view_id("main");
    if active_id.is_empty() {
        return None;
    }
    let views_lock = handle.ctx.views()?;
    let vc = views_lock.try_read().ok()?;
    let view = vc.all_views().iter().find(|v| v.id == active_id)?;
    Some(serde_json::to_value(&view.kind).ok()?.as_str()?.to_string())
}

/// Gather perspective info from an optional board handle for dynamic commands.
///
/// When `view_kind` is provided, only perspectives matching that view kind are
/// returned. This prevents duplicate "Default" entries across view kinds.
async fn gather_perspectives(
    handle: Option<&BoardHandle>,
    view_kind: Option<&str>,
) -> Vec<swissarmyhammer_kanban::scope_commands::PerspectiveInfo> {
    use swissarmyhammer_kanban::scope_commands::PerspectiveInfo;
    let Some(handle) = handle else { return vec![] };
    let Ok(pctx) = handle.ctx.perspective_context().await else {
        return vec![];
    };
    let Ok(pc) = pctx.try_read() else {
        return vec![];
    };
    pc.all()
        .iter()
        .filter(|p| view_kind.is_none_or(|vk| p.view == vk))
        .map(|p| PerspectiveInfo {
            id: p.id.clone(),
            name: p.name.clone(),
            view: p.view.clone(),
        })
        .collect()
}

/// Read the board entity's display name from the entity context.
///
/// Returns the `name` field of the board entity (entity type "board", id "board").
/// This is the canonical display name set during `init board` — typically the
/// directory name, but editable by the user.
async fn board_display_name(handle: &BoardHandle) -> Option<String> {
    let ectx = handle.ctx.entity_context().await.ok()?;
    let entity = ectx.read("board", "board").await.ok()?;
    entity
        .fields
        .get("name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
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
        let name = board_display_name(handle).await.unwrap_or_default();
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

/// Adapter that maps filter DSL atoms to entity fields.
///
/// Uses `filter_tags` (union of body tags + virtual tags) for `#tag` lookups,
/// `assignees` for `@user` lookups, `depends_on` + `id` for `^ref` lookups,
/// and the single-value `project` field for `$project` lookups.
struct EntityFilterAdapter<'a> {
    entity: &'a Entity,
}

impl<'a> FilterContext for EntityFilterAdapter<'a> {
    fn has_tag(&self, tag: &str) -> bool {
        self.entity
            .get_string_list("filter_tags")
            .iter()
            .any(|t| t.eq_ignore_ascii_case(tag))
    }

    fn has_assignee(&self, user: &str) -> bool {
        self.entity
            .get_string_list("assignees")
            .iter()
            .any(|a| a.eq_ignore_ascii_case(user))
    }

    fn has_ref(&self, id: &str) -> bool {
        self.entity.id.as_ref() == id
            || self
                .entity
                .get_string_list("depends_on")
                .iter()
                .any(|r| r == id)
    }

    fn has_project(&self, project: &str) -> bool {
        self.entity
            .get_str("project")
            .map(|p| p.eq_ignore_ascii_case(project))
            .unwrap_or(false)
    }
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

/// Apply a filter DSL expression to an entity list, retaining only matches.
///
/// Parses the filter string and evaluates it against each entity via
/// `EntityFilterAdapter`. Returns an error if the expression is invalid.
fn apply_filter(entities: &mut Vec<Entity>, filter_str: &str) -> Result<(), String> {
    let expr = swissarmyhammer_filter_expr::parse(filter_str).map_err(|errors| {
        let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        format!("invalid filter expression: {}", msgs.join("; "))
    })?;
    entities.retain(|e| expr.matches(&EntityFilterAdapter { entity: e }));
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
        apply_filter(&mut entities, filter_str)?;
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

    // Look up the mention_display_field for this entity type
    let fields_ctx = ectx.fields();
    let entity_def = fields_ctx
        .get_entity(&entity_type)
        .ok_or_else(|| format!("Unknown entity type: {}", entity_type))?;

    let display_field = entity_def
        .mention_display_field
        .as_ref()
        .map(|f| f.as_str())
        .unwrap_or("name");

    let entities = ectx.list(&entity_type).await.map_err(|e| e.to_string())?;

    let query_lower = query.to_lowercase();
    let matches: Vec<Value> = entities
        .iter()
        .filter(|e| {
            if query_lower.is_empty() {
                return true;
            }
            // Match on display field or entity ID
            let display = e.get_str(display_field).unwrap_or("");
            let id = e.id.as_str();
            display.to_lowercase().contains(&query_lower)
                || id.to_lowercase().contains(&query_lower)
        })
        .take(20)
        .map(|e| {
            json!({
                "id": e.id,
                "display_name": e.get_str(display_field).unwrap_or(""),
                "color": e.get_str("color"),
                "avatar": e.get_str("avatar"),
            })
        })
        .collect();

    Ok(json!(matches))
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
    let limit = limit.unwrap_or(50);

    // Cap query length to prevent excessive fuzzy matcher work
    if query.len() > 500 {
        return Err("Search query too long (max 500 characters)".into());
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

    let terminal_id = columns.last().map(|c| c.id.as_str()).unwrap_or("done");
    let (counts, ready_counts) = count_tasks_by_column(&all_tasks, terminal_id);
    let columns_json = serialize_columns_with_counts(&columns, &counts, &ready_counts);
    let tags_json: Vec<Value> = tags.iter().map(|tag| tag.to_json()).collect();
    let summary = build_board_summary(&board, all_tasks.len(), total_actors, &ready_counts);
    let virtual_tag_meta: Vec<Value> = default_virtual_tag_registry()
        .metadata()
        .into_iter()
        .map(|m| json!({ "slug": m.slug, "color": m.color, "description": m.description }))
        .collect();

    Ok(json!({
        "board": board.to_json(),
        "columns": columns_json,
        "tags": tags_json,
        "virtual_tag_meta": virtual_tag_meta,
        "summary": summary,
    }))
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

/// Options for restoring a window at a specific position and size.
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
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
        .inner_size(1200.0, 800.0)
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
        let name = board_display_name(handle).await;
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
fn match_dynamic_prefix(cmd: &str) -> Result<Option<(&str, &str, String, bool)>, String> {
    if let Some(suffix) = cmd.strip_prefix("view.switch:") {
        Ok(Some(("ui.view.set", "view_id", suffix.to_string(), false)))
    } else if let Some(suffix) = cmd.strip_prefix("board.switch:") {
        if suffix.contains("..") || !std::path::Path::new(suffix).is_absolute() {
            return Err(format!("Invalid board path in command: {:?}", suffix));
        }
        Ok(Some(("file.switchBoard", "path", suffix.to_string(), true)))
    } else if let Some(suffix) = cmd.strip_prefix("perspective.goto:") {
        Ok(Some((
            "ui.perspective.set",
            "perspective_id",
            suffix.to_string(),
            false,
        )))
    } else {
        Ok(None)
    }
}

/// Rewrite dynamic palette command prefixes to their canonical forms.
///
/// Handles `window.focus:*` (pure side-effect, returns early), `view.switch:*`,
/// `board.switch:*`, and `perspective.goto:*` by stripping the prefix and
/// injecting the suffix as an arg. Preserves all input validation (ASCII-only,
/// 128-char limit, bounded rewrite depth).
fn rewrite_dynamic_prefix(
    app: &AppHandle,
    cmd: &str,
    args: Option<Value>,
    board_path: Option<String>,
    target: &Option<String>,
    scope_chain: &Option<Vec<String>>,
) -> Result<RewriteResult, String> {
    if cmd.is_empty() || cmd.len() > 128 || !cmd.is_ascii() {
        return Err(format!("Invalid command ID: {:?}", cmd));
    }

    let mut effective_cmd = cmd.to_owned();
    let mut effective_args = args;
    let mut effective_board_path = board_path;

    for depth in 0..=MAX_REWRITE_DEPTH {
        tracing::info!(cmd = %effective_cmd, target = ?target, args = ?effective_args,
            scope_chain = ?scope_chain, board_path = ?effective_board_path, "command");

        if let Some(label) = effective_cmd.strip_prefix("window.focus:") {
            let result = handle_window_focus(app, label);
            return Ok(RewriteResult {
                cmd: effective_cmd,
                args: effective_args,
                board_path: effective_board_path,
                early_return: Some(result),
            });
        }

        if let Some((new_cmd, arg_key, arg_val, update_bp)) = match_dynamic_prefix(&effective_cmd)?
        {
            if depth >= MAX_REWRITE_DEPTH {
                return Err(format!(
                    "Command rewrite depth exceeded for: {}",
                    effective_cmd
                ));
            }
            let mut merged = match effective_args {
                Some(Value::Object(map)) => map,
                _ => serde_json::Map::new(),
            };
            merged.insert(arg_key.into(), Value::String(arg_val.clone()));
            if update_bp {
                effective_board_path = Some(arg_val);
            }
            effective_cmd = new_cmd.to_owned();
            effective_args = Some(Value::Object(merged));
            continue;
        }

        break;
    }

    Ok(RewriteResult {
        cmd: effective_cmd,
        args: effective_args,
        board_path: effective_board_path,
        early_return: None,
    })
}

/// Internal dispatch — single path for all state-mutating command execution.
///
/// This is the single path for all state-mutating command execution.
/// It handles: command lookup, context building, execution, undo tracking,
/// entity flush, event emission, and UIState change broadcasting.
///
/// Dynamic prefix commands (`view.switch:*`, `board.switch:*`) are rewritten
/// to their canonical command IDs via a single-pass loop. The rewrite is
/// limited to one iteration (`MAX_REWRITE_DEPTH`) to prevent unbounded
/// recursion from malformed command chains like `board.switch:board.switch:…`.
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
    // Rewrite dynamic prefixes (view.switch:*, board.switch:*, perspective.goto:*)
    // to canonical commands with merged args. Also validates command ID.
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
    let result = execute_registered_command(state, &effective_cmd, &ctx).await?;
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
                let name = board_display_name(handle).await;
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
/// and app quit. These are fire-and-forget hooks into the Tauri app.
async fn handle_ui_trigger_results(app: &AppHandle, state: &AppState, result: &Value) {
    if result.get("NewBoardDialog").is_some() {
        menu::trigger_new_board(app);
    }
    if result.get("OpenBoardDialog").is_some() {
        menu::trigger_open_board(app);
    }
    if result.get("CreateWindow").is_some() {
        create_window_internal(app, state).await;
    }
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
    if let Some(drag_start) = result.get("DragStart") {
        let payload = drag_start.clone();
        let _ = app.emit("drag-session-active", &payload);
    }
    if let Some(drag_cancel) = result.get("DragCancel") {
        let _ = app.emit("drag-session-cancelled", &drag_cancel);
    }
    if let Some(drag_complete) = result.get("DragComplete") {
        handle_drag_complete(app, state, active_handle, drag_complete).await;
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
        if let Some(handle) = active_handle {
            flush_and_emit_for_handle(app, handle).await;
        }
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
    flush_and_emit_for_handle(app, &tgt).await;
    if !params.copy_mode {
        flush_and_emit_for_handle(app, &src).await;
    }
    ok
}

/// Emit a fresh `ui-state-changed` event when the command either returned a
/// `UIStateChange` result envelope or mutated board open/close state (which
/// is not typed as a `UIStateChange` but still affects what the React
/// `UIStateProvider` renders).
fn emit_ui_state_change_if_needed(app: &AppHandle, state: &AppState, result: &Value) {
    if serde_json::from_value::<swissarmyhammer_commands::UIStateChange>(result.clone()).is_ok() {
        let _ = app.emit("ui-state-changed", state.ui_state.to_json());
    }
    if result.get("BoardSwitch").is_some() || result.get("BoardClose").is_some() {
        let _ = app.emit("ui-state-changed", state.ui_state.to_json());
    }
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

/// Post-command flush: write pending entity changes to disk, sync the
/// cached undo/redo flags, and refresh every window title that points at
/// the flushed board. Also emits the per-entity events through the watcher
/// cache so the file watcher doesn't re-fire for our own writes.
///
/// Runs for undoable commands **and** for `app.undo`/`app.redo`: both
/// mutate entities on disk but are themselves non-undoable (they must not
/// land on the undo stack), so they'd otherwise skip this flush.
async fn flush_and_sync_after_command(
    app: &AppHandle,
    state: &AppState,
    effective_cmd: &str,
    undoable: bool,
    active_handle: Option<&Arc<BoardHandle>>,
) {
    let needs_flush = undoable || effective_cmd == "app.undo" || effective_cmd == "app.redo";
    tracing::info!(
        cmd = %effective_cmd,
        undoable,
        needs_flush,
        has_handle = active_handle.is_some(),
        "flush gate"
    );
    if !needs_flush {
        tracing::info!(cmd = %effective_cmd, "non-mutating — skipping flush_and_emit");
        return;
    }
    let Some(handle) = active_handle else {
        tracing::info!(cmd = %effective_cmd, "needs_flush but no active_handle — events NOT emitted");
        return;
    };
    flush_and_emit_for_handle(app, handle).await;
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
    let display_name = board_display_name(handle).await;
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

/// Return all available commands for the given scope chain.
///
/// This is the single source of truth for what commands are available.
/// The frontend calls this with a scope chain and renders the result.
/// No command logic in the UI — just render and dispatch.
#[tauri::command]
pub async fn list_commands_for_scope(
    app: AppHandle,
    state: State<'_, AppState>,
    scope_chain: Vec<String>,
    context_menu: Option<bool>,
) -> Result<Value, String> {
    let active_handle = state.active_handle().await;
    let fields = active_handle.as_ref().and_then(|h| h.ctx.fields());

    let registry = state.commands_registry.read().await;

    let dynamic = {
        use swissarmyhammer_kanban::scope_commands::DynamicSources;
        let views = gather_views(active_handle.as_deref());
        let boards = gather_boards(&state.ui_state, &state.boards).await;
        let windows = gather_windows(&app);
        let view_kind = resolve_active_view_kind(active_handle.as_deref(), &state.ui_state);
        let perspectives =
            gather_perspectives(active_handle.as_deref(), view_kind.as_deref()).await;
        DynamicSources {
            views,
            boards,
            windows,
            perspectives,
        }
    };

    let result = swissarmyhammer_kanban::scope_commands::commands_for_scope(
        &scope_chain,
        &registry,
        &state.command_impls,
        fields,
        &state.ui_state,
        context_menu == Some(true),
        Some(&dynamic),
    );

    serde_json::to_value(&result).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// show_context_menu — generic native context menu
// ---------------------------------------------------------------------------

/// Flush entity changes for a board handle and emit events.
///
/// **Architecture rule (event-architecture):** Events are thin signals with
/// two granularities:
/// - Entity-level: `(entity_type, id)` for created/removed
/// - Field-level: `(entity_type, id, field, value)` for changes
///
/// The watcher produces field-level diffs by comparing file content hashes
/// and running `diff_fields`. Store events tell us WHICH entities were
/// written; the watcher tells us WHAT changed.
///
/// After collecting raw diffs, computed fields (e.g. `tags`, `progress`)
/// are re-derived via `EntityContext.read()` and appended to field-changed
/// events. Computed fields exist only at read time — the watcher never sees
/// them because they aren't stored on disk. This enrichment is the only way
/// the frontend learns about computed field updates.
async fn flush_and_emit_for_handle(app: &AppHandle, handle: &BoardHandle) {
    let kanban_root = handle.ctx.root().to_path_buf();

    // 1. Drain store-level change events. These tell us which entities were
    //    written by the command that just executed, but they carry only
    //    (store_name, id, event_kind) — no field data.
    let store_events = handle.store_context.flush_all().await;
    tracing::info!(
        store_event_count = store_events.len(),
        path = %kanban_root.display(),
        "store_context.flush_all result"
    );

    // 2. Flush the watcher cache. The watcher scans entity files on disk,
    //    compares against its cached hashes, and produces field-level diffs
    //    via diff_fields. This is the source of truth for WHAT changed.
    let watcher_events =
        crate::watcher::flush_and_emit(&kanban_root, &handle.store_roots, &handle.entity_cache);
    tracing::info!(
        watcher_event_count = watcher_events.len(),
        path = %kanban_root.display(),
        "watcher flush_and_emit result"
    );

    // 3. Build the event list. The watcher events are primary — they carry
    //    field-level diffs. Store events supplement with create/remove signals
    //    that the watcher may not have (e.g. the watcher sees a new file as
    //    EntityCreated, but the store knows the exact timing).
    //
    //    Dedup: collect watcher (entity_type, id) pairs for EntityFieldChanged
    //    and EntityCreated. If a store event has a matching watcher event, the
    //    watcher event wins (it has the diffs). Store-only events (no watcher
    //    match) are emitted as thin signals.
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    let mut events: Vec<crate::watcher::WatchEvent> = Vec::new();

    // Watcher events first — they carry the field-level diffs.
    for evt in watcher_events {
        match &evt {
            crate::watcher::WatchEvent::EntityFieldChanged {
                entity_type, id, ..
            }
            | crate::watcher::WatchEvent::EntityCreated {
                entity_type, id, ..
            } => {
                seen.insert((entity_type.clone(), id.clone()));
            }
            crate::watcher::WatchEvent::EntityRemoved {
                entity_type, id, ..
            } => {
                seen.insert((entity_type.clone(), id.clone()));
            }
            crate::watcher::WatchEvent::AttachmentChanged { .. } => {}
        }
        events.push(evt);
    }

    // Store events — only emit if the watcher didn't already cover them.
    for se in &store_events {
        let store_name = se
            .payload()
            .get("store")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let id = se
            .payload()
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if store_name.is_empty() || id.is_empty() {
            tracing::warn!(event_name = %se.event_name(), "dropping store event with empty store_name or id");
            continue;
        }

        let key = (store_name.to_string(), id.to_string());
        if seen.contains(&key) {
            // Watcher already produced a diff for this entity — skip the
            // store event to avoid duplicates.
            tracing::debug!(
                entity_type = store_name,
                id = id,
                event = se.event_name(),
                "store event deduped by watcher"
            );
            continue;
        }

        // Store event with no watcher match — emit as a thin signal.
        match se.event_name() {
            "item-created" => {
                events.push(crate::watcher::WatchEvent::EntityCreated {
                    entity_type: store_name.to_string(),
                    id: id.to_string(),
                    fields: std::collections::HashMap::new(),
                });
            }
            "item-changed" => {
                // The watcher didn't produce an event for this entity.
                // This can happen when the async file watcher's debounce
                // fires between the command write and this synchronous flush,
                // consuming the change and updating the cache. Read the
                // entity's current fields from disk and emit a synthetic
                // EntityFieldChanged so that enrich_computed_fields can
                // append computed fields (tags, progress, etc.).
                if let Some(fields) = crate::watcher::read_entity_fields_from_disk(
                    &kanban_root,
                    &handle.store_roots,
                    store_name,
                    id,
                ) {
                    let changes: Vec<crate::watcher::FieldChange> = fields
                        .into_iter()
                        .map(|(field, value)| crate::watcher::FieldChange { field, value })
                        .collect();
                    if !changes.is_empty() {
                        tracing::info!(
                            entity_type = store_name,
                            id = id,
                            change_count = changes.len(),
                            "store item-changed: async watcher race recovery, emitting synthetic event"
                        );
                        events.push(crate::watcher::WatchEvent::EntityFieldChanged {
                            entity_type: store_name.to_string(),
                            id: id.to_string(),
                            changes,
                        });
                    }
                } else {
                    tracing::debug!(
                        entity_type = store_name,
                        id = id,
                        "store item-changed but could not read entity from disk — skipping"
                    );
                }
            }
            "item-removed" => {
                events.push(crate::watcher::WatchEvent::EntityRemoved {
                    entity_type: store_name.to_string(),
                    id: id.to_string(),
                });
            }
            _ => {}
        }
    }

    // 4. Enrich field-changed events with computed field values.
    //
    // Computed fields (tags, progress, etc.) are derived at read time by
    // the ComputeEngine — they don't exist on disk, so the watcher's
    // diff_fields never sees them. For each EntityFieldChanged, read the
    // entity through EntityContext (which runs derive_all) and append any
    // computed fields whose values differ from the raw diff.
    let events = enrich_computed_fields(&handle.ctx, events).await;

    // 5. Sync search index and emit to frontend.
    {
        let board_path_str = kanban_root.display().to_string();
        let mut search_idx = handle.search_index.write().await;
        for evt in events {
            crate::watcher::sync_search_index(&mut search_idx, &evt);
            let event_name = match &evt {
                crate::watcher::WatchEvent::EntityCreated { .. } => "entity-created",
                crate::watcher::WatchEvent::EntityRemoved { .. } => "entity-removed",
                crate::watcher::WatchEvent::EntityFieldChanged { .. } => "entity-field-changed",
                crate::watcher::WatchEvent::AttachmentChanged { .. } => "attachment-changed",
            };
            let wrapped = crate::watcher::BoardWatchEvent {
                event: evt,
                board_path: board_path_str.clone(),
            };
            let _ = app.emit(event_name, &wrapped);
        }
    }
}

/// Enrich `EntityFieldChanged` events with re-derived computed field values.
///
/// Computed fields (e.g. `tags` from `parse-body-tags`, `progress` from
/// `parse-body-progress`) don't exist on disk — the watcher's `diff_fields`
/// never sees them. This function reads each changed entity through
/// `EntityContext` (which runs `ComputeEngine.derive_all()`) and appends
/// any computed fields to the event's changes array.
///
/// This is generic: any field with `FieldType::Computed` in the schema gets
/// picked up automatically. No hardcoded field names.
async fn enrich_computed_fields(
    ctx: &swissarmyhammer_kanban::KanbanContext,
    mut events: Vec<crate::watcher::WatchEvent>,
) -> Vec<crate::watcher::WatchEvent> {
    // Get the entity context (has ComputeEngine) and field definitions.
    let ectx = match ctx.entity_context().await {
        Ok(ectx) => ectx,
        Err(e) => {
            tracing::warn!("enrich_computed_fields: failed to get entity context: {e}");
            return events;
        }
    };
    let fields_ctx = match ctx.fields() {
        Some(f) => f,
        None => return events,
    };

    for evt in &mut events {
        enrich_one_watch_event(evt, ectx.as_ref(), fields_ctx).await;
    }

    events
}

/// Enrich a single `EntityFieldChanged` event with computed-field values.
///
/// No-op for non-EntityFieldChanged events or entity types that have no
/// computed fields. For `task` entities, runs the full enrichment pipeline
/// (virtual tags, ready/blocked, etc.) before appending computed values.
async fn enrich_one_watch_event(
    evt: &mut crate::watcher::WatchEvent,
    ectx: &swissarmyhammer_entity::EntityContext,
    fields_ctx: &swissarmyhammer_fields::FieldsContext,
) {
    let (entity_type, id, changes) = match evt {
        crate::watcher::WatchEvent::EntityFieldChanged {
            entity_type,
            id,
            changes,
        } => (entity_type.as_str(), id.as_str(), changes),
        _ => return,
    };

    let computed_names = computed_field_names(fields_ctx, entity_type);
    if computed_names.is_empty() {
        return;
    }

    // Read the entity through EntityContext so ComputeEngine-derived values
    // (tags from parse-body-tags, progress from parse-body-progress, …) are
    // already materialized on the entity we'll read fields from.
    let mut entity = match ectx.read(entity_type, id).await {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                entity_type = entity_type,
                id = id,
                "enrich_computed_fields: failed to read entity: {e}"
            );
            return;
        }
    };
    if entity_type == "task" {
        enrich_task_from_context(&mut entity, ectx).await;
    }

    let raw_change_count = changes.len();
    append_computed_changes(entity_type, id, &entity, &computed_names, changes);
    tracing::debug!(
        entity_type = entity_type,
        id = id,
        raw_changes = raw_change_count,
        enriched_changes = changes.len(),
        computed_fields = ?computed_names,
        "enrich_computed_fields: done"
    );
}

/// Return the names of every `Computed` field defined for `entity_type`.
fn computed_field_names<'a>(
    fields_ctx: &'a swissarmyhammer_fields::FieldsContext,
    entity_type: &str,
) -> Vec<&'a str> {
    fields_ctx
        .fields_for_entity(entity_type)
        .into_iter()
        .filter(|fd| matches!(fd.type_, swissarmyhammer_fields::FieldType::Computed { .. }))
        .map(|fd| fd.name.as_str())
        .collect()
}

/// Run the task-specific enrichment pipeline: virtual tags, filter tags,
/// ready/blocked flags, and the `blocks`/`blocked_by` reverse indices.
///
/// The ComputeEngine stubs for these fields return empty arrays; the real
/// values come from `enrich_task_entity`, which needs the full task list and
/// the terminal column to determine the "done" transition.
async fn enrich_task_from_context(
    entity: &mut swissarmyhammer_entity::Entity,
    ectx: &swissarmyhammer_entity::EntityContext,
) {
    let all_tasks = match ectx.list("task").await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("enrich_computed_fields: failed to list tasks for enrichment: {e}");
            vec![]
        }
    };
    let mut columns = ectx.list("column").await.unwrap_or_default();
    columns.sort_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0) as usize);
    let terminal_id = columns
        .last()
        .map(|c| c.id.to_string())
        .unwrap_or_else(|| "done".to_string());
    let registry = default_virtual_tag_registry();
    enrich_task_entity(entity, &all_tasks, &terminal_id, registry);
}

/// Append computed field values from `entity` into `changes`, skipping any
/// field already present in `changes`. Emits debug/warn traces for each
/// decision so flaky field populations are easy to diagnose.
fn append_computed_changes(
    entity_type: &str,
    id: &str,
    entity: &swissarmyhammer_entity::Entity,
    computed_names: &[&str],
    changes: &mut Vec<crate::watcher::FieldChange>,
) {
    let existing: std::collections::HashSet<String> =
        changes.iter().map(|c| c.field.clone()).collect();
    for name in computed_names {
        append_one_computed_change(entity_type, id, entity, name, &existing, changes);
    }
}

/// Append one computed field's value to `changes`, unless `existing` already
/// contains it or the entity hasn't materialized it. Emits the same
/// debug/warn traces the caller used to emit inline.
fn append_one_computed_change(
    entity_type: &str,
    id: &str,
    entity: &swissarmyhammer_entity::Entity,
    name: &str,
    existing: &std::collections::HashSet<String>,
    changes: &mut Vec<crate::watcher::FieldChange>,
) {
    if existing.contains(name) {
        tracing::debug!(
            entity_type = entity_type,
            id = id,
            field = name,
            "enrich_computed_fields: computed field already in changes, skipping"
        );
        return;
    }
    let Some(value) = entity.fields.get(name) else {
        tracing::warn!(
            entity_type = entity_type,
            id = id,
            field = name,
            "enrich_computed_fields: computed field not present in entity"
        );
        return;
    };
    tracing::debug!(
        entity_type = entity_type,
        id = id,
        field = name,
        value = %value,
        "enrich_computed_fields: appending computed field"
    );
    changes.push(crate::watcher::FieldChange {
        field: name.to_string(),
        value: value.clone(),
    });
}

/// A single item in a generic context menu.
///
/// Each item is self-contained: it carries the command ID, target, and scope
/// chain needed for dispatch. The frontend sends all dispatch info upfront;
/// when the user selects an item, Rust dispatches directly — no round-trip.
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
/// ID and dispatches directly via `dispatch_command_internal` — no round-trip
/// to the frontend.
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

#[cfg(test)]
mod tests {
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

    // ── EntityFilterAdapter tests ──────────────────────────────────

    use super::EntityFilterAdapter;
    use swissarmyhammer_entity::Entity;
    use swissarmyhammer_filter_expr::FilterContext;

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
    fn adapter_has_tag_matches_filter_tags() {
        let e = make_entity("t1", &["bug", "READY"], &[], &[]);
        let adapter = EntityFilterAdapter { entity: &e };
        assert!(adapter.has_tag("bug"));
        assert!(adapter.has_tag("READY"));
        assert!(!adapter.has_tag("feature"));
    }

    #[test]
    fn adapter_has_tag_case_insensitive() {
        let e = make_entity("t1", &["READY"], &[], &[]);
        let adapter = EntityFilterAdapter { entity: &e };
        assert!(adapter.has_tag("ready"));
        assert!(adapter.has_tag("Ready"));
    }

    #[test]
    fn adapter_has_assignee() {
        let e = make_entity("t1", &[], &["alice", "bob"], &[]);
        let adapter = EntityFilterAdapter { entity: &e };
        assert!(adapter.has_assignee("alice"));
        assert!(adapter.has_assignee("bob"));
        assert!(!adapter.has_assignee("carol"));
    }

    #[test]
    fn adapter_has_assignee_case_insensitive() {
        let e = make_entity("t1", &[], &["Alice"], &[]);
        let adapter = EntityFilterAdapter { entity: &e };
        assert!(adapter.has_assignee("alice"));
    }

    #[test]
    fn adapter_has_ref_matches_depends_on() {
        let e = make_entity("t1", &[], &[], &["dep1", "dep2"]);
        let adapter = EntityFilterAdapter { entity: &e };
        assert!(adapter.has_ref("dep1"));
        assert!(adapter.has_ref("dep2"));
        assert!(!adapter.has_ref("dep3"));
    }

    #[test]
    fn adapter_has_ref_matches_own_id() {
        let e = make_entity("t1", &[], &[], &[]);
        let adapter = EntityFilterAdapter { entity: &e };
        assert!(adapter.has_ref("t1"));
    }

    #[test]
    fn adapter_end_to_end_with_filter_expr() {
        let e = make_entity("t1", &["bug", "READY"], &["will"], &["dep1"]);
        let adapter = EntityFilterAdapter { entity: &e };

        let expr = swissarmyhammer_filter_expr::parse("#bug && @will").unwrap();
        assert!(expr.matches(&adapter));

        let expr = swissarmyhammer_filter_expr::parse("#bug && @alice").unwrap();
        assert!(!expr.matches(&adapter));

        let expr = swissarmyhammer_filter_expr::parse("#READY").unwrap();
        assert!(expr.matches(&adapter));

        let expr = swissarmyhammer_filter_expr::parse("!#done").unwrap();
        assert!(expr.matches(&adapter));

        let expr = swissarmyhammer_filter_expr::parse("^dep1").unwrap();
        assert!(expr.matches(&adapter));
    }

    /// Build a task entity with a `project` field set.
    fn make_entity_with_project(id: &str, project: &str) -> Entity {
        let mut e = Entity::new("task", id);
        e.set("project", serde_json::json!(project));
        e
    }

    #[test]
    fn adapter_has_project_matches_project_field() {
        let e = make_entity_with_project("t1", "auth-migration");
        let adapter = EntityFilterAdapter { entity: &e };
        assert!(adapter.has_project("auth-migration"));
        assert!(!adapter.has_project("frontend"));
    }

    #[test]
    fn adapter_has_project_case_insensitive() {
        let e = make_entity_with_project("t1", "auth-migration");
        let adapter = EntityFilterAdapter { entity: &e };
        assert!(adapter.has_project("AUTH-MIGRATION"));
        assert!(adapter.has_project("Auth-Migration"));
    }

    #[test]
    fn adapter_has_project_absent_field_is_false() {
        // Entity with no `project` field set — should never match a `$project` filter.
        let e = make_entity("t1", &[], &[], &[]);
        let adapter = EntityFilterAdapter { entity: &e };
        assert!(!adapter.has_project("anything"));
    }

    #[test]
    fn adapter_has_project_with_filter_expr() {
        let e = make_entity_with_project("t1", "auth");
        let adapter = EntityFilterAdapter { entity: &e };

        let expr = swissarmyhammer_filter_expr::parse("$auth").unwrap();
        assert!(expr.matches(&adapter));

        let expr = swissarmyhammer_filter_expr::parse("$frontend").unwrap();
        assert!(!expr.matches(&adapter));

        let expr = swissarmyhammer_filter_expr::parse("$AUTH").unwrap();
        assert!(expr.matches(&adapter));
    }
}
