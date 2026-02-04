//! Tree-sitter code analysis tools for MCP operations
//!
//! This module provides code intelligence tools powered by tree-sitter parsing
//! with semantic embeddings for similarity search and duplicate detection.
//!
//! ## Tool Overview
//!
//! - **treesitter_search**: Semantic search to find similar code chunks
//! - **treesitter_query**: Execute tree-sitter S-expression queries on parsed files
//! - **treesitter_duplicates**: Detect duplicate code clusters across the project
//! - **treesitter_status**: Get the current status of the code index
//!
//! ## Architecture
//!
//! The tools connect to a shared tree-sitter index leader process via RPC.
//! If no leader exists, queries will fail with a helpful error message suggesting
//! the user start the indexing process.
//!
//! ## Supported Languages
//!
//! The index supports 30+ programming languages including:
//! - Systems: Rust, C, C++, Go, Zig
//! - Web: JavaScript, TypeScript, HTML, CSS
//! - Backend: Python, Java, Ruby, PHP, C#
//! - Functional: Haskell, OCaml, Elixir, Scala
//! - Config: JSON, YAML, TOML, Markdown

pub mod duplicates;
pub mod query;
pub mod search;
mod shared;
pub mod status;

use crate::mcp::tool_registry::ToolRegistry;

/// Register all tree-sitter tools with the registry
pub fn register_treesitter_tools(registry: &mut ToolRegistry) {
    registry.register(search::TreesitterSearchTool::new());
    registry.register(query::TreesitterQueryTool::new());
    registry.register(duplicates::TreesitterDuplicatesTool::new());
    registry.register(status::TreesitterStatusTool::new());
}
