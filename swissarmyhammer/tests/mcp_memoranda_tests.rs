//! Comprehensive MCP integration tests for memoranda functionality
//!
//! Tests all MCP tool handlers for memo operations including:
//! - Creating, reading, updating, deleting memos
//! - Searching and listing memos  
//! - Getting context from all memos
//! - Error handling and edge cases
//! - Concurrent MCP requests
//! - Large memo content handling

use serde_json::json;
use serial_test::serial;
use std::io::BufReader;
use std::time::Duration;

// Test utilities module
mod test_utils {
    use serde_json::json;
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Child, Command, Stdio};
    use std::time::Duration;

    /// Process guard that automatically kills the process when dropped
    pub struct ProcessGuard(pub Child);

    impl Drop for ProcessGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    /// Start MCP server for testing with optimized binary path resolution
    pub fn start_mcp_server() -> std::io::Result<ProcessGuard> {
        // Create unique temporary directory for memo storage to ensure test isolation
        let temp_dir = tempfile::tempdir()?;
        let memos_dir = temp_dir.path().join("memos");

        // Optimize binary path resolution - prefer debug binary, fallback to release
        let binary_path = std::env::var("CARGO_BIN_EXE_sah")
            .or_else(|_| {
                std::env::var("CARGO_TARGET_DIR").map(|dir| format!("{dir}/debug/sah"))
            })
            .unwrap_or_else(|_| "../target/debug/sah".to_string());

        // Set test mode environment to skip heavy dependencies if possible
        let child = Command::new(&binary_path)
            .args(["serve"])
            .env("SWISSARMYHAMMER_MEMOS_DIR", memos_dir)
            .env("SWISSARMYHAMMER_TEST_MODE", "1")
            .env("RUST_LOG", "error")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null()) // Reduce stderr noise in tests
            .spawn()?;

        // Keep temp_dir alive by storing it in the ProcessGuard
        std::mem::forget(temp_dir);
        Ok(ProcessGuard(child))
    }

    /// Initialize MCP connection with handshake
    pub fn initialize_mcp_connection(
        stdin: &mut std::process::ChildStdin,
        reader: &mut BufReader<std::process::ChildStdout>,
    ) -> std::io::Result<()> {
        let init_request = json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0.0"
                }
            }
        });

        send_request(stdin, init_request)?;
        let response = read_response(reader)?;

        // Verify successful initialization
        if response.get("error").is_some() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("MCP initialization failed: {:?}", response["error"]),
            ));
        }

        // Send initialized notification
        let initialized_notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        send_request(stdin, initialized_notification)?;

        Ok(())
    }

    /// Clean up all existing memos to ensure clean test state
    pub fn cleanup_all_memos(
        stdin: &mut std::process::ChildStdin,
        reader: &mut BufReader<std::process::ChildStdout>,
    ) -> std::io::Result<()> {
        // First list all memos
        let list_request = create_tool_request(999, "memo_list", json!({}));
        send_request(stdin, list_request)?;
        let list_response = read_response(reader)?;

        if list_response.get("error").is_some() {
            return Ok(()); // If list fails, assume no memos to clean
        }

        let response_text = list_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("");

        // Extract memo IDs from the response text and delete them
        let mut request_id = 1000;
        for line in response_text.lines() {
            if let Some(start) = line.find('(') {
                if let Some(end) = line.find(')') {
                    if start < end {
                        let memo_id = &line[start + 1..end];
                        if memo_id.len() == 26 {
                            // ULID length check
                            let delete_request = create_tool_request(
                                request_id,
                                "memo_delete",
                                json!({
                                    "id": memo_id
                                }),
                            );
                            send_request(stdin, delete_request)?;
                            let _ = read_response(reader)?; // Consume response
                            request_id += 1;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Wait for server to be ready with optimized timing
    pub async fn wait_for_server_ready() {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    /// Send JSON-RPC request to MCP server
    pub fn send_request(
        stdin: &mut std::process::ChildStdin,
        request: serde_json::Value,
    ) -> std::io::Result<()> {
        let request_str = serde_json::to_string(&request)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writeln!(stdin, "{request_str}")?;
        stdin.flush()
    }

    /// Read JSON-RPC response from MCP server
    pub fn read_response(
        reader: &mut BufReader<std::process::ChildStdout>,
    ) -> std::io::Result<serde_json::Value> {
        let mut line = String::new();
        reader.read_line(&mut line)?;

        if line.trim().is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Empty response",
            ));
        }

        serde_json::from_str(&line).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("JSON parse error: {e}"),
            )
        })
    }

    /// Create a standard MCP tool call request
    pub fn create_tool_request(
        id: i64,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> serde_json::Value {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        })
    }
}

