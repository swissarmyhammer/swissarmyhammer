//! Tauri commands for board operations.

use crate::menu;
use crate::state::AppState;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer_kanban::{
    board::GetBoard,
    task_helpers::{enrich_all_task_entities, enrich_task_entity},
    OperationProcessor,
};
use tauri::menu::{ContextMenu, MenuBuilder};
use tauri::{AppHandle, Emitter, Manager, State, Window};

/// A single menu item entry received from the frontend manifest.
///
/// The frontend collects commands with `menuPlacement` metadata and sends
/// them as a JSON array of these entries. Rust uses them to build the
/// native menu bar.
#[derive(serde::Deserialize, Debug)]
pub struct MenuItemEntry {
    pub id: String,
    pub name: String,
    pub menu: String,
    pub group: usize,
    pub order: usize,
    pub accelerator: Option<String>,
    pub radio_group: Option<String>,
    pub checked: Option<bool>,
}

/// Open a board at the given path, resolving to its .kanban directory.
#[tauri::command]
pub async fn open_board(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<Value, String> {
    let canonical = state.open_board(&PathBuf::from(&path), Some(app)).await?;

    // Return the board data
    let handle = state
        .active_handle()
        .await
        .ok_or("Failed to get board after open")?;
    let board = handle
        .processor
        .process(&GetBoard::default(), &handle.ctx)
        .await
        .map_err(|e| e.to_string())?;

    Ok(json!({
        "path": canonical.display().to_string(),
        "board": board,
    }))
}

/// List all currently open boards.
#[tauri::command]
pub async fn list_open_boards(state: State<'_, AppState>) -> Result<Value, String> {
    let boards = state.boards.read().await;
    let active = state.active_board.read().await;

    let list: Vec<Value> = boards
        .keys()
        .map(|path| {
            let is_active = active.as_ref() == Some(path);
            json!({
                "path": path.display().to_string(),
                "is_active": is_active,
            })
        })
        .collect();

    Ok(json!(list))
}

/// Set the active board to the specified path.
#[tauri::command]
pub async fn set_active_board(state: State<'_, AppState>, path: String) -> Result<Value, String> {
    let canonical = PathBuf::from(&path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(&path));

    let boards = state.boards.read().await;
    if !boards.contains_key(&canonical) {
        return Err(format!("Board not open: {}", path));
    }
    drop(boards);

    *state.active_board.write().await = Some(canonical.clone());

    Ok(json!({
        "path": canonical.display().to_string(),
        "active": true,
    }))
}

/// Get the MRU list of recently opened boards.
#[tauri::command]
pub async fn get_recent_boards(state: State<'_, AppState>) -> Result<Value, String> {
    let config = state.config.read().await;
    serde_json::to_value(&config.recent_boards).map_err(|e| e.to_string())
}

/// Get the current editor keymap mode.
#[tauri::command]
pub async fn get_keymap_mode(state: State<'_, AppState>) -> Result<String, String> {
    let config = state.config.read().await;
    Ok(config.keymap_mode.clone())
}

/// Set the editor keymap mode and persist to config.
///
/// The frontend handles menu sync via `syncMenuToNative` after keymap
/// changes, so we no longer rebuild the native menu here.
#[tauri::command]
pub async fn set_keymap_mode(state: State<'_, AppState>, mode: String) -> Result<Value, String> {
    {
        let mut config = state.config.write().await;
        config.keymap_mode = mode.clone();
        config.save().map_err(|e| e.to_string())?;
    }
    Ok(json!({ "keymap_mode": mode }))
}

/// Get the persisted UI context (active view + inspector stack).
///
/// The frontend calls this on mount to restore state across hot reloads.
#[tauri::command]
pub async fn get_ui_context(state: State<'_, AppState>) -> Result<Value, String> {
    let config = state.config.read().await;
    Ok(json!({
        "active_view_id": config.active_view_id,
        "inspector_stack": config.inspector_stack,
    }))
}

/// Persist the active view ID to config.
#[tauri::command]
pub async fn set_active_view(state: State<'_, AppState>, view_id: String) -> Result<Value, String> {
    let mut config = state.config.write().await;
    config.active_view_id = Some(view_id.clone());
    config.save().map_err(|e| e.to_string())?;
    Ok(json!({ "active_view_id": view_id }))
}

/// Persist the inspector panel stack to config.
#[tauri::command]
pub async fn set_inspector_stack(
    state: State<'_, AppState>,
    stack: Vec<String>,
) -> Result<Value, String> {
    let mut config = state.config.write().await;
    config.inspector_stack = stack.clone();
    config.save().map_err(|e| e.to_string())?;
    Ok(json!({ "inspector_stack": stack }))
}

/// Get the field+entity schema for a given entity type.
///
/// Returns the EntityDef plus each resolved FieldDef, serialized as JSON.
#[tauri::command]
pub async fn get_entity_schema(
    state: State<'_, AppState>,
    entity_type: String,
) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;
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
) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;
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
) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;
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
) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;
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

    let entities = ectx
        .list(&entity_type)
        .await
        .map_err(|e| e.to_string())?;

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

