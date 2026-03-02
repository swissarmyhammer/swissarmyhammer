//! Integration layer for calling MCP tools from CLI commands
//!
//! This module provides utilities for CLI commands to call MCP tools directly,
//! eliminating code duplication between CLI and MCP implementations.
//!
//! sah rule ignore test_rule_with_allow

use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::{Map, Value};
use std::sync::Arc;

use swissarmyhammer_tools::mcp::server::McpServer;
use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server_with_options, McpServerMode};
use swissarmyhammer_tools::ToolRegistry;
use swissarmyhammer_tools::{
    register_file_tools, register_flow_tools, register_git_tools, register_js_tools,
    register_kanban_tools, register_questions_tools, register_shell_tools,
    register_treesitter_tools, register_web_tools,
};
use tokio::sync::RwLock;

/// CLI-specific tool context that can create and execute MCP tools
pub struct CliToolContext {
    tool_registry: Arc<RwLock<ToolRegistry>>,
    /// MCP server handle (must be kept alive for LlamaAgent to work)
    mcp_server_handle: Option<swissarmyhammer_tools::mcp::unified_server::McpServerHandle>,
    /// In-process server for isolated execution (no HTTP, no env var mutation)
    server: Option<Arc<McpServer>>,
}

impl CliToolContext {
    /// Create a new CLI tool context with all necessary storage backends
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let current_dir = std::env::current_dir()?;
        Self::new_with_config(&current_dir, None).await
    }

    /// Create a fully isolated context with no HTTP server and no env var mutation.
    ///
    /// Creates an in-process `McpServer` with agent_mode=true (all tools registered)
    /// using only the provided working directory. Safe for parallel test execution.
    pub async fn new_isolated(
        working_dir: &std::path::Path,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        use swissarmyhammer_prompts::PromptLibrary;

        let mcp_server = McpServer::new_with_work_dir(
            PromptLibrary::default(),
            working_dir.to_path_buf(),
            None,
            true,
        )
        .await?;
        mcp_server.initialize().await?;
        let server_arc = Arc::new(mcp_server);

        let tool_registry = Self::create_tool_registry().await;
        let tool_registry_arc = Arc::new(RwLock::new(tool_registry));

        Ok(Self {
            tool_registry: tool_registry_arc,
            mcp_server_handle: None,
            server: Some(server_arc),
        })
    }

    /// Create a new CLI tool context with optional model override
    ///
    /// # Arguments
    ///
    /// * `working_dir` - The working directory for tool operations
    /// * `model_override` - Optional model name to use for ALL use cases (runtime override)
    ///
    /// # Returns
    ///
    /// Result containing the initialized CliToolContext or an error
    pub async fn new_with_config(
        working_dir: &std::path::Path,
        model_override: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize MCP server with model override
        // The server will create its own tool_context with the correct model configuration
        let mcp_server_handle =
            Self::initialize_mcp_server(model_override, Some(working_dir.to_path_buf())).await?;

        let tool_registry = Self::create_tool_registry().await;
        let tool_registry_arc = Arc::new(RwLock::new(tool_registry));

        Ok(Self {
            tool_registry: tool_registry_arc,
            mcp_server_handle: Some(mcp_server_handle),
            server: None,
        })
    }

    /// Initialize MCP HTTP server, resolving agent mode from model config.
    async fn initialize_mcp_server(
        model_override: Option<&str>,
        working_dir: Option<std::path::PathBuf>,
    ) -> Result<
        swissarmyhammer_tools::mcp::unified_server::McpServerHandle,
        Box<dyn std::error::Error>,
    > {
        // Resolve the agent type to decide whether to register agent tools.
        // Claude Code has native tools (Bash, Read, Write, Edit, skills) so it
        // does NOT need MCP agent tools (shell, files, skill).
        // LlamaAgent has no native tools and relies entirely on MCP.
        use swissarmyhammer_config::model::{ModelExecutorType, ModelManager, ModelPaths};
        let agent_mode = match ModelManager::resolve_agent_config(&ModelPaths::sah()) {
            Ok(config) => config.executor_type() != ModelExecutorType::ClaudeCode,
            Err(_) => false, // Default is ClaudeCode, so no agent tools needed
        };

        Self::initialize_mcp_server_with_agent_mode(model_override, working_dir, agent_mode).await
    }

    /// Initialize MCP HTTP server with an explicit agent mode setting.
    ///
    /// When `agent_mode` is true, agent-specific tools (files, shell, skill) are
    /// registered in addition to the always-available domain tools.
    async fn initialize_mcp_server_with_agent_mode(
        model_override: Option<&str>,
        working_dir: Option<std::path::PathBuf>,
        agent_mode: bool,
    ) -> Result<
        swissarmyhammer_tools::mcp::unified_server::McpServerHandle,
        Box<dyn std::error::Error>,
    > {
        tracing::info!("Starting MCP HTTP server for CLI tool context");

        std::env::set_var("SAH_CLI_MODE", "1");

        tracing::info!("Agent mode for MCP server: {agent_mode}");

        let mcp_server_handle = start_mcp_server_with_options(
            McpServerMode::Http { port: None },
            None,
            model_override.map(|s| s.to_string()),
            working_dir,
            agent_mode,
        )
        .await?;

        tracing::info!(
            "MCP HTTP server ready on port {:?}",
            mcp_server_handle.info().port
        );

        Ok(mcp_server_handle)
    }

    /// Create and populate tool registry
    ///
    /// This should mirror the registration in `swissarmyhammer_tools::mcp::server::register_all_tools`
    async fn create_tool_registry() -> ToolRegistry {
        let mut tool_registry = ToolRegistry::new();
        register_js_tools(&mut tool_registry);
        register_file_tools(&mut tool_registry).await;
        register_flow_tools(&mut tool_registry);
        register_git_tools(&mut tool_registry);
        register_kanban_tools(&mut tool_registry);
        register_questions_tools(&mut tool_registry);
        register_shell_tools(&mut tool_registry);
        register_treesitter_tools(&mut tool_registry);
        register_web_tools(&mut tool_registry);
        tool_registry
    }

    /// Resolve the McpServer instance from either the isolated server or the HTTP handle
    fn resolve_server(&self) -> Result<Arc<McpServer>, McpError> {
        if let Some(ref server) = self.server {
            return Ok(server.clone());
        }
        self.mcp_server_handle
            .as_ref()
            .and_then(|h| h.server())
            .ok_or_else(|| {
                McpError::internal_error("MCP server instance not available".to_string(), None)
            })
    }

    /// Execute an MCP tool with the given arguments
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: Map<String, serde_json::Value>,
    ) -> Result<CallToolResult, McpError> {
        let server = self.resolve_server()?;
        server
            .execute_tool(tool_name, serde_json::Value::Object(arguments))
            .await
    }

    /// Get an Arc to the tool registry for dynamic CLI generation
    pub fn get_tool_registry_arc(&self) -> Arc<RwLock<ToolRegistry>> {
        self.tool_registry.clone()
    }

    /// Create arguments map from vector of key-value pairs
    pub fn create_arguments(&self, args: Vec<(&str, Value)>) -> Map<String, Value> {
        args.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }
}

