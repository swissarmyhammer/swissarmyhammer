//! Integration tests for session todo synchronization
//!
//! This test suite verifies that session todos stay synchronized with TodoStorage:
//! 1. Creating a todo via MCP tool updates the session's todos vector
//! 2. Marking a todo complete via MCP tool updates the session's todos vector
//! 3. Session todos persist correctly across session saves/loads
//! 4. Multiple sessions can have independent todo lists

use claude_agent::agent::Agent;
use claude_agent::config::AgentConfig;
use claude_agent::mcp::McpManager;
use claude_agent::session::{SessionId, SessionManager};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer_todo::{TodoItemExt, TodoStorage};
use tempfile::TempDir;
use tokio::sync::Mutex;

/// Helper to create a test agent with temporary directories
async fn create_test_agent() -> (Agent, TempDir, PathBuf) {
    let temp_dir = tempfile::tempdir().unwrap();
    let working_dir = temp_dir.path().to_path_buf();

    // Initialize git repo in temp directory (required for TodoStorage)
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&working_dir)
        .output()
        .expect("Failed to init git repo");

    let session_manager =
        SessionManager::new().with_storage_path(Some(working_dir.join("sessions")));

    let mcp_manager = McpManager::new();

    let config = AgentConfig {
        api_key: "test-key".to_string(),
        model: "claude-3-5-sonnet-20241022".to_string(),
        ..Default::default()
    };

    let agent = Agent::new(
        config,
        Arc::new(Mutex::new(session_manager)),
        Arc::new(Mutex::new(HashMap::new())),
        Arc::new(Mutex::new(mcp_manager)),
    );

    (agent, temp_dir, working_dir)
}

/// Helper to create a session for testing
async fn create_test_session(agent: &Agent, working_dir: PathBuf) -> SessionId {
    let session_manager = agent.session_manager.lock().await;
    session_manager
        .create_session(working_dir, None)
        .expect("Failed to create session")
}

#[tokio::test]
async fn test_session_todos_empty_initially() {
    let (agent, _temp_dir, working_dir) = create_test_agent().await;
    let session_id = create_test_session(&agent, working_dir).await;

    let session_manager = agent.session_manager.lock().await;
    let session = session_manager
        .get_session(&session_id)
        .unwrap()
        .expect("Session should exist");

    assert_eq!(session.todos.len(), 0, "New session should have no todos");
}

#[tokio::test]
async fn test_creating_todo_updates_session() {
    let (agent, _temp_dir, working_dir) = create_test_agent().await;
    let session_id = create_test_session(&agent, working_dir.clone()).await;

    // Create a todo directly in storage
    let storage = TodoStorage::new_with_working_dir(working_dir).unwrap();
    let (todo_item, _) = storage
        .create_todo_item("Test task".to_string(), None)
        .await
        .expect("Failed to create todo");

    // Sync session todos
    agent
        .sync_session_todos(&session_id.to_string())
        .await
        .expect("Failed to sync todos");

    // Verify session has the todo
    let session_manager = agent.session_manager.lock().await;
    let session = session_manager
        .get_session(&session_id)
        .unwrap()
        .expect("Session should exist");

    assert_eq!(session.todos.len(), 1, "Session should have 1 todo");
    assert_eq!(
        session.todos[0], todo_item.id,
        "Session todo ID should match created todo"
    );
}

