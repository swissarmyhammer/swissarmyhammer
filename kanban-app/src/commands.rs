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

/// List all entities of a given type, returning raw entity bags.
///
/// For tasks, enriches each entity with computed fields: `ready`, `blocked_by`,
/// `blocks`, and `progress_fraction`. Other entity types are returned as-is.
///
/// Returns `{ entities: [...], count: N }`.
#[tauri::command]
pub async fn list_entities(
    state: State<'_, AppState>,
    entity_type: String,
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
        // Need terminal column for readiness computation
        let mut columns = ectx
            .list("column")
            .await
            .map_err(|e| format!("list_entities({}): {}", entity_type, e))?;
        columns.sort_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0) as usize);
        let terminal_id = columns
            .last()
            .map(|c| c.id.to_string())
            .unwrap_or_else(|| "done".to_string());

        // Batch-enrich in O(N) using pre-built dependency indexes
        let registry = default_virtual_tag_registry();
        enrich_all_task_entities(&mut entities, &terminal_id, registry);

        // Sort by position so the frontend can trust the order
        entities.sort_by(|a, b| {
            let col_a = a.get_str("position_column").unwrap_or("");
            let col_b = b.get_str("position_column").unwrap_or("");
            col_a.cmp(col_b).then_with(|| {
                let ord_a = a.get_str("position_ordinal").unwrap_or("a0");
                let ord_b = b.get_str("position_ordinal").unwrap_or("a0");
                ord_a.cmp(ord_b)
            })
        });
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

            Some(json!({
                "entity_type": entity_type,
                "entity_id": entity.id,
                "display_name": display_name,
                "score": result.score,
            }))
        })
        .collect();

    Ok(json!(output))
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

    // Read board entity
    let board = ectx
        .read("board", "board")
        .await
        .map_err(|e| format!("get_board_data: {}", e))?;

    // Read and sort columns by order
    let mut columns = ectx
        .list("column")
        .await
        .map_err(|e| format!("get_board_data: {}", e))?;
    columns.sort_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0) as usize);

    // Read tags
    let tags = ectx
        .list("tag")
        .await
        .map_err(|e| format!("get_board_data: {}", e))?;

    // Read all tasks for counting
    let all_tasks = ectx
        .list("task")
        .await
        .map_err(|e| format!("get_board_data: {}", e))?;
    let terminal_id = columns.last().map(|c| c.id.as_str()).unwrap_or("done");

    // Count tasks per column, and ready tasks per column
    let mut column_counts: HashMap<String, usize> = HashMap::new();
    let mut column_ready_counts: HashMap<String, usize> = HashMap::new();
    for task in &all_tasks {
        let col = task
            .get_str("position_column")
            .unwrap_or("todo")
            .to_string();
        *column_counts.entry(col.clone()).or_insert(0) += 1;
        if swissarmyhammer_kanban::task_helpers::task_is_ready(task, &all_tasks, terminal_id) {
            *column_ready_counts.entry(col).or_insert(0) += 1;
        }
    }

    // Serialize columns with injected task_count and ready_count
    let columns_json: Vec<Value> = columns
        .iter()
        .map(|col| {
            let mut e = col.clone();
            let count = column_counts.get(col.id.as_str()).copied().unwrap_or(0);
            let ready = column_ready_counts
                .get(col.id.as_str())
                .copied()
                .unwrap_or(0);
            e.set("task_count", json!(count));
            e.set("ready_count", json!(ready));
            e.to_json()
        })
        .collect();

    // Serialize tags (no task_count — removed to avoid O(tasks × tags) scanning)
    let tags_json: Vec<Value> = tags.iter().map(|tag| tag.to_json()).collect();

    // Compute summary counts
    let total_tasks = all_tasks.len();
    // Sum pre-computed column ready counts instead of re-scanning all tasks
    let ready_tasks: usize = column_ready_counts.values().sum();
    let total_actors = ectx
        .list("actor")
        .await
        .map_err(|e| format!("get_board_data: {}", e))?
        .len();

    // Extract percent_complete from the board entity's computed field
    let pc = board
        .get("percent_complete")
        .cloned()
        .unwrap_or(json!(null));
    let done_tasks = pc.get("done").and_then(|v| v.as_u64()).unwrap_or(0);
    let percent_complete = pc.get("percent").and_then(|v| v.as_u64()).unwrap_or(0);

    Ok(json!({
        "board": board.to_json(),
        "columns": columns_json,
        "tags": tags_json,
        "summary": {
            "total_tasks": total_tasks,
            "total_actors": total_actors,
            "ready_tasks": ready_tasks,
            "blocked_tasks": total_tasks - ready_tasks,
            "done_tasks": done_tasks,
            "percent_complete": percent_complete,
        }
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

    // Resolve board path: explicit > AppState active board > first open board
    let resolved_path = match board_path {
        Some(bp) => Some(bp),
        None => {
            // Fall back to the most recently focused board
            state.ui_state.most_recent_board().or_else(|| {
                let boards = state.boards.try_read().ok();
                boards.and_then(|b| b.keys().next().map(|p| p.display().to_string()))
            })
        }
    };

    let mut url = String::from("index.html?window=board");
    if let Some(ref bp) = resolved_path {
        url.push_str("&board=");
        url.push_str(&urlencoding::encode(bp));
    }

    let builder = WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::App(url.into()))
        .title(APP_TITLE)
        .visible(false)
        .inner_size(1200.0, 800.0)
        .resizable(true)
        .disable_drag_drop_handler();

    let window = builder
        .build()
        .map_err(|e| format!("Failed to create window: {e}"))?;

    // Apply saved geometry if provided (restore path), otherwise read
    // initial geometry from the newly created window (new window path).
    if let Some(geo) = &geometry {
        let _ = window.set_size(tauri::PhysicalSize::new(geo.width, geo.height));
        let _ = window.set_position(tauri::PhysicalPosition::new(geo.x, geo.y));
        if geo.maximized {
            let _ = window.maximize();
        }
    }

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

        // Save geometry — either from the provided restore geometry or
        // from the actual window position after OS placement.
        if let Some(geo) = &geometry {
            state.ui_state.save_window_geometry(
                &label,
                geo.x,
                geo.y,
                geo.width,
                geo.height,
                geo.maximized,
            );
        } else if let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) {
            let maximized = window.is_maximized().unwrap_or(false);
            state.ui_state.save_window_geometry(
                &label,
                pos.x,
                pos.y,
                size.width,
                size.height,
                maximized,
            );
        }

        // Set window title from board entity display name
        let board_path = std::path::PathBuf::from(bp);
        let canonical = board_path
            .canonicalize()
            .unwrap_or_else(|_| board_path.clone());
        let boards = state.boards.read().await;
        if let Some(handle) = boards.get(&canonical) {
            let name = board_display_name(handle).await;
            update_window_title(&app, &label, name.as_deref());
        }
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
    /// Maximum number of prefix rewrites before we reject the command.
    /// One rewrite is sufficient for all known dynamic prefixes
    /// (`view.switch:*` -> `ui.view.set`, `board.switch:*` -> `file.switchBoard`).
    const MAX_REWRITE_DEPTH: u8 = 1;

    // Validate command ID: non-empty, reasonable length, ASCII-only
    if cmd.is_empty() || cmd.len() > 128 || !cmd.is_ascii() {
        return Err(format!("Invalid command ID: {:?}", cmd));
    }

    // --- Prefix rewrite loop ---
    // Dynamic palette commands (view.switch:*, board.switch:*) are rewritten
    // to their canonical command IDs with merged args. The loop runs at most
    // MAX_REWRITE_DEPTH times to prevent unbounded recursion.
    let mut effective_cmd = cmd.to_owned();
    let mut effective_args = args;
    let mut effective_board_path = board_path;

    for depth in 0..=MAX_REWRITE_DEPTH {
        tracing::info!(
            cmd = %effective_cmd,
            target = ?target,
            args = ?effective_args,
            scope_chain = ?scope_chain,
            board_path = ?effective_board_path,
            "command"
        );

        // `window.focus:*` — pure OS-level side-effect (unminimize + set focus).
        // Returns early without entering the standard result-processing pipeline
        // because window focus has no undo, UIState, or entity implications.
        if let Some(label) = effective_cmd.strip_prefix("window.focus:") {
            tracing::info!(label = %label, "window.focus — bringing window to front");
            if let Some(window) = app.get_webview_window(label) {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            return Ok(json!({ "WindowFocus": label }));
        }

        if let Some(view_id) = effective_cmd.strip_prefix("view.switch:") {
            if depth >= MAX_REWRITE_DEPTH {
                return Err(format!(
                    "Command rewrite depth exceeded for: {}",
                    effective_cmd
                ));
            }
            tracing::info!(view_id = %view_id, "redirecting view.switch to ui.view.set");
            let mut merged_args = match effective_args {
                Some(Value::Object(map)) => map,
                _ => serde_json::Map::new(),
            };
            merged_args.insert("view_id".into(), Value::String(view_id.to_string()));
            effective_cmd = "ui.view.set".to_owned();
            effective_args = Some(Value::Object(merged_args));
            continue;
        }

        if let Some(board_path_suffix) = effective_cmd.strip_prefix("board.switch:") {
            if depth >= MAX_REWRITE_DEPTH {
                return Err(format!(
                    "Command rewrite depth exceeded for: {}",
                    effective_cmd
                ));
            }
            tracing::info!(board_path = %board_path_suffix, "redirecting board.switch to file.switchBoard");
            let mut merged_args = match effective_args {
                Some(Value::Object(map)) => map,
                _ => serde_json::Map::new(),
            };
            merged_args.insert("path".into(), Value::String(board_path_suffix.to_string()));
            effective_board_path = Some(board_path_suffix.to_string());
            effective_cmd = "file.switchBoard".to_owned();
            effective_args = Some(Value::Object(merged_args));
            continue;
        }

        // No prefix matched — proceed with normal dispatch.
        break;
    }

    // Resolve scope chain: explicit > stored focus
    let scope = match scope_chain {
        Some(sc) => sc,
        None => state.ui_state.scope_chain(),
    };
    tracing::debug!(scope = ?scope, "resolved scope chain");

    // Look up command definition — clone the undoable flag so we don't
    // hold the registry read guard across the async execute call.
    let undoable = {
        let registry = state.commands_registry.read().await;
        let cmd_def = registry
            .get(effective_cmd.as_str())
            .ok_or_else(|| format!("Unknown command: {}", effective_cmd))?;
        cmd_def.undoable
    };

    // Look up command implementation
    let cmd_impl = state
        .command_impls
        .get(effective_cmd.as_str())
        .ok_or_else(|| format!("No implementation for command: {}", effective_cmd))?;

    // Build CommandContext
    let args_map: HashMap<String, Value> = match effective_args {
        Some(Value::Object(map)) => map.into_iter().collect(),
        _ => HashMap::new(),
    };
    let mut ctx = swissarmyhammer_commands::CommandContext::new(
        effective_cmd.clone(),
        scope,
        target,
        args_map,
    );
    ctx = ctx.with_ui_state(Arc::clone(&state.ui_state));

    // Set KanbanContext extension if board is open.
    // Uses effective_board_path when provided (multi-window) to avoid targeting the wrong board.
    // Falls back to the `store:` moniker in the scope chain so StoreContainer
    // can supply the board path without an explicit parameter.
    let resolved_board_path =
        effective_board_path.or_else(|| ctx.resolve_store_path().map(|s| s.to_string()));
    let active_handle = resolve_handle(state, resolved_board_path).await.ok();
    if let Some(ref handle) = active_handle {
        ctx.set_extension(Arc::clone(&handle.ctx));
        // Set EntityContext extension for entity-layer commands.
        if let Ok(ectx_arc) = handle.ctx.entity_context().await {
            ctx.set_extension(ectx_arc);
        }
        // Set StoreContext extension for undo/redo commands.
        ctx.set_extension(Arc::clone(&handle.store_context));
    }

    // Inject ClipboardProvider so commands can read/write the system clipboard.
    // Wrapped in ClipboardProviderExt (a sized newtype) for CommandContext storage.
    let clipboard_ext = swissarmyhammer_kanban::clipboard::ClipboardProviderExt(Arc::new(
        crate::state::TauriClipboardProvider::new(app.clone()),
    ));
    ctx.set_extension(Arc::new(clipboard_ext));

    // Check availability
    if !cmd_impl.available(&ctx) {
        tracing::warn!(cmd = %effective_cmd, "command not available in current context");
        return Err(format!("Command not available: {}", effective_cmd));
    }

    // Execute
    tracing::debug!(cmd = %effective_cmd, "executing command");
    let result = cmd_impl.execute(&ctx).await.map_err(|e| {
        tracing::error!(cmd = %effective_cmd, error = %e, "command execution failed");
        format!("Command failed: {}", e)
    })?;

    tracing::info!(cmd = %effective_cmd, undoable = undoable, result = %result, "command completed");

    // Undo stack push is handled automatically inside EntityContext::write()/delete()
    // (wired in the entity crate). No need to push at the dispatch level.

    // Handle board management side effects from file.switchBoard and file.closeBoard.
    // These commands update UIState, but the Tauri layer must also manage BoardHandles.
    if let Some(board_switch) = result.get("BoardSwitch") {
        if let Some(path_str) = board_switch.get("path").and_then(|v| v.as_str()) {
            let board_path = std::path::PathBuf::from(path_str);
            let label = board_switch
                .get("window_label")
                .and_then(|v| v.as_str())
                .unwrap_or("main");

            // Open the board idempotently (also starts file watcher)
            match state.open_board(&board_path, Some(app.clone())).await {
                Ok(canonical) => {
                    // Persist window→board mapping in UIState
                    state
                        .ui_state
                        .set_window_board(label, &canonical.display().to_string());
                    // Update window title to reflect the new board
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
    }

    if let Some(board_close) = result.get("BoardClose") {
        if let Some(path_str) = board_close.get("path").and_then(|v| v.as_str()) {
            let requesting_label = board_close
                .get("window_label")
                .and_then(|v| v.as_str())
                .unwrap_or("main")
                .to_string();

            // Count how many windows currently show this board
            let windows_showing: Vec<String> = state
                .ui_state
                .all_window_boards()
                .into_iter()
                .filter(|(_, bp)| bp == path_str)
                .map(|(label, _)| label)
                .collect();

            let is_last_viewer = windows_showing.len() <= 1;

            if is_last_viewer {
                // Last window showing this board — drop handle and remove from open list
                let target = std::path::PathBuf::from(path_str);
                if let Err(e) = state.close_board(&target).await {
                    tracing::error!(cmd = %effective_cmd, path = %path_str, error = %e, "BoardClose: failed to close board");
                }
                state.ui_state.remove_open_board(path_str);
            } else {
                // Other windows still show this board — just clear this window's assignment
                state.ui_state.set_window_board(&requesting_label, "");
            }

            // Close the requesting window — unless it's the last visible window
            let visible_windows: Vec<_> = app
                .webview_windows()
                .into_iter()
                .filter(|(label, w)| label != "quick-capture" && w.is_visible().unwrap_or(false))
                .collect();

            if visible_windows.len() > 1 {
                if let Some(win) = app.get_webview_window(&requesting_label) {
                    let _ = win.close();
                }
            } else {
                // Last window — keep open, just reset title
                update_window_title(app, &requesting_label, None);
            }

            let _ = app.emit("board-changed", ());
        }
    }

    // Handle UI-triggering command results: dialogs, window creation, quit, reset.
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

    // Emit drag-session-active event when drag.start completes successfully.
    if let Some(drag_start) = result.get("DragStart") {
        let payload = drag_start.clone();
        let _ = app.emit("drag-session-active", &payload);
    }

    // Emit drag-session-cancelled event when drag.cancel completes successfully.
    if let Some(drag_cancel) = result.get("DragCancel") {
        let _ = app.emit("drag-session-cancelled", &drag_cancel);
    }

    // Handle drag.complete result.
    //
    // Same-board: the task.move was already performed inside DragCompleteCmd;
    // the active_handle flush below (undoable=false for drag.complete) won't
    // run, so we flush the board here explicitly then emit drag-session-completed.
    //
    // Cross-board: call the standalone transfer_task() function with both board
    // handles, flush both, then emit drag-session-completed.
    if let Some(drag_complete) = result.get("DragComplete") {
        let session_id = drag_complete
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let transfer_ok = if drag_complete
            .get("cross_board")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            // Cross-board transfer: call transfer_task with both board handles
            let source_path = drag_complete
                .get("source_board_path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let target_path = drag_complete
                .get("target_board_path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let task_id = drag_complete
                .get("task_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let target_column = drag_complete
                .get("target_column")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let drop_index = drag_complete.get("drop_index").and_then(|v| v.as_u64());
            let before_id = drag_complete
                .get("before_id")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let after_id = drag_complete
                .get("after_id")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let copy_mode = drag_complete
                .get("copy_mode")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let source_handle = resolve_handle(state, Some(source_path)).await;
            let target_handle = resolve_handle(state, Some(target_path)).await;

            match (source_handle, target_handle) {
                (Ok(src), Ok(tgt)) => {
                    let transfer_result = swissarmyhammer_kanban::cross_board::transfer_task(
                        &src.ctx,
                        &tgt.ctx,
                        &task_id,
                        &target_column,
                        drop_index,
                        before_id.as_deref(),
                        after_id.as_deref(),
                        copy_mode,
                    )
                    .await;

                    let ok = transfer_result.is_ok();
                    // Flush both boards after transfer
                    flush_and_emit_for_handle(app, &tgt).await;
                    if !copy_mode {
                        flush_and_emit_for_handle(app, &src).await;
                    }
                    ok
                }
                _ => {
                    tracing::error!(
                        "drag.complete: failed to resolve board handles for cross-board transfer"
                    );
                    false
                }
            }
        } else {
            // Same-board: flush the board so entity-changed events go out
            if let Some(ref handle) = active_handle {
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

    // After any UIStateChange, push the full state snapshot to the frontend.
    // This broad approach ensures the React UIStateProvider stays in sync
    // without needing per-field event types. Optimise per-field later if needed.
    if serde_json::from_value::<swissarmyhammer_commands::UIStateChange>(result.clone()).is_ok() {
        let _ = app.emit("ui-state-changed", state.ui_state.to_json());
    }
    // Board switch/close results are not UIStateChanges but still update ui-state.
    if result.get("BoardSwitch").is_some() || result.get("BoardClose").is_some() {
        let _ = app.emit("ui-state-changed", state.ui_state.to_json());
    }

    // Rebuild the native menu when keymap mode changes (accelerators change)
    // or after board switches (command registry may have overrides).
    if effective_cmd.starts_with("settings.keymap.")
        || effective_cmd == "ui.setFocus"
        || result.get("BoardSwitch").is_some()
        || result.get("BoardClose").is_some()
    {
        menu::rebuild_menu_async(app).await;
    }

    // For commands that mutate entity data, scan entity files for changes
    // and emit granular entity-level events. This also updates the watcher
    // cache so the file watcher won't double-fire for our own writes.
    // Undo/redo are non-undoable (they must not push onto the undo stack) but
    // they DO mutate entities on disk, so we still need to flush and emit.
    let needs_flush = undoable || effective_cmd == "app.undo" || effective_cmd == "app.redo";
    tracing::info!(cmd = %effective_cmd, undoable = undoable, needs_flush = needs_flush, has_handle = active_handle.is_some(), "flush gate");
    if needs_flush {
        if let Some(ref handle) = active_handle {
            flush_and_emit_for_handle(app, handle).await;
            // Sync UIState's cached undo/redo flags from the StoreContext so that
            // Command::available() (synchronous) returns accurate results for
            // menu-item enabled state.
            state.ui_state.set_undo_redo_state(
                handle.store_context.can_undo().await,
                handle.store_context.can_redo().await,
            );
            // Refresh window titles from the board entity display name.
            // Catches board name edits, undo/redo of name changes, etc.
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
        } else {
            tracing::info!(cmd = %effective_cmd, "needs_flush but no active_handle — events NOT emitted");
        }
    } else {
        tracing::info!(cmd = %effective_cmd, "non-mutating — skipping flush_and_emit");
    }

    // Update all menu item enabled states after every command.
    // Each menu item's command checks its own available() against current scope/clipboard.
    menu::update_menu_enabled_state(state);

    // Wrap result with undoable info
    Ok(json!({
        "result": result,
        "undoable": undoable,
    }))
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

    // Build dynamic sources: views from the active board, open boards from UIState.
    let dynamic = {
        use swissarmyhammer_kanban::scope_commands::{
            BoardInfo, DynamicSources, ViewInfo, WindowInfo,
        };

        // Gather views from the active board handle
        let mut views = Vec::new();
        if let Some(handle) = active_handle.as_ref() {
            if let Some(views_lock) = handle.ctx.views() {
                if let Ok(vc) = views_lock.try_read() {
                    for v in vc.all_views() {
                        views.push(ViewInfo {
                            id: v.id.clone(),
                            name: v.name.clone(),
                        });
                    }
                }
            }
        }

        // Gather open boards from UIState, enriched with entity display names.
        let open_paths = state.ui_state.open_boards();
        let boards_lock = state.boards.read().await;
        let mut boards: Vec<BoardInfo> = Vec::new();
        for path in &open_paths {
            let p = std::path::Path::new(path);
            // Filesystem fallback: parent directory name.
            let dir_name = p
                .parent()
                .and_then(|parent| parent.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("Board")
                .to_string();
            // Read entity display name from the board entity's `name` field.
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
            boards.push(BoardInfo {
                path: path.clone(),
                name: dir_name,
                entity_name,
                context_name,
            });
        }
        drop(boards_lock);

        // Gather open windows from Tauri
        let windows: Vec<WindowInfo> = app
            .webview_windows()
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
            .collect();

        DynamicSources {
            views,
            boards,
            windows,
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
                // The watcher didn't detect a change (hash unchanged =
                // idempotent write). Nothing actually changed on disk.
                tracing::debug!(
                    entity_type = store_name,
                    id = id,
                    "store item-changed but watcher saw no diff — skipping"
                );
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
        let (entity_type, id, changes) = match evt {
            crate::watcher::WatchEvent::EntityFieldChanged {
                entity_type,
                id,
                changes,
            } => (entity_type.as_str(), id.as_str(), changes),
            _ => continue,
        };

        // Identify computed field names for this entity type.
        let field_defs = fields_ctx.fields_for_entity(entity_type);
        let computed_names: Vec<&str> = field_defs
            .iter()
            .filter(|fd| matches!(fd.type_, swissarmyhammer_fields::FieldType::Computed { .. }))
            .map(|fd| fd.name.as_str())
            .collect();
        if computed_names.is_empty() {
            continue;
        }

        // Read the entity through EntityContext to get derived computed values.
        let entity = match ectx.read(entity_type, id).await {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(
                    entity_type = entity_type,
                    id = id,
                    "enrich_computed_fields: failed to read entity: {e}"
                );
                continue;
            }
        };

        // Append computed field values that aren't already in the changes.
        let existing: std::collections::HashSet<String> =
            changes.iter().map(|c| c.field.clone()).collect();
        for name in computed_names {
            if existing.contains(name) {
                continue;
            }
            if let Some(value) = entity.fields.get(name) {
                changes.push(crate::watcher::FieldChange {
                    field: name.to_string(),
                    value: value.clone(),
                });
            }
        }
    }

    events
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
}
