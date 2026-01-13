// sah rule ignore acp/capability-enforcement
//! Integration tests for user approval flow
//!
//! These tests verify the complete end-to-end flow of requesting user approval
//! for tool calls, including:
//! - Permission request generation
//! - User response handling (allow/reject, once/always)
//! - Permission storage and retrieval
//! - Cancellation handling
//!
//! Note: This test file validates that capability enforcement is working correctly
//! in the production code. It is not production code that needs to enforce capabilities.

use claude_agent::permissions::{FilePermissionStorage, PermissionPolicyEngine};
use claude_agent::session::SessionManager;
use claude_agent::tools::{
    InternalToolRequest, PermissionOptionKind, ToolCallHandler, ToolCallResult, ToolPermissions,
};
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;

/// Create a test environment for user approval flow testing
fn create_test_environment() -> (
    Arc<SessionManager>,
    ToolCallHandler,
    agent_client_protocol::SessionId,
    TempDir,
    Arc<PermissionPolicyEngine>,
) {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilePermissionStorage::new(temp_dir.path().to_path_buf());
    let permission_engine = Arc::new(PermissionPolicyEngine::new(Box::new(storage)));

    // Use temp directory for session storage to avoid issues with current working directory
    let session_storage_path = temp_dir.path().join("sessions");
    let session_manager = Arc::new(SessionManager::new().with_storage_path(Some(session_storage_path)));

    // Create permissions with no auto-approved tools to force permission checks
    let permissions = ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec![],
        forbidden_paths: vec![],
    };

    let mut handler = ToolCallHandler::new(
        permissions,
        Arc::clone(&session_manager),
        Arc::clone(&permission_engine),
    );

    // Set client capabilities
    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(true);
    handler.set_client_capabilities(capabilities);

    // Create a session
    let test_dir = TempDir::new().unwrap();
    let internal_session_id = session_manager
        .create_session(test_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    (
        session_manager,
        handler,
        session_id,
        test_dir,
        permission_engine,
    )
}

/// Create a test environment with specific client capabilities
fn create_test_environment_with_capabilities(
    read_capability: bool,
    write_capability: bool,
    terminal_capability: bool,
) -> (
    Arc<SessionManager>,
    ToolCallHandler,
    agent_client_protocol::SessionId,
    TempDir,
    Arc<PermissionPolicyEngine>,
) {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilePermissionStorage::new(temp_dir.path().to_path_buf());
    let permission_engine = Arc::new(PermissionPolicyEngine::new(Box::new(storage)));

    // Use temp directory for session storage to avoid issues with current working directory
    let session_storage_path = temp_dir.path().join("sessions");
    let session_manager = Arc::new(SessionManager::new().with_storage_path(Some(session_storage_path)));

    // Create permissions with no auto-approved tools to force permission checks
    let permissions = ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec![],
        forbidden_paths: vec![],
    };

    let mut handler = ToolCallHandler::new(
        permissions,
        Arc::clone(&session_manager),
        Arc::clone(&permission_engine),
    );

    // Set client capabilities with specific settings
    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(read_capability)
            .write_text_file(write_capability))
        .terminal(terminal_capability);
    handler.set_client_capabilities(capabilities);

    // Create a session
    let test_dir = TempDir::new().unwrap();
    let internal_session_id = session_manager
        .create_session(test_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    (
        session_manager,
        handler,
        session_id,
        test_dir,
        permission_engine,
    )
}

#[tokio::test]
async fn test_user_approval_flow_basic_permission_request() {
    let (_, handler, session_id, test_dir, _) = create_test_environment();

    let test_file = test_dir.path().join("test_write.txt");

    // Request a write operation that requires permission
    let request = InternalToolRequest {
        id: "write-1".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "test content"
        }),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    // Should return PermissionRequired with proper options
    match result {
        ToolCallResult::PermissionRequired(perm_req) => {
            assert_eq!(perm_req.tool_name, "fs_write");
            assert_eq!(perm_req.tool_request_id, "write-1");
            assert!(perm_req.description.contains("Write to file"));

            // Verify that all expected option kinds are present
            assert!(
                perm_req.options.len() >= 2,
                "Should have at least allow and reject options"
            );

            let has_allow = perm_req.options.iter().any(|o| {
                matches!(
                    o.kind,
                    PermissionOptionKind::AllowOnce | PermissionOptionKind::AllowAlways
                )
            });
            let has_reject = perm_req.options.iter().any(|o| {
                matches!(
                    o.kind,
                    PermissionOptionKind::RejectOnce | PermissionOptionKind::RejectAlways
                )
            });

            assert!(has_allow, "Should have at least one allow option");
            assert!(has_reject, "Should have at least one reject option");
        }
        _ => panic!("Expected PermissionRequired for fs_write"),
    }
}

