//! Memoranda management tools for MCP operations
//!
//! This module provides all memo-related tools using the tool registry pattern.
//! Each tool is in its own submodule with dedicated implementation and description.
//!
//! ## Memoranda Overview
//!
//! Memoranda are persistent knowledge artifacts stored with ULID identifiers for
//! time-ordered access and retrieval. They provide a flexible note-taking and
//! knowledge management system integrated with MCP operations.
//!
//! ## Data Model
//!
//! Each memo contains:
//! - **ULID**: Sortable unique identifier for chronological ordering
//! - **Title**: Human-readable memo identifier
//! - **Content**: Markdown-formatted memo body
//! - **Metadata**: Creation timestamp and other system information
//!
//! ## Tool Categories
//!
//! ### CRUD Operations
//! - **create**: Generate new memos with titles and content
//! - **get**: Retrieve individual memos by ULID
//! - **update**: Modify existing memo content (title remains unchanged)
//! - **delete**: Permanently remove memos (irreversible operation)
//!
//! ### Discovery & Search
//! - **list**: Get all memos with metadata previews
//! - **search**: Full-text search across titles and content
//! - **get_all_context**: Retrieve all memo content for AI context consumption
//!
//! ## MCP Integration Patterns
//!
//! All memoranda tools follow consistent patterns:
//! - Request/response types defined in `crate::mcp::memo_types`
//! - Tool handlers implemented in `crate::mcp::tool_handlers`
//! - Schema validation through JSON Schema definitions
//! - Comprehensive error handling with MCP-compatible error types
//!
//! ## Storage & Persistence
//!
//! Memos are persisted through the `memoranda::storage` module, providing:
//! - Atomic operations for data consistency
//! - ULID-based chronological access patterns
//! - Full-text search indexing for efficient queries

pub mod create;
pub mod delete;
pub mod get;
pub mod get_all_context;
pub mod list;
pub mod search;
pub mod update;

use crate::mcp::tool_registry::ToolRegistry;

/// Register all memoranda-related tools with the registry
pub fn register_memoranda_tools(registry: &mut ToolRegistry) {
    registry.register(create::CreateMemoTool::new());
    registry.register(list::ListMemoTool::new());
    registry.register(get_all_context::GetAllContextMemoTool::new());
    registry.register(get::GetMemoTool::new());
    registry.register(update::UpdateMemoTool::new());
    registry.register(delete::DeleteMemoTool::new());
    registry.register(search::SearchMemoTool::new());
}
