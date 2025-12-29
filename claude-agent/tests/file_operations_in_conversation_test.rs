//! Integration tests for file operations in conversation context
//!
//! This test suite verifies that file operations (read, write, list) work correctly
//! when executed as part of a conversation flow with tool calls and results.
//!
//! Tests cover:
//! 1. File read operations returning content in conversation
//! 2. File write operations succeeding and reporting status
//! 3. Multiple file operations in sequence
//! 4. Error handling for invalid paths
//! 5. Permission-required scenarios
//! 6. File operation results properly formatted for LM consumption
//! 7. ACP client capability enforcement for file operations

use claude_agent::permissions::{FilePermissionStorage, PermissionPolicyEngine};
use claude_agent::session::SessionManager;
use claude_agent::tools::{InternalToolRequest, ToolCallHandler, ToolPermissions};
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;

/// Helper to create a test tool call handler with file permissions
async fn create_test_handler_with_permissions(
    auto_approved: Vec<String>,
    require_permission: Vec<String>,
) -> (ToolCallHandler, TempDir) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let storage = FilePermissionStorage::new(temp_dir.path().to_path_buf());
    let permission_engine = Arc::new(PermissionPolicyEngine::new(Box::new(storage)));

    let permissions = ToolPermissions {
        require_permission_for: require_permission,
        auto_approved,
        forbidden_paths: vec![],
    };

    let session_manager = Arc::new(SessionManager::new());
    let handler = ToolCallHandler::new(permissions, session_manager, permission_engine);

    (handler, temp_dir)
}

/// Helper to create a test tool call handler with file permissions and client capabilities
async fn create_test_handler_with_capabilities(
    auto_approved: Vec<String>,
    require_permission: Vec<String>,
    capabilities: agent_client_protocol::ClientCapabilities,
) -> (ToolCallHandler, TempDir) {
    let (mut handler, temp_dir) =
        create_test_handler_with_permissions(auto_approved, require_permission).await;
    handler.set_client_capabilities(capabilities);
    (handler, temp_dir)
}

/// Helper to create a temporary test file with content
async fn create_temp_file_with_content(dir: &TempDir, filename: &str, content: &str) -> String {
    let file_path = dir.path().join(filename);
    tokio::fs::write(&file_path, content)
        .await
        .expect("Failed to write test file");
    file_path.to_string_lossy().to_string()
}

/// Helper to create client capabilities with specific fs settings
fn create_client_capabilities(
    read_text_file: bool,
    write_text_file: bool,
) -> agent_client_protocol::ClientCapabilities {
    agent_client_protocol::ClientCapabilities {
        fs: agent_client_protocol::FileSystemCapability {
            read_text_file,
            write_text_file,
            meta: None,
        },
        terminal: false,
        meta: None,
    }
}

#[tokio::test]
async fn test_file_read_in_conversation() {
    let capabilities = create_client_capabilities(true, true);
    let (handler, temp_dir) =
        create_test_handler_with_capabilities(vec!["fs_read".to_string()], vec![], capabilities)
            .await;

    // Create a test file
    let test_content = "Hello from test file!\nLine 2\nLine 3";
    let file_path = create_temp_file_with_content(&temp_dir, "test_read.txt", test_content).await;

    // Simulate a file read tool call in conversation
    let tool_request = InternalToolRequest {
        id: "call_read_1".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": file_path
        }),
    };

    let session_id = agent_client_protocol::SessionId::new("test_session_read");
    let result = handler.handle_tool_call(&session_id, tool_request).await;

    // Verify the result contains the file content
    match result {
        Ok(tool_result) => {
            assert_eq!(tool_result.tool_call_id, "call_read_1");
            assert!(
                tool_result.output.contains("Hello from test file!"),
                "Output should contain file content, got: {}",
                tool_result.output
            );
            assert!(
                tool_result.output.contains("Line 2"),
                "Output should contain all lines"
            );
        }
        Err(e) => panic!("File read should succeed, got error: {:?}", e),
    }
}

