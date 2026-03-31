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
        // Read the board entity name if available
        let name = match handle.ctx.entity_context().await {
            Ok(ectx) => match ectx.read("board", "board").await {
                Ok(entity) => entity
                    .fields
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                Err(_) => String::new(),
            },
            Err(_) => String::new(),
        };
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
        enrich_all_task_entities(&mut entities, &terminal_id);

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
        enrich_task_entity(&mut entity, &all_tasks, &terminal_id);
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
/// Columns, swimlanes, and tags are returned as `Entity::to_json()` with
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

    // Read and sort swimlanes by order
    let mut swimlanes = ectx
        .list("swimlane")
        .await
        .map_err(|e| format!("get_board_data: {}", e))?;
    swimlanes.sort_by_key(|s| s.get("order").and_then(|v| v.as_u64()).unwrap_or(0) as usize);

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

    // Count tasks per swimlane
    let mut swimlane_counts: HashMap<String, usize> = HashMap::new();
    for task in &all_tasks {
        if let Some(sl) = task.get_str("position_swimlane") {
            if !sl.is_empty() {
                *swimlane_counts.entry(sl.to_string()).or_insert(0) += 1;
            }
        }
    }

    // Count tasks per tag name
    let mut tag_counts: HashMap<String, usize> = HashMap::new();
    for task in &all_tasks {
        for tag_name in swissarmyhammer_kanban::task_helpers::task_tags(task) {
            *tag_counts.entry(tag_name).or_insert(0) += 1;
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

    // Serialize swimlanes with injected task_count
    let swimlanes_json: Vec<Value> = swimlanes
        .iter()
        .map(|sl| {
            let mut e = sl.clone();
            let count = swimlane_counts.get(sl.id.as_str()).copied().unwrap_or(0);
            e.set("task_count", json!(count));
            e.to_json()
        })
        .collect();

    // Serialize tags with injected task_count
    let tags_json: Vec<Value> = tags
        .iter()
        .map(|tag| {
            let mut e = tag.clone();
            let tag_name = tag.get_str("tag_name").unwrap_or("");
            let count = tag_counts.get(tag_name).copied().unwrap_or(0);
            e.set("task_count", json!(count));
            e.to_json()
        })
        .collect();

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
        "swimlanes": swimlanes_json,
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

/// Reset saved window positions/sizes and exit.
///
/// Clear all saved window geometry (main + secondary) and restart.
#[tauri::command]
#[allow(unreachable_code)]
pub async fn reset_windows(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    // Clear window state from UIState (geometry + inspector stacks)
    state.ui_state.clear_windows();
    tracing::info!("Cleared all window state");
    app.restart();
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
    if let Err(e) = create_window_impl(app, state, None, None, None, true).await {
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
    create_window_impl(&app, &state, board_path, None, None, true).await
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
    rebuild_menu: bool,
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

        // Set window title from board display name
        let board_path = std::path::PathBuf::from(bp);
        let canonical = board_path
            .canonicalize()
            .unwrap_or_else(|_| board_path.clone());
        let boards = state.boards.read().await;
        if let Some(handle) = boards.get(&canonical) {
            update_window_title(&app, &label, Some(handle.ctx.name()));
        }
    }

    if rebuild_menu {
        menu::rebuild_menu(&app);
    }

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

    let ectx = handle
        .ctx
        .entity_context()
        .await
        .map_err(|e| e.to_string())?;
    let stack = ectx.undo_stack().await;
    Ok(json!({
        "can_undo": stack.can_undo(),
        "can_redo": stack.can_redo(),
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
    let active_handle = resolve_handle(state, effective_board_path).await.ok();
    if let Some(ref handle) = active_handle {
        ctx.set_extension(Arc::clone(&handle.ctx));
        // Set EntityContext extension for entity-layer commands (undo/redo).
        if let Ok(ectx_arc) = handle.ctx.entity_context().await {
            ctx.set_extension(ectx_arc);
        }
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
                        update_window_title(app, label, Some(handle.ctx.name()));
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
            // Find which window(s) had this board so we can reset their titles
            let close_labels: Vec<String> = state
                .ui_state
                .all_window_boards()
                .into_iter()
                .filter(|(_, bp)| bp == path_str)
                .map(|(label, _)| label)
                .collect();

            let target = std::path::PathBuf::from(path_str);
            if let Err(e) = state.close_board(&target).await {
                tracing::error!(cmd = %effective_cmd, path = %path_str, error = %e, "BoardClose: failed to close board");
            }
            // Reset window titles back to base name
            for label in &close_labels {
                update_window_title(app, label, None);
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
    if result.get("ResetWindows").is_some() {
        app.restart();
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
        || result.get("BoardSwitch").is_some()
        || result.get("BoardClose").is_some()
    {
        menu::rebuild_menu(app);
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
            // Sync UIState's cached undo/redo flags from the undo stack so that
            // Command::available() (synchronous) returns accurate results for
            // menu-item enabled state.
            if let Ok(ectx) = handle.ctx.entity_context().await {
                let stack = ectx.undo_stack().await;
                state
                    .ui_state
                    .set_undo_redo_state(stack.can_undo(), stack.can_redo());
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

        // Gather open boards from UIState, enriched with context names from handles.
        let open_paths = state.ui_state.open_boards();
        let boards_lock = state.boards.read().await;
        let boards: Vec<BoardInfo> = open_paths
            .iter()
            .map(|path| {
                // Use the parent directory name as the display name.
                // Board paths end in `.kanban`, so file_name() would just
                // return ".kanban" — the parent is the meaningful project name.
                let p = std::path::Path::new(path);
                let name = p
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("Board")
                    .to_string();
                // Pull context_name from the board handle if available.
                let context_name = boards_lock
                    .get(p)
                    .map(|h| h.ctx.name().to_string())
                    .unwrap_or_else(|| name.clone());
                BoardInfo {
                    path: path.clone(),
                    name: name.clone(),
                    entity_name: name,
                    context_name,
                }
            })
            .collect();
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
async fn flush_and_emit_for_handle(app: &AppHandle, handle: &BoardHandle) {
    let kanban_root = handle.ctx.root().to_path_buf();
    let mut events = crate::watcher::flush_and_emit(&kanban_root, &handle.entity_cache);
    tracing::info!(event_count = events.len(), path = %kanban_root.display(), "flush_and_emit result");
    tracing::debug!(event_count = events.len(), path = %kanban_root.display(), "flush_and_emit_for_handle");
    if let Ok(ectx) = handle.ctx.entity_context().await {
        for evt in &mut events {
            match evt {
                crate::watcher::WatchEvent::EntityCreated {
                    entity_type,
                    id,
                    fields,
                } => {
                    if let Ok(entity) = ectx.read(entity_type, id).await {
                        *fields = entity
                            .fields
                            .into_iter()
                            .map(|(k, v)| (k.to_string(), v))
                            .collect();
                    }
                }
                crate::watcher::WatchEvent::EntityFieldChanged {
                    entity_type,
                    id,
                    fields,
                    ..
                } => {
                    if let Ok(entity) = ectx.read(entity_type, id).await {
                        *fields = Some(
                            entity
                                .fields
                                .into_iter()
                                .map(|(k, v)| (k.to_string(), v))
                                .collect(),
                        );
                    }
                }
                crate::watcher::WatchEvent::EntityRemoved { .. } => {}
            }
        }

        // Cascade: recompute aggregate fields that depend on changed entity types
        let cascade = cascade_aggregate_events(&ectx, &events).await;
        events.extend(cascade);
    }
    {
        let board_path_str = kanban_root.display().to_string();
        let mut search_idx = handle.search_index.write().await;
        for evt in events {
            crate::watcher::sync_search_index(&mut search_idx, &evt);
            let event_name = match &evt {
                crate::watcher::WatchEvent::EntityCreated { .. } => "entity-created",
                crate::watcher::WatchEvent::EntityRemoved { .. } => "entity-removed",
                crate::watcher::WatchEvent::EntityFieldChanged { .. } => "entity-field-changed",
            };
            let wrapped = crate::watcher::BoardWatchEvent {
                event: evt,
                board_path: board_path_str.clone(),
            };
            let _ = app.emit(event_name, &wrapped);
        }
    }
}

/// Check if any aggregate computed fields depend on the changed entity types,
/// and if so, recompute them and produce additional `entity-field-changed` events.
///
/// Reads `depends_on` from field definitions — no hardcoded logic per command.
async fn cascade_aggregate_events(
    ectx: &swissarmyhammer_entity::EntityContext,
    primary_events: &[crate::watcher::WatchEvent],
) -> Vec<crate::watcher::WatchEvent> {
    use std::collections::HashSet;

    // Collect entity types that changed
    let changed_types: HashSet<&str> = primary_events
        .iter()
        .map(|evt| match evt {
            crate::watcher::WatchEvent::EntityCreated { entity_type, .. }
            | crate::watcher::WatchEvent::EntityFieldChanged { entity_type, .. }
            | crate::watcher::WatchEvent::EntityRemoved { entity_type, .. } => entity_type.as_str(),
        })
        .collect();

    if changed_types.is_empty() {
        return vec![];
    }

    let fields_ctx = ectx.fields();
    let mut cascade_events = Vec::new();

    // Find entity types with aggregate fields depending on the changed types
    let mut dependent_types: HashSet<&str> = HashSet::new();
    for trigger_type in &changed_types {
        for dep_type in fields_ctx.entity_types_depending_on(trigger_type) {
            dependent_types.insert(dep_type);
        }
    }

    // Re-read dependent entities to trigger recomputation
    for entity_type in dependent_types {
        if let Ok(entities) = ectx.list(entity_type).await {
            for entity in entities {
                let all_fields: std::collections::HashMap<String, serde_json::Value> = entity
                    .fields
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect();
                cascade_events.push(crate::watcher::WatchEvent::EntityFieldChanged {
                    entity_type: entity_type.to_string(),
                    id: entity.id.to_string(),
                    changes: vec![],
                    fields: Some(all_fields),
                });
            }
        }
    }

    cascade_events
}

/// A single item in a generic context menu.
#[derive(serde::Deserialize)]
pub struct ContextMenuItem {
    pub id: String,
    pub name: String,
}

/// Show a native context menu with the given items.
///
/// Menu selections are emitted as `context-menu-command` events to the
/// frontend. The item IDs are stored in AppState so `handle_menu_event`
/// can distinguish them from regular menu bar commands.
#[tauri::command]
pub async fn show_context_menu(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    items: Vec<ContextMenuItem>,
) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }

    // Store IDs so handle_menu_event can route selections correctly.
    // Separators are not selectable items — exclude them from the id set.
    {
        let ids: std::collections::HashSet<String> = items
            .iter()
            .filter(|item| item.id != "__separator__")
            .map(|item| item.id.clone())
            .collect();
        state.ui_state.set_context_menu_ids(ids);
    }

    let mut builder = MenuBuilder::new(&app);
    for item in &items {
        if item.id == "__separator__" {
            builder = builder.separator();
        } else {
            builder = builder.text(&item.id, &item.name);
        }
    }
    let menu = builder.build().map_err(|e| e.to_string())?;
    menu.popup(window)
        .map_err(|e: tauri::Error| e.to_string())?;

    Ok(())
}
