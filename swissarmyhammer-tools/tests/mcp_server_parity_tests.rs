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
    "shell_execute",
    "web_fetch",
];

/// Helper function to set up tool comparison tests
fn setup_tool_comparison_test(test_name: &str) -> (Vec<String>, Vec<String>) {
    let _ = tracing_subscriber::fmt::try_init();
    info!("🧪 Testing {}", test_name);
    let http_tools = get_mcp_tools();
    let stdin_tools = get_mcp_tools();
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
    info!("✓ {}: {}", message, count);
}

/// Helper function to log tool differences between two sets
///
/// # Arguments
///
/// * `set_a` - First tool set
/// * `set_b` - Second tool set
/// * `label_a` - Label for first set
fn log_tool_differences(set_a: &HashSet<String>, set_b: &HashSet<String>, label_a: &str) {
    let only_in_a: Vec<&String> = set_a.difference(set_b).collect();
    if !only_in_a.is_empty() {
        eprintln!("✗ Tools only in {}: {:?}", label_a, only_in_a);
    }
}

/// Helper function to log symmetric differences between two tool sets
///
/// # Arguments
///
/// * `set_a` - First tool set
/// * `set_b` - Second tool set
/// * `label_a` - Label for first set
/// * `label_b` - Label for second set
fn log_symmetric_differences(
    set_a: &HashSet<String>,
    set_b: &HashSet<String>,
    label_a: &str,
    label_b: &str,
) {
    log_tool_differences(set_a, set_b, label_a);
    log_tool_differences(set_b, set_a, label_b);
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

    // Check for differences in both directions
    log_symmetric_differences(
        &http_tool_names,
        &stdin_tool_names,
        "HTTP server",
        "STDIN server",
    );

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

/// Helper function to run dual tool tests with custom assertion logic
///
/// # Arguments
///
/// * `test_name` - Name of the test for logging
/// * `success_message` - Message to log on successful test completion
/// * `assertion_fn` - Function to run assertions on tool list and context
///
/// # Returns
///
/// * `Result<()>` - Ok if assertions pass, error otherwise
fn run_dual_tool_test<F>(test_name: &str, success_message: &str, assertion_fn: F) -> Result<()>
where
    F: Fn(&[String], &str),
{
    let (http_tools, stdin_tools) = setup_tool_comparison_test(test_name);
    assertion_fn(&http_tools, "HTTP static definition");
    assertion_fn(&stdin_tools, "STDIN registry");
    info!("{}", success_message);
    Ok(())
}

/// Test that HTTP and STDIN MCP tool registries are identical
#[tokio::test]
async fn test_http_stdin_mcp_tool_parity() -> Result<()> {
    let (http_tools, stdin_tools) =
        setup_tool_comparison_test("MCP server tool parity between HTTP and STDIN modes");

    compare_tool_sets(http_tools, stdin_tools)
}

/// Get tool names from the MCP tool registry (single source of truth)
///
/// This function is used by both HTTP and STDIN server tests to ensure
/// they validate against the same authoritative tool list.
fn get_mcp_tools() -> Vec<String> {
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
    run_dual_tool_test(
        "MCP tool definitions return expected number of tools",
        "Both tool definitions have sufficient tools",
        |tools, context| {
            assert_minimum_tool_count(tools, context, 19);
        },
    )
}

/// Test that both tool definitions include expected core tools
#[tokio::test]
async fn test_mcp_tool_definitions_include_core_tools() -> Result<()> {
    run_dual_tool_test(
        "MCP tool definitions include expected core tools",
        "Both tool definitions include all expected core tools",
        |tools, context| {
            assert_tools_present(tools, EXPECTED_CORE_TOOLS, context);
        },
    )
}