#[tokio::test]
async fn test_file_read_without_capability() {
    let capabilities = create_client_capabilities(false, true);
    let (handler, temp_dir) =
        create_test_handler_with_capabilities(vec!["fs_read".to_string()], vec![], capabilities)
            .await;

    // Create a test file
    let test_content = "Hello from test file!";
    let file_path = create_temp_file_with_content(&temp_dir, "test_read.txt", test_content).await;

    // Attempt to read file without fs.read_text_file capability
    let tool_request = InternalToolRequest {
        id: "call_read_no_cap".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": file_path
        }),
    };

    let session_id = agent_client_protocol::SessionId::new("test_session_read_no_cap");
    let result = handler.handle_tool_call(&session_id, tool_request).await;

    // Verify operation fails with appropriate error
    match result {
        Ok(tool_result) => {
            assert!(
                tool_result.output.contains("fs.read_text_file")
                    || tool_result.output.contains("capability")
                    || tool_result.output.contains("not available"),
                "Output should indicate capability error, got: {}",
                tool_result.output
            );
        }
        Err(e) => {
            let error_msg = format!("{:?}", e);
            assert!(
                error_msg.contains("fs.read_text_file") || error_msg.contains("capability"),
                "Error should indicate missing capability, got: {}",
                error_msg
            );
        }
    }
}

#[tokio::test]
async fn test_file_write_in_conversation() {
    let capabilities = create_client_capabilities(true, true);
    let (handler, temp_dir) =
        create_test_handler_with_capabilities(vec!["fs_write".to_string()], vec![], capabilities)
            .await;

    let file_path = temp_dir.path().join("test_write.txt");
    let file_path_str = file_path.to_string_lossy().to_string();
    let write_content = "Content written by test";

    // Simulate a file write tool call in conversation
    let tool_request = InternalToolRequest {
        id: "call_write_1".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": file_path_str,
            "content": write_content
        }),
    };

    let session_id = agent_client_protocol::SessionId::new("test_session_write");
    let result = handler.handle_tool_call(&session_id, tool_request).await;

    // Verify the write succeeded
    match result {
        Ok(tool_result) => {
            assert_eq!(tool_result.tool_call_id, "call_write_1");
            // Output should indicate success
            assert!(
                tool_result.output.contains("success") || tool_result.output.contains("written"),
                "Output should indicate success, got: {}",
                tool_result.output
            );
        }
        Err(e) => panic!("File write should succeed, got error: {:?}", e),
    }

    // Verify the file was actually written
    let actual_content = tokio::fs::read_to_string(&file_path)
        .await
        .expect("Should be able to read written file");
    assert_eq!(actual_content, write_content);
}

#[tokio::test]
async fn test_file_write_without_capability() {
    let capabilities = create_client_capabilities(true, false);
    let (handler, temp_dir) =
        create_test_handler_with_capabilities(vec!["fs_write".to_string()], vec![], capabilities)
            .await;

    let file_path = temp_dir.path().join("test_write.txt");
    let file_path_str = file_path.to_string_lossy().to_string();
    let write_content = "Content written by test";

    // Attempt to write file without fs.write_text_file capability
    let tool_request = InternalToolRequest {
        id: "call_write_no_cap".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": file_path_str,
            "content": write_content
        }),
    };

    let session_id = agent_client_protocol::SessionId::new("test_session_write_no_cap");
    let result = handler.handle_tool_call(&session_id, tool_request).await;

    // Verify operation fails with appropriate error
    match result {
        Ok(tool_result) => {
            assert!(
                tool_result.output.contains("fs.write_text_file")
                    || tool_result.output.contains("capability")
                    || tool_result.output.contains("not available"),
                "Output should indicate capability error, got: {}",
                tool_result.output
            );
        }
        Err(e) => {
            let error_msg = format!("{:?}", e);
            assert!(
                error_msg.contains("fs.write_text_file") || error_msg.contains("capability"),
                "Error should indicate missing capability, got: {}",
                error_msg
            );
        }
    }

    // Verify the file was NOT written
    assert!(
        !file_path.exists(),
        "File should not exist after failed write"
    );
}

