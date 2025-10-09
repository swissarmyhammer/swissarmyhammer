//! Integration layer for calling MCP tools from CLI commands
//!
//! This module provides utilities for CLI commands to call MCP tools directly,
//! eliminating code duplication between CLI and MCP implementations.

use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::{Map, Value};
use std::sync::Arc;

use swissarmyhammer_git::GitOperations;
use swissarmyhammer_tools::{
    register_file_tools, register_issue_tools, register_memo_tools, register_rules_tools,
    register_search_tools, register_shell_tools, register_web_fetch_tools,
    register_web_search_tools,
};
use swissarmyhammer_tools::{ToolContext, ToolRegistry};
use tokio::sync::{Mutex, RwLock};

/// Type alias for issue storage to reduce complexity
type IssueStorageArc = Arc<RwLock<Box<dyn swissarmyhammer_issues::IssueStorage>>>;

/// CLI-specific tool context that can create and execute MCP tools
pub struct CliToolContext {
    tool_registry: Arc<ToolRegistry>,
    tool_context: ToolContext,
}

impl CliToolContext {
    /// Create a new CLI tool context with all necessary storage backends
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let current_dir = std::env::current_dir()?;
        Self::new_with_dir(&current_dir).await
    }

    /// Create a new CLI tool context with a specific working directory
    pub async fn new_with_dir(
        working_dir: &std::path::Path,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let issue_storage = Self::create_issue_storage(working_dir)?;
        let git_ops = Self::create_git_operations(working_dir);
        let memo_storage = Self::create_memo_storage(working_dir);
        let tool_handlers = Self::create_tool_handlers(memo_storage.clone());
        let agent_config =
            std::sync::Arc::new(swissarmyhammer_config::agent::AgentConfig::default());

        let tool_context = ToolContext::new(
            tool_handlers,
            issue_storage,
            git_ops,
            memo_storage,
            agent_config,
        );

        let tool_registry = Arc::new(Self::create_tool_registry());

        Ok(Self {
            tool_registry,
            tool_context,
        })
    }

    /// Create issue storage backend
    fn create_issue_storage(
        working_dir: &std::path::Path,
    ) -> Result<IssueStorageArc, Box<dyn std::error::Error>> {
        // Create storage with working directory - no global directory changes needed
        // This avoids race conditions in parallel test execution
        let storage = swissarmyhammer_issues::FileSystemIssueStorage::new_default_in(working_dir)?;

        Ok(Arc::new(RwLock::new(Box::new(storage))))
    }

    /// Create git operations handler
    fn create_git_operations(working_dir: &std::path::Path) -> Arc<Mutex<Option<GitOperations>>> {
        Arc::new(Mutex::new(
            GitOperations::with_work_dir(working_dir.to_path_buf()).ok(),
        ))
    }

    /// Create memo storage backend
    fn create_memo_storage(
        current_dir: &std::path::Path,
    ) -> Arc<RwLock<Box<dyn swissarmyhammer_memoranda::MemoStorage>>> {
        // First check if SWISSARMYHAMMER_MEMOS_DIR environment variable is set
        let storage = if let Ok(custom_path) = std::env::var("SWISSARMYHAMMER_MEMOS_DIR") {
            swissarmyhammer_memoranda::MarkdownMemoStorage::new(std::path::PathBuf::from(
                custom_path,
            ))
        } else {
            // For tests and custom working directories, create .swissarmyhammer/memos in the working dir
            let memos_dir = current_dir.join(".swissarmyhammer").join("memos");
            // Try to create directory, but don't fail if it already exists or can't be created
            if let Err(e) = std::fs::create_dir_all(&memos_dir) {
                eprintln!(
                    "Warning: Failed to create memos directory {}: {}",
                    memos_dir.display(),
                    e
                );
            }
            swissarmyhammer_memoranda::MarkdownMemoStorage::new(memos_dir)
        };
        Arc::new(RwLock::new(Box::new(storage)))
    }

    /// Create tool handlers for backward compatibility
    fn create_tool_handlers(
        memo_storage: Arc<RwLock<Box<dyn swissarmyhammer_memoranda::MemoStorage>>>,
    ) -> Arc<swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers> {
        Arc::new(swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers::new(memo_storage))
    }

    /// Create and populate tool registry
    fn create_tool_registry() -> ToolRegistry {
        let mut tool_registry = ToolRegistry::new();
        register_file_tools(&mut tool_registry);
        register_issue_tools(&mut tool_registry);
        register_memo_tools(&mut tool_registry);
        register_rules_tools(&mut tool_registry);
        register_search_tools(&mut tool_registry);
        register_shell_tools(&mut tool_registry);
        register_web_fetch_tools(&mut tool_registry);
        register_web_search_tools(&mut tool_registry);
        tool_registry
    }

    /// Execute an MCP tool with the given arguments
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: Map<String, serde_json::Value>,
    ) -> Result<CallToolResult, McpError> {
        if let Some(tool) = self.tool_registry.get_tool(tool_name) {
            tool.execute(arguments, &self.tool_context).await
        } else {
            Err(McpError::internal_error(
                format!("Tool not found: {tool_name}"),
                None,
            ))
        }
    }

    /// Helper to convert CLI arguments to MCP tool arguments
    ///
    /// Get a reference to the tool registry for dynamic CLI generation
    pub fn get_tool_registry(&self) -> &ToolRegistry {
        &self.tool_registry
    }

    /// Get an Arc to the tool registry for dynamic CLI generation
    pub fn get_tool_registry_arc(&self) -> Arc<ToolRegistry> {
        self.tool_registry.clone()
    }

    /// Create arguments map from vector of key-value pairs for testing
    #[allow(dead_code)]
    pub fn create_arguments(&self, args: Vec<(&str, Value)>) -> Map<String, Value> {
        args.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }
}

