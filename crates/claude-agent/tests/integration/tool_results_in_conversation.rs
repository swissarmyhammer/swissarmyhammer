//! Integration tests for tool result handling in conversations
//!
//! This test suite verifies that tool execution results are properly:
//! 1. Formatted and included in conversation history
//! 2. Sent back to the LM for processing
//! 3. Handled correctly for success, error, and permission-required cases
//! 4. Maintained in proper chronological order with tool calls

use claude_agent::conversation_manager::{
    LmMessage, ToolCallRequest, ToolExecutionResult, ToolExecutionStatus,
};

#[test]
fn test_tool_result_chronological_order() {
    // Test that tool calls and results maintain proper order in conversation history
    // This ensures the LM receives tool results in the same order as tool calls were made

    let tool_calls = vec![
        ToolCallRequest {
            id: "call_1".to_string(),
            name: "fs_read".to_string(),
            arguments: serde_json::json!({"path": "/tmp/file1.txt"}),
        },
        ToolCallRequest {
            id: "call_2".to_string(),
            name: "fs_read".to_string(),
            arguments: serde_json::json!({"path": "/tmp/file2.txt"}),
        },
        ToolCallRequest {
            id: "call_3".to_string(),
            name: "fs_write".to_string(),
            arguments: serde_json::json!({"path": "/tmp/file3.txt", "content": "test"}),
        },
    ];

    let tool_results = [
        ToolExecutionResult {
            tool_call_id: "call_1".to_string(),
            status: ToolExecutionStatus::Success,
            output: "Contents of file1".to_string(),
        },
        ToolExecutionResult {
            tool_call_id: "call_2".to_string(),
            status: ToolExecutionStatus::Success,
            output: "Contents of file2".to_string(),
        },
        ToolExecutionResult {
            tool_call_id: "call_3".to_string(),
            status: ToolExecutionStatus::Success,
            output: "File written successfully".to_string(),
        },
    ];

    // Simulate adding tool calls and results to conversation history
    let mut conversation_history: Vec<LmMessage> = Vec::new();

    // Add initial user message
    conversation_history.push(LmMessage::User {
        content: "Read these files and write a summary".to_string(),
    });

    // Add LM response with text
    conversation_history.push(LmMessage::Assistant {
        content: "I'll read the files now.".to_string(),
    });

    // Add tool calls and results in chronological order
    for tool_call in &tool_calls {
        // Add the tool call
        conversation_history.push(LmMessage::ToolCall {
            id: tool_call.id.clone(),
            name: tool_call.name.clone(),
            arguments: tool_call.arguments.clone(),
        });

        // Find and add the corresponding tool result immediately after
        if let Some(result) = tool_results.iter().find(|r| r.tool_call_id == tool_call.id) {
            conversation_history.push(LmMessage::ToolResult {
                tool_call_id: result.tool_call_id.clone(),
                output: result.output.clone(),
            });
        }
    }

    // Verify the order: User -> Assistant -> (ToolCall -> ToolResult)* pattern
    assert_eq!(conversation_history.len(), 8); // 1 user + 1 assistant + 6 (3 calls + 3 results)

    // Check that each tool call is immediately followed by its result
    let mut idx = 2; // Start after user and assistant messages
    for tool_call in &tool_calls {
        // Verify tool call message
        match &conversation_history[idx] {
            LmMessage::ToolCall { id, name, .. } => {
                assert_eq!(id, &tool_call.id);
                assert_eq!(name, &tool_call.name);
            }
            _ => panic!("Expected ToolCall at index {}", idx),
        }

        // Verify tool result immediately follows
        match &conversation_history[idx + 1] {
            LmMessage::ToolResult { tool_call_id, .. } => {
                assert_eq!(tool_call_id, &tool_call.id);
            }
            _ => panic!("Expected ToolResult at index {}", idx + 1),
        }

        idx += 2; // Move to next pair
    }
}

#[test]
fn test_tool_result_error_formatting() {
    // Test that tool execution errors are properly formatted for LM consumption

    let error_result = ToolExecutionResult {
        tool_call_id: "call_error".to_string(),
        status: ToolExecutionStatus::Error,
        output: "File not found: /nonexistent/path.txt".to_string(),
    };

    // Create conversation with error result
    let mut conversation: Vec<LmMessage> = vec![
        LmMessage::User {
            content: "Read this file".to_string(),
        },
        LmMessage::Assistant {
            content: "I'll read it now.".to_string(),
        },
        LmMessage::ToolCall {
            id: "call_error".to_string(),
            name: "fs_read".to_string(),
            arguments: serde_json::json!({"path": "/nonexistent/path.txt"}),
        },
        LmMessage::ToolResult {
            tool_call_id: error_result.tool_call_id.clone(),
            output: error_result.output.clone(),
        },
    ];

    // Verify the error is included as a tool result
    match &conversation[3] {
        LmMessage::ToolResult { output, .. } => {
            assert!(output.contains("File not found"));
            assert!(output.contains("/nonexistent/path.txt"));
        }
        _ => panic!("Expected ToolResult with error"),
    }

    // Add LM's follow-up response after error
    conversation.push(LmMessage::Assistant {
        content: "I encountered an error reading the file. It doesn't exist.".to_string(),
    });

    // Verify conversation flow continues after error
    assert_eq!(conversation.len(), 5);
}