#[tokio::test]
async fn test_multiple_file_operations_in_sequence() {
    let capabilities = create_client_capabilities(true, true);
    let (handler, temp_dir) = create_test_handler_with_capabilities(
        vec!["fs_read".to_string(), "fs_write".to_string()],
        vec![],
        capabilities,
    )
    .await;

    let session_id = agent_client_protocol::SessionId::new("test_session_sequence");

    // Step 1: Write first file
    let file1_path = temp_dir.path().join("file1.txt");
    let file1_path_str = file1_path.to_string_lossy().to_string();

    let write1 = InternalToolRequest {
        id: "call_write_1".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": file1_path_str,
            "content": "First file content"
        }),
    };

    let result1 = handler.handle_tool_call(&session_id, write1).await;
    assert!(result1.is_ok(), "First write should succeed");

    // Step 2: Write second file
    let file2_path = temp_dir.path().join("file2.txt");
    let file2_path_str = file2_path.to_string_lossy().to_string();

    let write2 = InternalToolRequest {
        id: "call_write_2".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": file2_path_str,
            "content": "Second file content"
        }),
    };

    let result2 = handler.handle_tool_call(&session_id, write2).await;
    assert!(result2.is_ok(), "Second write should succeed");

    // Step 3: Read first file
    let read1 = InternalToolRequest {
        id: "call_read_1".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": file1_path_str
        }),
    };

    let result3 = handler.handle_tool_call(&session_id, read1).await;
    match result3 {
        Ok(tool_result) => {
            assert!(
                tool_result.output.contains("First file content"),
                "Should read first file content"
            );
        }
        Err(e) => panic!("Read should succeed, got: {:?}", e),
    }

    // Step 4: Read second file
    let read2 = InternalToolRequest {
        id: "call_read_2".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": file2_path_str
        }),
    };

    let result4 = handler.handle_tool_call(&session_id, read2).await;
    match result4 {
        Ok(tool_result) => {
            assert!(
                tool_result.output.contains("Second file content"),
                "Should read second file content"
            );
        }
        Err(e) => panic!("Read should succeed, got: {:?}", e),
    }
}

#[tokio::test]
async fn test_multiple_file_operations_capability_check() {
    let capabilities = create_client_capabilities(true, true);
    let (handler, temp_dir) = create_test_handler_with_capabilities(
        vec!["fs_read".to_string(), "fs_write".to_string()],
        vec![],
        capabilities,
    )
    .await;

    let session_id = agent_client_protocol::SessionId::new("test_session_cap_check");

    // Write a file
    let file_path = temp_dir.path().join("test.txt");
    let file_path_str = file_path.to_string_lossy().to_string();

    let write_request = InternalToolRequest {
        id: "call_write".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": file_path_str,
            "content": "Test content"
        }),
    };

    let write_result = handler.handle_tool_call(&session_id, write_request).await;
    assert!(
        write_result.is_ok(),
        "Write should succeed with capabilities"
    );

    // Read the file
    let read_request = InternalToolRequest {
        id: "call_read".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": file_path_str
        }),
    };

    let read_result = handler.handle_tool_call(&session_id, read_request).await;
    assert!(read_result.is_ok(), "Read should succeed with capabilities");
}

