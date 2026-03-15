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
use tauri::Manager;
use tauri_plugin_window_state::{StateFlags, WindowExt};
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
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_log::Builder::new().skip_logger().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all())
                .build(),
        )
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::log_command,
            commands::dispatch_command,
            commands::set_focus,
            commands::list_available_commands,
            commands::show_context_menu,
            commands::open_board,
            commands::list_open_boards,
            commands::set_active_board,
            commands::get_recent_boards,
            commands::get_keymap_mode,
            commands::set_keymap_mode,
            commands::get_ui_context,
            commands::set_active_view,
            commands::set_inspector_stack,
            commands::get_entity_schema,
            commands::list_entities,
            commands::get_entity,
            commands::search_mentions,
            commands::search_entities,
            commands::get_board_data,
            commands::quit_app,
            commands::reset_windows,
            commands::new_board_dialog,
            commands::open_board_dialog,
            commands::rebuild_menu_from_manifest,
            commands::list_views,
        ])
        .setup(|app| {
            // Build initial menu with OS chrome only — the frontend will
            // send the full manifest via rebuild_menu_from_manifest once loaded.
            let config = crate::state::AppConfig::load();
            let _ = menu::build_menu_from_manifest(app.handle(), &[], &config.recent_boards);

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
            // Explicitly restore saved position/size/monitor, then show.
            // The plugin's on_window_ready also calls restore_state, but
            // calling it again here is harmless and ensures it happens.
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.restore_state(StateFlags::all());
                let _ = win.show();
                let _ = win.set_focus();
            }

            // Register global hotkey for quick-capture window toggle
            {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                let handle = app.handle().clone();
                app.global_shortcut().on_shortcut("CmdOrCtrl+Shift+K", move |_app, _shortcut, event| {
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
                })?;
            }

            Ok(())
        })
        .on_menu_event(menu::handle_menu_event)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
