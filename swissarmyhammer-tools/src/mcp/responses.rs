//! Response creation utilities for MCP operations

use rmcp::model::*;

/// Create a success response for MCP tool calls
pub fn create_success_response(message: String) -> CallToolResult {
    CallToolResult::success(vec![Content::text(message)])
}

/// Create an error response for MCP tool calls
pub fn create_error_response(message: String) -> CallToolResult {
    CallToolResult::error(vec![Content::text(message)])
}