#[tokio::test]
async fn test_file_read_nonexistent_file_error() {
    let capabilities = create_client_capabilities(true, true);
    let (handler, temp_dir) =
        create_test_handler_with_capabilities(vec!["fs_read".to_string()], vec![], capabilities)
            .await;

    let nonexistent_path = temp_dir.path().join("does_not_exist.txt");
    let nonexistent_path_str = nonexistent_path.to_string_lossy().to_string();

    // Attempt to read nonexistent file
    let tool_request = InternalToolRequest {
        id: "call_read_error".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": nonexistent_path_str
        }),
    };

    let session_id = agent_client_protocol::SessionId::new("test_session_error");
    let result = handler.handle_tool_call(&session_id, tool_request).await;

    // Verify error is returned
    match result {
        Ok(tool_result) => {
            // Error should be in the output as a formatted error message
            assert!(
                tool_result.output.contains("not found")
                    || tool_result.output.contains("error")
                    || tool_result.output.contains("Error"),
                "Output should indicate file not found error, got: {}",
                tool_result.output
            );
        }
        Err(e) => {
            // Error might be returned as Err instead - that's also valid
            let error_msg = format!("{:?}", e);
            assert!(
                error_msg.contains("not found") || error_msg.contains("No such file"),
                "Error should indicate file not found"
            );
        }
    }
}

#[tokio::test]
async fn test_file_write_permission_required() {
    let capabilities = create_client_capabilities(true, true);
    let (handler, temp_dir) =
        create_test_handler_with_capabilities(vec![], vec!["fs_write".to_string()], capabilities)
            .await;

    let file_path = temp_dir.path().join("protected_file.txt");
    let file_path_str = file_path.to_string_lossy().to_string();

    // Attempt to write file that requires permission
    let tool_request = InternalToolRequest {
        id: "call_write_perm".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": file_path_str,
            "content": "Protected content"
        }),
    };

    let session_id = agent_client_protocol::SessionId::new("test_session_perm");
    let result = handler.handle_tool_call(&session_id, tool_request).await;

    // Verify permission is required
    match result {
        Ok(tool_result) => {
            // Result should indicate permission is required
            assert!(
                tool_result.output.contains("permission")
                    || tool_result.output.contains("Permission"),
                "Output should indicate permission required, got: {}",
                tool_result.output
            );
        }
        Err(e) => {
            let error_msg = format!("{:?}", e);
            assert!(
                error_msg.contains("permission") || error_msg.contains("Permission"),
                "Error should indicate permission required"
            );
        }
    }
}

#[tokio::test]
async fn test_file_operations_track_session_history() {
    let capabilities = create_client_capabilities(true, true);
    let (handler, temp_dir) = create_test_handler_with_capabilities(
        vec!["fs_read".to_string(), "fs_write".to_string()],
        vec![],
        capabilities,
    )
    .await;

    let session_id = agent_client_protocol::SessionId::new("test_session_history");

    // Perform multiple file operations
    let file_path = temp_dir.path().join("history_test.txt");
    let file_path_str = file_path.to_string_lossy().to_string();

    // Write operation
    let write_request = InternalToolRequest {
        id: "call_write_hist".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": file_path_str,
            "content": "Test content"
        }),
    };

    let _write_result = handler.handle_tool_call(&session_id, write_request).await;

    // Read operation
    let read_request = InternalToolRequest {
        id: "call_read_hist".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": file_path_str
        }),
    };

    let _read_result = handler.handle_tool_call(&session_id, read_request).await;

    // Get file operations for this session
    let operations = handler.get_file_operations(&session_id).await;

    // Verify both operations are tracked
    assert_eq!(
        operations.len(),
        2,
        "Should track both write and read operations"
    );

    // Verify operation types
    use claude_agent::tools::FileOperationType;
    assert!(
        operations
            .iter()
            .any(|op| op.operation_type == FileOperationType::Write),
        "Should have write operation"
    );
    assert!(
        operations
            .iter()
            .any(|op| op.operation_type == FileOperationType::Read),
        "Should have read operation"
    );
}

