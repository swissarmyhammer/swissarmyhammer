use anyhow::{Context, Result};
use clap::ArgMatches;
use std::sync::Arc;
use swissarmyhammer_tools::mcp::tool_registry::{ToolContext, ToolRegistry};
use tracing::{debug, error, info};

use crate::cli_builder::{CliBuilder, DynamicCommandInfo};
use crate::response_formatting::ResponseFormatter;
use crate::schema_conversion::SchemaConverter;

/// Handler for executing dynamic MCP-based CLI commands
///
/// The DynamicCommandExecutor bridges the gap between parsed CLI arguments
/// and MCP tool execution, handling argument conversion, tool invocation,
/// and response formatting.
pub struct DynamicCommandExecutor {
    tool_registry: Arc<ToolRegistry>,
    tool_context: Arc<ToolContext>,
}

impl DynamicCommandExecutor {
    /// Create a new dynamic command executor
    pub fn new(tool_registry: Arc<ToolRegistry>, tool_context: Arc<ToolContext>) -> Self {
        Self {
            tool_registry,
            tool_context,
        }
    }

    /// Execute a dynamic MCP command with comprehensive logging
    pub async fn execute_command(
        &self,
        command_info: DynamicCommandInfo,
        matches: &ArgMatches,
    ) -> Result<()> {
        info!(
            "Executing dynamic command: {} ({})",
            command_info.tool_name, command_info.mcp_tool_name
        );

        debug!("Command info: {:?}", command_info);

        match self.execute_command_internal(command_info, matches).await {
            Ok(()) => {
                debug!("Dynamic command completed successfully");
                Ok(())
            }
            Err(e) => {
                error!("Dynamic command failed: {}", e);
                Err(e)
            }
        }
    }

    /// Internal command execution logic
    async fn execute_command_internal(
        &self,
        command_info: DynamicCommandInfo,
        matches: &ArgMatches,
    ) -> Result<()> {
        // Get the MCP tool from registry
        let tool = self
            .tool_registry
            .get_tool(&command_info.mcp_tool_name)
            .with_context(|| format!("Tool not found: {}", command_info.mcp_tool_name))?;

        debug!("Found MCP tool: {}", tool.name());

        // Extract the appropriate ArgMatches for the tool
        let cli_builder = CliBuilder::new(self.tool_registry.clone());
        let tool_matches = cli_builder
            .get_tool_matches(matches, &command_info)
            .with_context(|| "Failed to extract tool matches")?;

        debug!("Extracted tool matches for argument conversion");

        // Convert clap matches to JSON arguments using the tool's schema
        let schema = tool.schema();
        let arguments = SchemaConverter::matches_to_json_args(tool_matches, &schema)
            .with_context(|| "Failed to convert arguments to JSON")?;

        debug!("Converted arguments to JSON: {:?}", arguments);

        // Execute the MCP tool - preserve original error messages
        let result = match tool.execute(arguments, &self.tool_context).await {
            Ok(result) => result,
            Err(e) => {
                // Return an error result rather than failing immediately to preserve the MCP error message
                return Err(anyhow::Error::msg(e.to_string()));
            }
        };

        debug!("Tool execution completed, formatting response");

        // Format and display the result
        self.display_result(&result)?;

        Ok(())
    }

    /// Format and display MCP tool result
    fn display_result(&self, result: &rmcp::model::CallToolResult) -> Result<()> {
        let formatted_output = ResponseFormatter::format_response(result)
            .with_context(|| "Failed to format tool response")?;

        // Handle different result types appropriately  
        if result.is_error.unwrap_or(false) {
            eprintln!("{}", formatted_output);
            std::process::exit(1);
        } else {
            println!("{}", formatted_output);
        }

        Ok(())
    }
}

/// Check if a command is a dynamic (MCP-based) command
pub fn is_dynamic_command(matches: &ArgMatches, builder: &CliBuilder) -> bool {
    builder.extract_command_info(matches).is_some()
}



#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::{model::CallToolResult, Error as McpError};
    use serde_json::json;
    use swissarmyhammer_tools::mcp::tool_registry::{BaseToolImpl, McpTool};

    /// Mock tool for testing
    #[derive(Default)]
    struct MockTool;

    #[async_trait::async_trait]
    impl McpTool for MockTool {
        fn name(&self) -> &'static str {
            "test_tool"
        }

        fn description(&self) -> &'static str {
            "A test tool"
        }

        fn schema(&self) -> serde_json::Value {
            json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Test parameter"
                    }
                },
                "required": []
            })
        }

        async fn execute(
            &self,
            _arguments: serde_json::Map<String, serde_json::Value>,
            _context: &ToolContext,
        ) -> std::result::Result<CallToolResult, McpError> {
            Ok(BaseToolImpl::create_success_response("Test executed"))
        }
    }



    #[tokio::test]
    async fn test_command_info_extraction() {
        let mut registry = ToolRegistry::new();
        registry.register(MockTool);

        let registry = Arc::new(registry);
        let builder = CliBuilder::new(registry.clone());
        let cli = builder.build_cli().unwrap();

        // This is a basic structure test - actual command parsing would require
        // the tool to be properly registered with categories, etc.
        assert!(cli.find_subcommand("serve").is_some());
    }

    #[test]
    fn test_dynamic_command_info_structure() {
        let info = DynamicCommandInfo {
            category: Some("issue".to_string()),
            tool_name: "create".to_string(),
            mcp_tool_name: "issue_create".to_string(),
        };

        assert_eq!(info.category.as_deref(), Some("issue"));
        assert_eq!(info.tool_name, "create");
        assert_eq!(info.mcp_tool_name, "issue_create");
    }
}