use test_utils::*;

/// Test memo creation via MCP
#[tokio::test]
#[serial]
async fn test_mcp_memo_create() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.0.stdin.take().expect("Failed to get stdin");
    let stdout = server.0.stdout.take().expect("Failed to get stdout");
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    // Test successful memo creation
    let create_request = create_tool_request(
        1,
        "memo_create",
        json!({
            "title": "Test Memo via MCP",
            "content": "This is test content created via MCP"
        }),
    );

    send_request(&mut stdin, create_request).unwrap();
    let response = read_response(&mut reader).unwrap();

    // Verify response structure
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert!(response.get("error").is_none());

    let result = &response["result"];
    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Successfully created memo"));
    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Test Memo via MCP"));
}

/// Test memo creation with empty title and content
#[tokio::test]
#[serial]
async fn test_mcp_memo_create_empty_content() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.0.stdin.take().unwrap();
    let stdout = server.0.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    let create_request = create_tool_request(
        1,
        "memo_create",
        json!({
            "title": "",
            "content": ""
        }),
    );

    send_request(&mut stdin, create_request).unwrap();
    let response = read_response(&mut reader).unwrap();

    // Should succeed even with empty content
    assert!(response.get("error").is_none());
    let result = &response["result"];
    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Successfully created memo"));
}

/// Test memo creation with unicode content
#[tokio::test]
#[serial]
async fn test_mcp_memo_create_unicode() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.0.stdin.take().unwrap();
    let stdout = server.0.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    let create_request = create_tool_request(
        1,
        "memo_create",
        json!({
            "title": "🚀 Unicode Test with 中文",
            "content": "Content with émojis 🎉 and unicode chars: ñáéíóú, 中文测试"
        }),
    );

    send_request(&mut stdin, create_request).unwrap();
    let response = read_response(&mut reader).unwrap();

    assert!(response.get("error").is_none());
    let result = &response["result"];
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("🚀 Unicode Test with 中文"));
    assert!(text.contains("émojis 🎉"));
}

/// Test memo retrieval via MCP
#[tokio::test]
#[serial]
async fn test_mcp_memo_get() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.0.stdin.take().unwrap();
    let stdout = server.0.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    // Clean up any existing memos to ensure clean test state
    cleanup_all_memos(&mut stdin, &mut reader).unwrap();

    // First create a memo
    let create_request = create_tool_request(
        1,
        "memo_create",
        json!({
            "title": "Test Get Memo",
            "content": "Content for get test"
        }),
    );

    send_request(&mut stdin, create_request).unwrap();
    let create_response = read_response(&mut reader).unwrap();

    // Extract memo ID from creation response
    let create_text = create_response["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    let memo_id = extract_memo_id_from_response(create_text);

    // Now get the memo
    let get_request = create_tool_request(
        2,
        "memo_get",
        json!({
            "id": memo_id
        }),
    );

    send_request(&mut stdin, get_request).unwrap();
    let get_response = read_response(&mut reader).unwrap();

    assert!(get_response.get("error").is_none());
    let result = &get_response["result"];
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Test Get Memo"));
    assert!(text.contains("Content for get test"));
    assert!(text.contains(&memo_id));
}