#[tokio::test]
async fn test_file_read_with_large_content() {
    let capabilities = create_client_capabilities(true, true);
    let (handler, temp_dir) =
        create_test_handler_with_capabilities(vec!["fs_read".to_string()], vec![], capabilities)
            .await;

    // Create a large file (10KB)
    let large_content = "x".repeat(10_000);
    let file_path =
        create_temp_file_with_content(&temp_dir, "large_file.txt", &large_content).await;

    // Read the large file
    let tool_request = InternalToolRequest {
        id: "call_read_large".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": file_path
        }),
    };

    let session_id = agent_client_protocol::SessionId::new("test_session_large");
    let result = handler.handle_tool_call(&session_id, tool_request).await;

    // Verify large content is read correctly
    match result {
        Ok(tool_result) => {
            assert!(
                tool_result.output.len() >= 10_000,
                "Output should contain full large content, got {} bytes",
                tool_result.output.len()
            );
        }
        Err(e) => panic!("Large file read should succeed, got error: {:?}", e),
    }
}

#[tokio::test]
async fn test_file_operations_with_special_characters() {
    let capabilities = create_client_capabilities(true, true);
    let (handler, temp_dir) = create_test_handler_with_capabilities(
        vec!["fs_read".to_string(), "fs_write".to_string()],
        vec![],
        capabilities,
    )
    .await;

    let session_id = agent_client_protocol::SessionId::new("test_session_special");

    // Content with special characters
    let special_content = "Line 1\n\tTabbed line\n\"Quoted text\"\n'Single quotes'\n\\Backslash\\";
    let file_path = temp_dir.path().join("special_chars.txt");
    let file_path_str = file_path.to_string_lossy().to_string();

    // Write file with special characters
    let write_request = InternalToolRequest {
        id: "call_write_special".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": file_path_str,
            "content": special_content
        }),
    };

    let _write_result = handler.handle_tool_call(&session_id, write_request).await;

    // Read back and verify special characters are preserved
    let read_request = InternalToolRequest {
        id: "call_read_special".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": file_path_str
        }),
    };

    let result = handler.handle_tool_call(&session_id, read_request).await;

    match result {
        Ok(tool_result) => {
            assert!(
                tool_result.output.contains("\t"),
                "Should preserve tab characters"
            );
            assert!(
                tool_result.output.contains("\"Quoted text\""),
                "Should preserve double quotes"
            );
            assert!(
                tool_result.output.contains("'Single quotes'"),
                "Should preserve single quotes"
            );
            assert!(
                tool_result.output.contains("\\"),
                "Should preserve backslashes"
            );
        }
        Err(e) => panic!("Read should succeed, got error: {:?}", e),
    }
}

#[tokio::test]
async fn test_file_write_overwrite_existing() {
    let capabilities = create_client_capabilities(true, true);
    let (handler, temp_dir) = create_test_handler_with_capabilities(
        vec!["fs_read".to_string(), "fs_write".to_string()],
        vec![],
        capabilities,
    )
    .await;

    let session_id = agent_client_protocol::SessionId::new("test_session_overwrite");
    let file_path = temp_dir.path().join("overwrite_test.txt");
    let file_path_str = file_path.to_string_lossy().to_string();

    // Write initial content
    let write1 = InternalToolRequest {
        id: "call_write_initial".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": file_path_str,
            "content": "Initial content"
        }),
    };

    handler
        .handle_tool_call(&session_id, write1)
        .await
        .expect("Initial write should succeed");

    // Overwrite with new content
    let write2 = InternalToolRequest {
        id: "call_write_overwrite".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": file_path_str,
            "content": "Overwritten content"
        }),
    };

    handler
        .handle_tool_call(&session_id, write2)
        .await
        .expect("Overwrite should succeed");

    // Read and verify new content
    let read_request = InternalToolRequest {
        id: "call_read_verify".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": file_path_str
        }),
    };

    let result = handler.handle_tool_call(&session_id, read_request).await;

    match result {
        Ok(tool_result) => {
            assert!(
                tool_result.output.contains("Overwritten content"),
                "Should contain new content"
            );
            assert!(
                !tool_result.output.contains("Initial content"),
                "Should not contain old content"
            );
        }
        Err(e) => panic!("Read should succeed, got error: {:?}", e),
    }
}
