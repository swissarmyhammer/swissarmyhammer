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
    pub struct ProcessGuard {
        pub child: Child,
        pub _temp_dir: tempfile::TempDir, // Keep temp directory alive
    }

    impl Drop for ProcessGuard {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    impl ProcessGuard {
        pub fn new(child: Child, temp_dir: tempfile::TempDir) -> Self {
            Self {
                child,
                _temp_dir: temp_dir,
            }
        }
    }

    /// Start MCP server for testing with optimized binary path resolution
    pub fn start_mcp_server() -> std::io::Result<ProcessGuard> {
        // Create unique temporary directory for memo storage to ensure test isolation
        // Use current timestamp + thread id + random component to ensure each test gets a truly unique directory
        let temp_dir = tempfile::tempdir()?;
        let thread_id = std::thread::current().id();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let random = rand::random::<u64>();
        let test_id = format!("{}_{}_{}", timestamp, format!("{:?}", thread_id).replace("ThreadId(", "").replace(")", ""), random);
        let memos_dir = temp_dir.path().join("memos").join(test_id);

        // Optimize binary path resolution - try multiple locations
        let binary_path = std::env::var("CARGO_BIN_EXE_sah")
            .or_else(|_| std::env::var("CARGO_TARGET_DIR").map(|dir| format!("{dir}/debug/sah")))
            .unwrap_or_else(|_| {
                // Get the current directory and determine relative paths
                let current_dir = std::env::current_dir().unwrap_or_default();
                let current_path = current_dir.to_string_lossy();

                let candidates = if current_path.ends_with("swissarmyhammer") {
                    // Running from swissarmyhammer directory (as tests do)
                    vec![
                        "../target/debug/sah",    // From swissarmyhammer to project root
                        "../../target/debug/sah", // From nested directory
                        "../swissarmyhammer-cli/target/debug/sah", // From CLI package
                        "./target/debug/sah",     // Local fallback
                    ]
                } else {
                    // Running from project root or other location
                    vec![
                        "./target/debug/sah",                     // From project root
                        "../target/debug/sah",                    // From nested directory
                        "./swissarmyhammer-cli/target/debug/sah", // From CLI package
                        "../../target/debug/sah",                 // Deep nested fallback
                    ]
                };

                for path in &candidates {
                    if std::path::Path::new(path).exists() {
                        eprintln!("Found binary at: {}", path);
                        return path.to_string();
                    }
                }

                // Absolute fallback - construct from current directory
                let absolute_path = current_dir
                    .parent()
                    .unwrap_or(&current_dir)
                    .join("target/debug/sah");

                if absolute_path.exists() {
                    return absolute_path.to_string_lossy().to_string();
                }

                // Final fallback
                "../target/debug/sah".to_string()
            });

        eprintln!("Using binary path: {binary_path}");
        println!("MCP_SERVER_START: Starting server with memos_dir: {}", memos_dir.display());

        // Set test mode environment to skip heavy dependencies if possible
        let child = Command::new(&binary_path)
            .args(["serve"])
            .env("SWISSARMYHAMMER_MEMOS_DIR", memos_dir)
            .env("SWISSARMYHAMMER_TEST_MODE", "1")
            .env("RUST_LOG", "error")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()) // Capture stderr for debugging
            .spawn()?;

        // MCP server started
        Ok(ProcessGuard::new(child, temp_dir))
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

        // Small delay to ensure server is ready for subsequent requests
        std::thread::sleep(std::time::Duration::from_millis(100));

        Ok(())
    }

    /// Clean up all existing memos to ensure clean test state
    pub fn cleanup_all_memos(
        stdin: &mut std::process::ChildStdin,
        reader: &mut BufReader<std::process::ChildStdout>,
    ) -> std::io::Result<()> {
        let mut total_deleted = 0;
        let max_attempts = 5; // Prevent infinite loops
        
        for attempt in 1..=max_attempts {
            eprintln!("CLEANUP: Attempt {} to clean memos", attempt);
            println!("CLEANUP DEBUG: Attempt {} to clean memos", attempt);
            
            // List all memos
            let list_request = create_tool_request(999 + attempt, "memo_list", json!({}));
            send_request(stdin, list_request)?;
            let list_response = read_response(reader)?;

            if list_response.get("error").is_some() {
                eprintln!("CLEANUP: List request failed: {:?}", list_response.get("error"));
                return Ok(()); // If list fails, assume no memos to clean
            }

            let response_text = list_response["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or("");

            eprintln!("CLEANUP: Attempt {}: Listed memos: {}", attempt, response_text);
            println!("CLEANUP DEBUG: Attempt {}: Listed memos: {}", attempt, response_text);

            if response_text.contains("Found 0 memos") || response_text.contains("No memos found") {
                eprintln!("CLEANUP: No memos found on attempt {}", attempt);
                break;
            }

            // Extract memo titles from the response text and delete them
            // Format is: • Title (Title) - use title as ID since that's how the system works
            let mut request_id = 1000 + (attempt * 100);
            let mut deletion_count = 0;
            let mut memo_ids = Vec::new();
            
            // First extract all memo titles (which serve as IDs)
            for line in response_text.lines() {
                if line.trim().starts_with('•') {
                    // Extract title between • and ( - handle Unicode properly
                    // Use strip_prefix to safely remove the bullet point and any whitespace
                    let after_bullet = line.strip_prefix('•').unwrap_or(line).trim();
                    if let Some(title_end) = after_bullet.find('(') {
                        let title = &after_bullet[..title_end];
                        let title = title.trim();
                        if !title.is_empty() {
                            memo_ids.push(title.to_string());
                            eprintln!("CLEANUP: Extracted title/ID: '{}'", title);
                        }
                    }
                }
            }
            
            eprintln!("CLEANUP: Found {} memo IDs to delete: {:?}", memo_ids.len(), memo_ids);
            println!("CLEANUP DEBUG: Found {} memo IDs to delete: {:?}", memo_ids.len(), memo_ids);
            
            // Delete each memo
            for memo_id in memo_ids {
                eprintln!("CLEANUP: Attempting to delete memo ID: '{}'", memo_id);
                println!("CLEANUP DEBUG: Attempting to delete memo ID: '{}'", memo_id);
                
                let delete_request = create_tool_request(
                    request_id,
                    "memo_delete",
                    json!({
                        "id": memo_id
                    }),
                );
                send_request(stdin, delete_request)?;
                let delete_response = read_response(reader)?;
                
                if delete_response.get("error").is_some() {
                    eprintln!("CLEANUP: Delete failed for '{}': {:?}", memo_id, delete_response.get("error"));
                } else {
                    let success_text = delete_response["result"]["content"][0]["text"].as_str().unwrap_or("");
                    eprintln!("CLEANUP: Delete successful for '{}': {}", memo_id, success_text);
                    deletion_count += 1;
                }
                request_id += 1;
                
                // Small delay to prevent overwhelming the server
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            
            total_deleted += deletion_count;
            eprintln!("CLEANUP: Attempt {}: Successfully deleted {} memos", attempt, deletion_count);
            
            if deletion_count == 0 {
                eprintln!("CLEANUP: No memos deleted on attempt {}, stopping", attempt);
                break;
            }
        }

        eprintln!("CLEANUP: Total deleted {} memos across all attempts", total_deleted);
        println!("CLEANUP DEBUG: Total deleted {} memos across all attempts", total_deleted);
        Ok(())
    }

    /// Wait for server to be ready with optimized timing
    pub async fn wait_for_server_ready() {
        tokio::time::sleep(Duration::from_millis(200)).await;
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
        let bytes_read = reader.read_line(&mut line)?;

        if bytes_read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Empty response - server may have exited",
            ));
        }

        if line.trim().is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Empty line response",
            ));
        }

        serde_json::from_str(&line).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("JSON parse error on line '{}': {e}", line.trim()),
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

    let mut stdin = server.child.stdin.take().unwrap();
    let stdout = server.child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    // Clean up any existing memos to ensure clean test state
    cleanup_all_memos(&mut stdin, &mut reader).unwrap();

    let unique_title = format!("Test Memo via MCP {}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    let create_request = create_tool_request(
        1,
        "memo_create",
        json!({
            "title": unique_title,
            "content": "This is test content created via MCP"
        }),
    );

    send_request(&mut stdin, create_request).unwrap();
    let response = read_response(&mut reader).unwrap();

    if response.get("error").is_some() {
        println!("ERROR: {:?}", response["error"]);
    }
    assert!(response.get("error").is_none());
    let result = &response["result"];
    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Successfully created memo"));

    // Clean up the memo we created (use title as ID)
    let delete_request = create_tool_request(
        2,
        "memo_delete",
        json!({
            "id": unique_title
        }),
    );

    send_request(&mut stdin, delete_request).unwrap();
    let delete_response = read_response(&mut reader).unwrap();
    assert!(delete_response.get("error").is_none());
}