#[tokio::test]
async fn test_user_approval_flow_allow_once_then_requires_permission_again() {
    let (_, handler, session_id, test_dir, engine) = create_test_environment();

    // Simulate user approving with "allow-once"
    engine
        .store_permission_decision(
            "fs_write",
            claude_agent::permissions::PermissionDecision::Allow,
            None,
        )
        .await
        .unwrap();

    let test_file = test_dir.path().join("test_write_once.txt");

    // First request should succeed due to stored allow-once decision
    let request1 = InternalToolRequest {
        id: "write-1".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "first write"
        }),
    };

    let result1 = handler
        .handle_tool_request(&session_id, request1)
        .await
        .unwrap();

    match result1 {
        ToolCallResult::Success(msg) => {
            assert!(msg.contains("Successfully wrote"));
        }
        _ => panic!("First write should succeed with allow-once permission"),
    }

    // Second request should require permission again (allow-once doesn't persist)
    let test_file2 = test_dir.path().join("test_write_twice.txt");
    let request2 = InternalToolRequest {
        id: "write-2".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file2.to_string_lossy(),
            "content": "second write"
        }),
    };

    let result2 = handler
        .handle_tool_request(&session_id, request2)
        .await
        .unwrap();

    match result2 {
        ToolCallResult::PermissionRequired(_) => {
            // Expected - allow-once should not persist
        }
        _ => panic!("Second write should require permission again after allow-once"),
    }
}

#[tokio::test]
async fn test_user_approval_flow_allow_always_persists() {
    let (_, handler, session_id, test_dir, engine) = create_test_environment();

    // Simulate user approving with "allow-always"
    engine
        .store_permission_decision(
            "fs_write",
            claude_agent::permissions::PermissionDecision::AllowAlways,
            None,
        )
        .await
        .unwrap();

    // Multiple requests should all succeed without requiring permission again
    for i in 0..3 {
        let test_file = test_dir.path().join(format!("test_write_{}.txt", i));
        let request = InternalToolRequest {
            id: format!("write-{}", i),
            name: "fs_write".to_string(),
            arguments: json!({
                "path": test_file.to_string_lossy(),
                "content": format!("write {}", i)
            }),
        };

        let result = handler
            .handle_tool_request(&session_id, request)
            .await
            .unwrap();

        match result {
            ToolCallResult::Success(msg) => {
                assert!(msg.contains("Successfully wrote"));
            }
            _ => panic!("Request {} should succeed with allow-always permission", i),
        }
    }
}

#[tokio::test]
async fn test_user_approval_flow_reject_once() {
    let (_, handler, session_id, test_dir, engine) = create_test_environment();

    // Simulate user rejecting with "reject-once"
    engine
        .store_permission_decision(
            "fs_write",
            claude_agent::permissions::PermissionDecision::Deny,
            None,
        )
        .await
        .unwrap();

    let test_file = test_dir.path().join("test_reject_once.txt");

    // First request should be denied
    let request1 = InternalToolRequest {
        id: "write-1".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "should be denied"
        }),
    };

    let result1 = handler
        .handle_tool_request(&session_id, request1)
        .await
        .unwrap();

    match result1 {
        ToolCallResult::Error(msg) => {
            assert!(
                msg.to_lowercase().contains("denied") || msg.to_lowercase().contains("rejected")
            );
        }
        _ => panic!("First write should be denied with reject-once"),
    }

    // Second request should require permission again (reject-once doesn't persist)
    let test_file2 = test_dir.path().join("test_reject_twice.txt");
    let request2 = InternalToolRequest {
        id: "write-2".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file2.to_string_lossy(),
            "content": "second attempt"
        }),
    };

    let result2 = handler
        .handle_tool_request(&session_id, request2)
        .await
        .unwrap();

    match result2 {
        ToolCallResult::PermissionRequired(_) => {
            // Expected - reject-once should not persist
        }
        _ => panic!("Second write should require permission again after reject-once"),
    }
}

