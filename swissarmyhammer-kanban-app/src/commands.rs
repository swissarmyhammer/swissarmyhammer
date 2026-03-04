//! Tauri commands for board operations.

use crate::menu;
use crate::state::AppState;
use serde_json::{json, Value};
use std::path::PathBuf;
use swissarmyhammer_kanban::{
    actor::DeleteActor,
    attachment::DeleteAttachment,
    board::GetBoard,
    column::{DeleteColumn, UpdateColumn},
    swimlane::DeleteSwimlane,
    tag::{DeleteTag, UpdateTag},
    task::{AddTask, DeleteTask, ListTasks, MoveTask, UntagTask},
    types::{Ordinal, Position},
    EntityContext, OperationProcessor,
};
use tauri::menu::{ContextMenu, MenuBuilder, PredefinedMenuItem};
use tauri::{AppHandle, State, Window};

/// Get the board metadata for the active (or specified) board.
#[tauri::command]
pub async fn get_board(state: State<'_, AppState>, path: Option<String>) -> Result<Value, String> {
    let handle = if let Some(p) = path {
        let canonical = PathBuf::from(&p)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&p));
        let boards = state.boards.read().await;
        boards
            .get(&canonical)
            .cloned()
            .ok_or_else(|| format!("Board not open: {}", p))?
    } else {
        state.active_handle().await.ok_or("No active board")?
    };

    let result = handle
        .processor
        .process(&GetBoard::default(), &handle.ctx)
        .await
        .map_err(|e| e.to_string())?;

    Ok(result)
}

/// List tasks for the active (or specified) board.
#[tauri::command]
pub async fn list_tasks(state: State<'_, AppState>, path: Option<String>) -> Result<Value, String> {
    let handle = if let Some(p) = path {
        let canonical = PathBuf::from(&p)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&p));
        let boards = state.boards.read().await;
        boards
            .get(&canonical)
            .cloned()
            .ok_or_else(|| format!("Board not open: {}", p))?
    } else {
        state.active_handle().await.ok_or("No active board")?
    };

    let result = handle
        .processor
        .process(&ListTasks::new(), &handle.ctx)
        .await
        .map_err(|e| e.to_string())?;

    Ok(result)
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

/// Move a task to a new position (column and/or ordinal).
#[tauri::command]
pub async fn move_task(
    state: State<'_, AppState>,
    id: String,
    column: String,
    ordinal: String,
    swimlane: Option<String>,
) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;

    let mut cmd = MoveTask::to_column(id, column);
    cmd.swimlane = swimlane.map(|s| s.into());
    if !ordinal.is_empty() {
        cmd.ordinal = Some(ordinal);
    }
    let result = handle
        .processor
        .process(&cmd, &handle.ctx)
        .await
        .map_err(|e| e.to_string())?;

    Ok(result)
}

/// Add a new task to the active board.
#[tauri::command]
pub async fn add_task(
    state: State<'_, AppState>,
    title: String,
    column: String,
) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;

    let position = Position::new(column.into(), None, Ordinal::first());
    let cmd = AddTask::new(title).with_position(position);
    let result = handle
        .processor
        .process(&cmd, &handle.ctx)
        .await
        .map_err(|e| e.to_string())?;

    Ok(result)
}

/// Update a tag's name, color, or description.
/// When name changes, bulk find-replaces `#old-name` → `#new-name` across all tasks.
#[tauri::command]
pub async fn update_tag(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    color: Option<String>,
    description: Option<String>,
) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;

    let mut cmd = UpdateTag::new(id);
    if let Some(n) = name {
        cmd = cmd.with_name(n);
    }
    if let Some(c) = color {
        cmd = cmd.with_color(c);
    }
    if let Some(d) = description {
        cmd = cmd.with_description(d);
    }
    let result = handle
        .processor
        .process(&cmd, &handle.ctx)
        .await
        .map_err(|e| e.to_string())?;

    Ok(result)
}

/// Show a native context menu for a tag pill.
#[tauri::command]
pub async fn show_tag_context_menu(
    app: AppHandle,
    window: Window,
    state: State<'_, AppState>,
    tag_id: String,
    task_id: Option<String>,
) -> Result<(), String> {
    // Store context for the menu event handler
    *state.context_tag.write().await = Some((tag_id, task_id));

    let menu = MenuBuilder::new(&app)
        .text("tag_edit", "Edit Tag\u{2026}")
        .item(&PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?)
        .text("tag_delete", "Remove Tag")
        .build()
        .map_err(|e| e.to_string())?;

    menu.popup(window)
        .map_err(|e: tauri::Error| e.to_string())?;

    Ok(())
}

/// Remove a tag from a task's markdown (does NOT delete the tag file).
#[tauri::command]
pub async fn untag_task(
    state: State<'_, AppState>,
    id: String,
    tag: String,
) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;

    let cmd = UntagTask::new(id, tag);
    let result = handle
        .processor
        .process(&cmd, &handle.ctx)
        .await
        .map_err(|e| e.to_string())?;

    Ok(result)
}

/// Reorder columns by updating their order fields.
///
/// Takes a list of {id, order} pairs and applies them. Each column update
/// goes through the processor and gets its own transaction. Returns the
/// list of `operation_id` values (one per column updated).
#[tauri::command]
pub async fn reorder_columns(
    state: State<'_, AppState>,
    columns: Vec<ColumnOrder>,
) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;

    let mut operation_ids: Vec<String> = Vec::new();
    for col in &columns {
        let cmd = UpdateColumn::new(col.id.clone()).with_order(col.order);
        let result = handle
            .processor
            .process(&cmd, &handle.ctx)
            .await
            .map_err(|e| e.to_string())?;
        if let Some(op_id) = result.get("operation_id").and_then(|v| v.as_str()) {
            operation_ids.push(op_id.to_string());
        }
    }

    Ok(json!({ "updated": columns.len(), "operation_ids": operation_ids }))
}

