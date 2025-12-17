//! Tests for concurrent session operations
//!
//! This test suite verifies that the SessionManager can handle multiple concurrent operations safely:
//! 1. Creating multiple sessions concurrently
//! 2. Reading from multiple sessions concurrently
//! 3. Updating multiple sessions concurrently
//! 4. Deleting multiple sessions concurrently
//! 5. Mixed operations (create, read, update, delete) running concurrently
//! 6. Concurrent operations with persistence enabled

use claude_agent::session::{Message, MessageRole, SessionManager};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::task::JoinSet;

/// Helper to create a session manager with a temporary storage directory
fn create_test_session_manager() -> (Arc<SessionManager>, TempDir) {
    let temp_dir = tempfile::tempdir().unwrap();
    let manager =
        Arc::new(SessionManager::new().with_storage_path(Some(temp_dir.path().join("sessions"))));

    (manager, temp_dir)
}

#[tokio::test]
async fn test_concurrent_session_creation() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    // Create 10 sessions concurrently
    let mut create_tasks = JoinSet::new();
    for i in 0..10 {
        let manager_clone = manager.clone();
        let cwd_clone = cwd.clone();
        create_tasks.spawn(async move {
            let session_id = manager_clone.create_session(cwd_clone, None).unwrap();
            (i, session_id)
        });
    }

    let mut session_ids = Vec::new();
    while let Some(result) = create_tasks.join_next().await {
        let (_index, session_id) = result.unwrap();
        session_ids.push(session_id);
    }

    // Verify all sessions were created
    assert_eq!(session_ids.len(), 10);

    // Verify all session IDs are unique
    let mut unique_ids: Vec<String> = session_ids.iter().map(|id| id.to_string()).collect();
    unique_ids.sort();
    unique_ids.dedup();
    assert_eq!(unique_ids.len(), 10, "All session IDs should be unique");

    // Verify all sessions exist
    for session_id in &session_ids {
        let session = manager.get_session(session_id).unwrap();
        assert!(session.is_some(), "Session {} should exist", session_id);
    }
}

#[tokio::test]
async fn test_concurrent_session_reads() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    // Create a session
    let session_id = manager.create_session(cwd, None).unwrap();

    // Add a message
    manager
        .update_session(&session_id, |session| {
            session.add_message(Message::new(MessageRole::User, "Test message".to_string()));
        })
        .unwrap();

    // Read the session concurrently from 20 threads
    let mut read_tasks = JoinSet::new();
    for i in 0..20 {
        let manager_clone = manager.clone();
        let session_id_clone = session_id;
        read_tasks.spawn(async move {
            let session = manager_clone.get_session(&session_id_clone).unwrap();
            (i, session)
        });
    }

    let mut read_count = 0;
    while let Some(result) = read_tasks.join_next().await {
        let (_index, session) = result.unwrap();
        assert!(session.is_some(), "Session should exist for all reads");
        let session = session.unwrap();
        assert_eq!(
            session.context.len(),
            1,
            "All reads should see the same message count"
        );
        read_count += 1;
    }

    assert_eq!(read_count, 20, "All read operations should complete");
}

#[tokio::test]
async fn test_concurrent_session_updates() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    // Create multiple sessions
    let mut session_ids = Vec::new();
    for _ in 0..5 {
        let session_id = manager.create_session(cwd.clone(), None).unwrap();
        session_ids.push(session_id);
    }

    // Update all sessions concurrently, each adding 3 messages
    let mut update_tasks = JoinSet::new();
    for (i, session_id) in session_ids.iter().enumerate() {
        let manager_clone = manager.clone();
        let session_id_clone = *session_id;
        update_tasks.spawn(async move {
            for j in 0..3 {
                manager_clone
                    .update_session(&session_id_clone, |session| {
                        session.add_message(Message::new(
                            MessageRole::User,
                            format!("Session {} message {}", i, j),
                        ));
                    })
                    .unwrap();
            }
            session_id_clone
        });
    }

    while let Some(result) = update_tasks.join_next().await {
        result.unwrap();
    }

    // Verify all sessions have 3 messages
    for session_id in &session_ids {
        let session = manager.get_session(session_id).unwrap().unwrap();
        assert_eq!(
            session.context.len(),
            3,
            "Session {} should have 3 messages",
            session_id
        );
    }
}

