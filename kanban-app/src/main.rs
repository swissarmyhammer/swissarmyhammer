// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod commands;
mod deeplink;
mod menu;
mod spatial;
mod state;
mod tauri_reporter;
// Board-fixture factory used by in-process Rust tests (unit + tauri
// integration). `#[cfg(test)]` keeps it out of production binaries and
// scopes the `tempfile` dev-dependency to the test build.
#[cfg(test)]
mod test_support;
mod watcher;

/// Re-exported so integration tests and binary-adjacent tooling can install
/// the custom `tracing` subscriber layer that routes records into Tauri's log
/// plugin.
pub use tauri_reporter::TauriReporter;

use clap::Parser;
use cli::Cli;
use state::AppState;
use std::sync::atomic::Ordering;
use tauri::Manager;
use tracing_subscriber::prelude::*;

fn main() {
    let cli = Cli::parse();
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    // CLI mode gets a stderr subscriber; GUI mode routes to macOS Console.app
    // via os_log. A CLI subcommand like `init` or `list` terminates here; `gui`
    // (or no subcommand) falls through to launch the GUI.
    if cli.command.is_some() {
        init_cli_tracing();
        if rt.block_on(cli::run_cli(&cli)) {
            return;
        }
    } else {
        init_gui_tracing();
    }

    // NOTE: `auto_open_board()` is intentionally NOT called here. A
    // `kanban://open/...` deep link delivered to `setup_app` must win over
    // whatever was open last session, but the URL isn't available until the
    // Tauri app exists. Session restore is driven from inside `setup_app`,
    // after the deep-link handler has had its chance to set
    // `AppState::deep_link_handled`.
    run_app(AppState::new());
}

fn run_app(app_state: AppState) {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_log::Builder::new().skip_logger().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(app_state)
        // `kanban_invoke_handler!` (defined in `spatial.rs`) is the single
        // source of truth for the debug-only command surface: it appends
        // `__spatial_dump` to the handler list in debug builds and drops it
        // entirely in release builds. No `#[cfg]` required here — the macro
        // internalizes the distinction so this call site cannot drift out of
        // sync with the command definition.
        .invoke_handler(kanban_invoke_handler![
            commands::log_command,
            commands::dispatch_command,
            commands::list_commands_for_scope,
            commands::show_context_menu,
            commands::list_open_boards,
            commands::get_ui_state,
            commands::get_entity_schema,
            commands::list_entity_types,
            commands::list_entities,
            commands::get_entity,
            commands::search_mentions,
            commands::search_entities,
            commands::get_board_data,
            commands::quit_app,
            commands::new_board_dialog,
            commands::open_board_dialog,
            commands::list_views,
            commands::get_undo_state,
            commands::create_window,
            commands::save_dropped_file,
            spatial::spatial_register,
            spatial::spatial_register_batch,
            spatial::spatial_unregister,
            spatial::spatial_unregister_batch,
            spatial::spatial_focus,
            spatial::spatial_clear_focus,
            spatial::spatial_navigate,
            spatial::spatial_push_layer,
            spatial::spatial_remove_layer,
        ])
        .setup(setup_app)
        .on_window_event(handle_window_event)
        .on_menu_event(menu::handle_menu_event)
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(handle_run_event);
}

fn init_cli_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
}

fn init_gui_tracing() {
    let oslog = tracing_oslog::OsLogger::new("com.swissarmyhammer.kanban", "default");
    tracing_subscriber::registry().with(oslog).init();
}

fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    build_initial_menu(app);
    // Must run before auto_open_board / restore_session_windows. Cold-start
    // URL delivery is synchronous — when this returns, the board is open and
    // a window is visible, and `deep_link_handled` is set for the two
    // downstream steps to observe.
    wire_deep_links(app);

    let state = app.state::<AppState>();

    tauri::async_runtime::block_on(state.auto_open_board());

    let app_handle = app.handle().clone();
    tauri::async_runtime::block_on(state.start_watchers(app_handle));

    // Skip session window restore when the user deep-linked — otherwise the
    // previous session's windows pile up on top of the one the deep-link
    // handler focused or created.
    if !state.deep_link_handled.load(Ordering::SeqCst) {
        restore_session_windows(app);
    }
    configure_quick_capture(app);
    register_quick_capture_hotkey(app)?;
    Ok(())
}

