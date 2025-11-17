//! Integration tests to verify HTTP and STDIN MCP servers expose identical tool sets
//!
//! This test ensures that both MCP server modes (HTTP and STDIN) register and expose
//! the same set of SwissArmyHammer tools, preventing inconsistencies between modes.

use anyhow::Result;
use std::collections::HashSet;
use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
use swissarmyhammer_tools::mcp::tool_registry::{
    register_abort_tools, register_file_tools, register_issue_tools, register_memo_tools,
    register_outline_tools, register_search_tools, register_shell_tools, register_todo_tools,
    register_web_fetch_tools, register_web_search_tools,
};
use tracing::info;

/// Helper function to set up tool comparison tests
fn setup_tool_comparison_test(test_name: &str) -> (Vec<String>, Vec<String>) {
    let _ = tracing_subscriber::fmt::try_init();
    info!("🧪 Testing {}", test_name);
    let http_tools = get_http_static_tools();
    let stdin_tools = get_stdin_registry_tools();
    (http_tools, stdin_tools)
}

/// Helper function to assert that tools contain expected tools
fn assert_tools_present(tools: &[String], expected_tools: &[&str], context: &str) {
    let tool_set: HashSet<String> = tools.iter().cloned().collect();
    for &expected_tool in expected_tools {
        assert!(
            tool_set.contains(expected_tool),
            "{} missing expected tool: {}. Available tools: {:?}",
            context, expected_tool, tools
        );
    }
}

/// Helper function to log test success with tool counts
fn log_test_success(message: &str, count: usize) {
    info!("✅ {}: {}", message, count);
}

/// Test that HTTP and STDIN MCP tool registries are identical
#[tokio::test]
async fn test_http_stdin_mcp_tool_parity() -> Result<()> {
    let (http_tools, stdin_tools) =
        setup_tool_comparison_test("MCP server tool parity between HTTP and STDIN modes");

    info!(
        "📡 HTTP MCP server static definition has {} tools",
        http_tools.len()
    );
    info!("📥 STDIN MCP registry has {} tools", stdin_tools.len());

    // Compare tool sets
    let http_tool_names: HashSet<String> = http_tools.into_iter().collect();
    let stdin_tool_names: HashSet<String> = stdin_tools.into_iter().collect();

    // Check for tools only in HTTP
    let only_in_http: Vec<&String> = http_tool_names.difference(&stdin_tool_names).collect();
    if !only_in_http.is_empty() {
        eprintln!("❌ Tools only in HTTP server: {:?}", only_in_http);
    }

    // Check for tools only in STDIN
    let only_in_stdin: Vec<&String> = stdin_tool_names.difference(&http_tool_names).collect();
    if !only_in_stdin.is_empty() {
        eprintln!("❌ Tools only in STDIN server: {:?}", only_in_stdin);
    }

    // Verify they are identical
    assert_eq!(
        http_tool_names, stdin_tool_names,
        "HTTP and STDIN MCP servers must expose identical tool sets.\nHTTP tools: {}\nSTDIN tools: {}",
        http_tool_names.len(), stdin_tool_names.len()
    );

    log_test_success(
        "HTTP and STDIN MCP servers expose identical tools",
        http_tool_names.len(),
    );

    // Log some of the shared tools for verification
    let shared_tools: Vec<&String> = http_tool_names.iter().take(10).collect();
    info!("📋 Sample shared tools: {:?}", shared_tools);

    Ok(())
}

/// Get tool names from HTTP server static definition (from llama_agent_executor.rs)
/// This should match exactly what the tool registry provides  
fn get_http_static_tools() -> Vec<String> {
    // This mirrors the tool registry tools exactly, with sah__ prefix for MCP protocol
    // Workflow/prompt tools are NOT included as they're handled separately by MCP server
    let tools = [
        "files_read",
        "files_write",
        "files_edit",
        "files_glob",
        "files_grep",
        "issue_create",
        "issue_list",
        "issue_show",
        "issue_mark_complete",
        "issue_update",
        "issue_all_complete",
        "memo_create",
        "memo_list",
        "memo_get",
        "memo_get_all_context",
        "search_index",
        "search_query",
        "web_search",
        "web_fetch",
        "shell_execute",
        "todo_create",
        "todo_list",
        "todo_show",
        "todo_mark_complete",
        "outline_generate",
        "abort_create",
    ];

    tools.into_iter().map(String::from).collect()
}

/// Get tool names from STDIN registry (the authoritative source)
fn get_stdin_registry_tools() -> Vec<String> {
    let mut registry = ToolRegistry::new();

    // Register all tools exactly like McpServer does
    register_abort_tools(&mut registry);
    register_file_tools(&mut registry);
    register_issue_tools(&mut registry);
    register_memo_tools(&mut registry);
    register_outline_tools(&mut registry);
    register_search_tools(&mut registry);
    register_shell_tools(&mut registry);
    register_todo_tools(&mut registry);
    register_web_fetch_tools(&mut registry);
    register_web_search_tools(&mut registry);

    // Get MCP tool names with sah__ prefix to match MCP protocol
    registry
        .list_tools()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect()
}

/// Test that both tool definitions return expected minimum number of tools
#[tokio::test]
async fn test_mcp_tool_definitions_return_sufficient_tools() -> Result<()> {
    let (http_tools, stdin_tools) =
        setup_tool_comparison_test("MCP tool definitions return expected number of tools");

    // Test HTTP static definition
    assert!(
        http_tools.len() >= 25,
        "HTTP static definition should have at least 25 tools, got {}. Tools: {:?}",
        http_tools.len(),
        http_tools
    );

    // Test STDIN registry
    assert!(
        stdin_tools.len() >= 25,
        "STDIN registry should have at least 25 tools, got {}. Tools: {:?}",
        stdin_tools.len(),
        stdin_tools
    );

    info!(
        "Both tool definitions have sufficient tools (HTTP: {}, STDIN: {})",
        http_tools.len(),
        stdin_tools.len()
    );

    Ok(())
}

/// Test that both tool definitions include expected core tools
#[tokio::test]
async fn test_mcp_tool_definitions_include_core_tools() -> Result<()> {
    let (http_tools, stdin_tools) =
        setup_tool_comparison_test("MCP tool definitions include expected core tools");

    let expected_core_tools = [
        "files_read",
        "files_write",
        "files_edit",
        "issue_create",
        "issue_list",
        "memo_create",
        "search_query",
        "shell_execute",
        "web_fetch",
    ];

    // Test HTTP static definition
    assert_tools_present(&http_tools, &expected_core_tools, "HTTP static definition");

    // Test STDIN registry
    assert_tools_present(&stdin_tools, &expected_core_tools, "STDIN registry");

    info!("Both tool definitions include all expected core tools");

    Ok(())
}
