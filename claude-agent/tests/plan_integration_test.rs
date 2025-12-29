//! Integration tests for plan generation and update functionality
//!
//! These tests verify that plan generation and updates work correctly
//! across the full agent system, including:
//! - TodoWrite to ACP Plan conversion
//! - Plan storage and retrieval via PlanManager
//! - Plan update notifications
//! - Plan entry status updates
//! - Integration with agent sessions

mod common;

use agent_client_protocol::SessionUpdate;
use claude_agent::{
    agent::{ClaudeAgent, NewSessionRequest},
    config::AgentConfig,
    plan::{todowrite_to_acp_plan, todowrite_to_agent_plan, PlanEntryStatus, Priority},
};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Create a test agent with default configuration
async fn create_test_agent() -> Arc<ClaudeAgent> {
    let config = AgentConfig::default();
    let agent = ClaudeAgent::new(config).await.unwrap();
    Arc::new(agent)
}

#[tokio::test]
async fn test_todowrite_conversion_basic() {
    // Test basic TodoWrite to ACP Plan conversion
    let todowrite_params = serde_json::json!({
        "todos": [
            {
                "content": "First task",
                "status": "pending",
                "activeForm": "Doing first task"
            },
            {
                "content": "Second task",
                "status": "pending",
                "activeForm": "Doing second task"
            }
        ]
    });

    let acp_plan = todowrite_to_acp_plan(&todowrite_params).unwrap();

    assert_eq!(acp_plan.entries.len(), 2);
    assert_eq!(acp_plan.entries[0].content, "First task");
    assert_eq!(acp_plan.entries[1].content, "Second task");

    // Verify status mapping
    let status_json = serde_json::to_value(&acp_plan.entries[0].status).unwrap();
    assert_eq!(status_json, "pending");
}

#[tokio::test]
async fn test_todowrite_conversion_with_status_progression() {
    // Test TodoWrite conversion with different statuses
    let todowrite_params = serde_json::json!({
        "todos": [
            {
                "content": "Completed task",
                "status": "completed",
                "activeForm": "Completing task"
            },
            {
                "content": "Active task",
                "status": "in_progress",
                "activeForm": "Working on active task"
            },
            {
                "content": "Pending task",
                "status": "pending",
                "activeForm": "Will do pending task"
            }
        ]
    });

    let acp_plan = todowrite_to_acp_plan(&todowrite_params).unwrap();

    assert_eq!(acp_plan.entries.len(), 3);

    // Verify completed status
    let status_0_json = serde_json::to_value(&acp_plan.entries[0].status).unwrap();
    assert_eq!(status_0_json, "completed");
    let priority_0_json = serde_json::to_value(&acp_plan.entries[0].priority).unwrap();
    assert_eq!(priority_0_json, "low");

    // Verify in_progress status and activeForm becomes content
    assert_eq!(acp_plan.entries[1].content, "Working on active task");
    let status_1_json = serde_json::to_value(&acp_plan.entries[1].status).unwrap();
    assert_eq!(status_1_json, "in_progress");
    let priority_1_json = serde_json::to_value(&acp_plan.entries[1].priority).unwrap();
    assert_eq!(priority_1_json, "high");

    // Verify pending status
    let status_2_json = serde_json::to_value(&acp_plan.entries[2].status).unwrap();
    assert_eq!(status_2_json, "pending");
    let priority_2_json = serde_json::to_value(&acp_plan.entries[2].priority).unwrap();
    assert_eq!(priority_2_json, "medium");
}

#[tokio::test]
async fn test_plan_storage_in_manager() {
    // Test that plans can be stored and retrieved from PlanManager
    let agent = create_test_agent().await;

    // Create a session
    let new_session_request = NewSessionRequest {
        cwd: std::path::PathBuf::from("/tmp"),
        meta: None,
        mcp_servers: vec![],
    };
    let session_response = agent.new_session(new_session_request).await.unwrap();
    let session_id = session_response.session_id;

    // Create and store a plan
    let todowrite_params = serde_json::json!({
        "todos": [
            {
                "content": "Test storage",
                "status": "pending",
                "activeForm": "Testing storage"
            }
        ]
    });

    let agent_plan = todowrite_to_agent_plan(&todowrite_params).unwrap();
    let entry_id = agent_plan.entries[0].id.clone();

    {
        let mut plan_manager = agent.plan_manager.write().await;
        plan_manager.set_plan(session_id.to_string(), agent_plan);
    }

    // Retrieve and verify the plan
    {
        let plan_manager = agent.plan_manager.read().await;
        let stored_plan = plan_manager.get_plan(&session_id.to_string()).unwrap();
        assert_eq!(stored_plan.entries.len(), 1);
        assert_eq!(stored_plan.entries[0].id, entry_id);
        assert_eq!(stored_plan.entries[0].content, "Test storage");
    }
}

