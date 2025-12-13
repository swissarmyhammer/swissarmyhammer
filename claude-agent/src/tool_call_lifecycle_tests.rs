//! Comprehensive tests for tool call status lifecycle and ACP-compliant notifications
//!
//! This module tests the complete tool call status reporting implementation to ensure
//! full ACP compliance with proper notification sequences for all scenarios.

#[cfg(test)]
mod tests {
    use crate::agent::NotificationSender;
    use crate::permissions::{FilePermissionStorage, PermissionPolicyEngine};
    use crate::session::SessionManager;
    use crate::tool_types::ToolCallStatus;
    use crate::tools::ToolCallHandler;
    use crate::ToolPermissions;
    use agent_client_protocol::{SessionId, SessionNotification, SessionUpdate};
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::broadcast;

    /// Helper to create a test tool call handler with notification sender
    async fn create_test_handler() -> (ToolCallHandler, broadcast::Receiver<SessionNotification>) {
        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec!["test_tool".to_string()],
            forbidden_paths: vec![],
        };
        let session_manager = Arc::new(SessionManager::new());
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = FilePermissionStorage::new(temp_dir.path().to_path_buf());
        let permission_engine = Arc::new(PermissionPolicyEngine::new(Box::new(storage)));

        let mut handler = ToolCallHandler::new(permissions, session_manager, permission_engine);
        let (sender, receiver) = NotificationSender::new(32);
        handler.set_notification_sender(sender);