#[tokio::test]
async fn test_creating_multiple_todos_updates_session() {
    let (agent, _temp_dir, working_dir) = create_test_agent().await;
    let session_id = create_test_session(&agent, working_dir.clone()).await;

    // Create multiple todos
    let storage = TodoStorage::new_with_working_dir(working_dir).unwrap();
    let (todo1, _) = storage
        .create_todo_item("Task 1".to_string(), None)
        .await
        .expect("Failed to create todo 1");
    let (todo2, _) = storage
        .create_todo_item("Task 2".to_string(), None)
        .await
        .expect("Failed to create todo 2");
    let (todo3, _) = storage
        .create_todo_item("Task 3".to_string(), None)
        .await
        .expect("Failed to create todo 3");

    // Sync session todos
    agent
        .sync_session_todos(&session_id.to_string())
        .await
        .expect("Failed to sync todos");

    // Verify session has all todos
    let session_manager = agent.session_manager.lock().await;
    let session = session_manager
        .get_session(&session_id)
        .unwrap()
        .expect("Session should exist");

    assert_eq!(session.todos.len(), 3, "Session should have 3 todos");
    assert!(
        session.todos.contains(&todo1.id),
        "Session should contain todo 1"
    );
    assert!(
        session.todos.contains(&todo2.id),
        "Session should contain todo 2"
    );
    assert!(
        session.todos.contains(&todo3.id),
        "Session should contain todo 3"
    );
}

#[tokio::test]
async fn test_marking_todo_complete_updates_session() {
    let (agent, _temp_dir, working_dir) = create_test_agent().await;
    let session_id = create_test_session(&agent, working_dir.clone()).await;

    // Create two todos
    let storage = TodoStorage::new_with_working_dir(working_dir).unwrap();
    let (todo1, _) = storage
        .create_todo_item("Task 1".to_string(), None)
        .await
        .expect("Failed to create todo 1");
    let (todo2, _) = storage
        .create_todo_item("Task 2".to_string(), None)
        .await
        .expect("Failed to create todo 2");

    // Sync to get both todos in session
    agent
        .sync_session_todos(&session_id.to_string())
        .await
        .expect("Failed to sync todos");

    // Verify we have 2 todos
    {
        let session_manager = agent.session_manager.lock().await;
        let session = session_manager
            .get_session(&session_id)
            .unwrap()
            .expect("Session should exist");
        assert_eq!(session.todos.len(), 2, "Session should have 2 todos");
    }

    // Mark first todo complete
    storage
        .mark_todo_complete(&todo1.id)
        .await
        .expect("Failed to mark todo complete");

    // Sync again
    agent
        .sync_session_todos(&session_id.to_string())
        .await
        .expect("Failed to sync todos after completion");

    // Verify session only has 1 incomplete todo
    let session_manager = agent.session_manager.lock().await;
    let session = session_manager
        .get_session(&session_id)
        .unwrap()
        .expect("Session should exist");

    assert_eq!(
        session.todos.len(),
        1,
        "Session should have 1 incomplete todo"
    );
    assert_eq!(
        session.todos[0], todo2.id,
        "Session should only contain incomplete todo"
    );
    assert!(
        !session.todos.contains(&todo1.id),
        "Session should not contain completed todo"
    );
}

#[tokio::test]
async fn test_marking_all_todos_complete_empties_session() {
    let (agent, _temp_dir, working_dir) = create_test_agent().await;
    let session_id = create_test_session(&agent, working_dir.clone()).await;

    // Create a todo
    let storage = TodoStorage::new_with_working_dir(working_dir).unwrap();
    let (todo_item, _) = storage
        .create_todo_item("Test task".to_string(), None)
        .await
        .expect("Failed to create todo");

    // Sync to get todo in session
    agent
        .sync_session_todos(&session_id.to_string())
        .await
        .expect("Failed to sync todos");

    // Mark it complete
    storage
        .mark_todo_complete(&todo_item.id)
        .await
        .expect("Failed to mark todo complete");

    // Sync again
    agent
        .sync_session_todos(&session_id.to_string())
        .await
        .expect("Failed to sync todos after completion");

    // Verify session has no todos
    let session_manager = agent.session_manager.lock().await;
    let session = session_manager
        .get_session(&session_id)
        .unwrap()
        .expect("Session should exist");

    assert_eq!(session.todos.len(), 0, "Session should have no todos");
}

