//! Registration helpers that populate a `ToolRegistry`.

use super::context::ToolContext;
use super::newtypes::str_newtype;
use super::registry::ToolRegistry;
use rmcp::model::{LoggingLevel, LoggingMessageNotification, LoggingMessageNotificationParam};

/// Tool registration functions for organizing tools by category
///
/// This macro generates registration functions that follow a consistent pattern,
/// eliminating code duplication while maintaining clear documentation and functionality.
macro_rules! register_tool_category {
    ($fn_name:ident, $module:ident, $doc:literal) => {
        #[doc = $doc]
        pub fn $fn_name(registry: &mut ToolRegistry) {
            use crate::mcp::tools::$module;
            $module::$fn_name(registry);
        }
    };
}

str_newtype!(
    /// The name of the logger a tool reports MCP log notifications under.
    ///
    /// A distinct type so a logger name cannot be swapped with the message text.
    LoggerName
);

/// Send an MCP LoggingMessageNotification via the peer (fire-and-forget).
///
/// Shared helper for all tools that need to send log notifications to the MCP client.
/// If `context.peer` is `None`, this is a no-op.
pub async fn send_mcp_log(
    context: &ToolContext,
    level: LoggingLevel,
    logger: LoggerName<'_>,
    message: &str,
) {
    if let Some(peer) = &context.peer {
        let param = LoggingMessageNotificationParam {
            level,
            logger: Some(logger.as_ref().to_string()),
            data: serde_json::json!(message),
        };
        let _ = peer
            .send_notification(LoggingMessageNotification::new(param).into())
            .await;
    }
}

register_tool_category!(
    register_git_tools,
    git,
    "Register all git-related tools with the registry"
);
register_tool_category!(
    register_kanban_tools,
    kanban,
    "Register all kanban board tools with the registry"
);
register_tool_category!(
    register_web_tools,
    web,
    "Register all web-related tools with the registry"
);
register_tool_category!(
    register_code_context_tools,
    code_context,
    "Register all code context tools with the registry"
);
register_tool_category!(
    register_ralph_tools,
    ralph,
    "Register all ralph persistent instruction tools with the registry"
);
register_tool_category!(
    register_review_tools,
    review,
    "Register the operation-based review tool with the registry"
);
register_tool_category!(
    register_diagnostics_tools,
    diagnostics,
    "Register the operation-based diagnostics tool with the registry"
);

/// Create a fully registered tool registry with all available tools
///
/// This function creates a new ToolRegistry and registers all tool categories,
/// providing a single source of truth for tool registration that can be used
/// by both the MCP server implementation and tests.
///
/// # Returns
///
/// * `ToolRegistry` - A registry with all tools registered
pub async fn create_fully_registered_tool_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    // Register all tools exactly like McpServer does
    register_git_tools(&mut registry);
    register_kanban_tools(&mut registry);
    register_code_context_tools(&mut registry);
    register_web_tools(&mut registry);
    register_ralph_tools(&mut registry);
    register_review_tools(&mut registry);
    register_diagnostics_tools(&mut registry);

    registry
}
