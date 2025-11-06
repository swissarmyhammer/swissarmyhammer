// Modules
pub mod ask;
mod persistence;
pub mod summary;

// Re-exports
pub use ask::QuestionAskTool;
pub use summary::QuestionSummaryTool;

use crate::mcp::tool_registry::ToolRegistry;

/// Register all question-related MCP tools
pub fn register_questions_tools(registry: &mut ToolRegistry) {
    registry.register(QuestionAskTool::new());
    registry.register(QuestionSummaryTool::new());
    tracing::debug!("Registered question tools");
}
