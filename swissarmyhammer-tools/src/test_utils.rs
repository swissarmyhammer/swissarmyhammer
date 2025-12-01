//! Test utilities for MCP tools

use crate::mcp::tool_handlers::ToolHandlers;
use crate::mcp::tool_registry::ToolContext;
use std::sync::Arc;

use swissarmyhammer_config::agent::AgentConfig;
use swissarmyhammer_git::GitOperations;
use tokio::sync::Mutex as TokioMutex;

#[cfg(test)]
use crate::mcp::progress_notifications::{ProgressNotification, ProgressSender};
#[cfg(test)]
use tokio::sync::mpsc;

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
    let agent_config = Arc::new(AgentConfig::default());

    let context = ToolContext::new(tool_handlers, git_ops, agent_config);

    // Set a test MCP server port for tests that need it
    *context.mcp_server_port.write().await = Some(8080);

    context
}

/// Helper for testing tools that send progress notifications
///
/// This helper consolidates the common pattern of:
/// 1. Creating a progress notification channel
/// 2. Setting up a test context with progress sender
/// 3. Executing a tool
/// 4. Collecting all progress notifications
///
/// # Type Parameters
/// * `F` - A future that executes the tool and returns a Result
///
/// # Arguments
/// * `tool_executor` - An async closure that receives a ToolContext and executes the tool
///
/// # Returns
/// A tuple containing:
/// * The result returned by the tool executor
/// * A vector of all progress notifications that were sent during execution
///
/// # Example
/// ```ignore
/// let (result, notifications) = execute_with_progress_capture(|context| async move {
///     let tool = MyTool::new();
///     let mut arguments = serde_json::Map::new();
///     arguments.insert("key".to_string(), json!("value"));
///     tool.execute(arguments, &context).await
/// }).await;
///
/// assert!(result.is_ok());
/// assert_eq!(notifications.len(), 2);
/// assert_eq!(notifications[0].progress, Some(0));
/// assert_eq!(notifications[1].progress, Some(100));
/// ```
#[cfg(test)]
pub async fn execute_with_progress_capture<F, Fut, T>(
    tool_executor: F,
) -> (T, Vec<ProgressNotification>)
where
    F: FnOnce(ToolContext) -> Fut,
    Fut: std::future::Future<Output = T>,
{
    // Create progress notification channel
    let (tx, mut rx) = mpsc::unbounded_channel();
    let progress_sender = ProgressSender::new(tx);

    // Set up test context with progress sender
    let mut context = create_test_context().await;
    context.progress_sender = Some(progress_sender);

    // Execute the tool
    let result = tool_executor(context).await;

    // Collect all notifications
    let mut notifications = Vec::new();
    while let Ok(notification) = rx.try_recv() {
        notifications.push(notification);
    }

    (result, notifications)
}
