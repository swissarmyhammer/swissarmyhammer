//! Test utilities for MCP tools

use crate::mcp::tool_handlers::ToolHandlers;
use crate::mcp::tool_registry::ToolContext;
use std::collections::HashMap;
use std::sync::Arc;

use swissarmyhammer_config::model::ModelConfig;
use swissarmyhammer_git::GitOperations;
use tokio::sync::Mutex as TokioMutex;

/// Git-specific test helpers
pub mod git_test_helpers;

/// Creates a test context with mock storage backends for testing MCP tools
///
/// This function creates a ToolContext similar to the one in swissarmyhammer,
/// but available for testing MCP tools in swissarmyhammer-tools.
/// Each call creates a unique test directory to prevent conflicts between parallel tests.
#[cfg(test)]
pub async fn create_test_context() -> ToolContext {
    let git_ops: Arc<TokioMutex<Option<GitOperations>>> = Arc::new(TokioMutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ModelConfig::default());

    let mut context = ToolContext::new(tool_handlers, git_ops, agent_config);

    // Initialize empty use case agents map
    context.use_case_agents = Arc::new(HashMap::new());

    // Set a test MCP server port for tests that need it
    *context.mcp_server_port.write().await = Some(8080);

    context
}
