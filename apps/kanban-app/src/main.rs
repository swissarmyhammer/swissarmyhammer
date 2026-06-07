// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod cli;
mod cli_install;
mod command_services;
mod commands;
mod confine;
mod deeplink;
mod menu;
mod plugins;
mod state;
mod tauri_reporter;
mod ui_request;
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
    //
    // `AppState::new()` is async because it constructs the embedded plugin
    // platform (the `PluginHost` with its builtin and user-layer plugins), so
    // it is driven to completion on the runtime before the Tauri app is built.
    let app_state = rt.block_on(AppState::new());
    run_app(app_state);
}

fn run_app(app_state: AppState) {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_log::Builder::new().skip_logger().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::dispatch_command,
            commands::list_commands_for_scope,
            commands::get_ui_state,
            commands::get_entity_schema,
            commands::list_entity_types,
            commands::list_entities,
            commands::search_mentions,
            commands::search_entities,
            commands::quit_app,
            commands::new_board_dialog,
            commands::open_board_dialog,
            commands::list_views,
            commands::get_undo_state,
            commands::create_window,
            commands::save_dropped_file,
            commands::command_tool_call,
            commands::mcp_subscribe,
            ui_request::ui_request_reply,
            ai::models::ai_list_models,
            ai::models::ai_start_agent,
            ai::models::ai_set_streaming,
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
    // Hand the AppHandle to the focus MCP server's event sink so every
    // produced `FocusChangedEvent` is mirrored onto the `focus-changed`
    // Tauri event the React `SpatialFocusProvider` listens on — restores
    // the side-effecting `emit` the legacy `spatial_*` Tauri commands
    // did before Stage 3 of the kanban cut-over removed them.
    command_services::install_focus_event_app_handle(app.handle().clone());

    build_initial_menu(app);
    // Must run before auto_open_board / restore_session_windows. Cold-start
    // URL delivery is synchronous — when this returns, the board is open and
    // a window is visible, and `deep_link_handled` is set for the two
    // downstream steps to observe.
    wire_deep_links(app);

    let state = app.state::<AppState>();

    // Build the AppHandle-backed `window` / `app` shells (impossible before the
    // AppHandle existed), store them on AppState, expose them on the global
    // plugin host, and run the global host's DEFERRED plugin discovery. This is
    // where the global fallback host finally loads all 8 builtin command
    // plugins: four of them activate the `window` / `app` backends, which only
    // exist now. Must run BEFORE `auto_open_board` so each per-board host built
    // at board-open time reads the stored shells and loads the same baseline.
    let (window_shell, app_shell) = build_apphandle_shells(app.handle());
    tauri::async_runtime::block_on(state.install_apphandle_shells(window_shell, app_shell));

    tauri::async_runtime::block_on(state.auto_open_board());

    let app_handle = app.handle().clone();
    tauri::async_runtime::block_on(state.start_watchers(app_handle));

    // Start the plugin hot-reload watcher now that the app is up — the same
    // discover-early, watch-once-up pattern board watchers follow. The global
    // plugin host was discovered just above (after the AppHandle shells were
    // wired); per-board hosts discover at board-open time.
    tauri::async_runtime::block_on(state.start_plugin_watcher());

    // Skip session window restore when the user deep-linked — otherwise the
    // previous session's windows pile up on top of the one the deep-link
    // handler focused or created.
    if !state.deep_link_handled.load(Ordering::SeqCst) {
        restore_session_windows(app);
    }
    configure_quick_capture(app);
    register_quick_capture_hotkey(app)?;

    // Make the bundled `kanban` CLI reachable on the user's PATH. This is
    // silent, idempotent, and self-healing; it runs on a detached background
    // thread so `brew --prefix`, filesystem probes, or an `osascript` password
    // dialog never delay the GUI becoming interactive.
    cli_install::spawn();
    Ok(())
}

