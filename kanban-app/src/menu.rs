//! Native menu bar construction and event handling.

use crate::state::{resolve_kanban_path, AppState, MenuItemHandle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use swissarmyhammer_commands::{CommandDef, CommandsRegistry, RecentBoard, UIState};
use swissarmyhammer_kanban::scope_commands::WindowInfo;
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
    windows: &[WindowInfo],
) -> tauri::Result<HashMap<String, MenuItemHandle>> {
    let mut menu_items: HashMap<String, MenuItemHandle> = HashMap::new();
    let menus = collect_menu_entries(registry, ui_state);

    let app_menu = build_app_submenu(app, &menus, &mut menu_items)?;
    let file_menu = build_file_submenu(app, &menus, recent, &mut menu_items)?;
    let edit_menu = build_grouped_submenu(app, "Edit", menus.get("Edit"), &mut menu_items)?;
    let window_menu = build_window_submenu(app, &menus, windows, &mut menu_items)?;

    let menu = Menu::with_items(app, &[&app_menu, &file_menu, &edit_menu, &window_menu])?;
    app.set_menu(menu).map_err(|e| {
        tracing::error!("Failed to set menu: {}", e);
        e
    })?;

    Ok(menu_items)
}

/// Collect commands with menu metadata into groups keyed by path (e.g. "App", "App/Settings").
fn collect_menu_entries(
    registry: &CommandsRegistry,
    ui_state: &UIState,
) -> HashMap<String, Vec<MenuEntry>> {
    let keymap_mode = ui_state.keymap_mode();
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
            name: cmd.menu_name.clone().unwrap_or_else(|| cmd.name.clone()),
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
    menus
}

/// Build the App menu with About, registry entries, Settings submenu, and OS chrome.
fn build_app_submenu(
    app: &AppHandle,
    menus: &HashMap<String, Vec<MenuEntry>>,
    menu_items: &mut HashMap<String, MenuItemHandle>,
) -> tauri::Result<Submenu<tauri::Wry>> {
    let app_menu = Submenu::new(app, app.package_info().name.clone(), true)?;
    app_menu.append(&PredefinedMenuItem::about(app, None, None)?)?;

    if let Some(items) = menus.get("App") {
        append_grouped_entries(app, &app_menu, items, menu_items, Some("app.about"))?;
    }

    // Settings submenu (nested path "App/Settings")
    if let Some(items) = menus.get("App/Settings") {
        app_menu.append(&PredefinedMenuItem::separator(app)?)?;
        let settings_sub = Submenu::new(app, "Settings", true)?;
        append_grouped_entries(app, &settings_sub, items, menu_items, None)?;
        app_menu.append(&settings_sub)?;
    }

    app_menu.append(&PredefinedMenuItem::separator(app)?)?;
    app_menu.append(&PredefinedMenuItem::hide(app, None)?)?;
    app_menu.append(&PredefinedMenuItem::hide_others(app, None)?)?;
    app_menu.append(&PredefinedMenuItem::show_all(app, None)?)?;
    app_menu.append(&PredefinedMenuItem::separator(app)?)?;
    app_menu.append(&PredefinedMenuItem::quit(app, None)?)?;
    Ok(app_menu)
}

/// Build the File menu with registry entries, Open Recent, and Close Window.
fn build_file_submenu(
    app: &AppHandle,
    menus: &HashMap<String, Vec<MenuEntry>>,
    recent: &[RecentBoard],
    menu_items: &mut HashMap<String, MenuItemHandle>,
) -> tauri::Result<Submenu<tauri::Wry>> {
    let file_menu = build_grouped_submenu(app, "File", menus.get("File"), menu_items)?;

    let recent_submenu = Submenu::new(app, "Open Recent", !recent.is_empty())?;
    for rb in recent {
        let id = format!("open_recent:{}", rb.path);
        let item = MenuItem::with_id(app, id, &rb.name, true, None::<&str>)?;
        recent_submenu.append(&item)?;
    }
    file_menu.append(&recent_submenu)?;
    file_menu.append(&PredefinedMenuItem::separator(app)?)?;
    file_menu.append(&PredefinedMenuItem::close_window(app, None)?)?;
    Ok(file_menu)
}

