// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod commands;
mod menu;
mod state;
mod watcher;

use clap::Parser;
use cli::Cli;
use state::AppState;
use tauri::Manager;
use tauri_plugin_window_state::{StateFlags, WindowExt};


fn main() {
    let cli = Cli::parse();

    // If a CLI subcommand was given (and it's not `gui`), handle it and exit.
    // CLI mode gets its own tracing subscriber since tauri-plugin-log isn't active.
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

    // Otherwise, launch the Tauri GUI — tauri-plugin-log owns the logger.
    tracing::info!("Launching Tauri GUI");
    let app_state = AppState::new();
    rt.block_on(app_state.auto_open_board());
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_log::Builder::new().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
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
            commands::get_board_data,
            commands::quit_app,
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
            app.handle().plugin(
                tauri_plugin_window_state::Builder::default().build()
            )?;

            // Start file watchers for boards opened during auto_open_board
            // (which ran before Tauri was ready, so didn't have an AppHandle).
            let app_handle = app.handle().clone();
            let state = app.state::<AppState>();
            tauri::async_runtime::block_on(state.start_watchers(app_handle));

            #[cfg(all(debug_assertions, target_os = "macos"))]
            {
                let win = app.get_webview_window("main").unwrap();
                win.restore_state(StateFlags::all())?;
                let _ = win.show();
            }

            Ok(())
        })
        .on_menu_event(menu::handle_menu_event)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