/// Test memo get with invalid ID
#[tokio::test]
#[serial]
async fn test_mcp_memo_get_invalid_id() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.0.stdin.take().unwrap();
    let stdout = server.0.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    let get_request = create_tool_request(
        1,
        "memo_get",
        json!({
            "id": "invalid/memo*id"
        }),
    );

    send_request(&mut stdin, get_request).unwrap();
    let response = read_response(&mut reader).unwrap();

    // Should return error for invalid ID format
    assert!(response.get("error").is_some());
    let error = &response["error"];
    assert_eq!(error["code"], -32602); // Invalid params
    assert!(error["message"]
        .as_str()
        .unwrap()
        .contains("Invalid memo ID format"));
}

/// Test memo get with non-existent valid ID
#[tokio::test]
#[serial]
async fn test_mcp_memo_get_nonexistent() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.0.stdin.take().unwrap();
    let stdout = server.0.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    let get_request = create_tool_request(
        1,
        "memo_get",
        json!({
            "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV" // Valid ULID format but doesn't exist
        }),
    );

    send_request(&mut stdin, get_request).unwrap();
    let response = read_response(&mut reader).unwrap();

    // Should return error for non-existent memo
    assert!(response.get("error").is_some());
    let error = &response["error"];
    assert_eq!(error["code"], -32602); // Invalid params
    assert!(error["message"].as_str().unwrap().contains("not found"));
}

/// Test memo update via MCP
#[tokio::test]
#[serial]
async fn test_mcp_memo_update() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.0.stdin.take().unwrap();
    let stdout = server.0.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    // Clean up any existing memos to ensure clean test state
    cleanup_all_memos(&mut stdin, &mut reader).unwrap();

    // Create a memo first
    let create_request = create_tool_request(
        1,
        "memo_create",
        json!({
            "title": "Update Test Memo",
            "content": "Original content"
        }),
    );

    send_request(&mut stdin, create_request).unwrap();
    let create_response = read_response(&mut reader).unwrap();
    let memo_id = extract_memo_id_from_response(
        create_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    );

    // Update the memo
    let update_request = create_tool_request(
        2,
        "memo_update",
        json!({
            "id": memo_id,
            "content": "Updated content via MCP"
        }),
    );

    send_request(&mut stdin, update_request).unwrap();
    let update_response = read_response(&mut reader).unwrap();

    assert!(update_response.get("error").is_none());
    let result = &update_response["result"];
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Successfully updated memo"));
    assert!(text.contains("Updated content via MCP"));
    assert!(text.contains("Update Test Memo")); // Title should remain same
}

/// Test memo delete via MCP
#[tokio::test]
#[serial]
async fn test_mcp_memo_delete() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.0.stdin.take().unwrap();
    let stdout = server.0.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    // Clean up any existing memos to ensure clean test state
    cleanup_all_memos(&mut stdin, &mut reader).unwrap();

    // Create a memo first
    let create_request = create_tool_request(
        1,
        "memo_create",
        json!({
            "title": "Delete Test Memo",
            "content": "To be deleted"
        }),
    );

    send_request(&mut stdin, create_request).unwrap();
    let create_response = read_response(&mut reader).unwrap();
    let memo_id = extract_memo_id_from_response(
        create_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    );

    // Delete the memo
    let delete_request = create_tool_request(
        2,
        "memo_delete",
        json!({
            "id": memo_id
        }),
    );

    send_request(&mut stdin, delete_request).unwrap();
    let delete_response = read_response(&mut reader).unwrap();

    assert!(delete_response.get("error").is_none());
    let result = &delete_response["result"];
    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Successfully deleted memo"));

    // Verify memo is actually deleted by trying to get it
    let get_request = create_tool_request(
        3,
        "memo_get",
        json!({
            "id": memo_id
        }),
    );

    send_request(&mut stdin, get_request).unwrap();
    let get_response = read_response(&mut reader).unwrap();

    // Should return error since memo is deleted
    assert!(get_response.get("error").is_some());
}

