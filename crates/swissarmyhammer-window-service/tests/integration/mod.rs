//! Integration tests for the `swissarmyhammer-window-service` crate.
//!
//! These tests stand up a real `WindowService` wired to a recording `SpyShell`
//! that implements the `WindowShell` seam, then drive the `window` MCP server
//! end-to-end. The service is constructed directly so the tests can exercise
//! its `ServerHandler` impl without spinning up the full plugin host, a live
//! GUI, or a real file manager.

mod board_lifecycle_e2e;
mod common;
mod meta_snapshot;
mod window_e2e;
