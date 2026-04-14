// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod commands;
mod deeplink;
mod enrichment;
mod menu;
mod state;
mod tauri_reporter;
mod watcher;

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

    if handle_cli_and_exit(&cli, &rt) {
        return;
    }

    init_gui_tracing();
    let app_state = AppState::new();
    rt.block_on(app_state.auto_open_board());

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_log::Builder::new().skip_logger().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
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
        ])
        .setup(setup_app)
        .on_window_event(handle_window_event)
        .on_menu_event(menu::handle_menu_event)
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(handle_run_event);
}

/// Run the CLI subcommand if one was given (and it's not `gui`). Returns
/// `true` when the process should exit instead of starting the GUI.
///
/// CLI mode gets its own tracing subscriber — stderr only.
fn handle_cli_and_exit(cli: &Cli, rt: &tokio::runtime::Runtime) -> bool {
    if cli.command.is_none() {
        return false;
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    rt.block_on(cli::run_cli(cli))
}

/// Route tracing for GUI mode to macOS Console.app via os_log.
fn init_gui_tracing() {
    let oslog = tracing_oslog::OsLogger::new("com.swissarmyhammer.kanban", "default");
    tracing_subscriber::registry().with(oslog).init();
}

/// Tauri `.setup` handler: builds the native menu, wires deep-link handling,
/// starts file watchers, restores saved windows, configures the quick-capture
/// window, and registers the global hotkey.
fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    build_initial_menu(app);
    init_deep_links(app);

    let app_handle = app.handle().clone();
    let state = app.state::<AppState>();
    tauri::async_runtime::block_on(state.start_watchers(app_handle));

    restore_saved_windows(app);
    configure_quick_capture_window(app);
    register_global_hotkeys(app)?;
    Ok(())
}

/// Build the native menu bar from the command registry. At startup there are
/// no visible windows yet, so the visible-windows list is empty.
fn build_initial_menu(app: &tauri::App) {
    let state = app.state::<AppState>();
    let recent = state.ui_state.recent_boards();
    let registry = state.commands_registry.blocking_read();
    match menu::build_menu_from_commands(app.handle(), &registry, &state.ui_state, &recent, &[]) {
        Ok(items) => {
            *state.menu_items.lock().unwrap() = items;
        }
        Err(e) => {
            tracing::error!("Failed to build initial menu: {}", e);
        }
    }
}

/// Handle deep-link URLs delivered at cold start and register the
/// `on_open_url` callback for subsequent URLs.
fn init_deep_links(app: &tauri::App) {
    use tauri_plugin_deep_link::DeepLinkExt;
    if let Ok(Some(urls)) = app.deep_link().get_current() {
        for url in urls {
            deeplink::handle_url(app.handle(), url.to_string());
        }
    }
    let handle = app.handle().clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            deeplink::handle_url(&handle, url.to_string());
        }
    });
}

/// Restore every persisted board window (except `quick-capture`). If none
/// were restored, create a single window for the active/first-open board.
///
/// Board windows are created dynamically via `create_window_impl` — there is
/// no static "main" window and no primary/secondary distinction.
fn restore_saved_windows(app: &tauri::App) {
    let state = app.state::<AppState>();
    let saved_windows = state.ui_state.all_windows();
    let app_handle = app.handle().clone();
    let restore_state = app.state::<AppState>();
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
            &restore_state,
            Some(board_path),
            Some(label.clone()),
            geometry,
        )) {
            Ok(_) => restored_any = true,
            Err(e) => tracing::warn!(label = %label, error = %e, "setup: failed to restore window"),
        }
    }

    if restored_any {
        return;
    }
    if let Err(e) = tauri::async_runtime::block_on(commands::create_window_impl(
        &app_handle,
        &restore_state,
        None,
        None,
        None,
    )) {
        tracing::error!(error = %e, "setup: failed to create initial window");
    }
}

/// The quick-capture window must always start hidden — it is shown only by
/// the global hotkey toggle. Also explicitly disable the window shadow; the
/// shadow config in `tauri.conf.json` handles the initial state, but calling
/// `set_shadow(false)` here suppresses a macOS shadow/border artifact around
/// the transparent region.
fn configure_quick_capture_window(app: &tauri::App) {
    let Some(win) = app.get_webview_window("quick-capture") else {
        return;
    };
    let _ = win.set_shadow(false);
    clear_quick_capture_macos_backing(&win);
    let _ = win.hide();
}