/// Build the Window menu with registry entries, Minimize/Maximize, and live window list.
fn build_window_submenu(
    app: &AppHandle,
    menus: &HashMap<String, Vec<MenuEntry>>,
    windows: &[WindowInfo],
    menu_items: &mut HashMap<String, MenuItemHandle>,
) -> tauri::Result<Submenu<tauri::Wry>> {
    let window_menu = Submenu::new(app, "Window", true)?;
    if let Some(items) = menus.get("Window") {
        append_grouped_entries(app, &window_menu, items, menu_items, None)?;
        if !items.is_empty() {
            window_menu.append(&PredefinedMenuItem::separator(app)?)?;
        }
    }
    window_menu.append(&PredefinedMenuItem::separator(app)?)?;
    window_menu.append(&PredefinedMenuItem::minimize(app, None)?)?;
    window_menu.append(&PredefinedMenuItem::maximize(app, None)?)?;

    if !windows.is_empty() {
        window_menu.append(&PredefinedMenuItem::separator(app)?)?;
        for win in windows {
            let id = format!("window.focus:{}", win.label);
            let item =
                CheckMenuItem::with_id(app, id, &win.title, true, win.focused, None::<&str>)?;
            window_menu.append(&item)?;
        }
    }
    Ok(window_menu)
}

/// Build a simple submenu from a single group of registry entries.
fn build_grouped_submenu(
    app: &AppHandle,
    label: &str,
    entries: Option<&Vec<MenuEntry>>,
    menu_items: &mut HashMap<String, MenuItemHandle>,
) -> tauri::Result<Submenu<tauri::Wry>> {
    let submenu = Submenu::new(app, label, true)?;
    if let Some(items) = entries {
        append_grouped_entries(app, &submenu, items, menu_items, None)?;
    }
    Ok(submenu)
}

/// Append entries to a submenu, inserting separators between groups.
///
/// If `skip_id` is provided, that entry is skipped (e.g. "app.about" which
/// is already added as a PredefinedMenuItem).
fn append_grouped_entries(
    app: &AppHandle,
    submenu: &Submenu<tauri::Wry>,
    entries: &[MenuEntry],
    menu_items: &mut HashMap<String, MenuItemHandle>,
    skip_id: Option<&str>,
) -> tauri::Result<()> {
    let mut last_group: Option<usize> = None;
    for entry in entries {
        if last_group.is_some() && last_group != Some(entry.group) {
            submenu.append(&PredefinedMenuItem::separator(app)?)?;
        }
        if skip_id == Some(entry.id.as_str()) {
            last_group = Some(entry.group);
            continue;
        }
        append_menu_entry(app, submenu, entry, menu_items)?;
        last_group = Some(entry.group);
    }
    Ok(())
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
    let resolved_map = match resolve_command_availability(state) {
        Some(map) => map,
        None => return, // locks were busy — skip this update
    };
    apply_menu_item_state(state, &resolved_map);
}

/// Resolve which commands are available in the current scope.
///
/// Returns a lookup of command ID → (display name, enabled) using
/// `commands_for_scope` as the single source of truth. Uses `try_read`
/// on both `boards` and `commands_registry` to avoid deadlock when called
/// from nested dispatch (e.g. open_and_notify → file.switchBoard).
/// Returns `None` if either lock is busy.
fn resolve_command_availability(state: &AppState) -> Option<HashMap<String, (String, bool)>> {
    let scope = state.ui_state.scope_chain();
    let boards = state.boards.try_read().ok().or_else(|| {
        tracing::debug!("update_menu_enabled_state: skipping — boards lock busy");
        None
    })?;
    let fields = boards.values().next().and_then(|h| h.ctx.fields());
    let registry = state.commands_registry.try_read().ok().or_else(|| {
        tracing::debug!("update_menu_enabled_state: skipping — registry lock busy");
        None
    })?;
    let resolved = swissarmyhammer_kanban::scope_commands::commands_for_scope(
        &scope,
        &registry,
        &state.command_impls,
        fields,
        &state.ui_state,
        false,
        None,
    );
    drop(registry);
    drop(boards);
    Some(
        resolved
            .into_iter()
            .map(|c| (c.id.clone(), (c.name, c.available)))
            .collect(),
    )
}