#[tokio::test]
async fn test_plan_update_preserves_entry_ids() {
    // Test that updating a plan preserves entry IDs
    let agent = create_test_agent().await;

    let new_session_request = NewSessionRequest {
        cwd: std::path::PathBuf::from("/tmp"),
        meta: None,
        mcp_servers: vec![],
    };
    let session_response = agent.new_session(new_session_request).await.unwrap();
    let session_id = session_response.session_id;

    // Create initial plan
    let initial_params = serde_json::json!({
        "todos": [
            {
                "content": "Task 1",
                "status": "pending",
                "activeForm": "Doing Task 1"
            },
            {
                "content": "Task 2",
                "status": "pending",
                "activeForm": "Doing Task 2"
            }
        ]
    });

    let initial_plan = todowrite_to_agent_plan(&initial_params).unwrap();
    let task1_id = initial_plan.entries[0].id.clone();
    let task2_id = initial_plan.entries[1].id.clone();

    {
        let mut plan_manager = agent.plan_manager.write().await;
        plan_manager.set_plan(session_id.to_string(), initial_plan);
    }

    // Update plan with status changes
    let updated_params = serde_json::json!({
        "todos": [
            {
                "content": "Task 1",
                "status": "completed",
                "activeForm": "Doing Task 1"
            },
            {
                "content": "Task 2",
                "status": "in_progress",
                "activeForm": "Doing Task 2"
            }
        ]
    });

    let updated_plan = todowrite_to_agent_plan(&updated_params).unwrap();

    {
        let mut plan_manager = agent.plan_manager.write().await;
        plan_manager.update_plan(&session_id.to_string(), updated_plan);
    }

    // Verify IDs are preserved
    {
        let plan_manager = agent.plan_manager.read().await;
        let stored_plan = plan_manager.get_plan(&session_id.to_string()).unwrap();

        let task1 = stored_plan
            .entries
            .iter()
            .find(|e| e.content.contains("Task 1"))
            .unwrap();
        assert_eq!(task1.id, task1_id);
        assert_eq!(task1.status, PlanEntryStatus::Completed);

        let task2 = stored_plan
            .entries
            .iter()
            .find(|e| e.content.contains("Task 2"))
            .unwrap();
        assert_eq!(task2.id, task2_id);
        assert_eq!(task2.status, PlanEntryStatus::InProgress);
    }
}

#[tokio::test]
async fn test_plan_entry_status_update() {
    // Test updating individual plan entry status
    let agent = create_test_agent().await;

    let new_session_request = NewSessionRequest {
        cwd: std::path::PathBuf::from("/tmp"),
        meta: None,
        mcp_servers: vec![],
    };
    let session_response = agent.new_session(new_session_request).await.unwrap();
    let session_id = session_response.session_id;

    // Create a plan
    let todowrite_params = serde_json::json!({
        "todos": [
            {
                "content": "Task to update",
                "status": "pending",
                "activeForm": "Updating task"
            }
        ]
    });

    let agent_plan = todowrite_to_agent_plan(&todowrite_params).unwrap();
    let entry_id = agent_plan.entries[0].id.clone();

    {
        let mut plan_manager = agent.plan_manager.write().await;
        plan_manager.set_plan(session_id.to_string(), agent_plan);
    }

    // Update entry status
    {
        let mut plan_manager = agent.plan_manager.write().await;
        let result = plan_manager.update_plan_entry_status(
            &session_id.to_string(),
            &entry_id,
            PlanEntryStatus::InProgress,
        );
        assert!(result, "Status update should succeed");
    }

    // Verify the update
    {
        let plan_manager = agent.plan_manager.read().await;
        let stored_plan = plan_manager.get_plan(&session_id.to_string()).unwrap();
        let entry = stored_plan.get_entry(&entry_id).unwrap();
        assert_eq!(entry.status, PlanEntryStatus::InProgress);
    }
}