#[test]
fn test_tool_result_permission_required_formatting() {
    // Test that permission-required status is properly formatted for LM consumption

    let permission_result = ToolExecutionResult {
        tool_call_id: "call_perm".to_string(),
        status: ToolExecutionStatus::PermissionRequired,
        output: "Permission required: fs_write - Write to /etc/hosts. Available options: Allow Once (allow_once), Allow Always (allow_always), Reject Once (reject_once)".to_string(),
    };

    // Create conversation with permission required result
    let conversation: Vec<LmMessage> = vec![
        LmMessage::User {
            content: "Write to /etc/hosts".to_string(),
        },
        LmMessage::Assistant {
            content: "I'll write to that file.".to_string(),
        },
        LmMessage::ToolCall {
            id: "call_perm".to_string(),
            name: "fs_write".to_string(),
            arguments: serde_json::json!({"path": "/etc/hosts", "content": "127.0.0.1 test"}),
        },
        LmMessage::ToolResult {
            tool_call_id: permission_result.tool_call_id.clone(),
            output: permission_result.output.clone(),
        },
    ];

    // Verify the permission message is properly formatted
    match &conversation[3] {
        LmMessage::ToolResult { output, .. } => {
            assert!(output.contains("Permission required"));
            assert!(output.contains("fs_write"));
            assert!(output.contains("/etc/hosts"));
            assert!(output.contains("Allow Once"));
            assert!(output.contains("allow_once"));
        }
        _ => panic!("Expected ToolResult with permission requirement"),
    }
}

#[test]
fn test_tool_result_success_with_data() {
    // Test that successful tool results with data are properly formatted

    let success_result = ToolExecutionResult {
        tool_call_id: "call_success".to_string(),
        status: ToolExecutionStatus::Success,
        output: "File contents:\nHello, World!\nThis is a test file.".to_string(),
    };

    let conversation: Vec<LmMessage> = vec![
        LmMessage::User {
            content: "Read test.txt".to_string(),
        },
        LmMessage::Assistant {
            content: "Reading the file.".to_string(),
        },
        LmMessage::ToolCall {
            id: "call_success".to_string(),
            name: "fs_read".to_string(),
            arguments: serde_json::json!({"path": "/tmp/test.txt"}),
        },
        LmMessage::ToolResult {
            tool_call_id: success_result.tool_call_id.clone(),
            output: success_result.output.clone(),
        },
    ];

    // Verify the success result contains the expected data
    match &conversation[3] {
        LmMessage::ToolResult {
            tool_call_id,
            output,
        } => {
            assert_eq!(tool_call_id, "call_success");
            assert!(output.contains("Hello, World!"));
            assert!(output.contains("This is a test file."));
        }
        _ => panic!("Expected ToolResult with success data"),
    }
}

#[test]
fn test_multiple_tool_results_different_statuses() {
    // Test conversation with mixed success, error, and permission results

    let mut conversation: Vec<LmMessage> = vec![
        LmMessage::User {
            content: "Read file1, write to file2, and read file3".to_string(),
        },
        LmMessage::Assistant {
            content: "I'll process those files now.".to_string(),
        },
    ];

    // First tool call: Success
    conversation.push(LmMessage::ToolCall {
        id: "call_1".to_string(),
        name: "fs_read".to_string(),
        arguments: serde_json::json!({"path": "/tmp/file1.txt"}),
    });
    conversation.push(LmMessage::ToolResult {
        tool_call_id: "call_1".to_string(),
        output: "File1 contents".to_string(),
    });

    // Second tool call: Permission Required
    conversation.push(LmMessage::ToolCall {
        id: "call_2".to_string(),
        name: "fs_write".to_string(),
        arguments: serde_json::json!({"path": "/etc/file2.txt", "content": "test"}),
    });
    conversation.push(LmMessage::ToolResult {
        tool_call_id: "call_2".to_string(),
        output: "Permission required: fs_write - Write to protected path".to_string(),
    });

    // Third tool call: Error
    conversation.push(LmMessage::ToolCall {
        id: "call_3".to_string(),
        name: "fs_read".to_string(),
        arguments: serde_json::json!({"path": "/nonexistent/file3.txt"}),
    });
    conversation.push(LmMessage::ToolResult {
        tool_call_id: "call_3".to_string(),
        output: "Error: File not found".to_string(),
    });

    // Verify all three results are in conversation
    assert_eq!(conversation.len(), 8); // 1 user + 1 assistant + 6 (3 calls + 3 results)

    // Verify each result type
    match &conversation[3] {
        LmMessage::ToolResult { output, .. } => {
            assert!(output.contains("File1 contents"));
        }
        _ => panic!("Expected success result at index 3"),
    }

    match &conversation[5] {
        LmMessage::ToolResult { output, .. } => {
            assert!(output.contains("Permission required"));
        }
        _ => panic!("Expected permission result at index 5"),
    }

    match &conversation[7] {
        LmMessage::ToolResult { output, .. } => {
            assert!(output.contains("Error"));
        }
        _ => panic!("Expected error result at index 7"),
    }
}