fn build_initial_menu(app: &tauri::App) {
    let state = app.state::<AppState>();
    let recent = state.ui_state.recent_boards();
    let registry = state.commands_registry.blocking_read();
    // At startup there are no visible windows yet, so pass an empty list.
    match menu::build_menu_from_commands(app.handle(), &registry, &state.ui_state, &recent, &[]) {
        Ok(items) => {
            *state.menu_items.lock().unwrap() = items;
        }
        Err(e) => {
            tracing::error!("Failed to build initial menu: {}", e);
        }
    }
}

fn wire_deep_links(app: &tauri::App) {
    use tauri_plugin_deep_link::DeepLinkExt;
    // Cold start: drive to completion synchronously so the window exists
    // before setup returns and the `deep_link_handled` flag is visible to
    // downstream setup steps.
    if let Ok(Some(urls)) = app.deep_link().get_current() {
        for url in urls {
            deeplink::handle_url_blocking(app.handle(), url.to_string());
        }
    }
    // Warm start: a second `kanban open` delivered via `on_open_url` to this
    // running instance runs off a worker thread (macOS only — other
    // platforms would need `tauri-plugin-single-instance`).
    let handle = app.handle().clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            deeplink::handle_url(&handle, url.to_string());
        }
    });
}

/// Restore ALL board windows from persisted UIState. Every board window is
/// created dynamically via `create_window_impl` — no static "main" window,
/// no primary/secondary distinction. Falls back to one window for the first
/// open board when nothing was restored.
fn restore_session_windows(app: &tauri::App) {
    let state = app.state::<AppState>();
    let saved_windows = state.ui_state.all_windows();
    let app_handle = app.handle().clone();
    let mut restored_any = false;

    for (label, entry) in &saved_windows {
        if label == "quick-capture" {
            continue;
        }
        let Some(board_path) = state.ui_state.window_board(label) else {
            continue;
        };
        let geometry = match (entry.x, entry.y, entry.width, entry.height) {
            (Some(x), Some(y), Some(w), Some(h)) => Some(commands::WindowGeometry {
                x,
                y,
                width: w,
                height: h,
                maximized: entry.maximized,
            }),
            _ => None,
        };
        match tauri::async_runtime::block_on(commands::create_window_impl(
            &app_handle,
            &state,
            Some(board_path),
            Some(label.clone()),
            geometry,
        )) {
            Ok(_) => restored_any = true,
            Err(e) => tracing::warn!(label = %label, error = %e, "setup: failed to restore window"),
        }
    }

    if !restored_any {
        // create_window_impl resolves board from active/first-open when None.
        if let Err(e) = tauri::async_runtime::block_on(commands::create_window_impl(
            &app_handle,
            &state,
            None,
            None,
            None,
        )) {
            tracing::error!(error = %e, "setup: failed to create initial window");
        }
    }
}

/// The quick-capture window must always start hidden — it is shown only by
/// the global hotkey toggle. We also explicitly disable the window shadow at
/// runtime: the shadow config in tauri.conf.json handles the initial state,
/// but calling set_shadow(false) here ensures the platform chrome is
/// suppressed on macOS. Without this, macOS renders a visible shadow/border
/// artifact around the transparent region.
fn configure_quick_capture(app: &tauri::App) {
    let Some(win) = app.get_webview_window("quick-capture") else {
        return;
    };
    let _ = win.set_shadow(false);

    // On macOS the NSWindow backing layer still has a non-clear background
    // color even with `transparent: true`, producing a subtle glass-blur
    // rectangle behind the webview content. Clear it via the raw NSWindow
    // pointer Tauri exposes when `macos-private-api` is enabled.
    #[cfg(target_os = "macos")]
    {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;
        if let Ok(ptr) = win.ns_window() {
            unsafe {
                let ns_win = ptr as *mut AnyObject;
                let cls = objc2::runtime::AnyClass::get(c"NSColor").unwrap();
                let clear: *mut AnyObject = msg_send![cls, clearColor];
                let _: () = msg_send![ns_win, setBackgroundColor: clear];
                let _: () = msg_send![ns_win, setOpaque: false];
            }
        }
    }
    let _ = win.hide();
}