/// Test memo creation with empty title and content
#[tokio::test]
#[serial]
async fn test_mcp_memo_create_empty_content() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.child.stdin.take().unwrap();
    let stdout = server.child.stdout.take().unwrap();
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
    
    // Should fail because empty title is not allowed
    println!("EMPTY TITLE TEST - RESPONSE: {}", serde_json::to_string_pretty(&response).unwrap());
    assert!(response.get("error").is_some() || 
            (response.get("result").is_some() && 
             response["result"]["isError"].as_bool().unwrap_or(false)));
}

/// Test memo creation with unicode content
#[tokio::test]
#[serial]
async fn test_mcp_memo_create_unicode() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.child.stdin.take().unwrap();
    let stdout = server.child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    // Clean up any existing memos to ensure clean test state
    cleanup_all_memos(&mut stdin, &mut reader).unwrap();

    let unique_title = format!("🚀 Unicode Test with 中文 {}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    let create_request = create_tool_request(
        1,
        "memo_create",
        json!({
            "title": unique_title.clone(),
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

    // Clean up the memo we created (use title as ID)
    let delete_request = create_tool_request(
        2,
        "memo_delete",
        json!({
            "id": unique_title
        }),
    );

    send_request(&mut stdin, delete_request).unwrap();
    let delete_response = read_response(&mut reader).unwrap();
    assert!(delete_response.get("error").is_none());
}

/// Test memo retrieval via MCP
#[tokio::test]
#[serial]
async fn test_mcp_memo_get() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.child.stdin.take().unwrap();
    let stdout = server.child.stdout.take().unwrap();
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

    // Check for errors first
    if create_response.get("error").is_some() {
        panic!("Create memo failed with error: {:?}", create_response.get("error"));
    }

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

    // Clean up the memo we created (use title as ID)
    let delete_request = create_tool_request(
        3,
        "memo_delete",
        json!({
            "id": "Test Get Memo"
        }),
    );

    send_request(&mut stdin, delete_request).unwrap();
    let delete_response = read_response(&mut reader).unwrap();
    assert!(delete_response.get("error").is_none());
}

/// Test memo get with invalid ID
#[tokio::test]
#[serial]
async fn test_mcp_memo_get_invalid_id() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.child.stdin.take().unwrap();
    let stdout = server.child.stdout.take().unwrap();
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

    let mut stdin = server.child.stdin.take().unwrap();
    let stdout = server.child.stdout.take().unwrap();
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

    // Should return success response with "not found" message
    assert!(response.get("error").is_none());
    let result = &response["result"];
    assert_eq!(result.get("is_error").or(result.get("isError")).unwrap_or(&serde_json::Value::Bool(false)), &serde_json::Value::Bool(false));
    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Memo not found"));
}

