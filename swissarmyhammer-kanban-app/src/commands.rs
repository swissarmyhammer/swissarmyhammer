//! Tauri commands for board operations.

use crate::state::AppState;
use serde_json::{json, Value};
use std::path::PathBuf;
use swissarmyhammer_kanban::{
    board::GetBoard,
    column::UpdateColumn,
    task::{AddTask, ListTasks, MoveTask, UpdateTask},
    types::{Ordinal, Position},
    OperationProcessor,
};
use tauri::State;

/// Get the board metadata for the active (or specified) board.
#[tauri::command]
pub async fn get_board(
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<Value, String> {
    let handle = if let Some(p) = path {
        let canonical = PathBuf::from(&p)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&p));
        let boards = state.boards.read().await;
        boards.get(&canonical).cloned().ok_or_else(|| {
            format!("Board not open: {}", p)
        })?
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
pub async fn list_tasks(
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<Value, String> {
    let handle = if let Some(p) = path {
        let canonical = PathBuf::from(&p)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&p));
        let boards = state.boards.read().await;
        boards.get(&canonical).cloned().ok_or_else(|| {
            format!("Board not open: {}", p)
        })?
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
pub async fn open_board(
    state: State<'_, AppState>,
    path: String,
) -> Result<Value, String> {
    let canonical = state.open_board(&PathBuf::from(&path)).await?;

    // Return the board data
    let handle = state.active_handle().await.ok_or("Failed to get board after open")?;
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
pub async fn list_open_boards(
    state: State<'_, AppState>,
) -> Result<Value, String> {
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
pub async fn set_active_board(
    state: State<'_, AppState>,
    path: String,
) -> Result<Value, String> {
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

    let position = Position::new(
        column.into(),
        swimlane.map(|s| s.into()),
        if ordinal.is_empty() {
            Ordinal::first()
        } else {
            Ordinal::from_string(&ordinal)
        },
    );

    let cmd = MoveTask::new(id, position);
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

/// Rename a column.
#[tauri::command]
pub async fn rename_column(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;

    let cmd = UpdateColumn::new(id).with_name(name);
    let result = handle
        .processor
        .process(&cmd, &handle.ctx)
        .await
        .map_err(|e| e.to_string())?;

    Ok(result)
}

/// Update a task's title.
#[tauri::command]
pub async fn update_task_title(
    state: State<'_, AppState>,
    id: String,
    title: String,
) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;

    let cmd = UpdateTask::new(id).with_title(title);
    let result = handle
        .processor
        .process(&cmd, &handle.ctx)
        .await
        .map_err(|e| e.to_string())?;

    Ok(result)
}

/// Reorder columns by updating their order fields.
/// Takes a list of {id, order} pairs and applies them.
#[tauri::command]
pub async fn reorder_columns(
    state: State<'_, AppState>,
    columns: Vec<ColumnOrder>,
) -> Result<Value, String> {
    let handle = state.active_handle().await.ok_or("No active board")?;

    for col in &columns {
        let cmd = UpdateColumn::new(col.id.clone()).with_order(col.order);
        handle
            .processor
            .process(&cmd, &handle.ctx)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(json!({ "updated": columns.len() }))
}

#[derive(serde::Deserialize)]
pub struct ColumnOrder {
    pub id: String,
    pub order: usize,
}

/// Get the MRU list of recently opened boards.
#[tauri::command]
pub async fn get_recent_boards(
    state: State<'_, AppState>,
) -> Result<Value, String> {
    let config = state.config.read().await;
    serde_json::to_value(&config.recent_boards).map_err(|e| e.to_string())
}