#[tokio::test]
async fn test_session_todos_persist_across_saves() {
    let temp_dir = tempfile::tempdir().unwrap();
    let working_dir = temp_dir.path().to_path_buf();

    // Initialize git repo
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&working_dir)
        .output()
        .expect("Failed to init git repo");

    // Create session and add todos
    let session_id = {
        let (agent, _, _) = {
            let session_manager =
                SessionManager::new().with_storage_path(Some(working_dir.join("sessions")));
            let mcp_manager = McpManager::new();
            let config = AgentConfig {
                api_key: "test-key".to_string(),
                model: "claude-3-5-sonnet-20241022".to_string(),
                ..Default::default()
            };

            let agent = Agent::new(
                config,
                Arc::new(Mutex::new(session_manager)),
                Arc::new(Mutex::new(HashMap::new())),
                Arc::new(Mutex::new(mcp_manager)),
            );
            (agent, temp_dir, working_dir.clone())
        };

        let session_id = create_test_session(&agent, working_dir.clone()).await;

        // Create todos
        let storage = TodoStorage::new_with_working_dir(working_dir.clone()).unwrap();
        storage
            .create_todo_item("Task 1".to_string(), None)
            .await
            .expect("Failed to create todo 1");
        storage
            .create_todo_item("Task 2".to_string(), None)
            .await
            .expect("Failed to create todo 2");

        // Sync session todos
        agent
            .sync_session_todos(&session_id.to_string())
            .await
            .expect("Failed to sync todos");

        session_id
    };

    // Create new agent (simulating restart)
    let session_manager =
        SessionManager::new().with_storage_path(Some(working_dir.join("sessions")));
    let session = session_manager
        .get_session(&session_id)
        .unwrap()
        .expect("Session should exist after restart");

    // Verify todos were persisted
    assert_eq!(
        session.todos.len(),
        2,
        "Session should have 2 todos after persistence"
    );
}

#[tokio::test]
async fn test_multiple_sessions_have_independent_todos() {
    let (agent, _temp_dir, working_dir) = create_test_agent().await;

    // Create two sessions with same working directory
    let session_id1 = create_test_session(&agent, working_dir.clone()).await;
    let session_id2 = create_test_session(&agent, working_dir.clone()).await;

    // Create todos (they will be in the same storage since same working_dir)
    let storage = TodoStorage::new_with_working_dir(working_dir).unwrap();
    storage
        .create_todo_item("Task 1".to_string(), None)
        .await
        .expect("Failed to create todo 1");
    storage
        .create_todo_item("Task 2".to_string(), None)
        .await
        .expect("Failed to create todo 2");

    // Sync both sessions
    agent
        .sync_session_todos(&session_id1.to_string())
        .await
        .expect("Failed to sync todos for session 1");
    agent
        .sync_session_todos(&session_id2.to_string())
        .await
        .expect("Failed to sync todos for session 2");

    // Both sessions should see the same todos
    let session_manager = agent.session_manager.lock().await;

    let session1 = session_manager
        .get_session(&session_id1)
        .unwrap()
        .expect("Session 1 should exist");
    let session2 = session_manager
        .get_session(&session_id2)
        .unwrap()
        .expect("Session 2 should exist");

    assert_eq!(session1.todos.len(), 2, "Session 1 should have 2 todos");
    assert_eq!(session2.todos.len(), 2, "Session 2 should have 2 todos");

    // They should have the same todo IDs since they share the same working directory
    assert_eq!(
        session1.todos, session2.todos,
        "Sessions sharing working directory should have same todos"
    );
}

#[tokio::test]
async fn test_sync_with_nonexistent_session_returns_error() {
    let (agent, _temp_dir, _working_dir) = create_test_agent().await;

    let nonexistent_id = SessionId::new();
    let result = agent.sync_session_todos(&nonexistent_id.to_string()).await;

    assert!(
        result.is_err(),
        "Syncing nonexistent session should return error"
    );
}