#[tokio::test]
async fn test_concurrent_session_removal() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    // Create 10 sessions
    let mut session_ids = Vec::new();
    for _ in 0..10 {
        let session_id = manager.create_session(cwd.clone(), None).unwrap();
        session_ids.push(session_id);
    }

    // Remove 5 sessions concurrently
    let mut remove_tasks = JoinSet::new();
    for session_id in session_ids.iter().take(5) {
        let manager_clone = manager.clone();
        let session_id_clone = *session_id;
        remove_tasks.spawn(async move {
            let removed = manager_clone.remove_session(&session_id_clone).unwrap();
            (session_id_clone, removed)
        });
    }

    let mut removed_count = 0;
    while let Some(result) = remove_tasks.join_next().await {
        let (_session_id, removed) = result.unwrap();
        assert!(removed.is_some(), "Removed session should be returned");
        removed_count += 1;
    }

    assert_eq!(removed_count, 5, "Should have removed 5 sessions");

    // Verify remaining sessions still exist
    for session_id in session_ids.iter().skip(5) {
        let session = manager.get_session(session_id).unwrap();
        assert!(
            session.is_some(),
            "Remaining session {} should still exist",
            session_id
        );
    }

    // Verify removed sessions don't exist
    for session_id in session_ids.iter().take(5) {
        let session = manager.get_session(session_id).unwrap();
        assert!(
            session.is_none(),
            "Removed session {} should not exist",
            session_id
        );
    }
}

#[tokio::test]
async fn test_concurrent_mixed_operations() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    // Create initial sessions
    let mut initial_session_ids = Vec::new();
    for _ in 0..5 {
        let session_id = manager.create_session(cwd.clone(), None).unwrap();
        initial_session_ids.push(session_id);
    }

    let mut tasks = JoinSet::new();

    // Task group 1: Create new sessions
    for i in 0..3 {
        let manager_clone = manager.clone();
        let cwd_clone = cwd.clone();
        tasks.spawn(async move {
            let session_id = manager_clone.create_session(cwd_clone, None).unwrap();
            (format!("create_{}", i), session_id)
        });
    }

    // Task group 2: Update existing sessions
    for (i, session_id) in initial_session_ids.iter().take(3).enumerate() {
        let manager_clone = manager.clone();
        let session_id_clone = *session_id;
        tasks.spawn(async move {
            manager_clone
                .update_session(&session_id_clone, |session| {
                    session.add_message(Message::new(MessageRole::User, format!("Update {}", i)));
                })
                .unwrap();
            (format!("update_{}", i), session_id_clone)
        });
    }

    // Task group 3: Read existing sessions
    for (i, session_id) in initial_session_ids.iter().enumerate() {
        let manager_clone = manager.clone();
        let session_id_clone = *session_id;
        tasks.spawn(async move {
            let _session = manager_clone.get_session(&session_id_clone).unwrap();
            (format!("read_{}", i), session_id_clone)
        });
    }

    // Task group 4: Remove sessions
    for (i, session_id) in initial_session_ids.iter().skip(3).enumerate() {
        let manager_clone = manager.clone();
        let session_id_clone = *session_id;
        tasks.spawn(async move {
            let _removed = manager_clone.remove_session(&session_id_clone).unwrap();
            (format!("remove_{}", i), session_id_clone)
        });
    }

    // Wait for all tasks to complete
    let mut completed_tasks = 0;
    while let Some(result) = tasks.join_next().await {
        result.unwrap();
        completed_tasks += 1;
    }

    // We should have completed: 3 creates + 3 updates + 5 reads + 2 removes = 13 tasks
    assert_eq!(completed_tasks, 13, "All mixed operations should complete");

    // Verify the final state
    let remaining_sessions = manager.list_sessions().unwrap();
    // Initial 5 + 3 new - 2 removed = 6 sessions
    assert_eq!(
        remaining_sessions.len(),
        6,
        "Should have 6 sessions remaining"
    );
}

