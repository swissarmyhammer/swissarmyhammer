//! Native menu bar construction and event handling.

use crate::state::{resolve_kanban_path, AppConfig, AppState, RecentBoard};
use std::path::PathBuf;
use swissarmyhammer_kanban::{board::InitBoard, KanbanContext, KanbanOperationProcessor, OperationProcessor};
use tauri::menu::{CheckMenuItem, IconMenuItem, Menu, MenuItem, NativeIcon, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

/// Build the native menu bar with app, File, Edit, and Settings submenus.
///
/// On macOS the first submenu is always the application menu, so we
/// prepend a proper app menu to keep File in the right position.
pub fn build_menu(app: &AppHandle, recent: &[RecentBoard], keymap_mode: &str) -> tauri::Result<Menu<tauri::Wry>> {
    // File menu items — standard macOS labels with native icons
    let new_board = IconMenuItem::with_id_and_native_icon(
        app, "new_board", "New", true, Some(NativeIcon::Add), Some("CmdOrCtrl+N"),
    )?;
    let open_board = IconMenuItem::with_id_and_native_icon(
        app, "open_board", "Open...", true, Some(NativeIcon::Folder), Some("CmdOrCtrl+O"),
    )?;

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

    // Settings menu with Editor Keymap radio items
    let keymap_cua = CheckMenuItem::with_id(
        app, "keymap_cua", "CUA (Standard)", true, keymap_mode == "cua", None::<&str>,
    )?;
    let keymap_vim = CheckMenuItem::with_id(
        app, "keymap_vim", "Vim", true, keymap_mode == "vim", None::<&str>,
    )?;
    let keymap_emacs = CheckMenuItem::with_id(
        app, "keymap_emacs", "Emacs", true, keymap_mode == "emacs", None::<&str>,
    )?;

    let settings_submenu = Submenu::with_items(
        app,
        "Settings",
        true,
        &[
            &keymap_cua,
            &keymap_vim,
            &keymap_emacs,
            &PredefinedMenuItem::separator(app)?,
        ],
    )?;

    // macOS app menu — insert Settings before the separator/quit group
    let app_menu = Submenu::with_items(
        app,
        app.package_info().name.clone(),
        true,
        &[
            &PredefinedMenuItem::about(app, None, None)?,
            &PredefinedMenuItem::separator(app)?,
            &settings_submenu,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;

    Menu::with_items(app, &[&app_menu, &file_submenu, &edit_submenu])
}

/// Rebuild the menu from current config and set it on the app.
pub fn rebuild_menu(handle: &AppHandle) {
    let config = AppConfig::load();
    match build_menu(handle, &config.recent_boards, &config.keymap_mode) {
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
    } else if let Some(mode) = id.strip_prefix("keymap_") {
        handle_keymap_change(app, mode);
    } else if id == "tag_edit" || id == "tag_delete" {
        handle_tag_menu(app, &id);
    }
}

/// Tag context menu actions: read the stored context tag and emit to frontend.
fn handle_tag_menu(app: &AppHandle, action: &str) {
    let handle = app.clone();
    let action = action.to_string();
    tauri::async_runtime::spawn(async move {
        let state = handle.state::<AppState>();
        let context = state.context_tag.read().await.clone();
        if let Some((tag_id, task_id)) = context {
            let _ = handle.emit("tag-context-menu", serde_json::json!({
                "action": action,
                "tag_id": tag_id,
                "task_id": task_id,
            }));
        }
    });
}

/// Settings > Editor Keymap > [mode]: update config, rebuild menu, notify frontend.
fn handle_keymap_change(app: &AppHandle, mode: &str) {
    let handle = app.clone();
    let mode = mode.to_string();
    tauri::async_runtime::spawn(async move {
        let state = handle.state::<AppState>();
        {
            let mut config = state.config.write().await;
            config.keymap_mode = mode.clone();
            let _ = config.save();
        }
        rebuild_menu(&handle);
        let _ = handle.emit("keymap-changed", &mode);
    });
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
