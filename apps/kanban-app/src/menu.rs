//! Native menu bar construction and event handling.

use crate::state::{resolve_kanban_path, AppState, MenuItemHandle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use swissarmyhammer_commands::{CommandDef, CommandsRegistry, RecentBoard, UIState, WindowInfo};
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
/// and builds native menu items. The menu bar carries six top-level
/// submenus in order: App, File, Edit, View, Navigation, Window. OS
/// chrome items (About, Quit, Hide, Open Recent, Edit shortcuts, Window
/// list) are injected in their standard positions.
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
    // View submenu hosts view-surface commands (currently the AI panel
    // toggle `ai.toggle`). Placed after Edit, before Navigation, in the
    // conventional menu-bar order (Edit → View → Navigate → Window).
    let view_menu = build_grouped_submenu(app, "View", menus.get("View"), &mut menu_items)?;
    // Navigation submenu hosts the nine `nav.*` commands contributed by
    // `swissarmyhammer-focus` (eight directional/drill plus
    // `nav.jump`). Placement between View and Window mirrors common
    // app conventions and keeps Window as the trailing menu before any
    // platform-specific Help.
    let nav_menu =
        build_grouped_submenu(app, "Navigation", menus.get("Navigation"), &mut menu_items)?;
    let window_menu = build_window_submenu(app, &menus, windows, &mut menu_items)?;

    let menu = Menu::with_items(
        app,
        &[
            &app_menu,
            &file_menu,
            &edit_menu,
            &view_menu,
            &nav_menu,
            &window_menu,
        ],
    )?;
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

/// Canonical Tauri-accelerator named keys.
///
/// Mirrors the named-key tokens recognised by muda's `parse_key`
/// (the accelerator parser used by Tauri 2 / muda 0.17 — see
/// `muda::accelerator::parse_key`). Stored as an upper-case allowlist
/// so the membership test is a single ASCII-uppercase pass.
///
/// Single-character keys (`a`–`z`, `0`–`9`, ASCII punctuation like
/// `,` `.` `-` `=` `;` `'` `[` `]` `\` `/` `` ` ``) are not in this
/// list — they pass the length-1 fast path in `is_valid_accelerator_key`
/// and never reach the named-key check. Modifier-chord strings (anything
/// containing `+`) likewise short-circuit before this list is consulted.
///
/// The list intentionally omits modifier names (`Shift`, `Ctrl`, `Alt`,
/// `Cmd`, `Super`, `CmdOrCtrl`) — those are only ever valid as the
/// modifier portion of a `+`-joined chord, never as the main key, and
/// the `+`-containing branch handles them.
const TAURI_NAMED_KEYS: &[&str] = &[
    // Editing / whitespace block
    "BACKSPACE",
    "TAB",
    "ENTER",
    "SPACE",
    "DELETE",
    "INSERT",
    // Navigation block
    "HOME",
    "END",
    "PAGEUP",
    "PAGEDOWN",
    "ARROWUP",
    "ARROWDOWN",
    "ARROWLEFT",
    "ARROWRIGHT",
    // Aliases that muda accepts for arrow keys
    "UP",
    "DOWN",
    "LEFT",
    "RIGHT",
    // Lock / system keys
    "CAPSLOCK",
    "NUMLOCK",
    "SCROLLLOCK",
    "PRINTSCREEN",
    // Escape and its alias
    "ESCAPE",
    "ESC",
    // Function keys F1–F24
    "F1",
    "F2",
    "F3",
    "F4",
    "F5",
    "F6",
    "F7",
    "F8",
    "F9",
    "F10",
    "F11",
    "F12",
    "F13",
    "F14",
    "F15",
    "F16",
    "F17",
    "F18",
    "F19",
    "F20",
    "F21",
    "F22",
    "F23",
    "F24",
    // Numpad block
    "NUMPAD0",
    "NUMPAD1",
    "NUMPAD2",
    "NUMPAD3",
    "NUMPAD4",
    "NUMPAD5",
    "NUMPAD6",
    "NUMPAD7",
    "NUMPAD8",
    "NUMPAD9",
    "NUM0",
    "NUM1",
    "NUM2",
    "NUM3",
    "NUM4",
    "NUM5",
    "NUM6",
    "NUM7",
    "NUM8",
    "NUM9",
    "NUMPADADD",
    "NUMADD",
    "NUMPADPLUS",
    "NUMPLUS",
    "NUMPADDECIMAL",
    "NUMDECIMAL",
    "NUMPADDIVIDE",
    "NUMDIVIDE",
    "NUMPADENTER",
    "NUMENTER",
    "NUMPADEQUAL",
    "NUMEQUAL",
    "NUMPADMULTIPLY",
    "NUMMULTIPLY",
    "NUMPADSUBTRACT",
    "NUMSUBTRACT",
    // Digit / letter long-form names (muda also accepts the bare
    // single-char form, but `KeyA`, `Digit5`, etc. are valid tokens too)
    "DIGIT0",
    "DIGIT1",
    "DIGIT2",
    "DIGIT3",
    "DIGIT4",
    "DIGIT5",
    "DIGIT6",
    "DIGIT7",
    "DIGIT8",
    "DIGIT9",
    "KEYA",
    "KEYB",
    "KEYC",
    "KEYD",
    "KEYE",
    "KEYF",
    "KEYG",
    "KEYH",
    "KEYI",
    "KEYJ",
    "KEYK",
    "KEYL",
    "KEYM",
    "KEYN",
    "KEYO",
    "KEYP",
    "KEYQ",
    "KEYR",
    "KEYS",
    "KEYT",
    "KEYU",
    "KEYV",
    "KEYW",
    "KEYX",
    "KEYY",
    "KEYZ",
    // Punctuation long-form names (single-char form passes the length-1
    // branch; the named form is included for symmetry with muda)
    "BACKQUOTE",
    "BACKSLASH",
    "BRACKETLEFT",
    "BRACKETRIGHT",
    "COMMA",
    "EQUAL",
    "MINUS",
    "PERIOD",
    "QUOTE",
    "SEMICOLON",
    "SLASH",
    // Audio volume keys
    "AUDIOVOLUMEDOWN",
    "VOLUMEDOWN",
    "AUDIOVOLUMEUP",
    "VOLUMEUP",
    "AUDIOVOLUMEMUTE",
    "VOLUMEMUTE",
];

/// Decide whether a YAML-supplied binding string is a valid Tauri
/// accelerator atom that can be displayed in a native menu.
///
/// Three accepted shapes, in priority order:
/// 1. **Modifier chord** — anything containing `+` (e.g. `Cmd+S`,
///    `Shift+G`, `Alt+ArrowDown`). The chord components are validated
///    by Tauri itself when the menu item is built; this filter only
///    decides whether to forward the string at all.
/// 2. **Single character** — exactly one character. Covers
///    `[A-Za-z0-9]` and ASCII punctuation atoms (`,`, `.`, `-`, `=`,
///    `;`, `'`, `[`, `]`, `\`, `/`, `` ` ``) which muda's `parse_key`
///    accepts directly.
/// 3. **Named key** — case-insensitively matches an entry in
///    [`TAURI_NAMED_KEYS`] (e.g. `Enter`, `Escape`, `ArrowUp`,
///    `Home`, `F5`, `KeyA`, `Digit5`).
///
/// Anything else — vim chord strings like `dd`, `gg`, `:q`, `yy`, or
/// empty / whitespace-only input — is rejected. This is the actual
/// filter target: vim's multi-character bindings are handled in the
/// frontend's `SEQUENCE_TABLES` and never become native accelerators.
fn is_valid_accelerator_key(binding: &str) -> bool {
    let trimmed = binding.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.contains('+') {
        // Modifier chord — Tauri's accelerator parser validates the
        // individual tokens when the menu item is constructed.
        return true;
    }
    // `chars().count() == 1` (not `len() == 1`) so multi-byte single
    // characters like `é` aren't truncated by byte-length comparison.
    if trimmed.chars().count() == 1 {
        return true;
    }
    let upper = trimmed.to_ascii_uppercase();
    TAURI_NAMED_KEYS.contains(&upper.as_str())
}

/// Resolve the keyboard accelerator for a command in the current keymap mode.
///
/// Looks up the binding for the active mode, falling back to CUA if the
/// mode-specific binding is absent. Replaces `Mod` with `CmdOrCtrl` so
/// Tauri maps it correctly per platform.
///
/// Bindings that aren't valid Tauri accelerators (vim chord strings
/// like `"dd"`, `"gg"`, `":q"`) are filtered out via
/// [`is_valid_accelerator_key`] so they don't render as garbled
/// menu accelerators.
fn resolve_accelerator(cmd: &CommandDef, keymap_mode: &str) -> Option<String> {
    let keys = cmd.keys.as_ref()?;
    let binding = match keymap_mode {
        "vim" => keys.vim.as_deref().or(keys.cua.as_deref()),
        "emacs" => keys.emacs.as_deref().or(keys.cua.as_deref()),
        _ => keys.cua.as_deref(),
    }?;
    if !is_valid_accelerator_key(binding) {
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

#[cfg(test)]
mod tests {
    use super::{collect_menu_entries, is_valid_accelerator_key, resolve_accelerator};
    use swissarmyhammer_commands::{compose_registry, CommandDef, KeysDef, UIState};

    /// Build a minimal `CommandDef` carrying only the per-mode keys —
    /// enough for `resolve_accelerator` to operate. All other fields
    /// take their default-ish values; the function never reads them.
    fn cmd_with_keys(
        id: &str,
        vim: Option<&str>,
        cua: Option<&str>,
        emacs: Option<&str>,
    ) -> CommandDef {
        CommandDef {
            id: id.to_string(),
            name: id.to_string(),
            menu_name: None,
            scope: None,
            visible: true,
            keys: Some(KeysDef {
                vim: vim.map(String::from),
                cua: cua.map(String::from),
                emacs: emacs.map(String::from),
            }),
            params: Vec::new(),
            undoable: false,
            context_menu: false,
            context_menu_group: None,
            context_menu_order: None,
            menu: None,
            view_kinds: None,
            tab_button: None,
        }
    }

    /// The composed registry contributed by `swissarmyhammer-focus` must
    /// land all nine `nav.*` commands under a single top-level
    /// `Navigation` submenu key. The native menu builder
    /// (`Menu::with_items` in `build_menu_from_commands`) feeds this map
    /// into `build_grouped_submenu(app, "Navigation",
    /// menus.get("Navigation"), …)`, so the count and grouping here is
    /// the load-bearing contract for the menu wiring.
    #[test]
    fn navigation_submenu_contains_all_nine_nav_commands() {
        let registry = compose_registry![
            swissarmyhammer_commands,
            swissarmyhammer_focus,
            swissarmyhammer_kanban,
        ];
        let ui_state = UIState::new();
        let menus = collect_menu_entries(&registry, &ui_state);

        let nav = menus
            .get("Navigation")
            .expect("Navigation submenu must exist after composing the focus crate");
        assert_eq!(
            nav.len(),
            9,
            "Navigation submenu must collect all 9 nav.* commands; got {:?}",
            nav.iter().map(|e| &e.id).collect::<Vec<_>>(),
        );

        // Verify every expected nav id appears under Navigation.
        let mut got_ids: Vec<&str> = nav.iter().map(|e| e.id.as_str()).collect();
        got_ids.sort();
        let expected_ids = [
            "nav.down",
            "nav.drillIn",
            "nav.drillOut",
            "nav.first",
            "nav.jump",
            "nav.last",
            "nav.left",
            "nav.right",
            "nav.up",
        ];
        assert_eq!(got_ids, expected_ids);

        // Entries must be sorted by (group, order). The YAML places
        // directional first (group 0), first/last next (group 1),
        // drill commands next (group 2), and `nav.jump` last
        // (group 3). Pull just the group sequence and assert it is
        // non-decreasing — that's the property
        // `append_grouped_entries` relies on to insert separators.
        let groups: Vec<usize> = nav.iter().map(|e| e.group).collect();
        let mut sorted_groups = groups.clone();
        sorted_groups.sort();
        assert_eq!(
            groups, sorted_groups,
            "Navigation entries must be sorted by group",
        );
        assert_eq!(groups.first().copied(), Some(0), "first group must be 0");
        assert_eq!(groups.last().copied(), Some(3), "last group must be 3");
    }

    /// The composed registry must place the AI panel toggle command
    /// (`ai.toggle`, contributed by `swissarmyhammer-kanban`) under a
    /// single top-level `View` submenu key. The native menu builder
    /// (`Menu::with_items` in `build_menu_from_commands`) feeds this map
    /// into `build_grouped_submenu(app, "View", menus.get("View"), …)`,
    /// so the presence of the `View` key and the `ai.toggle` entry is
    /// the load-bearing contract for the View menu wiring.
    #[test]
    fn view_submenu_contains_ai_toggle_command() {
        let registry = compose_registry![
            swissarmyhammer_commands,
            swissarmyhammer_focus,
            swissarmyhammer_kanban,
        ];
        let ui_state = UIState::new();
        let menus = collect_menu_entries(&registry, &ui_state);

        let view = menus
            .get("View")
            .expect("View submenu must exist once ai.toggle carries a menu placement");
        assert!(
            view.iter().any(|e| e.id == "ai.toggle"),
            "View submenu must collect the ai.toggle command; got {:?}",
            view.iter().map(|e| &e.id).collect::<Vec<_>>(),
        );
    }

    /// Single-character bindings are valid accelerator atoms — they
    /// pass straight through to muda's `parse_key`, which handles
    /// `[A-Za-z0-9]` and the punctuation atoms it lists.
    #[test]
    fn is_valid_accelerator_key_accepts_single_chars() {
        assert!(is_valid_accelerator_key("a"));
        assert!(is_valid_accelerator_key("g"));
        assert!(is_valid_accelerator_key("Z"));
        assert!(is_valid_accelerator_key("5"));
        assert!(is_valid_accelerator_key(","));
        assert!(is_valid_accelerator_key("/"));
    }

    /// Every Tauri/muda named key that appears in the YAML for `nav.*`
    /// commands must survive the filter. This is the load-bearing
    /// case — the bug that motivated this fix was that named keys
    /// like `Enter`, `Escape`, and `Arrow*` were being rejected.
    #[test]
    fn is_valid_accelerator_key_accepts_named_keys() {
        // Required by the task acceptance criteria
        for name in [
            "Enter",
            "Escape",
            "ArrowUp",
            "ArrowDown",
            "ArrowLeft",
            "ArrowRight",
            "Home",
            "End",
        ] {
            assert!(
                is_valid_accelerator_key(name),
                "named key {name} must be accepted as a valid accelerator atom",
            );
        }
        // A representative sample of the wider muda allowlist
        for name in [
            "Tab",
            "Space",
            "Backspace",
            "Delete",
            "PageUp",
            "PageDown",
            "Insert",
            "F1",
            "F12",
            "F24",
        ] {
            assert!(
                is_valid_accelerator_key(name),
                "named key {name} must be accepted as a valid accelerator atom",
            );
        }
    }

    /// Named-key matching is case-insensitive — muda's `parse_key`
    /// uppercases its input before comparing, so the YAML may use
    /// any casing (`enter`, `Enter`, `ENTER`).
    #[test]
    fn is_valid_accelerator_key_named_keys_are_case_insensitive() {
        assert!(is_valid_accelerator_key("enter"));
        assert!(is_valid_accelerator_key("ENTER"));
        assert!(is_valid_accelerator_key("ArRoWuP"));
    }

    /// Modifier chord strings — anything with `+` — pass through.
    /// Tauri's accelerator parser validates the individual tokens
    /// when the menu item is built, so the filter doesn't need to.
    #[test]
    fn is_valid_accelerator_key_accepts_modifier_chords() {
        assert!(is_valid_accelerator_key("Cmd+S"));
        assert!(is_valid_accelerator_key("Shift+G"));
        assert!(is_valid_accelerator_key("Alt+ArrowDown"));
        assert!(is_valid_accelerator_key("Mod+Shift+P"));
        assert!(is_valid_accelerator_key("Ctrl+p"));
        assert!(is_valid_accelerator_key("Alt+<"));
        assert!(is_valid_accelerator_key("Alt+>"));
    }

    /// Vim chord strings are the actual filter target. They appear
    /// in `keys.vim` for sequence bindings (`gg`, `dd`, `yy`, `:q`)
    /// and have no representation as a single Tauri accelerator —
    /// the frontend's `SEQUENCE_TABLES` handles them instead.
    #[test]
    fn is_valid_accelerator_key_rejects_vim_chord_strings() {
        assert!(!is_valid_accelerator_key("dd"));
        assert!(!is_valid_accelerator_key("gg"));
        assert!(!is_valid_accelerator_key(":q"));
        assert!(!is_valid_accelerator_key("yy"));
        assert!(!is_valid_accelerator_key("zo"));
    }

    /// Empty / whitespace-only input is rejected — there's nothing
    /// for the menu to render and Tauri would error on an empty
    /// accelerator string.
    #[test]
    fn is_valid_accelerator_key_rejects_empty_and_whitespace() {
        assert!(!is_valid_accelerator_key(""));
        assert!(!is_valid_accelerator_key("   "));
        assert!(!is_valid_accelerator_key("\t"));
    }

    /// Smoke test: with cua mode, `nav.up` (cua: ArrowUp) resolves
    /// to a non-None accelerator, demonstrating that the named-key
    /// path works end-to-end in `resolve_accelerator`.
    #[test]
    fn resolve_accelerator_returns_named_key_for_arrow_up() {
        let cmd = cmd_with_keys("nav.up", Some("k"), Some("ArrowUp"), Some("Ctrl+p"));
        assert_eq!(
            resolve_accelerator(&cmd, "cua"),
            Some("ArrowUp".to_string())
        );
    }

    /// `Mod+G` must canonicalise to `CmdOrCtrl+G` so Tauri picks
    /// the platform-specific modifier (Cmd on macOS, Ctrl elsewhere).
    #[test]
    fn resolve_accelerator_replaces_mod_with_cmd_or_ctrl() {
        let cmd = cmd_with_keys("nav.jump", Some("s"), Some("Mod+G"), Some("Mod+G"));
        assert_eq!(
            resolve_accelerator(&cmd, "cua"),
            Some("CmdOrCtrl+G".to_string()),
        );
    }

    /// Vim mode: `nav.first` has only `vim: gg` and `cua: Home` (no
    /// emacs binding). Under vim mode the chord string `gg` must be
    /// filtered out — leaving the menu item with no accelerator
    /// rather than rendering a garbled `gg` label. (`nav.first`'s
    /// real YAML omits the vim binding for exactly this reason —
    /// the chord is handled by `SEQUENCE_TABLES.vim` instead.)
    #[test]
    fn resolve_accelerator_filters_vim_chord_strings() {
        let cmd = cmd_with_keys("test.cmd", Some("gg"), Some("Home"), None);
        assert_eq!(resolve_accelerator(&cmd, "vim"), None);
        // Same command in cua mode picks up the cua binding instead.
        assert_eq!(resolve_accelerator(&cmd, "cua"), Some("Home".to_string()));
    }

    /// End-to-end check against the real YAML: every `nav.*` command
    /// that has a `cua` binding must render a non-None accelerator
    /// in cua mode. This is the AC checklist converted to an
    /// automated assertion — proves the fix without manual menu
    /// inspection.
    ///
    /// `nav.jump` is excluded from the cua-arrow assertion because
    /// it binds to `Mod+G` (a chord), not an Arrow/Home/End/Enter/
    /// Escape named key — it was already rendering correctly under
    /// the old filter and only proves the chord branch.
    #[test]
    fn nav_commands_render_accelerators_in_cua_mode() {
        let registry = compose_registry![
            swissarmyhammer_commands,
            swissarmyhammer_focus,
            swissarmyhammer_kanban,
        ];

        // Each (id, expected accelerator) pair maps directly to the
        // YAML in `swissarmyhammer-focus/builtin/commands/nav.yaml`.
        let expected = [
            ("nav.up", "ArrowUp"),
            ("nav.down", "ArrowDown"),
            ("nav.left", "ArrowLeft"),
            ("nav.right", "ArrowRight"),
            ("nav.first", "Home"),
            ("nav.last", "End"),
            ("nav.drillIn", "Enter"),
            ("nav.drillOut", "Escape"),
        ];

        for (id, want) in expected {
            let cmd = registry
                .get(id)
                .unwrap_or_else(|| panic!("registry must define {id}"));
            let got = resolve_accelerator(cmd, "cua");
            assert_eq!(
                got.as_deref(),
                Some(want),
                "{id} must resolve to {want:?} in cua mode (was the named-key allowlist regressed?)",
            );
        }
    }

    /// Same end-to-end check for vim mode: `nav.drillIn` /
    /// `nav.drillOut` carry `vim: Enter` / `vim: Escape` directly
    /// (those are not chord prefixes — see
    /// `swissarmyhammer-focus/builtin/commands/nav.yaml`), and
    /// `nav.last` carries `vim: Shift+G` (a chord with `+`). Each
    /// must produce a non-None accelerator; the directional vim
    /// bindings (`h`/`j`/`k`/`l`, single chars) are likewise valid.
    #[test]
    fn nav_commands_render_accelerators_in_vim_mode() {
        let registry = compose_registry![
            swissarmyhammer_commands,
            swissarmyhammer_focus,
            swissarmyhammer_kanban,
        ];

        let expected = [
            ("nav.up", "k"),
            ("nav.down", "j"),
            ("nav.left", "h"),
            ("nav.right", "l"),
            ("nav.drillIn", "Enter"),
            ("nav.drillOut", "Escape"),
            ("nav.last", "Shift+G"),
        ];

        for (id, want) in expected {
            let cmd = registry
                .get(id)
                .unwrap_or_else(|| panic!("registry must define {id}"));
            let got = resolve_accelerator(cmd, "vim");
            assert_eq!(
                got.as_deref(),
                Some(want),
                "{id} must resolve to {want:?} in vim mode",
            );
        }
    }

    /// `nav.first` has `cua: Home` and `emacs: Alt+<` but no vim
    /// binding — under vim mode the resolver falls back to cua,
    /// so the accelerator must be `Home` (not None). This exercises
    /// the fallback path through the named-key allowlist.
    #[test]
    fn resolve_accelerator_falls_back_to_cua_in_vim_mode() {
        let registry = compose_registry![
            swissarmyhammer_commands,
            swissarmyhammer_focus,
            swissarmyhammer_kanban,
        ];

        let cmd = registry
            .get("nav.first")
            .expect("registry must define nav.first");
        // Sanity: nav.first really has no vim binding in the YAML.
        let keys = cmd.keys.as_ref().expect("nav.first has keys");
        assert!(
            keys.vim.is_none(),
            "nav.first YAML must keep its vim binding empty so this test exercises the cua fallback",
        );

        assert_eq!(
            resolve_accelerator(cmd, "vim").as_deref(),
            Some("Home"),
            "nav.first under vim mode must fall back to its cua binding (Home)",
        );
    }
}
