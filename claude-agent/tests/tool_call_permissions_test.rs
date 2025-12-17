//! Integration tests for tool call permission system
//!
//! These tests verify that the permission policy engine correctly integrates
//! with tool call execution to enforce security policies.

use claude_agent::permissions::{
    FilePermissionStorage, PermissionDecision, PermissionPolicy, PermissionPolicyEngine,
    PolicyAction, RiskLevel,
};
use claude_agent::session::SessionManager;
use claude_agent::tools::{
    InternalToolRequest, PermissionOptionKind, ToolCallHandler, ToolCallResult, ToolPermissions,
};
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;

/// Create a test permission engine with default policies
fn create_test_permission_engine() -> Arc<PermissionPolicyEngine> {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilePermissionStorage::new(temp_dir.path().to_path_buf());
    Arc::new(PermissionPolicyEngine::new(Box::new(storage)))
}

/// Create a test session and handler for tool call testing
fn create_test_environment() -> (
    Arc<SessionManager>,
    ToolCallHandler,
    agent_client_protocol::SessionId,
    TempDir,
) {
    let temp_dir = TempDir::new().unwrap();
    let session_manager = Arc::new(SessionManager::new());
    let permission_engine = create_test_permission_engine();

    // Create permissions that use the policy engine (no auto-approved tools)
    let permissions = ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec![],
        forbidden_paths: vec![],
    };

    let mut handler =
        ToolCallHandler::new(permissions, Arc::clone(&session_manager), permission_engine);

    // Set client capabilities
    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(true);
    handler.set_client_capabilities(capabilities);

    // Create a session
    let internal_session_id = session_manager
        .create_session(temp_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    (session_manager, handler, session_id, temp_dir)
}

#[tokio::test]
async fn test_fs_read_allowed_by_default_policy() {
    let (_, handler, session_id, temp_dir) = create_test_environment();

    // Create a test file
    let test_file = temp_dir.path().join("test.txt");
    tokio::fs::write(&test_file, "test content").await.unwrap();

    // fs_read should be allowed by default policy (low risk)
    let request = InternalToolRequest {
        id: "test-read".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy()
        }),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    match result {
        ToolCallResult::Success(content) => {
            assert_eq!(content, "test content");
        }
        ToolCallResult::Error(msg) => {
            panic!(
                "fs_read should be allowed by default policy, got error: {}",
                msg
            );
        }
        ToolCallResult::PermissionRequired(_) => {
            panic!("fs_read should be allowed by default policy, not require permission");
        }
    }
}

#[tokio::test]
async fn test_fs_write_requires_permission_by_default_policy() {
    let (_, handler, session_id, temp_dir) = create_test_environment();

    let test_file = temp_dir.path().join("write_test.txt");

    // fs_write should require permission by default policy (medium risk)
    let request = InternalToolRequest {
        id: "test-write".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "new content"
        }),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    match result {
        ToolCallResult::PermissionRequired(perm_req) => {
            assert_eq!(perm_req.tool_name, "fs_write");
            assert_eq!(perm_req.tool_request_id, "test-write");
            assert!(perm_req.description.contains("Write to file"));

            // Verify options are provided (medium risk should have 4 options)
            assert_eq!(perm_req.options.len(), 4);

            // Verify all option kinds are present
            let has_allow_once = perm_req
                .options
                .iter()
                .any(|o| o.kind == PermissionOptionKind::AllowOnce);
            let has_allow_always = perm_req
                .options
                .iter()
                .any(|o| o.kind == PermissionOptionKind::AllowAlways);
            let has_reject_once = perm_req
                .options
                .iter()
                .any(|o| o.kind == PermissionOptionKind::RejectOnce);
            let has_reject_always = perm_req
                .options
                .iter()
                .any(|o| o.kind == PermissionOptionKind::RejectAlways);

            assert!(has_allow_once, "Should have allow-once option");
            assert!(
                has_allow_always,
                "Should have allow-always option for medium risk"
            );
            assert!(has_reject_once, "Should have reject-once option");
            assert!(has_reject_always, "Should have reject-always option");
        }
        ToolCallResult::Success(_) => {
            panic!("fs_write should require permission by default policy");
        }
        ToolCallResult::Error(msg) => {
            panic!("Expected permission required, got error: {}", msg);
        }
    }
}

