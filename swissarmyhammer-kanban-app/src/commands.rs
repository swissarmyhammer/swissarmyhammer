//! Tauri commands for board operations.

use crate::menu;
use crate::state::AppState;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use swissarmyhammer_kanban::{
    actor::DeleteActor,
    attachment::DeleteAttachment,
    board::GetBoard,
    column::{DeleteColumn, UpdateColumn},
    entity::UpdateEntityField,
    swimlane::DeleteSwimlane,
    tag::{DeleteTag, UpdateTag},
    task::{AddTask, DeleteTask, MoveTask, UntagTask},
    task_helpers::{enrich_all_task_entities, enrich_task_entity},
    types::{Ordinal, Position},
    OperationProcessor,
};
use tauri::menu::{ContextMenu, MenuBuilder};
use tauri::{AppHandle, State, Window};

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
pub async fn open_board(state: State<'_, AppState>, path: String) -> Result<Value, String> {
    let canonical = state.open_board(&PathBuf::from(&path)).await?;

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
pub async fn set_keymap_mode(
    state: State<'_, AppState>,
    mode: String,
) -> Result<Value, String> {
    {
        let mut config = state.config.write().await;
        config.keymap_mode = mode.clone();
        config.save().map_err(|e| e.to_string())?;
    }
    Ok(json!({ "keymap_mode": mode }))
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
    let terminal_id = columns
        .last()
        .map(|c| c.id.as_str())
        .unwrap_or("done");

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
            let ready = column_ready_counts.get(col.id.as_str()).copied().unwrap_or(0);
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
// Helpers for execute_command dispatcher
// ---------------------------------------------------------------------------

/// Column id + order pair used by the `column.reorder` dispatcher arm.
#[derive(serde::Deserialize)]
struct ColumnOrder {
    id: String,
    order: usize,
}

/// Parse a "type:id" moniker string into (entity_type, id).
///
/// The id portion may itself contain colons (e.g. "task:01JAB:extra" parses
/// as ("task", "01JAB:extra")).
fn parse_moniker(s: &str) -> Result<(&str, &str), String> {
    let (entity_type, id) = s
        .split_once(':')
        .ok_or_else(|| format!("Invalid moniker (no colon): {}", s))?;
    if entity_type.is_empty() {
        return Err(format!("Invalid moniker (empty type): {}", s));
    }
    if id.is_empty() {
        return Err(format!("Invalid moniker (empty id): {}", s));
    }
    Ok((entity_type, id))
}

/// Extract a required string arg from a JSON value.
fn required_str(args: &Value, key: &str, cmd: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("{}: missing required arg '{}'", cmd, key))
}

/// Extract an optional string arg from a JSON value.
fn optional_str(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// execute_command — unified command dispatcher
// ---------------------------------------------------------------------------

/// Unified command dispatcher that routes a `cmd` string and `args` JSON
/// object to the appropriate kanban mutation operation.
///
/// All mutations (entity updates, deletes, task moves, undo/redo, etc.)
/// flow through this single entry point.
#[tauri::command]
pub async fn execute_command(
    _app: AppHandle,
    _window: Window,
    state: State<'_, AppState>,
    cmd: String,
    args: Value,
) -> Result<Value, String> {
    tracing::info!(cmd = %cmd, "execute_command");

    match cmd.as_str() {
        "entity.update_field" => {
            let entity_type = required_str(&args, "entity_type", &cmd)?;
            let id = required_str(&args, "id", &cmd)?;
            let field_name = required_str(&args, "field_name", &cmd)?;
            let value = args
                .get("value")
                .cloned()
                .ok_or_else(|| format!("{}: missing required arg 'value'", cmd))?;

            let handle = state.active_handle().await.ok_or("No active board")?;
            let op =
                UpdateEntityField::new(entity_type.clone(), id.clone(), field_name.clone(), value);
            handle
                .processor
                .process(&op, &handle.ctx)
                .await
                .map_err(|e| format!("execute_command({}): {}", cmd, e))
        }

        "entity.delete" => {
            let moniker = required_str(&args, "moniker", &cmd)?;
            let (entity_type, id) = parse_moniker(&moniker)?;
            let handle = state.active_handle().await.ok_or("No active board")?;

            match entity_type {
                "task" => handle
                    .processor
                    .process(&DeleteTask::new(id), &handle.ctx)
                    .await
                    .map_err(|e| format!("execute_command({}): {}", cmd, e)),
                "tag" => handle
                    .processor
                    .process(&DeleteTag::new(id), &handle.ctx)
                    .await
                    .map_err(|e| format!("execute_command({}): {}", cmd, e)),
                "column" => handle
                    .processor
                    .process(&DeleteColumn::new(id), &handle.ctx)
                    .await
                    .map_err(|e| format!("execute_command({}): {}", cmd, e)),
                "actor" => handle
                    .processor
                    .process(&DeleteActor::new(id), &handle.ctx)
                    .await
                    .map_err(|e| format!("execute_command({}): {}", cmd, e)),
                "swimlane" => handle
                    .processor
                    .process(&DeleteSwimlane::new(id), &handle.ctx)
                    .await
                    .map_err(|e| format!("execute_command({}): {}", cmd, e)),
                _ => Err(format!(
                    "entity.delete: unknown entity type '{}'",
                    entity_type
                )),
            }
        }

        "task.move" => {
            let id = required_str(&args, "id", &cmd)?;
            let column = required_str(&args, "column", &cmd)?;
            let ordinal = optional_str(&args, "ordinal").unwrap_or_default();
            let swimlane = optional_str(&args, "swimlane");

            let handle = state.active_handle().await.ok_or("No active board")?;
            let mut op = MoveTask::to_column(id.clone(), column);
            op.swimlane = swimlane.map(|s| s.into());
            if !ordinal.is_empty() {
                op.ordinal = Some(ordinal);
            }
            handle
                .processor
                .process(&op, &handle.ctx)
                .await
                .map_err(|e| format!("execute_command({}): {}", cmd, e))
        }

        "task.add" => {
            let title = required_str(&args, "title", &cmd)?;
            let column = required_str(&args, "column", &cmd)?;

            let handle = state.active_handle().await.ok_or("No active board")?;
            let position = Position::new(column.into(), None, Ordinal::first());
            let op = AddTask::new(title).with_position(position);
            handle
                .processor
                .process(&op, &handle.ctx)
                .await
                .map_err(|e| format!("execute_command({}): {}", cmd, e))
        }

        "task.untag" => {
            let id = required_str(&args, "id", &cmd)?;
            let tag = required_str(&args, "tag", &cmd)?;

            let handle = state.active_handle().await.ok_or("No active board")?;
            let op = UntagTask::new(id, tag);
            handle
                .processor
                .process(&op, &handle.ctx)
                .await
                .map_err(|e| format!("execute_command({}): {}", cmd, e))
        }

        "tag.update" => {
            let id = required_str(&args, "id", &cmd)?;
            let name = optional_str(&args, "name");
            let color = optional_str(&args, "color");
            let description = optional_str(&args, "description");

            let handle = state.active_handle().await.ok_or("No active board")?;
            let mut op = UpdateTag::new(id);
            if let Some(n) = name {
                op = op.with_name(n);
            }
            if let Some(c) = color {
                op = op.with_color(c);
            }
            if let Some(d) = description {
                op = op.with_description(d);
            }
            handle
                .processor
                .process(&op, &handle.ctx)
                .await
                .map_err(|e| format!("execute_command({}): {}", cmd, e))
        }

        "column.reorder" => {
            let columns_val = args
                .get("columns")
                .ok_or_else(|| format!("{}: missing required arg 'columns'", cmd))?;
            let columns: Vec<ColumnOrder> = serde_json::from_value(columns_val.clone())
                .map_err(|e| format!("{}: invalid 'columns' arg: {}", cmd, e))?;

            let handle = state.active_handle().await.ok_or("No active board")?;
            let mut operation_ids: Vec<String> = Vec::new();
            for col in &columns {
                let op = UpdateColumn::new(col.id.clone()).with_order(col.order);
                let result = handle
                    .processor
                    .process(&op, &handle.ctx)
                    .await
                    .map_err(|e| format!("execute_command({}): {}", cmd, e))?;
                if let Some(op_id) = result.get("operation_id").and_then(|v| v.as_str()) {
                    operation_ids.push(op_id.to_string());
                }
            }
            Ok(json!({ "updated": columns.len(), "operation_ids": operation_ids }))
        }

        "op.undo" => {
            let id = required_str(&args, "id", &cmd)?;

            let handle = state.active_handle().await.ok_or("No active board")?;
            let ectx = handle
                .ctx
                .entity_context()
                .await
                .map_err(|e| e.to_string())?;
            let result_ulid = ectx.undo(&id).await.map_err(|e| e.to_string())?;
            Ok(json!({ "undone": id, "operation_id": result_ulid }))
        }

        "op.redo" => {
            let id = required_str(&args, "id", &cmd)?;

            let handle = state.active_handle().await.ok_or("No active board")?;
            let ectx = handle
                .ctx
                .entity_context()
                .await
                .map_err(|e| e.to_string())?;
            let result_ulid = ectx.redo(&id).await.map_err(|e| e.to_string())?;
            Ok(json!({ "redone": id, "operation_id": result_ulid }))
        }

        "attachment.delete" => {
            let task_id = required_str(&args, "task_id", &cmd)?;
            let id = required_str(&args, "id", &cmd)?;

            let handle = state.active_handle().await.ok_or("No active board")?;
            handle
                .processor
                .process(&DeleteAttachment::new(task_id, id), &handle.ctx)
                .await
                .map_err(|e| format!("execute_command({}): {}", cmd, e))
        }

        _ => Err(format!("Unknown command: {}", cmd)),
    }
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_moniker_valid() {
        let (t, id) = parse_moniker("task:01JAB").unwrap();
        assert_eq!(t, "task");
        assert_eq!(id, "01JAB");
    }

    #[test]
    fn test_parse_moniker_colon_in_id() {
        let (t, id) = parse_moniker("task:01JAB:extra").unwrap();
        assert_eq!(t, "task");
        assert_eq!(id, "01JAB:extra");
    }

    #[test]
    fn test_parse_moniker_no_colon() {
        assert!(parse_moniker("badstring").is_err());
    }

    #[test]
    fn test_parse_moniker_empty_parts() {
        assert!(parse_moniker(":id").is_err());
        assert!(parse_moniker("type:").is_err());
    }

    #[test]
    fn test_required_str_present() {
        let args = serde_json::json!({"name": "hello"});
        assert_eq!(required_str(&args, "name", "test").unwrap(), "hello");
    }

    #[test]
    fn test_required_str_missing() {
        let args = serde_json::json!({});
        let err = required_str(&args, "name", "test.cmd").unwrap_err();
        assert!(err.contains("test.cmd"));
        assert!(err.contains("name"));
    }

    #[test]
    fn test_required_str_not_string() {
        let args = serde_json::json!({"name": 42});
        assert!(required_str(&args, "name", "test").is_err());
    }

    #[test]
    fn test_optional_str() {
        let args = serde_json::json!({"name": "hello"});
        assert_eq!(optional_str(&args, "name"), Some("hello".to_string()));
        assert_eq!(optional_str(&args, "missing"), None);
    }
}