/// On macOS the NSWindow backing layer still has a non-clear background
/// color even with `transparent: true`, producing a subtle glass-blur
/// rectangle behind the webview content. Clear it via the raw NSWindow
/// pointer that Tauri exposes when `macos-private-api` is enabled.
#[cfg(target_os = "macos")]
fn clear_quick_capture_macos_backing(win: &tauri::WebviewWindow) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    let Ok(ptr) = win.ns_window() else {
        return;
    };
    unsafe {
        let ns_win = ptr as *mut AnyObject;
        let cls = objc2::runtime::AnyClass::get(c"NSColor").unwrap();
        let clear: *mut AnyObject = msg_send![cls, clearColor];
        let _: () = msg_send![ns_win, setBackgroundColor: clear];
        let _: () = msg_send![ns_win, setOpaque: false];
    }
}

#[cfg(not(target_os = "macos"))]
fn clear_quick_capture_macos_backing(_win: &tauri::WebviewWindow) {}

/// Register `CmdOrCtrl+Shift+K` as the global toggle for the quick-capture
/// window (hide-if-visible, show-center-focus otherwise).
fn register_global_hotkeys(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let handle = app.handle().clone();
    app.global_shortcut()
        .on_shortcut("CmdOrCtrl+Shift+K", move |_app, _shortcut, event| {
            if event.state != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                return;
            }
            if let Some(win) = handle.get_webview_window("quick-capture") {
                if win.is_visible().unwrap_or(false) {
                    let _ = win.hide();
                } else {
                    let _ = win.center();
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
        })?;
    Ok(())
}

/// Tauri `.on_window_event` dispatcher.
fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    use tauri::WindowEvent;
    let label = window.label().to_string();
    if label == "quick-capture" {
        return;
    }
    match event {
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
            update_window_geometry(window, &label);
        }
        WindowEvent::Focused(true) => {
            let state = window.app_handle().state::<AppState>();
            if let Some(board_path) = state.ui_state.window_board(&label) {
                state.ui_state.set_most_recent_board(&board_path);
            }
        }
        WindowEvent::CloseRequested { .. } => {
            remove_window_entry_if_live(window, &label);
        }
        WindowEvent::Destroyed => {
            rebuild_menu_if_live(window);
        }
        _ => {}
    }
}

/// Update window geometry in memory on move/resize. No disk write — final
/// state is persisted on quit via `save()` in `ExitRequested`. Synchronous:
/// no async spawn, no race with shutdown.
fn update_window_geometry(window: &tauri::Window, label: &str) {
    let state = window.app_handle().state::<AppState>();
    if state.shutting_down.load(Ordering::SeqCst) {
        return;
    }
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return;
    };
    let maximized = window.is_maximized().unwrap_or(false);
    state
        .ui_state
        .update_window_geometry(label, pos.x, pos.y, size.width, size.height, maximized);
}

/// Mid-session close: user clicked X on a secondary window. Remove the
/// UIState entry synchronously BEFORE the window is destroyed, so it won't
/// resurrect on restart. During app quit, `ExitRequested` sets
/// `shutting_down=true` before `CloseRequested` fires, so entries are
/// preserved for restore.
fn remove_window_entry_if_live(window: &tauri::Window, label: &str) {
    let state = window.app_handle().state::<AppState>();
    if state.shutting_down.load(Ordering::SeqCst) {
        return;
    }
    state.ui_state.remove_window(label);
    tracing::info!(label = %label, "removed window entry on mid-session close");
}

/// Rebuild the Window menu when a secondary window is destroyed. Skipped
/// during shutdown since the app is exiting.
fn rebuild_menu_if_live(window: &tauri::Window) {
    let state = window.app_handle().state::<AppState>();
    if state.shutting_down.load(Ordering::SeqCst) {
        return;
    }
    crate::menu::rebuild_menu(window.app_handle());
}

/// Tauri `.run` handler. On `ExitRequested`, sets the shutdown flag
/// (stopping further window event mutations) and persists final window
/// geometry — the single save point capturing the latest positions.
fn handle_run_event(app_handle: &tauri::AppHandle, event: tauri::RunEvent) {
    let tauri::RunEvent::ExitRequested { .. } = event else {
        return;
    };
    let state = app_handle.state::<AppState>();
    state.shutting_down.store(true, Ordering::SeqCst);
    if let Err(e) = state.ui_state.save() {
        tracing::error!(error = %e, "failed to save UIState on exit");
    }
}