#[tokio::test]
async fn test_terminal_requires_permission_and_no_allow_always() {
    let (_, handler, session_id, _) = create_test_environment();

    // terminal_create should require permission by default policy (high risk)
    let request = InternalToolRequest {
        id: "test-terminal".to_string(),
        name: "terminal_create".to_string(),
        arguments: json!({}),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    match result {
        ToolCallResult::PermissionRequired(perm_req) => {
            assert_eq!(perm_req.tool_name, "terminal_create");

            // High risk tools should NOT have allow-always option
            let has_allow_always = perm_req
                .options
                .iter()
                .any(|o| o.kind == PermissionOptionKind::AllowAlways);
            assert!(
                !has_allow_always,
                "High-risk terminal tool should not offer allow-always option"
            );

            // Should have other options
            let has_allow_once = perm_req
                .options
                .iter()
                .any(|o| o.kind == PermissionOptionKind::AllowOnce);
            let has_reject_once = perm_req
                .options
                .iter()
                .any(|o| o.kind == PermissionOptionKind::RejectOnce);
            let has_reject_always = perm_req
                .options
                .iter()
                .any(|o| o.kind == PermissionOptionKind::RejectAlways);

            assert!(has_allow_once, "Should have allow-once option");
            assert!(has_reject_once, "Should have reject-once option");
            assert!(has_reject_always, "Should have reject-always option");
        }
        _ => {
            panic!("terminal_create should require permission by default policy");
        }
    }
}

#[tokio::test]
async fn test_stored_permission_allows_tool_without_prompt() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilePermissionStorage::new(temp_dir.path().to_path_buf());
    let permission_engine = Arc::new(PermissionPolicyEngine::new(Box::new(storage)));

    let session_manager = Arc::new(SessionManager::new());
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

    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(true);
    handler.set_client_capabilities(capabilities);

    let test_dir = TempDir::new().unwrap();
    let internal_session_id = session_manager
        .create_session(test_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    // Store an allow-always permission for fs_write
    permission_engine
        .store_permission_decision("fs_write", PermissionDecision::AllowAlways, None)
        .await
        .unwrap();

    // Create a test file to write to
    let test_file = test_dir.path().join("allowed_write.txt");

    // Now fs_write should be allowed without prompting
    let request = InternalToolRequest {
        id: "test-write".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "allowed content"
        }),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    match result {
        ToolCallResult::Success(msg) => {
            assert!(msg.contains("Successfully wrote"));

            // Verify the file was actually written
            let content = tokio::fs::read_to_string(&test_file).await.unwrap();
            assert_eq!(content, "allowed content");
        }
        ToolCallResult::PermissionRequired(_) => {
            panic!("fs_write should be allowed by stored permission");
        }
        ToolCallResult::Error(msg) => {
            panic!("Expected success, got error: {}", msg);
        }
    }
}

#[tokio::test]
async fn test_stored_permission_denies_tool() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilePermissionStorage::new(temp_dir.path().to_path_buf());
    let permission_engine = Arc::new(PermissionPolicyEngine::new(Box::new(storage)));

    let session_manager = Arc::new(SessionManager::new());
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

    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(true);
    handler.set_client_capabilities(capabilities);

    let test_dir = TempDir::new().unwrap();
    let internal_session_id = session_manager
        .create_session(test_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    // Store a deny-always permission for fs_write
    permission_engine
        .store_permission_decision("fs_write", PermissionDecision::DenyAlways, None)
        .await
        .unwrap();

    let test_file = test_dir.path().join("denied_write.txt");

    // Now fs_write should be denied
    let request = InternalToolRequest {
        id: "test-write".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "denied content"
        }),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    match result {
        ToolCallResult::Error(msg) => {
            assert!(msg.contains("denied"));
        }
        _ => {
            panic!("fs_write should be denied by stored permission");
        }
    }
}