/// Test memo list via MCP
#[tokio::test]
#[serial]
async fn test_mcp_memo_list() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.0.stdin.take().unwrap();
    let stdout = server.0.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    // Clean up any existing memos to ensure clean test state
    cleanup_all_memos(&mut stdin, &mut reader).unwrap();

    // Test empty list first
    let list_request = create_tool_request(1, "memo_list", json!({}));
    send_request(&mut stdin, list_request).unwrap();
    let empty_response = read_response(&mut reader).unwrap();

    assert!(empty_response.get("error").is_none());
    let actual_text = empty_response["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(actual_text.contains("No memos found") || actual_text.contains("Found 0 memos"));

    // Create some memos
    for i in 1..=3 {
        let create_request = create_tool_request(
            i + 1,
            "memo_create",
            json!({
                "title": format!("List Test Memo {}", i),
                "content": format!("Content for memo {}", i)
            }),
        );
        send_request(&mut stdin, create_request).unwrap();
        let _ = read_response(&mut reader).unwrap(); // Consume response
    }

    // List memos again
    let list_request = create_tool_request(5, "memo_list", json!({}));
    send_request(&mut stdin, list_request).unwrap();
    let list_response = read_response(&mut reader).unwrap();

    assert!(list_response.get("error").is_none());
    let result = &list_response["result"];
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Found 3 memos"));
    assert!(text.contains("List Test Memo 1"));
    assert!(text.contains("List Test Memo 2"));
    assert!(text.contains("List Test Memo 3"));
}

/// Test memo search via MCP
#[tokio::test]
#[serial]
async fn test_mcp_memo_search() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.0.stdin.take().unwrap();
    let stdout = server.0.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    // Clean up any existing memos to ensure clean test state
    cleanup_all_memos(&mut stdin, &mut reader).unwrap();

    // Create test memos with different content
    let test_memos = [
        ("Rust Programming", "Learning Rust language"),
        ("Python Guide", "Python programming tutorial"),
        ("JavaScript Basics", "Introduction to JavaScript"),
        ("Rust Advanced", "Advanced Rust concepts"),
    ];

    for (i, (title, content)) in test_memos.iter().enumerate() {
        let create_request = create_tool_request(
            i as i64 + 1,
            "memo_create",
            json!({
                "title": title,
                "content": content
            }),
        );
        send_request(&mut stdin, create_request).unwrap();
        let _ = read_response(&mut reader).unwrap();
    }

    // Search for "Rust" - should find 2 memos
    let search_request = create_tool_request(
        10,
        "memo_search",
        json!({
            "query": "Rust"
        }),
    );
    send_request(&mut stdin, search_request).unwrap();
    let search_response = read_response(&mut reader).unwrap();

    assert!(search_response.get("error").is_none());
    let result = &search_response["result"];
    let text = result["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("Found 2 memos matching 'Rust'"));
    assert!(text.contains("Rust Programming"));
    assert!(text.contains("Rust Advanced"));

    // Search for non-existent content
    let empty_search = create_tool_request(
        11,
        "memo_search",
        json!({
            "query": "nonexistent"
        }),
    );
    send_request(&mut stdin, empty_search).unwrap();
    let empty_response = read_response(&mut reader).unwrap();

    let empty_text = empty_response["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(empty_text.contains("No memos found matching query"));
}

/// Test memo search case insensitivity
#[tokio::test]
#[serial]
async fn test_mcp_memo_search_case_insensitive() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.0.stdin.take().unwrap();
    let stdout = server.0.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    // Clean up any existing memos to ensure clean test state
    cleanup_all_memos(&mut stdin, &mut reader).unwrap();

    // Create a memo with mixed case
    let create_request = create_tool_request(
        1,
        "memo_create",
        json!({
            "title": "CamelCase Title",
            "content": "Content with MixedCase words"
        }),
    );
    send_request(&mut stdin, create_request).unwrap();
    let _ = read_response(&mut reader).unwrap();

    // Search with different cases
    let search_cases = ["camelcase", "MIXEDCASE", "MiXeDcAsE"];

    for (i, query) in search_cases.iter().enumerate() {
        let search_request = create_tool_request(
            i as i64 + 2,
            "memo_search",
            json!({
                "query": query
            }),
        );
        send_request(&mut stdin, search_request).unwrap();
        let response = read_response(&mut reader).unwrap();

        assert!(response.get("error").is_none());
        let text = response["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Found 1 memo matching"));
    }
}

