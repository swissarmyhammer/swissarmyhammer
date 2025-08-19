# Implement Dynamic Command Execution Handler

Refer to /Users/wballard/github/sah-cli/ideas/cli.md

## Objective

Create the command execution infrastructure that routes dynamic CLI commands to their corresponding MCP tools and handles response formatting.

## Implementation Tasks

### 1. Create Command Execution Handler

Create `swissarmyhammer-cli/src/dynamic_execution.rs`:

```rust
use clap::ArgMatches;
use std::sync::Arc;
use swissarmyhammer_tools::mcp::tool_registry::{ToolRegistry, ToolContext};
use crate::cli_builder::{CliBuilder, DynamicCommandInfo};
use crate::schema_conversion::SchemaConverter;
use anyhow::{Result, Context};

pub struct DynamicCommandExecutor {
    tool_registry: Arc<ToolRegistry>,
    tool_context: Arc<ToolContext>,
}

impl DynamicCommandExecutor {
    pub fn new(tool_registry: Arc<ToolRegistry>, tool_context: Arc<ToolContext>) -> Self {
        Self {
            tool_registry,
            tool_context,
        }
    }
    
    /// Execute a dynamic MCP command
    pub async fn execute_command(
        &self,
        command_info: DynamicCommandInfo,
        matches: &ArgMatches,
    ) -> Result<()> {
        // Get the MCP tool
        let tool = self.tool_registry
            .get_tool(&command_info.mcp_tool_name)
            .with_context(|| format!("Tool not found: {}", command_info.mcp_tool_name))?;
            
        // Extract subcommand matches
        let tool_matches = self.extract_tool_matches(matches, &command_info)?;
        
        // Convert clap matches to JSON arguments
        let schema = tool.schema();
        let arguments = SchemaConverter::matches_to_json_args(tool_matches, &schema)
            .with_context(|| "Failed to convert arguments to JSON")?;
            
        // Execute the MCP tool
        let result = tool.execute(arguments, &self.tool_context).await
            .with_context(|| format!("Tool execution failed: {}", command_info.mcp_tool_name))?;
            
        // Format and display the result
        self.display_result(&result)?;
        
        Ok(())
    }
    
    /// Extract the ArgMatches for the specific tool
    fn extract_tool_matches(
        &self,
        matches: &ArgMatches,
        command_info: &DynamicCommandInfo,
    ) -> Result<&ArgMatches> {
        if let Some(category) = &command_info.category {
            // Handle categorized tools: category -> tool
            let category_matches = matches.subcommand_matches(category)
                .with_context(|| format!("Category subcommand not found: {}", category))?;
                
            let tool_matches = category_matches.subcommand_matches(&command_info.tool_name)
                .with_context(|| format!("Tool subcommand not found: {}", command_info.tool_name))?;
                
            Ok(tool_matches)
        } else {
            // Handle root-level tools
            let tool_matches = matches.subcommand_matches(&command_info.tool_name)
                .with_context(|| format!("Root tool not found: {}", command_info.tool_name))?;
                
            Ok(tool_matches)
        }
    }
    
    /// Format and display MCP tool result
    fn display_result(&self, result: &rmcp::model::CallToolResult) -> Result<()> {
        match result {
            rmcp::model::CallToolResult::Success { content, .. } => {
                self.display_content(content)?;
            },
            rmcp::model::CallToolResult::Error { error, .. } => {
                eprintln!("Tool execution error: {}", error);
                std::process::exit(1);
            }
        }
        
        Ok(())
    }
    
    /// Display content from MCP tool response
    fn display_content(&self, content: &[rmcp::model::RawContent]) -> Result<()> {
        for item in content {
            match item {
                rmcp::model::RawContent::Text(text_content) => {
                    println!("{}", text_content.text);
                },
                rmcp::model::RawContent::Image(_) => {
                    println!("[Image content - not displayable in CLI]");
                },
                rmcp::model::RawContent::Resource(_) => {
                    println!("[Resource content]");
                }
            }
        }
        
        Ok(())
    }
}

/// Check if a command is a dynamic (MCP-based) command
pub fn is_dynamic_command(matches: &ArgMatches, builder: &CliBuilder) -> bool {
    builder.extract_command_info(matches).is_some()
}

/// Check if a command is a static (CLI-only) command
pub fn is_static_command(matches: &ArgMatches) -> bool {
    if let Some((command, _)) = matches.subcommand() {
        matches!(command, "serve" | "doctor" | "prompt" | "flow" | "completion" | "validate" | "plan" | "implement")
    } else {
        false
    }
}
```

### 2. Create Response Formatting Module

Create `swissarmyhammer-cli/src/response_formatting.rs`:

