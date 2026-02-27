//! Native menu bar construction and event handling.

use crate::state::{resolve_kanban_path, AppConfig, AppState, RecentBoard};
use std::path::PathBuf;
use swissarmyhammer_kanban::{board::InitBoard, KanbanContext, KanbanOperationProcessor, OperationProcessor};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

/// Build the native menu bar with app, File, and Edit submenus.
///
/// On macOS the first submenu is always the application menu, so we
/// prepend a proper app menu to keep File in the right position.
pub fn build_menu(app: &AppHandle, recent: &[RecentBoard]) -> tauri::Result<Menu<tauri::Wry>> {
    // macOS app menu (first submenu = app name menu)
    let app_menu = Submenu::with_items(
        app,
        app.package_info().name.clone(),
        true,
        &[
            &PredefinedMenuItem::about(app, None, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;

    // File menu items — standard macOS labels, routed to our command logic
    let new_board = MenuItem::with_id(app, "new_board", "New", true, Some("CmdOrCtrl+N"))?;
    let open_board =
        MenuItem::with_id(app, "open_board", "Open...", true, Some("CmdOrCtrl+O"))?;

    // Open Recent submenu (disabled when empty)
    let recent_submenu = Submenu::new(app, "Open Recent", !recent.is_empty())?;
    for rb in recent {
        let id = format!("open_recent:{}", rb.path.display());
        let item = MenuItem::with_id(app, id, &rb.name, true, None::<&str>)?;
        recent_submenu.append(&item)?;
    }

    let file_submenu = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &new_board,
            &open_board,
            &recent_submenu,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;

    // Edit submenu with standard items
    let edit_submenu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;

    Menu::with_items(app, &[&app_menu, &file_submenu, &edit_submenu])
}

/// Rebuild the menu from current config and set it on the app.
pub fn rebuild_menu(handle: &AppHandle) {
    let config = AppConfig::load();
    match build_menu(handle, &config.recent_boards) {
        Ok(menu) => {
            if let Err(e) = handle.set_menu(menu) {
                tracing::error!("Failed to set menu: {}", e);
            }
        }
        Err(e) => {
            tracing::error!("Failed to build menu: {}", e);
        }
    }
}

/// Dispatch native menu events.
pub fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    let id = event.id().as_ref().to_string();

    if id == "new_board" {
        handle_new_board(app);
    } else if id == "open_board" {
        handle_open_board(app);
    } else if let Some(path_str) = id.strip_prefix("open_recent:") {
        handle_open_recent(app, PathBuf::from(path_str));
    }
}

/// File > New Board: pick a folder, init the board, then open it.
fn handle_new_board(app: &AppHandle) {
    let handle = app.clone();
    app.dialog().file().pick_folder(move |folder| {
        if let Some(folder_path) = folder {
            let Ok(path) = folder_path.into_path() else {
                return;
            };
            tauri::async_runtime::spawn(async move {
                // Derive board name from folder
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("New Board")
                    .to_string();

                // Initialize if needed
                if let Ok(kanban_path) = resolve_kanban_path(&path) {
                    let ctx = KanbanContext::new(&kanban_path);
                    if !ctx.is_initialized() {
                        let processor = KanbanOperationProcessor::new();
                        let init = InitBoard::new(&name);
                        if let Err(e) = processor.process(&init, &ctx).await {
                            tracing::error!("Failed to init board: {}", e);
                            return;
                        }
                    }
                }

                open_and_notify(&handle, &path).await;
            });
        }
    });
}

/// File > Open Board...: pick a folder and open an existing board.
fn handle_open_board(app: &AppHandle) {
    let handle = app.clone();
    app.dialog().file().pick_folder(move |folder| {
        if let Some(folder_path) = folder {
            let Ok(path) = folder_path.into_path() else {
                return;
            };
            tauri::async_runtime::spawn(async move {
                open_and_notify(&handle, &path).await;
            });
        }
    });
}

/// File > Open Recent > <board>: open a board from the MRU list.
fn handle_open_recent(app: &AppHandle, path: PathBuf) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        open_and_notify(&handle, &path).await;
    });
}

/// Open a board, rebuild the menu, and emit a frontend event.
async fn open_and_notify(handle: &AppHandle, path: &PathBuf) {
    let state = handle.state::<AppState>();
    match state.open_board(path).await {
        Ok(_) => {
            rebuild_menu(handle);
            let _ = handle.emit("board-changed", ());
        }
        Err(e) => {
            tracing::error!("Failed to open board at {}: {}", path.display(), e);
        }
    }
}