#[tokio::test]
async fn test_user_approval_flow_reject_always_persists() {
    let (_, handler, session_id, test_dir, engine) = create_test_environment();

    // Simulate user rejecting with "reject-always"
    engine
        .store_permission_decision(
            "fs_write",
            claude_agent::permissions::PermissionDecision::DenyAlways,
            None,
        )
        .await
        .unwrap();

    // Multiple requests should all be denied without requiring permission again
    for i in 0..3 {
        let test_file = test_dir.path().join(format!("test_reject_{}.txt", i));
        let request = InternalToolRequest {
            id: format!("write-{}", i),
            name: "fs_write".to_string(),
            arguments: json!({
                "path": test_file.to_string_lossy(),
                "content": format!("should be denied {}", i)
            }),
        };

        let result = handler
            .handle_tool_request(&session_id, request)
            .await
            .unwrap();

        match result {
            ToolCallResult::Error(msg) => {
                assert!(
                    msg.to_lowercase().contains("denied")
                        || msg.to_lowercase().contains("rejected")
                );
            }
            _ => panic!(
                "Request {} should be denied with reject-always permission",
                i
            ),
        }
    }
}

#[tokio::test]
async fn test_user_approval_flow_different_tools_independent_permissions() {
    let (_, handler, session_id, test_dir, engine) = create_test_environment();

    // Allow fs_write always
    engine
        .store_permission_decision(
            "fs_write",
            claude_agent::permissions::PermissionDecision::AllowAlways,
            None,
        )
        .await
        .unwrap();

    // Reject terminal always
    engine
        .store_permission_decision(
            "terminal_create",
            claude_agent::permissions::PermissionDecision::DenyAlways,
            None,
        )
        .await
        .unwrap();

    // fs_write should succeed
    let test_file = test_dir.path().join("test_write.txt");
    let write_request = InternalToolRequest {
        id: "write-1".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "allowed"
        }),
    };

    let write_result = handler
        .handle_tool_request(&session_id, write_request)
        .await
        .unwrap();

    assert!(
        matches!(write_result, ToolCallResult::Success(_)),
        "fs_write should succeed with allow-always"
    );

    // terminal_create should be denied
    let terminal_request = InternalToolRequest {
        id: "terminal-1".to_string(),
        name: "terminal_create".to_string(),
        arguments: json!({}),
    };

    let terminal_result = handler
        .handle_tool_request(&session_id, terminal_request)
        .await
        .unwrap();

    assert!(
        matches!(terminal_result, ToolCallResult::Error(_)),
        "terminal_create should be denied with reject-always"
    );
}

#[tokio::test]
async fn test_user_approval_flow_permission_options_vary_by_risk_level() {
    let (_, handler, session_id, test_dir, _) = create_test_environment();

    // Test medium-risk tool (fs_write) - should have allow-always option
    let test_file = test_dir.path().join("medium_risk.txt");
    let medium_risk_request = InternalToolRequest {
        id: "write-1".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "test"
        }),
    };

    let medium_result = handler
        .handle_tool_request(&session_id, medium_risk_request)
        .await
        .unwrap();

    match medium_result {
        ToolCallResult::PermissionRequired(perm_req) => {
            let has_allow_always = perm_req
                .options
                .iter()
                .any(|o| o.kind == PermissionOptionKind::AllowAlways);
            assert!(
                has_allow_always,
                "Medium-risk fs_write should offer allow-always option"
            );
        }
        _ => panic!("Expected PermissionRequired for fs_write"),
    }

    // Test high-risk tool (terminal_create) - should NOT have allow-always option
    let high_risk_request = InternalToolRequest {
        id: "terminal-1".to_string(),
        name: "terminal_create".to_string(),
        arguments: json!({}),
    };

    let high_result = handler
        .handle_tool_request(&session_id, high_risk_request)
        .await
        .unwrap();

    match high_result {
        ToolCallResult::PermissionRequired(perm_req) => {
            let has_allow_always = perm_req
                .options
                .iter()
                .any(|o| o.kind == PermissionOptionKind::AllowAlways);
            assert!(
                !has_allow_always,
                "High-risk terminal_create should NOT offer allow-always option"
            );
        }
        _ => panic!("Expected PermissionRequired for terminal_create"),
    }
}

#[tokio::test]
async fn test_user_approval_flow_wildcard_permissions() {
    let (_, handler, session_id, test_dir, engine) = create_test_environment();

    // Store a wildcard permission for all fs_* tools
    engine
        .store_permission_decision(
            "fs_*",
            claude_agent::permissions::PermissionDecision::AllowAlways,
            None,
        )
        .await
        .unwrap();

    // Both fs_read and fs_write should succeed
    let read_file = test_dir.path().join("read_test.txt");
    tokio::fs::write(&read_file, "test content").await.unwrap();

    let read_request = InternalToolRequest {
        id: "read-1".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": read_file.to_string_lossy()
        }),
    };

    let read_result = handler
        .handle_tool_request(&session_id, read_request)
        .await
        .unwrap();

    assert!(
        matches!(read_result, ToolCallResult::Success(_)),
        "fs_read should succeed with fs_* wildcard permission"
    );

    let write_file = test_dir.path().join("write_test.txt");
    let write_request = InternalToolRequest {
        id: "write-1".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": write_file.to_string_lossy(),
            "content": "test content"
        }),
    };

    let write_result = handler
        .handle_tool_request(&session_id, write_request)
        .await
        .unwrap();

    assert!(
        matches!(write_result, ToolCallResult::Success(_)),
        "fs_write should succeed with fs_* wildcard permission"
    );
}