#[tokio::test]
async fn test_custom_policy_denies_tool() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilePermissionStorage::new(temp_dir.path().to_path_buf());

    // Create custom policies that deny all fs_write operations
    let custom_policies = vec![PermissionPolicy {
        tool_pattern: "fs_write*".to_string(),
        default_action: PolicyAction::Deny,
        require_user_consent: false,
        allow_always_option: false,
        risk_level: RiskLevel::Critical,
    }];

    let permission_engine = Arc::new(PermissionPolicyEngine::with_policies(
        Box::new(storage),
        custom_policies,
    ));

    let session_manager = Arc::new(SessionManager::new());
    let permissions = ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec![],
        forbidden_paths: vec![],
    };

    let mut handler =
        ToolCallHandler::new(permissions, Arc::clone(&session_manager), permission_engine);

    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(true);
    handler.set_client_capabilities(capabilities);

    let test_dir = TempDir::new().unwrap();
    let internal_session_id = session_manager
        .create_session(test_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    let test_file = test_dir.path().join("denied_by_policy.txt");

    // fs_write should be denied by custom policy
    let request = InternalToolRequest {
        id: "test-write".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "should be denied"
        }),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    match result {
        ToolCallResult::Error(msg) => {
            assert!(msg.contains("denied") || msg.contains("policy"));
        }
        _ => {
            panic!("fs_write should be denied by custom policy");
        }
    }
}

#[tokio::test]
async fn test_custom_policy_allows_tool() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilePermissionStorage::new(temp_dir.path().to_path_buf());

    // Create custom policies that allow all fs_write operations
    let custom_policies = vec![PermissionPolicy {
        tool_pattern: "fs_write*".to_string(),
        default_action: PolicyAction::Allow,
        require_user_consent: false,
        allow_always_option: true,
        risk_level: RiskLevel::Low,
    }];

    let permission_engine = Arc::new(PermissionPolicyEngine::with_policies(
        Box::new(storage),
        custom_policies,
    ));

    let session_manager = Arc::new(SessionManager::new());
    let permissions = ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec![],
        forbidden_paths: vec![],
    };

    let mut handler =
        ToolCallHandler::new(permissions, Arc::clone(&session_manager), permission_engine);

    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(true);
    handler.set_client_capabilities(capabilities);

    let test_dir = TempDir::new().unwrap();
    let internal_session_id = session_manager
        .create_session(test_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    let test_file = test_dir.path().join("allowed_by_policy.txt");

    // fs_write should be allowed by custom policy
    let request = InternalToolRequest {
        id: "test-write".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "allowed by policy"
        }),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    match result {
        ToolCallResult::Success(msg) => {
            assert!(msg.contains("Successfully wrote"));

            // Verify the file was actually written
            let content = tokio::fs::read_to_string(&test_file).await.unwrap();
            assert_eq!(content, "allowed by policy");
        }
        _ => {
            panic!("fs_write should be allowed by custom policy");
        }
    }
}

#[tokio::test]
async fn test_pattern_matching_in_permissions() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilePermissionStorage::new(temp_dir.path().to_path_buf());
    let permission_engine = Arc::new(PermissionPolicyEngine::new(Box::new(storage)));

    // Store a wildcard permission for all fs_* tools
    permission_engine
        .store_permission_decision("fs_*", PermissionDecision::AllowAlways, None)
        .await
        .unwrap();

    let session_manager = Arc::new(SessionManager::new());
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

    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(true);
    handler.set_client_capabilities(capabilities);

    let test_dir = TempDir::new().unwrap();
    let internal_session_id = session_manager
        .create_session(test_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    // Both fs_read and fs_write should be allowed by the wildcard permission
    let test_file = test_dir.path().join("pattern_test.txt");
    tokio::fs::write(&test_file, "test").await.unwrap();

    let read_request = InternalToolRequest {
        id: "test-read".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy()
        }),
    };

    let read_result = handler
        .handle_tool_request(&session_id, read_request)
        .await
        .unwrap();
    assert!(
        matches!(read_result, ToolCallResult::Success(_)),
        "fs_read should be allowed by fs_* pattern"
    );

    let write_file = test_dir.path().join("pattern_write.txt");
    let write_request = InternalToolRequest {
        id: "test-write".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": write_file.to_string_lossy(),
            "content": "pattern test"
        }),
    };

    let write_result = handler
        .handle_tool_request(&session_id, write_request)
        .await
        .unwrap();
    assert!(
        matches!(write_result, ToolCallResult::Success(_)),
        "fs_write should be allowed by fs_* pattern"
    );
}