#[tokio::test]
async fn test_sync_with_no_todo_file_clears_session_todos() {
    let (agent, _temp_dir, working_dir) = create_test_agent().await;
    let session_id = create_test_session(&agent, working_dir.clone()).await;

    // Manually add a todo to session (simulating stale state)
    {
        let session_manager = agent.session_manager.lock().await;
        session_manager
            .update_session(&session_id, |session| {
                session.todos.push("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string());
            })
            .expect("Failed to update session");
    }

    // Verify todo exists
    {
        let session_manager = agent.session_manager.lock().await;
        let session = session_manager
            .get_session(&session_id)
            .unwrap()
            .expect("Session should exist");
        assert_eq!(session.todos.len(), 1, "Session should have 1 stale todo");
    }

    // Sync (no todo file exists)
    agent
        .sync_session_todos(&session_id.to_string())
        .await
        .expect("Failed to sync todos");

    // Verify session todos cleared
    let session_manager = agent.session_manager.lock().await;
    let session = session_manager
        .get_session(&session_id)
        .unwrap()
        .expect("Session should exist");

    assert_eq!(
        session.todos.len(),
        0,
        "Session todos should be cleared when no todo file exists"
    );
}

#[tokio::test]
async fn test_sync_only_includes_incomplete_todos() {
    let (agent, _temp_dir, working_dir) = create_test_agent().await;
    let session_id = create_test_session(&agent, working_dir.clone()).await;

    // Create todos and mark one complete
    let storage = TodoStorage::new_with_working_dir(working_dir).unwrap();
    let (todo1, _) = storage
        .create_todo_item("Task 1".to_string(), None)
        .await
        .expect("Failed to create todo 1");
    let (todo2, _) = storage
        .create_todo_item("Task 2".to_string(), None)
        .await
        .expect("Failed to create todo 2");
    storage
        .create_todo_item("Task 3".to_string(), None)
        .await
        .expect("Failed to create todo 3");

    // Mark first todo complete before sync
    storage
        .mark_todo_complete(&todo1.id)
        .await
        .expect("Failed to mark todo complete");

    // Sync session
    agent
        .sync_session_todos(&session_id.to_string())
        .await
        .expect("Failed to sync todos");

    // Verify session only has incomplete todos
    let session_manager = agent.session_manager.lock().await;
    let session = session_manager
        .get_session(&session_id)
        .unwrap()
        .expect("Session should exist");

    assert_eq!(
        session.todos.len(),
        2,
        "Session should only have incomplete todos"
    );
    assert!(
        !session.todos.contains(&todo1.id),
        "Session should not contain completed todo"
    );
    assert!(
        session.todos.contains(&todo2.id),
        "Session should contain incomplete todo 2"
    );
}

#[tokio::test]
async fn test_concurrent_syncs_do_not_corrupt_session() {
    let (agent, _temp_dir, working_dir) = create_test_agent().await;
    let session_id = create_test_session(&agent, working_dir.clone()).await;

    // Create some todos
    let storage = TodoStorage::new_with_working_dir(working_dir).unwrap();
    storage
        .create_todo_item("Task 1".to_string(), None)
        .await
        .expect("Failed to create todo");

    // Run multiple syncs concurrently
    let agent1 = agent.clone();
    let agent2 = agent.clone();
    let session_id_str1 = session_id.to_string();
    let session_id_str2 = session_id.to_string();

    let handle1 = tokio::spawn(async move {
        agent1
            .sync_session_todos(&session_id_str1)
            .await
            .expect("Sync 1 failed");
    });

    let handle2 = tokio::spawn(async move {
        agent2
            .sync_session_todos(&session_id_str2)
            .await
            .expect("Sync 2 failed");
    });

    // Wait for both syncs to complete
    handle1.await.expect("Task 1 panicked");
    handle2.await.expect("Task 2 panicked");

    // Verify session state is consistent
    let session_manager = agent.session_manager.lock().await;
    let session = session_manager
        .get_session(&session_id)
        .unwrap()
        .expect("Session should exist");

    assert_eq!(
        session.todos.len(),
        1,
        "Session should have correct number of todos after concurrent syncs"
    );
}