/// Get the board data with all entities as raw entity bags.
///
/// Columns, swimlanes, and tags are returned as `Entity::to_json()` with
/// computed count fields injected. Tasks are NOT included (use `list_entities`
/// for that). A summary object provides aggregate counts.
#[tauri::command]
pub async fn get_board_data(state: State<'_, AppState>) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;
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
/// Deletes the plugin's `.window-state.json` then hard-exits via
/// `std::process::exit` so the plugin's shutdown hook can't re-save.
/// The user needs to relaunch — `app.restart()` would trigger the
/// plugin's save-on-exit, recreating the file we just deleted.
#[tauri::command]
pub async fn reset_windows(app: AppHandle) -> Result<(), String> {
    if let Some(config_dir) = app.path().app_config_dir().ok() {
        let state_file = config_dir.join(".window-state.json");
        if state_file.exists() {
            std::fs::remove_file(&state_file)
                .map_err(|e| format!("Failed to remove window state: {e}"))?;
            tracing::info!(?state_file, "Removed window state file");
        }
    }
    // Hard-exit — bypasses plugin shutdown hooks that would re-save the state.
    std::process::exit(0);
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

/// List all view definitions, returning a JSON array.
#[tauri::command]
pub async fn list_views(state: State<'_, AppState>) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;
    let views_lock = handle.ctx.views().ok_or("Views not initialized")?;
    let views = views_lock.read().await;

    let views_json: Vec<Value> = views
        .all_views()
        .iter()
        .map(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(json!(views_json))
}

/// Rebuild the native menu bar from a frontend-generated manifest.
///
/// The frontend collects all commands with `menuPlacement` metadata, builds
/// a sorted manifest, and sends it here. Rust constructs the native menu
/// from the manifest entries, injecting OS chrome (About, Quit, Hide, etc.)
/// and the Open Recent submenu.
#[tauri::command]
pub async fn rebuild_menu_from_manifest(
    app: AppHandle,
    state: State<'_, AppState>,
    manifest: Vec<MenuItemEntry>,
) -> Result<(), String> {
    let config = state.config.read().await;
    menu::build_menu_from_manifest(&app, &manifest, &config.recent_boards)
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// set_focus — store the current focus scope chain
// ---------------------------------------------------------------------------

/// Store the current focus scope chain from the frontend.
///
/// The scope chain is an ordered list of `type:id` monikers representing
/// the focused element hierarchy. It is used by `dispatch_command` when
/// no explicit scope chain is provided.
#[tauri::command]
pub async fn set_focus(state: State<'_, AppState>, scope_chain: Vec<String>) -> Result<(), String> {
    tracing::debug!(scope_chain = ?scope_chain, "set_focus");
    *state.focus_scope_chain.write().await = scope_chain;
    Ok(())
}

// ---------------------------------------------------------------------------
// dispatch_command — unified command dispatcher via Command trait
// ---------------------------------------------------------------------------

/// Unified command dispatcher that routes a `cmd` string through the
/// `Command` trait system.
///
/// Looks up the command definition in the registry, resolves the scope
/// chain, checks availability, and executes via the trait implementation.
#[tauri::command]
pub async fn dispatch_command(
    app: AppHandle,
    state: State<'_, AppState>,
    cmd: String,
    scope_chain: Option<Vec<String>>,
    target: Option<String>,
    args: Option<Value>,
) -> Result<Value, String> {
    // Validate command ID: non-empty, reasonable length, ASCII-only
    if cmd.is_empty() || cmd.len() > 128 || !cmd.is_ascii() {
        return Err(format!("Invalid command ID: {:?}", cmd));
    }

    tracing::info!(
        cmd = %cmd,
        target = ?target,
        args = ?args,
        scope_chain = ?scope_chain,
        "dispatch_command"
    );

    // Resolve scope chain: explicit > stored focus
    let scope = match scope_chain {
        Some(sc) => sc,
        None => state.focus_scope_chain.read().await.clone(),
    };
    tracing::debug!(scope = ?scope, "resolved scope chain");

    // Look up command definition — clone the undoable flag so we don't
    // hold the registry read guard across the async execute call.
    let undoable = {
        let registry = state.commands_registry.read().await;
        let cmd_def = registry
            .get(&cmd)
            .ok_or_else(|| format!("Unknown command: {}", cmd))?;
        cmd_def.undoable
    };

    // Look up command implementation
    let cmd_impl = state
        .command_impls
        .get(&cmd)
        .ok_or_else(|| format!("No implementation for command: {}", cmd))?;

    // Build CommandContext
    let args_map: HashMap<String, Value> = match args {
        Some(Value::Object(map)) => map.into_iter().collect(),
        _ => HashMap::new(),
    };
    let mut ctx =
        swissarmyhammer_commands::CommandContext::new(cmd.clone(), scope, target, args_map);
    ctx = ctx.with_ui_state(Arc::clone(&state.ui_state));

    // Set KanbanContext extension if board is open
    if let Some(handle) = state.active_handle().await {
        ctx.set_extension(Arc::clone(&handle.ctx));
    }

    // Check availability
    if !cmd_impl.available(&ctx) {
        tracing::warn!(cmd = %cmd, "command not available in current context");
        return Err(format!("Command not available: {}", cmd));
    }

    // Execute
    tracing::debug!(cmd = %cmd, "executing command");
    let result = cmd_impl.execute(&ctx).await.map_err(|e| {
        tracing::error!(cmd = %cmd, error = %e, "command execution failed");
        format!("Command failed: {}", e)
    })?;

    tracing::info!(cmd = %cmd, undoable = undoable, result = %result, "command completed");

    // For undoable commands (data mutations), scan entity files for changes
    // and emit granular entity-level events. This also updates the watcher
    // cache so the file watcher won't double-fire for our own writes.
    //
    // Events are enriched with the full entity state (including computed
    // fields like tags derived from body) by re-reading through the entity
    // context after detecting raw file changes.
    if undoable {
        if let Some(handle) = state.active_handle().await {
            let kanban_root = handle.ctx.root().to_path_buf();
            let mut events = crate::watcher::flush_and_emit(&kanban_root, &handle.entity_cache);

            // Enrich events with full entity state (including computed fields
            // like tags derived from body, progress, etc.) by re-reading
            // through the entity context.
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
            }

            for evt in events {
                let event_name = match &evt {
                    crate::watcher::WatchEvent::EntityCreated { .. } => "entity-created",
                    crate::watcher::WatchEvent::EntityRemoved { .. } => "entity-removed",
                    crate::watcher::WatchEvent::EntityFieldChanged { .. } => "entity-field-changed",
                };
                let _ = app.emit(event_name, &evt);
            }
        }
    }

    // Wrap result with undoable info
    Ok(json!({
        "result": result,
        "undoable": undoable,
    }))
}

// ---------------------------------------------------------------------------
// list_available_commands — return command defs filtered by scope + availability
// ---------------------------------------------------------------------------

/// List command definitions that are available in the current focus context.
///
/// Filters by both static scope requirements and dynamic `Command::available()`
/// checks. Optionally filters to only context-menu commands.
#[tauri::command]
pub async fn list_available_commands(
    state: State<'_, AppState>,
    context_menu: Option<bool>,
) -> Result<Value, String> {
    let scope = state.focus_scope_chain.read().await.clone();

    // Clone filtered defs so we can drop the registry read guard before
    // awaiting active_handle (avoids holding RwLock across an await point).
    let mut available: Vec<swissarmyhammer_commands::CommandDef> = {
        let registry = state.commands_registry.read().await;
        registry
            .available_commands(&scope)
            .into_iter()
            .cloned()
            .collect()
    };

    // Dynamic availability check — build a reusable context template and
    // only swap the command_id per iteration to avoid repeated allocations.
    let active_handle = state.active_handle().await;
    let empty_args: HashMap<String, Value> = HashMap::new();
    available.retain(|def| {
        if let Some(cmd_impl) = state.command_impls.get(&def.id) {
            let mut ctx = swissarmyhammer_commands::CommandContext::new(
                &def.id,
                scope.clone(),
                None,
                empty_args.clone(),
            );
            ctx = ctx.with_ui_state(Arc::clone(&state.ui_state));
            if let Some(ref handle) = active_handle {
                ctx.set_extension(Arc::clone(&handle.ctx));
            }
            cmd_impl.available(&ctx)
        } else {
            false
        }
    });

    // Filter to context menu commands if requested
    if context_menu == Some(true) {
        available.retain(|def| def.context_menu);
    }

    serde_json::to_value(&available).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// show_context_menu — generic native context menu
// ---------------------------------------------------------------------------

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

    // Store IDs so handle_menu_event can route selections correctly
    {
        let mut ids = state.context_menu_ids.write().await;
        ids.clear();
        for item in &items {
            ids.insert(item.id.clone());
        }
    }

    let mut builder = MenuBuilder::new(&app);
    for item in &items {
        builder = builder.text(&item.id, &item.name);
    }
    let menu = builder.build().map_err(|e| e.to_string())?;
    menu.popup(window)
        .map_err(|e: tauri::Error| e.to_string())?;

    Ok(())
}