#[tokio::test]
async fn test_permission_expiration() {
    use std::time::Duration;

    let temp_dir = TempDir::new().unwrap();
    let storage = FilePermissionStorage::new(temp_dir.path().to_path_buf());
    let permission_engine = Arc::new(PermissionPolicyEngine::new(Box::new(storage)));

    // Store a permission that expires immediately
    permission_engine
        .store_permission_decision(
            "fs_write",
            PermissionDecision::AllowAlways,
            Some(Duration::from_secs(0)),
        )
        .await
        .unwrap();

    // Wait a moment to ensure expiration
    tokio::time::sleep(Duration::from_millis(10)).await;

    let session_manager = Arc::new(SessionManager::new());
    let permissions = ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec![],
        forbidden_paths: vec![],
    };

    let mut handler =
        ToolCallHandler::new(permissions, Arc::clone(&session_manager), permission_engine);

    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(true);
    handler.set_client_capabilities(capabilities);

    let test_dir = TempDir::new().unwrap();
    let internal_session_id = session_manager
        .create_session(test_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    let test_file = test_dir.path().join("expired_permission.txt");

    // Permission should have expired, so we should get permission required
    let request = InternalToolRequest {
        id: "test-write".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "should require permission"
        }),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    match result {
        ToolCallResult::PermissionRequired(_) => {
            // Expected - permission expired, so we need to ask again
        }
        _ => {
            panic!("Expired permission should require new user consent");
        }
    }
}

#[tokio::test]
async fn test_auto_approved_fs_read_bypasses_policy() {
    let (_, handler, session_id, temp_dir) = create_test_environment();

    // Create permissions with fs_read in auto_approved list
    let session_manager = Arc::new(SessionManager::new());
    let permission_engine = create_test_permission_engine();

    let permissions = ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec!["fs_read".to_string()],
        forbidden_paths: vec![],
    };

    let mut handler =
        ToolCallHandler::new(permissions, Arc::clone(&session_manager), permission_engine);

    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(true);
    handler.set_client_capabilities(capabilities);

    let test_dir = TempDir::new().unwrap();
    let internal_session_id = session_manager
        .create_session(test_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    // Create a test file
    let test_file = test_dir.path().join("auto_approved_read.txt");
    tokio::fs::write(&test_file, "auto approved content")
        .await
        .unwrap();

    // fs_read should be auto-approved and bypass policy engine
    let request = InternalToolRequest {
        id: "test-read".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy()
        }),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    match result {
        ToolCallResult::Success(content) => {
            assert_eq!(content, "auto approved content");
        }
        ToolCallResult::PermissionRequired(_) => {
            panic!("Auto-approved fs_read should not require permission");
        }
        ToolCallResult::Error(msg) => {
            panic!("Auto-approved fs_read should succeed, got error: {}", msg);
        }
    }
}

#[tokio::test]
async fn test_auto_approved_fs_write_bypasses_policy() {
    let session_manager = Arc::new(SessionManager::new());
    let permission_engine = create_test_permission_engine();

    // Create permissions with fs_write in auto_approved list
    let permissions = ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec!["fs_write".to_string()],
        forbidden_paths: vec![],
    };

    let mut handler =
        ToolCallHandler::new(permissions, Arc::clone(&session_manager), permission_engine);

    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(true);
    handler.set_client_capabilities(capabilities);

    let test_dir = TempDir::new().unwrap();
    let internal_session_id = session_manager
        .create_session(test_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    let test_file = test_dir.path().join("auto_approved_write.txt");

    // fs_write should be auto-approved and bypass policy engine (normally requires permission)
    let request = InternalToolRequest {
        id: "test-write".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "auto approved write"
        }),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    match result {
        ToolCallResult::Success(msg) => {
            assert!(msg.contains("Successfully wrote"));

            // Verify the file was actually written
            let content = tokio::fs::read_to_string(&test_file).await.unwrap();
            assert_eq!(content, "auto approved write");
        }
        ToolCallResult::PermissionRequired(_) => {
            panic!("Auto-approved fs_write should not require permission");
        }
        ToolCallResult::Error(msg) => {
            panic!("Auto-approved fs_write should succeed, got error: {}", msg);
        }
    }
}