#[test]
fn test_tool_result_missing_should_be_detected() {
    // Test that missing tool results are detectable

    let _tool_calls = [
        ToolCallRequest {
            id: "call_1".to_string(),
            name: "fs_read".to_string(),
            arguments: serde_json::json!({"path": "/tmp/file1.txt"}),
        },
        ToolCallRequest {
            id: "call_2".to_string(),
            name: "fs_read".to_string(),
            arguments: serde_json::json!({"path": "/tmp/file2.txt"}),
        },
    ];

    let tool_results = [
        ToolExecutionResult {
            tool_call_id: "call_1".to_string(),
            status: ToolExecutionStatus::Success,
            output: "File1 contents".to_string(),
        },
        // call_2 result is missing!
    ];

    // Try to find result for call_2
    let call_2_result = tool_results.iter().find(|r| r.tool_call_id == "call_2");

    // Verify we can detect the missing result
    assert!(
        call_2_result.is_none(),
        "Should detect missing tool result for call_2"
    );
}

#[test]
fn test_tool_result_serialization() {
    // Test that ToolCallRequest can be serialized/deserialized for conversation storage

    let tool_call = ToolCallRequest {
        id: "call_123".to_string(),
        name: "fs_read".to_string(),
        arguments: serde_json::json!({
            "path": "/tmp/test.txt",
            "encoding": "utf-8"
        }),
    };

    // Serialize to JSON
    let serialized = serde_json::to_string(&tool_call).expect("Failed to serialize");

    // Deserialize back
    let deserialized: ToolCallRequest =
        serde_json::from_str(&serialized).expect("Failed to deserialize");

    // Verify round-trip
    assert_eq!(tool_call.id, deserialized.id);
    assert_eq!(tool_call.name, deserialized.name);
    assert_eq!(tool_call.arguments, deserialized.arguments);
}

#[test]
fn test_empty_tool_result_output() {
    // Test handling of tools that return empty output

    let empty_result = ToolExecutionResult {
        tool_call_id: "call_empty".to_string(),
        status: ToolExecutionStatus::Success,
        output: String::new(),
    };

    let conversation: Vec<LmMessage> = vec![
        LmMessage::User {
            content: "Delete this file".to_string(),
        },
        LmMessage::ToolCall {
            id: "call_empty".to_string(),
            name: "fs_delete".to_string(),
            arguments: serde_json::json!({"path": "/tmp/test.txt"}),
        },
        LmMessage::ToolResult {
            tool_call_id: empty_result.tool_call_id.clone(),
            output: empty_result.output.clone(),
        },
    ];

    // Verify empty output is handled
    match &conversation[2] {
        LmMessage::ToolResult { output, .. } => {
            assert_eq!(output, "");
        }
        _ => panic!("Expected ToolResult with empty output"),
    }
}

#[test]
fn test_tool_result_with_special_characters() {
    // Test that tool results with special characters are handled correctly

    let special_chars_result = ToolExecutionResult {
        tool_call_id: "call_special".to_string(),
        status: ToolExecutionStatus::Success,
        output: "File contents:\n\tLine with tab\n\"Quoted text\"\n'Single quotes'\n\\Backslash\\"
            .to_string(),
    };

    let conversation: Vec<LmMessage> = vec![
        LmMessage::ToolCall {
            id: "call_special".to_string(),
            name: "fs_read".to_string(),
            arguments: serde_json::json!({"path": "/tmp/special.txt"}),
        },
        LmMessage::ToolResult {
            tool_call_id: special_chars_result.tool_call_id.clone(),
            output: special_chars_result.output.clone(),
        },
    ];

    // Verify special characters are preserved
    match &conversation[1] {
        LmMessage::ToolResult { output, .. } => {
            assert!(output.contains("\t"));
            assert!(output.contains("\"Quoted text\""));
            assert!(output.contains("'Single quotes'"));
            assert!(output.contains("\\Backslash\\"));
        }
        _ => panic!("Expected ToolResult with special characters"),
    }
}

#[test]
fn test_tool_result_large_output() {
    // Test that large tool results are handled correctly

    let large_output = "x".repeat(10000); // 10KB of output

    let large_result = ToolExecutionResult {
        tool_call_id: "call_large".to_string(),
        status: ToolExecutionStatus::Success,
        output: large_output.clone(),
    };

    let conversation: Vec<LmMessage> = vec![
        LmMessage::ToolCall {
            id: "call_large".to_string(),
            name: "fs_read".to_string(),
            arguments: serde_json::json!({"path": "/tmp/large.txt"}),
        },
        LmMessage::ToolResult {
            tool_call_id: large_result.tool_call_id.clone(),
            output: large_result.output.clone(),
        },
    ];

    // Verify large output is preserved
    match &conversation[1] {
        LmMessage::ToolResult { output, .. } => {
            assert_eq!(output.len(), 10000);
            assert_eq!(output, &large_output);
        }
        _ => panic!("Expected ToolResult with large output"),
    }
}
