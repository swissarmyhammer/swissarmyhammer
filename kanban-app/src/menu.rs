//! Native menu bar construction and event handling.

use crate::state::{resolve_kanban_path, AppState, MenuItemHandle};
use std::sync::Arc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use swissarmyhammer_commands::{CommandDef, CommandsRegistry, RecentBoard, UIState};
use swissarmyhammer_kanban::{
    board::InitBoard, KanbanContext, KanbanOperationProcessor, OperationProcessor,
};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

/// A collected menu entry derived from a `CommandDef` with menu placement.
struct MenuEntry {
    id: String,
    name: String,
    group: usize,
    order: usize,
    accelerator: Option<String>,
    radio_group: Option<String>,
    checked: bool,
}

/// Build and set the native menu bar from the command registry.
///
/// Reads all `CommandDef` entries with `menu` metadata, groups them by
/// the first path element (top-level menu), sorts by `(group, order)`,
/// and builds native menu items. OS chrome items (About, Quit, Hide,
/// Open Recent, Edit shortcuts, Window list) are injected in their
/// standard positions.
///
/// Returns a `HashMap` of all created menu item handles keyed by command ID,
/// which can be stored in `AppState` for later enable/disable operations.
pub fn build_menu_from_commands(
    app: &AppHandle,
    registry: &CommandsRegistry,
    ui_state: &UIState,
    recent: &[RecentBoard],
) -> tauri::Result<HashMap<String, MenuItemHandle>> {
    let keymap_mode = ui_state.keymap_mode();
    let mut menu_items: HashMap<String, MenuItemHandle> = HashMap::new();

    // Collect commands with menu metadata, grouped by top-level menu name.
    // Nested paths (e.g. ["App", "Settings"]) are keyed as "App/Settings".
    let mut menus: HashMap<String, Vec<MenuEntry>> = HashMap::new();
    for cmd in registry.all_commands() {
        let Some(ref placement) = cmd.menu else {
            continue;
        };
        if placement.path.is_empty() {
            continue;
        }
        let key = placement.path.join("/");
        let accelerator = resolve_accelerator(cmd, &keymap_mode);
        let checked = resolve_checked(cmd, ui_state);
        menus.entry(key).or_default().push(MenuEntry {
            id: cmd.id.clone(),
            name: cmd.name.clone(),
            group: placement.group,
            order: placement.order,
            accelerator,
            radio_group: placement.radio_group.clone(),
            checked,
        });
    }
    for items in menus.values_mut() {
        items.sort_by_key(|e| (e.group, e.order));
    }

    // --- App menu ---
    let app_menu = Submenu::new(app, app.package_info().name.clone(), true)?;
    app_menu.append(&PredefinedMenuItem::about(app, None, None)?)?;

    if let Some(items) = menus.get("App") {
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
            append_menu_entry(app, &app_menu, entry, &mut menu_items)?;
            last_group = Some(entry.group);
        }
    }

    // Settings submenu inside the app menu (nested path "App/Settings")
    if let Some(items) = menus.get("App/Settings") {
        app_menu.append(&PredefinedMenuItem::separator(app)?)?;
        let settings_sub = Submenu::new(app, "Settings", true)?;
        let mut last_group: Option<usize> = None;
        for entry in items {
            if last_group.is_some() && last_group != Some(entry.group) {
                settings_sub.append(&PredefinedMenuItem::separator(app)?)?;
            }
            append_menu_entry(app, &settings_sub, entry, &mut menu_items)?;
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
    if let Some(items) = menus.get("File") {
        let mut last_group: Option<usize> = None;
        for entry in items {
            if last_group.is_some() && last_group != Some(entry.group) {
                file_menu.append(&PredefinedMenuItem::separator(app)?)?;
            }
            append_menu_entry(app, &file_menu, entry, &mut menu_items)?;
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

    // --- Edit menu (built from registry like all other menus) ---
    let edit_menu = Submenu::new(app, "Edit", true)?;
    if let Some(items) = menus.get("Edit") {
        let mut last_group: Option<usize> = None;
        for entry in items {
            if last_group.is_some() && last_group != Some(entry.group) {
                edit_menu.append(&PredefinedMenuItem::separator(app)?)?;
            }
            append_menu_entry(app, &edit_menu, entry, &mut menu_items)?;
            last_group = Some(entry.group);
        }
    }

    // --- Window menu ---
    let window_menu = Submenu::new(app, "Window", true)?;
    if let Some(items) = menus.get("Window") {
        let mut last_group: Option<usize> = None;
        let mut count = 0usize;
        for entry in items {
            if last_group.is_some() && last_group != Some(entry.group) {
                window_menu.append(&PredefinedMenuItem::separator(app)?)?;
            }
            append_menu_entry(app, &window_menu, entry, &mut menu_items)?;
            last_group = Some(entry.group);
            count += 1;
        }
        if count > 0 {
            window_menu.append(&PredefinedMenuItem::separator(app)?)?;
        }
    }
    window_menu.append(&PredefinedMenuItem::minimize(app, None)?)?;
    window_menu.append(&PredefinedMenuItem::maximize(app, None)?)?;

    // Manually list open windows with a checkmark on the focused one.
    // NOTE: muda's set_as_windows_menu_for_nsapp() is broken — it uses
    // a disconnected NSMenu that macOS ignores (muda#322, still unmerged).
    let windows = app.webview_windows();
    let visible: Vec<_> = windows
        .iter()
        .filter(|(_, w)| {
            let title = w.title().unwrap_or_default();
            !title.is_empty() && w.is_visible().unwrap_or(false)
        })
        .collect();
    if !visible.is_empty() {
        window_menu.append(&PredefinedMenuItem::separator(app)?)?;
        for (label, window) in &visible {
            let title = window.title().unwrap_or_else(|_| (*label).clone());
            let focused = window.is_focused().unwrap_or(false);
            let id = format!("window.focus:{}", label);
            let item = CheckMenuItem::with_id(app, id, &title, true, focused, None::<&str>)?;
            window_menu.append(&item)?;
        }
    }

    let menu = Menu::with_items(app, &[&app_menu, &file_menu, &edit_menu, &window_menu])?;
    app.set_menu(menu).map_err(|e| {
        tracing::error!("Failed to set menu: {}", e);
        e
    })?;

    Ok(menu_items)
}

/// Resolve the keyboard accelerator for a command in the current keymap mode.
///
/// Looks up the binding for the active mode, falling back to CUA if the
/// mode-specific binding is absent. Replaces `Mod` with `CmdOrCtrl` so
/// Tauri maps it correctly per platform.
fn resolve_accelerator(cmd: &CommandDef, keymap_mode: &str) -> Option<String> {
    let keys = cmd.keys.as_ref()?;
    let binding = match keymap_mode {
        "vim" => keys.vim.as_deref().or(keys.cua.as_deref()),
        "emacs" => keys.emacs.as_deref().or(keys.cua.as_deref()),
        _ => keys.cua.as_deref(),
    }?;
    // Vim chord-style bindings (e.g. ":q", "dd") are not valid accelerators.
    if binding.len() > 1 && !binding.contains('+') {
        return None;
    }
    Some(binding.replace("Mod", "CmdOrCtrl"))
}

/// Resolve the checked state for radio group items.
///
/// Currently supports the "keymap" radio group — the item matching the
/// active keymap mode is checked.
fn resolve_checked(cmd: &CommandDef, ui_state: &UIState) -> bool {
    let Some(ref placement) = cmd.menu else {
        return false;
    };
    match placement.radio_group.as_deref() {
        Some("keymap") => {
            let mode = ui_state.keymap_mode();
            match cmd.id.as_str() {
                "settings.keymap.vim" => mode == "vim",
                "settings.keymap.cua" => mode == "cua",
                "settings.keymap.emacs" => mode == "emacs",
                _ => false,
            }
        }
        _ => false,
    }
}

/// Append a single menu entry to a submenu, creating either a CheckMenuItem
/// (for radio group items) or a regular MenuItem.
fn append_menu_entry(
    app: &AppHandle,
    submenu: &Submenu<tauri::Wry>,
    entry: &MenuEntry,
    menu_items: &mut HashMap<String, MenuItemHandle>,
) -> tauri::Result<()> {
    if entry.radio_group.is_some() {
        let item = CheckMenuItem::with_id(
            app,
            &entry.id,
            &entry.name,
            true,
            entry.checked,
            entry.accelerator.as_deref(),
        )?;
        menu_items.insert(entry.id.clone(), MenuItemHandle::Check(item.clone()));
        submenu.append(&item)?;
    } else {
        let item = MenuItem::with_id(
            app,
            &entry.id,
            &entry.name,
            true,
            entry.accelerator.as_deref(),
        )?;
        menu_items.insert(entry.id.clone(), MenuItemHandle::Regular(item.clone()));
        submenu.append(&item)?;
    }
    Ok(())
}

/// Update the enabled state of all menu items based on command availability.
///
/// Builds a CommandContext from the current scope chain and UIState, then
/// checks each cached menu item's command `available()` to set enabled/disabled.
/// Called after every command dispatch.
pub fn update_menu_enabled_state(state: &AppState) {
    let scope = state.ui_state.scope_chain();
    let empty_args: HashMap<String, serde_json::Value> = HashMap::new();
    let ctx = swissarmyhammer_commands::CommandContext::new("_menu_check", scope, None, empty_args)
        .with_ui_state(Arc::clone(&state.ui_state));

    let menu_items = state.menu_items.lock().unwrap();
    for (cmd_id, menu_item) in menu_items.iter() {
        if let Some(cmd_impl) = state.command_impls.get(cmd_id) {
            let enabled = cmd_impl.available(&ctx);
            let _ = menu_item.set_enabled(enabled);
        }
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

    // Window list items — focus the named window and update check states
    if let Some(label) = id.strip_prefix("window.focus:") {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
        // Update check marks: only the clicked window should be checked
        if let Some(menu) = app.menu() {
            if let Ok(items) = menu.items() {
                for item in items {
                    if let Some(sub) = item.as_submenu() {
                        if let Ok(sub_items) = sub.items() {
                            for si in sub_items {
                                if let Some(check) = si.as_check_menuitem() {
                                    let cid = check.id().as_ref().to_string();
                                    if let Some(wlabel) = cid.strip_prefix("window.focus:") {
                                        let _ = check.set_checked(wlabel == label);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
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

/// Rebuild the native menu bar from the command registry.
///
/// Called after keymap mode changes or board switches to update accelerators
/// and checked states. Acquires the commands registry read lock synchronously
/// via `blocking_read` — safe in async contexts as the lock is never held
/// across an await point.
pub fn rebuild_menu(app: &AppHandle) {
    let state = app.state::<AppState>();
    let recent = state.ui_state.recent_boards();
    let registry = state.commands_registry.blocking_read();
    match build_menu_from_commands(app, &registry, &state.ui_state, &recent) {
        Ok(items) => {
            *state.menu_items.lock().unwrap() = items;
        }
        Err(e) => {
            tracing::error!("Failed to rebuild menu: {}", e);
        }
    }
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
/// Routes board opening through `dispatch_command_internal` so that all
/// UIState tracking and BoardHandle lifecycle are handled consistently.
///
/// Emits `board-opened` to the source window (the one that initiated
/// the open) so only that window switches. The `board-changed` broadcast
/// is handled by `dispatch_command_internal` via the `BoardSwitch` side effect.
async fn open_and_notify(handle: &AppHandle, path: &Path, source_window_label: Option<&str>) {
    use serde_json::json;
    use tauri::Emitter;

    let state = handle.state::<AppState>();
    let path_str = path.display().to_string();
    let window_label = source_window_label.unwrap_or("main").to_string();

    match crate::commands::dispatch_command_internal(
        handle,
        &state,
        "file.switchBoard",
        None,
        None,
        Some(json!({ "path": path_str, "windowLabel": window_label })),
        None,
        Some(window_label.clone()),
    )
    .await
    {
        Ok(_) => {
            // Resolve the canonical path so the frontend gets the same path
            // that the board was registered under in UIState.
            let canonical = resolve_kanban_path(path)
                .ok()
                .and_then(|p| p.canonicalize().ok().or(Some(p)))
                .unwrap_or_else(|| path.to_path_buf());

            // Emit board-opened to the source window only so it switches its active board.
            // dispatch_command_internal already emits board-changed to all windows.
            let payload = json!({ "path": canonical.display().to_string() });
            if let Some(label) = source_window_label {
                let _ = handle.emit_to(label, "board-opened", &payload);
            }
        }
        Err(e) => {
            tracing::error!("Failed to open board at {}: {}", path.display(), e);
        }
    }
}