#[tokio::test]
async fn test_user_approval_flow_permission_cleared() {
    let (_, handler, session_id, test_dir, engine) = create_test_environment();

    // Store allow-always permission
    engine
        .store_permission_decision(
            "fs_write",
            claude_agent::permissions::PermissionDecision::AllowAlways,
            None,
        )
        .await
        .unwrap();

    // First request should succeed
    let test_file1 = test_dir.path().join("before_clear.txt");
    let request1 = InternalToolRequest {
        id: "write-1".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file1.to_string_lossy(),
            "content": "before clear"
        }),
    };

    let result1 = handler
        .handle_tool_request(&session_id, request1)
        .await
        .unwrap();

    assert!(
        matches!(result1, ToolCallResult::Success(_)),
        "Should succeed with stored permission"
    );

    // Clear the permission
    engine.remove_permission("fs_write").await.unwrap();

    // Second request should require permission again
    let test_file2 = test_dir.path().join("after_clear.txt");
    let request2 = InternalToolRequest {
        id: "write-2".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file2.to_string_lossy(),
            "content": "after clear"
        }),
    };

    let result2 = handler
        .handle_tool_request(&session_id, request2)
        .await
        .unwrap();

    assert!(
        matches!(result2, ToolCallResult::PermissionRequired(_)),
        "Should require permission after clearing stored permission"
    );
}

// ACP Capability Enforcement Tests

