//! In-process MCP server for app-shell actions.
//!
//! This crate exposes the `app` operation tool: genuine application-shell
//! actions that belong to the window manager / OS chrome rather than to any
//! document or UI panel.
//!
//! # Operations
//!
//! - **quit** (`quit app`) — terminate the process. Ports the original
//!   `quit_app` Tauri command (`AppHandle::exit(0)`).
//! - **about** (`show about`) — surface the app's name / version for an about
//!   dialog.
//! - **help** (`show help`) — route the user to the help / documentation.
//!
//! Undo / redo are deliberately absent — those are store-layer concerns and
//! live on the `store` MCP server. UI panel toggles (command palette, search)
//! live on the `ui_state` server.
//!
//! # Architecture
//!
//! [`AppService`] is the `rmcp::ServerHandler`. It does not touch a
//! `tauri::AppHandle` directly; instead it holds an `Arc<dyn AppShell>` and
//! routes every action through the [`AppShell`] seam. The production impl,
//! [`TauriAppShell`], backs the seam with a live `AppHandle`; tests inject a
//! recording spy so the operation-dispatch path is exercised without a GUI.
//!
//! Both the wire `inputSchema` and the discovery `_meta` tree are generated
//! from the same operation slice in [`operations`] via the `operation_tool!`
//! macro, so the two surfaces cannot drift.

pub mod operations;
pub mod service;
pub mod shell;

pub use operations::{operations, QuitApp, ShowAbout, ShowHelp};
pub use service::AppService;
pub use shell::{AboutInfo, AppShell, TauriAppShell, HELP_TARGET};
