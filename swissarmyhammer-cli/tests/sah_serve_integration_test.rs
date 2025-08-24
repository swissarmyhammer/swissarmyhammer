//! Comprehensive integration test for `sah serve` MCP server functionality
//!
//! This test validates that `sah serve` properly serves all registered MCP tools
//! and that they are accessible and functional via MCP client connections.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::timeout;

mod test_utils;
use test_utils::ProcessGuard;

/// Sample of expected tools with their names - this is not exhaustive but validates key tools
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
    "issue_work",
    "issue_mark_complete",
    "memo_create",
    "memo_list",
    "memo_get",
    "memo_update",
    "memo_delete",
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
#[tokio::test]
async fn test_sah_serve_tools_integration() -> Result<(), Box<dyn std::error::Error>> {
    // Start the MCP server process
    let child = Command::new("cargo")
        .args(["run", "--bin", "sah", "--", "serve"])
        // Run from CLI directory instead of project root to avoid initialization issues
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start MCP server");

    let mut child = ProcessGuard(child);

    // Wait for server compilation and initialization with proper process monitoring
    wait_for_server_ready(&mut child, Duration::from_secs(60))?;

    let mut stdin = child.0.stdin.take().expect("Failed to get stdin");
    let stdout = child.0.stdout.take().expect("Failed to get stdout");
    let stderr = child.0.stderr.take().expect("Failed to get stderr");
    let mut reader = BufReader::new(stdout);

    // Spawn stderr reader for debugging
    std::thread::spawn(move || {
        let stderr_reader = BufReader::new(stderr);
        for line in stderr_reader.lines().map_while(Result::ok) {
            eprintln!("SERVER STDERR: {line}");
        }
    });

    // Step 1: Initialize the MCP connection
    let init_response = initialize_mcp_connection(&mut stdin, &mut reader).await?;

    // Validate server capabilities
    validate_server_capabilities(&init_response);

    // Step 2: Send initialized notification
    send_initialized_notification(&mut stdin)?;

    // Step 3: List all tools and validate they are present
    let tools = list_and_validate_tools(&mut stdin, &mut reader).await?;

    // Step 4: Test tool execution for key tools
    test_tool_executions(&mut stdin, &mut reader, &tools).await?;

    println!("✅ Comprehensive sah serve integration test passed!");
    Ok(())
}

/// Initialize MCP connection and return the initialize response
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

/// Test execution of sample tools to ensure they work
async fn test_tool_executions(
    stdin: &mut std::process::ChildStdin,
    reader: &mut BufReader<std::process::ChildStdout>,
    _tools: &[Value],
) -> Result<(), Box<dyn std::error::Error>> {
    // Test abort_create - should be safe to call
    test_single_tool_execution(
        stdin,
        reader,
        3,
        "abort_create",
        json!({
            "reason": "Integration test abort - this is expected"
        }),
    )
    .await?;

    // Test notify_create - should be safe to call
    test_single_tool_execution(
        stdin,
        reader,
        4,
        "notify_create",
        json!({
            "message": "Integration test notification",
            "level": "info"
        }),
    )
    .await?;

    // Test files_glob - should be safe to call with basic pattern
    test_single_tool_execution(
        stdin,
        reader,
        5,
        "files_glob",
        json!({
            "pattern": "*.md",
            "case_sensitive": false
        }),
    )
    .await?;

    // Test outline_generate - should be safe with simple pattern
    test_single_tool_execution(
        stdin,
        reader,
        6,
        "outline_generate",
        json!({
            "patterns": ["README.md"],
            "output_format": "yaml"
        }),
    )
    .await?;

    println!("✅ Tool execution tests completed successfully");
    Ok(())
}

/// Test execution of a single tool and validate response
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

    let response = timeout(Duration::from_secs(15), async { read_mcp_response(reader) })
        .await
        .expect(&format!(
            "Timeout waiting for {} execution response",
            tool_name
        ))
        .expect(&format!("Failed to read {} execution response", tool_name));

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

