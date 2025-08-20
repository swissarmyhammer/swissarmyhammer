//! Integration layer for calling MCP tools from CLI commands
//!
//! This module provides utilities for CLI commands to call MCP tools directly,
//! eliminating code duplication between CLI and MCP implementations.

use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use serde_json::Map;
use std::sync::Arc;
use swissarmyhammer_tools::{
    register_file_tools, register_issue_tools, register_memo_tools, register_search_tools,
    register_shell_tools, register_web_fetch_tools, register_web_search_tools,
};
use swissarmyhammer_tools::{ToolContext, ToolRegistry};
use tokio::sync::{Mutex, RwLock};

/// Type alias for issue storage to reduce complexity
type IssueStorageArc = Arc<RwLock<Box<dyn swissarmyhammer::issues::IssueStorage>>>;

/// CLI-specific tool context that can create and execute MCP tools
pub struct CliToolContext {
    tool_registry: ToolRegistry,
    tool_context: ToolContext,
}

impl CliToolContext {
    /// Check if Git operations are required and available, failing with appropriate error if not
    pub async fn require_git_repository(&self) -> Result<(), Box<dyn std::error::Error>> {
        let git_ops = self.tool_context.git_ops.lock().await;
        if git_ops.is_none() {
            return Err(Box::new(swissarmyhammer::SwissArmyHammerError::NotInGitRepository));
        }
        Ok(())
    }
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
        let rate_limiter = Self::create_rate_limiter();

        let tool_context = ToolContext::new(
            tool_handlers,
            issue_storage,
            git_ops,
            memo_storage,
            rate_limiter,
        );

        let tool_registry = Self::create_tool_registry();

        Ok(Self {
            tool_registry,
            tool_context,
        })
    }

    /// Create issue storage backend
    fn create_issue_storage(
        current_dir: &std::path::Path,
    ) -> Result<IssueStorageArc, Box<dyn std::error::Error>> {
        // Set working directory for migration context
        let original_dir = std::env::current_dir()?;
        if current_dir != original_dir {
            std::env::set_current_dir(current_dir)?;
        }

        // Create storage with automatic migration and detailed results
        let (storage, migration_result) =
            swissarmyhammer::issues::FileSystemIssueStorage::new_default_with_migration_info()?;

        // Log migration results for CLI users
        if let Some(result) = migration_result {
            match result {
                swissarmyhammer::issues::filesystem::MigrationResult::Success(stats) => {
                    eprintln!(
                        "✅ Migrated {} issues to .swissarmyhammer/issues",
                        stats.files_moved
                    );
                }
                swissarmyhammer::issues::filesystem::MigrationResult::NotNeeded(_) => {
                    // Silent for CLI - no need to inform about no migration
                }
            }
        }

        // Restore original directory
        if current_dir != original_dir {
            std::env::set_current_dir(original_dir)?;
        }

        Ok(Arc::new(RwLock::new(Box::new(storage))))
    }

    /// Create git operations handler
    fn create_git_operations(
        working_dir: &std::path::Path,
    ) -> Arc<Mutex<Option<swissarmyhammer::git::GitOperations>>> {
        Arc::new(Mutex::new(
            swissarmyhammer::git::GitOperations::with_work_dir(working_dir.to_path_buf()).ok(),
        ))
    }

    /// Create memo storage backend
    fn create_memo_storage(
        _current_dir: &std::path::Path,
    ) -> Arc<RwLock<Box<dyn swissarmyhammer::memoranda::MemoStorage>>> {
        // Use new_default() to respect SWISSARMYHAMMER_MEMOS_DIR environment variable
        let storage = swissarmyhammer::memoranda::storage::FileSystemMemoStorage::new_default()
            .unwrap_or_else(|_| {
                // Fallback to current directory if new_default() fails
                swissarmyhammer::memoranda::storage::FileSystemMemoStorage::new(
                    _current_dir.to_path_buf(),
                )
            });
        Arc::new(RwLock::new(Box::new(storage)))
    }

    /// Create tool handlers for backward compatibility
    fn create_tool_handlers(
        memo_storage: Arc<RwLock<Box<dyn swissarmyhammer::memoranda::MemoStorage>>>,
    ) -> Arc<swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers> {
        Arc::new(swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers::new(memo_storage))
    }

    /// Create rate limiter
    fn create_rate_limiter() -> Arc<dyn swissarmyhammer::common::rate_limiter::RateLimitChecker> {
        Arc::new(swissarmyhammer::common::rate_limiter::RateLimiter::new())
    }

    /// Create and populate tool registry
    fn create_tool_registry() -> ToolRegistry {
        let mut tool_registry = ToolRegistry::new();
        register_file_tools(&mut tool_registry);
        register_issue_tools(&mut tool_registry);
        register_memo_tools(&mut tool_registry);
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
    pub fn create_arguments(
        &self,
        pairs: Vec<(&str, serde_json::Value)>,
    ) -> Map<String, serde_json::Value> {
        let mut args = Map::new();
        for (key, value) in pairs {
            args.insert(key.to_string(), value);
        }
        args
    }
}