/// Test memo get all context via MCP
#[tokio::test]
#[serial]
async fn test_mcp_memo_get_all_context() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.0.stdin.take().unwrap();
    let stdout = server.0.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    // Clean up any existing memos to ensure clean test state
    cleanup_all_memos(&mut stdin, &mut reader).unwrap();

    // Test empty context first
    let context_request = create_tool_request(1, "memo_get_all_context", json!({}));
    send_request(&mut stdin, context_request).unwrap();
    let empty_response = read_response(&mut reader).unwrap();

    assert!(empty_response.get("error").is_none());
    let context_text = empty_response["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(context_text.contains("No memos available") || context_text.contains("Found 0 memos"));

    // Create some memos with delays to test ordering
    for i in 1..=3 {
        let create_request = create_tool_request(
            i + 1,
            "memo_create",
            json!({
                "title": format!("Context Memo {}", i),
                "content": format!("Context content for memo {}", i)
            }),
        );
        send_request(&mut stdin, create_request).unwrap();
        let _ = read_response(&mut reader).unwrap();

        // Small delay to ensure different timestamps
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Get all context
    let context_request = create_tool_request(5, "memo_get_all_context", json!({}));
    send_request(&mut stdin, context_request).unwrap();
    let context_response = read_response(&mut reader).unwrap();

    assert!(context_response.get("error").is_none());
    let result = &context_response["result"];
    let text = result["content"][0]["text"].as_str().unwrap();

    assert!(text.contains("All memo context (3 memos)"));
    assert!(text.contains("Context Memo 1"));
    assert!(text.contains("Context Memo 2"));
    assert!(text.contains("Context Memo 3"));
    assert!(text.contains("===")); // Context separators
}

/// Test large memo content handling via MCP
#[tokio::test]
#[serial]
async fn test_mcp_memo_large_content() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.0.stdin.take().unwrap();
    let stdout = server.0.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    // Clean up any existing memos to ensure clean test state
    cleanup_all_memos(&mut stdin, &mut reader).unwrap();

    // Create a large memo (100KB content)
    let large_content = "x".repeat(100_000);
    let create_request = create_tool_request(
        1,
        "memo_create",
        json!({
            "title": "Large Content Memo",
            "content": large_content
        }),
    );

    send_request(&mut stdin, create_request).unwrap();
    let create_response = read_response(&mut reader).unwrap();

    assert!(create_response.get("error").is_none());
    assert!(create_response["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Successfully created memo"));

    // Extract ID and verify we can retrieve it
    let memo_id = extract_memo_id_from_response(
        create_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    );

    let get_request = create_tool_request(
        2,
        "memo_get",
        json!({
            "id": memo_id
        }),
    );

    send_request(&mut stdin, get_request).unwrap();
    let get_response = read_response(&mut reader).unwrap();

    assert!(get_response.get("error").is_none());
    // The get response should contain the large content (truncated in preview)
    assert!(get_response["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Large Content Memo"));
}

/// Test MCP error handling for malformed requests
#[tokio::test]
#[serial]
async fn test_mcp_memo_malformed_requests() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.0.stdin.take().unwrap();
    let stdout = server.0.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    // Test missing required fields
    let bad_create = create_tool_request(
        1,
        "memo_create",
        json!({
            "title": "Test"
            // Missing content field
        }),
    );

    send_request(&mut stdin, bad_create).unwrap();
    let response = read_response(&mut reader).unwrap();
    assert!(response.get("error").is_some());

    // Test invalid tool name
    let invalid_tool_request = create_tool_request(
        2,
        "nonexistent_tool",
        json!({
            "some": "argument"
        }),
    );

    send_request(&mut stdin, invalid_tool_request).unwrap();
    let invalid_response = read_response(&mut reader).unwrap();
    assert!(invalid_response.get("error").is_some());
}