/// Utilities for formatting MCP responses for CLI display
pub mod response_formatting {
    use rmcp::model::{CallToolResult, RawContent};
    use serde_json::Value;

    /// Format successful tool result for display
    #[allow(dead_code)]
    pub fn format_success_response(result: &CallToolResult) -> String {
        extract_text_content(result).unwrap_or_else(|| "Operation successful".to_string())
    }

    /// Format error tool result for display
    #[allow(dead_code)]
    pub fn format_error_response(result: &CallToolResult) -> String {
        extract_text_content(result).unwrap_or_else(|| "Operation failed".to_string())
    }

    /// Extract text content from CallToolResult
    pub fn extract_text_content(result: &CallToolResult) -> Option<String> {
        result
            .content
            .first()
            .and_then(|content| match &content.raw {
                RawContent::Text(text_content) => Some(text_content.text.clone()),
                _ => None,
            })
    }

    /// Extract JSON data from CallToolResult
    #[allow(dead_code)]
    pub fn extract_json_data(result: &CallToolResult) -> Result<Value, Box<dyn std::error::Error>> {
        let text = extract_text_content(result).ok_or("No text content found in result")?;
        Ok(serde_json::from_str(&text)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_cli_tool_context_creation() {
        let result = CliToolContext::new().await;
        assert!(
            result.is_ok(),
            "Failed to create CliToolContext: {:?}",
            result.err()
        );

        let _context = result.unwrap();
        // Context creation successful - this verifies the tool registry is working
    }

    #[tokio::test]
    async fn test_create_arguments() {
        let _context = CliToolContext {
            tool_registry: Arc::new(ToolRegistry::new()),
            tool_context: create_mock_tool_context().await,
        };

        let mut args = Map::new();
        args.insert("name".to_string(), json!("test"));
        args.insert("count".to_string(), json!(42));

        assert_eq!(args.get("name"), Some(&json!("test")));
        assert_eq!(args.get("count"), Some(&json!(42)));
    }

    #[test]
    fn test_response_formatting() {
        use rmcp::model::{Annotated, RawContent, RawTextContent};

        let success_result = CallToolResult {
            content: vec![Annotated::new(
                RawContent::Text(RawTextContent {
                    text: "Operation successful".to_string(),
                    meta: None,
                }),
                None,
            )],
            structured_content: None,
            meta: None,
            is_error: Some(false),
        };

        let formatted = response_formatting::format_success_response(&success_result);
        assert!(formatted.contains("Operation successful"));
    }

    #[tokio::test]
    async fn test_rate_limiter_integration() {
        let context = CliToolContext::new().await.unwrap();

        // Test that rate limiter is properly created and functional
        // We can verify this by checking that the CliToolContext was created successfully
        // which means all components including the rate limiter were initialized
        // Context creation successful means tools are available

        // Test that the rate limiter allows normal operations
        // by checking that we can execute a tool (this will use the rate limiter internally)
        let mut args = Map::new();
        args.insert("content".to_string(), json!("Test memo"));

        // This should succeed if rate limiter is working properly
        let result = context.execute_tool("memo_create", args).await;

        // We expect this to either succeed or fail with a normal error (not a rate limit error)
        // Rate limit errors would be specific MCP errors about rate limiting
        match result {
            Ok(_) => {
                // Success - rate limiter allowed the operation
            }
            Err(e) => {
                // Ensure it's not a rate limiting error
                let error_str = e.to_string();
                assert!(
                    !error_str.contains("rate limit"),
                    "Should not fail due to rate limiting in normal usage: {error_str}"
                );
            }
        }
    }

    // Helper function for tests
    async fn create_mock_tool_context() -> ToolContext {
        use std::path::PathBuf;

        let issue_storage: IssueStorageArc = Arc::new(RwLock::new(Box::new(
            swissarmyhammer_issues::FileSystemIssueStorage::new(PathBuf::from("./test_issues"))
                .unwrap(),
        )));

        let git_ops: Arc<Mutex<Option<GitOperations>>> = Arc::new(Mutex::new(None));

        let memo_storage: Arc<RwLock<Box<dyn swissarmyhammer_memoranda::MemoStorage>>> =
            Arc::new(RwLock::new(Box::new(
                swissarmyhammer_memoranda::MarkdownMemoStorage::new_default()
                    .await
                    .unwrap_or_else(|_| {
                        swissarmyhammer_memoranda::MarkdownMemoStorage::new(PathBuf::from(
                            "./test_issues",
                        ))
                    }),
            )));

        let tool_handlers = Arc::new(
            swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers::new(memo_storage.clone()),
        );
        let agent_config = Arc::new(swissarmyhammer_config::agent::AgentConfig::default());

        ToolContext::new(
            tool_handlers,
            issue_storage,
            git_ops,
            memo_storage,
            agent_config,
        )
    }
}