#[tokio::test]
async fn test_concurrent_list_operations() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    // Create sessions
    for _ in 0..5 {
        manager.create_session(cwd.clone(), None).unwrap();
    }

    // List sessions concurrently from multiple threads
    let mut list_tasks = JoinSet::new();
    for i in 0..10 {
        let manager_clone = manager.clone();
        list_tasks.spawn(async move {
            let sessions = manager_clone.list_sessions().unwrap();
            (i, sessions)
        });
    }

    let mut list_count = 0;
    while let Some(result) = list_tasks.join_next().await {
        let (_index, sessions) = result.unwrap();
        assert_eq!(
            sessions.len(),
            5,
            "All list operations should see 5 sessions"
        );
        list_count += 1;
    }

    assert_eq!(list_count, 10, "All list operations should complete");
}

#[tokio::test]
async fn test_concurrent_operations_with_persistence() {
    let temp_dir = tempfile::tempdir().unwrap();
    let manager =
        Arc::new(SessionManager::new().with_storage_path(Some(temp_dir.path().join("sessions"))));
    let cwd = std::env::current_dir().unwrap();

    // Create sessions concurrently
    let mut create_tasks = JoinSet::new();
    for i in 0..5 {
        let manager_clone = manager.clone();
        let cwd_clone = cwd.clone();
        create_tasks.spawn(async move {
            let session_id = manager_clone.create_session(cwd_clone, None).unwrap();
            (i, session_id)
        });
    }

    let mut session_ids = Vec::new();
    while let Some(result) = create_tasks.join_next().await {
        let (_index, session_id) = result.unwrap();
        session_ids.push(session_id);
    }

    // Update sessions concurrently
    let mut update_tasks = JoinSet::new();
    for (i, session_id) in session_ids.iter().enumerate() {
        let manager_clone = manager.clone();
        let session_id_clone = *session_id;
        update_tasks.spawn(async move {
            manager_clone
                .update_session(&session_id_clone, |session| {
                    session.add_message(Message::new(
                        MessageRole::User,
                        format!("Persistent message {}", i),
                    ));
                })
                .unwrap();
        });
    }

    while let Some(result) = update_tasks.join_next().await {
        result.unwrap();
    }

    // Verify all session files exist on disk
    for session_id in &session_ids {
        let session_file = temp_dir
            .path()
            .join("sessions")
            .join(format!("{}.json", session_id));
        assert!(
            session_file.exists(),
            "Session file for {} should exist",
            session_id
        );
    }

    // Create a new manager (simulated restart) and verify sessions can be loaded
    let new_manager =
        Arc::new(SessionManager::new().with_storage_path(Some(temp_dir.path().join("sessions"))));

    // Load all sessions concurrently
    let mut load_tasks = JoinSet::new();
    for session_id in &session_ids {
        let manager_clone = new_manager.clone();
        let session_id_clone = *session_id;
        load_tasks.spawn(async move {
            let session = manager_clone.get_session(&session_id_clone).unwrap();
            (session_id_clone, session)
        });
    }

    let mut loaded_count = 0;
    while let Some(result) = load_tasks.join_next().await {
        let (session_id, session) = result.unwrap();
        assert!(
            session.is_some(),
            "Session {} should be loaded from disk",
            session_id
        );
        let session = session.unwrap();
        assert_eq!(session.context.len(), 1, "Session should have 1 message");
        loaded_count += 1;
    }

    assert_eq!(loaded_count, 5, "All sessions should be loaded");
}

#[tokio::test]
async fn test_concurrent_updates_same_session() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    // Create a single session
    let session_id = manager.create_session(cwd, None).unwrap();

    // Update the same session concurrently from multiple threads
    let mut update_tasks = JoinSet::new();
    for i in 0..10 {
        let manager_clone = manager.clone();
        let session_id_clone = session_id;
        update_tasks.spawn(async move {
            manager_clone
                .update_session(&session_id_clone, |session| {
                    session.add_message(Message::new(
                        MessageRole::User,
                        format!("Concurrent update {}", i),
                    ));
                })
                .unwrap();
            i
        });
    }

    while let Some(result) = update_tasks.join_next().await {
        result.unwrap();
    }

    // Verify the session has all 10 messages
    let session = manager.get_session(&session_id).unwrap().unwrap();
    assert_eq!(
        session.context.len(),
        10,
        "Session should have all 10 messages from concurrent updates"
    );
}