        (handler, receiver)
    }

    #[tokio::test]
    async fn test_complete_tool_call_lifecycle_success() {
        let (handler, mut receiver) = create_test_handler().await;
        let session_id = SessionId("test_session_123".into());
        let tool_name = "test_tool";
        let arguments = json!({"param": "value"});

        // 1. Create tool call - should send initial ToolCall notification
        let report = handler
            .create_tool_call_report(&session_id, tool_name, &arguments)
            .await;
        let tool_call_id = report.tool_call_id.clone();

        // Verify initial notification
        let notification = receiver
            .recv()
            .await
            .expect("Should receive initial notification");
        match notification.update {
            SessionUpdate::ToolCall(tool_call) => {
                assert_eq!(tool_call.id.0.as_ref(), tool_call_id.as_str());
                assert_eq!(
                    tool_call.status,
                    agent_client_protocol::ToolCallStatus::Pending
                );
                assert_eq!(tool_call.title, "Test tool");
                assert_eq!(tool_call.kind, agent_client_protocol::ToolKind::Other);
            }
            _ => panic!("Expected ToolCall notification"),
        }

        // 2. Update to in_progress - should send ToolCallUpdate notification
        let _updated_report = handler
            .update_tool_call_report(&session_id, &tool_call_id, |report| {
                report.update_status(ToolCallStatus::InProgress);
            })
            .await
            .expect("Should update successfully");

        // Verify progress notification
        let notification = receiver
            .recv()
            .await
            .expect("Should receive progress notification");
        match notification.update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.id.0.as_ref(), tool_call_id.as_str());
                assert_eq!(
                    update.fields.status,
                    Some(agent_client_protocol::ToolCallStatus::InProgress)
                );
            }
            _ => panic!("Expected ToolCallUpdate notification"),
        }

        // 3. Complete tool call - should send final ToolCallUpdate notification
        let output = json!({"result": "success"});
        let _completed_report = handler
            .complete_tool_call_report(&session_id, &tool_call_id, Some(output.clone()))
            .await
            .expect("Should complete successfully");

        // Verify completion notification
        let notification = receiver
            .recv()
            .await
            .expect("Should receive completion notification");
        match notification.update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.id.0.as_ref(), tool_call_id.as_str());
                assert_eq!(
                    update.fields.status,
                    Some(agent_client_protocol::ToolCallStatus::Completed)
                );
                assert_eq!(update.fields.raw_output, Some(output));
            }
            _ => panic!("Expected ToolCallUpdate completion notification"),
        }

        // Verify tool call was removed from active tracking
        let active_report = handler
            .update_tool_call_report(&session_id, &tool_call_id, |_| {})
            .await;
        assert!(
            active_report.is_none(),
            "Tool call should be removed from active tracking"
        );
    }

    #[tokio::test]
    async fn test_tool_call_failure_lifecycle() {
        let (handler, mut receiver) = create_test_handler().await;
        let session_id = SessionId("test_session_456".into());
        let tool_name = "failing_tool";
        let arguments = json!({"will_fail": true});

        // Create tool call
        let report = handler
            .create_tool_call_report(&session_id, tool_name, &arguments)
            .await;
        let tool_call_id = report.tool_call_id.clone();

        // Consume initial notification
        let _ = receiver
            .recv()
            .await
            .expect("Should receive initial notification");

        // Update to in_progress
        handler
            .update_tool_call_report(&session_id, &tool_call_id, |report| {
                report.update_status(ToolCallStatus::InProgress);
            })
            .await;

        // Consume progress notification
        let _ = receiver
            .recv()
            .await
            .expect("Should receive progress notification");

        // Fail tool call with error
        let error_output = json!({"error": "Tool execution failed", "code": 500});
        let _failed_report = handler
            .fail_tool_call_report(&session_id, &tool_call_id, Some(error_output.clone()))
            .await
            .expect("Should fail successfully");

        // Verify failure notification
        let notification = receiver
            .recv()
            .await
            .expect("Should receive failure notification");
        match notification.update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.id.0.as_ref(), tool_call_id.as_str());
                assert_eq!(
                    update.fields.status,
                    Some(agent_client_protocol::ToolCallStatus::Failed)
                );
                assert_eq!(update.fields.raw_output, Some(error_output));
            }
            _ => panic!("Expected ToolCallUpdate failure notification"),
        }
    }

    #[tokio::test]
    async fn test_tool_call_cancellation() {
        let (handler, mut receiver) = create_test_handler().await;
        let session_id = SessionId("test_session_789".into());
        let tool_name = "long_running_tool";
        let arguments = json!({"duration": 3600});

        // Create and start tool call
        let report = handler
            .create_tool_call_report(&session_id, tool_name, &arguments)
            .await;
        let tool_call_id = report.tool_call_id.clone();

        // Consume initial notification
        let _ = receiver
            .recv()
            .await
            .expect("Should receive initial notification");

        // Update to in_progress
        handler
            .update_tool_call_report(&session_id, &tool_call_id, |report| {
                report.update_status(ToolCallStatus::InProgress);
            })
            .await;

        // Consume progress notification
        let _ = receiver
            .recv()
            .await
            .expect("Should receive progress notification");

        // Cancel tool call
        let _cancelled_report = handler
            .cancel_tool_call_report(&session_id, &tool_call_id)
            .await
            .expect("Should cancel successfully");

        // Verify cancellation notification
        let notification = receiver
            .recv()
            .await
            .expect("Should receive cancellation notification");
        match notification.update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.id.0.as_ref(), tool_call_id.as_str());
                assert_eq!(
                    update.fields.status,
                    Some(agent_client_protocol::ToolCallStatus::Failed)
                );
            }
            _ => panic!("Expected ToolCallUpdate cancellation notification"),
        }
    }

    #[tokio::test]
    async fn test_concurrent_tool_execution() {
        let (handler, mut receiver) = create_test_handler().await;
        let session_id = SessionId("test_session_concurrent".into());

        // Create multiple tool calls concurrently
        let mut tasks = vec![];
        for i in 0..3 {
            let handler_clone = handler.clone();
            let session_clone = session_id.clone();
            let task = tokio::spawn(async move {
                let tool_name = format!("concurrent_tool_{}", i);
                let arguments = json!({"index": i});

                let report = handler_clone
                    .create_tool_call_report(&session_clone, &tool_name, &arguments)
                    .await;
                let tool_call_id = report.tool_call_id.clone();

                // Simulate some work
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

                // Complete the tool
                handler_clone
                    .complete_tool_call_report(
                        &session_clone,
                        &tool_call_id,
                        Some(json!({"result": i})),
                    )
                    .await
            });
            tasks.push(task);
        }

        // Wait for all tasks to complete
        let results = futures::future::join_all(tasks).await;
        for result in results {
            assert!(
                result.is_ok(),
                "All concurrent tasks should complete successfully"
            );
        }

        // Verify we received the expected number of notifications
        // Each tool call should generate 2 notifications: initial + completion
        let mut notification_count = 0;
        while receiver.try_recv().is_ok() {
            notification_count += 1;
        }
        assert_eq!(
            notification_count, 6,
            "Should receive 6 notifications total (3 initial + 3 completion)"
        );
    }

    #[tokio::test]
    async fn test_tool_call_with_content_and_locations() {
        let (handler, mut receiver) = create_test_handler().await;
        let session_id = SessionId("test_session_content".into());
        let tool_name = "file_operation_tool";
        let arguments = json!({"file_path": "/test/file.txt"});

        // Create tool call
        let report = handler
            .create_tool_call_report(&session_id, tool_name, &arguments)
            .await;
        let tool_call_id = report.tool_call_id.clone();

        // Consume initial notification
        let _ = receiver
            .recv()
            .await
            .expect("Should receive initial notification");

        // Update with content and location
        handler
            .update_tool_call_report(&session_id, &tool_call_id, |report| {
                report.update_status(ToolCallStatus::InProgress);
                report.add_content(crate::tool_types::ToolCallContent::Content {
                    content: agent_client_protocol::ContentBlock::Text(
                        agent_client_protocol::TextContent {
                            text: "Processing file...".to_string(),
                            annotations: None,
                            meta: None,
                        },
                    ),
                });
                report.add_location(crate::tool_types::ToolCallLocation {
                    path: "/test/file.txt".to_string(),
                    line: Some(42),
                });
            })
            .await;

        // Verify notification with content and location
        let notification = receiver
            .recv()
            .await
            .expect("Should receive update notification");
        match notification.update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.id.0.as_ref(), tool_call_id.as_str());
                assert!(update.fields.content.is_some(), "Should include content");
                assert!(
                    update.fields.locations.is_some(),
                    "Should include locations"
                );

                let locations = update.fields.locations.unwrap();
                // We add a location in the test, but the tool might also extract one from arguments
                assert!(!locations.is_empty(), "Should have at least one location");
                // Find our manually added location
                let our_location = locations
                    .iter()
                    .find(|l| l.line == Some(42))
                    .expect("Should find our location");
                assert_eq!(our_location.path.to_str().unwrap(), "/test/file.txt");
                assert_eq!(our_location.line, Some(42));
            }
            _ => panic!("Expected ToolCallUpdate notification with content"),
        }
    }

    #[tokio::test]
    async fn test_notification_sender_failure_resilience() {
        // Create handler but don't set notification sender
        let permissions = ToolPermissions {
            require_permission_for: vec![],
            auto_approved: vec!["test_tool".to_string()],
            forbidden_paths: vec![],
        };

        let session_manager = Arc::new(SessionManager::new());
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = FilePermissionStorage::new(temp_dir.path().to_path_buf());
        let permission_engine = Arc::new(PermissionPolicyEngine::new(Box::new(storage)));
        let handler = ToolCallHandler::new(permissions, session_manager, permission_engine);
        let session_id = SessionId("test_session_no_sender".into());

        // Tool call operations should still work without notification sender
        let report = handler
            .create_tool_call_report(&session_id, "test_tool", &json!({}))
            .await;
        assert_eq!(report.status, ToolCallStatus::Pending);

        let updated = handler
            .update_tool_call_report(&session_id, &report.tool_call_id, |r| {
                r.update_status(ToolCallStatus::InProgress);
            })
            .await;
        assert!(updated.is_some());

        let completed = handler
            .complete_tool_call_report(&session_id, &report.tool_call_id, None)
            .await;
        assert!(completed.is_some());
    }

    #[tokio::test]
    async fn test_terminal_embedding_in_tool_call() {
        let (handler, mut receiver) = create_test_handler().await;
        let session_id = SessionId("test_session_terminal".into());
        let tool_name = "execute_command";
        let arguments = json!({"command": "echo", "args": ["hello"]});

        // Create tool call
        let report = handler
            .create_tool_call_report(&session_id, tool_name, &arguments)
            .await;
        let tool_call_id = report.tool_call_id.clone();

        // Consume initial notification
        let _ = receiver
            .recv()
            .await
            .expect("Should receive initial notification");

        // Embed a terminal in the tool call
        let terminal_id = "term_01234567890ABCDEFGHIJK".to_string();
        let result = handler
            .embed_terminal_in_tool_call(&session_id, &tool_call_id, terminal_id.clone())
            .await;
        assert!(result.is_ok(), "Terminal embedding should succeed");

        // Verify notification with terminal content
        let notification = receiver
            .recv()
            .await
            .expect("Should receive terminal embedding notification");
        match notification.update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.id.0.as_ref(), tool_call_id.as_str());
                assert!(update.fields.content.is_some(), "Should include content");

                let content = update.fields.content.unwrap();
                assert_eq!(content.len(), 1, "Should have one content item");

                match &content[0] {
                    agent_client_protocol::ToolCallContent::Terminal { terminal_id: tid } => {
                        assert_eq!(
                            tid.0.as_ref(),
                            terminal_id.as_str(),
                            "Terminal ID should match"
                        );
                    }
                    _ => panic!("Expected Terminal content type"),
                }
            }
            _ => panic!("Expected ToolCallUpdate notification with terminal content"),
        }
    }

    #[tokio::test]
    async fn test_terminal_embedding_with_nonexistent_tool_call() {
        let (handler, _receiver) = create_test_handler().await;
        let session_id = SessionId("test_session_invalid".into());
        let nonexistent_tool_call_id = "call_nonexistent";
        let terminal_id = "term_01234567890ABCDEFGHIJK".to_string();

        // Attempt to embed terminal in nonexistent tool call
        let result = handler
            .embed_terminal_in_tool_call(&session_id, nonexistent_tool_call_id, terminal_id)
            .await;

        assert!(
            result.is_err(),
            "Embedding should fail for nonexistent tool call"
        );
        match result {
            Err(crate::AgentError::ToolExecution(msg)) => {
                assert!(
                    msg.contains("not found"),
                    "Error should indicate tool call not found"
                );
            }
            _ => panic!("Expected ToolExecution error"),
        }
    }

    #[tokio::test]
    async fn test_multiple_terminals_in_tool_call() {
        let (handler, mut receiver) = create_test_handler().await;
        let session_id = SessionId("test_session_multi_terminal".into());
        let tool_name = "parallel_execute";
        let arguments = json!({"commands": ["echo hello", "echo world"]});

        // Create tool call
        let report = handler
            .create_tool_call_report(&session_id, tool_name, &arguments)
            .await;
        let tool_call_id = report.tool_call_id.clone();

        // Consume initial notification
        let _ = receiver
            .recv()
            .await
            .expect("Should receive initial notification");

        // Embed first terminal
        let terminal_id_1 = "term_01111111111111111111111".to_string();
        handler
            .embed_terminal_in_tool_call(&session_id, &tool_call_id, terminal_id_1.clone())
            .await
            .expect("First terminal embedding should succeed");

        // Consume first terminal notification
        let _ = receiver
            .recv()
            .await
            .expect("Should receive first terminal notification");

        // Embed second terminal
        let terminal_id_2 = "term_02222222222222222222222".to_string();
        handler
            .embed_terminal_in_tool_call(&session_id, &tool_call_id, terminal_id_2.clone())
            .await
            .expect("Second terminal embedding should succeed");

        // Verify second notification includes both terminals
        let notification = receiver
            .recv()
            .await
            .expect("Should receive second terminal notification");
        match notification.update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.id.0.as_ref(), tool_call_id.as_str());
                let content = update.fields.content.expect("Should include content");
                assert_eq!(content.len(), 2, "Should have two terminal content items");

                // Verify both terminals are present
                let terminal_ids: Vec<String> = content
                    .iter()
                    .filter_map(|c| {
                        if let agent_client_protocol::ToolCallContent::Terminal { terminal_id } = c
                        {
                            Some(terminal_id.0.as_ref().to_string())
                        } else {
                            None
                        }
                    })
                    .collect();

                assert!(
                    terminal_ids.contains(&terminal_id_1),
                    "First terminal should be present"
                );
                assert!(
                    terminal_ids.contains(&terminal_id_2),
                    "Second terminal should be present"
                );
            }
            _ => panic!("Expected ToolCallUpdate with multiple terminals"),
        }
    }

    #[tokio::test]
    async fn test_execute_with_embedded_terminal() {
        let (handler, mut receiver) = create_test_handler().await;
        let tool_name = "bash_execute";
        let arguments = json!({"command": "echo test"});

        // Create a session first (required for terminal creation)
        let session_manager = handler.get_session_manager();
        let internal_session_id = session_manager
            .create_session(std::path::PathBuf::from("/tmp"), None)
            .expect("Should create session");

        // Internal session ID already has proper ACP format
        let session_id = SessionId(internal_session_id.to_string().into());

        // Create tool call
        let report = handler
            .create_tool_call_report(&session_id, tool_name, &arguments)
            .await;
        let tool_call_id = report.tool_call_id.clone();

        // Consume initial notification
        let _ = receiver
            .recv()
            .await
            .expect("Should receive initial notification");

        // Execute with embedded terminal
        let params = crate::terminal_manager::TerminalCreateParams {
            session_id: session_id.0.to_string(),
            command: "echo".to_string(),
            args: Some(vec!["test".to_string()]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let terminal_id = handler
            .execute_with_embedded_terminal(&session_id, &tool_call_id, params)
            .await
            .expect("Execute with embedded terminal should succeed");

        // Verify terminal ID format (ACP-compliant with term_ prefix)
        assert!(
            terminal_id.starts_with("term_"),
            "Terminal ID should have term_ prefix"
        );

        // Verify notification with embedded terminal
        let notification = receiver
            .recv()
            .await
            .expect("Should receive terminal embedding notification");
        match notification.update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.id.0.as_ref(), tool_call_id.as_str());
                let content = update.fields.content.expect("Should include content");
                assert_eq!(content.len(), 1, "Should have one terminal content item");

                match &content[0] {
                    agent_client_protocol::ToolCallContent::Terminal { terminal_id: tid } => {
                        assert_eq!(
                            tid.0.as_ref(),
                            terminal_id.as_str(),
                            "Terminal ID should match returned value"
                        );
                    }
                    _ => panic!("Expected Terminal content type"),
                }
            }
            _ => panic!("Expected ToolCallUpdate with terminal content"),
        }
    }

    #[tokio::test]
    async fn test_terminal_embedding_with_tool_call_completion() {
        let (handler, mut receiver) = create_test_handler().await;
        let session_id = SessionId("test_session_terminal_complete".into());
        let tool_name = "execute_with_result";
        let arguments = json!({"command": "ls"});

        // Create tool call
        let report = handler
            .create_tool_call_report(&session_id, tool_name, &arguments)
            .await;
        let tool_call_id = report.tool_call_id.clone();

        // Consume initial notification
        let _ = receiver
            .recv()
            .await
            .expect("Should receive initial notification");

        // Embed terminal
        let terminal_id = "term_01234567890ABCDEFGHIJK".to_string();
        handler
            .embed_terminal_in_tool_call(&session_id, &tool_call_id, terminal_id.clone())
            .await
            .expect("Terminal embedding should succeed");

        // Consume terminal embedding notification
        let _ = receiver
            .recv()
            .await
            .expect("Should receive terminal embedding notification");

        // Complete tool call
        let output = json!({"exit_code": 0, "terminal_id": terminal_id});
        handler
            .complete_tool_call_report(&session_id, &tool_call_id, Some(output.clone()))
            .await
            .expect("Should complete tool call");

        // Verify completion notification
        let notification = receiver
            .recv()
            .await
            .expect("Should receive completion notification");
        match notification.update {
            SessionUpdate::ToolCallUpdate(update) => {
                assert_eq!(update.id.0.as_ref(), tool_call_id.as_str());
                assert_eq!(
                    update.fields.status,
                    Some(agent_client_protocol::ToolCallStatus::Completed)
                );
                assert_eq!(update.fields.raw_output, Some(output));

                // Terminal content should still be present in the completed tool call
                let content = update.fields.content.expect("Should include content");
                assert!(
                    content.iter().any(|c| matches!(
                        c,
                        agent_client_protocol::ToolCallContent::Terminal { .. }
                    )),
                    "Terminal content should persist through completion"
                );
            }
            _ => panic!("Expected ToolCallUpdate completion notification"),
        }
    }
}