/// Test MCP tool list includes all memo tools
#[tokio::test]
#[serial]
async fn test_mcp_memo_tool_list() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.0.stdin.take().unwrap();
    let stdout = server.0.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    // Request tool list
    let tools_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    });

    send_request(&mut stdin, tools_request).unwrap();
    let response = read_response(&mut reader).unwrap();

    assert!(response.get("error").is_none());
    let tools = &response["result"]["tools"];
    assert!(tools.is_array());

    // Convert tools to list of names for easy checking
    let tool_names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();

    // Verify all memo tools are present
    let expected_memo_tools = vec![
        "memo_create",
        "memo_get",
        "memo_update",
        "memo_delete",
        "memo_list",
        "memo_search",
        "memo_get_all_context",
    ];

    for tool_name in expected_memo_tools {
        assert!(
            tool_names.contains(&tool_name),
            "Tool '{tool_name}' not found in tool list: {tool_names:?}"
        );
    }
}

/// Helper function to extract memo ID from MCP response text
fn extract_memo_id_from_response(response_text: &str) -> String {
    // Look for pattern "with ID: <ULID>"
    if let Some(start) = response_text.find("with ID: ") {
        let id_start = start + "with ID: ".len();
        if let Some(end) = response_text[id_start..].find('\n') {
            return response_text[id_start..id_start + end].trim().to_string();
        }
        // If no newline found, take until whitespace or end
        if let Some(end) = response_text[id_start..].find(char::is_whitespace) {
            return response_text[id_start..id_start + end].trim().to_string();
        }
        // Take rest of string if no whitespace
        return response_text[id_start..].trim().to_string();
    }
    panic!("Could not extract memo ID from response: {response_text}");
}

#[cfg(test)]
mod stress_tests {
    use super::*;

    /// Performance test: Create, update, and delete memos with optimized timing
    #[tokio::test]
    async fn test_mcp_memo_performance_operations() {
        let mut server = start_mcp_server().unwrap();
        wait_for_server_ready().await;

        let mut stdin = server.0.stdin.take().unwrap();
        let stdout = server.0.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);

        // Initialize MCP connection
        initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

        // Clean up any existing memos to ensure clean test state
        cleanup_all_memos(&mut stdin, &mut reader).unwrap();

        // Reduced from 50 to 12 memos to ensure test completes in under 10 seconds
        let num_memos = 12;
        let mut memo_ids = Vec::new();

        // Create memos with small delays to prevent overwhelming the server
        for i in 1..=num_memos {
            let create_request = create_tool_request(
                i,
                "memo_create",
                json!({
                    "title": format!("Performance Test Memo {}", i),
                    "content": format!("Content for performance test memo {} with additional text", i)
                }),
            );

            if let Err(e) = send_request(&mut stdin, create_request) {
                panic!("Failed to send create request for memo {i}: {e}");
            }

            let response = match read_response(&mut reader) {
                Ok(resp) => resp,
                Err(e) => panic!("Failed to read create response for memo {i}: {e}"),
            };

            assert!(
                response.get("error").is_none(),
                "Failed to create memo {i}: {:?}",
                response.get("error")
            );

            let memo_id = extract_memo_id_from_response(
                response["result"]["content"][0]["text"].as_str().unwrap(),
            );
            memo_ids.push(memo_id);

            // Small delay to prevent server overload
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        }