/// Utilities for formatting MCP responses for CLI display
pub mod response_formatting {
    use colored::*;
    use rmcp::model::{CallToolResult, RawContent};

    /// Extract and format success message from MCP response
    pub fn format_success_response(result: &CallToolResult) -> String {
        if result.is_error.unwrap_or(false) {
            format_error_response(result)
        } else {
            extract_text_content(result)
                .unwrap_or_else(|| "Operation completed successfully".to_string())
                .green()
                .to_string()
        }
    }

    /// Extract and format error message from MCP response
    pub fn format_error_response(result: &CallToolResult) -> String {
        extract_text_content(result)
            .unwrap_or_else(|| "An unknown error occurred".to_string())
            .red()
            .to_string()
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
    pub fn extract_json_data(
        result: &CallToolResult,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        if result.is_error.unwrap_or(false) {
            return Err(format!(
                "MCP tool returned error: {}",
                extract_text_content(result).unwrap_or_else(|| "Unknown error".to_string())
            )
            .into());
        }

        let text_content = extract_text_content(result).ok_or("No text content in MCP response")?;

        let json_data: serde_json::Value = serde_json::from_str(&text_content)
            .map_err(|e| format!("Failed to parse JSON response: {e}"))?;

        Ok(json_data)
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

    #[test]
    fn test_create_arguments() {
        let context = CliToolContext {
            tool_registry: ToolRegistry::new(),
            tool_context: create_mock_tool_context(),
        };

        let args = context.create_arguments(vec![("name", json!("test")), ("count", json!(42))]);

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
                }),
                None,
            )],
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
        let args = context.create_arguments(vec![("content", json!("Test memo"))]);

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

    #[test]
    fn test_rate_limiter_creation() {
        // Test that rate limiter can be created independently
        let rate_limiter1 = CliToolContext::create_rate_limiter();
        let rate_limiter2 = CliToolContext::create_rate_limiter();

        // Both rate limiters should be created successfully
        // This tests that the rate limiter creation is working properly
        // without the complexity of full context creation

        // We can't easily test the internals, but we can verify they exist
        // and that the creation doesn't panic or fail

        // Use Arc::ptr_eq to check they are different instances
        assert!(
            !Arc::ptr_eq(&rate_limiter1, &rate_limiter2),
            "Rate limiters should be different instances"
        );
    }

    // Helper function for tests
    fn create_mock_tool_context() -> ToolContext {
        use std::path::PathBuf;

        let issue_storage: IssueStorageArc = Arc::new(RwLock::new(Box::new(
            swissarmyhammer::issues::FileSystemIssueStorage::new(PathBuf::from("./test_issues"))
                .unwrap(),
        )));

        let git_ops: Arc<Mutex<Option<swissarmyhammer::git::GitOperations>>> =
            Arc::new(Mutex::new(None));

        let memo_storage: Arc<RwLock<Box<dyn swissarmyhammer::memoranda::MemoStorage>>> =
            Arc::new(RwLock::new(Box::new(
                swissarmyhammer::memoranda::storage::FileSystemMemoStorage::new_default()
                    .unwrap_or_else(|_| {
                        swissarmyhammer::memoranda::storage::FileSystemMemoStorage::new(
                            PathBuf::from("./test_issues"),
                        )
                    }),
            )));

        let tool_handlers = Arc::new(
            swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers::new(memo_storage.clone()),
        );

        let rate_limiter: Arc<dyn swissarmyhammer::common::rate_limiter::RateLimitChecker> =
            Arc::new(swissarmyhammer::common::rate_limiter::RateLimiter::new());

        ToolContext::new(
            tool_handlers,
            issue_storage,
            git_ops,
            memo_storage,
            rate_limiter,
        )
    }
}
