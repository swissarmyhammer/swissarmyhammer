//! In-process MCP server for window-level operations and OS file actions.
//!
//! This crate exposes the `window` operation tool, consolidating window-manager
//! concerns and OS-level file actions into one server (per the "fewer servers"
//! decision).
//!
//! # Operations
//!
//! **window** group:
//! - **new** (`new window`) — open a new application window. Ports the original
//!   `create_window` Tauri command.
//! - **activate** (`activate window`) — focus an existing window.
//! - **set** (`set position`) — move a window to a logical-pixel position.
//! - **get** (`get position`) — read a window's current position.
//! - **get** (`get monitors`) — enumerate the connected monitors.
//! - **close** (`close window`) — close a window.
//!
//! **OS file actions** group:
//! - **open** (`open path`) — open a file in the OS default app. Backs
//!   `attachment.open`.
//! - **reveal** (`reveal path`) — reveal a file in the OS file manager. Backs
//!   `attachment.reveal`.
//!
//! **board lifecycle** group:
//! - **switch** (`switch board`) — switch the active board. Backs
//!   `file.switchBoard`; wraps `AppState::open_board`.
//! - **close** (`close board`) — close a board. Backs `file.closeBoard`; wraps
//!   `AppState::close_board`.
//! - **new** (`new board`) — create a board via the OS folder picker. Backs
//!   `file.newBoard`; ports `new_board_dialog`.
//! - **open** (`open board`) — open a board via the OS file-open dialog. Backs
//!   `file.openBoard`; ports `open_board_dialog`.
//!
//! The board open / close / init side effects and the OS dialog all thread
//! through state this crate cannot own (`AppState`, the tauri dialog plugin), so
//! — exactly as new-window creation does — they are supplied as injected
//! callbacks plus an injectable picker shim the app-shell bootstrap wires up.
//!
//! # Architecture
//!
//! [`WindowService`] is the `rmcp::ServerHandler`. It does not touch a
//! `tauri::AppHandle` or spawn OS processes directly; instead it holds an
//! `Arc<dyn WindowShell>` and routes every action through the [`WindowShell`]
//! seam. The production impl, [`TauriWindowShell`], backs the seam with a live
//! `AppHandle` plus the OS opener / file-manager commands; tests inject a
//! recording spy so the operation-dispatch path is exercised without a GUI or a
//! real Finder.
//!
//! Both the wire `inputSchema` and the discovery `_meta` tree are generated
//! from the same operation slice in [`operations`] via the `operation_tool!`
//! macro, so the two surfaces cannot drift.

pub mod operations;
pub mod service;
pub mod shell;

pub use operations::{
    operations, ActivateWindow, CloseBoard, CloseWindow, GetMonitors, GetWindowPosition, NewBoard,
    OpenBoard, OpenNewWindow, OpenPath, RevealPath, SetWindowPosition, SwitchBoard,
};
pub use service::WindowService;
pub use shell::{
    run_new_board, run_open_board, CloseBoardFn, CreatedBoard, InitBoardFn, MonitorInfo, NewWindow,
    OpenedBoard, OpenWindowFn, PickFolderFn, SwitchBoardFn, TauriWindowShell, WindowPosition,
    WindowShell,
};