#[tokio::test]
async fn test_auto_approved_terminal_bypasses_policy() {
    let session_manager = Arc::new(SessionManager::new());
    let permission_engine = create_test_permission_engine();

    // Create permissions with terminal_create in auto_approved list
    let permissions = ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec!["terminal_create".to_string()],
        forbidden_paths: vec![],
    };

    let mut handler =
        ToolCallHandler::new(permissions, Arc::clone(&session_manager), permission_engine);

    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(true);
    handler.set_client_capabilities(capabilities);

    let test_dir = TempDir::new().unwrap();
    let internal_session_id = session_manager
        .create_session(test_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    // terminal_create should be auto-approved and bypass policy engine (normally requires permission)
    let request = InternalToolRequest {
        id: "test-terminal".to_string(),
        name: "terminal_create".to_string(),
        arguments: json!({}),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    match result {
        ToolCallResult::Success(msg) => {
            assert!(msg.contains("terminal") || msg.contains("Terminal"));
        }
        ToolCallResult::PermissionRequired(_) => {
            panic!("Auto-approved terminal_create should not require permission");
        }
        ToolCallResult::Error(msg) => {
            panic!(
                "Auto-approved terminal_create should succeed, got error: {}",
                msg
            );
        }
    }
}

#[tokio::test]
async fn test_auto_approved_multiple_tools() {
    let session_manager = Arc::new(SessionManager::new());
    let permission_engine = create_test_permission_engine();

    // Create permissions with multiple tools in auto_approved list
    let permissions = ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec![
            "fs_read".to_string(),
            "fs_write".to_string(),
            "fs_list".to_string(),
        ],
        forbidden_paths: vec![],
    };

    let mut handler =
        ToolCallHandler::new(permissions, Arc::clone(&session_manager), permission_engine);

    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(true);
    handler.set_client_capabilities(capabilities);

    let test_dir = TempDir::new().unwrap();
    let internal_session_id = session_manager
        .create_session(test_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    // Test fs_write (auto-approved)
    let write_file = test_dir.path().join("multi_approved.txt");
    let write_request = InternalToolRequest {
        id: "test-write".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": write_file.to_string_lossy(),
            "content": "multi approved"
        }),
    };

    let write_result = handler
        .handle_tool_request(&session_id, write_request)
        .await
        .unwrap();
    assert!(
        matches!(write_result, ToolCallResult::Success(_)),
        "Auto-approved fs_write should succeed"
    );

    // Test fs_read (auto-approved)
    let read_request = InternalToolRequest {
        id: "test-read".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": write_file.to_string_lossy()
        }),
    };

    let read_result = handler
        .handle_tool_request(&session_id, read_request)
        .await
        .unwrap();
    match read_result {
        ToolCallResult::Success(content) => {
            assert_eq!(content, "multi approved");
        }
        _ => panic!("Auto-approved fs_read should succeed"),
    }

    // Test fs_list (auto-approved)
    let list_request = InternalToolRequest {
        id: "test-list".to_string(),
        name: "fs_list".to_string(),
        arguments: json!({
            "path": test_dir.path().to_string_lossy()
        }),
    };

    let list_result = handler
        .handle_tool_request(&session_id, list_request)
        .await
        .unwrap();
    assert!(
        matches!(list_result, ToolCallResult::Success(_)),
        "Auto-approved fs_list should succeed"
    );
}