/// Apply resolved availability to native menu items.
///
/// For commands in the resolved map, sets the display name and enabled state.
/// For commands not in the map (unavailable in this scope), disables the item
/// and strips template placeholders from the generic name. Falls back to
/// applying only the resolved entries if the registry lock is busy.
fn apply_menu_item_state(state: &AppState, resolved_map: &HashMap<String, (String, bool)>) {
    let menu_items = state.menu_items.lock().unwrap();

    // Try to get the registry for fallback names on unavailable commands.
    let registry = state.commands_registry.try_read().ok();
    if registry.is_none() {
        tracing::debug!("update_menu_enabled_state: skipping fallback — registry lock busy");
    }

    for (cmd_id, menu_item) in menu_items.iter() {
        if let Some((name, enabled)) = resolved_map.get(cmd_id) {
            let _ = menu_item.set_enabled(*enabled);
            let _ = menu_item.set_text(name);
        } else {
            let _ = menu_item.set_enabled(false);
            if let Some(ref reg) = registry {
                if let Some(def) = reg.get(cmd_id) {
                    let clean = def.name.replace(" {{entity.type}}", "");
                    let _ = menu_item.set_text(&clean);
                }
            }
        }
    }
}

/// Dispatch native menu events.
///
/// Open Recent items are handled directly because they carry a file path.
/// Context menu items are emitted as `context-menu-command` events with the
/// full `ContextMenuItem` payload (cmd, target, scope_chain) so the frontend
/// routes them through `useDispatchCommand` — getting busy tracking and the
/// same dispatch path as keybindings/palette/drag.
/// Everything else is emitted as a `menu-command` event so the frontend
/// can route it through `executeCommand(id)`.
pub fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    let id = event.id().as_ref().to_string();

    // Open Recent items — handled directly (carry a file path)
    if let Some(path_str) = id.strip_prefix("open_recent:") {
        handle_open_recent(app, PathBuf::from(path_str));
        return;
    }

    // Window list items — focus the named window; menu rebuild happens
    // when the frontend re-dispatches ui.setFocus on window focus.
    if let Some(label) = id.strip_prefix("window.focus:") {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
        return;
    }

    // Context menu items carry self-contained dispatch info as JSON in the ID.
    // If it parses, emit a "context-menu-command" event so the frontend routes
    // it through useDispatchCommand — getting busy tracking, client-side
    // resolution, and the same dispatch path as keybindings/palette/drag.
    if let Ok(item) = serde_json::from_str::<crate::commands::ContextMenuItem>(&id) {
        if !item.cmd.is_empty() {
            let _ = app.emit("context-menu-command", &item);
            return;
        }
    }

    // Everything else: emit as a generic menu-command event.
    // The frontend listens for this and routes through executeCommand(id).
    let _ = app.emit("menu-command", &id);
}

/// Rebuild the native menu bar from the command registry (sync version).
///
/// Uses `blocking_read` — only safe from synchronous contexts (e.g. window
/// event handlers). For async contexts use `rebuild_menu_async`.
pub fn rebuild_menu(app: &AppHandle) {
    let state = app.state::<AppState>();
    let recent = state.ui_state.recent_boards();
    let registry = state.commands_registry.blocking_read();
    rebuild_menu_inner(app, &state, &registry, &recent);
}

/// Rebuild the native menu bar from the command registry (async version).
///
/// Uses `.read().await` on the tokio `RwLock`, safe from async contexts
/// like `dispatch_command_internal`.
pub async fn rebuild_menu_async(app: &AppHandle) {
    let state = app.state::<AppState>();
    let recent = state.ui_state.recent_boards();
    let registry = state.commands_registry.read().await;
    rebuild_menu_inner(app, &state, &registry, &recent);
}

/// Shared implementation for menu rebuilding.
fn rebuild_menu_inner(
    app: &AppHandle,
    state: &AppState,
    registry: &CommandsRegistry,
    recent: &[RecentBoard],
) {
    // Build WindowInfo list from live Tauri windows.
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
    match build_menu_from_commands(app, registry, &state.ui_state, recent, &windows) {
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

    // Build a synthetic scope chain with the window moniker so the backend
    // can derive window identity from the scope chain alone.
    let synthetic_scope = vec![format!("window:{}", window_label)];

    match crate::commands::dispatch_command_internal(
        handle,
        &state,
        "file.switchBoard",
        Some(synthetic_scope),
        None,
        Some(json!({ "path": path_str, "windowLabel": window_label })),
        None,
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

            // Emit board-opened so the originating window switches its active board.
            // Use emit_to when we know which window triggered the open; fall back
            // to a global emit so the event always reaches at least one listener
            // (focused_window_label can return None when OS focus shifts during
            // native dialogs or menu interactions).
            let payload = json!({ "path": canonical.display().to_string() });
            if let Some(label) = source_window_label {
                let _ = handle.emit_to(label, "board-opened", &payload);
            } else {
                let _ = handle.emit("board-opened", &payload);
            }
        }
        Err(e) => {
            tracing::error!("Failed to open board at {}: {}", path.display(), e);
        }
    }
}
