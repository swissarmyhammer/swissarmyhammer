//! In-process MCP server for the generic, type-agnostic `entity` operation tool.
//!
//! This crate hosts the `entity` MCP face over the entity **kernel**
//! ([`swissarmyhammer_entity::EntityContext`]). It lives "above" both the
//! kernel and the search crate so it can wrap [`swissarmyhammer_entity_search`]'s
//! `EntitySearchIndex` in a `Search` op without creating a dependency cycle —
//! `swissarmyhammer-entity-search` already depends on `swissarmyhammer-entity`,
//! so the server cannot live in the kernel crate (clean direction:
//! `entity-mcp → entity-search → entity`).
//!
//! - [`operations`] holds the `#[operation]` structs that are the single
//!   source of truth for the tool's verb / noun / description / parameters.
//! - [`server`] holds [`EntityServer`], the `rmcp::ServerHandler` that routes
//!   `tools/call` to the matching kernel method (or the search index).

pub mod operations;
pub mod server;

pub use server::EntityServer;
