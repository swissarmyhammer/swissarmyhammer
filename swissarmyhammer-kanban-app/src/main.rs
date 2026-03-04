// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod commands;
mod menu;
mod state;

use clap::Parser;
use cli::Cli;
use state::AppState;

fn main() {
    // Initialize tracing subscriber so log output is visible.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    // If a CLI subcommand was given (and it's not `gui`), handle it and exit
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    if rt.block_on(cli::run_cli(&cli)) {
        return;
    }

    // Otherwise, launch the Tauri GUI
    tracing::info!("Launching Tauri GUI");
    let app_state = AppState::new();
    rt.block_on(app_state.auto_open_board());
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_board,
            commands::list_tasks,
            commands::move_task,
            commands::add_task,
            commands::reorder_columns,
            commands::update_tag,
            commands::show_tag_context_menu,
            commands::untag_task,
            commands::open_board,
            commands::list_open_boards,
            commands::set_active_board,
            commands::get_recent_boards,
            commands::get_keymap_mode,
            commands::set_keymap_mode,
            commands::get_entity_schema,
            commands::update_entity_field,
            commands::delete_task,
            commands::delete_tag,
            commands::delete_column,
            commands::delete_actor,
            commands::delete_swimlane,
            commands::delete_attachment,
            commands::undo_operation,
            commands::redo_operation,
        ])
        .setup(|app| {
            menu::rebuild_menu(app.handle());
            Ok(())
        })
        .on_menu_event(menu::handle_menu_event)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
