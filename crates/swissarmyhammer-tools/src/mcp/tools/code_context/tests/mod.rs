//! The `code_context` tool's tests, split the way the tool itself is.
//!
//! - [`support`] — the fixtures every group builds on: a tool context rooted
//!   at a temporary directory, an indexed project, and the chunk-table reads
//!   the indexer assertions make.
//! - [`tool`] — the tool surface: registration, name, description, both
//!   schemas, and dispatch of an op the tool does not have.
//! - [`ops`] — one group per operation, driven through the real dispatch.
//! - [`indexer`] — the indexing pass, its progress events, and what it writes.
//! - [`search_code`] — `search code` answering while the index is still
//!   filling.

mod indexer;
mod ops;
mod search_code;
mod support;
mod tool;
