//! Tool command module
//!
//! Provides unified access to all registered MCP tools via `sah tool`.
//! This command serves as the single entry point for discovering and
//! executing any MCP tool, regardless of its category mapping.
//!
//! ## Usage
//!
//! ```sh
//! # List all available tools
//! sah tool list
//!
//! # Execute a specific tool
//! sah tool <tool_name> [arguments...]
//!
//! # Get help for a specific tool
//! sah tool <tool_name> --help
//! ```

use anyhow::Result;
use owo_colors::OwoColorize;
use std::sync::Arc;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolRegistry};
use tokio::sync::RwLock;

/// Display all registered MCP tools
pub async fn list_tools(registry: Arc<RwLock<ToolRegistry>>, verbose: bool) -> Result<()> {
    let registry = registry.read().await;
    let mut tools: Vec<_> = registry.iter_tools().collect();

    // Sort by name for consistent output
    tools.sort_by_key(|t| McpTool::name(*t));

    if tools.is_empty() {
        println!("No tools registered.");
        return Ok(());
    }

    println!(
        "{} registered tools:\n",
        tools.len().to_string().bold()
    );

    for tool in tools {
        let name = McpTool::name(tool);
        let hidden = if tool.hidden_from_cli() {
            " (hidden)".dimmed().to_string()
        } else {
            String::new()
        };

        if verbose {
            // Show full description
            let description = tool.description();
            let first_line = description.lines().next().unwrap_or("");
            println!("  {} {}", name.green().bold(), hidden);
            println!("    {}", first_line.dimmed());
            println!();
        } else {
            // Compact view - name and short description
            let about = tool.cli_about().unwrap_or_else(|| {
                tool.description().lines().next().unwrap_or("No description")
            });
            // Truncate long descriptions
            let about_display = if about.len() > 60 {
                format!("{}...", &about[..57])
            } else {
                about.to_string()
            };
            println!("  {:<30} {}{}", name.green(), about_display.dimmed(), hidden);
        }
    }

    if !verbose {
        println!();
        println!("Use {} for more details", "sah tool list --verbose".cyan());
        println!(
            "Use {} to execute a tool",
            "sah tool <tool_name> [args...]".cyan()
        );
    }

    Ok(())
}

/// Get a sorted list of all tool names for CLI completions
#[allow(dead_code)]
pub fn get_all_tool_names(registry: &ToolRegistry) -> Vec<String> {
    let mut names: Vec<_> = registry
        .iter_tools()
        .filter(|t| !t.hidden_from_cli())
        .map(|t| McpTool::name(t).to_string())
        .collect();
    names.sort();
    names
}
