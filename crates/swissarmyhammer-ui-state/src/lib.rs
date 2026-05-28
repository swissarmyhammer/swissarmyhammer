//! UI state for the kanban app, plus the in-process `ui_state` MCP server.
//!
//! This crate owns [`UIState`] — the thread-safe, file-backed state machine
//! for the app's UI surface: inspector stacks, command palette, keymap mode,
//! cross-window drag sessions, per-window geometry, and the recent-board MRU.
//! It was relocated here from `swissarmyhammer-commands` so it survives the
//! command-backend cut-over (that crate is deleted), and so the in-process
//! MCP server that wraps it can live alongside the type it mutates.
//!
//! # Modules
//!
//! - [`state`] — the relocated [`UIState`] struct and its supporting types
//!   ([`DragSession`], [`WindowState`], etc.). Moved verbatim; no behavior
//!   change.
//! - [`operations`] — the `#[operation]` structs that make up the `ui_state`
//!   operation tool's verb / noun / parameter surface.
//! - [`service`] — [`UiStateServer`], the `rmcp::ServerHandler` that routes
//!   `tools/call("ui_state", { op, … })` to the matching [`UIState`] method.
//!
//! # Scope
//!
//! The server exposes the UI-state mutations that the `ui.*`,
//! `settings.keymap.*`, `drag.*`, and the UI-toggle subset of `app.*`
//! commands depend on. Spatial focus is deliberately **not** here — it is
//! owned by the separate `focus` MCP server — so there is no `set_focus`
//! operation on `ui_state`.

pub mod operations;
pub mod service;
pub mod state;

pub use operations::operations;
pub use service::UiStateServer;
pub use state::{
    DragDestination, DragSession, DragSource, RecentBoard, UIState, UIStateChange, WindowState,
};
