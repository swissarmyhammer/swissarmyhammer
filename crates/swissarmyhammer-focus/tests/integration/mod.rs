//! Integration tests for the `focus` MCP server.
//!
//! These tests stand up a real [`swissarmyhammer_focus::FocusServer`] over a
//! fresh registry / state and drive the `focus` operation tool end-to-end
//! through its `ServerHandler` impl, asserting the resulting focus state
//! matches the behavior the original `spatial_*` Tauri commands produced.

mod common;
mod focus_server_e2e;
mod meta_snapshot;
