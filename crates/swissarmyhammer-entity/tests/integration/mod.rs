//! Integration tests for the `swissarmyhammer-entity` crate.
//!
//! These tests stand up a real `EntityContext` kernel wired to a shared
//! `StoreContext` with `EntityTypeStore` handles registered for a couple of
//! entity types, then exercise the generic `entity` MCP server end-to-end.
//! The server is constructed directly so the test can drive its
//! `ServerHandler` impl without spinning up the full plugin host.

mod common;
mod entity_server_e2e;
mod meta_snapshot;
mod undo;
