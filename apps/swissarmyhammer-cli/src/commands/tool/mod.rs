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

/// Maximum description length displayed inline in compact `sah tool list`
/// output before the description is truncated. Descriptions longer than this
/// are clipped to [`DESCRIPTION_TRUNCATE_POINT`] characters with a trailing
/// `...` marker.
const DESCRIPTION_MAX_LENGTH: usize = 60;

/// Number of characters retained from a long description before appending
/// `...`. Three less than [`DESCRIPTION_MAX_LENGTH`] so the visual width of
/// the truncated form (text + ellipsis) stays at the cap.
const DESCRIPTION_TRUNCATE_POINT: usize = DESCRIPTION_MAX_LENGTH - 3;

/// Padded width of the tool-name column in compact `sah tool list` output.
/// Sized so typical tool names left-align cleanly against the description
/// column without wrapping.
const TOOL_NAME_COLUMN_WIDTH: usize = 30;

/// Display all registered MCP tools
pub async fn list_tools(registry: Arc<RwLock<ToolRegistry>>, verbose: bool) -> Result<()> {
    let registry = registry.read().await;
    let mut tools: Vec<_> = registry.iter_tools().collect();
    tools.sort_by_key(|t| McpTool::name(*t));

    if tools.is_empty() {
        println!("No tools registered.");
        return Ok(());
    }

    println!("{} registered tools:\n", tools.len().to_string().bold());

    for tool in tools {
        if verbose {
            print_verbose_tool_entry(tool);
        } else {
            print_compact_tool_entry(tool);
        }
    }

    if !verbose {
        print_list_footer();
    }

    Ok(())
}

/// Format the optional `(hidden)` suffix shown after a tool's name.
fn hidden_suffix(tool: &dyn McpTool) -> String {
    if tool.hidden_from_cli() {
        " (hidden)".dimmed().to_string()
    } else {
        String::new()
    }
}

/// Print one tool's entry in verbose mode: name + first description line on
/// the next indented line, followed by a blank separator.
fn print_verbose_tool_entry(tool: &dyn McpTool) {
    let name = McpTool::name(tool);
    let description = tool.description();
    let first_line = description.lines().next().unwrap_or("");
    println!("  {} {}", name.green().bold(), hidden_suffix(tool));
    println!("    {}", first_line.dimmed());
    println!();
}

/// Print one tool's entry in compact mode: padded name column followed by
/// a truncated single-line description.
fn print_compact_tool_entry(tool: &dyn McpTool) {
    let name = McpTool::name(tool);
    let about = tool
        .cli_about()
        .unwrap_or_else(|| tool.description().lines().next().unwrap_or("No description"));
    let about_display = if about.len() > DESCRIPTION_MAX_LENGTH {
        format!("{}...", &about[..DESCRIPTION_TRUNCATE_POINT])
    } else {
        about.to_string()
    };
    println!(
        "  {:<width$} {}{}",
        name.green(),
        about_display.dimmed(),
        hidden_suffix(tool),
        width = TOOL_NAME_COLUMN_WIDTH,
    );
}

/// Print the closing usage hints shown after a compact `sah tool list`.
fn print_list_footer() {
    println!();
    println!("Use {} for more details", "sah tool list --verbose".cyan());
    println!(
        "Use {} to execute a tool",
        "sah tool <tool_name> [args...]".cyan()
    );
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
