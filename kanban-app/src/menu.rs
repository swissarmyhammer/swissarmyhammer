//! Native menu bar construction and event handling.

use crate::commands::MenuItemEntry;
use crate::state::{resolve_kanban_path, AppState};
use std::path::{Path, PathBuf};
use swissarmyhammer_commands::RecentBoard;
use swissarmyhammer_kanban::{
    board::InitBoard, KanbanContext, KanbanOperationProcessor, OperationProcessor,
};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

/// Build and set the native menu from a frontend-generated manifest.
///
/// Groups entries by menu name (app, file, settings), builds each submenu
/// with separators between groups, and injects OS chrome items (About, Quit,
/// Hide, Close Window, Open Recent, Edit menu). The manifest entries are
/// Sorts entries by (group, order) within each menu submenu.
pub fn build_menu_from_manifest(
    app: &AppHandle,
    manifest: &[MenuItemEntry],
    recent: &[RecentBoard],
) -> tauri::Result<()> {
    // Group entries by menu name, then sort by (group, order) within each menu
    let mut menus: std::collections::HashMap<String, Vec<&MenuItemEntry>> =
        std::collections::HashMap::new();
    for entry in manifest {
        menus.entry(entry.menu.clone()).or_default().push(entry);
    }
    for items in menus.values_mut() {
        items.sort_by_key(|e| (e.group, e.order));
    }

    // --- App menu ---
    let app_menu = Submenu::new(app, app.package_info().name.clone(), true)?;
    app_menu.append(&PredefinedMenuItem::about(app, None, None)?)?;

    if let Some(items) = menus.get("app") {
        let mut last_group: Option<usize> = None;
        for entry in items {
            if last_group.is_some() && last_group != Some(entry.group) {
                app_menu.append(&PredefinedMenuItem::separator(app)?)?;
            }
            // Skip "app.about" — already added as PredefinedMenuItem above
            if entry.id == "app.about" {
                last_group = Some(entry.group);
                continue;
            }
            app_menu.append(build_menu_item(app, entry)?.as_ref())?;
            last_group = Some(entry.group);
        }
    }

    // Settings submenu inside the app menu (matches original structure)
    if let Some(items) = menus.get("settings") {
        app_menu.append(&PredefinedMenuItem::separator(app)?)?;
        let settings_sub = Submenu::new(app, "Settings", true)?;
        let mut last_group: Option<usize> = None;
        for entry in items {
            if last_group.is_some() && last_group != Some(entry.group) {
                settings_sub.append(&PredefinedMenuItem::separator(app)?)?;
            }
            settings_sub.append(build_menu_item(app, entry)?.as_ref())?;
            last_group = Some(entry.group);
        }
        app_menu.append(&settings_sub)?;
    }

    // OS chrome at the end of app menu
    app_menu.append(&PredefinedMenuItem::separator(app)?)?;
    app_menu.append(&PredefinedMenuItem::hide(app, None)?)?;
    app_menu.append(&PredefinedMenuItem::hide_others(app, None)?)?;
    app_menu.append(&PredefinedMenuItem::show_all(app, None)?)?;
    app_menu.append(&PredefinedMenuItem::separator(app)?)?;
    app_menu.append(&PredefinedMenuItem::quit(app, None)?)?;

    // --- File menu ---
    let file_menu = Submenu::new(app, "File", true)?;
    if let Some(items) = menus.get("file") {
        let mut last_group: Option<usize> = None;
        for entry in items {
            if last_group.is_some() && last_group != Some(entry.group) {
                file_menu.append(&PredefinedMenuItem::separator(app)?)?;
            }
            file_menu.append(build_menu_item(app, entry)?.as_ref())?;
            last_group = Some(entry.group);
        }
    }
    // Open Recent submenu
    let recent_submenu = Submenu::new(app, "Open Recent", !recent.is_empty())?;
    for rb in recent {
        let id = format!("open_recent:{}", rb.path);
        let item = MenuItem::with_id(app, id, &rb.name, true, None::<&str>)?;
        recent_submenu.append(&item)?;
    }
    file_menu.append(&recent_submenu)?;
    file_menu.append(&PredefinedMenuItem::separator(app)?)?;
    file_menu.append(&PredefinedMenuItem::close_window(app, None)?)?;

    // --- Edit menu (OS chrome + manifest items) ---
    let edit_menu = Submenu::with_items(
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
    if let Some(items) = menus.get("edit") {
        edit_menu.append(&PredefinedMenuItem::separator(app)?)?;
        let mut last_group: Option<usize> = None;
        for entry in items {
            if last_group.is_some() && last_group != Some(entry.group) {
                edit_menu.append(&PredefinedMenuItem::separator(app)?)?;
            }
            edit_menu.append(build_menu_item(app, entry)?.as_ref())?;
            last_group = Some(entry.group);
        }
    }

    // --- Window menu ---
    let window_menu = Submenu::new(app, "Window", true)?;
    if let Some(items) = menus.get("window") {
        let mut last_group: Option<usize> = None;
        let mut count = 0usize;
        for entry in items {
            if last_group.is_some() && last_group != Some(entry.group) {
                window_menu.append(&PredefinedMenuItem::separator(app)?)?;
            }
            window_menu.append(build_menu_item(app, entry)?.as_ref())?;
            last_group = Some(entry.group);
            count += 1;
        }
        if count > 0 {
            window_menu.append(&PredefinedMenuItem::separator(app)?)?;
        }
    }
    window_menu.append(&PredefinedMenuItem::minimize(app, None)?)?;
    window_menu.append(&PredefinedMenuItem::maximize(app, None)?)?;

    let menu = Menu::with_items(app, &[&app_menu, &file_menu, &edit_menu, &window_menu])?;
    app.set_menu(menu).map_err(|e| {
        tracing::error!("Failed to set menu: {}", e);
        e
    })?;

    Ok(())
}

/// Build a single native menu item from a manifest entry.
///
/// If the entry has a `radio_group`, a CheckMenuItem is created (for radio-style
/// toggle items). Otherwise a regular MenuItem is created. Both support
/// optional keyboard accelerators.
fn build_menu_item(
    app: &AppHandle,
    entry: &MenuItemEntry,
) -> tauri::Result<Box<dyn tauri::menu::IsMenuItem<tauri::Wry>>> {
    if entry.radio_group.is_some() {
        Ok(Box::new(CheckMenuItem::with_id(
            app,
            &entry.id,
            &entry.name,
            true,
            entry.checked.unwrap_or(false),
            entry.accelerator.as_deref(),
        )?))
    } else {
        Ok(Box::new(MenuItem::with_id(
            app,
            &entry.id,
            &entry.name,
            true,
            entry.accelerator.as_deref(),
        )?))
    }
}

/// Dispatch native menu events.
///
/// Open Recent items are handled directly because they carry a file path.
/// Generic context menu items are emitted as `context-menu-command` events.
/// Everything else is emitted as a `menu-command` event so the frontend
/// can route it through `executeCommand(id)`.
pub fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    let id = event.id().as_ref().to_string();

    // Open Recent items — handled directly (carry a file path)
    if let Some(path_str) = id.strip_prefix("open_recent:") {
        handle_open_recent(app, PathBuf::from(path_str));
        return;
    }

    // Generic context menu items — emit as context-menu-command so the
    // frontend can distinguish them from menu bar commands.
    // UIState uses std::sync::RwLock (not tokio), so read access works in
    // sync contexts like this menu event handler.
    {
        let state = app.state::<AppState>();
        if state.ui_state.is_context_menu_id(&id) {
            let _ = app.emit("context-menu-command", &id);
            return;
        }
    }

    // Everything else: emit as a generic menu-command event.
    // The frontend listens for this and routes through executeCommand(id).
    let _ = app.emit("menu-command", &id);
}