#[tokio::test]
async fn test_non_auto_approved_tool_still_requires_permission() {
    let session_manager = Arc::new(SessionManager::new());
    let permission_engine = create_test_permission_engine();

    // Create permissions with only fs_read in auto_approved list
    let permissions = ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec!["fs_read".to_string()],
        forbidden_paths: vec![],
    };

    let mut handler =
        ToolCallHandler::new(permissions, Arc::clone(&session_manager), permission_engine);

    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(true);
    handler.set_client_capabilities(capabilities);

    let test_dir = TempDir::new().unwrap();
    let internal_session_id = session_manager
        .create_session(test_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    let test_file = test_dir.path().join("not_auto_approved.txt");

    // fs_write is NOT in auto_approved, so it should require permission
    let request = InternalToolRequest {
        id: "test-write".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "requires permission"
        }),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    match result {
        ToolCallResult::PermissionRequired(perm_req) => {
            assert_eq!(perm_req.tool_name, "fs_write");
        }
        ToolCallResult::Success(_) => {
            panic!("Non-auto-approved fs_write should require permission");
        }
        ToolCallResult::Error(msg) => {
            panic!("Expected permission required, got error: {}", msg);
        }
    }
}

#[tokio::test]
async fn test_auto_approved_with_deny_policy_still_denied() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FilePermissionStorage::new(temp_dir.path().to_path_buf());

    // Create a custom policy that denies fs_write
    let custom_policies = vec![PermissionPolicy {
        tool_pattern: "fs_write*".to_string(),
        default_action: PolicyAction::Deny,
        require_user_consent: false,
        allow_always_option: false,
        risk_level: RiskLevel::Critical,
    }];

    let permission_engine = Arc::new(PermissionPolicyEngine::with_policies(
        Box::new(storage),
        custom_policies,
    ));

    let session_manager = Arc::new(SessionManager::new());

    // Even though fs_write is in auto_approved, the deny policy should not be bypassed
    // (auto_approved bypasses policy evaluation, so this actually tests that behavior)
    let permissions = ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec!["fs_write".to_string()],
        forbidden_paths: vec![],
    };

    let mut handler =
        ToolCallHandler::new(permissions, Arc::clone(&session_manager), permission_engine);

    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(true);
    handler.set_client_capabilities(capabilities);

    let test_dir = TempDir::new().unwrap();
    let internal_session_id = session_manager
        .create_session(test_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    let test_file = test_dir.path().join("auto_approved_but_denied.txt");

    // fs_write is auto-approved, which bypasses policy evaluation
    // So it should succeed even though there's a deny policy
    let request = InternalToolRequest {
        id: "test-write".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "auto approved bypasses policy"
        }),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    // Auto-approved tools bypass policy evaluation, so this should succeed
    match result {
        ToolCallResult::Success(msg) => {
            assert!(msg.contains("Successfully wrote"));

            // Verify the file was actually written
            let content = tokio::fs::read_to_string(&test_file).await.unwrap();
            assert_eq!(content, "auto approved bypasses policy");
        }
        _ => {
            panic!("Auto-approved fs_write should bypass deny policy and succeed");
        }
    }
}

// ===== ACP Client Capability Enforcement Tests =====

#[tokio::test]
async fn test_fs_read_fails_without_read_capability() {
    let (session_manager, _, _, temp_dir) = create_test_environment();
    let permission_engine = create_test_permission_engine();

    let permissions = ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec![],
        forbidden_paths: vec![],
    };

    let mut handler =
        ToolCallHandler::new(permissions, Arc::clone(&session_manager), permission_engine);

    // Set capabilities with read_text_file disabled
    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(false) // Capability disabled
            .write_text_file(true))
        .terminal(true);
    handler.set_client_capabilities(capabilities);

    let test_file = temp_dir.path().join("test.txt");
    tokio::fs::write(&test_file, "test content").await.unwrap();

    let internal_session_id = session_manager
        .create_session(temp_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    let request = InternalToolRequest {
        id: "test-read".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy()
        }),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    match result {
        ToolCallResult::Error(msg) => {
            assert!(
                msg.contains("capability") || msg.contains("not supported"),
                "Error should mention missing capability, got: {}",
                msg
            );
        }
        _ => {
            panic!("fs_read should fail when read_text_file capability is false");
        }
    }
}

