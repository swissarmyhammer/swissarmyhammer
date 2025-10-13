//! Issue management tools for MCP operations
//!
//! This module provides all issue-related tools using the tool registry pattern.
//! Each tool is in its own submodule with dedicated implementation and description.
//!
//! ## Issue Workflow
//!
//! Issues are tracked as markdown files in the `./issues/` directory, following a complete
//! lifecycle from creation to completion:
//!
//! 1. **Creation**: `create` tool generates numbered issues (e.g., `000123_feature_name.md`)
//! 2. **Updates**: `update` tool modifies issue content and tracking information
//! 3. **Completion**: `mark_complete` tool moves issues to `./issues/complete/`
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
//! pub struct ExampleIssueTool;
//!
//! impl ExampleIssueTool {
//!     pub fn new() -> Self { Self }
//! }
//!
//! #[async_trait]
//! impl McpTool for ExampleIssueTool {
//!     fn name(&self) -> &'static str {
//!         "issue_example"
//!     }
//!     
//!     fn description(&self) -> &'static str {
//!         tool_descriptions::get_tool_description("issues", "example")
//!             .unwrap_or("Tool description not available")
//!     }
//!     
//!     fn schema(&self) -> serde_json::Value {
//!         serde_json::json!({})
//!     }
//!     
//!     async fn execute(
//!         &self,
//!         _arguments: serde_json::Map<String, serde_json::Value>,
//!         _context: &ToolContext,
//!     ) -> std::result::Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
//!         Ok(BaseToolImpl::create_success_response("Example executed"))
//!     }
//! }
//! ```
//!
//! ## Available Tools
//!
//! - **create**: Create new issues with auto-assigned numbers
//! - **list**: List all available issues with filtering and formatting options
//! - **show**: Display details of a specific issue by name
//! - **mark_complete**: Mark issues as completed and archive them
//! - **all_complete**: Check if all pending issues are completed
//! - **update**: Modify existing issue content and metadata

pub mod all_complete;
pub mod create;
pub mod list;
pub mod mark_complete;
pub mod show;
pub mod update;

use crate::mcp::tool_registry::ToolRegistry;

/// Register all issue-related tools with the registry
pub fn register_issue_tools(registry: &mut ToolRegistry) {
    registry.register(create::CreateIssueTool::new());
    registry.register(list::ListIssuesTool::new());
    registry.register(mark_complete::MarkCompleteIssueTool::new());
    registry.register(all_complete::AllCompleteIssueTool::new());
    registry.register(show::ShowIssueTool::new());
    registry.register(update::UpdateIssueTool::new());
}