/// Construct the production `AppHandle`-backed shells the plugin hosts expose as
/// the `window` and `app` MCP modules.
///
/// `TauriAppShell` is a direct wrapper over the `AppHandle`. `TauriWindowShell`
/// additionally needs callbacks for the operations that thread through
/// `AppState` (which the window-service crate cannot reach) or show a native
/// dialog; each is wired to the app's existing, proven code paths here.
fn build_apphandle_shells(
    handle: &tauri::AppHandle,
) -> (
    std::sync::Arc<dyn swissarmyhammer_window_service::WindowShell>,
    std::sync::Arc<dyn swissarmyhammer_app_service::AppShell>,
) {
    use std::sync::Arc;
    use swissarmyhammer_kanban::board::InitBoard;
    use swissarmyhammer_kanban::{KanbanContext, KanbanOperationProcessor, OperationProcessor};
    use swissarmyhammer_window_service::{NewWindow, TauriWindowShell, WindowShell};
    use tauri_plugin_dialog::DialogExt;

    // open_new_window → the single window-creation path (`create_window_impl`),
    // run to completion on the confinement runtime (off the Tokio worker pool)
    // and mapped to the seam's result.
    let open_window: swissarmyhammer_window_service::OpenWindowFn<tauri::Wry> =
        Arc::new(|app: &tauri::AppHandle, board_path: Option<String>| {
            let app = app.clone();
            let value = crate::confine::run_future(async move {
                let state = app.state::<AppState>();
                commands::create_window_impl(&app, &state, board_path, None, None).await
            })?;
            Ok(NewWindow {
                label: value
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                board_path: value
                    .get("board_path")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            })
        });

    // pick_folder → the native folder picker, in blocking form (the seam shim
    // is synchronous).
    let pick_handle = handle.clone();
    let pick_folder: swissarmyhammer_window_service::PickFolderFn = Arc::new(move || {
        pick_handle
            .dialog()
            .file()
            .blocking_pick_folder()
            .and_then(|f| f.into_path().ok())
    });

    // init_board → run the in-process `InitBoard` op against a context rooted at
    // the resolved `.kanban` path; a no-op when the board already exists.
    let init_board: swissarmyhammer_window_service::InitBoardFn =
        Arc::new(|path: &std::path::Path, name: &str| {
            let path = path.to_path_buf();
            let name = name.to_string();
            crate::confine::run_future(async move {
                let ctx = KanbanContext::new(&path);
                if ctx.is_initialized() {
                    return Ok(());
                }
                KanbanOperationProcessor::new()
                    .process(&InitBoard::new(&name), &ctx)
                    .await
                    .map(|_| ())
                    .map_err(|e| format!("failed to init board: {e}"))
            })
        });

    // switch_board / close_board → thread through AppState's board lifecycle.
    let switch_board: swissarmyhammer_window_service::SwitchBoardFn<tauri::Wry> =
        Arc::new(|app: &tauri::AppHandle, path: &std::path::Path| {
            let app = app.clone();
            let path = path.to_path_buf();
            crate::confine::run_future(async move {
                let state = app.state::<AppState>();
                state.open_board(&path, Some(app.clone())).await.map(|_| ())
            })
        });
    let close_board: swissarmyhammer_window_service::CloseBoardFn<tauri::Wry> =
        Arc::new(|app: &tauri::AppHandle, path: &std::path::Path| {
            let app = app.clone();
            let path = path.to_path_buf();
            crate::confine::run_future(async move {
                let state = app.state::<AppState>();
                state.close_board(&path).await
            })
        });

    // list_open_boards / get_board_data → the multi-board management reads,
    // hosted on the `window` server alongside the board-lifecycle writes. Both
    // thread through `AppState` (the open-board set / per-board entity contexts),
    // which the window-service crate cannot reach, so — exactly like the
    // switch_board / close_board callbacks above — they run the existing
    // projection on the confinement runtime and hand the JSON back to the seam.
    let list_handle = handle.clone();
    let list_open_boards: swissarmyhammer_window_service::ListOpenBoardsFn = Arc::new(move || {
        let app = list_handle.clone();
        crate::confine::run_future(async move {
            let state = app.state::<AppState>();
            commands::list_open_boards_impl(&state).await
        })
    });
    let data_handle = handle.clone();
    let get_board_data: swissarmyhammer_window_service::GetBoardDataFn =
        Arc::new(move |board_path: Option<String>| {
            let app = data_handle.clone();
            crate::confine::run_future(async move {
                let state = app.state::<AppState>();
                commands::get_board_data_impl(&state, board_path).await
            })
        });

    let window_shell: Arc<dyn WindowShell> = Arc::new(TauriWindowShell::new(
        handle.clone(),
        open_window,
        pick_folder,
        init_board,
        switch_board,
        close_board,
        list_open_boards,
        get_board_data,
    ));

    // The `app` shell is now a plain AppHandle wrapper (quit / about / help).
    let app_shell: Arc<dyn swissarmyhammer_app_service::AppShell> = Arc::new(
        swissarmyhammer_app_service::TauriAppShell::new(handle.clone()),
    );
    (window_shell, app_shell)
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
    // Abort and drop this window's notification forwarder so the per-window
    // forwarder map never grows unbounded across a session and a reused Tauri
    // label re-binds cleanly on its next `mcp_subscribe`.
    crate::commands::unbind_window_forwarder(label);
    tracing::info!(label = %label, "removed window entry on mid-session close");
}

/// Rebuild the Window menu when a secondary window is destroyed. Actual
/// UIState cleanup already happened in `on_window_close_requested`.
fn on_window_destroyed(window: &tauri::Window) {
    let state = window.app_handle().state::<AppState>();
    if state.shutting_down.load(Ordering::SeqCst) {
        return;
    }
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
    // Stop every in-process AI agent endpoint so no loopback WebSocket server
    // outlives the process. Each `RunningAgent` also aborts its accept loop on
    // drop, but this drives the teardown deterministically at exit.
    tauri::async_runtime::block_on(state.running_agents.stop_all());
}