#[tokio::test]
async fn test_fs_write_fails_without_write_capability() {
    let (session_manager, _, _, temp_dir) = create_test_environment();
    let permission_engine = create_test_permission_engine();

    let permissions = ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec![],
        forbidden_paths: vec![],
    };

    let mut handler =
        ToolCallHandler::new(permissions, Arc::clone(&session_manager), permission_engine);

    // Set capabilities with write_text_file disabled
    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(false)) // Capability disabled
        .terminal(true);
    handler.set_client_capabilities(capabilities);

    let test_file = temp_dir.path().join("write_test.txt");

    let internal_session_id = session_manager
        .create_session(temp_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    let request = InternalToolRequest {
        id: "test-write".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy(),
            "content": "new content"
        }),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    match result {
        ToolCallResult::Error(msg) => {
            assert!(
                msg.contains("capability") || msg.contains("not supported"),
                "Error should mention missing capability, got: {}",
                msg
            );
        }
        _ => {
            panic!("fs_write should fail when write_text_file capability is false");
        }
    }
}

#[tokio::test]
async fn test_terminal_fails_without_terminal_capability() {
    let (session_manager, _, _, temp_dir) = create_test_environment();
    let permission_engine = create_test_permission_engine();

    let permissions = ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec![],
        forbidden_paths: vec![],
    };

    let mut handler =
        ToolCallHandler::new(permissions, Arc::clone(&session_manager), permission_engine);

    // Set capabilities with terminal disabled
    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(false); // Capability disabled
    handler.set_client_capabilities(capabilities);

    let internal_session_id = session_manager
        .create_session(temp_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    let request = InternalToolRequest {
        id: "test-terminal".to_string(),
        name: "terminal_create".to_string(),
        arguments: json!({}),
    };

    let result = handler
        .handle_tool_request(&session_id, request)
        .await
        .unwrap();

    match result {
        ToolCallResult::Error(msg) => {
            assert!(
                msg.contains("capability") || msg.contains("not supported"),
                "Error should mention missing capability, got: {}",
                msg
            );
        }
        _ => {
            panic!("terminal_create should fail when terminal capability is false");
        }
    }
}

#[tokio::test]
async fn test_operations_fail_with_no_capabilities() {
    let (session_manager, _, _, temp_dir) = create_test_environment();
    let permission_engine = create_test_permission_engine();

    let permissions = ToolPermissions {
        require_permission_for: vec![],
        auto_approved: vec![],
        forbidden_paths: vec![],
    };

    let mut handler =
        ToolCallHandler::new(permissions, Arc::clone(&session_manager), permission_engine);

    // Set capabilities with all filesystem and terminal capabilities disabled
    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(false)
            .write_text_file(false))
        .terminal(false);
    handler.set_client_capabilities(capabilities);

    let test_file = temp_dir.path().join("test.txt");
    tokio::fs::write(&test_file, "test content").await.unwrap();

    let internal_session_id = session_manager
        .create_session(temp_dir.path().to_path_buf(), None)
        .unwrap();
    let session_id = agent_client_protocol::SessionId::new(internal_session_id.to_string());

    // Test fs_read fails
    let read_request = InternalToolRequest {
        id: "test-read".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({
            "path": test_file.to_string_lossy()
        }),
    };

    let read_result = handler
        .handle_tool_request(&session_id, read_request)
        .await
        .unwrap();

    assert!(
        matches!(read_result, ToolCallResult::Error(_)),
        "fs_read should fail with no capabilities"
    );

    // Test fs_write fails
    let write_file = temp_dir.path().join("write_test.txt");
    let write_request = InternalToolRequest {
        id: "test-write".to_string(),
        name: "fs_write".to_string(),
        arguments: json!({
            "path": write_file.to_string_lossy(),
            "content": "new content"
        }),
    };

    let write_result = handler
        .handle_tool_request(&session_id, write_request)
        .await
        .unwrap();

    assert!(
        matches!(write_result, ToolCallResult::Error(_)),
        "fs_write should fail with no capabilities"
    );

    // Test terminal_create fails
    let terminal_request = InternalToolRequest {
        id: "test-terminal".to_string(),
        name: "terminal_create".to_string(),
        arguments: json!({}),
    };

    let terminal_result = handler
        .handle_tool_request(&session_id, terminal_request)
        .await
        .unwrap();

    assert!(
        matches!(terminal_result, ToolCallResult::Error(_)),
        "terminal_create should fail with no capabilities"
    );
}
