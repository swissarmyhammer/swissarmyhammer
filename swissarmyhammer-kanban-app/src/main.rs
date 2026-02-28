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
    let cli = Cli::parse();

    // If a CLI subcommand was given (and it's not `gui`), handle it and exit
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    if rt.block_on(cli::run_cli(&cli)) {
        return;
    }

    // Otherwise, launch the Tauri GUI
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
            commands::rename_column,
            commands::update_task_title,
            commands::reorder_columns,
            commands::open_board,
            commands::list_open_boards,
            commands::set_active_board,
            commands::get_recent_boards,
        ])
        .setup(|app| {
            menu::rebuild_menu(app.handle());
            Ok(())
        })
        .on_menu_event(|app, event| menu::handle_menu_event(app, event))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