/// Public entry point for creating a new board -- used by both native menu and command palette.
pub fn trigger_new_board(app: &AppHandle) {
    handle_new_board(app);
}

/// Public entry point for opening a board -- used by both native menu and command palette.
pub fn trigger_open_board(app: &AppHandle) {
    handle_open_board(app);
}

/// File > New Board: pick a folder, init the board, then open it.
fn handle_new_board(app: &AppHandle) {
    let handle = app.clone();
    let source_window = focused_window_label(app);
    app.dialog().file().pick_folder(move |folder| {
        if let Some(folder_path) = folder {
            let Ok(path) = folder_path.into_path() else {
                return;
            };
            let sw = source_window.clone();
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

                open_and_notify(&handle, &path, sw.as_deref()).await;
            });
        }
    });
}

/// File > Open Board...: pick a folder and open an existing board.
fn handle_open_board(app: &AppHandle) {
    let handle = app.clone();
    // Capture the focused window BEFORE the dialog steals focus
    let source_window = focused_window_label(app);
    app.dialog().file().pick_folder(move |folder| {
        if let Some(folder_path) = folder {
            let Ok(path) = folder_path.into_path() else {
                return;
            };
            let sw = source_window.clone();
            tauri::async_runtime::spawn(async move {
                open_and_notify(&handle, &path, sw.as_deref()).await;
            });
        }
    });
}

/// File > Open Recent > <board>: open a board from the MRU list.
fn handle_open_recent(app: &AppHandle, path: PathBuf) {
    let handle = app.clone();
    let source_window = focused_window_label(app);
    tauri::async_runtime::spawn(async move {
        open_and_notify(&handle, &path, source_window.as_deref()).await;
    });
}

/// Get the label of the currently focused window, if any.
fn focused_window_label(app: &AppHandle) -> Option<String> {
    app.webview_windows()
        .values()
        .find(|w| w.is_focused().unwrap_or(false))
        .map(|w| w.label().to_string())
}

/// Open a board and emit frontend events.
///
/// Emits `board-opened` to the source window (the one that initiated
/// the open) so only that window switches. Broadcasts `board-changed`
/// to all windows so they refresh their open boards list.
async fn open_and_notify(handle: &AppHandle, path: &Path, source_window_label: Option<&str>) {
    use serde_json::json;
    use tauri::Emitter;

    let state = handle.state::<AppState>();
    match state.open_board(path, Some(handle.clone())).await {
        Ok(canonical) => {
            let payload = json!({ "path": canonical.display().to_string() });

            // Emit board-opened to the source window only (emit_to scopes to one window)
            if let Some(label) = source_window_label {
                let _ = handle.emit_to(label, "board-opened", &payload);
            }

            let _ = handle.emit("board-changed", ());
        }
        Err(e) => {
            tracing::error!("Failed to open board at {}: {}", path.display(), e);
        }
    }
}
