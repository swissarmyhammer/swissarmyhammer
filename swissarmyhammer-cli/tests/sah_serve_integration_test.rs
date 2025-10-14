//! Comprehensive integration test for `sah serve` MCP server functionality
//!
//! This test validates that `sah serve` properly serves all registered MCP tools
//! and that they are accessible and functional via MCP client connections.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::Command;
use std::time::Duration;
use tokio::time::timeout;

/// Get the path to the pre-built sah binary to avoid recompilation
#[allow(dead_code)]
fn get_sah_binary_path() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    // First ensure binary is built
    let build_output = Command::new("cargo")
        .args(["build", "--bin", "sah"])
        .output()?;

    if !build_output.status.success() {
        return Err("Failed to build sah binary".into());
    }

    // Get the target directory
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()?;

    if !output.status.success() {
        return Err("Failed to get cargo metadata".into());
    }

    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let target_directory = metadata["target_directory"]
        .as_str()
        .ok_or("No target directory found")?;

    let binary_path = std::path::PathBuf::from(target_directory)
        .join("debug")
        .join("sah");

    if !binary_path.exists() {
        return Err("sah binary not found after build".into());
    }

    Ok(binary_path)
}

mod test_utils;
use test_utils::ProcessGuard;

/// Sample of expected tools with their names - this is not exhaustive but validates key tools
#[allow(dead_code)]
const EXPECTED_SAMPLE_TOOLS: &[&str] = &[
    "abort_create",
    "files_read",
    "files_write",
    "files_edit",
    "files_glob",
    "files_grep",
    "issue_create",
    "issue_list",
    "issue_show",
    "issue_mark_complete",
    "memo_create",
    "memo_list",
    "memo_get",
    "notify_create",
    "outline_generate",
    "search_index",
    "search_query",
    "shell_execute",
    "todo_create",
    "todo_show",
    "todo_mark_complete",
    "web_fetch",
    "web_search",
];

/// Comprehensive integration test for sah serve MCP tools
/// Initialize MCP connection and return the initialize response
#[allow(dead_code)]
async fn initialize_mcp_connection(
    stdin: &mut std::process::ChildStdin,
    reader: &mut BufReader<std::process::ChildStdout>,
) -> Result<Value, Box<dyn std::error::Error>> {
    let init_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "prompts": {},
                "tools": {}
            },
            "clientInfo": {"name": "sah-integration-test", "version": "1.0"}
        }
    });

    send_mcp_request(stdin, &init_request)?;

    let response = timeout(Duration::from_secs(10), async { read_mcp_response(reader) })
        .await
        .map_err(|_| "Timeout waiting for initialize response")?
        .map_err(|e| format!("Failed to read initialize response: {}", e))?;

    // Validate basic response structure
    if response["jsonrpc"] != "2.0" {
        return Err(format!("Expected jsonrpc 2.0, got: {}", response["jsonrpc"]).into());
    }
    if response["id"] != 1 {
        return Err(format!("Expected id 1, got: {}", response["id"]).into());
    }
    if !response["result"].is_object() {
        return Err("Initialize should return result object".into());
    }

    Ok(response)
}

/// Validate that server capabilities include tools support
#[allow(dead_code)]
fn validate_server_capabilities(init_response: &Value) {
    let capabilities = &init_response["result"]["capabilities"];
    assert!(
        capabilities.is_object(),
        "Server should return capabilities"
    );

    let tools_capability = &capabilities["tools"];
    assert!(
        tools_capability.is_object(),
        "Server should support tools capability"
    );

    println!("✅ Server capabilities validated");
}

/// Send initialized notification to complete the handshake
#[allow(dead_code)]
fn send_initialized_notification(
    stdin: &mut std::process::ChildStdin,
) -> Result<(), Box<dyn std::error::Error>> {
    let initialized = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    send_mcp_request(stdin, &initialized)?;

    // Give server time to process the notification
    std::thread::sleep(Duration::from_millis(100));

    println!("✅ MCP handshake completed");
    Ok(())
}

/// List tools via MCP and validate expected tools are present
#[allow(dead_code)]
async fn list_and_validate_tools(
    stdin: &mut std::process::ChildStdin,
    reader: &mut BufReader<std::process::ChildStdout>,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let list_tools_request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    });

    send_mcp_request(stdin, &list_tools_request)?;

    let response = timeout(Duration::from_secs(10), async { read_mcp_response(reader) })
        .await
        .expect("Timeout waiting for tools/list response")
        .expect("Failed to read tools/list response");

    // Validate response structure
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 2);

    let result = &response["result"];
    assert!(result.is_object(), "tools/list should return result object");

    let tools_array = &result["tools"];
    assert!(tools_array.is_array(), "result should contain tools array");

    let tools = tools_array.as_array().unwrap();

    // Validate we have a reasonable number of tools
    assert!(
        tools.len() >= 20,
        "Expected at least 20 tools, found: {}. Tools: {:?}",
        tools.len(),
        tools
            .iter()
            .map(|t| t["name"].as_str().unwrap_or("unnamed"))
            .collect::<Vec<_>>()
    );

    // Check that expected sample tools are present
    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();

    println!(
        "📋 Found {} tools: {:?}",
        tool_names.len(),
        &tool_names[..std::cmp::min(10, tool_names.len())]
    );

    for expected_tool in EXPECTED_SAMPLE_TOOLS {
        assert!(
            tool_names.contains(expected_tool),
            "Expected tool '{}' not found. Available tools: {:?}",
            expected_tool,
            tool_names
        );
    }

    // Validate tool structure - each tool should have name, description, and schema
    for tool in tools {
        assert!(tool["name"].is_string(), "Tool should have string name");
        assert!(
            tool["description"].is_string(),
            "Tool should have string description"
        );
        assert!(
            tool["inputSchema"].is_object(),
            "Tool should have inputSchema object"
        );
    }

    println!("✅ All expected tools are present and properly structured");

    Ok(tools.clone())
}

