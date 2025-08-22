//! Dynamic Command Execution Module
//!
//! Provides the infrastructure for executing dynamically generated CLI commands
//! that correspond to MCP tools. This module bridges the gap between Clap's
//! parsed arguments and MCP tool execution.
//!
//! The module handles:
//! - Tool lookup from the registry
//! - Argument conversion from Clap to JSON
//! - MCP tool execution
//! - Result formatting and display

use crate::schema_conversion::{ConversionError, SchemaConverter};
use anyhow::{anyhow, Context, Result};
use clap::ArgMatches;
use rmcp::model::{CallToolResult, RawContent};
use std::sync::Arc;
use swissarmyhammer_tools::mcp::tool_registry::{ToolContext, ToolRegistry};

/// Handle execution of a dynamic command corresponding to an MCP tool
///
/// This function orchestrates the complete execution flow:
/// 1. Look up the tool in the registry
/// 2. Convert Clap arguments to JSON format
/// 3. Execute the tool via MCP
/// 4. Format and display the result
///
/// # Arguments
/// * `category` - CLI category (e.g., "memo", "issue")
/// * `tool_name` - CLI tool name (e.g., "create", "list")
/// * `matches` - Parsed command line arguments from Clap
/// * `tool_registry` - Registry containing all available MCP tools
/// * `context` - Shared context for tool execution
///
/// # Returns
/// `Ok(())` on successful execution, `Err(anyhow::Error)` on failure
///
/// # Errors
/// Returns errors for:
/// - Tool not found in registry
/// - Argument conversion failures
/// - MCP execution errors
/// - Result display errors
pub async fn handle_dynamic_command(
    category: &str,
    tool_name: &str,
    matches: &ArgMatches,
    tool_registry: Arc<ToolRegistry>,
    context: Arc<ToolContext>,
) -> Result<()> {
    // Construct tool name using MCP naming convention: {category}_{action}
    let mcp_tool_name = format!("{}_{}", category, tool_name);

    // Look up the tool in the registry
    let tool = tool_registry.get_tool(&mcp_tool_name).ok_or_else(|| {
        let available_tools: Vec<String> = tool_registry
            .get_tools_for_category(category)
            .iter()
            .map(|t| t.cli_name().to_string())
            .collect();
        anyhow!(
            "Tool '{}' not found in category '{}'. Available tools in this category: [{}]",
            tool_name,
            category,
            available_tools.join(", ")
        )
    })?;

    // Get the tool's schema for argument conversion
    let schema = tool.schema();

    // Convert Clap matches to JSON arguments
    let arguments = SchemaConverter::matches_to_json_args(matches, &schema)
        .map_err(|e| anyhow!(
            "Argument conversion failed for tool '{}' (category: {}): {}",
            tool_name,
            category,
            e
        ))
        .with_context(|| {
            let required_fields = schema
                .get("required")
                .and_then(|r| r.as_array())
                .map(|arr| arr.len())
                .unwrap_or(0);
            let total_properties = schema
                .get("properties")
                .and_then(|p| p.as_object())
                .map(|obj| obj.len())
                .unwrap_or(0);
            format!(
                "Converting arguments for tool '{}' in category '{}' (schema: {} properties, {} required)",
                tool_name,
                category,
                total_properties,
                required_fields
            )
        })?;

    let arg_count = arguments.len();

    tracing::debug!(
        "Executing tool {} with arguments: {:?}",
        mcp_tool_name,
        arguments
    );

    // Execute the tool via MCP
    let result = tool
        .execute(arguments, &context)
        .await
        .map_err(|e| {
            anyhow!(
                "Tool execution failed for '{}' in category '{}': {}",
                tool_name,
                category,
                e
            )
        })
        .with_context(|| {
            format!(
                "Executing MCP tool '{}' (full name: {}) with {} argument(s)",
                tool_name, mcp_tool_name, arg_count
            )
        })?;

    // Format and display the result
    display_mcp_result(result).with_context(|| {
        format!(
            "Displaying result for tool '{}' in category '{}'",
            tool_name, category
        )
    })?;

    Ok(())
}

/// Display the result of an MCP tool execution
///
/// Formats and prints the result to stdout, handling different content types
/// and error conditions appropriately.
///
/// # Arguments
/// * `result` - The result from MCP tool execution
///
/// # Returns
/// `Ok(())` on successful display, `Err(anyhow::Error)` on formatting errors
///
/// # Format
/// The display format matches the existing CLI tool patterns:
/// - Success results show content directly
/// - Error results show error information
/// - Structured content is formatted appropriately
pub fn display_mcp_result(result: CallToolResult) -> Result<()> {
    // Check if this is an error result
    if result.is_error == Some(true) {
        eprintln!("Error executing command:");
        for content in &result.content {
            match &**content {
                RawContent::Text(text_content) => {
                    eprintln!("{}", text_content.text);
                }
                RawContent::Image(_) => {
                    eprintln!("(Error result contains image content)");
                }
                RawContent::Resource(_) => {
                    eprintln!("(Error result contains resource content)");
                }
                RawContent::Audio(_) => {
                    eprintln!("(Error result contains audio content)");
                }
            }
        }
        return Err(anyhow!("Command execution failed"));
    }

    // Display successful result content
    for content in &result.content {
        match &**content {
            RawContent::Text(text_content) => {
                println!("{}", text_content.text);
            }
            RawContent::Image(_) => {
                println!("(Result contains image content - cannot display in terminal)");
            }
            RawContent::Resource(_) => {
                println!("(Result contains resource content - cannot display in terminal)");
            }
            RawContent::Audio(_) => {
                println!("(Result contains audio content - cannot display in terminal)");
            }
        }
    }

    Ok(())
}

/// Convert a ConversionError to a user-friendly error message
///
/// Provides specific guidance based on the type of conversion error that occurred.
///
/// # Arguments
/// * `error` - The conversion error to format
/// * `tool_name` - Name of the tool being executed (for context)
///
/// # Returns
/// Formatted error message suitable for display to users
pub fn format_conversion_error(error: ConversionError, tool_name: &str) -> String {
    match error {
        ConversionError::MissingRequired(field) => {
            format!(
                "Missing required argument '--{}' for tool '{}'.\nUse '--help' to see all required arguments.",
                field, tool_name
            )
        }
        ConversionError::InvalidType {
            name,
            expected,
            actual,
        } => {
            format!(
                "Invalid type for argument '--{}' in tool '{}': expected {}, got {}.\nPlease check the argument format.",
                name, tool_name, expected, actual
            )
        }
        ConversionError::ParseError {
            field,
            data_type,
            message,
        } => {
            format!(
                "Failed to parse '--{}' as {} for tool '{}': {}.\nPlease check the argument value format.",
                field, data_type, tool_name, message
            )
        }
        ConversionError::SchemaValidation(msg) => {
            format!(
                "Schema validation failed for tool '{}': {}.\nThis may indicate an internal tool configuration error.",
                tool_name, msg
            )
        }
        ConversionError::UnsupportedSchemaType { schema_type } => {
            format!(
                "Tool '{}' uses unsupported argument type '{}'. This tool may not be compatible with CLI execution.",
                tool_name, schema_type
            )
        }
    }
}
