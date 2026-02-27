//! Tauri commands for board operations.

use crate::state::AppState;
use serde_json::{json, Value};
use std::path::PathBuf;
use swissarmyhammer_kanban::{board::GetBoard, task::ListTasks, OperationProcessor};
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

/// Get the MRU list of recently opened boards.
#[tauri::command]
pub async fn get_recent_boards(
    state: State<'_, AppState>,
) -> Result<Value, String> {
    let config = state.config.read().await;
    serde_json::to_value(&config.recent_boards).map_err(|e| e.to_string())
}