#[tokio::test]
async fn test_plan_update_notification_sent() {
    // Test that plan updates trigger notifications
    let agent = create_test_agent().await;

    let new_session_request = NewSessionRequest {
        cwd: std::path::PathBuf::from("/tmp"),
        meta: None,
        mcp_servers: vec![],
    };
    let session_response = agent.new_session(new_session_request).await.unwrap();
    let session_id = session_response.session_id;

    // Subscribe to notifications before creating plan
    let mut notification_receiver = agent.notification_sender.sender.subscribe();

    // Create a plan
    let todowrite_params = serde_json::json!({
        "todos": [
            {
                "content": "Notification test",
                "status": "pending",
                "activeForm": "Testing notifications"
            }
        ]
    });

    let agent_plan = todowrite_to_agent_plan(&todowrite_params).unwrap();
    let entry_id = agent_plan.entries[0].id.clone();

    {
        let mut plan_manager = agent.plan_manager.write().await;
        plan_manager.set_plan(session_id.to_string(), agent_plan);
    }

    // Send plan update notification
    let result = agent.send_plan_update(&session_id).await;
    assert!(result.is_ok(), "Sending plan update should succeed");

    // Verify notification was received
    let notification = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        notification_receiver.recv(),
    )
    .await;

    assert!(
        notification.is_ok(),
        "Should receive notification within timeout"
    );
    let notification = notification.unwrap().unwrap();

    // Verify it's a Plan update
    match notification.update {
        SessionUpdate::Plan(plan) => {
            assert_eq!(plan.entries.len(), 1);
            assert_eq!(plan.entries[0].content, "Notification test");
        }
        _ => panic!(
            "Expected Plan update notification, got: {:?}",
            notification.update
        ),
    }
}

#[tokio::test]
async fn test_plan_completion_tracking() {
    // Test that plan completion percentage is tracked correctly
    let agent = create_test_agent().await;

    let new_session_request = NewSessionRequest {
        cwd: std::path::PathBuf::from("/tmp"),
        meta: None,
        mcp_servers: vec![],
    };
    let session_response = agent.new_session(new_session_request).await.unwrap();
    let session_id = session_response.session_id;

    // Create a plan with multiple entries
    let todowrite_params = serde_json::json!({
        "todos": [
            {
                "content": "Task 1",
                "status": "pending",
                "activeForm": "Doing Task 1"
            },
            {
                "content": "Task 2",
                "status": "pending",
                "activeForm": "Doing Task 2"
            },
            {
                "content": "Task 3",
                "status": "pending",
                "activeForm": "Doing Task 3"
            }
        ]
    });

    let agent_plan = todowrite_to_agent_plan(&todowrite_params).unwrap();
    let entry_ids: Vec<String> = agent_plan.entries.iter().map(|e| e.id.clone()).collect();

    {
        let mut plan_manager = agent.plan_manager.write().await;
        plan_manager.set_plan(session_id.to_string(), agent_plan);
    }

    // Verify initial completion
    {
        let plan_manager = agent.plan_manager.read().await;
        let plan = plan_manager.get_plan(&session_id.to_string()).unwrap();
        assert_eq!(plan.completion_percentage(), 0.0);
        assert!(!plan.is_complete());
    }

    // Complete one task
    {
        let mut plan_manager = agent.plan_manager.write().await;
        plan_manager.update_plan_entry_status(
            &session_id.to_string(),
            &entry_ids[0],
            PlanEntryStatus::Completed,
        );
    }

    // Verify 33% completion
    {
        let plan_manager = agent.plan_manager.read().await;
        let plan = plan_manager.get_plan(&session_id.to_string()).unwrap();
        assert!((plan.completion_percentage() - 0.333).abs() < 0.01);
        assert!(!plan.is_complete());
    }

    // Complete remaining tasks
    {
        let mut plan_manager = agent.plan_manager.write().await;
        plan_manager.update_plan_entry_status(
            &session_id.to_string(),
            &entry_ids[1],
            PlanEntryStatus::Completed,
        );
        plan_manager.update_plan_entry_status(
            &session_id.to_string(),
            &entry_ids[2],
            PlanEntryStatus::Completed,
        );
    }

    // Verify 100% completion
    {
        let plan_manager = agent.plan_manager.read().await;
        let plan = plan_manager.get_plan(&session_id.to_string()).unwrap();
        assert_eq!(plan.completion_percentage(), 1.0);
        assert!(plan.is_complete());
    }
}