```rust
use rmcp::model::{CallToolResult, RawContent, RawTextContent};
use anyhow::Result;
use serde_json::Value;

pub struct ResponseFormatter;

impl ResponseFormatter {
    /// Format MCP tool response for CLI display
    pub fn format_response(result: &CallToolResult) -> Result<String> {
        match result {
            CallToolResult::Success { content, .. } => {
                Self::format_success_content(content)
            },
            CallToolResult::Error { error, .. } => {
                Ok(format!("Error: {}", error))
            }
        }
    }
    
    /// Format successful response content
    fn format_success_content(content: &[RawContent]) -> Result<String> {
        let mut output = String::new();
        
        for item in content {
            match item {
                RawContent::Text(text_content) => {
                    output.push_str(&text_content.text);
                    output.push('\n');
                },
                RawContent::Image(_) => {
                    output.push_str("[Image content - not displayable in CLI]\n");
                },
                RawContent::Resource(_) => {
                    output.push_str("[Resource content]\n");
                }
            }
        }
        
        // Remove trailing newline
        if output.ends_with('\n') {
            output.pop();
        }
        
        Ok(output)
    }
    
    /// Format structured JSON response in a readable way
    pub fn format_json_response(json: &Value, format: Option<&str>) -> Result<String> {
        match format {
            Some("json") => Ok(serde_json::to_string_pretty(json)?),
            Some("yaml") => Ok(serde_yaml::to_string(json)?),
            _ => {
                // Default table-like formatting
                Self::format_table_response(json)
            }
        }
    }
    
    /// Format JSON as a table when possible
    fn format_table_response(json: &Value) -> Result<String> {
        match json {
            Value::Object(map) => {
                let mut output = String::new();
                for (key, value) in map {
                    output.push_str(&format!("{}: ", key));
                    match value {
                        Value::String(s) => output.push_str(s),
                        Value::Number(n) => output.push_str(&n.to_string()),
                        Value::Bool(b) => output.push_str(&b.to_string()),
                        other => output.push_str(&serde_json::to_string(other)?),
                    }
                    output.push('\n');
                }
                Ok(output)
            },
            Value::Array(arr) => {
                let mut output = String::new();
                for (i, item) in arr.iter().enumerate() {
                    output.push_str(&format!("{}. {}\n", i + 1, 
                        Self::format_table_response(item)?));
                }
                Ok(output)
            },
            other => Ok(serde_json::to_string_pretty(other)?),
        }
    }
}
```

### 3. Integrate with Main CLI Handler

Update `swissarmyhammer-cli/src/main.rs` or create the integration:

```rust
use crate::dynamic_execution::{DynamicCommandExecutor, is_dynamic_command, is_static_command};
use crate::cli_builder::CliBuilder;

pub async fn handle_cli_command(matches: ArgMatches) -> Result<()> {
    // Initialize MCP infrastructure
    let tool_registry = Arc::new(create_tool_registry().await?);
    let tool_context = Arc::new(create_tool_context().await?);
    
    // Create CLI builder
    let cli_builder = CliBuilder::new(tool_registry.clone());
    
    // Route command based on type
    if is_static_command(&matches) {
        handle_static_command(&matches).await?;
    } else if is_dynamic_command(&matches, &cli_builder) {
        let command_info = cli_builder.extract_command_info(&matches)
            .ok_or_else(|| anyhow::anyhow!("Failed to extract command info"))?;
            
        let executor = DynamicCommandExecutor::new(tool_registry, tool_context);
        executor.execute_command(command_info, &matches).await?;
    } else {
        anyhow::bail!("Unknown command");
    }
    
    Ok(())
}

async fn handle_static_command(matches: &ArgMatches) -> Result<()> {
    match matches.subcommand() {
        Some(("serve", _)) => {
            // Existing serve implementation
            crate::serve::run_serve().await
        },
        Some(("doctor", sub_matches)) => {
            // Existing doctor implementation
            crate::doctor::run_doctor(sub_matches).await
        },
        Some(("prompt", sub_matches)) => {
            // Existing prompt implementation
            crate::prompt::run_prompt(sub_matches).await
        },
        // ... other static commands
        _ => anyhow::bail!("Unknown static command"),
    }
}
```

### 4. Add Error Handling and Logging

```rust
use tracing::{info, error, debug};

impl DynamicCommandExecutor {
    pub async fn execute_command_with_logging(
        &self,
        command_info: DynamicCommandInfo,
        matches: &ArgMatches,
    ) -> Result<()> {
        info!("Executing dynamic command: {} ({})", 
            command_info.tool_name, command_info.mcp_tool_name);
            
        debug!("Command info: {:?}", command_info);
        
        match self.execute_command(command_info, matches).await {
            Ok(()) => {
                debug!("Dynamic command completed successfully");
                Ok(())
            },
            Err(e) => {
                error!("Dynamic command failed: {}", e);
                Err(e)
            }
        }
    }
}
```

### 5. Create Integration Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_dynamic_command_execution() {
        // This would need a test registry and context
        // Test the full flow of dynamic command execution
    }
    
    #[test]
    fn test_static_command_detection() {
        // Test that static commands are properly identified
    }
    
    #[test] 
    fn test_dynamic_command_detection() {
        // Test that dynamic commands are properly identified
    }
}
```

## Success Criteria

- [ ] DynamicCommandExecutor routes CLI commands to MCP tools
- [ ] Schema-based argument conversion works correctly
- [ ] MCP tool responses formatted appropriately for CLI display
- [ ] Static vs dynamic command detection works reliably
- [ ] Error handling provides clear user feedback
- [ ] Integration with existing CLI infrastructure
- [ ] Logging and debugging support for troubleshooting
- [ ] Response formatting handles different content types
- [ ] Tests validate the command execution pipeline

## Architecture Notes

- Bridges CLI argument parsing with MCP tool execution
- Maintains clear separation between static and dynamic commands
- Uses schema conversion for type-safe argument handling
- Provides foundation for replacing static command enums
- Enables unified execution path for all MCP-based commands