//! Integration tests for the `swissarmyhammer-views` crate's `views` server.
//!
//! These tests stand up a real `PerspectiveContext` + `ViewsContext` wired to
//! a shared `StoreContext` (mirroring `wire_store_substrate`), then exercise
//! the `views` MCP server end-to-end. The server is constructed directly so
//! the tests can drive its `ServerHandler` impl without spinning up the full
//! plugin host.

mod common;
mod meta_snapshot;
mod undo;
mod views_e2e;
