//! Integration tests for the `swissarmyhammer-store` crate.
//!
//! These tests stand up a real `StoreContext` with a small mock
//! `TrackedStore` implementation and exercise the `store` MCP server
//! end-to-end. They do not depend on any other workspace crate — the
//! plugin host is not involved here; the server is constructed
//! directly so the test can drive its `ServerHandler` impl.

mod common;
mod meta_snapshot;
mod store_server_e2e;
mod txn_grouping_e2e;