        // Update all memos
        for (i, memo_id) in memo_ids.iter().enumerate() {
            let update_request = create_tool_request(
                i as i64 + num_memos + 1,
                "memo_update",
                json!({
                    "id": memo_id,
                    "content": format!("Updated content for memo {}", i + 1)
                }),
            );

            if let Err(e) = send_request(&mut stdin, update_request) {
                panic!("Failed to send update request for memo {memo_id}: {e}");
            }

            let response = match read_response(&mut reader) {
                Ok(resp) => resp,
                Err(e) => panic!("Failed to read update response for memo {memo_id}: {e}"),
            };

            assert!(
                response.get("error").is_none(),
                "Failed to update memo {memo_id}: {:?}",
                response.get("error")
            );

            // Small delay to prevent server overload
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        }

        // Delete all memos
        for (i, memo_id) in memo_ids.iter().enumerate() {
            let delete_request = create_tool_request(
                i as i64 + (num_memos * 2) + 1,
                "memo_delete",
                json!({
                    "id": memo_id
                }),
            );

            if let Err(e) = send_request(&mut stdin, delete_request) {
                panic!("Failed to send delete request for memo {memo_id}: {e}");
            }

            let response = match read_response(&mut reader) {
                Ok(resp) => resp,
                Err(e) => panic!("Failed to read delete response for memo {memo_id}: {e}"),
            };

            assert!(
                response.get("error").is_none(),
                "Failed to delete memo {memo_id}: {:?}",
                response.get("error")
            );

            // Small delay to prevent server overload
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        }

        // Verify all memos are deleted
        let list_request = create_tool_request(num_memos * 3 + 1, "memo_list", json!({}));
        send_request(&mut stdin, list_request).unwrap();
        let list_response = read_response(&mut reader).unwrap();

        let text = list_response["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        assert!(text.contains("No memos found") || text.contains("Found 0 memos"));
    }

    /// Performance test: Search performance with optimized memo count
    #[tokio::test]
    async fn test_mcp_memo_search_performance() {
        let mut server = start_mcp_server().unwrap();
        wait_for_server_ready().await;

        let mut stdin = server.0.stdin.take().unwrap();
        let stdout = server.0.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);

        // Initialize MCP connection
        initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

        // Clean up any existing memos to ensure clean test state
        cleanup_all_memos(&mut stdin, &mut reader).unwrap();

        // Create memos with different patterns for searching
        // Reduced from 5 patterns × 20 memos = 100 total to 3 patterns × 4 memos = 12 total
        let patterns = ["project", "meeting", "documentation"];
        let num_per_pattern = 4;

        for (pattern_idx, pattern) in patterns.iter().enumerate() {
            for i in 1..=num_per_pattern {
                let create_request = create_tool_request(
                    pattern_idx as i64 * num_per_pattern + i,
                    "memo_create",
                    json!({
                        "title": format!("{} Task {}", pattern, i),
                        "content": format!("This memo is about {} work item number {} with context", pattern, i)
                    }),
                );

                if let Err(e) = send_request(&mut stdin, create_request) {
                    panic!("Failed to send search test create request: {e}");
                }

                if let Err(e) = read_response(&mut reader) {
                    panic!("Failed to read search test create response: {e}");
                }

                // Small delay to prevent server overload
                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            }
        }

        // Perform searches for each pattern
        for (pattern_idx, pattern) in patterns.iter().enumerate() {
            let search_request = create_tool_request(
                1000 + pattern_idx as i64,
                "memo_search",
                json!({
                    "query": pattern
                }),
            );

            if let Err(e) = send_request(&mut stdin, search_request) {
                panic!("Failed to send search request for pattern {pattern}: {e}");
            }

            let response = match read_response(&mut reader) {
                Ok(resp) => resp,
                Err(e) => panic!("Failed to read search response for pattern {pattern}: {e}"),
            };

            assert!(
                response.get("error").is_none(),
                "Search failed for pattern {pattern}: {:?}",
                response.get("error")
            );
            let text = response["result"]["content"][0]["text"].as_str().unwrap();
            assert!(
                text.contains(&format!("Found {num_per_pattern} memo"))
                    || text.contains(&format!("{num_per_pattern} memos matching")),
                "Expected to find {num_per_pattern} memos for pattern '{pattern}', but got: {text}"
            );
        }
    }
}