#[tokio::test]
async fn test_plan_next_pending_entry() {
    // Test that next pending entry returns highest priority
    let agent = create_test_agent().await;

    let new_session_request = NewSessionRequest {
        cwd: std::path::PathBuf::from("/tmp"),
        meta: None,
        mcp_servers: vec![],
    };
    let session_response = agent.new_session(new_session_request).await.unwrap();
    let session_id = session_response.session_id;

    // Create a plan with different priorities
    let todowrite_params = serde_json::json!({
        "todos": [
            {
                "content": "Low priority task",
                "status": "completed",
                "activeForm": "Low priority"
            },
            {
                "content": "Medium priority task",
                "status": "pending",
                "activeForm": "Medium priority"
            },
            {
                "content": "High priority task",
                "status": "in_progress",
                "activeForm": "High priority"
            }
        ]
    });

    let agent_plan = todowrite_to_agent_plan(&todowrite_params).unwrap();

    {
        let mut plan_manager = agent.plan_manager.write().await;
        plan_manager.set_plan(session_id.to_string(), agent_plan);
    }

    // Get next pending entry
    {
        let plan_manager = agent.plan_manager.read().await;
        let plan = plan_manager.get_plan(&session_id.to_string()).unwrap();
        let next = plan.next_pending_entry();
        assert!(next.is_some());
        assert_eq!(next.unwrap().content, "Medium priority task");
    }
}

#[tokio::test]
async fn test_plan_manager_cleanup() {
    // Test that plan manager can clean up expired sessions
    let agent = create_test_agent().await;

    // Create multiple sessions
    let session1 = agent
        .new_session(NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            meta: None,
            mcp_servers: vec![],
        })
        .await
        .unwrap()
        .session_id;

    let session2 = agent
        .new_session(NewSessionRequest {
            cwd: std::path::PathBuf::from("/tmp"),
            meta: None,
            mcp_servers: vec![],
        })
        .await
        .unwrap()
        .session_id;

    // Store plans for both sessions
    let todowrite_params = serde_json::json!({
        "todos": [
            {
                "content": "Test",
                "status": "pending",
                "activeForm": "Testing"
            }
        ]
    });

    let plan1 = todowrite_to_agent_plan(&todowrite_params).unwrap();
    let plan2 = todowrite_to_agent_plan(&todowrite_params).unwrap();

    {
        let mut plan_manager = agent.plan_manager.write().await;
        plan_manager.set_plan(session1.to_string(), plan1);
        plan_manager.set_plan(session2.to_string(), plan2);
    }

    // Cleanup, keeping only session1
    {
        let mut plan_manager = agent.plan_manager.write().await;
        plan_manager.cleanup_expired_plans(&[session1.to_string()]);
    }

    // Verify session1 plan still exists, session2 plan is removed
    {
        let plan_manager = agent.plan_manager.read().await;
        assert!(plan_manager.get_plan(&session1.to_string()).is_some());
        assert!(plan_manager.get_plan(&session2.to_string()).is_none());
    }
}

#[tokio::test]
async fn test_invalid_todowrite_params() {
    // Test error handling for invalid TodoWrite parameters
    let invalid_params = serde_json::json!({
        "not_todos": []
    });

    let result = todowrite_to_acp_plan(&invalid_params);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("todos"));
}

#[tokio::test]
async fn test_missing_todo_fields() {
    // Test error handling for missing required fields
    let invalid_params = serde_json::json!({
        "todos": [
            {
                "content": "Task without status"
            }
        ]
    });

    let result = todowrite_to_acp_plan(&invalid_params);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_send_plan_update_without_plan() {
    // Test that sending plan update without a plan returns error
    let agent = create_test_agent().await;

    let new_session_request = NewSessionRequest {
        cwd: std::path::PathBuf::from("/tmp"),
        meta: None,
        mcp_servers: vec![],
    };
    let session_response = agent.new_session(new_session_request).await.unwrap();
    let session_id = session_response.session_id;

    // Try to send plan update without creating a plan first
    let result = agent.send_plan_update(&session_id).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No plan found"));
}