fn register_quick_capture_hotkey(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let handle = app.handle().clone();
    app.global_shortcut()
        .on_shortcut("CmdOrCtrl+Shift+K", move |_app, _shortcut, event| {
            if event.state != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                return;
            }
            let Some(win) = handle.get_webview_window("quick-capture") else {
                return;
            };
            if win.is_visible().unwrap_or(false) {
                let _ = win.hide();
            } else {
                let _ = win.center();
                let _ = win.show();
                let _ = win.set_focus();
            }
        })?;
    Ok(())
}

fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    use tauri::WindowEvent;
    let label = window.label();
    if label == "quick-capture" {
        return;
    }
    match event {
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
            on_window_geometry_changed(window, label)
        }
        WindowEvent::Focused(true) => on_window_focused(window, label),
        WindowEvent::CloseRequested { .. } => on_window_close_requested(window, label),
        WindowEvent::Destroyed => on_window_destroyed(window),
        _ => {}
    }
}

/// Update geometry in memory on move/resize. No disk write — final state is
/// persisted on quit via `save()` in ExitRequested. Synchronous: no async
/// spawn, no race with shutdown.
fn on_window_geometry_changed(window: &tauri::Window, label: &str) {
    let state = window.app_handle().state::<AppState>();
    if state.shutting_down.load(Ordering::SeqCst) {
        return;
    }
    if let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) {
        let maximized = window.is_maximized().unwrap_or(false);
        state.ui_state.update_window_geometry(
            label,
            pos.x,
            pos.y,
            size.width,
            size.height,
            maximized,
        );
    }
}

/// When a board window gains focus, update `most_recent_board_path` so quick
/// capture and commands without an explicit `board_path` target the right
/// board. Menu rebuild is handled by the frontend re-dispatching `ui.setFocus`
/// on window focus.
fn on_window_focused(window: &tauri::Window, label: &str) {
    let state = window.app_handle().state::<AppState>();
    if let Some(board_path) = state.ui_state.window_board(label) {
        state.ui_state.set_most_recent_board(&board_path);
    }
}

/// Mid-session close: user clicked X on a secondary window. Remove the UIState
/// entry synchronously BEFORE the window is destroyed so it won't resurrect
/// on restart. During app quit, ExitRequested sets `shutting_down=true` before
/// CloseRequested fires, so entries are preserved for restore.
fn on_window_close_requested(window: &tauri::Window, label: &str) {
    let state = window.app_handle().state::<AppState>();
    if state.shutting_down.load(Ordering::SeqCst) {
        return;
    }
    state.ui_state.remove_window(label);
    tracing::info!(label = %label, "removed window entry on mid-session close");
}

/// Rebuild the Window menu when a secondary window is destroyed, and drop
/// the window's per-window `SpatialState` so its entries don't leak back
/// into a freshly-created window that happens to reuse the label. Actual
/// UIState cleanup already happened in `on_window_close_requested`.
fn on_window_destroyed(window: &tauri::Window) {
    let state = window.app_handle().state::<AppState>();
    if state.shutting_down.load(Ordering::SeqCst) {
        return;
    }
    // Spatial state cleanup has to happen here, not in CloseRequested —
    // CloseRequested can be vetoed and the window can live on; Destroyed is
    // the point of no return.
    let label = window.label().to_string();
    let app_handle = window.app_handle().clone();
    tauri::async_runtime::spawn(async move {
        let state = app_handle.state::<AppState>();
        state.remove_spatial_state(&label).await;
    });
    crate::menu::rebuild_menu(window.app_handle());
}

fn handle_run_event(app_handle: &tauri::AppHandle, event: tauri::RunEvent) {
    let tauri::RunEvent::ExitRequested { .. } = event else {
        return;
    };
    let state = app_handle.state::<AppState>();
    // Set flag FIRST so window event handlers stop mutating state.
    state.shutting_down.store(true, Ordering::SeqCst);
    // Persist final window geometry. Move/resize events only update memory
    // (via update_window_geometry), so this is the single save point that
    // captures the latest positions.
    if let Err(e) = state.ui_state.save() {
        tracing::error!(error = %e, "failed to save UIState on exit");
    }
}
