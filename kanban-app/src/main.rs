// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod commands;
mod deeplink;
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

    // If a CLI subcommand was given (and it's not `gui`), handle it and exit.
    // CLI mode gets its own tracing subscriber — stderr only.
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    if cli.command.is_some() {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .init();
        if rt.block_on(cli::run_cli(&cli)) {
            return;
        }
    }

    // GUI mode — route tracing to macOS Console.app via os_log.
    if cli.command.is_none() {
        let oslog = tracing_oslog::OsLogger::new("com.swissarmyhammer.kanban", "default");
        tracing_subscriber::registry().with(oslog).init();
    }

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
            commands::reset_windows,
            commands::new_board_dialog,
            commands::open_board_dialog,
            commands::list_views,
            commands::get_undo_state,
            commands::create_window,
        ])
        .setup(|app| {
            // Build native menu bar from the command registry.
            {
                let state = app.state::<AppState>();
                let recent = state.ui_state.recent_boards();
                let registry = state.commands_registry.blocking_read();
                // At startup there are no visible windows yet, so pass an empty list.
                match menu::build_menu_from_commands(
                    app.handle(),
                    &registry,
                    &state.ui_state,
                    &recent,
                    &[],
                ) {
                    Ok(items) => {
                        *state.menu_items.lock().unwrap() = items;
                    }
                    Err(e) => {
                        tracing::error!("Failed to build initial menu: {}", e);
                    }
                }
            }

            // Handle deep-link URLs at cold start
            {
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

            // Start file watchers for boards opened during auto_open_board
            // (which ran before Tauri was ready, so didn't have an AppHandle).
            let app_handle = app.handle().clone();
            let state = app.state::<AppState>();
            tauri::async_runtime::block_on(state.start_watchers(app_handle));

            // The window starts hidden (visible: false in tauri.conf.json).
            // Restore saved geometry from UIState, then show.
            if let Some(win) = app.get_webview_window("main") {
                if let Some(main_state) = state.ui_state.get_window_state("main") {
                    if let (Some(x), Some(y)) = (main_state.x, main_state.y) {
                        let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
                    }
                    if let (Some(w), Some(h)) = (main_state.width, main_state.height) {
                        let _ = win.set_size(tauri::PhysicalSize::new(w, h));
                    }
                    if main_state.maximized {
                        let _ = win.maximize();
                    }
                }
                let _ = win.show();
                let _ = win.set_focus();
            }

            // Restore secondary windows from persisted UIState.
            // Uses the same create_window_impl path as window.new — no special
            // cases, fully observable in logs.
            {
                let saved_windows = state.ui_state.all_windows();
                let app_handle = app.handle().clone();
                let restore_state = app.state::<AppState>();
                for (label, entry) in &saved_windows {
                    if label == "main" || label == "quick-capture" {
                        continue;
                    }
                    let board_path = match state.ui_state.window_board(label) {
                        Some(bp) => bp,
                        None => continue,
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
                    if let Err(e) = tauri::async_runtime::block_on(commands::create_window_impl(
                        &app_handle,
                        &restore_state,
                        Some(board_path),
                        Some(label.clone()),
                        geometry,
                        false, // skip rebuild_menu — blocking_read panics inside block_on
                    )) {
                        tracing::warn!(
                            label = %label, error = %e,
                            "setup: failed to restore secondary window"
                        );
                    }
                }
            }

            // The quick-capture window must always start hidden — it is shown
            // only by the global hotkey toggle.
            //
            // We also explicitly disable the window shadow at runtime.  The
            // shadow config in tauri.conf.json handles the initial state, but
            // calling set_shadow(false) here ensures the platform chrome is
            // suppressed on macOS. Without this, macOS renders a visible
            // shadow/border artifact around the transparent region.
            if let Some(win) = app.get_webview_window("quick-capture") {
                let _ = win.set_shadow(false);

                // On macOS the NSWindow backing layer still has a non-clear
                // background color even with `transparent: true`, producing a
                // subtle glass-blur rectangle behind the webview content.
                // Clear it via the raw NSWindow pointer that Tauri exposes
                // when the `macos-private-api` feature is enabled.
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

            // Register global hotkey for quick-capture window toggle
            {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                let handle = app.handle().clone();
                app.global_shortcut().on_shortcut(
                    "CmdOrCtrl+Shift+K",
                    move |_app, _shortcut, event| {
                        if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                            if let Some(win) = handle.get_webview_window("quick-capture") {
                                if win.is_visible().unwrap_or(false) {
                                    let _ = win.hide();
                                } else {
                                    let _ = win.center();
                                    let _ = win.show();
                                    let _ = win.set_focus();
                                }
                            }
                        }
                    },
                )?;
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            use tauri::WindowEvent;
            let label = window.label().to_string();

            match event {
                // Update geometry in memory on move/resize. No disk write —
                // final state is persisted on quit via save() in ExitRequested.
                // Synchronous: no async spawn, no race with shutdown.
                WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                    if label == "quick-capture" {
                        return;
                    }
                    let state = window.app_handle().state::<AppState>();
                    if state.shutting_down.load(Ordering::SeqCst) {
                        return;
                    }
                    if let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) {
                        let maximized = window.is_maximized().unwrap_or(false);
                        // Memory-only update — no disk IO. update_window_geometry
                        // only updates existing entries, preventing zombie creation
                        // from stale events during teardown.
                        state.ui_state.update_window_geometry(
                            &label,
                            pos.x,
                            pos.y,
                            size.width,
                            size.height,
                            maximized,
                        );
                    }
                }
                // When a board window gains focus, update most_recent_board_path so quick
                // capture and commands without an explicit board_path target the right board.
                WindowEvent::Focused(true) => {
                    if label == "quick-capture" {
                        return;
                    }
                    let state = window.app_handle().state::<AppState>();
                    if let Some(board_path) = state.ui_state.window_board(&label) {
                        state.ui_state.set_most_recent_board(&board_path);
                    }
                    // Update only the Window menu checkmarks — much cheaper than a
                    // full rebuild_menu which tears down and recreates all native
                    // menu items, causing visible flicker on macOS.
                    crate::menu::update_window_focus_checkmarks(window.app_handle(), &label);
                }
                // Mid-session close: user clicked X on a secondary window.
                // Remove the UIState entry synchronously BEFORE the window is
                // destroyed, so it won't resurrect on restart. During app quit,
                // ExitRequested sets shutting_down=true before CloseRequested
                // fires, so entries are preserved for restore.
                WindowEvent::CloseRequested { .. } => {
                    if label == "main" || label == "quick-capture" {
                        return;
                    }
                    let state = window.app_handle().state::<AppState>();
                    if state.shutting_down.load(Ordering::SeqCst) {
                        return;
                    }
                    // Synchronous remove + disk write — deterministic, no race.
                    state.ui_state.remove_window(&label);
                    tracing::info!(label = %label, "removed window entry on mid-session close");
                }
                // Rebuild the Window menu when a secondary window is destroyed.
                // Actual UIState cleanup happened in CloseRequested above.
                WindowEvent::Destroyed => {
                    if label == "main" || label == "quick-capture" {
                        return;
                    }
                    // Don't rebuild menu during shutdown — the app is exiting.
                    let state = window.app_handle().state::<AppState>();
                    if state.shutting_down.load(Ordering::SeqCst) {
                        return;
                    }
                    crate::menu::rebuild_menu(window.app_handle());
                }
                _ => {}
            }
        })
        .on_menu_event(menu::handle_menu_event)
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let state = app_handle.state::<AppState>();
                // Set flag FIRST so window event handlers stop mutating state.
                state.shutting_down.store(true, Ordering::SeqCst);
                // Persist final window geometry. Move/resize events only update
                // memory (via update_window_geometry), so this is the single
                // save point that captures the latest positions.
                if let Err(e) = state.ui_state.save() {
                    tracing::error!(error = %e, "failed to save UIState on exit");
                }
            }
        });
}