#[tokio::test]
async fn test_concurrent_create_and_immediate_read() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    // Create and immediately read sessions concurrently
    let mut tasks = JoinSet::new();
    for i in 0..5 {
        let manager_clone = manager.clone();
        let cwd_clone = cwd.clone();
        tasks.spawn(async move {
            // Create session
            let session_id = manager_clone.create_session(cwd_clone, None).unwrap();

            // Immediately try to read it
            let session = manager_clone.get_session(&session_id).unwrap();
            assert!(
                session.is_some(),
                "Session {} should be immediately readable",
                i
            );

            session_id
        });
    }

    let mut created_session_ids = Vec::new();
    while let Some(result) = tasks.join_next().await {
        let session_id = result.unwrap();
        created_session_ids.push(session_id);
    }

    assert_eq!(created_session_ids.len(), 5);
}

#[tokio::test]
async fn test_concurrent_operations_stress_test() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    // Create a larger number of sessions to stress test the system
    let num_sessions = 50;
    let mut tasks = JoinSet::new();

    // Phase 1: Create many sessions concurrently
    for i in 0..num_sessions {
        let manager_clone = manager.clone();
        let cwd_clone = cwd.clone();
        tasks.spawn(async move {
            let session_id = manager_clone.create_session(cwd_clone, None).unwrap();
            (i, session_id)
        });
    }

    let mut session_ids = Vec::new();
    while let Some(result) = tasks.join_next().await {
        let (_index, session_id) = result.unwrap();
        session_ids.push(session_id);
    }

    assert_eq!(session_ids.len(), num_sessions);

    // Phase 2: Perform mixed operations on all sessions concurrently
    let mut tasks = JoinSet::new();

    for session_id in &session_ids {
        let manager_clone = manager.clone();
        let session_id_clone = *session_id;

        // Update
        tasks.spawn(async move {
            manager_clone
                .update_session(&session_id_clone, |session| {
                    session.add_message(Message::new(
                        MessageRole::User,
                        "Stress test message".to_string(),
                    ));
                })
                .unwrap();
        });

        // Read
        let manager_clone = manager.clone();
        tasks.spawn(async move {
            let _session = manager_clone.get_session(&session_id_clone).unwrap();
        });
    }

    let mut completed = 0;
    while let Some(result) = tasks.join_next().await {
        result.unwrap();
        completed += 1;
    }

    // Should have completed num_sessions updates + num_sessions reads
    assert_eq!(completed, num_sessions * 2);

    // Verify all sessions still exist and have the message
    for session_id in &session_ids {
        let session = manager.get_session(session_id).unwrap().unwrap();
        assert_eq!(session.context.len(), 1);
    }
}

#[tokio::test]
async fn test_concurrent_delete_and_recreate() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    // Create initial sessions
    let mut session_ids = Vec::new();
    for _ in 0..5 {
        let session_id = manager.create_session(cwd.clone(), None).unwrap();
        session_ids.push(session_id);
    }

    // Delete and recreate sessions concurrently
    let mut tasks = JoinSet::new();
    for (i, old_session_id) in session_ids.iter().enumerate() {
        let manager_clone = manager.clone();
        let old_session_id_clone = *old_session_id;
        let cwd_clone = cwd.clone();

        tasks.spawn(async move {
            // Delete old session
            let removed = manager_clone.remove_session(&old_session_id_clone).unwrap();
            assert!(removed.is_some(), "Session {} should be removed", i);

            // Small delay to simulate real-world timing
            tokio::time::sleep(Duration::from_millis(10)).await;

            // Create new session
            let new_session_id = manager_clone.create_session(cwd_clone, None).unwrap();
            (i, old_session_id_clone, new_session_id)
        });
    }

    let mut new_session_ids = Vec::new();
    while let Some(result) = tasks.join_next().await {
        let (_index, old_id, new_id) = result.unwrap();
        assert_ne!(old_id, new_id, "New session should have different ID");
        new_session_ids.push(new_id);
    }

    // Verify old sessions are gone
    for old_id in &session_ids {
        let session = manager.get_session(old_id).unwrap();
        assert!(session.is_none(), "Old session should not exist");
    }

    // Verify new sessions exist
    for new_id in &new_session_ids {
        let session = manager.get_session(new_id).unwrap();
        assert!(session.is_some(), "New session should exist");
    }

    // Verify total count
    let all_sessions = manager.list_sessions().unwrap();
    assert_eq!(all_sessions.len(), 5);
}