/// Test memo update via MCP
#[tokio::test]
#[serial]
async fn test_mcp_memo_update() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.child.stdin.take().unwrap();
    let stdout = server.child.stdout.take().unwrap();
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

    // Clean up the memo we created (use title as ID)
    let delete_request = create_tool_request(
        3,
        "memo_delete",
        json!({
            "id": "Update Test Memo"
        }),
    );

    send_request(&mut stdin, delete_request).unwrap();
    let delete_response = read_response(&mut reader).unwrap();
    assert!(delete_response.get("error").is_none());
}

/// Test memo delete via MCP
#[tokio::test]
#[serial]
async fn test_mcp_memo_delete() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.child.stdin.take().unwrap();
    let stdout = server.child.stdout.take().unwrap();
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

    // Should return success response with "not found" message since memo is deleted
    assert!(get_response.get("error").is_none());
    let result = &get_response["result"];
    assert_eq!(result.get("is_error").or(result.get("isError")).unwrap_or(&serde_json::Value::Bool(false)), &serde_json::Value::Bool(false));
    assert!(result["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("Memo not found"));
}

/// Test memo list via MCP
#[tokio::test]
#[serial]
async fn test_mcp_memo_list() {
    println!("=== STARTING test_mcp_memo_list ===");
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.child.stdin.take().unwrap();
    let stdout = server.child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();
    println!("=== MCP connection initialized ===");

    // Clean up any existing memos to ensure clean test state
    println!("=== Starting cleanup ===");
    cleanup_all_memos(&mut stdin, &mut reader).unwrap();
    println!("=== Cleanup completed ===");

    // Test empty list first
    let list_request = create_tool_request(1, "memo_list", json!({}));
    send_request(&mut stdin, list_request).unwrap();
    let empty_response = read_response(&mut reader).unwrap();

    assert!(empty_response.get("error").is_none());
    let actual_text = empty_response["result"]["content"][0]["text"]
        .as_str()
        .expect("Expected empty_response[\"result\"][\"content\"][0][\"text\"] to be string");

    println!("DEBUG memo_list: actual_text = '{}'", actual_text);
    println!("DEBUG memo_list: full response = {}", serde_json::to_string_pretty(&empty_response).unwrap());
    
    if !actual_text.contains("No memos found") && !actual_text.contains("Found 0 memos") {
        panic!("Expected 'No memos found' or 'Found 0 memos', but got: '{}'", actual_text);
    }

    println!("=== Empty list test passed, now creating 3 memos ===");

    // Create some memos
    for i in 1..=3 {
        println!("=== Creating memo {} ===", i);
        let create_request = create_tool_request(
            i + 1,
            "memo_create",
            json!({
                "title": format!("List Test Memo {}", i),
                "content": format!("Content for memo {}", i)
            }),
        );
        send_request(&mut stdin, create_request).unwrap();
        let response = read_response(&mut reader).unwrap();
        println!("=== Created memo {}: {} ===", i, response["result"]["content"][0]["text"].as_str().unwrap_or(""));
    }

    println!("=== All 3 memos created, now listing ===");

    // List memos again
    let list_request = create_tool_request(5, "memo_list", json!({}));
    send_request(&mut stdin, list_request).unwrap();
    let list_response = read_response(&mut reader).unwrap();

    assert!(list_response.get("error").is_none());
    let result = &list_response["result"];
    let text = result["content"][0]["text"].as_str().unwrap();
    println!("DEBUG: memo_list text with 3 memos = '{}'", text);
    println!("DEBUG: looking for 'Found 3 memos' in above text");
    assert!(text.contains("Found 3 memos"));
    assert!(text.contains("List Test Memo 1"));
    assert!(text.contains("List Test Memo 2"));
    assert!(text.contains("List Test Memo 3"));
    println!("=== test_mcp_memo_list PASSED ===");
}

/// Test memo search functionality is disabled
#[tokio::test]
#[serial]
async fn test_mcp_memo_search_disabled() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.child.stdin.take().unwrap();
    let stdout = server.child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    // Attempt to use the memo_search tool - should fail with "Unknown tool" error
    let search_request = create_tool_request(
        1,
        "memo_search",
        json!({
            "query": "test"
        }),
    );
    send_request(&mut stdin, search_request).unwrap();
    let search_response = read_response(&mut reader).unwrap();

    // Verify the search tool is not available
    assert!(search_response.get("error").is_some());
    let error = &search_response["error"];
    let error_message = error["message"].as_str().unwrap();
    assert!(error_message.contains("Unknown tool: memo_search"));
}

