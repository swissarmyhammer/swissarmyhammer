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
//! The board-file lifecycle operations (`SwitchBoard` / `CloseBoard` /
//! `NewBoard` / `OpenBoard`) are a separate follow-up task on this same crate
//! and are deliberately absent.
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
    operations, ActivateWindow, CloseWindow, GetMonitors, GetWindowPosition, OpenNewWindow,
    OpenPath, RevealPath, SetWindowPosition,
};
pub use service::WindowService;
pub use shell::{
    MonitorInfo, NewWindow, OpenWindowFn, TauriWindowShell, WindowPosition, WindowShell,
};