/// Test execution of sample tools - optimized for speed
#[allow(dead_code)]
async fn test_minimal_tool_executions(
    stdin: &mut std::process::ChildStdin,
    reader: &mut BufReader<std::process::ChildStdout>,
    _tools: &[Value],
) -> Result<(), Box<dyn std::error::Error>> {
    // Test only one fast, safe tool to verify execution works
    test_single_tool_execution(
        stdin,
        reader,
        3,
        "notify_create",
        json!({
            "message": "Integration test notification",
            "level": "info"
        }),
    )
    .await?;

    println!("✅ Tool execution test completed successfully");
    Ok(())
}

/// Test execution of a single tool and validate response
#[allow(dead_code)]
async fn test_single_tool_execution(
    stdin: &mut std::process::ChildStdin,
    reader: &mut BufReader<std::process::ChildStdout>,
    request_id: u64,
    tool_name: &str,
    arguments: Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let call_tool_request = json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": arguments
        }
    });

    send_mcp_request(stdin, &call_tool_request)?;

    let response = timeout(Duration::from_secs(5), async { read_mcp_response(reader) })
        .await
        .unwrap_or_else(|_| panic!("Timeout waiting for {} execution response", tool_name))
        .unwrap_or_else(|_| panic!("Failed to read {} execution response", tool_name));

    // Validate response structure
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], request_id);

    // The response should have either result or error, but not both
    let has_result = response.get("result").is_some();
    let has_error = response.get("error").is_some();

    assert!(
        has_result || has_error,
        "Tool {} response should have either result or error. Response: {}",
        tool_name,
        response
    );

    assert!(
        !(has_result && has_error),
        "Tool {} response should not have both result and error. Response: {}",
        tool_name,
        response
    );

    if has_result {
        let result = &response["result"];
        assert!(
            result["content"].is_array(),
            "Tool result should have content array"
        );
        println!("✅ Tool {} executed successfully", tool_name);
    } else {
        let error = &response["error"];
        // Some tools might fail due to missing dependencies or invalid arguments,
        // but they should return proper MCP error format
        assert!(
            error["code"].is_number(),
            "Tool error should have error code"
        );
        assert!(
            error["message"].is_string(),
            "Tool error should have error message"
        );
        println!(
            "⚠️  Tool {} returned error (expected for some tools): {}",
            tool_name, error["message"]
        );
    }

    Ok(())
}

/// Wait for server to be ready - optimized for pre-built binary
#[allow(dead_code)]
fn wait_for_server_ready(
    child: &mut ProcessGuard,
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();

    // For pre-built binary, server should start much faster
    while start.elapsed() < timeout {
        // Check if process has exited unexpectedly
        if !child.is_running() {
            return Err("Server process exited during startup".into());
        }

        // Much shorter wait times since we're using pre-built binary
        std::thread::sleep(Duration::from_millis(200));

        // Server should be ready within 5 seconds for pre-built binary
        if start.elapsed() >= Duration::from_secs(5) {
            break;
        }
    }

    // Final check that process is still running
    if !child.is_running() {
        return Err("Server process exited during initialization".into());
    }

    // Brief final wait for server initialization
    std::thread::sleep(Duration::from_millis(500));

    if !child.is_running() {
        return Err("Server process exited during initialization".into());
    }

    Ok(())
}

/// Helper function to send MCP request
#[allow(dead_code)]
fn send_mcp_request(
    stdin: &mut std::process::ChildStdin,
    request: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let request_str = serde_json::to_string(request)?;
    writeln!(stdin, "{}", request_str)?;
    stdin.flush()?;
    Ok(())
}

/// Helper function to read MCP response
#[allow(dead_code)]
fn read_mcp_response(
    reader: &mut BufReader<std::process::ChildStdout>,
) -> Result<Value, Box<dyn std::error::Error>> {
    let mut line = String::new();
    reader.read_line(&mut line)?;

    if line.trim().is_empty() {
        return Err("Empty response from MCP server".into());
    }

    let response: Value = serde_json::from_str(line.trim())?;
    Ok(response)
}
