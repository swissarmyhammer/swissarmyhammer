//! Integration tests for the `swissarmyhammer-app-service` crate.
//!
//! These tests stand up a real `AppService` wired to a recording `SpyShell`
//! that implements the `AppShell` seam, then drive the `app` MCP server
//! end-to-end. The service is constructed directly so the tests can exercise
//! its `ServerHandler` impl without spinning up the full plugin host or a live
//! GUI.

mod app_e2e;
mod common;
mod meta_snapshot;