#[derive(serde::Deserialize)]
pub struct ColumnOrder {
    pub id: String,
    pub order: usize,
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

/// Set the editor keymap mode, persist to config, and rebuild the menu.
#[tauri::command]
pub async fn set_keymap_mode(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: String,
) -> Result<Value, String> {
    {
        let mut config = state.config.write().await;
        config.keymap_mode = mode.clone();
        config.save().map_err(|e| e.to_string())?;
    }
    menu::rebuild_menu(&app);
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
    let ectx = handle.ctx.entity_context().await.map_err(|e| e.to_string())?;
    let fields_ctx = ectx.fields();

    let entity_def = fields_ctx
        .get_entity(&entity_type)
        .ok_or_else(|| format!("Unknown entity type: {}", entity_type))?;

    let field_defs: Vec<Value> = fields_ctx
        .fields_for_entity(&entity_type)
        .iter()
        .map(|f| serde_json::to_value(f).map_err(|e| format!("failed to serialize field '{}': {}", f.name, e)))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(json!({
        "entity": serde_json::to_value(entity_def).map_err(|e| e.to_string())?,
        "fields": field_defs,
    }))
}

/// Update a single field on an entity.
///
/// Generic command that works with any entity type. Wraps the write in a
/// transaction so the returned `operation_id` can be used for undo/redo.
#[tauri::command]
pub async fn update_entity_field(
    state: State<'_, AppState>,
    entity_type: String,
    id: String,
    field_name: String,
    value: Value,
) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;
    let ectx = handle.ctx.entity_context().await.map_err(|e| e.to_string())?;

    // Validate field_name against the entity's schema
    let entity_def = ectx.entity_def(&entity_type).map_err(|e| e.to_string())?;
    if !entity_def.fields.contains(&field_name) {
        return Err(format!(
            "field '{}' is not defined for entity type '{}'",
            field_name, entity_type
        ));
    }

    let mut entity = ectx
        .read(&entity_type, &id)
        .await
        .map_err(|e| e.to_string())?;

    if value.is_null() {
        entity.remove(&field_name);
    } else {
        entity.set(&field_name, value);
    }

    // Wrap in a transaction so the operation_id is trackable for undo/redo
    let tx_id = EntityContext::generate_transaction_id();
    ectx.set_transaction(tx_id.clone()).await;
    ectx.write(&entity).await.map_err(|e| e.to_string())?;
    ectx.clear_transaction().await;

    let mut result = entity.to_json();
    if let Some(obj) = result.as_object_mut() {
        obj.insert("operation_id".to_string(), Value::String(tx_id));
    }
    Ok(result)
}

/// Delete a task from the active board.
#[tauri::command]
pub async fn delete_task(state: State<'_, AppState>, id: String) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;
    handle
        .processor
        .process(&DeleteTask::new(id), &handle.ctx)
        .await
        .map_err(|e| e.to_string())
}

/// Delete a tag from the active board.
#[tauri::command]
pub async fn delete_tag(state: State<'_, AppState>, id: String) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;
    handle
        .processor
        .process(&DeleteTag::new(id), &handle.ctx)
        .await
        .map_err(|e| e.to_string())
}

/// Delete a column from the active board.
#[tauri::command]
pub async fn delete_column(state: State<'_, AppState>, id: String) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;
    handle
        .processor
        .process(&DeleteColumn::new(id), &handle.ctx)
        .await
        .map_err(|e| e.to_string())
}

/// Delete an actor from the active board.
#[tauri::command]
pub async fn delete_actor(state: State<'_, AppState>, id: String) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;
    handle
        .processor
        .process(&DeleteActor::new(id), &handle.ctx)
        .await
        .map_err(|e| e.to_string())
}

/// Delete a swimlane from the active board.
#[tauri::command]
pub async fn delete_swimlane(state: State<'_, AppState>, id: String) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;
    handle
        .processor
        .process(&DeleteSwimlane::new(id), &handle.ctx)
        .await
        .map_err(|e| e.to_string())
}

/// Delete an attachment from a task on the active board.
#[tauri::command]
pub async fn delete_attachment(
    state: State<'_, AppState>,
    task_id: String,
    id: String,
) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;
    handle
        .processor
        .process(&DeleteAttachment::new(task_id, id), &handle.ctx)
        .await
        .map_err(|e| e.to_string())
}

/// Undo a previously executed operation by its ULID.
///
/// Accepts either a single changelog ULID or a transaction ULID (which
/// undoes all constituent entries in reverse order).
#[tauri::command]
pub async fn undo_operation(state: State<'_, AppState>, id: String) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;
    let ectx = handle.ctx.entity_context().await.map_err(|e| e.to_string())?;
    let result_ulid = ectx.undo(&id).await.map_err(|e| e.to_string())?;
    Ok(json!({ "undone": id, "operation_id": result_ulid }))
}

/// Redo a previously undone operation by its ULID.
///
/// Accepts either a single changelog ULID or a transaction ULID (which
/// redoes all constituent entries in forward order).
#[tauri::command]
pub async fn redo_operation(state: State<'_, AppState>, id: String) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;
    let ectx = handle.ctx.entity_context().await.map_err(|e| e.to_string())?;
    let result_ulid = ectx.redo(&id).await.map_err(|e| e.to_string())?;
    Ok(json!({ "redone": id, "operation_id": result_ulid }))
}
