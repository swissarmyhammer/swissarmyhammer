//! Integration tests for the `swissarmyhammer-ui-state` crate.
//!
//! These tests stand up a real `UiStateServer` over a temp-file-backed
//! `UIState` and drive the `ui_state` MCP server end-to-end through its
//! `ServerHandler` / `call_tool` path, then observe the persisted state. The
//! service is constructed directly so the tests exercise the dispatch path
//! without spinning up the full plugin host or touching the real home dir.

mod common;
mod meta_snapshot;
mod ui_state_e2e;