/// Test memo search case insensitivity is disabled
#[tokio::test]
#[serial]
async fn test_mcp_memo_search_case_insensitive_disabled() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.child.stdin.take().unwrap();
    let stdout = server.child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    // Attempt to use the memo_search tool - should fail with "Unknown tool" error
    let search_request = create_tool_request(
        1,
        "memo_search",
        json!({
            "query": "CamelCase"
        }),
    );
    send_request(&mut stdin, search_request).unwrap();
    let response = read_response(&mut reader).unwrap();

    // Verify the search tool is not available
    assert!(response.get("error").is_some());
    let error = &response["error"];
    let error_message = error["message"].as_str().unwrap();
    assert!(error_message.contains("Unknown tool: memo_search"));
}

/// Test memo get all context via MCP
#[tokio::test]
#[serial]
async fn test_mcp_memo_get_all_context() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.child.stdin.take().unwrap();
    let stdout = server.child.stdout.take().unwrap();
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
    println!("DEBUG get_all_context: context_text = '{}'", context_text);
    println!("DEBUG get_all_context: full response = {}", serde_json::to_string_pretty(&empty_response).unwrap());
    
    if !context_text.contains("No memos available") && !context_text.contains("Found 0 memos") {
        panic!("Expected 'No memos available' or 'Found 0 memos', but got: '{}'", context_text);
    }

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

    let mut stdin = server.child.stdin.take().unwrap();
    let stdout = server.child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize MCP connection
    initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

    // Clean up any existing memos to ensure clean test state
    cleanup_all_memos(&mut stdin, &mut reader).unwrap();

    // Create a large memo (100KB content)
    let large_content = "x".repeat(100_000);
    let unique_title = format!("Large Content Memo {}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    let create_request = create_tool_request(
        1,
        "memo_create",
        json!({
            "title": unique_title.clone(),
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

    // Clean up the memo we created (use title as ID)
    let delete_request = create_tool_request(
        3,
        "memo_delete",
        json!({
            "id": unique_title
        }),
    );

    send_request(&mut stdin, delete_request).unwrap();
    let delete_response = read_response(&mut reader).unwrap();
    assert!(delete_response.get("error").is_none());
}

/// Test MCP error handling for malformed requests
#[tokio::test]
#[serial]
async fn test_mcp_memo_malformed_requests() {
    let mut server = start_mcp_server().unwrap();
    wait_for_server_ready().await;

    let mut stdin = server.child.stdin.take().unwrap();
    let stdout = server.child.stdout.take().unwrap();
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

    let mut stdin = server.child.stdin.take().unwrap();
    let stdout = server.child.stdout.take().unwrap();
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
    // Look for pattern "with ID: <ID>" (now uses title as ID)
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
    #[serial]
    async fn test_mcp_memo_performance_operations() {
        let mut server = start_mcp_server().unwrap();
        wait_for_server_ready().await;

        let mut stdin = server.child.stdin.take().unwrap();
        let stdout = server.child.stdout.take().unwrap();
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
        println!("DEBUG performance test: text = '{}'", text);
        println!("DEBUG performance test: full response = {}", serde_json::to_string_pretty(&list_response).unwrap());
        assert!(text.contains("No memos found") || text.contains("Found 0 memos"), 
                "Expected empty memo list, but got: '{}'", text);
    }

    /// Performance test: Search performance is disabled
    #[tokio::test]
    #[serial]
    async fn test_mcp_memo_search_performance_disabled() {
        let mut server = start_mcp_server().unwrap();
        wait_for_server_ready().await;

        let mut stdin = server.child.stdin.take().unwrap();
        let stdout = server.child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);

        // Initialize MCP connection
        initialize_mcp_connection(&mut stdin, &mut reader).unwrap();

        // Attempt to use the memo_search tool - should fail with "Unknown tool" error
        let search_request = create_tool_request(
            1,
            "memo_search",
            json!({
                "query": "performance"
            }),
        );
        send_request(&mut stdin, search_request).unwrap();
        let response = read_response(&mut reader).unwrap();

        // Verify the search tool is not available
        assert!(response.get("error").is_some());
        let error = &response["error"];
        let error_message = error["message"].as_str().unwrap();
        assert!(error_message.contains("Unknown tool: memo_search"));
    }
}