#[tokio::test]
async fn test_fs_read_requires_capability() {
    let (_, handler, session_id, test_dir, engine) = create_test_environment_with_capabilities(
        false, // read_text_file disabled
        true,  // write_text_file enabled
        true,  // terminal enabled
    );

    // Allow fs_read to bypass permission checks
    engine
        .store_permission_decision(
            "fs_read",
            claude_agent::permissions::PermissionDecision::AllowAlways,
            None,
        )
        .await
        .unwrap();

    let test_file = test_dir.path().join("test_read.txt");
    tokio::fs::write(&test_file, "test content").await.unwrap();

    let request = InternalToolRequest {
        id: "read-1".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy()
        }),
    };

    let result = handler.handle_tool_request(&session_id, request).await;

    match result {
        Ok(ToolCallResult::Error(msg)) => {
            assert!(
                msg.contains("not declare fs.read_text_file capability")
                    || msg.contains("capability"),
                "Error should indicate missing read capability: {}",
                msg
            );
        }
        Ok(other) => panic!(
            "Expected error for missing read capability, got: {:?}",
            other
        ),
        Err(e) => {
            // Also accept errors from the agent
            assert!(
                e.to_string().contains("capability")
                    || e.to_string().contains("not declare fs.read_text_file"),
                "Error should indicate missing capability: {}",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_fs_write_requires_capability() {
    let (_, handler, session_id, test_dir, engine) = create_test_environment_with_capabilities(
        true,  // read_text_file enabled
        false, // write_text_file disabled
        true,  // terminal enabled
    );

    // Allow fs_write to bypass permission checks
    engine
        .store_permission_decision(
            "fs_write",
            claude_agent::permissions::PermissionDecision::AllowAlways,
            None,
        )
        .await
        .unwrap();

    let test_file = test_dir.path().join("test_write.txt");

    let request = InternalToolRequest {
        id: "write-1".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "test content"
        }),
    };

    let result = handler.handle_tool_request(&session_id, request).await;

    match result {
        Ok(ToolCallResult::Error(msg)) => {
            assert!(
                msg.contains("not declare fs.write_text_file capability")
                    || msg.contains("capability"),
                "Error should indicate missing write capability: {}",
                msg
            );
        }
        Ok(other) => panic!(
            "Expected error for missing write capability, got: {:?}",
            other
        ),
        Err(e) => {
            // Also accept errors from the agent
            assert!(
                e.to_string().contains("capability")
                    || e.to_string().contains("not declare fs.write_text_file"),
                "Error should indicate missing capability: {}",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_terminal_create_requires_capability() {
    let (_, handler, session_id, _, engine) = create_test_environment_with_capabilities(
        true,  // read_text_file enabled
        true,  // write_text_file enabled
        false, // terminal disabled
    );

    // Allow terminal_create to bypass permission checks
    engine
        .store_permission_decision(
            "terminal_create",
            claude_agent::permissions::PermissionDecision::AllowAlways,
            None,
        )
        .await
        .unwrap();

    let request = InternalToolRequest {
        id: "terminal-1".to_string(),
        name: "terminal_create".to_string(),
        arguments: json!({}),
    };

    let result = handler.handle_tool_request(&session_id, request).await;

    match result {
        Ok(ToolCallResult::Error(msg)) => {
            assert!(
                msg.contains("does not support terminal capability")
                    || msg.contains("terminal capability")
                    || msg.contains("clientCapabilities.terminal"),
                "Error should indicate missing terminal capability: {}",
                msg
            );
        }
        Ok(other) => panic!(
            "Expected error for missing terminal capability, got: {:?}",
            other
        ),
        Err(e) => {
            // Also accept errors from the agent
            assert!(
                e.to_string().contains("terminal capability")
                    || e.to_string().contains("clientCapabilities.terminal"),
                "Error should indicate missing capability: {}",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_terminal_write_requires_capability() {
    let (_, handler, session_id, _, engine) = create_test_environment_with_capabilities(
        true,  // read_text_file enabled
        true,  // write_text_file enabled
        false, // terminal disabled
    );

    // Allow terminal_write to bypass permission checks
    engine
        .store_permission_decision(
            "terminal_write",
            claude_agent::permissions::PermissionDecision::AllowAlways,
            None,
        )
        .await
        .unwrap();

    let request = InternalToolRequest {
        id: "terminal-1".to_string(),
        name: "terminal_write".to_string(),
        arguments: json!({
            "terminal_id": "term_test123",
            "command": "echo test"
        }),
    };

    let result = handler.handle_tool_request(&session_id, request).await;

    match result {
        Ok(ToolCallResult::Error(msg)) => {
            assert!(
                msg.contains("does not support terminal capability")
                    || msg.contains("terminal capability")
                    || msg.contains("clientCapabilities.terminal"),
                "Error should indicate missing terminal capability: {}",
                msg
            );
        }
        Ok(other) => panic!(
            "Expected error for missing terminal capability, got: {:?}",
            other
        ),
        Err(e) => {
            // Also accept errors from the agent
            assert!(
                e.to_string().contains("terminal capability")
                    || e.to_string().contains("clientCapabilities.terminal"),
                "Error should indicate missing capability: {}",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_multiple_capabilities_disabled() {
    let (_, handler, session_id, test_dir, engine) = create_test_environment_with_capabilities(
        false, // read_text_file disabled
        false, // write_text_file disabled
        false, // terminal disabled
    );

    // Allow all operations to bypass permission checks
    engine
        .store_permission_decision(
            "fs_*",
            claude_agent::permissions::PermissionDecision::AllowAlways,
            None,
        )
        .await
        .unwrap();
    engine
        .store_permission_decision(
            "terminal_*",
            claude_agent::permissions::PermissionDecision::AllowAlways,
            None,
        )
        .await
        .unwrap();

    let test_file = test_dir.path().join("test.txt");

    // Test fs_read fails
    let read_request = InternalToolRequest {
        id: "read-1".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy()
        }),
    };

    let read_result = handler.handle_tool_request(&session_id, read_request).await;
    assert!(
        matches!(read_result, Ok(ToolCallResult::Error(_)) | Err(_)),
        "fs_read should fail when read capability is disabled"
    );

    // Test fs_write fails
    let write_request = InternalToolRequest {
        id: "write-1".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "test"
        }),
    };

    let write_result = handler
        .handle_tool_request(&session_id, write_request)
        .await;
    assert!(
        matches!(write_result, Ok(ToolCallResult::Error(_)) | Err(_)),
        "fs_write should fail when write capability is disabled"
    );

    // Test terminal_create fails
    let terminal_request = InternalToolRequest {
        id: "terminal-1".to_string(),
        name: "terminal_create".to_string(),
        arguments: json!({}),
    };

    let terminal_result = handler
        .handle_tool_request(&session_id, terminal_request)
        .await;
    assert!(
        matches!(terminal_result, Ok(ToolCallResult::Error(_)) | Err(_)),
        "terminal_create should fail when terminal capability is disabled"
    );
}