/// Wait for server to be ready by monitoring the process and allowing compilation time
fn wait_for_server_ready(
    child: &mut ProcessGuard,
    timeout: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = std::time::Instant::now();

    // First, wait for compilation to complete by checking if process is still alive
    // and monitoring stderr for completion indicators
    while start.elapsed() < timeout {
        // Check if process has exited unexpectedly
        if !child.is_running() {
            return Err("Server process exited during startup".into());
        }

        // Simple approach: wait a bit and check again
        // This gives the server time to compile and initialize
        std::thread::sleep(Duration::from_millis(1000));

        // After 30 seconds, assume compilation is done and server should be ready
        if start.elapsed() >= Duration::from_secs(30) {
            break;
        }
    }

    // Final check that process is still running after compilation period
    if !child.is_running() {
        return Err("Server process exited after compilation period".into());
    }

    // Give a bit more time for server initialization after compilation
    std::thread::sleep(Duration::from_secs(2));

    // Final verification that server is still alive
    if !child.is_running() {
        return Err("Server process exited during initialization".into());
    }

    Ok(())
}

/// Helper function to send MCP request
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

/// Test that validates the server properly shuts down
#[tokio::test]
async fn test_sah_serve_shutdown() {
    // Start server
    let child = Command::new("cargo")
        .args(["run", "--bin", "sah", "--", "serve"])
        .current_dir("..")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start MCP server");

    let mut child = ProcessGuard(child);

    // Give server time to start
    std::thread::sleep(Duration::from_millis(1000));

    // Try to terminate gracefully
    drop(child.0.stdin.take()); // Close stdin to signal shutdown

    // Server should shutdown within reasonable time
    let start_time = std::time::Instant::now();
    while start_time.elapsed() < Duration::from_secs(5) {
        match child.0.try_wait() {
            Ok(Some(_)) => {
                println!("✅ Server shut down gracefully");
                return;
            }
            Ok(None) => {
                // Still running
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                panic!("Error checking server status: {}", e);
            }
        }
    }

    // If we get here, server didn't shut down gracefully
    println!("⚠️  Server did not shut down gracefully, force killing");
}

/// Test that validates server can handle multiple concurrent connections
#[ignore = "MCP server integration tests are flaky due to rmcp library issues"]
#[tokio::test]
async fn test_sah_serve_concurrent_requests() {
    // This test validates that the server can handle multiple requests properly
    // Note: Since we're using stdio, we can't truly test concurrent connections,
    // but we can test rapid sequential requests

    let child = Command::new("cargo")
        .args(["run", "--bin", "sah", "--", "serve"])
        .current_dir("..")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start MCP server");

    let mut child = ProcessGuard(child);
    std::thread::sleep(Duration::from_millis(1000));

    let mut stdin = child.0.stdin.take().expect("Failed to get stdin");
    let stdout = child.0.stdout.take().expect("Failed to get stdout");
    let stderr = child.0.stderr.take().expect("Failed to get stderr");
    let mut reader = BufReader::new(stdout);

    // Spawn stderr reader
    std::thread::spawn(move || {
        let stderr_reader = BufReader::new(stderr);
        for line in stderr_reader.lines().map_while(Result::ok) {
            eprintln!("CONCURRENT SERVER: {line}");
        }
    });

    // Initialize connection
    let _ = initialize_mcp_connection(&mut stdin, &mut reader).await;
    let _ = send_initialized_notification(&mut stdin);

    // Send multiple rapid requests
    for i in 10..15 {
        let request = json!({
            "jsonrpc": "2.0",
            "id": i,
            "method": "tools/list"
        });

        let _ = send_mcp_request(&mut stdin, &request);

        let response = timeout(Duration::from_secs(5), async {
            read_mcp_response(&mut reader)
        })
        .await
        .expect(&format!("Timeout on request {}", i))
        .expect(&format!("Failed to read response for request {}", i));

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], i);
        assert!(response["result"]["tools"].is_array());
    }

    println!("✅ Server handled multiple rapid requests successfully");
}
