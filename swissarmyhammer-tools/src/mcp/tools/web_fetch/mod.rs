//! Web fetch tools for MCP operations
//!
//! This module provides web content fetching tools using the tool registry pattern.
//! Each tool is in its own submodule with dedicated implementation and description.
//!
//! ## Web Fetch Workflow
//!
//! Web fetch tools provide capability to retrieve and process web content for AI workflows:
//!
//! 1. **Content Fetching**: `fetch` tool retrieves web pages with HTML-to-markdown conversion
//! 2. **Security Controls**: URL validation, content-type verification, and rate limiting
//! 3. **Error Handling**: Comprehensive network and content processing error management
//! 4. **Metadata Extraction**: Response headers, timing, and content analysis
//!
//! ## Tool Implementation Pattern
//!
//! Each tool follows the standard MCP pattern:
//! ```rust,no_run
//! use async_trait::async_trait;
//! use swissarmyhammer_tools::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
//! use swissarmyhammer_tools::mcp::tool_descriptions;
//!
//! #[derive(Default)]
//! pub struct ExampleWebFetchTool;
//!
//! impl ExampleWebFetchTool {
//!     pub fn new() -> Self { Self }
//! }
//!
//! #[async_trait]
//! impl McpTool for ExampleWebFetchTool {
//!     fn name(&self) -> &'static str { "web_fetch_example" }
//!
//!     fn description(&self) -> &'static str {
//!         tool_descriptions::get_tool_description("web_fetch", "example")
//!             .expect("Tool description should be available")
//!     }
//!
//!     fn schema(&self) -> serde_json::Value {
//!         serde_json::json!({ "type": "object" })
//!     }
//!
//!     async fn execute(
//!         &self,
//!         arguments: serde_json::Map<String, serde_json::Value>,
//!         _context: &ToolContext,
//!     ) -> std::result::Result<rmcp::model::CallToolResult, rmcp::Error> {
//!         Ok(BaseToolImpl::create_success_response("Example executed"))
//!     }
//! }
//! ```
//!
//! ## Available Tools
//!
//! - **fetch**: Retrieve web content and convert HTML to markdown for AI processing

pub mod fetch;
pub mod security;

use crate::mcp::tool_registry::ToolRegistry;

/// Register all web fetch-related tools with the registry
pub fn register_web_fetch_tools(registry: &mut ToolRegistry) {
    registry.register(fetch::WebFetchTool::new());
}