/// Utilities for formatting MCP responses for CLI display
pub mod response_formatting {
    use rmcp::model::{CallToolResult, RawContent};
    use serde_json::Value;

    /// Format successful tool result for display
    /// This is the ONE PLACE where we convert JSON output to YAML for display
    pub fn format_success_response(result: &CallToolResult) -> String {
        // First check if there's structured content - serialize it to YAML
        if let Some(ref data) = result.structured_content {
            return serde_yaml::to_string(data).unwrap_or_else(|_| {
                serde_json::to_string_pretty(data)
                    .unwrap_or_else(|_| "Operation successful".to_string())
            });
        }

        // Try to extract text content and parse as JSON, then convert to YAML
        if let Some(text) = extract_text_content(result) {
            // Try to parse as JSON
            if let Ok(json_value) = serde_json::from_str::<Value>(&text) {
                // Successfully parsed as JSON - convert to YAML with leading newline
                return serde_yaml::to_string(&json_value)
                    .map(|yaml| format!("\n{}", yaml))
                    .unwrap_or(text); // Fall back to original text if YAML serialization fails
            }
            // Not JSON, return as-is
            return text;
        }

        "Operation successful".to_string()
    }

    /// Format error tool result for display
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
        let temp = tempfile::TempDir::new().unwrap();
        let context = CliToolContext::new_isolated(temp.path()).await.unwrap();

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

        // Verify extract_json_data works on non-JSON text
        let result = response_formatting::extract_json_data(&success_result);
        assert!(result.is_err(), "Non-JSON text should fail to parse");
    }

    #[tokio::test]
    async fn test_isolated_tool_execution() {
        let temp = tempfile::TempDir::new().unwrap();
        let context = CliToolContext::new_isolated(temp.path()).await.unwrap();

        let args = context.create_arguments(vec![
            ("op", json!("add task")),
            ("title", json!("Test task")),
            ("description", json!("Test context")),
        ]);

        let result = context.execute_tool("kanban", args).await;

        match result {
            Ok(_) => {}
            Err(e) => {
                let error_str = e.to_string();
                assert!(
                    !error_str.contains("rate limit"),
                    "Should not fail due to rate limiting in normal usage: {error_str}"
                );
            }
        }
    }

    /// Validates that all registered tools pass CLI validation.
    ///
    /// This test uses the same code path as the actual CLI (CliToolContext::new())
    /// to ensure the test validates the real tool registration, not a separate copy.
    /// If this test fails, it means a tool was added without proper schema validation.
    #[tokio::test]
    async fn test_all_registered_tools_pass_cli_validation() {
        use crate::dynamic_cli::CliBuilder;

        // Use the same code path as the actual CLI
        let context = CliToolContext::new()
            .await
            .expect("Failed to create CliToolContext");
        let tool_registry_arc = context.get_tool_registry_arc();

        // Create CLI builder and validate all tools
        let cli_builder = CliBuilder::new(tool_registry_arc);
        let validation_errors = cli_builder.validate_all_tools();

        // If there are validation errors, fail with detailed messages
        if !validation_errors.is_empty() {
            let error_messages: Vec<String> =
                validation_errors.iter().map(|e| e.to_string()).collect();
            panic!(
                "Tool validation failed! All registered tools must have valid schemas for CLI generation.\n\
                 Validation errors:\n  - {}",
                error_messages.join("\n  - ")
            );
        }

        // Also verify the stats show all tools are valid
        let stats = cli_builder.get_validation_stats();
        assert!(
            stats.is_all_valid(),
            "Expected all tools to be valid. Stats: {}",
            stats.summary()
        );
        assert!(
            stats.total_tools > 0,
            "Expected at least one tool to be registered"
        );
    }
}
