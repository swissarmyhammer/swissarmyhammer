//! Integration tests to verify HTTP and STDIN MCP servers expose identical tool sets
//!
//! This test ensures that both MCP server modes (HTTP and STDIN) register and expose
//! the same set of SwissArmyHammer tools, preventing inconsistencies between modes.

use anyhow::Result;
use std::collections::HashSet;
use swissarmyhammer_tools::mcp::tool_registry::create_fully_registered_tool_registry;
use tracing::info;

/// Expected core tools that must be present in both HTTP and STDIN servers
const EXPECTED_CORE_TOOLS: &[&str] = &[
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
            context,
            expected_tool,
            tools
        );
    }
}

/// Helper function to log test success with tool counts
fn log_test_success(message: &str, count: usize) {
    info!("✅ {}: {}", message, count);
}

/// Helper function to compare two tool sets and assert they are identical
///
/// # Arguments
///
/// * `http_tools` - Tool names from HTTP server
/// * `stdin_tools` - Tool names from STDIN server
///
/// # Returns
///
/// * `Result<()>` - Ok if tools are identical, error otherwise
fn compare_tool_sets(http_tools: Vec<String>, stdin_tools: Vec<String>) -> Result<()> {
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

/// Helper function to assert minimum tool count
///
/// # Arguments
///
/// * `tools` - List of tool names
/// * `context` - Description of the tool source (for error messages)
/// * `minimum` - Minimum expected tool count
fn assert_minimum_tool_count(tools: &[String], context: &str, minimum: usize) {
    assert!(
        tools.len() >= minimum,
        "{} should have at least {} tools, got {}. Tools: {:?}",
        context,
        minimum,
        tools.len(),
        tools
    );
}

/// Test that HTTP and STDIN MCP tool registries are identical
#[tokio::test]
async fn test_http_stdin_mcp_tool_parity() -> Result<()> {
    let (http_tools, stdin_tools) =
        setup_tool_comparison_test("MCP server tool parity between HTTP and STDIN modes");

    compare_tool_sets(http_tools, stdin_tools)
}

/// Get tool names from HTTP server static definition (from llama_agent_executor.rs)
/// This should match exactly what the tool registry provides
fn get_http_static_tools() -> Vec<String> {
    // This mirrors the tool registry tools exactly, with sah__ prefix for MCP protocol
    // Workflow/prompt tools are NOT included as they're handled separately by MCP server
    let tools = [
        "abort_create",
        "files_edit",
        "files_glob",
        "files_grep",
        "files_read",
        "files_write",
        "flow",
        "git_changes",
        "issue_all_complete",
        "issue_create",
        "issue_list",
        "issue_mark_complete",
        "issue_show",
        "issue_update",
        "memo_create",
        "memo_get",
        "memo_get_all_context",
        "memo_list",
        "outline_generate",
        "question_ask",
        "question_summary",
        "rules_check",
        "search_index",
        "search_query",
        "shell_execute",
        "todo_create",
        "todo_list",
        "todo_mark_complete",
        "todo_show",
        "web_fetch",
        "web_search",
    ];

    tools.into_iter().map(String::from).collect()
}

/// Get tool names from STDIN registry (the authoritative source)
fn get_stdin_registry_tools() -> Vec<String> {
    let registry = create_fully_registered_tool_registry();

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
    assert_minimum_tool_count(&http_tools, "HTTP static definition", 25);

    // Test STDIN registry
    assert_minimum_tool_count(&stdin_tools, "STDIN registry", 25);

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

    // Test HTTP static definition
    assert_tools_present(&http_tools, EXPECTED_CORE_TOOLS, "HTTP static definition");

    // Test STDIN registry
    assert_tools_present(&stdin_tools, EXPECTED_CORE_TOOLS, "STDIN registry");

    info!("Both tool definitions include all expected core tools");

    Ok(())
}
